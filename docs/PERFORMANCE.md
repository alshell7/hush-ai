# Performance

[Home](../README.md) · [Architecture](ARCHITECTURE.md) · [Accuracy](ACCURACY.md)

Hush has two engines. A RE-USE frame is 20 ms of audio; a DeepFilterNet 3 frame is 10 ms.
To run in real time, the CPU must finish every frame in less than its length, with margin
for other programs. Less CPU per second of audio means less battery and less fan noise.

## DeepFilterNet 3 (default)

8-core x64 laptop, release build, one thread. The laptop ran other work during these runs,
so the numbers are rough.

| Run | CPU per audio second | Lost audio |
| --- | ---: | --- |
| Offline, 30 s clip, median of 3 (`enhance_dir`) | 7.0% (0.71 ms per 10 ms frame) | – |
| Paced live worker, 30 s, 2 runs (`bench_live`) | 11–13% of one core | none |
| Same paced run, RE-USE Light on 1 thread, same time | 96% | could not keep up |
| Same paced run, RE-USE Light on 2 threads | 155% | some |

The live run costs more than the offline one because the worker wakes up for each 10 ms
frame. DeepFilterNet 3 needs no worker pool, so **Automatic** threads do not apply. On an
idle laptop, RE-USE Light used 71–74% on one thread (below).

## RE-USE

### Frame time

#### Apple M1

Mac mini M1, release build, 1000 frames after 50 warm-up frames.

| Quality | Threads | p50 | p99 | CPU per audio second |
| --- | ---: | ---: | ---: | ---: |
| Light (16 kHz, 3 layers) | 1 | 17.9 ms | 21.2 ms | 90% |
|  | 2 | 10.4 ms | **11.7 ms** | 99% |
|  | 4 | 7.9 ms | **9.5 ms** | 127% |
| Balanced (24 kHz, 4 layers) | 2 | 19.7 ms | 21.3 ms | 188% |
|  | 4 | 14.2 ms | 21.8 ms | 237% |
| Studio (48 kHz, 3 layers) | 2 | 31.2 ms | 36.8 ms | 296% |
|  | 4 | 22.7 ms | 26.4 ms | 360% |

#### Intel x64 (cloud)

Modal Linux x64, shared Intel cores at 3.6 GHz with AVX-512. Median of 2 runs of 1500
frames. **Bold**: p99 below the 20 ms budget.

| Quality | Threads | p50 | p99 | CPU per audio second | Late frames |
| --- | ---: | ---: | ---: | ---: | ---: |
| Light (16 kHz, 3 layers) | 2 | 19.9 ms | 25.1 ms | 195% | 48.7% |
|  | 4 | 13.5 ms | **17.1 ms** | 266% | 0.2% |
|  | 8 | 11.9 ms | **14.9 ms** | 465% | 0.0% |
| Balanced (24 kHz, 4 layers) | 4 | 17.7 ms | 21.4 ms | 336% | 5.0% |
|  | 8 | 15.6 ms | 22.9 ms | 594% | 2.2% |
| Studio (48 kHz, 3 layers) | 8 | 21.2 ms | 23.7 ms | 774% | 86.1% |

Cloud containers run in a sandbox (gVisor) that makes thread switches expensive, so these
numbers overstate the cost on a normal computer. Desktop CPUs usually have faster cores.
The app shows the live **Load** for your computer.

What this shows:

- **Light** runs in real time on both, with large margin on M1. It is the default RE-USE setting.
- Rate and layers set the cost: 48 kHz costs about 2.5 times 16 kHz.
- More threads cut the frame time but add CPU. Use the fewest that keep up.
- Memory (RSS) is 80–250 MB, mostly the weights.

### Threads and CPU use

In live use, frames arrive every 20 ms, and idle workers sleep and wake up around each
frame. That costs CPU. A paced run through the live worker (`bench_live`, Light, 8-core
x64 laptop):

| Threads | CPU (100% = one core) |
| ---: | ---: |
| 1 | 71–74% |
| 2 | 81–83% |
| 4 | 110–116% |
| 8 | 159% |

So the fewest threads that keep up use the least CPU. **Automatic** (the default) picks
them: on that laptop 1–2 threads instead of a fixed 4, about 30% less CPU.

## Priority under load

Live through a USB headset, all 16 logical CPUs busy with other work, 90 s each:

| Inference priority | Lost audio |
| --- | --- |
| Normal | 8,180 dropped input samples, 2,383 missing output samples, 21 resets |
| Windows audio (MMCSS) | Only during the first seconds, while Automatic measured; none after |

**Audio priority** is on by default on Windows.

## Long runs

The live path does not grow in memory or CPU over time.

| Run | Length | Memory | CPU | Slowest frame | Lost audio |
| --- | --- | --- | --- | --- | --- |
| Windows laptop, USB headset, Automatic (2 threads), audio priority | 60 min, 180,002 frames | 75.65 → 75.63 MB | 85% of one core | 16.9 ms | none |
| Mac mini M1, 48 kHz devices, Automatic (2 threads) | 30 min | 64.8 MB, flat | 98% | ~48 ms, about once a minute | 0.1–2 ms per minute |

The rare M1 stalls also appear without any audio path, on a Mac that was screen-shared at
the time, so their cause is likely outside Hush.

## Delay

| Setting | Delay before device buffers |
| --- | --- |
| RE-USE (look-ahead 0, one 20 ms output frame) | about 60 ms |
| DeepFilterNet 3 (30 ms model delay, two 10 ms output frames) | about 60 ms |
| VB-CABLE on Windows | about 84 ms more (measured with `loop_latency`) |

## Repeat the measurements

```sh
# Frame time: <rate> <layers> <look-ahead> <frames> <warm-up>
cargo run --release -p hush-core --example bench_stream -- 16000 3 0 1050 50

# Paced live worker
cargo run --release -p hush-core --example bench_live -- 16000 3 0 160 30 1 2
HUSH_MODEL=$PWD/models/DeepFilterNet3_onnx.tar.gz cargo run --release -p hush-core --example bench_live -- 48000 3 0 3000 300 4 8

# Long run with a real microphone and headphones (prints JSON every 60 s)
cargo run --release -p hush-ai --example live_device -- --list
cargo run --release -p hush-ai --example live_device -- --input "<mic>" --output "<headphones>" --seconds 3600 --report-seconds 60
```

Set `RAYON_NUM_THREADS` and `HUSH_WORKER_PLACEMENT=unrestricted` to fix the thread count.
Do not build at the same time as you measure.
