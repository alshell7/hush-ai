# Independent T4 exponential witnesses

A standalone T4 probe compared 19,927,297 inputs against inline PTX, CUDA `exp2f`
and the independently published envytools model. All agree. The probe source is in
the development history archive ([Development](../../../docs/DEVELOPMENT.md#archive)).
The inputs are numerical grids, integer neighbors and hashed FP32 bit patterns;
they contain no model data or recordings.

`witnesses.u32` retains every 4096th input plus every mismatch (none): 4,866 rows.
[The manifest](manifest.json) defines the binary format, generator, source hashes
and GPU environment. The Rust trace test compares its port against these witnesses;
the Python test verifies the hash, input generator and coverage. This is sampled
evidence for T4, not a proof over every FP32 input or every NVIDIA architecture.
