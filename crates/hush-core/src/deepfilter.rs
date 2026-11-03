//! DeepFilterNet 3 (Schröter et al. 2023, <https://github.com/Rikorose/DeepFilterNet>) behind
//! the same streaming interface as [`Denoiser`](crate::Denoiser): the official Rust inference
//! (libDF 0.5.6 with tract), with the settings of its `deep-filter` command.

use crate::{Enhancer, WorkerScheduling};
use df::tract::{DfParams, DfTract, ReduceMask, RuntimeParams};
use eyre::{ensure, eyre, Result};
use ndarray::{ArrayView2, ArrayViewMut2};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

/// DeepFilterNet 3 runs at 48 kHz.
pub const DEEP_FILTER_RATE: u32 = 48_000;

/// Streaming DeepFilterNet 3 for mono 48 kHz audio, on one thread.
///
/// Feed any number of samples to [`process_into`](Self::process_into); it returns the enhanced
/// samples that are complete so far, in blocks of [`frame`](Self::frame) samples (10 ms).
/// Output sample `i` is the enhanced input sample `i`, delayed by [`latency`](Self::latency)
/// samples (30 ms: the STFT window and two frames of look-ahead).
///
/// As in libDF, a frame of digital silence comes out as silence and does not move the model
/// forward.
pub struct DeepFilter {
    model: DfTract,
    /// The model as loaded; [`reset`](Self::reset) copies it back.
    fresh: DfTract,
    hop: usize,
    delay: usize,
    /// Input for the next frame; the first `filled` samples are set.
    input: Vec<f32>,
    filled: usize,
    output: Vec<f32>,
    /// Output samples still to drop: the model delay at the start of a stream.
    skip: usize,
    /// Input samples not yet returned.
    owed: usize,
    scheduling: WorkerScheduling,
}

impl DeepFilter {
    /// Loads a model archive such as `DeepFilterNet3_onnx.tar.gz`. The settings are the defaults
    /// of `deep-filter` 0.5.6: no post-filter and no attenuation limit.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        ensure!(
            path.is_file(),
            "DeepFilterNet model not found: {}",
            path.display()
        );
        let runtime = RuntimeParams::default_with_ch(1)
            .with_atten_lim(100.0)
            .with_thresholds(-15.0, 35.0, 35.0)
            .with_mask_reduce(ReduceMask::MAX);
        // libDF panics on some malformed archives; report those as errors.
        let model = catch_unwind(AssertUnwindSafe(|| {
            DfParams::new(path).and_then(|params| DfTract::new(params, &runtime))
        }))
        .map_err(|_| eyre!("Cannot read the DeepFilterNet model: the archive is malformed"))?
        .map_err(|error| eyre!("Cannot read the DeepFilterNet model: {error:#}"))?;
        ensure!(
            model.sr == DEEP_FILTER_RATE as usize && model.ch == 1,
            "Unsupported DeepFilterNet model: {} Hz, {} channels",
            model.sr,
            model.ch
        );
        let hop = model.hop_size;
        let delay = model.fft_size - hop + model.lookahead * hop;
        Ok(Self {
            fresh: model.clone(),
            model,
            hop,
            delay,
            input: vec![0.0; hop],
            filled: 0,
            output: vec![0.0; hop],
            skip: delay,
            owed: 0,
            scheduling: WorkerScheduling::OsDefault,
        })
    }

    /// OS scheduling for the thread that runs inference ([`LiveWorker`](crate::live::LiveWorker)
    /// registers it). Configure on a control thread.
    pub fn set_worker_scheduling(&mut self, scheduling: WorkerScheduling) {
        self.scheduling = scheduling;
    }

    pub fn sample_rate(&self) -> u32 {
        DEEP_FILTER_RATE
    }

    /// Samples consumed and produced per model step (10 ms).
    pub fn frame(&self) -> usize {
        self.hop
    }

    /// Delay between an input sample and its enhanced output, in samples.
    pub fn latency(&self) -> usize {
        self.delay
    }

    /// Clears all stream state; the next sample starts a new stream.
    pub fn reset(&mut self) {
        self.model.clone_from(&self.fresh);
        self.filled = 0;
        self.skip = self.delay;
        self.owed = 0;
    }

    /// Run a few frames before audio starts, then return to a fresh stream. Call on a control
    /// thread.
    pub fn prepare(&mut self) -> Result<()> {
        self.reset();
        // Quiet noise, because frames of digital silence skip the model.
        let mut seed = 0x9e37_79b9u32;
        let noise: Vec<f32> = (0..self.hop * 4)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                ((seed >> 8) as f32 / 16_777_216.0 - 0.5) * 0.01
            })
            .collect();
        let mut discarded = Vec::with_capacity(noise.len());
        let result = self.process_into(&noise, &mut discarded);
        self.reset();
        result
    }

    /// Append completed samples to a caller-owned buffer. Inference allocates: run it on a
    /// worker, not in an audio callback.
    pub fn process_into(&mut self, input: &[f32], out: &mut Vec<f32>) -> Result<()> {
        let start = out.len();
        let mut rest = input;
        while !rest.is_empty() {
            let take = (self.hop - self.filled).min(rest.len());
            self.input[self.filled..self.filled + take].copy_from_slice(&rest[..take]);
            self.filled += take;
            rest = &rest[take..];
            if self.filled == self.hop {
                self.step(out)?;
            }
        }
        self.owed += input.len();
        self.owed -= out.len() - start;
        Ok(())
    }

    /// Pushes silence through the model so every sample given so far comes out, then resets.
    pub fn flush(&mut self) -> Result<Vec<f32>> {
        let owed = self.owed;
        let silence = vec![0f32; self.hop];
        let mut out = Vec::with_capacity(owed + self.hop);
        while out.len() < owed {
            self.process_into(&silence, &mut out)?;
        }
        out.truncate(owed);
        self.reset();
        Ok(out)
    }

    fn step(&mut self, out: &mut Vec<f32>) -> Result<()> {
        self.filled = 0;
        let noisy = ArrayView2::from_shape((1, self.hop), &self.input[..])?;
        let enhanced = ArrayViewMut2::from_shape((1, self.hop), &mut self.output[..])?;
        self.model
            .process(noisy, enhanced)
            .map_err(|error| eyre!("DeepFilterNet failed: {error:#}"))?;
        let skip = self.skip.min(self.hop);
        self.skip -= skip;
        out.extend_from_slice(&self.output[skip..]);
        Ok(())
    }
}

// SAFETY: tract 0.19 is not `Send` for two reasons. It keeps intermediate tensors in `Rc`, and
// its op states are trait objects without a `Send` bound. In this model the op states are plain
// tensors (convolution and GRU history). Every `Rc` is owned by `model` or `fresh`, and both are
// owned by this struct. Nothing hands a clone out, and tract keeps no thread-local references to
// them. So a move takes every reference to a count with it, and only the owning thread touches a
// count. The struct is not `Sync`.
unsafe impl Send for DeepFilter {}

impl Enhancer for DeepFilter {
    fn sample_rate(&self) -> u32 {
        DeepFilter::sample_rate(self)
    }
    fn frame(&self) -> usize {
        DeepFilter::frame(self)
    }
    fn latency(&self) -> usize {
        DeepFilter::latency(self)
    }
    fn reset(&mut self) {
        DeepFilter::reset(self)
    }
    fn prepare(&mut self) -> Result<()> {
        DeepFilter::prepare(self)
    }
    fn process_into(&mut self, input: &[f32], out: &mut Vec<f32>) -> Result<()> {
        DeepFilter::process_into(self, input, out)
    }
    fn flush(&mut self) -> Result<Vec<f32>> {
        DeepFilter::flush(self)
    }
    fn worker_scheduling(&self) -> WorkerScheduling {
        self.scheduling
    }
}
