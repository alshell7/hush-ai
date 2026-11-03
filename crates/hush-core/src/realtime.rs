//! Real-time RE-USE (nvidia/Real-time_RE-USE), one 20 ms STFT frame per step.

use crate::layers::{
    BiMambaBlock, BiMambaScratch, DenseBlock, LayerNorm, Linear, Mamba, MambaScratch, NormAct,
    StreamConv,
};
use crate::ops::{gemm, transpose, ChannelNormScratch};
use crate::weights::Weights;
use eyre::Result;
use std::path::Path;

#[cfg(feature = "trace")]
type Observer<'a> = dyn FnMut(&str, &[usize], &[f32]) + 'a;

#[cfg(feature = "trace")]
fn observe(observer: &mut Option<&mut Observer<'_>>, name: &str, shape: &[usize], x: &[f32]) {
    if let Some(observer) = observer {
        observer(name, shape, x);
    }
}

/// Causal time Mamba: `norm(output_proj(mamba.step(x) + x))`, one step per frame for every bin.
struct TimeMamba {
    mamba: Mamba,
    output_proj: Linear,
    norm: LayerNorm,
    conv_state: Vec<f32>,
    ssm_state: Vec<f32>,
}

impl TimeMamba {
    fn load(w: &Weights, prefix: &str) -> Result<Self> {
        Ok(Self {
            mamba: Mamba::load(w, &format!("{prefix}.forward_blocks"))?,
            output_proj: Linear::load(w, &format!("{prefix}.output_proj"), true)?,
            norm: LayerNorm::load(w, &format!("{prefix}.norm"))?,
            conv_state: Vec::new(),
            ssm_state: Vec::new(),
        })
    }

    fn step(&mut self, x: &[f32], rows: usize, scratch: &mut MambaScratch) -> Vec<f32> {
        let m = &self.mamba;
        if self.ssm_state.len() != rows * m.d_inner * m.d_state {
            self.conv_state.clear();
            self.ssm_state.clear();
            self.conv_state.resize(rows * m.d_inner * m.d_conv, 0.0);
            self.ssm_state.resize(rows * m.d_inner * m.d_state, 0.0);
        }
        let y = m.step(x, rows, &mut self.conv_state, &mut self.ssm_state, scratch);
        y.iter_mut().zip(x).for_each(|(v, x)| *v += x);
        let mut out = self.output_proj.forward(y, rows);
        self.norm.apply(&mut out);
        out
    }

    fn reset(&mut self) {
        self.conv_state.clear();
        self.ssm_state.clear();
    }
}

struct Block {
    time: TimeMamba,
    freq: BiMambaBlock,
}

struct Decoder {
    dense: DenseBlock,
    up1: StreamConv,
    up1_act: NormAct,
    up2: StreamConv,
    up2_act: NormAct,
    norm_scratch: ChannelNormScratch,
    /// One 1×1 head (magnitude) or two (phase real, imaginary).
    heads: Vec<(Vec<f32>, f32)>,
}

impl Decoder {
    fn load(w: &Weights, prefix: &str, heads: &[&str]) -> Result<Self> {
        Ok(Self {
            dense: DenseBlock::load(w, &format!("{prefix}.dense_block"))?,
            up1: StreamConv::load(w, &format!("{prefix}.up_conv1.0.conv"), 1, 1, 1)?,
            up1_act: NormAct::load(
                w,
                &format!("{prefix}.up_conv1.1"),
                &format!("{prefix}.up_conv1.2"),
            )?,
            up2: StreamConv::load_time_last(w, &format!("{prefix}.up_conv2.0.conv"))?,
            up2_act: NormAct::load(
                w,
                &format!("{prefix}.up_conv2.1"),
                &format!("{prefix}.up_conv2.2"),
            )?,
            norm_scratch: ChannelNormScratch::default(),
            heads: heads
                .iter()
                .map(|h| {
                    Ok((
                        w.get(&format!("{prefix}.{h}.weight"))?,
                        w.get(&format!("{prefix}.{h}.bias"))?[0],
                    ))
                })
                .collect::<Result<_>>()?,
        })
    }

    fn reset(&mut self) {
        self.dense.reset();
        self.up2.reset();
    }

    /// `x` is `[channels][freq]`; returns one `[2·freq]` row per head.
    fn step(&mut self, x: &[f32]) -> Vec<Vec<f32>> {
        let y = self.dense.step(x);
        let up = self.up1.step(&y);
        // Sub-pixel shuffle: conv channel r·C + c at bin w becomes channel c at bin 2w + r.
        let channels = self.heads[0].0.len();
        let freq = y.len() / channels;
        let mut shuffled = vec![0f32; up.len()];
        for r in 0..2 {
            for c in 0..channels {
                for w in 0..freq {
                    shuffled[c * 2 * freq + 2 * w + r] = up[(r * channels + c) * freq + w];
                }
            }
        }
        self.up1_act
            .apply_buffered(&mut shuffled, &mut self.norm_scratch);
        let mut z = self.up2.step(&shuffled);
        self.up2_act.apply_buffered(&mut z, &mut self.norm_scratch);
        let positions = 2 * freq;
        self.heads
            .iter()
            .map(|(weight, bias)| {
                let mut out = vec![*bias; positions];
                gemm(
                    false, false, 1, positions, channels, weight, channels, &z, positions, 1.0,
                    &mut out, positions,
                );
                out
            })
            .collect()
    }
}

/// Streaming real-time RE-USE on the CPU.
pub struct Streaming {
    encoder_in: StreamConv,
    encoder_in_act: NormAct,
    encoder_dense: DenseBlock,
    encoder_out: StreamConv,
    encoder_out_act: NormAct,
    encoder_norm_scratch: ChannelNormScratch,
    blocks: Vec<Block>,
    // Sequential layers share scratch; forward/backward frequency passes remain disjoint.
    // Retained activations are overwritten, so reset only needs to clear recurrent state.
    time_scratch: MambaScratch,
    freq_scratch: BiMambaScratch,
    mag: Decoder,
    pha: Decoder,
    look_ahead: usize,
    /// Frames still to buffer before the first output.
    waiting: usize,
}

impl Streaming {
    /// Loads the first `layers` TF-Mamba blocks (3–12) and the decoder pair trained to exit there.
    ///
    /// `look_ahead` (0–2) is how many future frames the model sees: the output for frame `t`
    /// comes out of the step that receives frame `t + look_ahead`.
    pub fn load(path: impl AsRef<Path>, layers: usize, look_ahead: usize) -> Result<Self> {
        let w = Weights::load(path)?;
        eyre::ensure!(
            (3..=12).contains(&layers),
            "layers must be 3..=12, got {layers}"
        );
        eyre::ensure!(
            look_ahead <= 2,
            "look_ahead must be 0..=2, got {look_ahead}"
        );
        let encoder_in = format!("dense_encoder.dense_conv_1_{}", look_ahead + 1);
        let blocks = (0..layers)
            .map(|i| {
                Ok(Block {
                    time: TimeMamba::load(&w, &format!("TSMamba.{i}.time_mamba"))?,
                    freq: BiMambaBlock::load(&w, &format!("TSMamba.{i}.freq_mamba"))?,
                })
            })
            .collect::<Result<_>>()?;
        let exit = layers - 1;
        Ok(Self {
            encoder_in: StreamConv::load(&w, &format!("{encoder_in}.0"), 1, 1, 1)?,
            encoder_in_act: NormAct::load(
                &w,
                &format!("{encoder_in}.1"),
                &format!("{encoder_in}.2"),
            )?,
            encoder_dense: DenseBlock::load(&w, "dense_encoder.dense_block")?,
            encoder_out: StreamConv::load(&w, "dense_encoder.dense_conv_2.0", 1, 0, 2)?,
            encoder_out_act: NormAct::load(
                &w,
                "dense_encoder.dense_conv_2.1",
                "dense_encoder.dense_conv_2.2",
            )?,
            encoder_norm_scratch: ChannelNormScratch::default(),
            blocks,
            time_scratch: MambaScratch::default(),
            freq_scratch: BiMambaScratch::default(),
            mag: Decoder::load(&w, &format!("mask_decoder_list.{exit}"), &["final_conv"])?,
            pha: Decoder::load(
                &w,
                &format!("phase_decoder_list.{exit}"),
                &["phase_conv_r", "phase_conv_i"],
            )?,
            look_ahead,
            waiting: look_ahead,
        })
    }

    /// Future frames the model sees (0–2).
    pub fn look_ahead(&self) -> usize {
        self.look_ahead
    }

    /// Read-only temporal Mamba caches for validation. Layouts are respectively
    /// `[frequency][inner_channel][conv_tap]` and `[frequency][inner_channel][state]`.
    /// They are empty until the first inferred frame and are cleared by `reset`.
    pub fn temporal_state(&self, layer: usize) -> Option<(&[f32], &[f32])> {
        self.blocks.get(layer).map(|block| {
            (
                block.time.conv_state.as_slice(),
                block.time.ssm_state.as_slice(),
            )
        })
    }

    pub fn reset(&mut self) {
        self.waiting = self.look_ahead;
        self.encoder_in.reset();
        self.encoder_dense.reset();
        for b in &mut self.blocks {
            b.time.reset();
        }
        self.mag.reset();
        self.pha.reset();
    }

    /// One frame: log1p-compressed magnitude and phase (`bins` each) in, enhanced ones out.
    ///
    /// The first `look_ahead` steps after loading or [`reset`](Self::reset) only buffer and
    /// return `None`; every later step returns the frame `look_ahead` steps back.
    pub fn step(&mut self, mag: &[f32], pha: &[f32]) -> Option<(Vec<f32>, Vec<f32>)> {
        self.step_inner(
            mag,
            pha,
            #[cfg(feature = "trace")]
            None,
            #[cfg(feature = "trace")]
            None,
        )
    }

    /// Observe read-only intermediate tensors during a normal step (development only).
    ///
    /// Values are borrowed only for the duration of the callback. The callback must not
    /// be used in an audio callback or a latency measurement. Look-ahead buffering emits
    /// no observations. Shapes omit unit batch/time axes; encoder tensors are channel-major,
    /// Mamba activations are frequency-major. No replacement tensors are accepted.
    #[cfg(feature = "trace")]
    pub fn step_with_trace(
        &mut self,
        mag: &[f32],
        pha: &[f32],
        observer: &mut Observer<'_>,
    ) -> Option<(Vec<f32>, Vec<f32>)> {
        self.step_inner(mag, pha, Some(observer), None)
    }

    /// Allocating, development-only 48 kHz experiment: recompute frequency scans
    /// with the observed CUDA arithmetic. Other operators and derived A stay native.
    /// Never use this path for latency measurements or mix it into a live session.
    #[cfg(feature = "trace")]
    pub fn step_with_frequency_sfu_diagnostic(
        &mut self,
        mag: &[f32],
        pha: &[f32],
    ) -> Result<Option<(Vec<f32>, Vec<f32>)>> {
        self.step_with_frequency_math_diagnostic(mag, pha, crate::trace::FrequencyMath::ScanSfu)
    }

    /// Explicit 48 kHz diagnostic. Modes progressively add convolution/gating,
    /// sliced projections, then derived A and normalization (`ExpNormSfu`).
    /// Temporal operators and synthesis remain native in every mode.
    #[cfg(feature = "trace")]
    pub fn step_with_frequency_math_diagnostic(
        &mut self,
        mag: &[f32],
        pha: &[f32],
        math: crate::trace::FrequencyMath,
    ) -> Result<Option<(Vec<f32>, Vec<f32>)>> {
        self.step_frequency_diagnostic_inner(mag, pha, math, None)
    }

    /// Observe the combined frequency diagnostic without substituting saved activations.
    /// This allocating path is for numerical localization, never latency measurement.
    #[cfg(feature = "trace")]
    pub fn step_with_frequency_math_trace_diagnostic(
        &mut self,
        mag: &[f32],
        pha: &[f32],
        math: crate::trace::FrequencyMath,
        observer: &mut Observer<'_>,
    ) -> Result<Option<(Vec<f32>, Vec<f32>)>> {
        self.step_frequency_diagnostic_inner(mag, pha, math, Some(observer))
    }

    #[cfg(feature = "trace")]
    fn step_frequency_diagnostic_inner(
        &mut self,
        mag: &[f32],
        pha: &[f32],
        math: crate::trace::FrequencyMath,
        observer: Option<&mut Observer<'_>>,
    ) -> Result<Option<(Vec<f32>, Vec<f32>)>> {
        eyre::ensure!(
            mag.len() == 961 && pha.len() == 961,
            "SFU diagnostic requires 48 kHz / 961 input bins"
        );
        eyre::ensure!(
            mag.iter().chain(pha).all(|x| x.is_finite()),
            "nonfinite diagnostic input"
        );
        Ok(self.step_inner(mag, pha, observer, Some(math)))
    }

    /// Replay one stateless frequency block from `[frequency, channels]` input.
    ///
    /// Development-only numerical isolation: uses fresh scratch and never changes
    /// streaming caches. This is not a streaming input override or a timing path.
    #[cfg(feature = "trace")]
    pub fn replay_frequency_block(&self, layer: usize, input: &[f32]) -> Result<Vec<f32>> {
        self.replay_frequency_inner(layer, input, None, None)
    }

    /// Replay the complete 481-row frequency diagnostic with fresh scratch.
    /// All intermediates are computed from `input` and original weights; no caches change.
    #[cfg(feature = "trace")]
    pub fn replay_frequency_block_with_math_diagnostic(
        &self,
        layer: usize,
        input: &[f32],
        math: crate::trace::FrequencyMath,
    ) -> Result<Vec<f32>> {
        self.replay_frequency_inner(layer, input, None, Some(math))
    }

    /// Development-only, stateless replay of a named frequency projection or norm.
    /// Names: `fw.`/`bw.` plus `x_proj`, `delta_raw`, `delta_logits`, `out_proj`,
    /// `a`; or block-level `projection`, `norm`. A takes empty input; other inputs
    /// contain complete position-major rows, with delta using full x-projection rows.
    /// Add `_sliced` to x/out/block projection names for the measured 481-row
    /// cuBLAS sliced1x4 diagnostic; this is never selected by ordinary inference.
    /// `fw.a_cuda`, `bw.a_cuda` and `norm_cuda` use the independently checked
    /// exponential and 64-channel normalization diagnostics.
    #[cfg(feature = "trace")]
    pub fn replay_frequency_operator(
        &self,
        layer: usize,
        name: &str,
        input: &[f32],
    ) -> Result<Vec<f32>> {
        self.blocks
            .get(layer)
            .ok_or_else(|| eyre::eyre!("invalid layer {layer}"))?
            .freq
            .replay_operator(name, input)
    }

    /// Replay an isolated encoder operator from an explicit NCHW tensor.
    /// Names are `in`, `dense.0`..`dense.3`, or `out`, followed by `.conv`, `.block`, `.block_cuda`, `.act`,
    /// `.prelu`, `.norm_cuda`, `.act_cuda`, `.mean_cuda` or `.rstd_cuda`. Convolution input includes its complete
    /// temporal history and frequency padding. No streaming state is read or modified.
    #[cfg(feature = "trace")]
    pub fn replay_encoder_operator(
        &self,
        name: &str,
        input: &[f32],
        shape: &[usize],
    ) -> Result<Vec<f32>> {
        eyre::ensure!(
            shape.len() == 4
                && shape.iter().all(|n| *n > 0)
                && shape.iter().try_fold(1usize, |n, v| n.checked_mul(*v)) == Some(input.len())
                && input.iter().all(|v| v.is_finite()),
            "invalid encoder diagnostic shape/data"
        );
        let (stage, mode) = name
            .rsplit_once('.')
            .ok_or_else(|| eyre::eyre!("missing encoder operator"))?;
        let (conv, act) = match stage {
            "in" => (&self.encoder_in, &self.encoder_in_act),
            "out" => (&self.encoder_out, &self.encoder_out_act),
            _ => {
                let layer = stage
                    .strip_prefix("dense.")
                    .ok_or_else(|| eyre::eyre!("unknown encoder stage"))?
                    .parse::<usize>()?;
                self.encoder_dense
                    .diagnostic_layer(layer)
                    .ok_or_else(|| eyre::eyre!("invalid dense layer"))?
            }
        };
        if matches!(mode, "conv" | "block" | "block_cuda") {
            let convolved = conv.replay_padded_diagnostic(input, shape)?;
            if mode == "conv" {
                Ok(convolved)
            } else {
                act.replay_diagnostic(
                    &convolved,
                    if mode == "block_cuda" {
                        "act_cuda"
                    } else {
                        "act"
                    },
                )
            }
        } else {
            eyre::ensure!(
                shape[0] == 1 && shape[1] == conv.cout && shape[2] == 1,
                "invalid encoder activation axes"
            );
            act.replay_diagnostic(input, mode)
        }
    }

    /// Inspect the unchanged frequency pass and explicitly named recomputed stages.
    /// Development only; never use this path for latency measurements.
    #[cfg(feature = "trace")]
    pub fn replay_frequency_block_with_trace(
        &self,
        layer: usize,
        input: &[f32],
        observer: &mut Observer<'_>,
    ) -> Result<Vec<f32>> {
        self.replay_frequency_inner(layer, input, Some(observer), None)
    }

    #[cfg(feature = "trace")]
    fn replay_frequency_inner(
        &self,
        layer: usize,
        input: &[f32],
        observer: Option<&mut Observer<'_>>,
        math: Option<crate::trace::FrequencyMath>,
    ) -> Result<Vec<f32>> {
        let block = self
            .blocks
            .get(layer)
            .ok_or_else(|| eyre::eyre!("invalid layer {layer}"))?;
        let channels = self.encoder_out.cout;
        eyre::ensure!(
            !input.is_empty() && input.len().is_multiple_of(channels),
            "frequency input must contain complete rows of {channels} channels"
        );
        eyre::ensure!(
            input.iter().all(|x| x.is_finite()),
            "nonfinite frequency input"
        );
        eyre::ensure!(
            math.is_none() || input.len() == 481 * channels,
            "SFU diagnostic requires 481 frequency rows"
        );
        if let Some(observer) = observer {
            block
                .freq
                .forward_with_trace(input, input.len() / channels, observer)
        } else {
            let mut scratch = BiMambaScratch::default();
            let len = input.len() / channels;
            let output = block.freq.forward(input, len, &mut scratch);
            Ok(if let Some(math) = math {
                block
                    .freq
                    .rescan_sfu_diagnostic(input, len, &mut scratch, math)
            } else {
                output
            })
        }
    }

    fn step_inner(
        &mut self,
        mag: &[f32],
        pha: &[f32],
        #[cfg(feature = "trace")] mut observer: Option<&mut Observer<'_>>,
        #[cfg(feature = "trace")] frequency_math: Option<crate::trace::FrequencyMath>,
    ) -> Option<(Vec<f32>, Vec<f32>)> {
        let bins = mag.len();
        let freq_in = bins + 2;
        let mut x = vec![0f32; 2 * freq_in];
        x[..bins].copy_from_slice(mag);
        x[freq_in..freq_in + bins].copy_from_slice(pha);

        // The look-ahead encoder conv starts with 2 - look_ahead zero frames instead of 2.
        if self.waiting > 0 {
            self.encoder_in.push(&x, 2 - self.look_ahead);
            self.waiting -= 1;
            return None;
        }

        let mut h = self.encoder_in.step(&x);
        self.encoder_in_act
            .apply_buffered(&mut h, &mut self.encoder_norm_scratch);
        #[cfg(feature = "trace")]
        observe(
            &mut observer,
            "encoder.in",
            &[self.encoder_in.cout, h.len() / self.encoder_in.cout],
            &h,
        );
        let h = self.encoder_dense.step(&h);
        #[cfg(feature = "trace")]
        observe(
            &mut observer,
            "encoder.dense",
            &[self.encoder_in.cout, h.len() / self.encoder_in.cout],
            &h,
        );
        let mut h = self.encoder_out.step(&h);
        self.encoder_out_act
            .apply_buffered(&mut h, &mut self.encoder_norm_scratch);

        let channels = self.encoder_out.cout;
        let freq = h.len() / channels;
        #[cfg(feature = "trace")]
        observe(&mut observer, "encoder.out", &[channels, freq], &h);
        let mut rows = transpose(&h, channels, freq);
        // Only diagnostic builds consume the index; normal builds erase the observer.
        #[cfg_attr(not(feature = "trace"), allow(clippy::unused_enumerate_index))]
        for (_layer, block) in self.blocks.iter_mut().enumerate() {
            let t = block.time.step(&rows, freq, &mut self.time_scratch);
            #[cfg(feature = "trace")]
            observe(
                &mut observer,
                &format!("blocks.{_layer}.time"),
                &[freq, channels],
                &t,
            );
            rows.iter_mut().zip(&t).for_each(|(r, t)| *r += t);
            #[cfg(feature = "trace")]
            observe(
                &mut observer,
                &format!("blocks.{_layer}.time_residual"),
                &[freq, channels],
                &rows,
            );
            let f = block.freq.forward(&rows, freq, &mut self.freq_scratch);
            #[cfg(feature = "trace")]
            let f = if let Some(math) = frequency_math {
                block
                    .freq
                    .rescan_sfu_diagnostic(&rows, freq, &mut self.freq_scratch, math)
            } else {
                f
            };
            #[cfg(feature = "trace")]
            observe(
                &mut observer,
                &format!("blocks.{_layer}.freq"),
                &[freq, channels],
                &f,
            );
            rows.iter_mut().zip(&f).for_each(|(r, f)| *r += f);
            #[cfg(feature = "trace")]
            {
                observe(
                    &mut observer,
                    &format!("blocks.{_layer}.output"),
                    &[freq, channels],
                    &rows,
                );
                let m = &block.time.mamba;
                observe(
                    &mut observer,
                    &format!("blocks.{_layer}.conv_state"),
                    &[freq, m.d_inner, m.d_conv],
                    &block.time.conv_state,
                );
                observe(
                    &mut observer,
                    &format!("blocks.{_layer}.ssm_state"),
                    &[freq, m.d_inner, m.d_state],
                    &block.time.ssm_state,
                );
            }
        }
        let h = transpose(&rows, freq, channels);

        let (mag_dec, pha_dec) = (&mut self.mag, &mut self.pha);
        let (mut mag, pha) = rayon::join(|| mag_dec.step(&h), || pha_dec.step(&h));
        let mag = mag.swap_remove(0);
        #[cfg(feature = "trace")]
        {
            observe(&mut observer, "decoder.mag", &[mag.len()], &mag);
            observe(&mut observer, "decoder.real", &[pha[0].len()], &pha[0]);
            observe(&mut observer, "decoder.imag", &[pha[1].len()], &pha[1]);
        }
        let phase = pha[0]
            .iter()
            .zip(&pha[1])
            .map(|(r, i)| i.atan2(*r))
            .collect::<Vec<_>>();
        #[cfg(feature = "trace")]
        {
            observe(&mut observer, "output.mag", &[bins], &mag[..bins]);
            observe(&mut observer, "output.phase", &[bins], &phase[..bins]);
        }
        Some((mag[..bins].to_vec(), phase[..bins].to_vec()))
    }
}
