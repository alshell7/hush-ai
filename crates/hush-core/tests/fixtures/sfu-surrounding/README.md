# Frequency convolution and gate witnesses

These are samples from the unchanged, controlled NVIDIA T4 capture used by the
frequency-inner diagnostic. All 72 convolution outputs and 72 gates were first
reproduced exactly with Rust FMA and the pinned envytools SFU algorithms.

| File | Layout per witness |
| --- | --- |
| `convolution.f32` | Bias, four weight/input pairs, expected GPU activated output |
| `gate.f32` | Gate input z, ungated scan including D*x, expected GPU gated output |

All values use little-endian FP32. Each file contains 2,808 witnesses, selected at
the deterministic indices in `manifest.json`. The manifest pins weights and the
original GPU trace. Rust tests compare bit patterns; Python checks payload hashes.

These captured model values are not an independent reciprocal hardware sweep.
They do not establish full model parity or justify changing normal inference.
No microphone audio is included.
