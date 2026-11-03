# Complete frequency-block controls

Three input/output pairs from the independently controlled Modal T4 stage capture:
frame/layer **0/0, 1381/1 and 1781/2**. Each tensor contains 481 × 64 little-endian FP32
values. Inputs are `time_residual`; outputs are the complete `freq` block, including
bidirectional Mamba, projections and normalization.

The manifest preserves source filenames, shapes, hashes, the pinned checkpoint and NVIDIA
source revision. The source trace's hooked/unhooked full outputs are identical. Its output
still differs from the audited Colab reference; these fixtures do not replace that reference.

`complete_frequency_blocks_match_controlled_t4` checks every output bit after recomputing
all intermediates from the original weights and saved block input. It uses the development-only
`trace` feature and `ExpNormSfu` arithmetic. No model intermediate is injected inside the block.
`scripts/verify_gpu_fixtures.py` verifies the six payload hashes and sizes in CI.

The development history archive ([Development](../../../docs/DEVELOPMENT.md#archive))
and full-stream checks retain broader evidence. These three cases are compact regression
coverage, not a substitute for that replay or proof of full audio parity.
