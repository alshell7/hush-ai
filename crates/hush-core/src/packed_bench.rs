//! Focused development benchmark; never runs in the normal test suite or application.
use super::*;
use std::{hint::black_box, time::Instant};

#[test]
#[ignore = "timing experiment: run alone with --release --lib conv_panel_layouts -- --ignored --nocapture"]
#[allow(clippy::assertions_on_constants)]
fn conv_panel_layouts() {
    // Keep this a runtime failure only when someone explicitly runs the ignored benchmark
    // with profiling. A const assertion would prevent normal profile-feature test builds.
    assert!(
        !cfg!(feature = "profile"),
        "use a build without profiling counters"
    );
    let Some(width) = preferred_width() else {
        return;
    };
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(8)
        .build()
        .unwrap();
    // Representative 48 kHz encoder/decoder geometry. Three adjacent frequency taps share
    // their source row, as in convolution history; this is not a whole-frame latency benchmark.
    for (rows, source_len, reduction, stride) in [
        (963, 963, 2304, 1),
        (481, 481, 2304, 1),
        (963, 963, 576, 1),
        (481, 481, 576, 1),
        (481, 963, 192, 2),
    ] {
        let outputs = 64;
        let sources: Vec<_> = (0..(reduction / 3) * source_len)
            .map(|i| ((i % 97) as f32 - 48.0) / 128.0)
            .collect();
        let weights: Vec<_> = (0..reduction * outputs)
            .map(|i| ((i % 31) as f32 - 15.0) / 64.0)
            .collect();
        let packed = PackedWeights::pack(&weights, reduction, outputs, width);
        let source = |p: usize| {
            (
                &sources[(p / 3) * source_len..][..source_len],
                (p % 3) as isize - if stride == 1 { 1 } else { 0 },
            )
        };
        let mut output = vec![0.0; rows * outputs];
        let mut scratch: [Vec<f32>; 7] = std::array::from_fn(|_| Vec::new());
        let mut elapsed: [Vec<f64>; 7] = std::array::from_fn(|_| Vec::new());
        pool.install(|| {
            let mut expected = Vec::new();
            for iteration in 0..1100 {
                for turn in 0..7 {
                    let variant = (turn + iteration) % 7;
                    let scratch = &mut scratch[variant];
                    let start = Instant::now();
                    let ran = match variant {
                        0 => packed.multiply_conv_panel::<1, false, _>(
                            rows,
                            stride,
                            scratch,
                            &mut output,
                            source,
                        ),
                        1 => packed.multiply_conv_panel::<2, false, _>(
                            rows,
                            stride,
                            scratch,
                            &mut output,
                            source,
                        ),
                        2 => packed.multiply_conv_panel::<2, true, _>(
                            rows,
                            stride,
                            scratch,
                            &mut output,
                            source,
                        ),
                        3 => packed.multiply_conv_panel::<4, false, _>(
                            rows,
                            stride,
                            scratch,
                            &mut output,
                            source,
                        ),
                        4 => packed.multiply_conv_panel::<4, true, _>(
                            rows,
                            stride,
                            scratch,
                            &mut output,
                            source,
                        ),
                        5 => packed.multiply_conv_panel::<8, false, _>(
                            rows,
                            stride,
                            scratch,
                            &mut output,
                            source,
                        ),
                        6 => packed.multiply_conv_panel::<8, true, _>(
                            rows,
                            stride,
                            scratch,
                            &mut output,
                            source,
                        ),
                        _ => unreachable!(),
                    };
                    let ms = start.elapsed().as_secs_f64() * 1000.0;
                    assert!(ran);
                    black_box(&output);
                    if iteration == 0 {
                        if variant == 0 {
                            expected.clone_from(&output);
                        } else {
                            assert_eq!(output, expected);
                        }
                    }
                    if iteration >= 100 {
                        elapsed[variant].push(ms);
                    }
                }
            }
        });
        for (variant, values) in elapsed.iter_mut().enumerate() {
            values.sort_by(f64::total_cmp);
            let (tiles, padding) = [
                (1, false),
                (2, false),
                (2, true),
                (4, false),
                (4, true),
                (8, false),
                (8, true),
            ][variant];
            println!("conv_panel_bench rows={rows} reduction={reduction} stride={stride} width={width} tiles={tiles} padding={padding} mean_ms={:.5} p50_ms={:.5} p95_ms={:.5} p99_ms={:.5}",
                values.iter().sum::<f64>() / values.len() as f64, values[499], values[949], values[989]);
        }
    }
}

// Experimental layouts are compiled only into this ignored development benchmark.
impl PackedWeights {
    fn multiply_conv_panel<'a, const TILES: usize, const PAD: bool, F>(
        &self,
        rows: usize,
        stride: usize,
        scratch: &mut Vec<f32>,
        output: &mut [f32],
        source: F,
    ) -> bool
    where
        F: Fn(usize) -> (&'a [f32], isize) + Sync,
    {
        if rows < self.width
            || rows.saturating_mul(self.input).saturating_mul(self.output) < 500_000
        {
            return false;
        }
        #[cfg(feature = "profile")]
        let _timer = crate::profile::scope("tiled_conv_gemm");
        assert_eq!(output.len(), rows * self.output);
        let width = self.width;
        let padded_output = self.output.div_ceil(width) * width;
        let block = rows.div_ceil(rayon::current_num_threads()).div_ceil(width) * width;
        let tasks = rows.div_ceil(block);
        let panel_rows = (width * TILES).min(block);
        let panel_stride = panel_rows + if PAD { width } else { 0 };
        scratch.resize(tasks * KC * panel_stride, 0.0);
        output
            .par_chunks_mut(block * self.output)
            .zip(scratch.par_chunks_mut(KC * panel_stride))
            .enumerate()
            .for_each(|(task, (dst, tile))| {
                let start = task * block;
                let count = block.min(rows - start);
                #[cfg(feature = "profile")]
                let (mut packing_time, mut product_time, mut packing_calls, mut tile_count) = (
                    std::time::Duration::ZERO,
                    std::time::Duration::ZERO,
                    0u64,
                    0u64,
                );
                for pc in (0..self.input).step_by(KC) {
                    let kc = KC.min(self.input - pc);
                    for panel_start in (0..count).step_by(panel_rows) {
                        let panel_count = panel_rows.min(count - panel_start);
                        #[cfg(feature = "profile")]
                        let packing_start = std::time::Instant::now();
                        for p in 0..kc {
                            let (src, offset) = source(pc + p);
                            let first = ((start + panel_start) * stride) as isize + offset;
                            let row = &mut tile[p * panel_stride..][..panel_count];
                            if stride == 1
                                && first >= 0
                                && first as usize + panel_count <= src.len()
                            {
                                row.copy_from_slice(&src[first as usize..][..panel_count]);
                            } else {
                                for (j, value) in row.iter_mut().enumerate() {
                                    let index = first + (j * stride) as isize;
                                    *value = if index >= 0 {
                                        src.get(index as usize).copied().unwrap_or(0.0)
                                    } else {
                                        0.0
                                    };
                                }
                            }
                        }
                        #[cfg(feature = "profile")]
                        {
                            packing_time += packing_start.elapsed();
                            packing_calls += 1;
                        }
                        for r in (0..panel_count).step_by(width) {
                            let mr = width.min(panel_count - r);
                            #[cfg(feature = "profile")]
                            let product_start = std::time::Instant::now();
                            for col in (0..self.output).step_by(width) {
                                let nr = width.min(self.output - col);
                                let b = &self.data[pc * padded_output + col * kc..];
                                let c = &mut dst[(panel_start + r) * self.output + col..];
                                // SAFETY: tile holds kc columns with panel_stride elements. The
                                // kernels read only mr rows; output tiles are bounded and disjoint.
                                // Construction has checked the selected CPU feature set.
                                unsafe {
                                    #[cfg(target_arch = "x86_64")]
                                    match width {
                                        16 => tile_avx512(
                                            &tile[r..],
                                            b,
                                            c,
                                            kc,
                                            mr,
                                            nr,
                                            1,
                                            panel_stride,
                                            self.output,
                                            pc != 0,
                                        ),
                                        8 => tile_avx2(
                                            &tile[r..],
                                            b,
                                            c,
                                            kc,
                                            mr,
                                            nr,
                                            1,
                                            panel_stride,
                                            self.output,
                                            pc != 0,
                                        ),
                                        _ => unreachable!(),
                                    }
                                    #[cfg(not(target_arch = "x86_64"))]
                                    tile_scalar(
                                        &tile[r..],
                                        b,
                                        c,
                                        kc,
                                        mr,
                                        nr,
                                        1,
                                        panel_stride,
                                        self.output,
                                        pc != 0,
                                        width,
                                    );
                                }
                            }
                            #[cfg(feature = "profile")]
                            {
                                product_time += product_start.elapsed();
                                tile_count += 1;
                            }
                        }
                    }
                }
                #[cfg(feature = "profile")]
                {
                    crate::profile::record("conv_panel_pack_worker", packing_calls, packing_time);
                    crate::profile::record("conv_tile_product_worker", tile_count, product_time);
                }
            });
        true
    }
}
