// Studio: clean up a recording. Drop a file or record yourself; Hush removes the noise on this
// computer, then you compare before and after and save a clean WAV.
import { AnimatePresence, motion } from "motion/react";
import { useCallback, useEffect, useRef, useState } from "react";
import { DevicePicker, Segmented, Spinner } from "../components/Controls";
import { CheckIcon, FolderIcon, MicIcon, PauseIcon, PlayIcon, RecordIcon, SaveIcon, StopIcon, UploadIcon } from "../components/Icons";
import { Mochi } from "../components/Mochi";
import { toast } from "../components/Toast";
import { features, mediaUrl, useFeatures, type StudioResult } from "../lib/features";
import { isTauri, on } from "../lib/ipc";
import { useApp } from "../lib/store";

// What Symphonia decodes: no Opus yet.
const FORMATS = ["wav", "mp3", "m4a", "aac", "flac", "ogg", "oga", "mp4", "mov", "aif", "aiff", "caf", "mkv"];

function clock(seconds: number) {
  const s = Math.max(0, Math.floor(seconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const rest = String(s % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${rest}` : `${m}:${rest}`;
}

async function chooseFile() {
  if (!isTauri) return features.studioOpen("Interview take 3.wav");
  const { open } = await import("@tauri-apps/plugin-dialog");
  const path = await open({ multiple: false, directory: false, filters: [{ name: "Audio and video", extensions: FORMATS }] });
  if (typeof path === "string") await features.studioOpen(path);
}

export function Studio() {
  const event = useFeatures((s) => s.studio);
  const [over, setOver] = useState(false);

  // Files dropped on the window. Tauri gives real paths; a browser preview simulates.
  useEffect(() => {
    if (!isTauri) return;
    let stop: (() => void) | undefined;
    import("@tauri-apps/api/webview").then(({ getCurrentWebview }) =>
      getCurrentWebview()
        .onDragDropEvent((e) => {
          const { type } = e.payload;
          if (type === "enter" || type === "over") setOver(true);
          else if (type === "leave") setOver(false);
          else if (type === "drop") {
            setOver(false);
            const path = e.payload.paths[0];
            if (path) features.studioOpen(path);
          }
        })
        .then((fn) => (stop = fn)),
    );
    return () => stop?.();
  }, []);

  const browserDrop = isTauri
    ? {}
    : {
        onDragOver: (e: React.DragEvent) => {
          e.preventDefault();
          setOver(true);
        },
        onDragLeave: () => setOver(false),
        onDrop: (e: React.DragEvent) => {
          e.preventDefault();
          setOver(false);
          const file = e.dataTransfer.files[0];
          if (file) features.studioOpen(file.name);
        },
      };

  const phase = event?.phase;
  const view = phase === "recording" ? "recording" : phase === "decoding" || phase === "processing" ? "working" : phase === "ready" ? "result" : "empty";

  return (
    <section className="glass relative flex min-h-0 flex-1 flex-col overflow-hidden p-6" {...browserDrop}>
      <AnimatePresence mode="wait" initial={false}>
        <motion.div
          key={view}
          className="flex min-h-0 flex-1 flex-col"
          initial={{ opacity: 0, y: 10, filter: "blur(6px)" }}
          animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
          exit={{ opacity: 0, y: -8, filter: "blur(6px)" }}
          transition={{ duration: 0.32, ease: [0.2, 0.9, 0.1, 1] }}
        >
          {view === "empty" && <Empty over={over} error={phase === "error" ? (event as { message: string }).message : null} />}
          {view === "recording" && event?.phase === "recording" && <Recording seconds={event.seconds} level={event.level} />}
          {view === "working" && event && (event.phase === "decoding" || event.phase === "processing") && (
            <Working name={event.name} progress={event.phase === "processing" ? event.progress : 0} eta={event.phase === "processing" ? event.eta : null} />
          )}
          {view === "result" && event?.phase === "ready" && <Result result={event} />}
        </motion.div>
      </AnimatePresence>
      <AnimatePresence>
        {over && view !== "empty" && (
          <motion.div className="absolute inset-3 z-10 grid place-items-center rounded-[22px] bg-white/85 backdrop-blur-sm" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}>
            <div className="text-center text-[15px] font-semibold">Drop to clean up this file instead</div>
          </motion.div>
        )}
      </AnimatePresence>
    </section>
  );
}

/** Before recording: pick the microphone and see its level. */
function RecordPanel({ onClose }: { onClose: () => void }) {
  const devices = useApp((s) => s.devices);
  const live = useApp((s) => s.settings?.input ?? null);
  const [mic, setMic] = useState<string | null>(live);
  const [level, setLevel] = useState(0);
  const [busy, setBusy] = useState(false);

  // The level shows while the panel is open; it reads the microphone as it is.
  useEffect(() => {
    features.studioMonitor(true, mic).catch(() => undefined);
    const stop = on("mic-level", (v) => setLevel((old) => Math.max(v, old * 0.8)));
    return () => {
      stop.then((fn) => fn());
      features.studioMonitor(false, null).catch(() => undefined);
    };
  }, [mic]);

  const start = async () => {
    setBusy(true);
    try {
      await features.studioMonitor(false, null);
      await features.studioRecord(mic);
    } catch (e) {
      toast(String(e), "error");
      setBusy(false);
    }
  };

  return (
    <motion.div
      initial={{ opacity: 0, y: 8, scale: 0.98 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, y: 8, scale: 0.98 }}
      transition={{ type: "spring", stiffness: 380, damping: 30 }}
      className="w-full max-w-[420px] rounded-[22px] bg-white p-4 text-left"
      style={{ boxShadow: "0 0 0 1px var(--h-line), var(--h-shadow-2)" }}
    >
      <div className="mb-2 text-[14px] font-[650]">Record with</div>
      <DevicePicker label="Microphone" icon={<MicIcon size={17} />} devices={devices?.inputs ?? [{ name: null, label: "System default" }]} value={mic} onChange={setMic} />
      <div className="mt-3 flex h-2 overflow-hidden rounded-full bg-[rgba(30,27,24,.07)]">
        <span className="h-full rounded-full transition-[width] duration-75" style={{ width: `${Math.min(100, level * 140)}%`, background: "linear-gradient(90deg,#9fe8b8,#6fd9d1)" }} />
      </div>
      <p className="mt-2.5 text-[11.5px] leading-snug text-muted">
        Hush records this microphone as it is, up to 30 minutes, then cleans the recording here. Your calls and Hush AI Microphone are not affected.
      </p>
      <div className="mt-3 flex items-center gap-2">
        <button className="btn btn-primary btn-sm" disabled={busy} onClick={start}>
          {busy ? <Spinner size={13} /> : <RecordIcon size={14} className="text-red" />} Start recording
        </button>
        <button className="btn btn-ghost btn-sm" onClick={onClose}>
          Cancel
        </button>
      </div>
    </motion.div>
  );
}

function Empty({ over, error }: { over: boolean; error: string | null }) {
  const [recording, setRecording] = useState(false);
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="mb-4 flex items-start justify-between gap-4">
        <div>
          <div className="eyebrow">Studio</div>
          <h1 className="mt-1 text-[23px] font-[650] tracking-[-0.03em]">Clean up a recording</h1>
          <p className="mt-1 max-w-[460px] text-[13.5px] leading-snug text-muted">
            Podcasts, voice notes, interviews, lectures. Hush removes the noise on this computer; nothing is uploaded.
          </p>
        </div>
      </div>
      <motion.div
        animate={{ scale: over ? 1.012 : 1 }}
        transition={{ type: "spring", stiffness: 300, damping: 26 }}
        className="relative flex min-h-0 flex-1 flex-col items-center justify-center gap-4 rounded-[24px] p-6 text-center"
        style={{ background: over ? "linear-gradient(140deg, rgba(255,196,176,.35), rgba(201,242,212,.35))" : "rgba(255,255,255,.55)" }}
      >
        <svg className="pointer-events-none absolute inset-0 h-full w-full" aria-hidden>
          <rect
            x="1"
            y="1"
            width="calc(100% - 2px)"
            height="calc(100% - 2px)"
            rx="23"
            fill="none"
            stroke={over ? "#e8735a" : "rgba(30,27,24,.18)"}
            strokeWidth="1.5"
            strokeDasharray="7 7"
            style={{ animation: over ? "h-dash 1s linear infinite" : undefined, transition: "stroke .2s" }}
          />
        </svg>
        {/* While the recording panel is open it takes the illustration's place. */}
        {!recording && (
          <>
            <motion.div animate={{ y: over ? -6 : 0, rotate: over ? -4 : 0 }} transition={{ type: "spring", stiffness: 260, damping: 18 }}>
              <Mochi size={74} mood={over ? "happy" : "awake"} />
            </motion.div>
            <div>
              <div className="text-[17px] font-[640] tracking-[-0.02em]">{over ? "Let go to clean it up" : "Drop an audio or video file here"}</div>
              <div className="mt-1 text-[12.5px] text-muted">WAV, MP3, M4A, FLAC, OGG, or the sound of an MP4 or MOV. Long files are fine.</div>
            </div>
          </>
        )}
        <div className="flex items-center gap-2">
          <button className="btn btn-primary" onClick={() => chooseFile().catch((e) => toast(String(e), "error"))}>
            <UploadIcon size={17} /> Choose a file
          </button>
          <button className="btn btn-soft" aria-expanded={recording} onClick={() => setRecording((r) => !r)}>
            <RecordIcon size={16} className="text-red" /> Record yourself
          </button>
        </div>
        <AnimatePresence>{recording && <RecordPanel onClose={() => setRecording(false)} />}</AnimatePresence>
        {error && <div className="max-w-[420px] rounded-2xl bg-[#fdecea] px-3 py-2 text-[12.5px] text-red">{error}</div>}
      </motion.div>
      <ul className="mt-4 grid grid-cols-3 gap-3 text-[12px] text-ink-2">
        {[
          ["Private", "Your audio never leaves this computer."],
          ["Free, no limits", "No account, no minutes, no file caps."],
          ["Same engine as calls", "The model you use for live noise cancellation."],
        ].map(([title, text]) => (
          <li key={title} className="tile px-3 py-2.5">
            <div className="font-semibold text-ink">{title}</div>
            <div className="mt-0.5 leading-snug text-muted">{text}</div>
          </li>
        ))}
      </ul>
    </div>
  );
}

function Recording({ seconds, level }: { seconds: number; level: number }) {
  const bars = useRef<number[]>(new Array(48).fill(0));
  bars.current = [...bars.current.slice(1), level];
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-6">
      <div className="flex items-center gap-2 text-[13px] font-semibold text-red">
        <span className="relative grid h-3 w-3 place-items-center">
          <span className="absolute inset-0 rounded-full bg-red" style={{ animation: "h-pulse-ring 1.2s var(--h-ease-soft) infinite" }} />
          <span className="h-2.5 w-2.5 rounded-full bg-red" />
        </span>
        Recording
      </div>
      <div className="text-[54px] font-[600] tabular-nums tracking-[-0.04em]">{clock(seconds)}</div>
      <div className="flex h-16 w-full max-w-[460px] items-center gap-[3px]">
        {bars.current.map((v, i) => (
          <span key={i} className="flex-1 rounded-full bg-ink/70 transition-[height] duration-100" style={{ height: `${Math.max(6, Math.min(100, v * 140))}%`, opacity: 0.35 + (i / 48) * 0.65 }} />
        ))}
      </div>
      <div className="flex items-center gap-2">
        <button className="btn btn-primary" onClick={() => features.studioStop()}>
          <StopIcon size={16} /> Stop and clean up
        </button>
        <button className="btn btn-ghost" onClick={() => features.studioCancel()}>
          Discard
        </button>
      </div>
      <p className="flex items-center gap-1.5 text-[12px] text-muted">
        <MicIcon size={14} /> Recording your microphone as it is. Hush cleans it when you stop.
      </p>
    </div>
  );
}

function Working({ name, progress, eta }: { name: string; progress: number; eta: number | null }) {
  const pct = Math.round(progress * 100);
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-5 text-center">
      <div className="relative grid h-[132px] w-[132px] place-items-center">
        <svg width="132" height="132" viewBox="0 0 132 132" className="absolute inset-0 -rotate-90">
          <defs>
            <linearGradient id="studio-ring" x1="0" x2="1" y1="0" y2="1">
              <stop offset="0" stopColor="#ffae94" />
              <stop offset="0.5" stopColor="#ffd27a" />
              <stop offset="1" stopColor="#6fd9d1" />
            </linearGradient>
          </defs>
          <circle cx="66" cy="66" r="58" fill="none" stroke="rgba(30,27,24,.07)" strokeWidth="8" />
          <motion.circle cx="66" cy="66" r="58" fill="none" stroke="url(#studio-ring)" strokeWidth="8" strokeLinecap="round" initial={false} animate={{ pathLength: Math.max(0.02, progress) }} transition={{ type: "spring", stiffness: 120, damping: 24 }} />
        </svg>
        <Mochi size={70} mood="happy" />
      </div>
      <div>
        <div className="eyebrow">Cleaning up</div>
        <div className="mt-1 max-w-[460px] truncate text-[19px] font-[640] tracking-[-0.02em]">{name}</div>
        <div className="mt-1 text-[13px] tabular-nums text-muted">
          {progress > 0 ? `${pct}%` : "Reading the file…"}
          {eta !== null && eta > 1 ? ` · about ${eta > 90 ? `${Math.round(eta / 60)} min` : `${Math.round(eta)} s`} left` : ""}
        </div>
      </div>
      <button className="btn btn-ghost btn-sm" onClick={() => features.studioCancel()}>
        Cancel
      </button>
    </div>
  );
}

function Result({ result }: { result: StudioResult }) {
  const original = useRef<HTMLAudioElement>(null);
  const enhanced = useRef<HTMLAudioElement>(null);
  const [urls, setUrls] = useState<{ original: string; enhanced: string } | null>(null);
  const [side, setSide] = useState<"hush" | "original">("hush");
  const [playing, setPlaying] = useState(false);
  const [time, setTime] = useState(0);
  const [saved, setSaved] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const duration = result.seconds;

  useEffect(() => {
    let alive = true;
    Promise.all([mediaUrl(result.original), mediaUrl(result.enhanced)]).then(([o, e]) => alive && setUrls({ original: o, enhanced: e }));
    return () => {
      alive = false;
    };
  }, [result.original, result.enhanced]);

  // Both versions play together; the one you are not listening to is muted, so switching is instant.
  useEffect(() => {
    if (original.current) original.current.muted = side !== "original";
    if (enhanced.current) enhanced.current.muted = side !== "hush";
  }, [side, urls]);

  useEffect(() => {
    if (!playing) return;
    let frame = 0;
    const tick = () => {
      const a = enhanced.current;
      if (a) {
        setTime(a.currentTime);
        const b = original.current;
        if (b && Math.abs(b.currentTime - a.currentTime) > 0.06) b.currentTime = a.currentTime;
      }
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [playing]);

  const toggle = useCallback(async () => {
    const a = enhanced.current;
    const b = original.current;
    if (!a || !b) return;
    if (playing) {
      a.pause();
      b.pause();
      setPlaying(false);
    } else {
      if (a.ended || a.currentTime >= duration - 0.05) a.currentTime = b.currentTime = 0;
      b.currentTime = a.currentTime;
      await Promise.all([a.play(), b.play()]).catch(() => undefined);
      setPlaying(true);
    }
  }, [playing, duration]);

  const seek = (fraction: number) => {
    const t = Math.max(0, Math.min(duration, fraction * duration));
    if (enhanced.current) enhanced.current.currentTime = t;
    if (original.current) original.current.currentTime = t;
    setTime(t);
  };

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement) return;
      if (e.code === "Space") {
        e.preventDefault();
        toggle();
      } else if (e.key === "a" || e.key === "b") setSide((s) => (s === "hush" ? "original" : "hush"));
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [toggle]);

  const save = async () => {
    setSaving(true);
    try {
      let target: string | null = `${result.name} (Hush).wav`;
      if (isTauri) {
        const { save: dialog } = await import("@tauri-apps/plugin-dialog");
        target = await dialog({ defaultPath: `${result.name} (Hush).wav`, filters: [{ name: "WAV audio", extensions: ["wav"] }] });
      }
      if (!target) return;
      if (!/\.wav$/i.test(target)) target += ".wav";
      await features.studioSave(target);
      setSaved(target);
      toast("Saved the clean version.");
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setSaving(false);
    }
  };

  const reveal = async () => {
    if (!saved || !isTauri) return;
    const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
    await revealItemInDir(saved).catch((e) => toast(String(e), "error"));
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0">
          <div className="eyebrow flex items-center gap-1.5 text-green">
            <CheckIcon size={14} draw /> Cleaned up
          </div>
          <h1 className="mt-1 truncate text-[22px] font-[650] tracking-[-0.03em]">{result.name}</h1>
          <p className="mt-0.5 text-[12.5px] text-muted">
            {clock(duration)} · {result.engine} · done in {result.elapsed < 60 ? `${result.elapsed.toFixed(1)} s` : clock(result.elapsed)}
          </p>
        </div>
        <button className="btn btn-ghost btn-sm shrink-0" onClick={() => features.studioCancel()}>
          Clean up another
        </button>
      </div>

      <div className="mt-5 flex min-h-0 flex-1 flex-col justify-center">
        <Wave original={result.originalPeaks} enhanced={result.enhancedPeaks} side={side} progress={duration ? time / duration : 0} onSeek={seek} />
        <div className="mt-2 flex justify-between text-[11.5px] tabular-nums text-muted">
          <span>{clock(time)}</span>
          <span>{clock(duration)}</span>
        </div>
      </div>

      <div className="mt-5 flex items-center gap-3">
        <button className="btn btn-primary btn-icon h-12 w-12" aria-label={playing ? "Pause" : "Play"} disabled={!urls} onClick={toggle}>
          {playing ? <PauseIcon size={20} /> : <PlayIcon size={20} />}
        </button>
        <div className="w-[230px]">
          <Segmented
            label="Listen to"
            value={side}
            options={[
              { value: "original", label: "Original" },
              { value: "hush", label: "With Hush" },
            ]}
            onChange={setSide}
          />
        </div>
        <span className="hidden text-[11.5px] text-faint md:inline">Space plays · A switches</span>
        <div className="flex-1" />
        {saved && isTauri && (
          <button className="btn btn-ghost btn-sm" onClick={reveal}>
            <FolderIcon size={15} /> Show in folder
          </button>
        )}
        <button className="btn btn-opal" disabled={saving} onClick={save}>
          {saving ? <Spinner size={14} /> : <SaveIcon size={17} />} Save clean WAV
        </button>
      </div>
      {urls && (
        <>
          <audio ref={original} src={urls.original} preload="auto" onEnded={() => setPlaying(false)} />
          <audio ref={enhanced} src={urls.enhanced} preload="auto" onEnded={() => setPlaying(false)} />
        </>
      )}
    </div>
  );
}

/** Mirrored peak bars. Switching sides sweeps from one waveform to the other. */
function Wave({ original, enhanced, side, progress, onSeek }: { original: number[]; enhanced: number[]; side: "hush" | "original"; progress: number; onSeek: (fraction: number) => void }) {
  // About 160 separate bars read better than one bar per peak.
  const n = 160;
  const H = 120;
  const pool = (peaks: number[]) =>
    Array.from({ length: n }, (_, i) => {
      const from = Math.floor((i * peaks.length) / n);
      const to = Math.max(from + 1, Math.floor(((i + 1) * peaks.length) / n));
      return Math.max(0, ...peaks.slice(from, to));
    });
  const max = Math.max(0.05, ...original, ...enhanced);
  const path = (peaks: number[]) => {
    let d = "";
    pool(peaks).forEach((v, i) => {
      const h = Math.max(1.5, (v / max) * (H / 2 - 6));
      d += `M${i + 0.5} ${H / 2 - h}V${H / 2 + h}`;
    });
    return d;
  };
  const [o, e] = [path(original), path(enhanced)];
  return (
    <div
      role="slider"
      aria-label="Position"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(progress * 100)}
      tabIndex={0}
      className="relative h-[120px] w-full cursor-pointer"
      onClick={(ev) => {
        const rect = ev.currentTarget.getBoundingClientRect();
        onSeek((ev.clientX - rect.left) / rect.width);
      }}
      onKeyDown={(ev) => {
        if (ev.key === "ArrowRight") onSeek(progress + 0.02);
        if (ev.key === "ArrowLeft") onSeek(progress - 0.02);
      }}
    >
      <svg viewBox={`0 0 ${n} ${H}`} preserveAspectRatio="none" className="absolute inset-0 h-full w-full">
        <defs>
          <linearGradient id="wave-hush" x1="0" x2="1">
            <stop offset="0" stopColor="#1f9d72" />
            <stop offset="1" stopColor="#178f8a" />
          </linearGradient>
          <clipPath id="wave-sweep">
            <motion.rect y="0" height={H} initial={false} animate={{ x: 0, width: side === "hush" ? n : 0 }} transition={{ duration: 0.55, ease: [0.65, 0, 0.35, 1] }} />
          </clipPath>
        </defs>
        <path d={o} stroke="#c3bcb1" strokeLinecap="round" vectorEffect="non-scaling-stroke" style={{ strokeWidth: 3 }} />
        <g clipPath="url(#wave-sweep)">
          <rect width={n} height={H} fill="rgba(251,250,247,.92)" />
          <path d={e} stroke="url(#wave-hush)" strokeLinecap="round" style={{ strokeWidth: 3 }} vectorEffect="non-scaling-stroke" />
        </g>
      </svg>
      <div className="pointer-events-none absolute inset-y-0 w-[2px] rounded-full bg-ink" style={{ left: `${Math.min(100, progress * 100)}%`, boxShadow: "0 0 0 3px rgba(255,255,255,.7)" }} />
    </div>
  );
}
