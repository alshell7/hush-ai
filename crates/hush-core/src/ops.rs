//! Small dense kernels on flat row-major `f32` buffers.

/// `C[m×n] = op(A)[m×k] · op(B)[k×n] + beta·C`, row-major, like `cblas_sgemm`.
/// `op(X)` is `X` or its transpose; `lda`/`ldb`/`ldc` are row strides of the stored matrices.
#[allow(clippy::too_many_arguments)]
pub fn gemm(
    trans_a: bool,
    trans_b: bool,
    m: usize,
    n: usize,
    k: usize,
    a: &[f32],
    lda: usize,
    b: &[f32],
    ldb: usize,
    beta: f32,
    c: &mut [f32],
    ldc: usize,
) {
    #[cfg(feature = "profile")]
    let _timer = crate::profile::scope("gemm");
    let a_len = if trans_a {
        (k - 1) * lda + m
    } else {
        (m - 1) * lda + k
    };
    let b_len = if trans_b {
        (n - 1) * ldb + k
    } else {
        (k - 1) * ldb + n
    };
    assert!(a.len() >= a_len && b.len() >= b_len && c.len() >= (m - 1) * ldc + n);
    imp::gemm(trans_a, trans_b, m, n, k, a, lda, b, ldb, beta, c, ldc);
}

#[cfg(target_os = "macos")]
mod imp {
    #[link(name = "Accelerate", kind = "framework")]
    unsafe extern "C" {
        fn cblas_sgemm(
            order: i32,
            trans_a: i32,
            trans_b: i32,
            m: i32,
            n: i32,
            k: i32,
            alpha: f32,
            a: *const f32,
            lda: i32,
            b: *const f32,
            ldb: i32,
            beta: f32,
            c: *mut f32,
            ldc: i32,
        );
    }

    const ROW_MAJOR: i32 = 101;
    const NO_TRANS: i32 = 111;
    const TRANS: i32 = 112;

    #[allow(clippy::too_many_arguments)]
    pub fn gemm(
        trans_a: bool,
        trans_b: bool,
        m: usize,
        n: usize,
        k: usize,
        a: &[f32],
        lda: usize,
        b: &[f32],
        ldb: usize,
        beta: f32,
        c: &mut [f32],
        ldc: usize,
    ) {
        let t = |x: bool| if x { TRANS } else { NO_TRANS };
        unsafe {
            cblas_sgemm(
                ROW_MAJOR,
                t(trans_a),
                t(trans_b),
                m as i32,
                n as i32,
                k as i32,
                1.0,
                a.as_ptr(),
                lda as i32,
                b.as_ptr(),
                ldb as i32,
                beta,
                c.as_mut_ptr(),
                ldc as i32,
            )
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    #[allow(clippy::too_many_arguments)]
    pub fn gemm(
        trans_a: bool,
        trans_b: bool,
        m: usize,
        n: usize,
        k: usize,
        a: &[f32],
        lda: usize,
        b: &[f32],
        ldb: usize,
        beta: f32,
        c: &mut [f32],
        ldc: usize,
    ) {
        use rayon::prelude::*;
        let threads = rayon::current_num_threads();
        // Large products dominate streaming inference. Partition independent output rows
        // across the caller's existing Rayon pool; small products stay serial. A 16-row
        // multiple respects both AVX2 and AVX-512 microkernel tiles.
        if threads > 1 && m >= 32 && m.saturating_mul(n).saturating_mul(k) >= 500_000 {
            let block = m.div_ceil(threads).div_ceil(16) * 16;
            c[..(m - 1) * ldc + n]
                .par_chunks_mut(block * ldc)
                .enumerate()
                .for_each(|(chunk, out)| {
                    let row = chunk * block;
                    let rows = block.min(m - row);
                    let start = if trans_a { row } else { row * lda };
                    gemm_serial(
                        trans_a,
                        trans_b,
                        rows,
                        n,
                        k,
                        &a[start..],
                        lda,
                        b,
                        ldb,
                        beta,
                        out,
                        ldc,
                    );
                });
        } else {
            gemm_serial(trans_a, trans_b, m, n, k, a, lda, b, ldb, beta, c, ldc);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn gemm_serial(
        trans_a: bool,
        trans_b: bool,
        m: usize,
        n: usize,
        k: usize,
        a: &[f32],
        lda: usize,
        b: &[f32],
        ldb: usize,
        beta: f32,
        c: &mut [f32],
        ldc: usize,
    ) {
        let (rsa, csa) = if trans_a { (1, lda) } else { (lda, 1) };
        let (rsb, csb) = if trans_b { (1, ldb) } else { (ldb, 1) };
        unsafe {
            matrixmultiply::sgemm(
                m,
                k,
                n,
                1.0,
                a.as_ptr(),
                rsa as isize,
                csa as isize,
                b.as_ptr(),
                rsb as isize,
                csb as isize,
                beta,
                c.as_mut_ptr(),
                ldc as isize,
                1,
            )
        }
    }
}

/// FP32 exp. ARM64 uses reference math: the exp/log approximation combination exceeded
/// the expanded NVIDIA phase gate on Apple Silicon. x86 retains its validated SIMD polynomial.
#[inline(always)]
pub fn fast_exp(x: f32) -> f32 {
    if cfg!(target_arch = "aarch64") {
        x.exp()
    } else {
        exp_approx(x)
    }
}

/// `exp` accurate to about 1 ulp for `x` in [-87, 88], written so loops vectorize.
#[inline(always)]
fn exp_approx(x: f32) -> f32 {
    let x = x.clamp(-87.0, 88.0);
    // Adding 1.5·2^23 rounds to the nearest integer and leaves it in the low mantissa bits.
    let t = x * std::f32::consts::LOG2_E + 12_582_912.0;
    let n = t - 12_582_912.0;
    let r = x - n * 0.693_359_4 + n * 2.121_944_4e-4;
    let mut p = 1.987_569_1e-4_f32;
    p = p * r + 1.398_199_9e-3;
    p = p * r + 8.333_452e-3;
    p = p * r + 4.166_579_6e-2;
    p = p * r + 1.666_666_5e-1;
    p = p * r + 0.5;
    let p = p * r * r + r + 1.0;
    p * f32::from_bits(((t.to_bits() as i32 - 0x4B40_0000 + 127) as u32) << 23)
}

/// [`fast_exp`] with fused multiply-adds, for `x <= 88`: fewer instructions, and rounding
/// differs slightly.
#[inline(always)]
pub fn fast_exp_fma(x: f32) -> f32 {
    let x = x.max(-87.0);
    let t = x.mul_add(std::f32::consts::LOG2_E, 12_582_912.0);
    let n = t - 12_582_912.0;
    let r = n.mul_add(2.121_944_4e-4, n.mul_add(-0.693_359_4, x));
    let mut p = 1.987_569_1e-4_f32;
    p = p.mul_add(r, 1.398_199_9e-3);
    p = p.mul_add(r, 8.333_452e-3);
    p = p.mul_add(r, 4.166_579_6e-2);
    p = p.mul_add(r, 1.666_666_5e-1);
    p = p.mul_add(r, 0.5);
    let p = (p * r).mul_add(r, r + 1.0);
    p * f32::from_bits(((t.to_bits() as i32 - 0x4B40_0000 + 127) as u32) << 23)
}

/// FP32 log, using reference math on ARM64 together with [`fast_exp`].
#[inline(always)]
pub fn fast_ln(x: f32) -> f32 {
    if cfg!(target_arch = "aarch64") {
        x.ln()
    } else {
        ln_approx(x)
    }
}

/// Natural log accurate to about 1 ulp for positive normal `x` (Cephes `logf`), vectorizable.
#[inline(always)]
fn ln_approx(x: f32) -> f32 {
    let bits = x.to_bits();
    let e = ((bits >> 23) & 0xff) as i32 - 126;
    let m = f32::from_bits((bits & 0x007f_ffff) | 0x3f00_0000);
    let small = m < std::f32::consts::FRAC_1_SQRT_2;
    let e = (e - small as i32) as f32;
    let m = if small { m + m - 1.0 } else { m - 1.0 };
    let z = m * m;
    let mut y = 7.037_683_6e-2_f32;
    y = y * m - 1.151_461e-1;
    y = y * m + 1.167_699_9e-1;
    y = y * m - 1.242_014_1e-1;
    y = y * m + 1.424_932_3e-1;
    y = y * m - 1.666_805_8e-1;
    y = y * m + 2.000_071_5e-1;
    y = y * m - 2.499_999_4e-1;
    y = y * m + 3.333_333e-1;
    let y = y * m * z - 2.121_944_4e-4 * e - 0.5 * z;
    m + y + 0.693_359_4 * e
}

/// [`fast_ln`] with fused multiply-adds.
#[inline(always)]
pub fn fast_ln_fma(x: f32) -> f32 {
    let bits = x.to_bits();
    let e = ((bits >> 23) & 0xff) as i32 - 126;
    let m = f32::from_bits((bits & 0x007f_ffff) | 0x3f00_0000);
    let small = m < std::f32::consts::FRAC_1_SQRT_2;
    let e = (e - small as i32) as f32;
    let m = if small { m + m - 1.0 } else { m - 1.0 };
    let z = m * m;
    let mut y = 7.037_683_6e-2_f32;
    y = y.mul_add(m, -1.151_461e-1);
    y = y.mul_add(m, 1.167_699_9e-1);
    y = y.mul_add(m, -1.242_014_1e-1);
    y = y.mul_add(m, 1.424_932_3e-1);
    y = y.mul_add(m, -1.666_805_8e-1);
    y = y.mul_add(m, 2.000_071_5e-1);
    y = y.mul_add(m, -2.499_999_4e-1);
    y = y.mul_add(m, 3.333_333e-1);
    let y = e.mul_add(-2.121_944_4e-4, (y * m).mul_add(z, -0.5 * z));
    e.mul_add(0.693_359_4, m + y)
}

/// [`softplus`] with fused multiply-adds.
#[inline(always)]
pub fn softplus_fma(x: f32) -> f32 {
    let y = fast_exp_fma(-x.abs());
    let u = 1.0 + y;
    let log1p = if u == 1.0 {
        y
    } else {
        fast_ln_fma(u) * y / (u - 1.0)
    };
    if x > 20.0 {
        x
    } else {
        x.max(0.0) + log1p
    }
}

#[inline(always)]
pub fn silu(x: f32) -> f32 {
    x / (1.0 + fast_exp(-x))
}

/// `torch.nn.functional.softplus` (threshold 20).
#[inline(always)]
pub fn softplus(x: f32) -> f32 {
    let y = fast_exp(-x.abs());
    let u = 1.0 + y;
    let log1p = if u == 1.0 {
        y
    } else {
        fast_ln(u) * y / (u - 1.0)
    };
    let soft = x.max(0.0) + log1p;
    if x > 20.0 {
        x
    } else {
        soft
    }
}

/// Elementwise activations with runtime SIMD selection; arithmetic stays unfused.
pub fn silu_in_place(x: &mut [f32]) {
    #[cfg(target_arch = "x86_64")]
    if std::is_x86_feature_detected!("avx512f") {
        // SAFETY: runtime detection guards the target feature; all accesses use slices.
        unsafe { silu_avx512(x) };
        return;
    }
    #[cfg(target_arch = "x86_64")]
    if std::is_x86_feature_detected!("avx2") {
        // SAFETY: runtime detection guards the target feature; all accesses use slices.
        unsafe { silu_avx2(x) };
        return;
    }
    x.iter_mut().for_each(|v| *v = silu(*v));
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn silu_avx2(x: &mut [f32]) {
    x.iter_mut().for_each(|v| *v = silu(*v));
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
unsafe fn silu_avx512(x: &mut [f32]) {
    x.iter_mut().for_each(|v| *v = silu(*v));
}

/// Mamba output gate, with the same unfused FP32 operations as the scalar baseline.
pub(crate) fn mamba_gate(y: &mut [f32], x: &[f32], z: &[f32], d: &[f32]) {
    assert_eq!(y.len(), x.len());
    assert_eq!(y.len(), z.len());
    assert_eq!(y.len(), d.len());
    #[cfg(target_arch = "x86_64")]
    if std::is_x86_feature_detected!("avx512f") {
        // SAFETY: runtime detection guards the target feature; accesses are checked slices.
        unsafe { gate_avx512(y, x, z, d) };
        return;
    }
    #[cfg(target_arch = "x86_64")]
    if std::is_x86_feature_detected!("avx2") {
        // SAFETY: runtime detection guards the target feature; accesses are checked slices.
        unsafe { gate_avx2(y, x, z, d) };
        return;
    }
    gate_scalar(y, x, z, d);
}

#[inline(always)]
fn gate_scalar(y: &mut [f32], x: &[f32], z: &[f32], d: &[f32]) {
    for (((y, x), z), d) in y.iter_mut().zip(x).zip(z).zip(d) {
        *y = (*y + d * x) * silu(*z);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn gate_avx2(y: &mut [f32], x: &[f32], z: &[f32], d: &[f32]) {
    gate_scalar(y, x, z, d);
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
unsafe fn gate_avx512(y: &mut [f32], x: &[f32], z: &[f32], d: &[f32]) {
    gate_scalar(y, x, z, d);
}

pub fn softplus_in_place<const FAST: bool>(x: &mut [f32]) {
    #[cfg(target_arch = "x86_64")]
    if !FAST && std::is_x86_feature_detected!("avx512f") {
        // SAFETY: runtime detection guards the target feature; all accesses use slices.
        unsafe { softplus_avx512(x) };
        return;
    }
    #[cfg(target_arch = "x86_64")]
    if !FAST && std::is_x86_feature_detected!("avx2") {
        // SAFETY: runtime detection guards the target feature; all accesses use slices.
        unsafe { softplus_avx2(x) };
        return;
    }
    for v in x {
        *v = if FAST { softplus_fma(*v) } else { softplus(*v) };
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn softplus_avx2(x: &mut [f32]) {
    x.iter_mut().for_each(|v| *v = softplus(*v));
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
unsafe fn softplus_avx512(x: &mut [f32]) {
    x.iter_mut().for_each(|v| *v = softplus(*v));
}

/// LayerNorm over the last dimension of `rows × dim` data (eps 1e-5).
pub fn layer_norm_rows(x: &mut [f32], dim: usize, weight: &[f32], bias: &[f32]) {
    for row in x.chunks_exact_mut(dim) {
        let mean = row.iter().sum::<f32>() / dim as f32;
        let var = row.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / dim as f32;
        let inv = 1.0 / (var + 1e-5).sqrt();
        for ((v, w), b) in row.iter_mut().zip(weight).zip(bias) {
            *v = (*v - mean) * inv * w + b;
        }
    }
}

/// Sequential normalization calls may share this storage. No values carry across calls.
#[derive(Default)]
pub(crate) struct ChannelNormScratch {
    mean: Vec<f32>,
    var: Vec<f32>,
}

/// LayerNorm over channel-major data, then per-channel PReLU, reusing reduction buffers.
pub(crate) fn layer_norm_prelu_channels(
    x: &mut [f32],
    channels: usize,
    weight: &[f32],
    bias: &[f32],
    prelu: &[f32],
    scratch: &mut ChannelNormScratch,
) {
    #[cfg(feature = "profile")]
    let _timer = crate::profile::scope("channel_norm");
    #[cfg(target_arch = "x86_64")]
    if std::is_x86_feature_detected!("avx512f") {
        // SAFETY: runtime detection guards the target feature; all accesses use slices.
        unsafe { layer_norm_prelu_avx512(x, channels, weight, bias, prelu, scratch) };
        return;
    }
    #[cfg(target_arch = "x86_64")]
    if std::is_x86_feature_detected!("avx2") {
        // SAFETY: runtime detection guards the target feature; all accesses use slices.
        unsafe { layer_norm_prelu_avx2(x, channels, weight, bias, prelu, scratch) };
        return;
    }
    layer_norm_prelu_impl(x, channels, weight, bias, prelu, scratch);
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn layer_norm_prelu_avx2(
    x: &mut [f32],
    channels: usize,
    weight: &[f32],
    bias: &[f32],
    prelu: &[f32],
    scratch: &mut ChannelNormScratch,
) {
    layer_norm_prelu_impl(x, channels, weight, bias, prelu, scratch);
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
unsafe fn layer_norm_prelu_avx512(
    x: &mut [f32],
    channels: usize,
    weight: &[f32],
    bias: &[f32],
    prelu: &[f32],
    scratch: &mut ChannelNormScratch,
) {
    layer_norm_prelu_impl(x, channels, weight, bias, prelu, scratch);
}

#[inline(always)]
fn layer_norm_prelu_impl(
    x: &mut [f32],
    channels: usize,
    weight: &[f32],
    bias: &[f32],
    prelu: &[f32],
    scratch: &mut ChannelNormScratch,
) {
    let positions = x.len() / channels;
    let ChannelNormScratch { mean, var } = scratch;
    mean.resize(positions, 0.0);
    var.resize(positions, 0.0);
    mean.fill(0.0);
    var.fill(0.0);
    for row in x.chunks_exact(positions) {
        for (m, v) in mean.iter_mut().zip(row) {
            *m += v;
        }
    }
    mean.iter_mut().for_each(|m| *m /= channels as f32);
    for row in x.chunks_exact(positions) {
        for ((s, v), m) in var.iter_mut().zip(row).zip(mean.iter()) {
            *s += (v - m) * (v - m);
        }
    }
    var.iter_mut()
        .for_each(|s| *s = 1.0 / (*s / channels as f32 + 1e-5).sqrt());
    for (c, row) in x.chunks_exact_mut(positions).enumerate() {
        let (w, b, a) = (weight[c], bias[c], prelu[c]);
        for ((v, m), inv) in row.iter_mut().zip(mean.iter()).zip(var.iter()) {
            let y = (*v - m) * inv * w + b;
            *v = if y > 0.0 { y } else { a * y };
        }
    }
}

/// Adds `bias[c]` to every position of channel `c` in channel-major data.
pub fn add_channel_bias(x: &mut [f32], bias: &[f32]) {
    let positions = x.len() / bias.len();
    for (row, b) in x.chunks_exact_mut(positions).zip(bias) {
        row.iter_mut().for_each(|v| *v += b);
    }
}

/// Transposes a `rows × cols` row-major matrix.
pub fn transpose(x: &[f32], rows: usize, cols: usize) -> Vec<f32> {
    let mut out = vec![0f32; x.len()];
    for r in 0..rows {
        for c in 0..cols {
            out[c * rows + r] = x[r * cols + c];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatched_mamba_gate_preserves_scalar_results_with_tail() {
        let z: Vec<_> = (0..2051).map(|i| (i as f32 - 1024.0) / 10.0).collect();
        let x: Vec<_> = z.iter().map(|v| v.sin()).collect();
        let d: Vec<_> = z.iter().map(|v| v.cos()).collect();
        let mut expected = x.clone();
        gate_scalar(&mut expected, &x, &z, &d);
        let mut actual = x;
        let input = actual.clone();
        mamba_gate(&mut actual, &input, &z, &d);
        assert_eq!(actual, expected);
        #[cfg(target_arch = "x86_64")]
        if std::is_x86_feature_detected!("avx2") {
            actual.copy_from_slice(&input);
            // SAFETY: guarded by runtime detection.
            unsafe { gate_avx2(&mut actual, &input, &z, &d) };
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn dispatched_activations_preserve_scalar_results_with_tail() {
        let input: Vec<_> = (0..2051).map(|i| (i as f32 - 1024.0) / 10.0).collect();
        let mut actual = input.clone();
        silu_in_place(&mut actual);
        assert_eq!(actual, input.iter().copied().map(silu).collect::<Vec<_>>());
        actual.copy_from_slice(&input);
        softplus_in_place::<false>(&mut actual);
        assert_eq!(
            actual,
            input.iter().copied().map(softplus).collect::<Vec<_>>()
        );
    }

    #[test]
    fn dispatched_channel_norm_preserves_scalar_results_with_tail() {
        let channels = 17;
        let mut actual: Vec<_> = (0..channels * 513)
            .map(|i| (i as f32 * 0.13).sin())
            .collect();
        let weight: Vec<_> = (0..channels).map(|i| i as f32 / 13.0 - 0.5).collect();
        let bias = vec![0.125; channels];
        let prelu = vec![0.25; channels];
        let mut expected = actual.clone();
        layer_norm_prelu_impl(
            &mut expected,
            channels,
            &weight,
            &bias,
            &prelu,
            &mut ChannelNormScratch::default(),
        );
        layer_norm_prelu_channels(
            &mut actual,
            channels,
            &weight,
            &bias,
            &prelu,
            &mut ChannelNormScratch::default(),
        );
        assert_eq!(actual, expected);
    }

    #[test]
    fn channel_norm_scratch_overwrites_poison_across_shapes_and_dispatch() {
        let mut scratch = ChannelNormScratch::default();
        for (channels, positions) in [(64, 963), (17, 7), (1, 1), (32, 321), (3, 17)] {
            let input: Vec<_> = (0..channels * positions)
                .map(|i| ((i * 19 % 97) as f32 - 48.0) / 31.0)
                .collect();
            let weight: Vec<_> = (0..channels).map(|i| i as f32 / 13.0 - 0.5).collect();
            let bias = vec![0.125; channels];
            let prelu = vec![0.25; channels];
            // Independent position-at-a-time reference keeps the original FP32 reduction
            // and affine order while avoiding the production scratch implementation.
            let mut expected = input.clone();
            for p in 0..positions {
                let mean = (0..channels)
                    .map(|ch| input[ch * positions + p])
                    .sum::<f32>()
                    / channels as f32;
                let var = (0..channels)
                    .map(|ch| {
                        let delta = input[ch * positions + p] - mean;
                        delta * delta
                    })
                    .sum::<f32>()
                    / channels as f32;
                let inv = 1.0 / (var + 1e-5).sqrt();
                for ch in 0..channels {
                    let y = (input[ch * positions + p] - mean) * inv * weight[ch] + bias[ch];
                    expected[ch * positions + p] = if y > 0.0 { y } else { prelu[ch] * y };
                }
            }
            for mode in 0..4 {
                scratch.mean.fill(f32::NAN);
                scratch.var.fill(f32::NEG_INFINITY);
                let mut actual = input.clone();
                match mode {
                    0 => layer_norm_prelu_channels(
                        &mut actual,
                        channels,
                        &weight,
                        &bias,
                        &prelu,
                        &mut scratch,
                    ),
                    1 => layer_norm_prelu_impl(
                        &mut actual,
                        channels,
                        &weight,
                        &bias,
                        &prelu,
                        &mut scratch,
                    ),
                    #[cfg(target_arch = "x86_64")]
                    2 if std::is_x86_feature_detected!("avx2") => {
                        // SAFETY: guarded by runtime detection; dimensions agree.
                        unsafe {
                            layer_norm_prelu_avx2(
                                &mut actual,
                                channels,
                                &weight,
                                &bias,
                                &prelu,
                                &mut scratch,
                            )
                        };
                    }
                    #[cfg(target_arch = "x86_64")]
                    3 if std::is_x86_feature_detected!("avx512f") => {
                        // SAFETY: guarded by runtime detection; dimensions agree.
                        unsafe {
                            layer_norm_prelu_avx512(
                                &mut actual,
                                channels,
                                &weight,
                                &bias,
                                &prelu,
                                &mut scratch,
                            )
                        };
                    }
                    _ => continue,
                }
                assert_eq!(
                    actual.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    "channels={channels}, positions={positions}, mode={mode}"
                );
            }
        }
    }

    #[test]
    fn fast_exp_matches_std() {
        let mut worst = 0f32;
        let mut x = -80.0f32;
        while x < 80.0 {
            worst = worst.max((fast_exp(x) / x.exp() - 1.0).abs());
            x += 0.0137;
        }
        assert!(worst < 1e-6, "relative error {worst}");
    }

    #[test]
    fn fast_ln_and_softplus_match_std() {
        let mut worst = 0f32;
        let mut x = 1e-6f32;
        while x < 1e6 {
            worst = worst.max((fast_ln(x) - x.ln()).abs() / x.ln().abs().max(1.0));
            x *= 1.0137;
        }
        assert!(worst < 2e-7, "ln error {worst}");
        let mut worst = 0f32;
        let mut x = -30.0f32;
        while x < 30.0 {
            let exact = if x > 20.0 {
                x
            } else {
                (x as f64).exp().ln_1p() as f32
            };
            worst = worst.max((softplus(x) - exact).abs() / exact.max(1e-3));
            x += 0.0071;
        }
        assert!(worst < 1e-6, "softplus error {worst}");
    }

    #[test]
    fn fma_variants_match_std() {
        let mut worst = 0f32;
        let mut x = -80.0f32;
        while x < 0.0 {
            worst = worst.max((fast_exp_fma(x) / x.exp() - 1.0).abs());
            x += 0.0137;
        }
        assert!(worst < 1e-6, "exp error {worst}");
        let mut worst = 0f32;
        let mut x = 1e-6f32;
        while x < 1e6 {
            worst = worst.max((fast_ln_fma(x) - x.ln()).abs() / x.ln().abs().max(1.0));
            x *= 1.0137;
        }
        assert!(worst < 2e-7, "ln error {worst}");
        let mut worst = 0f32;
        let mut x = -30.0f32;
        while x < 30.0 {
            worst = worst.max((softplus_fma(x) - softplus(x)).abs() / softplus(x).max(1e-3));
            x += 0.0071;
        }
        assert!(worst < 1e-6, "softplus error {worst}");
    }

    #[test]
    fn gemm_transposes() {
        // A = [[1,2,3],[4,5,6]] (2×3), B = Aᵀ, A·Aᵀ = [[14,32],[32,77]]
        let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let mut c = [0f32; 4];
        gemm(false, true, 2, 2, 3, &a, 3, &a, 3, 0.0, &mut c, 2);
        assert_eq!(c, [14.0, 32.0, 32.0, 77.0]);
        gemm(true, false, 3, 3, 2, &a, 3, &a, 3, 0.0, &mut [0f32; 9], 3);
    }

    #[test]
    fn large_gemm_preserves_transposes_padding_and_beta() {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap();
        pool.install(|| {
            let (m, n, k) = (47, 37, 300);
            for ta in [false, true] {
                for tb in [false, true] {
                    let lda = if ta { m + 3 } else { k + 3 };
                    let ldb = if tb { k + 5 } else { n + 5 };
                    let ldc = n + 7;
                    let a: Vec<_> = (0..if ta { k * lda } else { m * lda })
                        .map(|i| ((i * 17 % 29) as f32 - 14.0) / 16.0)
                        .collect();
                    let b: Vec<_> = (0..if tb { n * ldb } else { k * ldb })
                        .map(|i| ((i * 13 % 31) as f32 - 15.0) / 16.0)
                        .collect();
                    for beta in [0.0, 0.5] {
                        // The final row need only contain n values, not a whole ldc stride.
                        let mut actual = vec![2.0; (m - 1) * ldc + n];
                        let mut expected = actual.clone();
                        for r in 0..m {
                            for c in 0..n {
                                let sum: f32 = (0..k)
                                    .map(|j| {
                                        a[if ta { j * lda + r } else { r * lda + j }]
                                            * b[if tb { c * ldb + j } else { j * ldb + c }]
                                    })
                                    .sum();
                                expected[r * ldc + c] = sum + beta * 2.0;
                            }
                        }
                        gemm(ta, tb, m, n, k, &a, lda, &b, ldb, beta, &mut actual, ldc);
                        // Binary-fraction inputs make this oracle exact, including padding.
                        assert_eq!(actual, expected, "ta={ta}, tb={tb}, beta={beta}");
                    }
                }
            }
        });
    }
}
