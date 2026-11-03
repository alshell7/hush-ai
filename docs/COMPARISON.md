# Quality comparison

[Home](../README.md) · [Performance](PERFORMANCE.md) · [Accuracy](ACCURACY.md)

This page compares the speech quality of Hush with two other real-time noise suppressors:
DeepFilterNet 3 and RNNoise. All three ran on the same 824 noisy recordings. We scored each
output against the clean recording. We did not train or tune any model.

## Summary

- **DeepFilterNet 3 scores higher than Hush Light on every metric.** Hush Light is the
  app default. The largest gap is in PESQ: 3.10 against 2.61. The smallest is DNSMOS SIG,
  where the gap is within the noise of the measurement.
- **With 8 layers, RE-USE has the best DNSMOS scores**, but DeepFilterNet 3 still has the best
  PESQ, STOI, ESTOI and SI-SDR.
- **Every RE-USE setting scores higher than RNNoise on every metric.**
- **Hush matches NVIDIA's published scores.** At 16 kHz with 8 layers, our scores are within
  0.01 PESQ and 0.1 dB SI-SDR of the paper.
- **Hush uses much more CPU.** On this laptop, Hush Light needs 13 to 15 times the CPU time
  of DeepFilterNet 3.

## What we tested

| System | Version | Input | Settings |
| --- | --- | --- | --- |
| RE-USE via Hush, Light | hush-core 1.0.0, `enhance_dir` example; NVIDIA weights rev. 4c0c186 | 16 kHz | 3 layers, look-ahead 0 (app default) |
| RE-USE via Hush, Studio | same | 48 kHz | 3 layers, look-ahead 0 |
| RE-USE via Hush, 8 layers | same | 16 kHz | 8 layers, look-ahead 0 (NVIDIA's paper setting) |
| DeepFilterNet 3 | official `deep-filter` 0.5.6 binary for Windows | 48 kHz | defaults: no post-filter, no attenuation limit |
| RNNoise | `pyrnnoise` 0.4.5 wheel: xiph/rnnoise at commit 70f1d25, model `0a8755f8` | 48 kHz | library defaults |
| Noisy | the unprocessed input | – | – |

Hush ran in the same way as live audio: 1000-sample chunks, one stream per file.
DeepFilterNet ran as one process per file, because the binary keeps its state between files.

## Method

- **Data:** VoiceBank-DEMAND test set: 824 files, 34.5 minutes, 2 speakers (p232, p257),
  5 noise types, input SNR 2.5, 7.5, 12.5 and 17.5 dB, 48 kHz
  ([Valentini-Botinhao, 2017](https://doi.org/10.7488/ds/2117), CC BY 4.0).
- **Input:** every system got the same 48 kHz file with 100 ms of silence added at the end.
  This lets each system output the full utterance. The 16 kHz inputs come from these files.
- **Resampling:** one resampler for every signal: ffmpeg with soxr, to 32-bit float.
- **Scoring rate:** all metrics use 16 kHz audio. So they do not see sound above 8 kHz.
- **Alignment:** we found each system's delay from the cross-correlation peak with the clean
  file, on 24 files spread over the set. All 24 files gave the same delay for each system. We
  removed this delay and cut each output to the clean length.
- **Metrics:** PESQ wide-band (ITU-T P.862.2, `pesq` 0.0.4, which wraps the ITU C code); STOI
  and ESTOI (`pystoi` 0.4.1); SI-SDR (zero-mean, scale-invariant); DNSMOS P.835 SIG, BAK and
  OVRL (Microsoft's `sig_bak_ovr.onnx` and the logic of `dnsmos_local.py`, not personalized,
  from [microsoft/DNS-Challenge](https://github.com/microsoft/DNS-Challenge), MIT).
- **Statistics:** the mean over all 824 files, ± the 95% confidence interval (1.96 × std / √n).
  For the Hush–DeepFilterNet gaps we use paired differences, because the files are the same.

The noisy row matches the published noisy scores (PESQ 1.97, STOI 0.921, ESTOI 0.79,
SI-SDR 8.4 dB). This confirms the scoring chain.

## Results

Mean ± 95% confidence interval. **Bold**: best processed system in the column. Higher is better.

| System | PESQ-WB | STOI | ESTOI | SI-SDR (dB) | DNSMOS SIG | DNSMOS BAK | DNSMOS OVRL |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Noisy (unprocessed) | 1.97 ±0.05 | 0.921 ±0.005 | 0.787 ±0.010 | 8.4 ±0.4 | 3.35 ±0.04 | 3.13 ±0.05 | 2.70 ±0.04 |
| RE-USE via Hush, Light (16 kHz, 3 layers) | 2.61 ±0.04 | 0.934 ±0.004 | 0.848 ±0.007 | 18.0 ±0.2 | 3.43 ±0.01 | 4.04 ±0.01 | 3.16 ±0.02 |
| RE-USE via Hush, Studio (48 kHz, 3 layers) | 2.70 ±0.04 | 0.935 ±0.004 | 0.850 ±0.007 | 16.9 ±0.2 | 3.41 ±0.01 | 4.03 ±0.01 | 3.13 ±0.02 |
| RE-USE via Hush, 16 kHz, 8 layers | 2.77 ±0.04 | 0.938 ±0.004 | 0.859 ±0.007 | 18.5 ±0.2 | **3.48** ±0.01 | **4.10** ±0.01 | **3.23** ±0.01 |
| DeepFilterNet 3 (48 kHz) | **3.10** ±0.04 | **0.945** ±0.003 | **0.864** ±0.006 | **19.0** ±0.2 | 3.43 ±0.01 | 4.08 ±0.01 | 3.18 ±0.02 |
| RNNoise (48 kHz) | 2.11 ±0.04 | 0.890 ±0.006 | 0.783 ±0.009 | 12.3 ±0.2 | 3.29 ±0.02 | 3.85 ±0.02 | 2.94 ±0.02 |
| Clean reference | – | – | – | – | 3.51 ±0.01 | 4.04 ±0.01 | 3.22 ±0.01 |

Paired difference, RE-USE minus DeepFilterNet 3 (negative: DeepFilterNet 3 is better):

| RE-USE setting | PESQ-WB | STOI | ESTOI | SI-SDR (dB) | SIG | BAK | OVRL |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Light | −0.49 ±0.02 | −0.011 ±0.002 | −0.016 ±0.002 | −1.03 ±0.11 | −0.007 ±0.007 | −0.036 ±0.007 | −0.022 ±0.008 |
| Studio | −0.41 ±0.02 | −0.010 ±0.001 | −0.014 ±0.002 | −2.08 ±0.14 | −0.025 ±0.007 | −0.048 ±0.008 | −0.046 ±0.008 |
| 16 kHz, 8 layers | −0.34 ±0.02 | −0.007 ±0.002 | −0.006 ±0.002 | −0.46 ±0.09 | +0.046 ±0.007 | +0.022 ±0.005 | +0.055 ±0.008 |

PESQ-WB by input SNR:

| System | 2.5 dB | 7.5 dB | 12.5 dB | 17.5 dB |
| --- | ---: | ---: | ---: | ---: |
| Noisy | 1.42 | 1.76 | 2.10 | 2.60 |
| Hush Light | 2.22 | 2.54 | 2.70 | 2.98 |
| Hush Studio | 2.28 | 2.63 | 2.80 | 3.09 |
| Hush 16 kHz, 8 layers | 2.37 | 2.72 | 2.86 | 3.12 |
| DeepFilterNet 3 | **2.57** | **3.06** | **3.25** | **3.55** |
| RNNoise | 1.75 | 2.07 | 2.19 | 2.43 |

## Published results

These numbers come from the papers. We did not measure them, and they use other code and
model versions. "–" means not reported.

| Source | System | PESQ | STOI | ESTOI | SI-SDR (dB) | Other | Algorithmic latency |
| --- | --- | ---: | ---: | ---: | ---: | --- | ---: |
| Fu et al. 2026, [arXiv 2606.25621](https://arxiv.org/abs/2606.25621), Table II | Noisy | 1.97 | – | 0.79 | 8.4 | – | – |
|  | RE-USE, 8 layers, look-ahead 0 | 2.76 | – | 0.86 | 18.6 | – | 40 ms |
|  | RE-USE, 8 layers, look-ahead 1 | 2.82 | – | 0.86 | 18.8 | – | 60 ms |
|  | DeepFilterNet3 (taken from Welker et al.) | 2.71 | – | 0.84 | 17.3 | – | 40 ms |
| Welker et al. 2025, [arXiv 2512.19442](https://arxiv.org/abs/2512.19442), Table III (16 kHz, WB-PESQ) | DeepFilterNet3, official checkpoint | 2.71 | – | 0.84 | 17.3 | DiMOS 3.34 | 40 ms |
| Schröter et al. 2023, [arXiv 2305.08227](https://arxiv.org/abs/2305.08227), Table 1 | DeepFilterNet3 | 3.17 | 0.944 | – | – | CSIG 4.34, CBAK 3.61, COVL 3.77 | 40 ms |
| Schröter et al. 2022, [arXiv 2205.05474](https://arxiv.org/abs/2205.05474), Table 1 (WB-PESQ) | Noisy | 1.97 | 0.921 | – | – | CSIG 3.34, CBAK 2.44, COVL 2.63 | – |
|  | RNNoise (xiph code and weights, 2022) | 2.33 | 0.922 | – | – | CSIG 3.40, CBAK 2.51, COVL 2.84 | – |

Notes on these numbers:

- The RE-USE paper does not say which PESQ it uses or at which rate. Its noisy row and its
  DeepFilterNet3 row are equal to the 16 kHz wide-band numbers of Welker et al.
- The RE-USE paper reports only 8 layers on this test set. It does not report 3 layers.
- NVIDIA's model card says that the released checkpoint differs from the one in the paper.
  Hush uses the released checkpoint.
- The DeepFilterNet 3 paper does not say which PESQ it uses.

## Observations

- **Hush reproduces NVIDIA's paper.** With 8 layers at 16 kHz we measured PESQ 2.77, ESTOI
  0.859 and SI-SDR 18.5 dB. The paper reports 2.76, 0.86 and 18.6 dB.
- **We could not reproduce the DeepFilterNet3 row in the RE-USE paper.** We measured PESQ 3.10
  with the official binary at 48 kHz. Its own paper reports 3.17. The RE-USE paper uses 2.71
  from Welker et al. With our numbers, DeepFilterNet 3 is ahead of RE-USE with 8 layers on
  PESQ, STOI, ESTOI and SI-SDR. We do not know why the two DeepFilterNet3 results differ.
- **RE-USE wins on DNSMOS only with 8 layers.** DNSMOS estimates a listener's opinion without
  the clean file. With 8 layers, RE-USE is ahead of DeepFilterNet 3 by 0.055 OVRL. With 3 layers
  (Light), it is behind by 0.022 OVRL. The SIG difference is less than 0.01.
- **DeepFilterNet 3 keeps its lead at every input SNR.** The PESQ gap to Hush Light is 0.34 at
  2.5 dB and 0.57 at 17.5 dB.
- **Studio does not score higher than Light on most metrics.** Studio has higher PESQ
  (+0.08) but lower SI-SDR (−1.0 dB) and DNSMOS. The metrics use 16 kHz audio, so they cannot
  show what Studio adds above 8 kHz.
- **RNNoise improves the least.** It removes some noise (DNSMOS BAK 3.13 → 3.85), but its
  STOI and ESTOI are slightly below the noisy input.
- **Delay:** see the table below. At look-ahead 0, RE-USE and DeepFilterNet 3 have the same
  algorithmic latency: 40 ms. The Hush app adds one 20 ms output frame, so Hush adds about
  60 ms. DeepFilterNet 3 has the higher quality at this latency on this test set.

| System | Frame step | Delay we measured in the output file | Algorithmic latency |
| --- | ---: | ---: | ---: |
| RE-USE via Hush | 20 ms | 0 ms: the engine removes it | 40 ms ([paper](https://arxiv.org/abs/2606.25621), Table I); about 60 ms in the app ([Architecture](ARCHITECTURE.md#delay-budget)) |
| DeepFilterNet 3 | 10 ms | 30.0 ms (1440 samples at 48 kHz) | 40 ms ([paper](https://arxiv.org/abs/2305.08227)): 20 ms window + 2 look-ahead frames |
| RNNoise (tested library) | 10 ms | 20.0 ms (960 samples at 48 kHz) | about 30 ms: 20 ms + one 10 ms frame |

For DeepFilterNet 3, the measured 30 ms plus one 10 ms frame gives the published 40 ms.
RNNoise is not a 10 ms system: the 10 ms is its frame step and the look-ahead given on the
2017 [demo page](https://jmvalin.ca/demo/rnnoise/). The tested library also applies each
frame's gains to the previous frame (`delayed_X` in `src/denoise.c`), so its file delay is
20 ms.

## Caveats

- **One dataset.** VoiceBank-DEMAND has two speakers, five noise types and synthetic mixtures
  without reverberation. Real microphones are harder. Do not generalize these numbers.
- **The test speech is in the training data of two models.** The clean test files are VCTK
  recordings: the VCTK 0.92 file `p232_001_mic1` matches the clean test file `p232_001` with a
  correlation of 0.999998. NVIDIA's model card lists VCTK (about 80 hours) in the RE-USE
  training data. DeepFilterNet 3 trains on DNS4 and repeats VCTK 10 times
  ([paper](https://arxiv.org/abs/2305.08227)). The [DNS4 paper](https://arxiv.org/abs/2202.13288)
  lists 110 VCTK talkers in its English clean speech and noise clips from DEMAND. So this test set is close to
  the training domain of DeepFilterNet 3 for both speech and noise. The RE-USE model card does
  not list DEMAND. The RNNoise training list (`datasets.txt`) does not list VCTK.
- **RE-USE ran in its low-latency streaming setting:** look-ahead 0, and 3 layers for Light and
  Studio. More layers and look-ahead give higher scores and cost more CPU or delay.
- **Objective metrics only.** We did no listening test. PESQ, STOI and SI-SDR compare the
  waveform with the clean file. DNSMOS is a neural estimate of a listener's score.
- **48 kHz output was scored at 16 kHz.** The metrics ignore the 8–24 kHz band.
- DeepFilterNet's binary writes 16-bit output. Hush and RNNoise outputs are 32-bit float.

## CPU cost (approximate)

One process at a time on a 30 s clip (the first test files joined), median of 3 runs. The
laptop (8-core x64, 16 logical CPUs) ran other work at the same time, so treat these
numbers as rough. "Time" is processing time divided by audio time; below 1.0 is faster than
real time. The CPU column is CPU time divided by audio time.

| System | Threads | Time | CPU |
| --- | ---: | ---: | ---: |
| Hush Light (16 kHz, 3 layers) | 1 | 0.79 | 0.79 |
|  | 2 | 0.45 | 0.84 |
| Hush Studio (48 kHz, 3 layers) | 1 | 2.23 | 2.22 |
|  | 2 | 1.10 | 2.04 |
| Hush 16 kHz, 8 layers | 1 | 1.45 | 1.46 |
|  | 2 | 0.84 | 1.59 |
| DeepFilterNet 3 (`deep-filter`) | 1 | 0.054 | 0.059 |
| RNNoise (loop in Python) | 1 | 0.049 | 0.049 |

Hush threads are set with `RAYON_NUM_THREADS`. The `deep-filter` CPU time includes model
loading; its CPU time is close to its processing time, so it used one thread. The RNNoise
time includes one Python call per 10 ms frame. [Performance](PERFORMANCE.md) has the Hush
frame times measured on idle machines.

## Reproduce

Windows x64, from the repository root. You need Rust, Python 3.13 and ffmpeg built with
libsoxr. Use a work folder outside the repository: it receives about 3 GB of audio.

```sh
WORK=/c/temp/hush-compare
python -m venv $WORK/venv
$WORK/venv/Scripts/python -m pip install -r benchmarks/compare/requirements.txt
cargo build --release -p hush-core --example enhance_dir
$WORK/venv/Scripts/python -I benchmarks/compare/compare.py --work $WORK --jobs 8 all
```

[`compare.py`](../benchmarks/compare/compare.py) downloads the data, the `deep-filter` binary
and the DNSMOS model, and checks their SHA-256. Then it runs every system, scores every file
and writes [`results.json`](../benchmarks/compare/results.json). Each step is also a
sub-command (`download`, `prepare`, `process`, `score`, `timing`, `report`). The full run
took about 2.5 hours on this laptop. The `deep-filter` binary and the RNNoise wheel in the
script are the Windows builds.
