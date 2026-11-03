//! Immutable FP32 weight panels. The 256-element reduction blocks retain the existing
//! matrixmultiply accumulation boundaries. Callers retain their portable GEMM fallback.

use rayon::prelude::*;

const KC: usize = 256;

pub(crate) struct PackedWeights {
    data: Vec<f32>,
    input: usize,
    output: usize,
    width: usize,
}

impl PackedWeights {
    /// Weights are [output,input]. macOS continues to use Accelerate.
    pub fn new(weights: &[f32], input: usize, output: usize) -> Option<Self> {
        let width = preferred_width()?;
        Some(Self::pack(weights, input, output, width))
    }

    fn pack(weights: &[f32], input: usize, output: usize, width: usize) -> Self {
        assert!(input > 0 && output > 0 && weights.len() == input * output);
        let mut data = Vec::with_capacity(input * output.div_ceil(width) * width);
        for pc in (0..input).step_by(KC) {
            let end = (pc + KC).min(input);
            for col in (0..output).step_by(width) {
                for k in pc..end {
                    for j in col..col + width {
                        data.push(if j < output {
                            weights[j * input + k]
                        } else {
                            0.0
                        });
                    }
                }
            }
        }
        Self {
            data,
            input,
            output,
            width,
        }
    }

    /// Pack only the small input tile being consumed, directly from convolution history.
    /// This replaces the full im2col matrix. Scratch is retained by the owning convolution.
    pub fn multiply_conv<'a, F>(
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
        // This also bounds every row index below isize::MAX: a valid f32 output slice
        // cannot contain more than isize::MAX bytes, and self.output is positive.
        assert_eq!(rows.checked_mul(self.output), Some(output.len()));
        let width = self.width;
        let padded_output = self.output.div_ceil(width) * width;
        let block = rows.div_ceil(rayon::current_num_threads()).div_ceil(width) * width;
        let tasks = rows.div_ceil(block);
        scratch.resize(tasks * KC * width, 0.0);
        // Resolve history/channel slices once; every frequency tile reads the same sources.
        let sources: Vec<_> = (0..self.input).map(source).collect();
        // The direct kernel loads 16 adjacent frequencies from every source. Intersect
        // their valid ranges once; padding, strided inputs and tails keep the packed path.
        let interior_start = sources
            .iter()
            .map(|(_, offset)| offset.saturating_neg())
            .max()
            .unwrap()
            .max(0);
        let interior_end = sources
            .iter()
            .map(|(src, offset)| (src.len() as isize).saturating_sub(*offset))
            .min()
            .unwrap();
        output
            .par_chunks_mut(block * self.output)
            .zip(scratch.par_chunks_mut(KC * width))
            .enumerate()
            .for_each(|(task, (dst, tile))| {
                let start = task * block;
                let count = block.min(rows - start);
                #[cfg(feature = "profile")]
                let (mut packing_time, mut product_time, mut tile_count) =
                    (std::time::Duration::ZERO, std::time::Duration::ZERO, 0u64);
                for pc in (0..self.input).step_by(KC) {
                    let kc = KC.min(self.input - pc);
                    for r in (0..count).step_by(width) {
                        let mr = width.min(count - r);
                        let direct = cfg!(target_arch = "x86_64")
                            && width == 16
                            && stride == 1
                            && mr == 16
                            && (start + r) as isize >= interior_start
                            && ((start + r + mr) as isize) <= interior_end
                            && self.output <= i32::MAX as usize / 16;
                        #[cfg(feature = "profile")]
                        let packing_start = std::time::Instant::now();
                        for p in 0..if direct { 0 } else { kc } {
                            let (src, offset) = sources[pc + p];
                            let first = ((start + r) * stride) as isize + offset;
                            let row = &mut tile[p * width..][..mr];
                            if stride == 1 && first >= 0 && first as usize + mr <= src.len() {
                                row.copy_from_slice(&src[first as usize..][..mr]);
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
                            tile_count += 1;
                        }
                        #[cfg(feature = "profile")]
                        let product_start = std::time::Instant::now();
                        for col in (0..self.output).step_by(width) {
                            let nr = width.min(self.output - col);
                            let b = &self.data[pc * padded_output + col * kc..];
                            let c = &mut dst[r * self.output + col..];
                            // SAFETY: construction checked the selected CPU feature set.
                            // Packed kernels read only mr rows from the padded scratch tile.
                            // The direct path has 16 valid source values; its masked row stores
                            // preserve output tails. Every output tile is bounded and disjoint.
                            unsafe {
                                #[cfg(target_arch = "x86_64")]
                                if direct {
                                    conv_direct_avx512(
                                        &sources[pc..pc + kc],
                                        start + r,
                                        b,
                                        c,
                                        nr,
                                        self.output,
                                        pc != 0,
                                    );
                                    continue;
                                }
                                #[cfg(target_arch = "x86_64")]
                                match width {
                                    16 => tile_avx512(
                                        tile,
                                        b,
                                        c,
                                        kc,
                                        mr,
                                        nr,
                                        1,
                                        width,
                                        self.output,
                                        pc != 0,
                                    ),
                                    8 => tile_avx2(
                                        tile,
                                        b,
                                        c,
                                        kc,
                                        mr,
                                        nr,
                                        1,
                                        width,
                                        self.output,
                                        pc != 0,
                                    ),
                                    _ => unreachable!(),
                                }
                                #[cfg(not(target_arch = "x86_64"))]
                                tile_scalar(
                                    tile,
                                    b,
                                    c,
                                    kc,
                                    mr,
                                    nr,
                                    1,
                                    width,
                                    self.output,
                                    pc != 0,
                                    width,
                                );
                            }
                        }
                        #[cfg(feature = "profile")]
                        {
                            product_time += product_start.elapsed();
                        }
                    }
                }
                #[cfg(feature = "profile")]
                {
                    crate::profile::record("conv_tile_pack_worker", tile_count, packing_time);
                    crate::profile::record("conv_tile_product_worker", tile_count, product_time);
                }
            });
        true
    }

    /// C = X W^T. X's row/column strides support both convolution columns and linear rows.
    /// Returns false for small products, for which the caller uses its existing GEMM.
    pub fn multiply(
        &self,
        x: &[f32],
        rows: usize,
        row_stride: usize,
        col_stride: usize,
        output: &mut [f32],
        output_stride: usize,
    ) -> bool {
        if rows < self.width
            || rows.saturating_mul(self.input).saturating_mul(self.output) < 500_000
        {
            return false;
        }
        #[cfg(feature = "profile")]
        let _timer = crate::profile::scope("prepacked_gemm");
        assert!(x.len() > (rows - 1) * row_stride + (self.input - 1) * col_stride);
        assert!(
            output_stride >= self.output
                && output.len() >= (rows - 1) * output_stride + self.output
        );
        let width = self.width;
        let padded_output = self.output.div_ceil(width) * width;
        let block = rows.div_ceil(rayon::current_num_threads()).div_ceil(width) * width;
        output[..(rows - 1) * output_stride + self.output]
            .par_chunks_mut(block * output_stride)
            .enumerate()
            .for_each(|(task, dst)| {
                let start = task * block;
                let count = block.min(rows - start);
                for pc in (0..self.input).step_by(KC) {
                    let kc = KC.min(self.input - pc);
                    for r in (0..count).step_by(width) {
                        let mr = width.min(count - r);
                        for col in (0..self.output).step_by(width) {
                            let nr = width.min(self.output - col);
                            let a = &x[(start + r) * row_stride + pc * col_stride..];
                            let b = &self.data[pc * padded_output + col * kc..];
                            let c = &mut dst[r * output_stride + col..];
                            // SAFETY: constructors select only CPU-supported kernels; the
                            // checks above cover X/C strides, and packing pads every B panel.
                            // Each Rayon task owns disjoint complete output rows.
                            unsafe {
                                #[cfg(target_arch = "x86_64")]
                                match width {
                                    16 => tile_avx512(
                                        a,
                                        b,
                                        c,
                                        kc,
                                        mr,
                                        nr,
                                        row_stride,
                                        col_stride,
                                        output_stride,
                                        pc != 0,
                                    ),
                                    8 => tile_avx2(
                                        a,
                                        b,
                                        c,
                                        kc,
                                        mr,
                                        nr,
                                        row_stride,
                                        col_stride,
                                        output_stride,
                                        pc != 0,
                                    ),
                                    _ => unreachable!(),
                                }
                                #[cfg(not(target_arch = "x86_64"))]
                                tile_scalar(
                                    a,
                                    b,
                                    c,
                                    kc,
                                    mr,
                                    nr,
                                    row_stride,
                                    col_stride,
                                    output_stride,
                                    pc != 0,
                                    width,
                                );
                            }
                        }
                    }
                }
            });
        true
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
unsafe fn conv_direct_avx512(
    sources: &[(&[f32], isize)],
    first: usize,
    b: &[f32],
    c: &mut [f32],
    nr: usize,
    ldc: usize,
    add: bool,
) {
    use std::arch::x86_64::*;
    debug_assert!(nr <= 16 && ldc >= nr && ldc <= i32::MAX as usize / 16);
    debug_assert!(c.len() >= 15 * ldc + nr && b.len() >= sources.len() * 16);
    // Vectorize across input frequencies, broadcasting one output weight at a time.
    // Each result keeps the packed kernel's p-order FMA and 256-value block sum.
    // Transpose the accumulators before storing the existing position-major output.
    let mut accum = [_mm512_setzero_ps(); 16];
    for (p, &(src, offset)) in sources.iter().enumerate() {
        let at = (first as isize + offset) as usize;
        let values = _mm512_loadu_ps(src[at..at + 16].as_ptr());
        for (j, acc) in accum.iter_mut().enumerate() {
            *acc = _mm512_fmadd_ps(values, _mm512_set1_ps(*b.get_unchecked(p * 16 + j)), *acc);
        }
    }
    let accum = transpose16_avx512(accum);
    let mask = if nr == 16 { u16::MAX } else { (1u16 << nr) - 1 };
    for (r, acc) in accum.iter().enumerate() {
        let dst = c.as_mut_ptr().add(r * ldc);
        let old = if add {
            _mm512_maskz_loadu_ps(mask, dst)
        } else {
            _mm512_setzero_ps()
        };
        _mm512_mask_storeu_ps(dst, mask, _mm512_add_ps(old, *acc));
    }
}

/// Rearrange a 16x16 tile without arithmetic, preserving every floating-point bit.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
#[inline]
unsafe fn transpose16_avx512(
    mut rows: [std::arch::x86_64::__m512; 16],
) -> [std::arch::x86_64::__m512; 16] {
    use std::arch::x86_64::*;
    // First transpose each 4x4 group within independent 128-bit lanes.
    for base in (0..16).step_by(4) {
        let a = _mm512_unpacklo_ps(rows[base], rows[base + 1]);
        let b = _mm512_unpackhi_ps(rows[base], rows[base + 1]);
        let c = _mm512_unpacklo_ps(rows[base + 2], rows[base + 3]);
        let d = _mm512_unpackhi_ps(rows[base + 2], rows[base + 3]);
        rows[base] = _mm512_shuffle_ps::<0x44>(a, c);
        rows[base + 1] = _mm512_shuffle_ps::<0xee>(a, c);
        rows[base + 2] = _mm512_shuffle_ps::<0x44>(b, d);
        rows[base + 3] = _mm512_shuffle_ps::<0xee>(b, d);
    }
    // Select lanes 0/2 or 1/3 from each input, then join the four channel groups.
    for i in 0..4 {
        let a = _mm512_shuffle_f32x4::<0x88>(rows[i], rows[i + 4]);
        let b = _mm512_shuffle_f32x4::<0xdd>(rows[i], rows[i + 4]);
        let c = _mm512_shuffle_f32x4::<0x88>(rows[i + 8], rows[i + 12]);
        let d = _mm512_shuffle_f32x4::<0xdd>(rows[i + 8], rows[i + 12]);
        rows[i] = _mm512_shuffle_f32x4::<0x88>(a, c);
        rows[i + 4] = _mm512_shuffle_f32x4::<0x88>(b, d);
        rows[i + 8] = _mm512_shuffle_f32x4::<0xdd>(a, c);
        rows[i + 12] = _mm512_shuffle_f32x4::<0xdd>(b, d);
    }
    rows
}

fn preferred_width() -> Option<usize> {
    #[cfg(all(target_arch = "x86_64", not(target_os = "macos")))]
    {
        if std::is_x86_feature_detected!("avx512f") {
            return Some(16);
        }
        if std::is_x86_feature_detected!("avx2") && std::is_x86_feature_detected!("fma") {
            return Some(8);
        }
    }
    None
}

#[cfg(test)]
#[path = "packed_bench.rs"]
mod benchmark;

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
#[allow(clippy::too_many_arguments)]
unsafe fn tile_avx512(
    a: &[f32],
    b: &[f32],
    c: &mut [f32],
    k: usize,
    mr: usize,
    nr: usize,
    rs: usize,
    cs: usize,
    ldc: usize,
    add: bool,
) {
    // The common convolution tile has contiguous rows and no row tail. Specializing
    // it removes generic per-row address calculation from the reduction loop.
    if mr == 16 && rs == 1 {
        tile_avx512_impl::<true>(a, b, c, k, mr, nr, rs, cs, ldc, add);
    } else {
        tile_avx512_impl::<false>(a, b, c, k, mr, nr, rs, cs, ldc, add);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
#[inline]
#[allow(clippy::too_many_arguments)]
unsafe fn tile_avx512_impl<const FULL_CONTIGUOUS: bool>(
    a: &[f32],
    b: &[f32],
    c: &mut [f32],
    k: usize,
    mr: usize,
    nr: usize,
    rs: usize,
    cs: usize,
    ldc: usize,
    add: bool,
) {
    use std::arch::x86_64::*;
    let mut accum = [_mm512_setzero_ps(); 16];
    for p in 0..k {
        let weights = _mm512_loadu_ps(b.as_ptr().add(p * 16));
        // Duplicating the first input row for an incomplete final tile avoids out-of-bounds
        // reads while allowing the fixed-size accumulator loop to stay in registers.
        for (r, acc) in accum.iter_mut().enumerate() {
            let at = if FULL_CONTIGUOUS {
                p * cs + r
            } else {
                let r = if r < mr { r } else { 0 };
                r * rs + p * cs
            };
            *acc = _mm512_fmadd_ps(_mm512_set1_ps(*a.get_unchecked(at)), weights, *acc);
        }
    }
    let mask = if nr == 16 { u16::MAX } else { (1u16 << nr) - 1 };
    for (r, acc) in accum
        .iter()
        .take(if FULL_CONTIGUOUS { 16 } else { mr })
        .enumerate()
    {
        let dst = c.as_mut_ptr().add(r * ldc);
        let old = if add {
            _mm512_maskz_loadu_ps(mask, dst)
        } else {
            _mm512_setzero_ps()
        };
        _mm512_mask_storeu_ps(dst, mask, _mm512_add_ps(old, *acc));
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
#[allow(clippy::too_many_arguments)]
unsafe fn tile_avx2(
    a: &[f32],
    b: &[f32],
    c: &mut [f32],
    k: usize,
    mr: usize,
    nr: usize,
    rs: usize,
    cs: usize,
    ldc: usize,
    add: bool,
) {
    // The common convolution tile has contiguous rows and no row tail. Specializing
    // it removes generic per-row address calculation from the reduction loop.
    if mr == 8 && rs == 1 {
        tile_avx2_impl::<true>(a, b, c, k, mr, nr, rs, cs, ldc, add);
    } else {
        tile_avx2_impl::<false>(a, b, c, k, mr, nr, rs, cs, ldc, add);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
#[inline]
#[allow(clippy::too_many_arguments)]
unsafe fn tile_avx2_impl<const FULL_CONTIGUOUS: bool>(
    a: &[f32],
    b: &[f32],
    c: &mut [f32],
    k: usize,
    mr: usize,
    nr: usize,
    rs: usize,
    cs: usize,
    ldc: usize,
    add: bool,
) {
    use std::arch::x86_64::*;
    let mut accum = [_mm256_setzero_ps(); 8];
    for p in 0..k {
        let weights = _mm256_loadu_ps(b.as_ptr().add(p * 8));
        for (r, acc) in accum.iter_mut().enumerate() {
            let at = if FULL_CONTIGUOUS {
                p * cs + r
            } else {
                let r = if r < mr { r } else { 0 };
                r * rs + p * cs
            };
            *acc = _mm256_fmadd_ps(_mm256_set1_ps(*a.get_unchecked(at)), weights, *acc);
        }
    }
    for (r, acc) in accum
        .iter()
        .take(if FULL_CONTIGUOUS { 8 } else { mr })
        .enumerate()
    {
        let dst = &mut c[r * ldc..][..nr];
        let mut values = [0.0f32; 8];
        _mm256_storeu_ps(values.as_mut_ptr(), *acc);
        for (value, partial) in dst.iter_mut().zip(values) {
            *value = if add { *value } else { 0.0 } + partial;
        }
    }
}

#[cfg(not(target_arch = "x86_64"))]
#[allow(clippy::too_many_arguments)]
unsafe fn tile_scalar(
    a: &[f32],
    b: &[f32],
    c: &mut [f32],
    k: usize,
    mr: usize,
    nr: usize,
    rs: usize,
    cs: usize,
    ldc: usize,
    add: bool,
    width: usize,
) {
    for r in 0..mr {
        for j in 0..nr {
            let mut acc = 0.0f32;
            for p in 0..k {
                acc = a[r * rs + p * cs].mul_add(b[p * width + j], acc);
            }
            c[r * ldc + j] = if add { c[r * ldc + j] } else { 0.0 } + acc;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic]
    fn convolution_rejects_overflowing_output_shape_before_access() {
        let packed = PackedWeights::pack(&[0.0; 3 * 64], 3, 64, 16);
        let rows = usize::MAX / 64 + 1;
        let mut scratch = Vec::new();
        packed.multiply_conv(rows, 1, &mut scratch, &mut [], |_| (&[], 0));
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn register_transpose_preserves_every_lane_bit_including_nonfinite_values() {
        if !std::is_x86_feature_detected!("avx512f") {
            return;
        }
        use std::arch::x86_64::*;
        let special = [
            0,
            0x8000_0000,
            0x7f80_0000,
            0xff80_0000,
            0x7fc0_1234,
            0x7f80_0001,
            1,
        ];
        let input: [[f32; 16]; 16] = std::array::from_fn(|r| {
            std::array::from_fn(|c| {
                let i = r * 16 + c;
                f32::from_bits(
                    special
                        .get(i)
                        .copied()
                        .unwrap_or_else(|| (i as u32).wrapping_mul(0x9e37_79b9).rotate_left(7)),
                )
            })
        });
        // SAFETY: AVX-512 was checked above and each load/store covers exactly 16 floats.
        unsafe {
            let rows = std::array::from_fn(|r| _mm512_loadu_ps(input[r].as_ptr()));
            let transposed = transpose16_avx512(rows);
            for (r, row) in transposed.into_iter().enumerate() {
                let mut actual = [0.0; 16];
                _mm512_storeu_ps(actual.as_mut_ptr(), row);
                for c in 0..16 {
                    assert_eq!(
                        actual[c].to_bits(),
                        input[c][r].to_bits(),
                        "row {r}, column {c}"
                    );
                }
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn contiguous_full_tiles_preserve_generic_bits_and_output_padding() {
        for width in [8, 16] {
            let supported = if width == 8 {
                std::is_x86_feature_detected!("avx2") && std::is_x86_feature_detected!("fma")
            } else {
                std::is_x86_feature_detected!("avx512f")
            };
            if !supported {
                continue;
            }
            for k in [3, 63, 256, 263] {
                for nr in [1, width - 1, width] {
                    for add in [false, true] {
                        let (cs, ldc) = (width + 3, width + 5);
                        let a: Vec<_> = (0..k * cs)
                            .map(|i| ((i * 73 % 139) as f32 - 69.0) * 0.0314159)
                            .collect();
                        let b: Vec<_> = (0..k * width)
                            .map(|i| ((i * 37 % 151) as f32 - 75.0) * 0.00271828)
                            .collect();
                        let mut expected = vec![-0.375; width * ldc];
                        let mut actual = expected.clone();
                        // SAFETY: supported CPU checked above; slices cover the full padded
                        // reduction/input/output tiles. The two paths keep the same FMA order.
                        unsafe {
                            if width == 16 {
                                tile_avx512_impl::<false>(
                                    &a,
                                    &b,
                                    &mut expected,
                                    k,
                                    width,
                                    nr,
                                    1,
                                    cs,
                                    ldc,
                                    add,
                                );
                                tile_avx512(&a, &b, &mut actual, k, width, nr, 1, cs, ldc, add);
                                let sources: Vec<_> =
                                    (0..k).map(|p| (&a[p * cs..][..16], 0)).collect();
                                let mut direct = vec![-0.375; width * ldc];
                                conv_direct_avx512(&sources, 0, &b, &mut direct, nr, ldc, add);
                                assert!(direct
                                    .iter()
                                    .zip(&expected)
                                    .all(|(a, b)| a.to_bits() == b.to_bits()));
                            } else {
                                tile_avx2_impl::<false>(
                                    &a,
                                    &b,
                                    &mut expected,
                                    k,
                                    width,
                                    nr,
                                    1,
                                    cs,
                                    ldc,
                                    add,
                                );
                                tile_avx2(&a, &b, &mut actual, k, width, nr, 1, cs, ldc, add);
                            }
                        }
                        assert!(actual
                            .iter()
                            .zip(&expected)
                            .all(|(a, b)| a.to_bits() == b.to_bits()));
                        for row in actual.chunks_exact(ldc) {
                            assert!(row[nr..].iter().all(|v| *v == -0.375));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn convolution_panels_preserve_padding_strides_reduction_tails_and_pool_changes() {
        let Some(available_width) = preferred_width() else {
            return;
        };
        for width in [8, 16].into_iter().filter(|&w| w <= available_width) {
            #[cfg(target_arch = "x86_64")]
            if width == 8
                && !(std::is_x86_feature_detected!("avx2") && std::is_x86_feature_detected!("fma"))
            {
                continue;
            }
            let (rows, outputs, reduction) = (79, 35, 263);
            let weights: Vec<_> = (0..outputs * reduction)
                .map(|i| ((i % 11) as f32 - 5.0) / 64.0)
                .collect();
            let packed = PackedWeights::pack(&weights, reduction, outputs, width);
            let mut scratch = Vec::new();
            for (threads, stride) in [(3, 1), (8, 2), (1, 1)] {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap();
                pool.install(|| {
                    let source_len = rows * stride;
                    for step in 0..2 {
                        let sources: Vec<_> = (0..reduction * source_len)
                            .map(|i| (((i + step * 5) % 17) as f32 - 8.0) / 32.0)
                            .collect();
                        let source = |p: usize| {
                            (
                                &sources[p * source_len..][..source_len],
                                (p % 3) as isize - 1,
                            )
                        };
                        let mut expected = vec![0.0; rows * outputs];
                        for row in 0..rows {
                            for col in 0..outputs {
                                for p in 0..reduction {
                                    let (src, offset) = source(p);
                                    let at = (row * stride) as isize + offset;
                                    if let Ok(at) = usize::try_from(at) {
                                        expected[row * outputs + col] +=
                                            src.get(at).copied().unwrap_or(0.0)
                                                * weights[col * reduction + p];
                                    }
                                }
                            }
                        }
                        scratch.fill(f32::NAN);
                        let old_capacity = scratch.capacity();
                        let mut actual = vec![f32::NAN; rows * outputs];
                        assert!(packed.multiply_conv(
                            rows,
                            stride,
                            &mut scratch,
                            &mut actual,
                            source
                        ));
                        assert_eq!(actual, expected);
                        if step == 1 {
                            assert_eq!(scratch.capacity(), old_capacity);
                        }
                    }
                });
            }
        }
    }

    #[test]
    fn prepacked_products_match_gemm_with_strides_tails_and_reduction_blocks() {
        let Some(width) = preferred_width() else {
            return;
        };
        for nr in [8, 16].into_iter().filter(|&nr| nr <= width) {
            // AVX-512 machines used here also need FMA/AVX2 before explicitly testing that kernel.
            #[cfg(target_arch = "x86_64")]
            if nr == 8
                && !(std::is_x86_feature_detected!("avx2") && std::is_x86_feature_detected!("fma"))
            {
                continue;
            }
            for transpose in [false, true] {
                let (m, n, k) = (79, 35, 263);
                let lda = if transpose { m + 3 } else { k + 3 };
                let weights: Vec<_> = (0..n * k)
                    .map(|i| ((i % 31) as f32 - 15.0) / 64.0)
                    .collect();
                let a: Vec<_> = (0..if transpose { k * lda } else { m * lda })
                    .map(|i| ((i % 23) as f32 - 11.0) / 32.0)
                    .collect();
                let (rs, cs) = if transpose { (1, lda) } else { (lda, 1) };
                let ldc = n + 5;
                let packed = PackedWeights::pack(&weights, k, n, nr);
                let mut expected = vec![-123.0; m * ldc];
                crate::ops::gemm(
                    transpose,
                    true,
                    m,
                    n,
                    k,
                    &a,
                    lda,
                    &weights,
                    k,
                    0.0,
                    &mut expected,
                    ldc,
                );
                for _ in 0..2 {
                    let mut actual = vec![-123.0; m * ldc];
                    assert!(packed.multiply(&a, m, rs, cs, &mut actual, ldc));
                    assert_eq!(actual, expected);
                }
            }
        }
    }
}
