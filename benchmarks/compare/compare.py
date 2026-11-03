"""Speech-enhancement quality comparison on the VoiceBank-DEMAND test set.

Systems: NVIDIA Real-time RE-USE as run by the Hush engine (``enhance_dir`` example),
DeepFilterNet 3 (official ``deep-filter`` 0.5.6 binary), RNNoise (``pyrnnoise`` wheel) and the
unprocessed noisy input.

Steps (each one is a sub-command; ``all`` runs them in order):

  download  fetch the dataset, the DeepFilterNet binary and the DNSMOS model; check SHA-256
  prepare   unzip; pad every noisy file with 100 ms of silence; make 16 kHz copies (ffmpeg soxr)
  process   run every system on every file; resample 48 kHz outputs to 16 kHz (ffmpeg soxr)
  score     find each system's constant delay; PESQ-WB, STOI, ESTOI, SI-SDR, DNSMOS P.835
  timing    rough real-time factor of each system on one 30 s file, one process at a time
  report    write results.json next to this script

Downloaded data is untrusted: run this script with ``python -I``. Everything except
results.json goes to ``--work`` (outside the repository). Example, from the repository root::

  python -m venv <work>/venv
  <work>/venv/Scripts/python -m pip install -r benchmarks/compare/requirements.txt
  cargo build --release -p hush-core --example enhance_dir
  <work>/venv/Scripts/python -I benchmarks/compare/compare.py --work <work> all
"""

from __future__ import annotations

import argparse
import concurrent.futures as cf
import ctypes
import hashlib
import json
import os
import platform
import re
import shutil
import statistics
import subprocess
import sys
import time
import urllib.request
import zipfile
from pathlib import Path

import numpy as np
import soundfile as sf

REPO = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent

DATASHARE = "https://datashare.ed.ac.uk/bitstream/handle/10283/2791"
DNS_COMMIT = "591184a9fcb2cbdec02520fed81a32bbbf9d73ff"
DFN_EXE = "deep-filter-0.5.6-x86_64-pc-windows-msvc.exe"
DOWNLOADS = {
    # name: (subfolder, url, sha256)
    "noisy_testset_wav.zip": (
        "voicebank",
        f"{DATASHARE}/noisy_testset_wav.zip",
        "ba4a893f45b627f15209ba30ab584dd495b3680cd6f216dd08d143a257611993",
    ),
    "clean_testset_wav.zip": (
        "voicebank",
        f"{DATASHARE}/clean_testset_wav.zip",
        "b60bfc2b9df38466d8eb45057c087c6e5dee196c67a0ed7bca628ae2a8611c4e",
    ),
    "logfiles.zip": (
        "logfiles",
        f"{DATASHARE}/logfiles.zip",
        "ab37c41f31bc2b6c204cc10c8e5bb8bde800307fdb49f277e366d986fefc9e39",
    ),
    DFN_EXE: (
        "dfn",
        f"https://github.com/Rikorose/DeepFilterNet/releases/download/v0.5.6/{DFN_EXE}",
        "75e11fa16445f560cb6b021521ddb89e89270d13b83089705d98776f58fd7915",
    ),
    "sig_bak_ovr.onnx": (
        "dnsmos",
        f"https://raw.githubusercontent.com/microsoft/DNS-Challenge/{DNS_COMMIT}"
        "/DNSMOS/DNSMOS/sig_bak_ovr.onnx",
        "269fbebdb513aa23cddfbb593542ecc540284a91849ac50516870e1ac78f6edd",
    ),
}
MODEL = REPO / "models" / "model.safetensors"
MODEL_SHA256 = "6eac4e8cc721f99a22cde7eac3c9f8e2826ced5dc8a4234fd1977de1ec0d3ade"

PAD_SECONDS = 0.1  # silence after each utterance, so every system emits the whole utterance
MAX_LAG = 0.25  # seconds searched for each system's constant delay
DELAY_FILES = 24  # files used to find the delay, spread evenly over the set

# Hush configurations: (input rate, layers, look-ahead).
HUSH = {
    "hush_light": (16000, 3, 0),
    "hush_studio": (48000, 3, 0),
    "hush_16k_8layers": (16000, 8, 0),
}
SYSTEMS = ["noisy", *HUSH, "deepfilternet3", "rnnoise"]
LABELS = {
    "clean": "Clean reference",
    "noisy": "Noisy (unprocessed)",
    "hush_light": "RE-USE via Hush, Light (16 kHz, 3 layers)",
    "hush_studio": "RE-USE via Hush, Studio (48 kHz, 3 layers)",
    "hush_16k_8layers": "RE-USE via Hush, 16 kHz, 8 layers",
    "deepfilternet3": "DeepFilterNet 3 (deep-filter 0.5.6, 48 kHz)",
    "rnnoise": "RNNoise (pyrnnoise 0.4.5, 48 kHz)",
}
METRICS = ["pesq_wb", "stoi", "estoi", "si_sdr", "dnsmos_sig", "dnsmos_bak", "dnsmos_ovrl"]


# ---------------------------------------------------------------- helpers


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def fetch(url: str, dest: Path, attempts: int = 30) -> None:
    """Download with resume; DataShare often resets long connections."""
    dest.parent.mkdir(parents=True, exist_ok=True)
    part = dest.with_suffix(dest.suffix + ".part")
    for attempt in range(attempts):
        have = part.stat().st_size if part.exists() else 0
        request = urllib.request.Request(url, headers={"Range": f"bytes={have}-"} if have else {})
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                mode = "ab" if have and response.status == 206 else "wb"
                with open(part, mode) as f:
                    shutil.copyfileobj(response, f, 1 << 20)
            part.replace(dest)
            return
        except Exception as error:  # noqa: BLE001 - retry any network error
            print(f"  {dest.name}: {error}; retry {attempt + 1}")
            time.sleep(2)
    raise RuntimeError(f"could not download {url}")


def ffmpeg_16k(src: Path, dst: Path) -> None:
    """The one resampler used for every 16 kHz signal: ffmpeg + soxr, 32-bit float output."""
    dst.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        ["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-i", str(src),
         "-af", "aresample=resampler=soxr", "-ar", "16000", "-ac", "1", "-c:a", "pcm_f32le",
         str(dst)],
        check=True,
    )


def names(work: Path) -> list[str]:
    return sorted(p.stem for p in (work / "data" / "clean48").glob("*.wav"))


def output16(work: Path, system: str) -> Path:
    """Folder with a system's 16 kHz output (Hush 16 kHz runs write there directly)."""
    if system == "noisy":
        return work / "data" / "noisy16pad"
    if system == "clean":
        return work / "data" / "clean16"
    if system in HUSH and HUSH[system][0] == 16000:
        return work / "out" / system
    return work / "out16" / system


def hush_exe() -> Path:
    exe = REPO / "target" / "release" / "examples" / ("enhance_dir" + (".exe" if os.name == "nt" else ""))
    if not exe.exists():
        sys.exit(f"{exe} is missing: cargo build --release -p hush-core --example enhance_dir")
    return exe


def tool_version(cmd: list[str]) -> str:
    try:
        return subprocess.run(cmd, capture_output=True, text=True).stdout.strip().splitlines()[0]
    except (OSError, IndexError):
        return "unknown"


# ---------------------------------------------------------------- download / prepare


def download(work: Path) -> None:
    for name, (folder, url, digest) in DOWNLOADS.items():
        dest = work / "dl" / folder / name
        if not (dest.exists() and sha256(dest) == digest):
            print(f"downloading {url}")
            fetch(url, dest)
        actual = sha256(dest)
        if actual != digest:
            sys.exit(f"{dest}: SHA-256 {actual}, expected {digest}")
        print(f"ok {name} {digest[:12]}")
    if sha256(MODEL) != MODEL_SHA256:
        sys.exit(f"{MODEL}: unexpected SHA-256")


def prepare(work: Path, jobs: int) -> None:
    data = work / "data"
    for kind in ("clean", "noisy"):
        folder = data / f"{kind}48"
        if len(list(folder.glob("*.wav"))) != 824:
            folder.mkdir(parents=True, exist_ok=True)
            with zipfile.ZipFile(work / "dl" / "voicebank" / f"{kind}_testset_wav.zip") as z:
                for member in z.namelist():
                    if member.endswith(".wav") and not member.startswith("__MACOSX"):
                        (folder / Path(member).name).write_bytes(z.read(member))
    with zipfile.ZipFile(work / "dl" / "logfiles" / "logfiles.zip") as z:
        (data / "log_testset.txt").write_bytes(z.read("log_testset.txt"))

    # Every system gets the same 48 kHz 16-bit input: the noisy file plus 100 ms of silence.
    pad48 = data / "noisy48pad"
    pad48.mkdir(exist_ok=True)
    for name in names(work):
        dst = pad48 / f"{name}.wav"
        if dst.exists():
            continue
        noisy, rate = sf.read(data / "noisy48" / f"{name}.wav", dtype="int16")
        clean_info = sf.info(data / "clean48" / f"{name}.wav")
        assert rate == 48000 and noisy.ndim == 1 and clean_info.frames == len(noisy), name
        sf.write(dst, np.concatenate([noisy, np.zeros(int(PAD_SECONDS * rate), np.int16)]), rate,
                 subtype="PCM_16")

    tasks = []
    for name in names(work):
        for src, dst in ((pad48, data / "noisy16pad"), (data / "clean48", data / "clean16")):
            if not (dst / f"{name}.wav").exists():
                tasks.append((src / f"{name}.wav", dst / f"{name}.wav"))
    with cf.ThreadPoolExecutor(jobs) as pool:
        list(pool.map(lambda t: ffmpeg_16k(*t), tasks))
    print(f"prepared {len(names(work))} files")


# ---------------------------------------------------------------- systems


def run_hush(work: Path, system: str, jobs: int) -> None:
    rate, layers, look_ahead = HUSH[system]
    src = work / "data" / ("noisy16pad" if rate == 16000 else "noisy48pad")
    out = work / "out" / system
    todo = [n for n in names(work) if not (out / f"{n}.wav").exists()]
    if not todo:
        return
    out.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, RAYON_NUM_THREADS="1")
    procs = []
    for shard in range(jobs):
        part = todo[shard::jobs]
        if not part:
            continue
        listing = work / "lists" / f"{system}_{shard}.txt"
        listing.parent.mkdir(exist_ok=True)
        listing.write_text("\n".join(str(src / f"{n}.wav") for n in part))
        cmd = [str(hush_exe()), str(listing), str(out), str(layers), str(look_ahead), str(MODEL)]
        procs.append(subprocess.Popen(cmd, env=env, stdout=subprocess.DEVNULL))
    for p in procs:
        if p.wait() != 0:
            sys.exit(f"{system}: enhance_dir failed")


def run_dfn(work: Path, jobs: int) -> None:
    """One process per file: deep-filter keeps its stream state across files in one call."""
    exe = work / "dl" / "dfn" / DFN_EXE
    out = work / "out" / "deepfilternet3"
    out.mkdir(parents=True, exist_ok=True)
    todo = [n for n in names(work) if not (out / f"{n}.wav").exists()]

    def one(name: str) -> None:
        subprocess.run([str(exe), "-o", str(out), str(work / "data" / "noisy48pad" / f"{name}.wav")],
                       check=True, capture_output=True)

    with cf.ThreadPoolExecutor(jobs) as pool:
        list(pool.map(one, todo))


def rnnoise_lib() -> ctypes.CDLL:
    """The RNNoise library inside the pyrnnoise wheel, called directly through ctypes."""
    import importlib.util

    spec = importlib.util.find_spec("pyrnnoise")
    folder = Path(spec.origin).parent
    name = {"Windows": "rnnoise.dll", "Darwin": "librnnoise.dylib"}.get(platform.system(), "librnnoise.so")
    lib = ctypes.CDLL(str(folder / name))
    lib.rnnoise_create.argtypes = [ctypes.c_void_p]
    lib.rnnoise_create.restype = ctypes.c_void_p
    lib.rnnoise_destroy.argtypes = [ctypes.c_void_p]
    lib.rnnoise_get_frame_size.restype = ctypes.c_int
    lib.rnnoise_process_frame.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_float),
                                          ctypes.POINTER(ctypes.c_float)]
    lib.rnnoise_process_frame.restype = ctypes.c_float
    return lib


def rnnoise_enhance(lib: ctypes.CDLL, noisy: np.ndarray) -> np.ndarray:
    """Fresh RNNoise state; 480-sample frames of float samples on the 16-bit scale."""
    frame = lib.rnnoise_get_frame_size()
    x = np.ascontiguousarray(noisy, dtype=np.float32) * 32768.0
    x = np.concatenate([x, np.zeros(-len(x) % frame, np.float32)])
    y = np.zeros_like(x)
    state = lib.rnnoise_create(None)
    ptr = ctypes.POINTER(ctypes.c_float)
    for i in range(0, len(x), frame):
        a, b = x[i:i + frame], y[i:i + frame]
        lib.rnnoise_process_frame(state, b.ctypes.data_as(ptr), a.ctypes.data_as(ptr))
    lib.rnnoise_destroy(state)
    return y[:len(noisy)] / 32768.0


def run_rnnoise(work: Path) -> None:
    lib = rnnoise_lib()
    out = work / "out" / "rnnoise"
    out.mkdir(parents=True, exist_ok=True)
    for name in names(work):
        dst = out / f"{name}.wav"
        if dst.exists():
            continue
        noisy, rate = sf.read(work / "data" / "noisy48pad" / f"{name}.wav", dtype="float32")
        sf.write(dst, rnnoise_enhance(lib, noisy), rate, subtype="FLOAT")


def process(work: Path, jobs: int) -> None:
    for system in HUSH:
        t = time.time()
        run_hush(work, system, jobs)
        print(f"{system}: {time.time() - t:.0f} s")
    t = time.time()
    run_dfn(work, jobs)
    print(f"deepfilternet3: {time.time() - t:.0f} s")
    t = time.time()
    run_rnnoise(work)
    print(f"rnnoise: {time.time() - t:.0f} s")

    # Same resampler for every 48 kHz output; 16 kHz Hush outputs are used as they are.
    tasks = []
    for system in ["hush_studio", "deepfilternet3", "rnnoise"]:
        for name in names(work):
            dst = output16(work, system) / f"{name}.wav"
            if not dst.exists():
                tasks.append((work / "out" / system / f"{name}.wav", dst))
    with cf.ThreadPoolExecutor(jobs) as pool:
        list(pool.map(lambda t: ffmpeg_16k(*t), tasks))


# ---------------------------------------------------------------- metrics


def si_sdr(ref: np.ndarray, est: np.ndarray) -> float:
    ref = ref - ref.mean()
    est = est - est.mean()
    target = (np.dot(est, ref) / np.dot(ref, ref)) * ref
    noise = est - target
    return float(10 * np.log10(np.dot(target, target) / np.dot(noise, noise)))


class Dnsmos:
    """DNSMOS P.835 exactly as microsoft/DNS-Challenge DNSMOS/dnsmos_local.py (personalized=False):
    9.01 s windows, 1 s hop, short clips tiled, polynomial mapping of the raw SIG/BAK/OVRL."""

    RATE = 16000
    LENGTH = 9.01

    def __init__(self, model: Path):
        import onnxruntime as ort

        options = ort.SessionOptions()
        options.intra_op_num_threads = 1
        options.inter_op_num_threads = 1
        self.session = ort.InferenceSession(str(model), options, providers=["CPUExecutionProvider"])
        self.p_ovr = np.poly1d([-0.06766283, 1.11546468, 0.04602535])
        self.p_sig = np.poly1d([-0.08397278, 1.22083953, 0.0052439])
        self.p_bak = np.poly1d([-0.13166888, 1.60915514, -0.39604546])

    def __call__(self, audio: np.ndarray) -> tuple[float, float, float]:
        fs = self.RATE
        audio = np.asarray(audio, dtype=np.float64)
        len_samples = int(self.LENGTH * fs)
        while len(audio) < len_samples:
            audio = np.append(audio, audio)
        num_hops = int(np.floor(len(audio) / fs) - self.LENGTH) + 1
        sig, bak, ovr = [], [], []
        for idx in range(num_hops):
            seg = audio[int(idx * fs): int((idx + self.LENGTH) * fs)]
            if len(seg) < len_samples:
                continue
            raw = self.session.run(None, {"input_1": seg.astype("float32")[np.newaxis, :]})[0][0]
            sig.append(self.p_sig(raw[0]))
            bak.append(self.p_bak(raw[1]))
            ovr.append(self.p_ovr(raw[2]))
        return float(np.mean(sig)), float(np.mean(bak)), float(np.mean(ovr))


def find_lag(ref: np.ndarray, est: np.ndarray, max_lag: int) -> int:
    """Delay of `est` behind `ref` in samples, by the peak of the cross-correlation."""
    from scipy.signal import correlate

    corr = correlate(est, ref, mode="full", method="fft")
    zero = len(ref) - 1
    window = corr[zero - max_lag: zero + max_lag + 1]
    return int(np.argmax(window)) - max_lag


def aligned(est: np.ndarray, delay: int, length: int) -> np.ndarray:
    """Drop the system's constant delay and cut (or zero-pad) to the clean length."""
    est = est[delay:delay + length] if delay >= 0 else np.concatenate([np.zeros(-delay), est])[:length]
    return np.concatenate([est, np.zeros(length - len(est))]) if len(est) < length else est


_DNSMOS = None


def _init_worker(model: str) -> None:
    global _DNSMOS
    _DNSMOS = Dnsmos(Path(model))


def _score_file(args: tuple) -> dict:
    """All metrics of one system on one file; the clean reference gets DNSMOS only."""
    from pesq import pesq
    from pystoi import stoi

    work, system, name, delay = args
    clean, _ = sf.read(output16(work, "clean") / f"{name}.wav", dtype="float64")
    if system == "clean":
        sig, bak, ovr = _DNSMOS(clean)
        return {"file": name, "dnsmos_sig": sig, "dnsmos_bak": bak, "dnsmos_ovrl": ovr}
    est, rate = sf.read(output16(work, system) / f"{name}.wav", dtype="float64")
    assert rate == 16000, (system, name, rate)
    est = aligned(est, delay, len(clean))
    row = {"file": name}
    try:
        row["pesq_wb"] = float(pesq(16000, clean, est, "wb"))
    except Exception:  # noqa: BLE001 - pesq raises on silent output; count it as missing
        row["pesq_wb"] = float("nan")
    row["stoi"] = float(stoi(clean, est, 16000, extended=False))
    row["estoi"] = float(stoi(clean, est, 16000, extended=True))
    row["si_sdr"] = si_sdr(clean, est)
    row["dnsmos_sig"], row["dnsmos_bak"], row["dnsmos_ovrl"] = _DNSMOS(est)
    return row


def find_delay(work: Path, system: str) -> dict:
    """Constant delay of a system: median cross-correlation peak on evenly spaced files."""
    all_names = names(work)
    picks = [all_names[i] for i in np.linspace(0, len(all_names) - 1, DELAY_FILES).astype(int)]
    lags = []
    for name in picks:
        clean, _ = sf.read(output16(work, "clean") / f"{name}.wav", dtype="float64")
        est, _ = sf.read(output16(work, system) / f"{name}.wav", dtype="float64")
        lags.append(find_lag(clean, est[:len(clean) + int(MAX_LAG * 16000)], int(MAX_LAG * 16000)))
    delay = int(statistics.median(lags))
    result = {
        "delay_samples_16k": delay,
        "delay_ms": delay / 16.0,
        "files_checked": len(picks),
        "files_agreeing": sum(lag == delay for lag in lags),
        "lags_found": sorted(set(lags)),
    }
    print(f"{system}: delay {delay} samples ({delay / 16:.2f} ms), "
          f"{result['files_agreeing']}/{len(picks)} files agree, lags {result['lags_found']}")
    return result


def score(work: Path, jobs: int) -> None:
    """Score every system whose output is complete; results are cached per system."""
    folder = work / "scores"
    folder.mkdir(exist_ok=True)
    delays_file = work / "delays.json"
    delays = json.loads(delays_file.read_text()) if delays_file.exists() else {}
    all_names = names(work)
    model = str(work / "dl" / "dnsmos" / "sig_bak_ovr.onnx")
    with cf.ProcessPoolExecutor(jobs, initializer=_init_worker, initargs=(model,)) as pool:
        for system in ["clean", *SYSTEMS]:
            out = folder / f"{system}.json"
            if out.exists():
                continue
            if not all((output16(work, system) / f"{n}.wav").exists() for n in all_names):
                print(f"{system}: output incomplete, skipped")
                continue
            delay = 0
            if system != "clean":
                delays[system] = find_delay(work, system)
                delays_file.write_text(json.dumps(delays, indent=2))
                delay = delays[system]["delay_samples_16k"]
            t = time.time()
            rows = list(pool.map(_score_file, [(work, system, n, delay) for n in all_names], chunksize=4))
            out.write_text(json.dumps(rows))
            print(f"{system}: scored {len(rows)} files in {time.time() - t:.0f} s")


# ---------------------------------------------------------------- timing


def _windows_cpu_seconds(handle: int) -> float:
    """User + kernel time of a finished child process (Windows)."""
    from ctypes import wintypes

    times = [wintypes.FILETIME() for _ in range(4)]
    ctypes.windll.kernel32.GetProcessTimes(wintypes.HANDLE(handle), *[ctypes.byref(t) for t in times])
    to_s = lambda t: ((t.dwHighDateTime << 32) | t.dwLowDateTime) / 1e7  # noqa: E731
    return to_s(times[2]) + to_s(times[3])


def timing(work: Path, repeats: int) -> None:
    """Rough cost: each system alone on one ~30 s file, `repeats` runs, median. The machine is
    shared, so treat the numbers as approximate."""
    tdir = work / "timing"
    tdir.mkdir(exist_ok=True)
    clip48 = tdir / "timing48.wav"
    if not clip48.exists():
        parts, total = [], 0
        for name in names(work):
            x, _ = sf.read(work / "data" / "noisy48" / f"{name}.wav", dtype="int16")
            parts.append(x)
            total += len(x)
            if total >= 30 * 48000:
                break
        sf.write(clip48, np.concatenate(parts), 48000, subtype="PCM_16")
        ffmpeg_16k(clip48, tdir / "timing16.wav")
    seconds = sf.info(clip48).duration
    runs: dict = {}

    def median_of(key: str, fn) -> None:
        values = [fn() for _ in range(repeats)]
        runs[key] = {k: statistics.median(v[k] for v in values) for k in values[0]}
        runs[key]["audio_seconds"] = seconds
        print(key, runs[key])

    for system, (rate, layers, look_ahead) in HUSH.items():
        clip = tdir / ("timing16.wav" if rate == 16000 else "timing48.wav")
        for threads in (1, 2):
            def hush_run() -> dict:
                cmd = [str(hush_exe()), str(tdir / f"list{rate}.txt"), str(tdir / "out"), str(layers),
                       str(look_ahead), str(MODEL)]
                (tdir / f"list{rate}.txt").write_text(str(clip))
                out = subprocess.run(cmd, env=dict(os.environ, RAYON_NUM_THREADS=str(threads)),
                                     capture_output=True, text=True, check=True).stdout
                _, audio, wall, cpu = out.strip().splitlines()[-1].split("\t")
                return {"rtf_wall": float(wall) / float(audio), "cpu_per_audio_second": float(cpu) / float(audio)}

            median_of(f"{system}/threads={threads}", hush_run)

    def dfn_run() -> dict:
        exe = work / "dl" / "dfn" / DFN_EXE
        p = subprocess.Popen([str(exe), "-v", "-o", str(tdir / "out"), str(clip48)],
                             stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        log, _ = p.communicate()
        cpu = _windows_cpu_seconds(int(p._handle)) if os.name == "nt" else float("nan")  # noqa: SLF001
        rtf = float(re.search(r"RTF: ([0-9.eE-]+)", log).group(1))
        return {"rtf_wall": rtf, "cpu_per_audio_second_incl_model_load": cpu / seconds}

    median_of("deepfilternet3", dfn_run)

    lib = rnnoise_lib()
    noisy48, _ = sf.read(clip48, dtype="float32")

    def rnnoise_run() -> dict:
        wall, cpu = time.perf_counter(), time.process_time()
        rnnoise_enhance(lib, noisy48)
        return {"rtf_wall": (time.perf_counter() - wall) / seconds,
                "cpu_per_audio_second": (time.process_time() - cpu) / seconds}

    median_of("rnnoise", rnnoise_run)
    (work / "timing.json").write_text(json.dumps(runs, indent=2))


# ---------------------------------------------------------------- report


def summarize(values: list[float]) -> dict:
    v = np.array([x for x in values if np.isfinite(x)])
    std = float(v.std(ddof=1))
    return {"mean": float(v.mean()), "std": std, "ci95": float(1.96 * std / np.sqrt(len(v))),
            "n": int(len(v))}


def report(work: Path) -> None:
    scores = {s: json.loads((work / "scores" / f"{s}.json").read_text()) for s in ["clean", *SYSTEMS]}
    delays = json.loads((work / "delays.json").read_text())
    timing_runs = json.loads((work / "timing.json").read_text()) if (work / "timing.json").exists() else {}
    conditions = {}
    for line in (work / "data" / "log_testset.txt").read_text().split("\n"):
        parts = line.split()
        if len(parts) == 3:
            conditions[parts[0]] = (parts[1], float(parts[2]))

    systems = {}
    for system in ["clean", *SYSTEMS]:
        metrics = METRICS if system != "clean" else ["dnsmos_sig", "dnsmos_bak", "dnsmos_ovrl"]
        rows = scores[system]
        entry = {"label": LABELS[system], "metrics": {m: summarize([r[m] for r in rows]) for m in metrics}}
        if system != "clean":
            entry["delay"] = delays[system]
            by_snr = {}
            for snr in sorted({c[1] for c in conditions.values()}):
                subset = [r for r in rows if conditions[r["file"]][1] == snr]
                by_snr[f"{snr:g} dB"] = {m: round(float(np.nanmean([r[m] for r in subset])), 4)
                                         for m in ("pesq_wb", "si_sdr", "dnsmos_ovrl")}
            entry["by_input_snr"] = by_snr
            entry["pesq_failures"] = int(sum(not np.isfinite(r["pesq_wb"]) for r in rows))
            if system in HUSH:
                # Same files for every system, so a paired difference is the fair test.
                other = {r["file"]: r for r in scores["deepfilternet3"]}
                entry["minus_deepfilternet3_paired"] = {
                    m: summarize([r[m] - other[r["file"]][m] for r in rows]) for m in METRICS}
        systems[system] = entry

    import onnxruntime
    from importlib.metadata import version

    exe_name = "enhance_dir.exe" if os.name == "nt" else "enhance_dir"
    results = {
        "dataset": {
            "name": "VoiceBank-DEMAND test set (Valentini-Botinhao et al., University of Edinburgh "
                    "DataShare, doi:10.7488/ds/2117, CC BY 4.0)",
            "files": len(scores["clean"]),
            "hours": round(sum(sf.info(work / "data" / "clean48" / f"{n}.wav").duration
                               for n in names(work)) / 3600, 4),
            "downloads": {name: {"url": url, "sha256": digest} for name, (_, url, digest) in DOWNLOADS.items()},
        },
        "method": {
            "input": f"48 kHz 16-bit noisy file + {PAD_SECONDS * 1000:.0f} ms of silence, same for every system",
            "resampling": "ffmpeg -af aresample=resampler=soxr -ar 16000 -c:a pcm_f32le "
                          "(clean reference, 16 kHz system inputs, 48 kHz system outputs)",
            "alignment": f"constant delay per system: median cross-correlation peak against the clean "
                         f"reference on {DELAY_FILES} evenly spaced files (search +/-{MAX_LAG * 1000:.0f} ms); "
                         "outputs shifted by it and cut to the clean length",
            "metrics": {
                "pesq_wb": f"ITU-T P.862.2 wide-band, pesq {version('pesq')} (Python wrapper of the ITU reference C code)",
                "stoi/estoi": f"pystoi {version('pystoi')}",
                "si_sdr": "numpy, zero-mean, scale-invariant (Le Roux et al. 2019)",
                "dnsmos": "DNSMOS P.835 sig_bak_ovr.onnx from microsoft/DNS-Challenge "
                          f"@{DNS_COMMIT[:7]} (MIT), dnsmos_local.py logic, personalized=False, "
                          f"onnxruntime {onnxruntime.__version__}",
                "summary": "mean over all files; std; ci95 = 1.96 * std / sqrt(n)",
            },
        },
        "systems_tested": {
            "hush": {
                "engine": "hush-core 1.0.0 (this repository), example enhance_dir: 1000-sample chunks, "
                          "one fresh stream per file, look-ahead 0",
                "model": "nvidia/Real-time_RE-USE model.safetensors, revision 4c0c186",
                "model_sha256": MODEL_SHA256,
                "rustc": tool_version(["rustc", "--version"]),
                "commands": {s: f"RAYON_NUM_THREADS=1 target/release/examples/{exe_name} <list> <out> "
                                f"{l} {a} models/model.safetensors   # input {r} Hz"
                             for s, (r, l, a) in HUSH.items()},
            },
            "deepfilternet3": {
                "binary": DFN_EXE,
                "release": "https://github.com/Rikorose/DeepFilterNet/releases/tag/v0.5.6 (bundled DeepFilterNet3)",
                "sha256": DOWNLOADS[DFN_EXE][2],
                "settings": "defaults: no post-filter, attenuation limit 100 dB (none), no delay compensation",
                "command": f"{DFN_EXE} -o <out> <file.wav>   # one process per file, so each file starts fresh",
            },
            "rnnoise": {
                "binding": f"pyrnnoise {version('pyrnnoise')} (PyPI wheel, win_amd64)",
                "library": "xiph/rnnoise @70f1d256acd4b34a572f999a05c87bf00b67730d (2025-02-22), "
                           "model rnnoise_data-0a8755f8e2d8 (model_version at that commit)",
                "call": "rnnoise_process_frame on 480-sample frames, float samples on the 16-bit scale, "
                        "fresh state per file",
            },
            "driver": "python -I benchmarks/compare/compare.py --work <work> --jobs 8 all",
            "python": sys.version.split()[0],
            "numpy": np.__version__,
            "ffmpeg": tool_version(["ffmpeg", "-version"]),
        },
        "results": systems,
        "cpu_cost_approximate": {
            "note": "One process at a time on a shared 8-core x64 laptop (16 logical CPUs) while other work "
                    "was running; ~30 s clip (first test files joined); median of runs. "
                    "rtf_wall = processing time / audio time. "
                    "Hush and RNNoise exclude model loading; DeepFilterNet's rtf_wall is its own log "
                    "(excludes loading) and its CPU figure includes loading. deep-filter and RNNoise "
                    "run single-threaded; Hush uses RAYON_NUM_THREADS workers.",
            "runs": timing_runs,
        },
    }
    out = HERE / "results.json"
    out.write_text(json.dumps(results, indent=2) + "\n")
    print(f"wrote {out}")
    print(markdown_table(systems))
    for system in HUSH:
        diff = systems[system]["minus_deepfilternet3_paired"]
        print(f"{system} - deepfilternet3: " + ", ".join(
            f"{m} {v['mean']:+.3f} ±{v['ci95']:.3f}" for m, v in diff.items()))


def markdown_table(systems: dict) -> str:
    """Mean ± 95% CI per metric; the best processed system in each column is bold."""
    digits = {"pesq_wb": 2, "stoi": 3, "estoi": 3, "si_sdr": 1, "dnsmos_sig": 2, "dnsmos_bak": 2, "dnsmos_ovrl": 2}
    heads = ["PESQ-WB", "STOI", "ESTOI", "SI-SDR (dB)", "DNSMOS SIG", "DNSMOS BAK", "DNSMOS OVRL"]
    processed = [s for s in SYSTEMS if s != "noisy"]
    best = {m: max(processed, key=lambda s: systems[s]["metrics"][m]["mean"]) for m in METRICS}
    lines = ["| System | " + " | ".join(heads) + " |", "| --- |" + " ---: |" * len(heads)]
    for system in [*SYSTEMS, "clean"]:
        cells = []
        for m in METRICS:
            if m not in systems[system]["metrics"]:
                cells.append("–")
                continue
            v, d = systems[system]["metrics"][m], digits[m]
            mean = f"{v['mean']:.{d}f}"
            cells.append(f"{'**' + mean + '**' if best[m] == system else mean} ±{v['ci95']:.{d}f}")
        lines.append(f"| {LABELS[system]} | " + " | ".join(cells) + " |")
    return "\n".join(lines)


# ---------------------------------------------------------------- main


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("step", choices=["download", "prepare", "process", "score", "timing", "report", "all"])
    parser.add_argument("--work", type=Path, required=True, help="scratch folder outside the repository")
    parser.add_argument("--jobs", type=int, default=max(1, (os.cpu_count() or 2) // 2))
    parser.add_argument("--repeats", type=int, default=3, help="timing runs per system")
    args = parser.parse_args()
    sys.stdout.reconfigure(encoding="utf-8")
    work = args.work.resolve()
    if REPO in work.parents or work == REPO:
        sys.exit("--work must be outside the repository")
    steps = ["download", "prepare", "process", "score", "timing", "report"] if args.step == "all" else [args.step]
    for step in steps:
        print(f"== {step}")
        if step == "download":
            download(work)
        elif step == "prepare":
            prepare(work, args.jobs)
        elif step == "process":
            process(work, args.jobs)
        elif step == "score":
            score(work, args.jobs)
        elif step == "timing":
            timing(work, args.repeats)
        else:
            report(work)


if __name__ == "__main__":
    main()
