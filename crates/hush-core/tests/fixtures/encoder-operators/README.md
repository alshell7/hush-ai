# Controlled encoder fixtures

These two pairs come from the original NVIDIA encoder running on Modal T4 with
PyTorch 2.11.0 / CUDA 13.0. The capture preserves the unhooked encoder stream and
all 36 selected boundaries from the earlier full-model run.

| Pair | Check |
| --- | --- |
| Frame 458, input stage | Original padded convolution input through native convolution and diagnostic CUDA normalization/PReLU |
| Frame 50, dense layer 2 | Original convolution output through diagnostic CUDA normalization |

`manifest.json` records source, capture and payload hashes. Values are little-endian
FP32 in the recorded layout. `scripts/verify_gpu_fixtures.py` checks their bytes;
`encoder_blocks_and_norm_match_controlled_t4` compares every output bit.

These are isolated operator regressions. Full-stream audio parity remains open.
the development history archive ([Development](../../../docs/DEVELOPMENT.md#archive)).
