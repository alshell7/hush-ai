//! Bounded, lock-free audio callback adapters and a dedicated inference worker.
//!
//! Callbacks use mono f32 samples at their configured device rates. Optional rate conversion
//! runs on the inference worker, with continuous filter history. `push` and `read`
//! do not allocate, lock, perform I/O, or run inference. Start and stop on a control thread.

use crate::Denoiser;
use eyre::{ensure, Result};
use rtrb::{Consumer, Producer, RingBuffer};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
struct InputSample {
    sequence: u64,
    value: f32,
}
#[derive(Clone, Copy, Debug)]
struct OutputSample {
    epoch: u64,
    value: f32,
}

#[derive(Default)]
struct Counters {
    input_dropped: AtomicU64,
    output_dropped: AtomicU64,
    output_underrun: AtomicU64,
    output_buffering: AtomicU64,
    clock_correction_ppm: AtomicI64,
    playing: AtomicBool,
    discontinuities: AtomicU64,
    frames_processed: AtomicU64,
    epoch: AtomicU64,
    stopped: AtomicBool,
    failed: AtomicBool,
    busy_ns: AtomicU64,
    peak_frame_ns: AtomicU64,
    // Non-negative f32 bit patterns order like the values, so fetch_max works.
    input_peak: AtomicU32,
    output_peak: AtomicU32,
    original: AtomicBool,
}

/// Monitoring handle for a UI thread. Lock-free; safe to read while audio runs.
#[derive(Clone)]
pub struct LiveMeters(Arc<Counters>);

/// One meter reading. Peaks cover the time since the previous [`LiveMeters::take`].
#[derive(Debug, Clone, Copy, Default)]
pub struct MeterReading {
    /// Total worker time spent on frames since start, in nanoseconds.
    pub busy_ns: u64,
    pub peak_frame_ns: u64,
    pub input_peak: f32,
    pub output_peak: f32,
    pub stats: LiveStats,
}

impl LiveMeters {
    /// Read counters and reset the peak values.
    pub fn take(&self) -> MeterReading {
        let c = &self.0;
        MeterReading {
            busy_ns: c.busy_ns.load(Ordering::Relaxed),
            peak_frame_ns: c.peak_frame_ns.swap(0, Ordering::Relaxed),
            input_peak: f32::from_bits(c.input_peak.swap(0, Ordering::Relaxed)),
            output_peak: f32::from_bits(c.output_peak.swap(0, Ordering::Relaxed)),
            stats: snapshot(c),
        }
    }

    /// Play the unprocessed input instead of the enhanced output, for A/B listening.
    /// The model keeps running and the input is delayed by the model latency, so both
    /// signals stay aligned and switching back is seamless. No effect in bypass mode.
    pub fn set_original(&self, original: bool) {
        self.0.original.store(original, Ordering::Relaxed);
    }
}

fn peak(samples: &[f32]) -> u32 {
    samples.iter().fold(0f32, |m, v| m.max(v.abs())).to_bits()
}

/// Cumulative callback and worker counters since the worker was created.
#[derive(Debug, Clone, Copy, Default)]
pub struct LiveStats {
    pub input_dropped_samples: u64,
    pub output_dropped_samples: u64,
    pub output_underrun_samples: u64,
    /// Silence while filling/recovering a clocked output queue, separate from underruns.
    pub output_buffering_samples: u64,
    /// Latest output resampling correction, rounded to parts per million.
    pub clock_correction_ppm: i64,
    pub discontinuities: u64,
    pub frames_processed: u64,
    pub worker_failed: bool,
}

fn snapshot(c: &Counters) -> LiveStats {
    LiveStats {
        input_dropped_samples: c.input_dropped.load(Ordering::Relaxed),
        output_dropped_samples: c.output_dropped.load(Ordering::Relaxed),
        output_underrun_samples: c.output_underrun.load(Ordering::Relaxed),
        output_buffering_samples: c.output_buffering.load(Ordering::Relaxed),
        clock_correction_ppm: c.clock_correction_ppm.load(Ordering::Relaxed),
        discontinuities: c.discontinuities.load(Ordering::Relaxed),
        frames_processed: c.frames_processed.load(Ordering::Acquire),
        worker_failed: c.failed.load(Ordering::Relaxed),
    }
}

/// Owned by one microphone callback. Each call drops incoming samples that do not fit.
pub struct LiveInput {
    producer: Producer<InputSample>,
    next: u64,
    counters: Arc<Counters>,
}

impl LiveInput {
    /// Queue samples without blocking. Returns the number accepted.
    pub fn push(&mut self, samples: &[f32]) -> usize {
        let mut accepted = 0;
        for &value in samples {
            let item = InputSample {
                sequence: self.next,
                value,
            };
            self.next = self.next.wrapping_add(1);
            if self.producer.push(item).is_ok() {
                accepted += 1;
            }
        }
        self.counters
            .input_dropped
            .fetch_add((samples.len() - accepted) as u64, Ordering::Relaxed);
        self.counters
            .input_peak
            .fetch_max(peak(samples), Ordering::Relaxed);
        accepted
    }

    /// Marks a device restart or other explicit discontinuity. The worker resets its state when
    /// it receives the next sample; call only from the thread that owns this input handle.
    pub fn discontinuity(&mut self) {
        self.next = self.next.wrapping_add(1);
    }

    pub fn stats(&self) -> LiveStats {
        snapshot(&self.counters)
    }
}

/// Owned by one output callback. Missing or stale samples are faded to silence.
pub struct LiveOutput {
    consumer: Consumer<OutputSample>,
    counters: Arc<Counters>,
    epoch: u64,
    missing: bool,
    fade_len: usize,
    remaining: usize,
    fade_from: f32,
    last: f32,
    prefill: usize,
    prefilling: bool,
}

impl LiveOutput {
    /// Fill the entire buffer and return the number of model samples consumed. This never waits.
    pub fn read(&mut self, output: &mut [f32]) -> usize {
        let consumed = self.fill(output);
        self.counters
            .output_peak
            .fetch_max(peak(output), Ordering::Relaxed);
        consumed
    }

    fn fill(&mut self, output: &mut [f32]) -> usize {
        let mut consumed = 0;
        let mut missing = 0;
        let mut buffering = 0;
        let epoch = self.counters.epoch.load(Ordering::Acquire);
        if epoch != self.epoch {
            self.prefilling = self.prefill > 0;
            if self.prefilling {
                self.epoch = epoch;
                self.fade_from = self.last;
                self.remaining = self.fade_len;
            }
        }
        // Old output must not satisfy the prefill threshold after a state reset.
        while self.consumer.peek().is_ok_and(|s| s.epoch != epoch) {
            let _ = self.consumer.pop();
        }
        if self.prefilling && self.consumer.slots() >= self.prefill {
            self.prefilling = false;
        }
        self.counters
            .playing
            .store(!self.prefilling, Ordering::Release);
        for dst in output {
            let epoch = self.counters.epoch.load(Ordering::Acquire);
            if epoch != self.epoch && self.prefill > 0 {
                self.prefilling = true;
                self.counters.playing.store(false, Ordering::Release);
            }
            let mut next = None;
            if !self.prefilling {
                while let Ok(sample) = self.consumer.pop() {
                    if sample.epoch == epoch {
                        next = Some(sample.value);
                        break;
                    }
                }
            }
            let now_missing = next.is_none();
            if epoch != self.epoch || now_missing != self.missing {
                self.epoch = epoch;
                self.missing = now_missing;
                self.fade_from = self.last;
                self.remaining = self.fade_len;
            }
            let target = if let Some(value) = next {
                consumed += 1;
                value
            } else {
                if self.prefilling {
                    buffering += 1;
                } else {
                    missing += 1;
                }
                0.0
            };
            *dst = if self.remaining > 0 {
                let old = self.remaining as f32 / self.fade_len as f32;
                self.remaining -= 1;
                self.fade_from * old + target * (1.0 - old)
            } else {
                target
            };
            self.last = *dst;
        }
        self.counters
            .output_underrun
            .fetch_add(missing, Ordering::Relaxed);
        self.counters
            .output_buffering
            .fetch_add(buffering, Ordering::Relaxed);
        if missing > 0 && self.prefill > 0 {
            self.prefilling = true;
            self.counters.playing.store(false, Ordering::Release);
        }
        consumed
    }

    /// Queued count is approximate and can include samples invalidated by a reset.
    pub fn available(&self) -> usize {
        self.consumer.slots()
    }
    pub fn stats(&self) -> LiveStats {
        snapshot(&self.counters)
    }
}

/// Owns the inference thread. Shutdown may block while a frame finishes; use a control thread.
pub struct LiveWorker {
    counters: Arc<Counters>,
    thread: Option<JoinHandle<()>>,
}

impl LiveWorker {
    /// Start with bounded input/output capacities in samples. A few frames of buffering provides
    /// scheduling headroom without allowing unbounded latency. The model is reset on input gaps.
    pub fn start(
        denoiser: Denoiser,
        input_capacity: usize,
        output_capacity: usize,
    ) -> Result<(Self, LiveInput, LiveOutput)> {
        Self::start_with_bypass(denoiser, input_capacity, output_capacity, false)
    }

    /// As [`start`](Self::start), optionally passing captured frames through unchanged. The mode
    /// is fixed for the lifetime of the worker; stop and create a new worker to change it.
    pub fn start_with_bypass(
        denoiser: Denoiser,
        input_capacity: usize,
        output_capacity: usize,
        bypass: bool,
    ) -> Result<(Self, LiveInput, LiveOutput)> {
        let rate = denoiser.sample_rate();
        Self::start_at_rates(
            denoiser,
            rate,
            rate,
            input_capacity,
            output_capacity,
            bypass,
        )
    }

    /// Run devices at their supported rates while preserving the selected model profile.
    /// Capacities and input/output sample counters are in the respective device-rate samples;
    /// frames_processed counts model frames (device-rate blocks when bypassed). Conversion and
    /// inference run outside callbacks. Each unequal-rate conversion adds 16 source samples of
    /// filter delay.
    /// This handles fixed nominal rates, not drift between independent hardware clocks.
    pub fn start_at_rates(
        denoiser: Denoiser,
        input_rate: u32,
        output_rate: u32,
        input_capacity: usize,
        output_capacity: usize,
        bypass: bool,
    ) -> Result<(Self, LiveInput, LiveOutput)> {
        Self::start_impl(
            denoiser,
            input_rate,
            output_rate,
            input_capacity,
            output_capacity,
            bypass,
            0,
        )
    }

    /// Lock-free meters and the A/B switch for a monitoring UI.
    pub fn meters(&self) -> LiveMeters {
        LiveMeters(self.counters.clone())
    }

    /// Experimental live device mode: prefill two output frames and track independent clocks
    /// with a bounded +/-1000 ppm output resampler. Requires at least four output frames of
    /// capacity. Adds 16 model-rate samples of filter delay even at equal nominal rates.
    /// Fixed-rate APIs remain deterministic and do not enable clock tracking.
    pub fn start_clocked(
        denoiser: Denoiser,
        input_rate: u32,
        output_rate: u32,
        input_capacity: usize,
        output_capacity: usize,
        bypass: bool,
    ) -> Result<(Self, LiveInput, LiveOutput)> {
        Self::start_clocked_with_prefill(
            denoiser,
            input_rate,
            output_rate,
            input_capacity,
            output_capacity,
            bypass,
            2,
        )
    }

    /// As [`start_clocked`](Self::start_clocked) with 1..=3 output frames of prefill.
    /// Each frame of prefill adds one frame (20 ms) of latency and of scheduling headroom.
    pub fn start_clocked_with_prefill(
        denoiser: Denoiser,
        input_rate: u32,
        output_rate: u32,
        input_capacity: usize,
        output_capacity: usize,
        bypass: bool,
        prefill_frames: usize,
    ) -> Result<(Self, LiveInput, LiveOutput)> {
        ensure!(
            (1..=3).contains(&prefill_frames),
            "prefill must be 1 to 3 frames"
        );
        Self::start_impl(
            denoiser,
            input_rate,
            output_rate,
            input_capacity,
            output_capacity,
            bypass,
            prefill_frames,
        )
    }

    /// `prefill_frames` > 0 enables clocked output with that much prefill.
    fn start_impl(
        mut denoiser: Denoiser,
        input_rate: u32,
        output_rate: u32,
        input_capacity: usize,
        output_capacity: usize,
        bypass: bool,
        prefill_frames: usize,
    ) -> Result<(Self, LiveInput, LiveOutput)> {
        let clocked = prefill_frames > 0;
        // Bypass preserves captured audio; do not downsample it through the model's rate.
        let model_rate = if bypass {
            input_rate
        } else {
            denoiser.sample_rate()
        };
        let frame = if bypass {
            input_rate.div_ceil(50) as usize
        } else {
            denoiser.frame()
        };
        let mut input_resampler = crate::resample::StreamingMono::new(input_rate, model_rate)?;
        let mut output_resampler = if clocked {
            crate::resample::StreamingMono::adaptive(model_rate, output_rate)?
        } else {
            crate::resample::StreamingMono::new(model_rate, output_rate)?
        };
        let input_frame = (frame as u64 * input_rate as u64).div_ceil(model_rate as u64) as usize;
        let output_frame = (frame as u64 * output_rate as u64).div_ceil(model_rate as u64) as usize;
        ensure!(
            input_capacity >= input_frame,
            "input capacity must hold at least one device-rate frame"
        );
        ensure!(
            output_capacity >= output_frame,
            "output capacity must hold at least one device-rate frame"
        );
        ensure!(
            !clocked || output_capacity >= output_frame * (prefill_frames + 2),
            "clocked output capacity must hold prefill plus two device-rate frames"
        );
        let prefill = output_frame * prefill_frames;
        let mut drift = clocked.then(|| {
            crate::live_drift::DriftController::new(
                prefill as f64 + output_frame as f64 * 0.5,
                output_rate,
            )
        });
        let fade_len = (output_rate as usize / 200).max(1); // 5 ms at the output device
        if bypass {
            denoiser.reset();
        } else {
            denoiser.prepare()?;
        }
        let (input_tx, mut input_rx) = RingBuffer::<InputSample>::new(input_capacity);
        let (mut output_tx, output_rx) = RingBuffer::<OutputSample>::new(output_capacity);
        let counters = Arc::new(Counters::default());
        let shared = counters.clone();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let thread = thread::Builder::new()
            .name("hush-inference".into())
            .spawn(move || {
                crate::audio_thread::interactive();
                let _registration = {
                    let result = denoiser.worker_scheduling().enter_current_thread();
                    let status = result.as_ref().map(|_| ()).map_err(ToString::to_string);
                    let _ = started_tx.send(status);
                    match result {
                        Ok(guard) => guard,
                        Err(_) => return,
                    }
                };
                let mut frame_in =
                    Vec::with_capacity(frame + (model_rate as usize).div_ceil(input_rate as usize));
                let mut frame_out = Vec::with_capacity(frame * 2);
                let mut device_out = if model_rate == output_rate && !clocked {
                    Vec::new()
                } else {
                    Vec::with_capacity(output_frame * 3)
                };
                // A/B: input delayed by the model latency, and a 10 ms crossfade gain.
                let mut dry = VecDeque::with_capacity(if bypass {
                    0
                } else {
                    denoiser.latency() + frame * 2
                });
                let fade_step = 100.0 / model_rate as f32;
                let mut dry_gain = 0f32;
                let mut expected = None;
                while !shared.stopped.load(Ordering::Acquire) {
                    match input_rx.pop() {
                        Ok(sample) => {
                            if expected.is_some_and(|value| value != sample.sequence) {
                                denoiser.reset();
                                input_resampler.reset();
                                output_resampler.reset();
                                if let Some(controller) = drift.as_mut() {
                                    controller.reset();
                                    shared.playing.store(false, Ordering::Release);
                                    shared.clock_correction_ppm.store(0, Ordering::Relaxed);
                                }
                                frame_in.clear();
                                dry.clear();
                                shared.discontinuities.fetch_add(1, Ordering::Relaxed);
                                shared.epoch.fetch_add(1, Ordering::AcqRel);
                            }
                            expected = Some(sample.sequence.wrapping_add(1));
                            input_resampler.push(sample.value, |value| frame_in.push(value));
                            while frame_in.len() >= frame {
                                let started = Instant::now();
                                frame_out.clear();
                                if bypass {
                                    frame_out.extend_from_slice(&frame_in[..frame]);
                                } else if denoiser
                                    .process_into(&frame_in[..frame], &mut frame_out)
                                    .is_err()
                                {
                                    shared.failed.store(true, Ordering::Relaxed);
                                    return;
                                } else {
                                    dry.extend(&frame_in[..frame]);
                                    let target = if shared.original.load(Ordering::Relaxed) {
                                        1.0
                                    } else {
                                        0.0
                                    };
                                    if dry_gain == 0.0 && target == 0.0 {
                                        // Keep enhanced output bit-identical when A/B is unused.
                                        dry.drain(..frame_out.len().min(dry.len()));
                                    } else {
                                        for wet in frame_out.iter_mut() {
                                            let input = dry.pop_front().unwrap_or(0.0);
                                            dry_gain = if dry_gain < target {
                                                (dry_gain + fade_step).min(target)
                                            } else {
                                                (dry_gain - fade_step).max(target)
                                            };
                                            *wet += dry_gain * (input - *wet);
                                        }
                                    }
                                }
                                frame_in.drain(..frame);
                                let rendered = if model_rate == output_rate && !clocked {
                                    &frame_out
                                } else {
                                    device_out.clear();
                                    for &value in &frame_out {
                                        output_resampler
                                            .push(value, |value| device_out.push(value));
                                    }
                                    &device_out
                                };
                                let epoch = shared.epoch.load(Ordering::Acquire);
                                for (index, &value) in rendered.iter().enumerate() {
                                    if output_tx.push(OutputSample { epoch, value }).is_err() {
                                        shared.output_dropped.fetch_add(
                                            (rendered.len() - index) as u64,
                                            Ordering::Relaxed,
                                        );
                                        shared.epoch.fetch_add(1, Ordering::AcqRel);
                                        shared.playing.store(false, Ordering::Release);
                                        break;
                                    }
                                }
                                if let Some(controller) = drift.as_mut() {
                                    let ppm = if shared.playing.load(Ordering::Acquire) {
                                        controller.update(
                                            output_capacity - output_tx.slots(),
                                            output_rate,
                                            frame_out.len() as f64 / model_rate as f64,
                                        )
                                    } else {
                                        controller.reset();
                                        0.0
                                    };
                                    output_resampler.set_correction_ppm(ppm);
                                    shared
                                        .clock_correction_ppm
                                        .store(ppm.round() as i64, Ordering::Relaxed);
                                }
                                let elapsed = started.elapsed().as_nanos() as u64;
                                shared.busy_ns.fetch_add(elapsed, Ordering::Relaxed);
                                shared.peak_frame_ns.fetch_max(elapsed, Ordering::Relaxed);
                                shared.frames_processed.fetch_add(1, Ordering::Release);
                            }
                        }
                        Err(_) => thread::sleep(Duration::from_millis(1)),
                    }
                }
            })?;
        if let Err(error) = started_rx
            .recv()
            .map_err(|error| error.to_string())
            .and_then(|result| result)
        {
            counters.stopped.store(true, Ordering::Release);
            let _ = thread.join();
            eyre::bail!("Inference-thread scheduling failed: {error}");
        }
        Ok((
            Self {
                counters: counters.clone(),
                thread: Some(thread),
            },
            LiveInput {
                producer: input_tx,
                next: 0,
                counters: counters.clone(),
            },
            LiveOutput {
                consumer: output_rx,
                counters,
                epoch: 0,
                missing: true,
                fade_len,
                remaining: 0,
                fade_from: 0.0,
                last: 0.0,
                prefill,
                prefilling: prefill > 0,
            },
        ))
    }

    pub fn stats(&self) -> LiveStats {
        snapshot(&self.counters)
    }
}

impl Drop for LiveWorker {
    fn drop(&mut self) {
        self.counters.stopped.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clocked_output(capacity: usize, prefill: usize) -> (Producer<OutputSample>, LiveOutput) {
        let (producer, consumer) = RingBuffer::new(capacity);
        let counters = Arc::new(Counters::default());
        (
            producer,
            LiveOutput {
                consumer,
                counters,
                epoch: 0,
                missing: true,
                fade_len: 2,
                remaining: 0,
                fade_from: 0.0,
                last: 0.0,
                prefill,
                prefilling: true,
            },
        )
    }

    #[test]
    fn clocked_output_prefills_recovers_and_rejects_stale_prefill_after_reset() {
        let (mut producer, mut output) = clocked_output(16, 4);
        let mut samples = [9.0; 2];
        assert_eq!(output.read(&mut samples), 0);
        assert_eq!(samples, [0.0; 2]);
        assert_eq!(output.stats().output_buffering_samples, 2);
        assert_eq!(output.stats().output_underrun_samples, 0);
        for _ in 0..4 {
            producer
                .push(OutputSample {
                    epoch: 0,
                    value: 0.25,
                })
                .unwrap();
        }
        assert_eq!(output.read(&mut samples), 2);
        assert_eq!(output.read(&mut samples), 2);
        assert_eq!(samples, [0.25; 2]);
        assert_eq!(output.read(&mut samples), 0);
        assert_eq!(output.stats().output_underrun_samples, 2);
        producer
            .push(OutputSample {
                epoch: 0,
                value: 0.25,
            })
            .unwrap();
        assert_eq!(output.read(&mut samples), 0); // Wait for the full prefill after starvation.
        assert_eq!(output.available(), 1);
        for _ in 0..3 {
            producer
                .push(OutputSample {
                    epoch: 0,
                    value: 0.25,
                })
                .unwrap();
        }
        output.counters.epoch.store(1, Ordering::Release);
        producer
            .push(OutputSample {
                epoch: 1,
                value: -0.25,
            })
            .unwrap();
        assert_eq!(output.read(&mut samples), 0); // Four stale samples must not start playback.
        assert_eq!(output.available(), 1);
        for _ in 0..3 {
            producer
                .push(OutputSample {
                    epoch: 1,
                    value: -0.25,
                })
                .unwrap();
        }
        assert_eq!(output.read(&mut samples), 2);
        assert_eq!(output.read(&mut samples), 2);
        assert_eq!(samples, [-0.25; 2]);
    }

    #[test]
    fn clocked_resampler_and_queue_follow_skew_without_losses() {
        for rate in [8000u32, 48000] {
            for initial_skew in [-750.0, 750.0] {
                let frame = (rate / 50) as usize;
                let (mut producer, mut output) = clocked_output(frame * 4, frame * 2);
                let mut converter = crate::resample::StreamingMono::adaptive(16000, rate).unwrap();
                let mut controller =
                    crate::live_drift::DriftController::new(frame as f64 * 2.5, rate);
                let mut fractional = 0.0;
                let mut samples = vec![0.0; frame + 4];
                for tick in 0..15_000 {
                    // Five minutes, including a clock-direction change.
                    let skew = if tick < 7500 {
                        initial_skew
                    } else {
                        -initial_skew
                    };
                    for _ in 0..320 {
                        converter.push(0.25, |value| {
                            producer
                                .push(OutputSample { epoch: 0, value })
                                .expect("output overflow");
                        });
                    }
                    let ppm = if output.counters.playing.load(Ordering::Acquire) {
                        controller.update(frame * 4 - producer.slots(), rate, 0.020)
                    } else {
                        controller.reset();
                        0.0
                    };
                    converter.set_correction_ppm(ppm);
                    fractional += frame as f64 / (1.0 + skew * 1e-6);
                    let count = fractional.floor() as usize;
                    fractional -= count as f64;
                    // Split like a device callback; counts are not tied to model frame size.
                    for block in samples[..count].chunks_mut(137) {
                        output.read(block);
                    }
                    assert!(samples[..count].iter().all(|s| s.is_finite()));
                    if tick > 10 {
                        assert!(samples[..count].iter().all(|s| (*s - 0.25).abs() < 1e-5));
                    }
                }
                assert_eq!(output.stats().output_underrun_samples, 0);
                assert!(output.stats().output_buffering_samples > 0);
                assert!((frame..frame * 3).contains(&output.available()));
            }
        }
    }

    #[test]
    fn output_fades_on_gaps_and_discards_old_epoch() {
        let (mut producer, consumer) = RingBuffer::new(16);
        let counters = Arc::new(Counters::default());
        let mut output = LiveOutput {
            consumer,
            counters: counters.clone(),
            epoch: 0,
            missing: false,
            fade_len: 2,
            remaining: 0,
            fade_from: 0.0,
            last: 0.0,
            prefill: 0,
            prefilling: false,
        };
        for _ in 0..3 {
            producer
                .push(OutputSample {
                    epoch: 0,
                    value: 1.0,
                })
                .unwrap();
        }
        let mut first = [0.0; 2];
        assert_eq!(output.read(&mut first), 2);
        assert_eq!(first, [1.0, 1.0]);
        counters.epoch.store(1, Ordering::Release);
        producer
            .push(OutputSample {
                epoch: 1,
                value: -1.0,
            })
            .unwrap();
        producer
            .push(OutputSample {
                epoch: 1,
                value: -1.0,
            })
            .unwrap();
        let mut next = [0.0; 4];
        assert_eq!(output.read(&mut next), 2);
        assert_eq!(next, [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(output.stats().output_underrun_samples, 2);
    }
}
