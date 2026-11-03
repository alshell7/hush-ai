# Accuracy

[Home](../README.md) · [Architecture](ARCHITECTURE.md) · [Performance](PERFORMANCE.md)

Hush uses NVIDIA's original FP32 weights without change: no training, fine-tuning or
quantization. This page compares Hush's CPU output with NVIDIA's own GPU code.

## Summary

| Check | Result |
| --- | --- |
| Seven saved NVIDIA regression cases | **Pass** on Windows x64, Linux x64 and macOS (M1), in CI |
| Eight short audio cases (8–48 kHz) | Waveform SNR 113–118 dB to every GPU run |
| 60 s continuous case, 48 kHz | Waveform SNR **100.7 dB**, peak error **6.9e-6** |
| GPU-to-GPU spread, same 60 s case | SNR 118–121 dB, peak error 0.8–1.4e-6 |

The CPU output differs from the GPU output at about −100 dB or less: far below what anyone
can hear. On long streams the difference is about 5 times the spread between two GPUs, so
the output is not bit-identical to a GPU.

## Regression gates

[`gpu_reference.rs`](../crates/hush-core/tests/gpu_reference.rs) runs seven saved GPU
cases on every CI run. Each quantity must reach the SNR **and** stay below the peak error.

| Quantity | Short traces | Four fused profiles |
| --- | --- | --- |
| Enhanced log magnitude | 90 dB / 1e-5 | 90 dB / 1e-5 |
| Enhanced phase (wrapped, rad) | 90 dB / 1e-3 | 82 dB / 2e-3 |
| Temporal convolution state | 110 dB / 1e-4 | 103 dB / 1e-4 |
| Temporal SSM state | 110 dB / 1e-4 | 100 dB / 1e-4 |

The speech waveform case uses **100 dB / 2e-6**. The profiles cover 8, 16, 44.1 and
48 kHz, 3 to 12 layers and look-ahead 0 to 2.

## GPU reference spread

Two runs of the same FP32 model on different GPUs do not give identical output, because
the order of floating-point additions differs. To find a fair target, NVIDIA's pinned code
ran on T4 and A10G GPUs, twice each, from the same input spectra.

| Case | CPU vs GPU: peak / SNR | GPU vs GPU: peak / SNR |
| --- | --- | --- |
| Silence, 8 kHz | 1.5e-10 / 91.8 dB | 1.9e-10 / 90.5 dB |
| Impulses, 16 kHz, look-ahead 1 | 7.0e-9 / 102.2 dB | 9.0e-9 / 100.5 dB |
| Clipping, 24 kHz | 3.0e-6 / 111.3 dB | 1.6e-6 / 116.0 dB |
| Speech, 8 kHz | 3.8e-7 / 115.8 dB | 2.1e-7 / 122.2 dB |
| Speech, 16 kHz, look-ahead 1 | 4.5e-7 / 117.5 dB | 2.5e-7 / 123.3 dB |
| Speech, 44.1 kHz, 8 layers, look-ahead 2 | 6.2e-7 / 113.9 dB | 4.2e-7 / 119.4 dB |
| Speech, 48 kHz | 6.3e-7 / 113.0 dB | 2.4e-7 / 122.4 dB |
| Overlapped speech, 32 kHz, look-ahead 2 | 3.7e-7 / 117.4 dB | 2.0e-7 / 123.5 dB |
| Continuous 60 s, 48 kHz | 6.9e-6 / 100.7 dB | 1.4e-6 / 118.1 dB |

### Where the long-stream difference comes from

The SSM state has a long memory and integrates very small input differences over time; the
convolution state remembers only 4 frames and stays flat. Making the CPU math more exact
(correctly rounded `exp` and `softplus`, fused multiply-add, f64 accumulation) did **not**
reduce the difference to the GPU. Copying the GPU's approximate math did. So the gap comes
mostly from approximate math in NVIDIA's CUDA kernels, which all NVIDIA GPUs share.

## Known differences

- NVIDIA's code fails to reset at 22.05 kHz (cache shape). Hush runs 22.05 kHz, but there
  is no GPU reference for it.
- Native STFT phase can take a different branch near ±π for very small bins.

## Run the gates

```sh
export HUSH_MODEL=$PWD/models/model.safetensors
cargo test -p hush-core --release --test gpu_reference -- --test-threads=1
```
