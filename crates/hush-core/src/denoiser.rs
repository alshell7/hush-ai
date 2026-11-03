//! Samples in, enhanced samples out: streaming STFT, [`Streaming`], inverse STFT.

use crate::audio::{hann_window, stft_sizes};
use crate::Streaming;
use eyre::{eyre, Result};
use realfft::num_complex::Complex;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use std::path::Path;
use std::sync::Arc;

/// Settings for [`Denoiser::with_config`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    /// Mamba layers, 3–12. Fewer is faster.
    pub layers: usize,
    /// Mono sample rate, one of [`audio::SAMPLE_RATES`](crate::audio::SAMPLE_RATES).
    pub sample_rate: u32,
    /// Future frames the model waits for, 0–2. Each one adds a frame (20 ms) of latency.
    pub look_ahead: usize,
}

impl Default for Config {
    /// 8 layers, 16 kHz, no look-ahead.
    fn default() -> Self {
        Self {
            layers: 8,
            sample_rate: 16000,
            look_ahead: 0,
        }
    }
}

/// Streaming speech enhancement for mono audio.
///
/// Feed any number of samples to [`process`](Self::process); it returns the enhanced samples that
/// are complete so far, in blocks of [`frame`](Self::frame) samples. Output sample `i` is the
/// enhanced input sample `i`, delayed by [`latency`](Self::latency) samples. Call
/// [`flush`](Self::flush) at the end of a stream to get the remaining samples.
pub struct Denoiser {
    model: Streaming,
    config: Config,
    n_fft: usize,
    hop: usize,
    window: Vec<f32>,
    /// Overlap-add normalization for each output position: `Σ w²(j + k·hop)`.
    envelope: Vec<f32>,
    fft: Arc<dyn RealToComplex<f32>>,
    ifft: Arc<dyn ComplexToReal<f32>>,
    /// Input for the next frame; the first `filled` samples are set.
    history: Vec<f32>,
    filled: usize,
    /// Overlap-add accumulator, starting at the first output position not yet emitted.
    overlap: Vec<f32>,
    /// Output positions still to drop: the first frame is centred on sample 0.
    skip: usize,
    /// Input samples not yet returned.
    owed: usize,
    time: Vec<f32>,
    spectrum: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    mag: Vec<f32>,
    pha: Vec<f32>,
    /// Optional per-instance pool; the default uses Rayon's global pool.
    pool: Option<rayon::ThreadPool>,
    scheduling: crate::WorkerScheduling,
    /// Set by `set_automatic_worker_threads`: add a worker when frames get slow.
    automatic: Option<Automatic>,
}

#[derive(Clone, Copy)]
struct Automatic {
    placement: crate::WorkerPlacement,
    limit: usize,
    /// Model time and frames in the current window.
    busy: f64,
    frames: u32,
    /// Load of the window before the last added worker, until the next check.
    before: Option<f64>,
    /// Count chosen at start; workers added later are removed after a quiet period.
    min: usize,
    /// Consecutive quiet windows with more than `min` workers.
    quiet: u32,
}

/// Frames per load check in automatic mode (one second).
const AUTOMATIC_WINDOW: u32 = 50;
/// Automatic mode adds a worker when the model uses more of the frame time than this.
const AUTOMATIC_HIGH_LOAD: f64 = 0.75;
/// Automatic mode removes an added worker after this many windows below the low load.
const AUTOMATIC_QUIET_WINDOWS: u32 = 30;
const AUTOMATIC_LOW_LOAD: f64 = 0.4;
/// A worker must cut frame time by at least 10% to be worth its CPU.
const AUTOMATIC_MIN_GAIN: f64 = 0.9;

impl Denoiser {
    /// Loads the model for 16 kHz audio with `layers` Mamba layers (3–12) and no look-ahead.
    ///
    /// Fewer layers are faster, because each frame passes through every layer. The model was
    /// trained to stop after any of these layer counts, so every choice gives usable output.
    pub fn load(path: impl AsRef<Path>, layers: usize) -> Result<Self> {
        Self::with_config(
            path,
            Config {
                layers,
                ..Config::default()
            },
        )
    }

    /// Loads the model with any sample rate and look-ahead.
    pub fn with_config(path: impl AsRef<Path>, config: Config) -> Result<Self> {
        let (n_fft, hop) = stft_sizes(config.sample_rate)?;
        let model = Streaming::load(path, config.layers, config.look_ahead)?;
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(n_fft);
        let ifft = planner.plan_fft_inverse(n_fft);
        let window = hann_window(n_fft);
        let envelope = (0..hop)
            .map(|j| (j..n_fft).step_by(hop).map(|i| window[i] * window[i]).sum())
            .collect();
        let scratch_len = fft.get_scratch_len().max(ifft.get_scratch_len());
        let bins = n_fft / 2 + 1;
        Ok(Self {
            model,
            config,
            n_fft,
            hop,
            window,
            envelope,
            history: vec![0f32; n_fft],
            filled: n_fft / 2,
            overlap: vec![0f32; n_fft],
            skip: n_fft / 2,
            owed: 0,
            time: vec![0f32; n_fft],
            spectrum: vec![Complex::default(); bins],
            scratch: vec![Complex::default(); scratch_len],
            mag: vec![0f32; bins],
            pha: vec![0f32; bins],
            pool: None,
            scheduling: crate::WorkerScheduling::OsDefault,
            automatic: None,
            fft,
            ifft,
        })
    }

    pub fn config(&self) -> Config {
        self.config
    }

    /// Use a dedicated Rayon pool for inference. Configure this outside the audio callback.
    /// A value of zero restores the global Rayon pool (`RAYON_NUM_THREADS` may configure it).
    pub fn set_worker_threads(&mut self, threads: usize) -> Result<()> {
        self.set_worker_threads_with_placement(threads, crate::WorkerPlacement::Unrestricted)
    }

    /// Configure a dedicated inference pool and optional OS placement on a control thread.
    /// With `PhysicalCores`, zero selects the available physical-core count. A positive count
    /// cannot exceed it. This experimental policy currently requires 64-bit Windows and a
    /// single processor group; unsupported configurations return an error. Other application
    /// threads and process affinity are unchanged. Failure preserves the previous pool.
    /// Changing a pool does not reset audio/model state.
    pub fn set_worker_threads_with_placement(
        &mut self,
        threads: usize,
        placement: crate::WorkerPlacement,
    ) -> Result<()> {
        self.set_worker_threads_with_options(threads, placement, crate::WorkerScheduling::OsDefault)
    }

    /// Configure worker count, placement and optional OS scheduling on a control thread.
    /// Windows Audio scheduling uses a dedicated unrestricted pool, including when zero
    /// selects Rayon's default worker count. Other policies keep the zero-count behavior
    /// described by [`Self::set_worker_threads_with_placement`]. Failure preserves the
    /// previous pool and policy. Existing two-argument setters restore OS-default scheduling.
    /// Direct processing loops may also register their coordinator with
    /// [`crate::WorkerScheduling::enter_current_thread`]; `LiveWorker` handles it automatically.
    pub fn set_worker_threads_with_options(
        &mut self,
        threads: usize,
        placement: crate::WorkerPlacement,
        scheduling: crate::WorkerScheduling,
    ) -> Result<()> {
        self.pool = crate::workers::build(threads, placement, scheduling)?;
        self.scheduling = scheduling;
        self.automatic = None;
        Ok(())
    }

    /// Use the fewest dedicated workers that finish a frame in at most 60% of the frame time.
    /// Tries 1, 2, 3, … workers up to about one per physical core, each for about 30 frames.
    /// It stops when one more worker cuts the frame time by less than 10%, because each
    /// worker adds CPU use. Returns the chosen count.
    /// Later, if the model uses more than 75% of the frame time over one second (for example
    /// because other programs got busy), processing adds one worker. If that worker does not
    /// cut the load by 10%, it is removed again and no more are added. Added workers are
    /// removed again after 30 seconds below 40% load.
    /// Call on a control thread before starting audio. The stream is reset.
    pub fn set_automatic_worker_threads(
        &mut self,
        placement: crate::WorkerPlacement,
        scheduling: crate::WorkerScheduling,
    ) -> Result<usize> {
        let limit = std::thread::available_parallelism().map_or(1, |n| (n.get() / 2).max(1));
        let target = self.hop as f64 / self.config.sample_rate as f64 * 0.6;
        let mut seed = 0x9e37_79b9u32;
        let input: Vec<f32> = (0..self.hop * 32)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                ((seed >> 8) as f32 / 16_777_216.0 - 0.5) * 0.1
            })
            .collect();
        let mut out = Vec::with_capacity(self.hop * 2);
        let mut chosen = (f64::INFINITY, 1);
        for threads in 1..=limit {
            self.set_worker_threads_with_options(threads, placement, scheduling)?;
            self.reset();
            let mut times = Vec::with_capacity(32);
            for chunk in input.chunks_exact(self.hop) {
                out.clear();
                let start = std::time::Instant::now();
                self.process_into(chunk, &mut out)?;
                times.push(start.elapsed().as_secs_f64());
            }
            // Skip warm-up frames, then take the 90th percentile.
            let times = &mut times[8..];
            times.sort_by(f64::total_cmp);
            let p90 = times[times.len() * 9 / 10];
            if p90 > chosen.0 * AUTOMATIC_MIN_GAIN {
                break;
            }
            chosen = (p90, threads);
            if p90 <= target {
                break;
            }
        }
        self.set_worker_threads_with_options(chosen.1, placement, scheduling)?;
        self.automatic = Some(Automatic {
            placement,
            limit,
            busy: 0.0,
            frames: 0,
            before: None,
            min: chosen.1,
            quiet: 0,
        });
        self.reset();
        Ok(chosen.1)
    }

    /// Selected worker scheduling policy.
    pub fn worker_scheduling(&self) -> crate::WorkerScheduling {
        self.scheduling
    }

    /// Number of inference workers in the configured dedicated or global pool.
    pub fn worker_threads(&self) -> usize {
        self.pool.as_ref().map_or_else(
            rayon::current_num_threads,
            rayon::ThreadPool::current_num_threads,
        )
    }

    pub fn sample_rate(&self) -> u32 {
        self.config.sample_rate
    }

    /// Samples consumed and produced per model step (20 ms).
    pub fn frame(&self) -> usize {
        self.hop
    }

    /// Delay between an input sample and its enhanced output, in samples: one frame, plus one
    /// more per look-ahead frame.
    pub fn latency(&self) -> usize {
        self.n_fft - self.hop + self.config.look_ahead * self.hop
    }

    /// Clears all stream state; the next sample starts a new stream.
    pub fn reset(&mut self) {
        self.model.reset();
        self.history.fill(0.0);
        self.filled = self.n_fft / 2;
        self.overlap.fill(0.0);
        self.skip = self.n_fft / 2;
        self.owed = 0;
    }

    /// Initialize inference buffers before starting audio, then return to a fresh stream.
    /// Call on a control thread after configuring the worker pool and precision. This performs
    /// several silent frames; it is not suitable for an audio callback. Transient inference
    /// allocations still occur, but temporal state and convolution history retain capacity.
    pub fn prepare(&mut self) -> Result<()> {
        self.reset();
        let silence = vec![0.0; self.hop];
        let mut discarded = Vec::with_capacity(self.hop);
        let result = (|| {
            for _ in 0..self.config.look_ahead + 2 {
                discarded.clear();
                self.process_into(&silence, &mut discarded)?;
            }
            Ok(())
        })();
        self.reset();
        result
    }

    /// Enhances `input` and returns every output sample that is complete.
    pub fn process(&mut self, input: &[f32]) -> Result<Vec<f32>> {
        let mut out = Vec::with_capacity(input.len() + self.hop);
        self.process_into(input, &mut out)?;
        Ok(out)
    }

    /// Append completed samples to a caller-owned buffer. Reuse its capacity between calls to
    /// avoid the output allocation made by [`process`](Self::process).
    /// Inference itself still allocates and must run on a worker, not an audio callback.
    pub fn process_into(&mut self, input: &[f32], out: &mut Vec<f32>) -> Result<()> {
        let start = out.len();
        for &sample in input {
            self.history[self.filled] = sample;
            self.filled += 1;
            if self.filled == self.n_fft {
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
            out.extend(self.process(&silence)?);
        }
        out.truncate(owed);
        self.reset();
        Ok(out)
    }

    /// Automatic mode: add one worker after a window in which the model was too slow.
    fn check_load(&mut self, seconds: f64) {
        let threads = self.worker_threads();
        let Some(auto) = self.automatic.as_mut() else {
            return;
        };
        auto.busy += seconds;
        auto.frames += 1;
        if auto.frames < AUTOMATIC_WINDOW {
            return;
        }
        let budget = f64::from(AUTOMATIC_WINDOW) * self.hop as f64 / self.config.sample_rate as f64;
        let load = auto.busy / budget;
        auto.busy = 0.0;
        auto.frames = 0;
        let next = match auto.before.take() {
            // The last added worker did not help: remove it and stop adding.
            Some(before) if load > before * AUTOMATIC_MIN_GAIN => {
                auto.limit = threads - 1;
                threads - 1
            }
            _ if load > AUTOMATIC_HIGH_LOAD && threads < auto.limit => {
                auto.before = Some(load);
                auto.quiet = 0;
                threads + 1
            }
            _ if load < AUTOMATIC_LOW_LOAD && threads > auto.min => {
                auto.quiet += 1;
                if auto.quiet < AUTOMATIC_QUIET_WINDOWS {
                    return;
                }
                auto.quiet = 0;
                threads - 1
            }
            _ => {
                auto.quiet = 0;
                return;
            }
        };
        // Building a pool starts threads: rare, and never inside an audio callback.
        // If it fails, keep the current pool and stop changing it.
        match crate::workers::build(next, auto.placement, self.scheduling) {
            Ok(pool) => self.pool = pool,
            Err(_) => self.automatic = None,
        }
    }

    /// One STFT frame from `history`: analyse, enhance, overlap-add, emit one finished block.
    fn step(&mut self, out: &mut Vec<f32>) -> Result<()> {
        for ((t, x), w) in self.time.iter_mut().zip(&self.history).zip(&self.window) {
            *t = x * w;
        }
        self.history.copy_within(self.hop.., 0);
        self.filled = self.n_fft - self.hop;
        self.fft
            .process_with_scratch(&mut self.time, &mut self.spectrum, &mut self.scratch)
            .map_err(|e| eyre!("FFT failed: {e}"))?;
        for ((c, m), p) in self.spectrum.iter().zip(&mut self.mag).zip(&mut self.pha) {
            *m = c.norm().ln_1p();
            *p = c.im.atan2(c.re);
        }

        let started = self.automatic.is_some().then(std::time::Instant::now);
        let result = if let Some(pool) = &self.pool {
            pool.install(|| self.model.step(&self.mag, &self.pha))
        } else {
            self.model.step(&self.mag, &self.pha)
        };
        if let Some(started) = started {
            self.check_load(started.elapsed().as_secs_f64());
        }
        let Some((mut mag, pha)) = result else {
            return Ok(());
        };
        // Frames where most bins come out as zero are cleared, as NVIDIA's inference does.
        if mag.iter().filter(|m| **m <= 0.0).count() * 2 > mag.len() {
            mag.fill(0.0);
        }
        for ((c, m), p) in self.spectrum.iter_mut().zip(&mag).zip(&pha) {
            *c = Complex::from_polar(m.max(0.0).exp_m1(), *p);
        }
        let last = self.spectrum.len() - 1;
        self.spectrum[0].im = 0.0;
        self.spectrum[last].im = 0.0;
        self.ifft
            .process_with_scratch(&mut self.spectrum, &mut self.time, &mut self.scratch)
            .map_err(|e| eyre!("IFFT failed: {e}"))?;

        let norm = 1.0 / self.n_fft as f32;
        for ((o, t), w) in self.overlap.iter_mut().zip(&self.time).zip(&self.window) {
            *o += t * norm * w;
        }
        let hop = self.hop;
        let skip = self.skip.min(hop);
        out.extend(
            self.overlap[skip..hop]
                .iter()
                .zip(&self.envelope[skip..])
                .map(|(o, e)| o / e),
        );
        self.skip -= skip;
        self.overlap.copy_within(hop.., 0);
        let n_fft = self.n_fft;
        self.overlap[n_fft - hop..].fill(0.0);
        Ok(())
    }
}
