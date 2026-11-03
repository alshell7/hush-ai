# Exponential and normalization witnesses

`witnesses.u32` contains 5,396 deterministic rows from 22,024,467 independent T4
inputs: group, index, input, CUDA expf, explicit expf formula, FTZ reciprocal square
root, non-FTZ library reciprocal square root, host expf and host FTZ reciprocal
square root. Each row is nine little-endian u32 values. NaN/Inf payloads are intentional.

`norm.f32` contains 30 saved rows from six controlled blocks. Each 258-value row is
64 inputs, mean, reciprocal standard deviation, 64 weights, 64 biases and 64 outputs.
The manifest identifies original inputs, weights, captures and selected rows.

The entire Rust exponential/FTZ-rsqrt output stream matches the independent GPU
SHA-256 in the manifest. Non-FTZ CUDA rsqrtf differs on subnormal inputs; the model's
normalization variance plus epsilon is at least 1e-5. These are diagnostic witnesses,
not evidence of complete model or audio parity. No microphone data is included.
