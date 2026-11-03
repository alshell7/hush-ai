//! RE-USE building blocks. Conv activations are channel-major `[channels][freq]`;
//! Mamba activations are position-major `[positions][channels]`.

use crate::ops::{
    add_channel_bias, fast_exp, fast_exp_fma, gemm, layer_norm_prelu_channels, layer_norm_rows,
    mamba_gate, silu, silu_in_place, softplus_in_place, ChannelNormScratch,
};
use crate::packed::PackedWeights;
use crate::weights::Weights;
use eyre::Result;
use rayon::prelude::*;
use std::collections::VecDeque;

/// Conv2d over (time, freq) for one new time frame. Time is causal: the last `(taps - 1) * dilation`
/// input frames are cached. Frequency uses `freq_pad` zeros on both sides and `stride`.
pub struct StreamConv {
    weight: Vec<f32>,
    packed: Option<PackedWeights>,
    /// (input channel, history index, frequency offset) for each reduction row.
    packing_map: Vec<(usize, usize, isize)>,
    tile_scratch: Vec<f32>,
    bias: Vec<f32>,
    cin: usize,
    pub cout: usize,
    taps: usize,
    dilation: usize,
    kf: usize,
    freq_pad: usize,
    stride: usize,
    cache: VecDeque<Vec<f32>>,
    /// Empty after initialization; retains history allocations across stream resets.
    spare_frames: Vec<Vec<f32>>,
    /// Reused im2col storage. Its shape is determined by the current input frame.
    cols: Vec<f32>,
    /// Reused position-major output for wide, parallel convolution products.
    position_out: Vec<f32>,
}

impl StreamConv {
    /// Replay a complete, frequency-padded NCHW convolution input with fresh history.
    /// Development only; the streaming caches and ordinary arithmetic are unchanged.
    #[cfg(feature = "trace")]
    pub(crate) fn replay_padded_diagnostic(
        &self,
        input: &[f32],
        shape: &[usize],
    ) -> Result<Vec<f32>> {
        let history = (self.taps - 1) * self.dilation;
        eyre::ensure!(
            shape.len() == 4
                && shape[0] == 1
                && shape[1] == self.cin
                && shape[2] == history + 1
                && shape[3] > 2 * self.freq_pad
                && shape[3] >= self.kf,
            "invalid padded convolution input shape"
        );
        let padded = shape[3];
        let freq = padded - 2 * self.freq_pad;
        let frame = |time: usize| -> Vec<f32> {
            let mut frame = Vec::with_capacity(self.cin * freq);
            for channel in 0..self.cin {
                let start = (channel * shape[2] + time) * padded + self.freq_pad;
                frame.extend_from_slice(&input[start..start + freq]);
            }
            frame
        };
        // Reject padding that would silently disappear when rebuilding native history.
        for row in input.chunks_exact(padded) {
            eyre::ensure!(
                row[..self.freq_pad]
                    .iter()
                    .chain(&row[padded - self.freq_pad..])
                    .all(|v| v.to_bits() == 0),
                "expected positive-zero frequency padding"
            );
        }
        let mut replay = Self {
            weight: self.weight.clone(),
            packed: PackedWeights::new(&self.weight, self.cin * self.taps * self.kf, self.cout),
            packing_map: self.packing_map.clone(),
            tile_scratch: Vec::new(),
            bias: self.bias.clone(),
            cin: self.cin,
            cout: self.cout,
            taps: self.taps,
            dilation: self.dilation,
            kf: self.kf,
            freq_pad: self.freq_pad,
            stride: self.stride,
            cache: (0..history).map(frame).collect(),
            spare_frames: Vec::new(),
            cols: Vec::new(),
            position_out: Vec::new(),
        };
        Ok(replay.step(&frame(history)))
    }

    /// `prefix.weight` is `[cout][cin][taps][kf]`.
    pub fn load(
        w: &Weights,
        prefix: &str,
        dilation: usize,
        freq_pad: usize,
        stride: usize,
    ) -> Result<Self> {
        let shape = w.shape(&format!("{prefix}.weight"))?.to_vec();
        let weight = w.get(&format!("{prefix}.weight"))?;
        let packed = PackedWeights::new(&weight, shape[1] * shape[2] * shape[3], shape[0]);
        Ok(Self {
            weight,
            packed,
            packing_map: (0..shape[1] * shape[2] * shape[3])
                .map(|p| {
                    (
                        p / (shape[2] * shape[3]),
                        (p / shape[3] % shape[2]) * dilation,
                        (p % shape[3]) as isize - freq_pad as isize,
                    )
                })
                .collect(),
            tile_scratch: Vec::new(),
            bias: w.get(&format!("{prefix}.bias"))?,
            cout: shape[0],
            cin: shape[1],
            taps: shape[2],
            kf: shape[3],
            dilation,
            freq_pad,
            stride,
            cache: VecDeque::new(),
            spare_frames: Vec::new(),
            cols: Vec::new(),
            position_out: Vec::new(),
        })
    }

    /// Same as `load` for a kernel stored as `[cout][cin][1][taps]` (time on the last axis).
    pub fn load_time_last(w: &Weights, prefix: &str) -> Result<Self> {
        let mut conv = Self::load(w, prefix, 1, 0, 1)?;
        conv.taps = conv.kf;
        conv.kf = 1;
        conv.packing_map = (0..conv.cin * conv.taps)
            .map(|p| (p / conv.taps, p % conv.taps, 0))
            .collect();
        Ok(conv)
    }

    pub fn reset(&mut self) {
        self.spare_frames.extend(self.cache.drain(..));
    }

    fn zero_frame(&mut self, len: usize) -> Vec<f32> {
        let mut frame = self.spare_frames.pop().unwrap_or_default();
        frame.resize(len, 0.0);
        frame.fill(0.0);
        frame
    }

    /// Adds `x` to the time history without computing an output. On an empty history, `zeros`
    /// zero frames come first.
    pub fn push(&mut self, x: &[f32], zeros: usize) {
        if self.cache.is_empty() {
            for _ in 0..zeros {
                let frame = self.zero_frame(x.len());
                self.cache.push_back(frame);
            }
        }
        let mut frame = self.spare_frames.pop().unwrap_or_default();
        frame.clear();
        frame.extend_from_slice(x);
        self.cache.push_back(frame);
    }

    /// `x` is `[cin][freq]`; returns `[cout][freq_out]`.
    pub fn step(&mut self, x: &[f32]) -> Vec<f32> {
        #[cfg(feature = "profile")]
        let _timer = crate::profile::scope("stream_conv");
        let freq = x.len() / self.cin;
        let history = (self.taps - 1) * self.dilation;
        while self.cache.len() < history {
            let frame = self.zero_frame(x.len());
            self.cache.push_back(frame);
        }
        let freq_out = (freq + 2 * self.freq_pad - self.kf) / self.stride + 1;
        let rows = self.cin * self.taps * self.kf;
        let wide = !cfg!(target_os = "macos")
            && freq_out >= 256
            && freq_out >= 2 * self.cout
            && rayon::current_num_threads() > 1;
        let direct = if wide {
            self.position_out.resize(freq_out * self.cout, 0.0);
            let cache = &self.cache;
            let map = &self.packing_map;
            self.packed.as_ref().is_some_and(|p| {
                p.multiply_conv(
                    freq_out,
                    self.stride,
                    &mut self.tile_scratch,
                    &mut self.position_out,
                    |p| {
                        let (channel, at, offset) = map[p];
                        let frame: &[f32] = if at < history { &cache[at] } else { x };
                        (&frame[channel * freq..][..freq], offset)
                    },
                )
            })
        } else {
            false
        };
        if !direct {
            self.cols.resize(rows * freq_out, 0.0);
            #[cfg(feature = "profile")]
            let packing_timer = crate::profile::scope("conv_pack");
            let (taps, kf, stride, pad) = (self.taps, self.kf, self.stride, self.freq_pad);
            let cache = &self.cache;
            let dilation = self.dilation;
            let pack = |(ci, cols): (usize, &mut [f32])| {
                for tap in 0..taps {
                    let at = tap * dilation;
                    let frame: &[f32] = if at < history { &cache[at] } else { x };
                    let src = &frame[ci * freq..(ci + 1) * freq];
                    for k in 0..kf {
                        let row = &mut cols[(tap * kf + k) * freq_out..][..freq_out];
                        let first = pad.saturating_sub(k).div_ceil(stride).min(freq_out);
                        let end = (freq + pad)
                            .saturating_sub(k)
                            .div_ceil(stride)
                            .min(freq_out);
                        row[..first].fill(0.0);
                        row[end..].fill(0.0);
                        if first < end {
                            let start = first * stride + k - pad;
                            if stride == 1 {
                                row[first..end].copy_from_slice(&src[start..start + end - first]);
                            } else {
                                for (out, value) in row[first..end]
                                    .iter_mut()
                                    .zip(src[start..].iter().step_by(stride))
                                {
                                    *out = *value;
                                }
                            }
                        }
                    }
                }
            };
            let channel_size = taps * kf * freq_out;
            if self.cols.len() >= 32768 {
                self.cols
                    .par_chunks_mut(channel_size)
                    .enumerate()
                    .for_each(pack);
            } else {
                self.cols
                    .chunks_mut(channel_size)
                    .enumerate()
                    .for_each(pack);
            }
            #[cfg(feature = "profile")]
            drop(packing_timer);
        }
        let mut out = vec![0f32; self.cout * freq_out];
        // On non-Accelerate targets, put frequency on GEMM's row axis so wide
        // convolutions can use the whole worker pool without repacking a huge
        // activation matrix separately for each small group of output channels.
        if wide {
            self.position_out.resize(freq_out * self.cout, 0.0);
            if !direct
                && !self.packed.as_ref().is_some_and(|p| {
                    p.multiply(
                        &self.cols,
                        freq_out,
                        1,
                        freq_out,
                        &mut self.position_out,
                        self.cout,
                    )
                })
            {
                gemm(
                    true,
                    true,
                    freq_out,
                    self.cout,
                    rows,
                    &self.cols,
                    freq_out,
                    &self.weight,
                    rows,
                    0.0,
                    &mut self.position_out,
                    self.cout,
                );
            }
            out.par_chunks_mut(freq_out)
                .enumerate()
                .for_each(|(channel, dst)| {
                    for (f, value) in dst.iter_mut().enumerate() {
                        *value = self.position_out[f * self.cout + channel];
                    }
                });
        } else {
            gemm(
                false,
                false,
                self.cout,
                freq_out,
                rows,
                &self.weight,
                rows,
                &self.cols,
                freq_out,
                0.0,
                &mut out,
                freq_out,
            );
        }
        add_channel_bias(&mut out, &self.bias);
        if history > 0 {
            // Reuse the oldest frame allocation instead of allocating on every step.
            let mut oldest = self.cache.pop_front().expect("history is initialized");
            oldest.copy_from_slice(x);
            self.cache.push_back(oldest);
        }
        out
    }
}

/// ChannelLayerNorm followed by PReLU.
pub struct NormAct {
    weight: Vec<f32>,
    bias: Vec<f32>,
    prelu: Vec<f32>,
}

impl NormAct {
    /// Isolated normalization/activation arithmetic on channel-major tensors.
    #[cfg(feature = "trace")]
    pub(crate) fn replay_diagnostic(&self, input: &[f32], mode: &str) -> Result<Vec<f32>> {
        eyre::ensure!(
            !input.is_empty() && input.len().is_multiple_of(self.weight.len()),
            "invalid activation shape"
        );
        let mut output = input.to_vec();
        let positions = input.len() / self.weight.len();
        if matches!(mode, "mean_cuda" | "rstd_cuda") {
            eyre::ensure!(
                self.weight.len() == 64,
                "CUDA norm diagnostic requires 64 channels"
            );
            return Ok((0..positions)
                .map(|position| {
                    let row = std::array::from_fn::<_, 64, _>(|c| input[c * positions + position]);
                    let (mean, rstd) = crate::trace::exp_norm::layer_norm_stats_64(&row);
                    if mode == "mean_cuda" {
                        mean
                    } else {
                        rstd
                    }
                })
                .collect());
        }
        match mode {
            "act" => self.apply_buffered(&mut output, &mut ChannelNormScratch::default()),
            "prelu" => {
                for (channel, row) in output.chunks_exact_mut(positions).enumerate() {
                    for value in row {
                        if *value <= 0. {
                            *value *= self.prelu[channel];
                        }
                    }
                }
            }
            "act_cuda" | "norm_cuda" => {
                eyre::ensure!(
                    self.weight.len() == 64,
                    "CUDA norm diagnostic requires 64 channels"
                );
                for position in 0..positions {
                    let row = std::array::from_fn::<_, 64, _>(|c| input[c * positions + position]);
                    let (mean, rstd) = crate::trace::exp_norm::layer_norm_stats_64(&row);
                    for c in 0..64 {
                        let y = (rstd * (row[c] - mean)).mul_add(self.weight[c], self.bias[c]);
                        output[c * positions + position] = if mode == "act_cuda" && y <= 0. {
                            y * self.prelu[c]
                        } else {
                            y
                        };
                    }
                }
            }
            _ => {
                eyre::bail!("unknown activation diagnostic {mode}");
            }
        }
        Ok(output)
    }

    pub fn load(w: &Weights, norm: &str, prelu: &str) -> Result<Self> {
        Ok(Self {
            weight: w.get(&format!("{norm}.norm.weight"))?,
            bias: w.get(&format!("{norm}.norm.bias"))?,
            prelu: w.get(&format!("{prelu}.weight"))?,
        })
    }

    pub(crate) fn apply_buffered(&self, x: &mut [f32], scratch: &mut ChannelNormScratch) {
        layer_norm_prelu_channels(
            x,
            self.weight.len(),
            &self.weight,
            &self.bias,
            &self.prelu,
            scratch,
        );
    }
}

/// Causal DenseBlock: 4 dilated 3×3 convs, each seeing all earlier outputs and the input.
pub struct DenseBlock {
    layers: Vec<(StreamConv, NormAct)>,
    channels: usize,
    buf: Vec<f32>,
    norm_scratch: ChannelNormScratch,
}

impl DenseBlock {
    #[cfg(feature = "trace")]
    pub(crate) fn diagnostic_layer(&self, layer: usize) -> Option<(&StreamConv, &NormAct)> {
        self.layers.get(layer).map(|(conv, act)| (conv, act))
    }

    pub fn load(w: &Weights, prefix: &str) -> Result<Self> {
        let mut layers = Vec::new();
        for i in 0..4 {
            let p = format!("{prefix}.dense_block.{i}");
            layers.push((
                StreamConv::load(w, &format!("{p}.1"), 1 << i, 1, 1)?,
                NormAct::load(w, &format!("{p}.2"), &format!("{p}.3"))?,
            ));
        }
        let channels = w.shape(&format!("{prefix}.dense_block.0.1.weight"))?[0];
        Ok(Self {
            layers,
            channels,
            buf: Vec::new(),
            norm_scratch: ChannelNormScratch::default(),
        })
    }

    pub fn reset(&mut self) {
        self.layers.iter_mut().for_each(|(c, _)| c.reset());
    }

    /// `x` is `[channels][freq]`. Layer `i` reads `[y_{i-1}, …, y_0, x]`, which is kept as one
    /// contiguous tail of `buf`.
    pub fn step(&mut self, x: &[f32]) -> Vec<f32> {
        let size = x.len();
        let depth = self.layers.len();
        self.buf.resize((depth + 1) * size, 0.0);
        self.buf[depth * size..].copy_from_slice(x);
        let mut y = Vec::new();
        for (i, (conv, act)) in self.layers.iter_mut().enumerate() {
            y = conv.step(&self.buf[(depth - i) * size..]);
            act.apply_buffered(&mut y, &mut self.norm_scratch);
            if i + 1 < depth {
                self.buf[(depth - 1 - i) * size..(depth - i) * size].copy_from_slice(&y);
            }
        }
        debug_assert_eq!(y.len(), self.channels * (size / self.channels));
        y
    }
}

/// Linear layer `y = x·Wᵀ + b` on position-major rows.
pub struct Linear {
    weight: Vec<f32>,
    packed: Option<PackedWeights>,
    bias: Option<Vec<f32>>,
    pub out: usize,
    pub inp: usize,
}

impl Linear {
    pub fn load(w: &Weights, prefix: &str, bias: bool) -> Result<Self> {
        let shape = w.shape(&format!("{prefix}.weight"))?.to_vec();
        let weight = w.get(&format!("{prefix}.weight"))?;
        let packed = PackedWeights::new(&weight, shape[1], shape[0]);
        Ok(Self {
            weight,
            packed,
            bias: if bias {
                Some(w.get(&format!("{prefix}.bias"))?)
            } else {
                None
            },
            out: shape[0],
            inp: shape[1],
        })
    }

    /// `x` holds `rows` rows of `inp` values, each starting `ld` apart.
    pub fn forward_strided(&self, x: &[f32], rows: usize, ld: usize) -> Vec<f32> {
        let mut y = Vec::new();
        self.forward_strided_into(x, rows, ld, &mut y);
        y
    }

    fn forward_strided_into(&self, x: &[f32], rows: usize, ld: usize, y: &mut Vec<f32>) {
        y.resize(rows * self.out, 0.0);
        if !self
            .packed
            .as_ref()
            .is_some_and(|p| p.multiply(x, rows, ld, 1, y, self.out))
        {
            gemm(
                false,
                true,
                rows,
                self.out,
                self.inp,
                x,
                ld,
                &self.weight,
                self.inp,
                0.0,
                y,
                self.out,
            );
        }
        if let Some(b) = &self.bias {
            for row in y.chunks_exact_mut(self.out) {
                row.iter_mut().zip(b).for_each(|(v, b)| *v += b);
            }
        }
    }

    pub fn forward(&self, x: &[f32], rows: usize) -> Vec<f32> {
        self.forward_strided(x, rows, self.inp)
    }

    /// Diagnostic copy of the native FP32 product, before bias is added.
    /// Keeps the ordinary forward implementation and its dispatch unchanged.
    #[cfg(feature = "trace")]
    fn product_diagnostic(&self, x: &[f32], rows: usize, ld: usize) -> Vec<f32> {
        let mut y = vec![0.; rows * self.out];
        if !self
            .packed
            .as_ref()
            .is_some_and(|p| p.multiply(x, rows, ld, 1, &mut y, self.out))
        {
            gemm(
                false,
                true,
                rows,
                self.out,
                self.inp,
                x,
                ld,
                &self.weight,
                self.inp,
                0.,
                &mut y,
                self.out,
            );
        }
        y
    }

    /// Reproduce the measured sm_75/cuBLAS sliced1x4 order, diagnostic only.
    /// Each warp accumulates eight consecutive K values per 32-value tile;
    /// the epilogue combines slices as (p0+p1)+(p2+p3), then adds bias.
    #[cfg(feature = "trace")]
    fn forward_sliced_diagnostic(&self, x: &[f32], rows: usize, ld: usize) -> Vec<f32> {
        let width = self.inp / 4;
        let mut partial = vec![vec![0.; rows * self.out]; 4];
        for (slice, values) in partial.iter_mut().enumerate() {
            let indices: Vec<_> = (0..self.inp).filter(|k| k / 8 % 4 == slice).collect();
            let weights: Vec<_> = self
                .weight
                .chunks_exact(self.inp)
                .flat_map(|row| indices.iter().map(|k| row[*k]))
                .collect();
            let input: Vec<_> = x
                .chunks_exact(ld)
                .flat_map(|row| indices.iter().map(|k| row[*k]))
                .collect();
            // The existing packed kernel preserves ascending FMA within each
            // reordered slice. Keep the scalar path for unsupported dispatch.
            if !PackedWeights::new(&weights, width, self.out)
                .is_some_and(|p| p.multiply(&input, rows, width, 1, values, self.out))
            {
                return self.forward_sliced_scalar_diagnostic(x, rows, ld);
            }
        }
        let mut y = vec![0.; rows * self.out];
        for (i, value) in y.iter_mut().enumerate() {
            *value = (partial[0][i] + partial[1][i]) + (partial[2][i] + partial[3][i]);
            if let Some(bias) = &self.bias {
                *value += bias[i % self.out];
            }
        }
        y
    }

    #[cfg(feature = "trace")]
    fn forward_sliced_scalar_diagnostic(&self, x: &[f32], rows: usize, ld: usize) -> Vec<f32> {
        let mut y = vec![0.; rows * self.out];
        y.par_chunks_exact_mut(self.out)
            .enumerate()
            .for_each(|(r, row)| {
                for (out, value) in row.iter_mut().enumerate() {
                    let mut partial = [0.0f32; 4];
                    let weights = &self.weight[out * self.inp..(out + 1) * self.inp];
                    for base in (0..self.inp).step_by(32) {
                        for (slice, sum) in partial.iter_mut().enumerate() {
                            for k in base + slice * 8..base + slice * 8 + 8 {
                                *sum = x[r * ld + k].mul_add(weights[k], *sum);
                            }
                        }
                    }
                    *value = (partial[0] + partial[1]) + (partial[2] + partial[3]);
                    if let Some(bias) = &self.bias {
                        *value += bias[out];
                    }
                }
            });
        y
    }

    fn forward_into(&self, x: &[f32], rows: usize, y: &mut Vec<f32>) {
        self.forward_strided_into(x, rows, self.inp, y);
    }
}

/// SSM state size; RE-USE uses 16 for every Mamba.
const STATE: usize = 16;

fn state16(x: &[f32]) -> &[f32; STATE] {
    x[..STATE].try_into().unwrap()
}

/// One selective-scan update of a channel: `s = exp(Δ·A)·s + Δx·B`, returns `C·s`.
/// The exps, the update and the sum are separate loops so each one vectorizes.
#[inline(always)]
pub(crate) fn scan16(
    s: &mut [f32; STATE],
    a: &[f32; STATE],
    delta: f32,
    dx: f32,
    b: &[f32; STATE],
    c: &[f32; STATE],
) -> f32 {
    let mut decay = [0f32; STATE];
    for n in 0..STATE {
        decay[n] = fast_exp(delta * a[n]);
    }
    let mut out = [0f32; STATE];
    for n in 0..STATE {
        s[n] = decay[n] * s[n] + dx * b[n];
        out[n] = s[n] * c[n];
    }
    let mut acc = [0f32; 4];
    for n in 0..STATE {
        acc[n % 4] += out[n];
    }
    (acc[0] + acc[1]) + (acc[2] + acc[3])
}

#[cfg(any(target_arch = "x86_64", test))]
type Scan16 =
    unsafe fn(&mut [f32; STATE], &[f32; STATE], f32, f32, &[f32; STATE], &[f32; STATE]) -> f32;

// Compile this small kernel for AVX2 without requiring AVX2 for the binary. Keep the
// scalar arithmetic/reduction order, including separate multiplies and adds.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn scan16_avx2(
    s: &mut [f32; STATE],
    a: &[f32; STATE],
    delta: f32,
    dx: f32,
    b: &[f32; STATE],
    c: &[f32; STATE],
) -> f32 {
    scan16(s, a, delta, dx, b, c)
}

#[cfg(all(test, target_arch = "x86_64"))]
#[target_feature(enable = "avx512f")]
unsafe fn scan16_avx512(
    s: &mut [f32; STATE],
    a: &[f32; STATE],
    delta: f32,
    dx: f32,
    b: &[f32; STATE],
    c: &[f32; STATE],
) -> f32 {
    // All 16 state values fit in one vector. Retain the unfused updates and the
    // original four-group reduction so the temporal cache stays bit-identical.
    scan16(s, a, delta, dx, b, c)
}

#[cfg(any(target_arch = "x86_64", test))]
pub(crate) fn scan_kernel() -> Scan16 {
    #[cfg(target_arch = "x86_64")]
    if std::is_x86_feature_detected!("avx2") {
        return scan16_avx2;
    }
    scan16
}

/// Scan independent channels together so vector constants and callee-saved registers can
/// be reused across the row. Keep each channel's arithmetic and four-group sum unchanged.
#[inline(always)]
fn scan_row(
    state: &mut [f32],
    a: &[f32],
    delta: &[f32],
    x: &[f32],
    b: &[f32; STATE],
    c: &[f32; STATE],
    y: &mut [f32],
) {
    assert_eq!(state.len(), y.len() * STATE);
    assert_eq!(a.len(), state.len());
    assert_eq!(delta.len(), y.len());
    assert_eq!(x.len(), y.len());
    for (channel, (s, a)) in state
        .as_chunks_mut::<STATE>()
        .0
        .iter_mut()
        .zip(a.as_chunks::<STATE>().0)
        .enumerate()
    {
        y[channel] = scan16(s, a, delta[channel], delta[channel] * x[channel], b, c);
    }
}

#[cfg(any(target_arch = "x86_64", test))]
type ScanRow =
    unsafe fn(&mut [f32], &[f32], &[f32], &[f32], &[f32; STATE], &[f32; STATE], &mut [f32]);

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn scan_row_avx2(
    state: &mut [f32],
    a: &[f32],
    delta: &[f32],
    x: &[f32],
    b: &[f32; STATE],
    c: &[f32; STATE],
    y: &mut [f32],
) {
    scan_row(state, a, delta, x, b, c, y);
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
unsafe fn scan_row_avx512(
    state: &mut [f32],
    a: &[f32],
    delta: &[f32],
    x: &[f32],
    b: &[f32; STATE],
    c: &[f32; STATE],
    y: &mut [f32],
) {
    scan_row(state, a, delta, x, b, c, y);
}

#[cfg(any(target_arch = "x86_64", test))]
fn scan_row_kernel() -> ScanRow {
    #[cfg(target_arch = "x86_64")]
    if std::is_x86_feature_detected!("avx512f") {
        return scan_row_avx512;
    }
    #[cfg(target_arch = "x86_64")]
    if std::is_x86_feature_detected!("avx2") {
        return scan_row_avx2;
    }
    scan_row
}

/// Temporary activations, not recurrent state. Every active element is overwritten per call.
#[derive(Default)]
pub struct MambaScratch {
    xz: Vec<f32>,
    xc: Vec<f32>,
    dt: Vec<f32>,
    dbl: Vec<f32>,
    y: Vec<f32>,
    out: Vec<f32>,
    blocks: Vec<Vec<f32>>,
}

impl MambaScratch {
    fn retain_output_only(&mut self) {
        let out = std::mem::take(&mut self.out);
        *self = Self {
            out,
            ..Self::default()
        };
    }
}

/// Mamba-1 parameters. Scratch is owned by the caller so sequential layers can share it.
pub struct Mamba {
    #[cfg(target_arch = "x86_64")]
    scan: Scan16,
    #[cfg(target_arch = "x86_64")]
    scan_row: ScanRow,
    in_proj: Linear,
    conv_w: Vec<f32>,
    /// `conv_w` as `[d_conv][d_inner]`, so a tap runs over contiguous channels.
    conv_taps: Vec<f32>,
    conv_b: Vec<f32>,
    x_proj: Linear,
    dt_proj: Linear,
    out_proj: Linear,
    a: Vec<f32>,
    #[cfg(feature = "trace")]
    a_cuda: Vec<f32>,
    /// `a` as `[d_state][d_inner]`, for the scan that runs across channels.
    a_t: Vec<f32>,
    d: Vec<f32>,
    pub d_inner: usize,
    pub d_state: usize,
    pub d_conv: usize,
    dt_rank: usize,
}

impl Mamba {
    fn scan_row(
        &self,
        state: &mut [f32],
        delta: &[f32],
        x: &[f32],
        b: &[f32; STATE],
        c: &[f32; STATE],
        y: &mut [f32],
    ) {
        #[cfg(target_arch = "x86_64")]
        // SAFETY: the function pointer is selected after runtime feature detection.
        unsafe {
            (self.scan_row)(state, &self.a, delta, x, b, c, y);
        }
        #[cfg(not(target_arch = "x86_64"))]
        scan_row(state, &self.a, delta, x, b, c, y);
    }

    #[inline(always)]
    fn scan(
        &self,
        s: &mut [f32; STATE],
        a: &[f32; STATE],
        delta: f32,
        dx: f32,
        b: &[f32; STATE],
        c: &[f32; STATE],
    ) -> f32 {
        #[cfg(target_arch = "x86_64")]
        // SAFETY: selected once at load after runtime CPU feature detection.
        unsafe {
            (self.scan)(s, a, delta, dx, b, c)
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            scan16(s, a, delta, dx, b, c)
        }
    }

    pub fn load(w: &Weights, prefix: &str) -> Result<Self> {
        let a_log = w.get(&format!("{prefix}.A_log"))?;
        let a_shape = w.shape(&format!("{prefix}.A_log"))?.to_vec();
        let conv_shape = w.shape(&format!("{prefix}.conv1d.weight"))?.to_vec();
        let dt_proj = Linear::load(w, &format!("{prefix}.dt_proj"), true)?;
        let conv_w = w.get(&format!("{prefix}.conv1d.weight"))?;
        let (di, dc) = (conv_shape[0], conv_shape[2]);
        let conv_taps = (0..dc * di)
            .map(|i| conv_w[(i % di) * dc + i / di])
            .collect();
        eyre::ensure!(
            a_shape[1] == STATE,
            "{prefix}: d_state {} is not {STATE}",
            a_shape[1]
        );
        Ok(Self {
            #[cfg(target_arch = "x86_64")]
            scan: scan_kernel(),
            #[cfg(target_arch = "x86_64")]
            scan_row: scan_row_kernel(),
            in_proj: Linear::load(w, &format!("{prefix}.in_proj"), false)?,
            conv_w,
            conv_taps,
            conv_b: w.get(&format!("{prefix}.conv1d.bias"))?,
            x_proj: Linear::load(w, &format!("{prefix}.x_proj"), false)?,
            out_proj: Linear::load(w, &format!("{prefix}.out_proj"), false)?,
            a_t: (0..a_log.len())
                .map(|i| -a_log[(i % di) * STATE + i / di].exp())
                .collect(),
            a: a_log.iter().map(|v| -v.exp()).collect(),
            #[cfg(feature = "trace")]
            a_cuda: a_log
                .iter()
                .map(|v| -crate::trace::exp_norm::exp(*v))
                .collect(),
            d: w.get(&format!("{prefix}.D"))?,
            d_inner: a_shape[0],
            d_state: a_shape[1],
            d_conv: conv_shape[2],
            dt_rank: dt_proj.inp,
            dt_proj,
        })
    }

    /// x_proj and dt_proj on the conv output `xc` (`rows × d_inner`), where
    /// `dbl` rows are `[dt_raw | B | C]`.
    fn ssm_inputs_into<const FAST: bool>(
        &self,
        xc: &[f32],
        rows: usize,
        dt: &mut Vec<f32>,
        dbl: &mut Vec<f32>,
    ) {
        #[cfg(feature = "profile")]
        let _timer = crate::profile::scope("ssm_inputs");
        self.x_proj.forward_into(xc, rows, dbl);
        self.dt_proj
            .forward_strided_into(dbl, rows, self.x_proj.out, dt);
        dt.par_chunks_mut(4096).for_each(softplus_in_place::<FAST>);
    }

    /// `(y + D·x) · silu(z)`, then out_proj.
    fn finish_into(&self, y: &mut [f32], xc: &[f32], xz: &[f32], rows: usize, out: &mut Vec<f32>) {
        #[cfg(feature = "profile")]
        let _timer = crate::profile::scope("mamba_finish");
        let di = self.d_inner;
        #[cfg(feature = "profile")]
        let gate_timer = crate::profile::scope("mamba_gate");
        y.par_chunks_mut(di)
            .zip(xc.par_chunks(di))
            .zip(xz.par_chunks(2 * di))
            .for_each(|((y, xc), xz)| {
                mamba_gate(y, xc, &xz[di..], &self.d);
            });
        #[cfg(feature = "profile")]
        drop(gate_timer);
        self.out_proj.forward_into(y, rows, out);
    }

    /// One state update and output, for each of `rows` independent sequences.
    /// `x` is `rows × d_model`; `conv_state` is `rows × d_inner × d_conv`; `ssm_state` is
    /// `rows × d_inner × d_state`.
    pub fn step<'a>(
        &self,
        x: &[f32],
        rows: usize,
        conv_state: &mut [f32],
        ssm_state: &mut [f32],
        scratch: &'a mut MambaScratch,
    ) -> &'a mut [f32] {
        #[cfg(feature = "profile")]
        let _timer = crate::profile::scope("time_mamba");
        let (di, ds, dc) = (self.d_inner, self.d_state, self.d_conv);
        let MambaScratch {
            xz,
            xc,
            dt,
            dbl,
            y,
            out,
            ..
        } = scratch;
        self.in_proj.forward_into(x, rows, xz);
        xc.resize(rows * di, 0.0);
        #[cfg(feature = "profile")]
        let conv_timer = crate::profile::scope("time_conv");
        xc.par_chunks_mut(di)
            .zip(conv_state.par_chunks_mut(di * dc))
            .zip(xz.par_chunks(2 * di))
            .for_each(|((out, states), input)| {
                if dc == 4 {
                    // The checkpoint uses four taps. Avoid a tiny memmove per channel,
                    // while keeping the upstream sum's order and cache representation.
                    for (c, state) in states.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                        let w = &self.conv_w[c * 4..][..4];
                        let acc = ((0.0 + state[1] * w[0]) + state[2] * w[1]) + state[3] * w[2];
                        let acc = acc + input[c] * w[3];
                        state[0] = state[1];
                        state[1] = state[2];
                        state[2] = state[3];
                        state[3] = input[c];
                        out[c] = silu(acc + self.conv_b[c]);
                    }
                } else {
                    for (c, state) in states.chunks_exact_mut(dc).enumerate() {
                        state.copy_within(1.., 0);
                        state[dc - 1] = input[c];
                        let w = &self.conv_w[c * dc..][..dc];
                        let acc: f32 = state.iter().zip(w).map(|(s, w)| s * w).sum();
                        out[c] = silu(acc + self.conv_b[c]);
                    }
                }
            });
        #[cfg(feature = "profile")]
        drop(conv_timer);
        self.ssm_inputs_into::<false>(xc, rows, dt, dbl);
        let nd = self.x_proj.out;
        y.resize(rows * di, 0.0);
        #[cfg(feature = "profile")]
        let scan_timer = crate::profile::scope("time_scan");
        y.par_chunks_mut(di)
            .zip(ssm_state.par_chunks_mut(di * ds))
            .enumerate()
            .for_each(|(r, (y, state))| {
                let b = state16(&dbl[r * nd + self.dt_rank..]);
                let cm = state16(&dbl[r * nd + self.dt_rank + ds..]);
                self.scan_row(state, &dt[r * di..][..di], &xc[r * di..][..di], b, cm, y);
            });
        #[cfg(feature = "profile")]
        drop(scan_timer);
        self.finish_into(y, xc, xz, rows, out);
        out
    }

    /// Selective scan of channels `c0..c0 + 32` from zero state over the rows of `y`
    /// (`len × d_inner`), which start at row `start`. Each vector lane is one channel, so `C·s`
    /// needs no horizontal sum.
    fn scan_channels(
        &self,
        dt: &[f32],
        dbl: &[f32],
        xc: &[f32],
        start: usize,
        c0: usize,
        y: &mut [f32],
    ) {
        const W: usize = 32;
        let (di, nd) = (self.d_inner, self.x_proj.out);
        let lanes = |v: &[f32], at: usize| -> [f32; W] { v[at..][..W].try_into().unwrap() };
        let a: [[f32; W]; STATE] = std::array::from_fn(|n| lanes(&self.a_t, n * di + c0));
        let mut s = [[0f32; W]; STATE];
        for (l, out) in y.chunks_exact_mut(di).enumerate() {
            let row = start + l;
            let b = state16(&dbl[row * nd + self.dt_rank..]);
            let cm = state16(&dbl[row * nd + self.dt_rank + STATE..]);
            let delta = lanes(dt, row * di + c0);
            let x = lanes(xc, row * di + c0);
            let dx: [f32; W] = std::array::from_fn(|j| delta[j] * x[j]);
            let mut acc = [0f32; W];
            for n in 0..STATE {
                for j in 0..W {
                    let decay = fast_exp_fma(delta[j] * a[n][j]);
                    s[n][j] = decay.mul_add(s[n][j], dx[j] * b[n]);
                    acc[j] = s[n][j].mul_add(cm[n], acc[j]);
                }
            }
            out[c0..][..W].copy_from_slice(&acc);
        }
    }

    /// Scan independent channels together, keeping scan16's four accumulator
    /// groups and separate multiplies/adds. This removes a function call and
    /// horizontal reduction per channel per position without changing rounding order.
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn scan_block_avx2<const W: usize>(
        &self,
        dt: &[f32],
        dbl: &[f32],
        xc: &[f32],
        start: usize,
        c0: usize,
        out: &mut [f32],
    ) {
        self.scan_block_impl::<W>(dt, dbl, xc, start, c0, out);
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx512f")]
    unsafe fn scan_block_avx512<const W: usize>(
        &self,
        dt: &[f32],
        dbl: &[f32],
        xc: &[f32],
        start: usize,
        c0: usize,
        out: &mut [f32],
    ) {
        self.scan_block_impl::<W>(dt, dbl, xc, start, c0, out);
    }

    #[cfg(target_arch = "x86_64")]
    #[inline(always)]
    fn scan_block_impl<const W: usize>(
        &self,
        dt: &[f32],
        dbl: &[f32],
        xc: &[f32],
        start: usize,
        c0: usize,
        out: &mut [f32],
    ) {
        let (di, nd) = (self.d_inner, self.x_proj.out);
        let a: [[f32; W]; STATE] =
            std::array::from_fn(|n| self.a_t[n * di + c0..][..W].try_into().unwrap());
        let mut state = [[0.0f32; W]; STATE];
        for (l, row_out) in out.as_chunks_mut::<W>().0.iter_mut().enumerate() {
            let row = start + l;
            let b = state16(&dbl[row * nd + self.dt_rank..]);
            let c = state16(&dbl[row * nd + self.dt_rank + STATE..]);
            let delta: &[f32; W] = dt[row * di + c0..][..W].try_into().unwrap();
            let x: &[f32; W] = xc[row * di + c0..][..W].try_into().unwrap();
            let dx: [f32; W] = std::array::from_fn(|j| delta[j] * x[j]);
            let mut acc = [[0.0f32; W]; 4];
            for n in 0..STATE {
                for j in 0..W {
                    state[n][j] = fast_exp(delta[j] * a[n][j]) * state[n][j] + dx[j] * b[n];
                    acc[n % 4][j] += state[n][j] * c[n];
                }
            }
            for j in 0..W {
                row_out[j] = (acc[0][j] + acc[1][j]) + (acc[2][j] + acc[3][j]);
            }
        }
    }

    /// Full causal passes over `batch` independent sequences, each starting from zero state.
    /// `x` is `batch × len × d_model`.
    ///
    /// With `FAST`, the scan runs across channels with fused multiply-adds, which rounds
    /// slightly differently.
    pub fn sequences<'a, const FAST: bool>(
        &self,
        x: &[f32],
        batch: usize,
        len: usize,
        scratch: &'a mut MambaScratch,
    ) -> &'a mut [f32] {
        #[cfg(feature = "profile")]
        let _timer = crate::profile::scope("frequency_mamba");
        let (di, ds, dc) = (self.d_inner, self.d_state, self.d_conv);
        let rows = batch * len;
        let MambaScratch {
            xz,
            xc,
            dt,
            dbl,
            y,
            out,
            blocks,
        } = scratch;
        self.in_proj.forward_into(x, rows, xz);
        xc.resize(rows * di, 0.0);
        // Every frequency row only reads the already-projected input; rows need not
        // wait for one another. Keep tap accumulation order identical to the reference.
        xc.par_chunks_mut(di).enumerate().for_each(|(row, out)| {
            let (seq, l) = (row / len, row % len);
            out.copy_from_slice(&self.conv_b);
            for k in (dc - 1).saturating_sub(l)..dc {
                let src = &xz[(seq * len + l + k + 1 - dc) * 2 * di..][..di];
                let taps = &self.conv_taps[k * di..][..di];
                for ((o, w), x) in out.iter_mut().zip(taps).zip(src) {
                    *o += w * x;
                }
            }
            silu_in_place(out);
        });
        self.ssm_inputs_into::<FAST>(xc, rows, dt, dbl);
        let nd = self.x_proj.out;
        // Channels are independent: each block scans its channels over one whole sequence.
        const BLOCK: usize = 32;
        y.resize(rows * di, 0.0);
        #[cfg(feature = "profile")]
        let scan_timer = crate::profile::scope("frequency_scan");
        if FAST && di % BLOCK == 0 {
            y.par_chunks_mut(len * di).enumerate().for_each(|(seq, y)| {
                for c0 in (0..di).step_by(BLOCK) {
                    self.scan_channels(dt, dbl, xc, seq * len, c0, y);
                }
            });
            #[cfg(feature = "profile")]
            drop(scan_timer);
            self.finish_into(y, xc, xz, rows, out);
            return out;
        }
        let per_seq = di.div_ceil(BLOCK);
        blocks.resize_with(batch * per_seq, Vec::new);
        blocks.par_iter_mut().enumerate().for_each(|(task, out)| {
            let (seq, blk) = (task / per_seq, task % per_seq);
            let channels = blk * BLOCK..((blk + 1) * BLOCK).min(di);
            let width = channels.len();
            out.resize(len * width, 0.0);
            #[cfg(target_arch = "x86_64")]
            if width == BLOCK && std::is_x86_feature_detected!("avx512f") {
                // SAFETY: AVX-512 is checked here; typed arrays and slices bound all accesses.
                unsafe {
                    self.scan_block_avx512::<BLOCK>(dt, dbl, xc, seq * len, blk * BLOCK, out);
                }
                return;
            }
            #[cfg(target_arch = "x86_64")]
            if width == BLOCK && std::is_x86_feature_detected!("avx2") {
                // SAFETY: AVX2 is checked here; typed arrays and slices bound all accesses.
                unsafe {
                    self.scan_block_avx2::<BLOCK>(dt, dbl, xc, seq * len, blk * BLOCK, out);
                }
                return;
            }
            let mut state = [[0f32; STATE]; BLOCK];
            for l in 0..len {
                let row = seq * len + l;
                let b = state16(&dbl[row * nd + self.dt_rank..]);
                let cm = state16(&dbl[row * nd + self.dt_rank + ds..]);
                for (j, c) in channels.clone().enumerate() {
                    let delta = dt[row * di + c];
                    out[l * width + j] = self.scan(
                        &mut state[j],
                        state16(&self.a[c * ds..]),
                        delta,
                        delta * xc[row * di + c],
                        b,
                        cm,
                    );
                }
            }
        });
        for (task, out) in blocks.iter().enumerate() {
            let (seq, blk) = (task / per_seq, task % per_seq);
            let width = out.len() / len;
            for l in 0..len {
                y[(seq * len + l) * di + blk * BLOCK..][..width]
                    .copy_from_slice(&out[l * width..][..width]);
            }
        }
        #[cfg(feature = "profile")]
        drop(scan_timer);
        self.finish_into(y, xc, xz, rows, out);
        out
    }
}

/// LayerNorm over the last dimension.
pub struct LayerNorm {
    weight: Vec<f32>,
    bias: Vec<f32>,
}

impl LayerNorm {
    pub fn load(w: &Weights, prefix: &str) -> Result<Self> {
        Ok(Self {
            weight: w.get(&format!("{prefix}.weight"))?,
            bias: w.get(&format!("{prefix}.bias"))?,
        })
    }

    pub fn apply(&self, x: &mut [f32]) {
        let dim = self.weight.len();
        #[cfg(feature = "profile")]
        let _timer = crate::profile::scope("row_norm");
        x.par_chunks_mut(dim * 1024)
            .for_each(|rows| layer_norm_rows(rows, dim, &self.weight, &self.bias));
    }

    #[cfg(feature = "trace")]
    fn apply_cuda_diagnostic(&self, x: &mut [f32]) {
        assert_eq!(self.weight.len(), 64);
        assert!(x.len().is_multiple_of(64));
        x.par_chunks_mut(64).for_each(|row| {
            let (mean, rstd) = crate::trace::exp_norm::layer_norm_stats_64(row);
            for (i, value) in row.iter_mut().enumerate() {
                *value = (rstd * (*value - mean)).mul_add(self.weight[i], self.bias[i]);
            }
        });
    }
}

/// Bidirectional MambaBlock: `norm(output_proj([fw(x) + x, flip(bw(flip x)) + x]))`.
#[derive(Default)]
pub struct BiMambaScratch {
    forward: MambaScratch,
    backward: MambaScratch,
    reversed: Vec<f32>,
    cat: Vec<f32>,
}

/// Bidirectional parameters; forward/backward passes own separate scratch when joined.
pub struct BiMambaBlock {
    forward: Mamba,
    backward: Mamba,
    output_proj: Linear,
    norm: LayerNorm,
}

#[cfg(feature = "trace")]
type FrequencyObserver<'a> = dyn FnMut(&str, &[usize], &[f32]) + 'a;

impl BiMambaBlock {
    /// Isolate a named native operator from explicit saved input. Stateless and
    /// allocating, with no streaming input override or changes to model parameters.
    #[cfg(feature = "trace")]
    pub fn replay_operator(&self, name: &str, input: &[f32]) -> Result<Vec<f32>> {
        let (name, sliced) = name
            .strip_suffix("_sliced")
            .map_or((name, false), |n| (n, true));
        eyre::ensure!(
            !sliced
                || matches!(
                    name,
                    "projection" | "fw.x_proj" | "bw.x_proj" | "fw.out_proj" | "bw.out_proj"
                ),
            "sliced replay supports only the measured projections"
        );
        eyre::ensure!(
            input.iter().all(|v| v.is_finite()),
            "nonfinite operator input"
        );
        if matches!(name, "norm" | "norm_cuda") {
            let dim = self.norm.weight.len();
            eyre::ensure!(
                !input.is_empty() && input.len().is_multiple_of(dim),
                "invalid norm shape"
            );
            let mut output = input.to_vec();
            if name == "norm_cuda" {
                eyre::ensure!(dim == 64, "CUDA norm diagnostic requires 64 channels");
                self.norm.apply_cuda_diagnostic(&mut output);
            } else {
                self.norm.apply(&mut output);
            }
            return Ok(output);
        }
        let (linear, stride, raw) = if name == "projection" {
            (&self.output_proj, self.output_proj.inp, false)
        } else {
            let (model, operation) = if let Some(op) = name.strip_prefix("fw.") {
                (&self.forward, op)
            } else if let Some(op) = name.strip_prefix("bw.") {
                (&self.backward, op)
            } else {
                eyre::bail!("unknown frequency operator {name}");
            };
            match operation {
                "a" | "a_cuda" => {
                    eyre::ensure!(input.is_empty(), "derived A takes no activation input");
                    return Ok(if operation == "a_cuda" {
                        model.a_cuda.clone()
                    } else {
                        model.a.clone()
                    });
                }
                "x_proj" => (&model.x_proj, model.d_inner, false),
                "delta_raw" => (&model.dt_proj, model.x_proj.out, true),
                "delta_logits" => (&model.dt_proj, model.x_proj.out, false),
                "out_proj" => (&model.out_proj, model.d_inner, false),
                _ => {
                    eyre::bail!("unknown frequency operator {name}");
                }
            }
        };
        eyre::ensure!(
            !input.is_empty() && input.len().is_multiple_of(stride),
            "invalid operator input shape"
        );
        let rows = input.len() / stride;
        Ok(if sliced {
            eyre::ensure!(
                rows == 481 && matches!(linear.inp, 128 | 256),
                "sliced replay requires the measured 48 kHz shape"
            );
            linear.forward_sliced_diagnostic(input, rows, stride)
        } else if raw {
            linear.product_diagnostic(input, rows, stride)
        } else {
            linear.forward_strided(input, rows, stride)
        })
    }

    /// Reuse intermediates from a completed native forward pass. The chosen
    /// diagnostic recomputes scans and optionally convolution/SiLU/gates.
    #[cfg(feature = "trace")]
    pub fn rescan_sfu_diagnostic(
        &self,
        x: &[f32],
        len: usize,
        scratch: &mut BiMambaScratch,
        math: crate::trace::FrequencyMath,
    ) -> Vec<f32> {
        assert_eq!(len, 481, "SFU diagnostic requires 481 frequency rows");
        let sliced = matches!(
            math,
            crate::trace::FrequencyMath::SlicedSfu | crate::trace::FrequencyMath::ExpNormSfu
        );
        for (model, s) in [
            (&self.forward, &mut scratch.forward),
            (&self.backward, &mut scratch.backward),
        ] {
            let di = model.d_inner;
            let ds = model.d_state;
            let nd = model.x_proj.out;
            if math != crate::trace::FrequencyMath::ScanSfu {
                s.xc.par_chunks_mut(di).enumerate().for_each(|(row, out)| {
                    for (channel, value) in out.iter_mut().enumerate() {
                        let mut acc = model.conv_b[channel];
                        for tap in 0..model.d_conv {
                            let input = if row + tap + 1 < model.d_conv {
                                0.
                            } else {
                                s.xz[(row + tap + 1 - model.d_conv) * 2 * di + channel]
                            };
                            acc = model.conv_w[channel * model.d_conv + tap].mul_add(input, acc);
                        }
                        *value = crate::trace::sfu::silu(acc);
                    }
                });
                if sliced {
                    s.dbl = model.x_proj.forward_sliced_diagnostic(&s.xc, len, di);
                } else {
                    model.ssm_inputs_into::<false>(&s.xc, len, &mut s.dt, &mut s.dbl);
                }
            }
            let delta: Vec<_> = model
                .dt_proj
                .forward_strided(&s.dbl, len, nd)
                .into_iter()
                .map(crate::trace::sfu::softplus)
                .collect();
            let b: Vec<_> = s
                .dbl
                .chunks_exact(nd)
                .flat_map(|r| r[model.dt_rank..model.dt_rank + ds].iter().copied())
                .collect();
            let c: Vec<_> = s
                .dbl
                .chunks_exact(nd)
                .flat_map(|r| r[model.dt_rank + ds..].iter().copied())
                .collect();
            let mut y = crate::trace::replay_scan_with_math(
                crate::trace::ScanInputs {
                    rows: len,
                    channels: di,
                    delta: &delta,
                    x: &s.xc,
                    a: if math == crate::trace::FrequencyMath::ExpNormSfu {
                        &model.a_cuda
                    } else {
                        &model.a
                    },
                    b: &b,
                    c: &c,
                    d: &model.d,
                },
                crate::trace::ScanMath::CudaSfu,
            )
            .expect("invalid SFU diagnostic scan");
            for (row, xz) in y.chunks_exact_mut(di).zip(s.xz.chunks_exact(2 * di)) {
                for (v, z) in row.iter_mut().zip(&xz[di..]) {
                    *v *= match math {
                        crate::trace::FrequencyMath::ScanSfu => silu(*z),
                        crate::trace::FrequencyMath::ConvGateSfu
                        | crate::trace::FrequencyMath::SlicedSfu
                        | crate::trace::FrequencyMath::ExpNormSfu => crate::trace::sfu::silu(*z),
                    };
                }
            }
            if sliced {
                s.out = model.out_proj.forward_sliced_diagnostic(&y, len, di);
            } else {
                model.out_proj.forward_into(&y, len, &mut s.out);
            }
        }
        let dm = x.len() / len;
        for (l, row) in scratch.cat.chunks_exact_mut(2 * dm).enumerate() {
            for c in 0..dm {
                row[c] = scratch.forward.out[l * dm + c] + x[l * dm + c];
                row[dm + c] = scratch.backward.out[(len - 1 - l) * dm + c] + x[l * dm + c];
            }
        }
        let mut output = if sliced {
            self.output_proj
                .forward_sliced_diagnostic(&scratch.cat, len, 2 * dm)
        } else {
            self.output_proj.forward(&scratch.cat, len)
        };
        if math == crate::trace::FrequencyMath::ExpNormSfu {
            self.norm.apply_cuda_diagnostic(&mut output);
        } else {
            self.norm.apply(&mut output);
        }
        output
    }

    /// Development-only inspection after the unchanged streaming frequency pass.
    /// Recomputed projection/logits are named explicitly; no input/cache is replaced.
    #[cfg(feature = "trace")]
    pub fn forward_with_trace(
        &self,
        x: &[f32],
        len: usize,
        observer: &mut FrequencyObserver<'_>,
    ) -> Result<Vec<f32>> {
        let mut scratch = BiMambaScratch::default();
        let output = self.forward(x, len, &mut scratch);
        for (label, model, s) in [
            ("fw", &self.forward, &scratch.forward),
            ("bw", &self.backward, &scratch.backward),
        ] {
            let (di, ds, nd) = (model.d_inner, model.d_state, model.x_proj.out);
            let mut emit = |name: &str, shape: &[usize], data: &[f32]| {
                observer(&format!("{label}.{name}"), shape, data);
            };
            emit("xz", &[len, 2 * di], &s.xz);
            emit("xc", &[len, di], &s.xc);
            emit("dbl", &[len, nd], &s.dbl);
            emit("dt", &[len, di], &s.dt);
            emit("a", &[di, ds], &model.a);
            emit("d", &[di], &model.d);
            emit("delta_bias", &[di], model.dt_proj.bias.as_ref().unwrap());
            let logits = model.dt_proj.forward_strided(&s.dbl, len, nd);
            let mut checked_dt = logits.clone();
            softplus_in_place::<false>(&mut checked_dt);
            eyre::ensure!(
                checked_dt
                    .iter()
                    .zip(&s.dt)
                    .all(|(a, b)| a.to_bits() == b.to_bits()),
                "recomputed delta changed"
            );
            emit("delta_logits_replay", &[len, di], &logits);
            let b: Vec<_> = s
                .dbl
                .chunks_exact(nd)
                .flat_map(|r| r[model.dt_rank..model.dt_rank + ds].iter().copied())
                .collect();
            let c: Vec<_> = s
                .dbl
                .chunks_exact(nd)
                .flat_map(|r| r[model.dt_rank + ds..].iter().copied())
                .collect();
            let z: Vec<_> =
                s.xz.chunks_exact(2 * di)
                    .flat_map(|r| r[di..].iter().copied())
                    .collect();
            emit("b", &[len, ds], &b);
            emit("c", &[len, ds], &c);
            emit("z", &[len, di], &z);
            // Retained scan blocks contain the exact ungated output used by finish_into.
            let mut raw = vec![0.; len * di];
            for (block, values) in s.blocks.iter().enumerate() {
                let width = values.len() / len;
                for (row, values) in values.chunks_exact(width).enumerate() {
                    raw[row * di + block * 32..][..width].copy_from_slice(values);
                }
            }
            let mut checked_gate = raw.clone();
            for ((row, xc), xz) in checked_gate
                .chunks_exact_mut(di)
                .zip(s.xc.chunks_exact(di))
                .zip(s.xz.chunks_exact(2 * di))
            {
                mamba_gate(row, xc, &xz[di..], &model.d);
            }
            eyre::ensure!(
                checked_gate
                    .iter()
                    .zip(&s.y)
                    .all(|(a, b)| a.to_bits() == b.to_bits()),
                "reconstructed scan changed gated output"
            );
            let plus_d: Vec<_> = raw
                .iter()
                .zip(&s.xc)
                .enumerate()
                .map(|(i, (y, xc))| y + model.d[i % di] * xc)
                .collect();
            emit("scan_plus_d_replay", &[len, di], &plus_d);
            emit("gate", &[len, di], &s.y);
            emit("out", &[len, x.len() / len], &s.out);
        }
        observer("cat", &[len, 2 * x.len() / len], &scratch.cat);
        let projection = self.output_proj.forward(&scratch.cat, len);
        let mut checked_output = projection.clone();
        self.norm.apply(&mut checked_output);
        eyre::ensure!(
            checked_output
                .iter()
                .zip(&output)
                .all(|(a, b)| a.to_bits() == b.to_bits()),
            "recomputed projection changed normalized output"
        );
        observer("projection_replay", &[len, x.len() / len], &projection);
        observer("output", &[len, x.len() / len], &output);
        Ok(output)
    }

    pub fn load(w: &Weights, prefix: &str) -> Result<Self> {
        Ok(Self {
            forward: Mamba::load(w, &format!("{prefix}.forward_blocks"))?,
            backward: Mamba::load(w, &format!("{prefix}.backward_blocks"))?,
            output_proj: Linear::load(w, &format!("{prefix}.output_proj"), true)?,
            norm: LayerNorm::load(w, &format!("{prefix}.norm"))?,
        })
    }

    /// `x` is `len × d_model`.
    pub fn forward(&self, x: &[f32], len: usize, scratch: &mut BiMambaScratch) -> Vec<f32> {
        self.forward_batch_buffered::<false, true>(x, 1, len, scratch)
    }

    /// [`forward`](Self::forward) for `batch` independent sequences, `x` is
    /// `batch × len × d_model`.
    pub fn forward_batch<const FAST: bool>(&self, x: &[f32], batch: usize, len: usize) -> Vec<f32> {
        self.forward_batch_buffered::<FAST, false>(x, batch, len, &mut BiMambaScratch::default())
    }

    fn forward_batch_buffered<const FAST: bool, const RETAIN: bool>(
        &self,
        x: &[f32],
        batch: usize,
        len: usize,
        scratch: &mut BiMambaScratch,
    ) -> Vec<f32> {
        let rows = batch * len;
        let dm = x.len() / rows;
        let BiMambaScratch {
            forward,
            backward,
            reversed,
            cat,
        } = scratch;
        reversed.resize(x.len(), 0.0);
        reversed
            .par_chunks_mut(len * dm)
            .zip(x.par_chunks(len * dm))
            .for_each(|(dst, src)| {
                for (d, s) in dst.chunks_exact_mut(dm).zip(src.chunks_exact(dm).rev()) {
                    d.copy_from_slice(s);
                }
            });
        rayon::join(
            || {
                self.forward.sequences::<FAST>(x, batch, len, forward);
                if !RETAIN {
                    forward.retain_output_only();
                }
            },
            || {
                self.backward
                    .sequences::<FAST>(reversed, batch, len, backward);
                if !RETAIN {
                    backward.retain_output_only();
                }
            },
        );
        // One-shot offline batches release their large intermediates at the same boundary
        // as before. Streaming keeps them for the next layer/frame, without recurrent state.
        let (fw, bw) = (&forward.out, &backward.out);
        cat.resize(rows * 2 * dm, 0.0);
        cat.par_chunks_mut(len * 2 * dm)
            .enumerate()
            .for_each(|(s, cat)| {
                for l in 0..len {
                    let r = s * len + l;
                    let row = &mut cat[l * 2 * dm..][..2 * dm];
                    let xin = &x[r * dm..][..dm];
                    let back = &bw[(s * len + len - 1 - l) * dm..][..dm];
                    for c in 0..dm {
                        row[c] = fw[r * dm + c] + xin[c];
                        row[dm + c] = back[c] + xin[c];
                    }
                }
            });
        let mut out = self.output_proj.forward(cat, rows);
        self.norm.apply(&mut out);
        out
    }
}

#[cfg(test)]
mod scan_tests {
    use super::*;

    #[test]
    fn mamba_scratch_overwrites_stale_values_across_shapes_modes_and_state_resets() {
        fn linear(inp: usize, out: usize, seed: usize, bias: bool) -> Linear {
            Linear {
                weight: (0..inp * out)
                    .map(|i| (((i * 7 + seed) % 31) as f32 - 15.0) * 0.00317)
                    .collect(),
                packed: None,
                bias: bias.then(|| (0..out).map(|i| 0.02 + (i % 7) as f32 * 0.001).collect()),
                out,
                inp,
            }
        }
        fn synthetic_model(di: usize) -> Mamba {
            let a: Vec<_> = (0..di * STATE)
                .map(|i| -0.01 * (1 + i % 7) as f32)
                .collect();
            let conv_w: Vec<_> = (0..di * 4).map(|i| (i % 11) as f32 * 0.01).collect();
            Mamba {
                #[cfg(target_arch = "x86_64")]
                scan: scan_kernel(),
                #[cfg(target_arch = "x86_64")]
                scan_row: scan_row_kernel(),
                in_proj: linear(8, di * 2, 1, false),
                x_proj: linear(di, 4 + STATE * 2, 2, false),
                dt_proj: linear(4, di, 3, true),
                out_proj: linear(di, 8, 4, false),
                conv_taps: (0..4 * di).map(|i| conv_w[(i % di) * 4 + i / di]).collect(),
                conv_w,
                conv_b: vec![0.05; di],
                d: vec![0.1; di],
                a_t: (0..di * STATE)
                    .map(|i| a[(i % di) * STATE + i / di])
                    .collect(),
                #[cfg(feature = "trace")]
                a_cuda: a.clone(),
                a,
                d_inner: di,
                d_state: STATE,
                d_conv: 4,
                dt_rank: 4,
            }
        }
        fn poison(s: &mut MambaScratch) {
            for buffer in [
                &mut s.xz, &mut s.xc, &mut s.dt, &mut s.dbl, &mut s.y, &mut s.out,
            ] {
                buffer.fill(f32::NAN);
            }
            s.blocks.iter_mut().for_each(|block| block.fill(f32::NAN));
        }
        fn same(actual: &[f32], expected: &[f32]) {
            assert_eq!(actual.len(), expected.len());
            assert!(actual
                .iter()
                .zip(expected)
                .all(|(a, b)| a.is_finite() && a.to_bits() == b.to_bits()));
        }
        for di in [45, 64] {
            let model = synthetic_model(di);
            let mut scratch = MambaScratch::default();
            for (batch, len) in [(1, 17), (2, 95), (1, 1), (2, 33), (1, 17)] {
                let input: Vec<_> = (0..batch * len * 8)
                    .map(|i| (i as f32 * 0.17).sin() * 0.1)
                    .collect();
                for fast in [false, true] {
                    poison(&mut scratch);
                    let mut fresh = MambaScratch::default();
                    if fast {
                        same(
                            model.sequences::<true>(&input, batch, len, &mut scratch),
                            model.sequences::<true>(&input, batch, len, &mut fresh),
                        );
                    } else {
                        same(
                            model.sequences::<false>(&input, batch, len, &mut scratch),
                            model.sequences::<false>(&input, batch, len, &mut fresh),
                        );
                    }
                }
            }
            for rows in [7, 19, 3] {
                let mut conv = vec![0.0; rows * di * 4];
                let mut state = vec![0.0; rows * di * STATE];
                let (mut expected_conv, mut expected_state) = (conv.clone(), state.clone());
                for step in 0..4 {
                    if step == 2 {
                        conv.fill(0.0);
                        state.fill(0.0);
                        expected_conv.fill(0.0);
                        expected_state.fill(0.0);
                    }
                    let input: Vec<_> = (0..rows * 8)
                        .map(|i| ((i + step) as f32 * 0.23).cos() * 0.1)
                        .collect();
                    poison(&mut scratch);
                    let mut fresh = MambaScratch::default();
                    same(
                        model.step(&input, rows, &mut conv, &mut state, &mut scratch),
                        model.step(
                            &input,
                            rows,
                            &mut expected_conv,
                            &mut expected_state,
                            &mut fresh,
                        ),
                    );
                    same(&conv, &expected_conv);
                    same(&state, &expected_state);
                }
            }
            let block = BiMambaBlock {
                forward: model,
                backward: synthetic_model(di),
                output_proj: linear(16, 8, 7, true),
                norm: LayerNorm {
                    weight: vec![1.0; 8],
                    bias: vec![0.0; 8],
                },
            };
            let mut scratch = BiMambaScratch::default();
            for len in [17, 95, 1, 33, 17] {
                let input: Vec<_> = (0..len * 8)
                    .map(|i| (i as f32 * 0.19).sin() * 0.1)
                    .collect();
                poison(&mut scratch.forward);
                poison(&mut scratch.backward);
                scratch.reversed.fill(f32::NAN);
                scratch.cat.fill(f32::NAN);
                same(
                    &block.forward(&input, len, &mut scratch),
                    &block.forward_batch::<false>(&input, 1, len),
                );
            }
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn block_scan_matches_scalar_channels_with_offsets_and_reset() {
        if !std::is_x86_feature_detected!("avx2") {
            return;
        }
        let (di, rank, len, offset) = (64, 4, 1025, 3);
        let nd = rank + 2 * STATE;
        let empty_linear = || Linear {
            weight: Vec::new(),
            packed: None,
            bias: None,
            out: nd,
            inp: 0,
        };
        let a: Vec<_> = (0..di * STATE)
            .map(|i| -((i % 23 + 1) as f32) * 0.25)
            .collect();
        let model = Mamba {
            scan: scan_kernel(),
            scan_row: scan_row_kernel(),
            in_proj: empty_linear(),
            x_proj: empty_linear(),
            dt_proj: empty_linear(),
            out_proj: empty_linear(),
            conv_w: Vec::new(),
            conv_taps: Vec::new(),
            conv_b: Vec::new(),
            d: Vec::new(),
            a_t: (0..di * STATE)
                .map(|i| a[(i % di) * STATE + i / di])
                .collect(),
            #[cfg(feature = "trace")]
            a_cuda: a.clone(),
            a,
            d_inner: di,
            d_state: STATE,
            d_conv: 4,
            dt_rank: rank,
        };
        let dt: Vec<_> = (0..(len + offset) * di)
            .map(|i| [0.0, 0.001, 0.5, 100.0][i % 4])
            .collect();
        let xc: Vec<_> = (0..dt.len()).map(|i| (i as f32 * 0.071).sin()).collect();
        let dbl: Vec<_> = (0..(len + offset) * nd)
            .map(|i| (i as f32 * 0.13).cos())
            .collect();
        fn check<const W: usize>(model: &Mamba, dt: &[f32], dbl: &[f32], xc: &[f32]) {
            let (di, rank, len, offset) = (64, 4, 1025, 3);
            let nd = rank + 2 * STATE;
            for c0 in (0..di).step_by(W) {
                let mut expected = vec![0.0; len * W];
                let mut state = [[0.0; STATE]; W];
                for l in 0..len {
                    let row = offset + l;
                    for j in 0..W {
                        let channel = c0 + j;
                        expected[l * W + j] = scan16(
                            &mut state[j],
                            state16(&model.a[channel * STATE..]),
                            dt[row * di + channel],
                            dt[row * di + channel] * xc[row * di + channel],
                            state16(&dbl[row * nd + rank..]),
                            state16(&dbl[row * nd + rank + STATE..]),
                        );
                    }
                }
                let mut actual = vec![f32::NAN; len * W];
                for _ in 0..2 {
                    // SAFETY: AVX2 was checked above; both full blocks and offset rows are allocated.
                    unsafe {
                        model.scan_block_avx2::<W>(dt, dbl, xc, offset, c0, &mut actual);
                    }
                    assert!(
                        actual
                            .iter()
                            .zip(&expected)
                            .all(|(a, b)| a.to_bits() == b.to_bits()),
                        "AVX2 width={W} channel={c0}"
                    );
                    if std::is_x86_feature_detected!("avx512f") {
                        // SAFETY: AVX-512 was checked above; buffers are the same full blocks.
                        unsafe {
                            model.scan_block_avx512::<W>(dt, dbl, xc, offset, c0, &mut actual);
                        }
                        assert!(
                            actual
                                .iter()
                                .zip(&expected)
                                .all(|(a, b)| a.to_bits() == b.to_bits()),
                            "AVX-512 width={W} channel={c0}"
                        );
                    }
                }
            }
        }
        check::<8>(&model, &dt, &dbl, &xc);
        check::<16>(&model, &dt, &dbl, &xc);
        check::<32>(&model, &dt, &dbl, &xc);
        check::<64>(&model, &dt, &dbl, &xc);
    }

    #[test]
    fn wide_convolution_matches_direct_sum_with_padding_dilation_and_reset() {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap();
        pool.install(|| {
            for stride in [1, 2] {
                let (cin, cout, taps, kf, freq, dilation, pad) = (16, 32, 3, 3, 513, 2, 1);
                let rows = cin * taps * kf;
                let weight: Vec<_> = (0..cout * rows)
                    .map(|i| ((i % 11) as f32 - 5.0) / 128.0)
                    .collect();
                let bias: Vec<_> = (0..cout).map(|i| i as f32 / 64.0).collect();
                let mut conv = StreamConv {
                    packed: PackedWeights::new(&weight, rows, cout),
                    packing_map: (0..rows)
                        .map(|p| {
                            (
                                p / (taps * kf),
                                (p / kf % taps) * dilation,
                                (p % kf) as isize - pad as isize,
                            )
                        })
                        .collect(),
                    tile_scratch: Vec::new(),
                    weight,
                    bias,
                    cin,
                    cout,
                    taps,
                    dilation,
                    kf,
                    freq_pad: pad,
                    stride,
                    cache: VecDeque::new(),
                    spare_frames: Vec::new(),
                    cols: Vec::new(),
                    position_out: Vec::new(),
                };
                let mut history =
                    VecDeque::from(vec![vec![0.0; cin * freq]; (taps - 1) * dilation]);
                for step in 0..6 {
                    if step == 3 {
                        conv.reset();
                        history.iter_mut().for_each(|frame| frame.fill(0.0));
                    }
                    let x: Vec<_> = (0..cin * freq)
                        .map(|i| ((i + step * 17) % 31) as f32 / 64.0 - 0.25)
                        .collect();
                    history.push_back(x.clone());
                    let actual = conv.step(&x);
                    let width = (freq + 2 * pad - kf) / stride + 1;
                    for co in 0..cout {
                        for f in 0..width {
                            let mut expected = 0.0;
                            for ci in 0..cin {
                                for tap in 0..taps {
                                    for k in 0..kf {
                                        let at = f * stride + k;
                                        if at >= pad && at - pad < freq {
                                            expected += conv.weight
                                                [co * rows + (ci * taps + tap) * kf + k]
                                                * history[tap * dilation][ci * freq + at - pad];
                                        }
                                    }
                                }
                            }
                            assert_eq!(actual[co * width + f], expected + conv.bias[co]);
                        }
                    }
                    history.pop_front();
                }
            }
        });
    }

    #[test]
    fn dispatched_scan_preserves_portable_state_and_reduction() {
        let check = |kernel: Scan16| {
            let mut expected = [0.0; STATE];
            let mut actual = expected;
            let a = std::array::from_fn(|i| -(i as f32 + 1.0));
            for step in 0..2048 {
                let b = std::array::from_fn(|i| ((step * 13 + i * 7) as f32 * 0.17).sin());
                let c = std::array::from_fn(|i| ((step * 11 + i * 3) as f32 * 0.11).cos());
                let delta = match step % 4 {
                    0 => 0.0,
                    1 => 0.001,
                    2 => 0.5,
                    _ => 100.0,
                };
                let dx = delta * (step as f32 * 0.1).sin();
                let reference = scan16(&mut expected, &a, delta, dx, &b, &c);
                // SAFETY: callers check support before passing a kernel to this closure.
                let result = unsafe { kernel(&mut actual, &a, delta, dx, &b, &c) };
                assert_eq!(
                    result.to_bits(),
                    reference.to_bits(),
                    "reduction at update {step}"
                );
                assert_eq!(
                    actual.map(f32::to_bits),
                    expected.map(f32::to_bits),
                    "state at update {step}"
                );
            }
        };
        check(scan_kernel());
        #[cfg(target_arch = "x86_64")]
        {
            if std::is_x86_feature_detected!("avx2") {
                check(scan16_avx2);
            }
            if std::is_x86_feature_detected!("avx512f") {
                check(scan16_avx512);
            }
        }
    }

    #[test]
    fn row_scan_preserves_per_channel_bits_with_offsets_and_resets() {
        let check = |kernel: ScanRow| {
            for channels in [1, 7, 32, 128] {
                for offset in [0, 3] {
                    let count = channels * STATE;
                    let mut storage = vec![123.0; offset + count + 4];
                    let mut reference = vec![0.0; count];
                    storage[offset..offset + count].fill(0.0);
                    let a: Vec<_> = (0..count)
                        .map(|i| -((i % STATE) as f32 + 1.0) * 0.25)
                        .collect();
                    let mut delta = vec![0.0; channels];
                    let mut x = vec![0.0; channels];
                    let mut actual = vec![0.0; channels];
                    for step in 0..512 {
                        if step == 256 {
                            reference.fill(0.0);
                            storage[offset..offset + count].fill(0.0);
                        }
                        let b = std::array::from_fn(|i| ((step * 13 + i * 7) as f32 * 0.17).sin());
                        let c = std::array::from_fn(|i| ((step * 11 + i * 3) as f32 * 0.11).cos());
                        for ch in 0..channels {
                            delta[ch] = [0.0, 0.001, 0.5, 100.0][(step + ch) % 4];
                            x[ch] = ((step * 7 + ch) as f32 * 0.13).sin();
                        }
                        // SAFETY: feature support is checked before invoking this closure.
                        unsafe {
                            kernel(
                                &mut storage[offset..offset + count],
                                &a,
                                &delta,
                                &x,
                                &b,
                                &c,
                                &mut actual,
                            );
                        }
                        for ch in 0..channels {
                            let s = (&mut reference[ch * STATE..][..STATE]).try_into().unwrap();
                            let expected = scan16(
                                s,
                                state16(&a[ch * STATE..]),
                                delta[ch],
                                delta[ch] * x[ch],
                                &b,
                                &c,
                            );
                            assert_eq!(
                                actual[ch].to_bits(),
                                expected.to_bits(),
                                "output ch={ch} step={step}"
                            );
                        }
                        assert!(storage[offset..offset + count]
                            .iter()
                            .zip(&reference)
                            .all(|(a, b)| a.to_bits() == b.to_bits()));
                        assert!(storage[..offset]
                            .iter()
                            .chain(&storage[offset + count..])
                            .all(|v| *v == 123.0));
                    }
                }
            }
        };
        check(scan_row_kernel());
        #[cfg(target_arch = "x86_64")]
        {
            if std::is_x86_feature_detected!("avx2") {
                check(scan_row_avx2);
            }
            if std::is_x86_feature_detected!("avx512f") {
                check(scan_row_avx512);
            }
        }
    }
}

#[cfg(all(test, target_arch = "x86_64"))]
#[path = "scan_bench.rs"]
mod benchmark;
