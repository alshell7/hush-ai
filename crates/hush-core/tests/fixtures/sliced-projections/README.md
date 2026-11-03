# Projection arithmetic witnesses

Five original GPU rows from frame 0 / layer 0 of the controlled frequency-inner
capture: forward x projection, backward output projection and block projection.
Each file contains little-endian FP32 values. The manifest pins payloads, source
tensor hashes and the original checkpoint; no microphone recording is included.

The checkpoint-backed trace test cycles these rows to 481 positions so the existing
packed CPU implementation executes at the measured shape. Matrix rows are independent.
The result must reproduce every saved output bit. The scalar fallback uses the same
four-slice order. The separate full replay checks all 180 original projection outputs.
