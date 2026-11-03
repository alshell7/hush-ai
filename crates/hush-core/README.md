# hush-core

The speech enhancement engine of [Hush AI](https://github.com/alshell7/hush-ai): NVIDIA
[Real-time RE-USE](https://huggingface.co/nvidia/Real-time_RE-USE) (Mamba) in plain Rust,
in real time on the CPU. No Python, GPU or ONNX runtime.

## Stream a signal

```rust
use hush_core::{Config, Denoiser};

let mut denoiser = Denoiser::with_config(
    "models/model.safetensors",
    Config { layers: 3, sample_rate: 16_000, look_ahead: 0 },
)?;
denoiser.set_automatic_worker_threads(
    hush_core::WorkerPlacement::Unrestricted,
    hush_core::WorkerScheduling::OsDefault,
)?;
let mut clean = Vec::new();
for chunk in noisy.chunks(480) {
    denoiser.process_into(chunk, &mut clean)?; // output as frames complete
}
clean.extend(denoiser.flush()?);
```

Sample rates: 8, 16, 22.05, 24, 32, 44.1 and 48 kHz. Layers: 3–12 (fewer is faster).
Look-ahead: 0–2 frames (each adds 20 ms).

## Live devices

`hush_core::live::LiveWorker` connects audio callbacks to the model with lock-free queues,
rate conversion, clock-drift control, meters and an A/B switch. Callbacks never allocate,
lock or wait. `hush_core::virtual_mic` writes into the Hush AI Microphone drivers.

## Examples

```sh
cargo run --release -p hush-core --example enhance -- noisy.wav clean.wav 3
cargo run --release -p hush-core --example bench_stream -- 16000 3 0 1050 50
cargo run --release -p hush-core --features devices --example audio_devices
```

## Features

| Feature | Use |
| --- | --- |
| `devices` | Examples that open audio devices (`cpal`) |
| `wgpu`, `metal`, `vulkan`, `cuda`, `rocm` | The offline RE-USE model on a GPU (Burn) |
| `profile`, `trace` | Development diagnostics. Never measure latency with them |

## License

Apache-2.0. The model weights are NVIDIA's, under the NVIDIA One-Way Noncommercial License.
