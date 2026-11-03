//! Audio callbacks only copy samples; recording and inference run on a worker.
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, Stream};
use eyre::{bail, Result};
use hush_core::{resample, Config, DeepFilter, Denoiser, Enhancer};
use rtrb::{Consumer, Producer, RingBuffer};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

const MAX_SECONDS: usize = 300;

#[path = "devices.rs"]
pub mod devices;
pub use devices::Selection;
#[path = "audio_task.rs"]
mod task;
pub use task::AudioTask;

pub struct Recording {
    pub original: Arc<Vec<f32>>,
    pub enhanced: Arc<Vec<f32>>,
    pub rate: u32,
    /// Microphone samples lost because the recorder could not keep up.
    pub dropped: usize,
    /// The microphone failed during the recording.
    pub device_error: bool,
}

/// Delivers audio events to the app from any thread.
#[derive(Clone)]
pub struct Sender(Arc<dyn Fn(Message) + Send + Sync>);

impl Sender {
    pub fn new(deliver: impl Fn(Message) + Send + Sync + 'static) -> Self {
        Self(Arc::new(deliver))
    }

    pub fn send(&self, message: Message) {
        (self.0)(message)
    }
}

pub enum Message {
    CaptureStarted(u64),
    CaptureStopped(u64),
    CaptureFinished(u64, Result<Option<Recording>, String>),
    PlaybackStarted(u64, bool),
    PlaybackFinished(u64, Result<PlaybackEnd, String>),
    LiveStarted(
        u64,
        Result<(super::live_io::LiveRates, hush_core::live::LiveMeters), String>,
    ),
    LiveStopped(u64, super::live_io::StopReport),
}

pub enum PlaybackEnd {
    Completed,
    Stopped,
}

/// Which model [`Settings::model`] holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelKind {
    /// NVIDIA RE-USE at `rate`, `layers` and `look_ahead`.
    Reuse,
    /// DeepFilterNet 3: 48 kHz on one thread; `layers`, `look_ahead` and `threads` are unused.
    DeepFilter,
}

pub struct Settings {
    pub kind: ModelKind,
    pub model: PathBuf,
    pub rate: u32,
    pub layers: usize,
    pub look_ahead: usize,
    pub threads: usize,
    pub placement: hush_core::WorkerPlacement,
    pub scheduling: hush_core::WorkerScheduling,
    pub bypass: bool,
    /// Live output prefill in model frames (1 or 2). Each frame adds 20 ms of latency.
    pub prefill: usize,
    /// Send live output to the Hush AI Microphone instead of the speakers.
    pub virtual_mic: bool,
    /// Live Translate's tap into the live output, if any.
    pub translate: Option<crate::translate::Tap>,
}

pub(crate) fn input_stream<T: SizedSample>(
    dev: &cpal::Device,
    config: &cpal::StreamConfig,
    mut producer: Producer<f32>,
    dropped: Arc<AtomicUsize>,
    failed: Arc<AtomicBool>,
) -> Result<Stream>
where
    f32: FromSample<T>,
{
    let channels = config.channels as usize;
    Ok(dev.build_input_stream(
        config,
        move |data: &[T], _| {
            let mut lost = 0;
            for frame in data.chunks_exact(channels) {
                let sample =
                    frame.iter().map(|s| f32::from_sample(*s)).sum::<f32>() / channels as f32;
                if producer.push(sample).is_err() {
                    lost += 1;
                }
            }
            if lost > 0 {
                dropped.fetch_add(lost, Ordering::Relaxed);
            }
        },
        move |_| {
            failed.store(true, Ordering::Relaxed);
        },
        None,
    )?)
}

fn output_stream<T: SizedSample + FromSample<f32>>(
    dev: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: Arc<Vec<f32>>,
    done: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
) -> Result<Stream> {
    let channels = config.channels as usize;
    let ramp = (config.sample_rate.0 as usize / 100).max(1);
    let mut position = 0usize;
    Ok(dev.build_output_stream(
        config,
        move |data: &mut [T], _| {
            for frame in data.chunks_mut(channels) {
                let sample = samples.get(position).copied().unwrap_or(0.0);
                let fade_in = (position as f32 / ramp as f32).min(1.0);
                let fade_out =
                    (samples.len().saturating_sub(position) as f32 / ramp as f32).min(1.0);
                frame.fill(T::from_sample(
                    (sample * fade_in * fade_out).clamp(-1.0, 1.0),
                ));
                position = position.saturating_add(1);
            }
            if position >= samples.len() {
                done.store(true, Ordering::Release);
            }
        },
        move |_| {
            failed.store(true, Ordering::Release);
        },
        None,
    )?)
}

/// Load the model and set up its workers. `automatic` with zero threads picks the count
/// (RE-USE only). Returns the model and its worker count.
pub fn load_model(settings: &Settings, automatic: bool) -> Result<(Box<dyn Enhancer>, usize)> {
    if settings.kind == ModelKind::DeepFilter {
        let mut model = DeepFilter::load(&settings.model)?;
        model.set_worker_scheduling(settings.scheduling);
        return Ok((Box::new(model), 1));
    }
    let mut model = Denoiser::with_config(
        &settings.model,
        Config {
            layers: settings.layers,
            sample_rate: settings.rate,
            look_ahead: settings.look_ahead,
        },
    )?;
    let threads = if automatic && settings.threads == 0 {
        model.set_automatic_worker_threads(settings.placement, settings.scheduling)?
    } else {
        model.set_worker_threads_with_options(
            settings.threads,
            settings.placement,
            settings.scheduling,
        )?;
        settings.threads
    };
    Ok((Box::new(model), threads))
}

fn enhance(
    samples: Vec<f32>,
    input_rate: u32,
    dropped: usize,
    device_error: bool,
    settings: Settings,
) -> Result<Recording> {
    let original = resample::mono(&samples, input_rate, settings.rate)?;
    let enhanced = if settings.bypass {
        original.clone()
    } else {
        let (mut denoiser, _) = load_model(&settings, false)?;
        let _registration = settings.scheduling.enter_current_thread()?;
        let mut out = Vec::with_capacity(original.len() + denoiser.frame());
        for chunk in original.chunks(denoiser.frame()) {
            denoiser.process_into(chunk, &mut out)?;
        }
        out.extend(denoiser.flush()?);
        out
    };
    Ok(Recording {
        original: Arc::new(original),
        enhanced: Arc::new(enhanced),
        rate: settings.rate,
        dropped,
        device_error,
    })
}

fn drain_capture(consumer: &mut Consumer<f32>, samples: &mut Vec<f32>, limit: usize) {
    // Bound each drain even if a producer remains active. After stream destruction the final
    // drain consumes the remaining queue, including a callback racing the stop request.
    let available = consumer.slots();
    for _ in 0..available {
        if let Ok(sample) = consumer.pop() {
            if samples.len() < limit {
                samples.push(sample);
            }
        }
    }
}

fn capture_run(
    selection: &Selection,
    settings: Settings,
    stop: Arc<AtomicBool>,
    sender: Sender,
    id: u64,
) -> Result<Option<Recording>> {
    if stop.load(Ordering::Acquire) {
        return Ok(None);
    }
    let dev = devices::resolve(true, selection)?;
    let supported = dev.default_input_config()?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let rate = config.sample_rate.0;
    if stop.load(Ordering::Acquire) {
        return Ok(None);
    }
    let (producer, mut consumer) = RingBuffer::new(rate as usize * 2);
    let dropped = Arc::new(AtomicUsize::new(0));
    let failed = Arc::new(AtomicBool::new(false));
    macro_rules! build {
        ($t:ty) => {
            input_stream::<$t>(&dev, &config, producer, dropped.clone(), failed.clone())?
        };
    }
    let stream = match format {
        SampleFormat::F32 => build!(f32),
        SampleFormat::F64 => build!(f64),
        SampleFormat::I8 => build!(i8),
        SampleFormat::I16 => build!(i16),
        SampleFormat::I32 => build!(i32),
        SampleFormat::I64 => build!(i64),
        SampleFormat::U8 => build!(u8),
        SampleFormat::U16 => build!(u16),
        SampleFormat::U32 => build!(u32),
        SampleFormat::U64 => build!(u64),
        other => {
            bail!("Unsupported microphone format: {other:?}");
        }
    };
    if stop.load(Ordering::Acquire) {
        return Ok(None);
    }
    stream.play()?;
    sender.send(Message::CaptureStarted(id));
    let limit = rate as usize * MAX_SECONDS;
    let mut samples = Vec::new();
    loop {
        drain_capture(&mut consumer, &mut samples, limit);
        if stop.load(Ordering::Acquire) || failed.load(Ordering::Acquire) || samples.len() == limit
        {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    stop.store(true, Ordering::Release);
    drop(stream);
    drain_capture(&mut consumer, &mut samples, limit);
    sender.send(Message::CaptureStopped(id));
    let device_error = failed.load(Ordering::Acquire);
    if samples.is_empty() {
        if device_error {
            bail!("Microphone stream failed. Reconnect it, refresh devices and record again.");
        }
        return Ok(None);
    }
    enhance(
        samples,
        rate,
        dropped.load(Ordering::Relaxed),
        device_error,
        settings,
    )
    .map(Some)
}

pub fn record(
    selection: Selection,
    settings: Settings,
    sender: Sender,
    id: u64,
) -> Result<AudioTask> {
    let events = sender.clone();
    AudioTask::spawn(
        "hush-recording",
        move |stop| capture_run(&selection, settings, stop, events, id),
        move |result| {
            sender.send(Message::CaptureFinished(
                id,
                result.map_err(|e| format!("{e:#}")),
            ))
        },
    )
}

fn playback_run(
    selection: &Selection,
    source: Arc<Vec<f32>>,
    rate: u32,
    enhanced: bool,
    stop: Arc<AtomicBool>,
    sender: Sender,
    id: u64,
) -> Result<PlaybackEnd> {
    if stop.load(Ordering::Acquire) {
        return Ok(PlaybackEnd::Stopped);
    }
    let dev = devices::resolve(false, selection)?;
    let supported = dev.default_output_config()?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let samples = if rate == config.sample_rate.0 {
        source
    } else {
        Arc::new(resample::mono(&source, rate, config.sample_rate.0)?)
    };
    if stop.load(Ordering::Acquire) {
        return Ok(PlaybackEnd::Stopped);
    }
    let done = Arc::new(AtomicBool::new(false));
    let failed = Arc::new(AtomicBool::new(false));
    macro_rules! build {
        ($t:ty) => {
            output_stream::<$t>(&dev, &config, samples, done.clone(), failed.clone())?
        };
    }
    let stream = match format {
        SampleFormat::F32 => build!(f32),
        SampleFormat::F64 => build!(f64),
        SampleFormat::I8 => build!(i8),
        SampleFormat::I16 => build!(i16),
        SampleFormat::I32 => build!(i32),
        SampleFormat::I64 => build!(i64),
        SampleFormat::U8 => build!(u8),
        SampleFormat::U16 => build!(u16),
        SampleFormat::U32 => build!(u32),
        SampleFormat::U64 => build!(u64),
        other => {
            bail!("Unsupported speaker format: {other:?}");
        }
    };
    if stop.load(Ordering::Acquire) {
        return Ok(PlaybackEnd::Stopped);
    }
    stream.play()?;
    sender.send(Message::PlaybackStarted(id, enhanced));
    while !done.load(Ordering::Acquire)
        && !failed.load(Ordering::Acquire)
        && !stop.load(Ordering::Acquire)
    {
        thread::sleep(Duration::from_millis(10));
    }
    drop(stream);
    if failed.load(Ordering::Acquire) {
        bail!("Speaker stream failed. Reconnect it, refresh devices and play again.");
    }
    Ok(if stop.load(Ordering::Acquire) {
        PlaybackEnd::Stopped
    } else {
        PlaybackEnd::Completed
    })
}

pub fn playback(
    selection: Selection,
    recording: &Recording,
    enhanced: bool,
    sender: Sender,
    id: u64,
) -> Result<AudioTask> {
    let source = if enhanced {
        recording.enhanced.clone()
    } else {
        recording.original.clone()
    };
    let rate = recording.rate;
    AudioTask::spawn(
        "hush-playback",
        {
            let events = sender.clone();
            move |stop| playback_run(&selection, source, rate, enhanced, stop, events, id)
        },
        move |result| {
            sender.send(Message::PlaybackFinished(
                id,
                result.map_err(|e| format!("{e:#}")),
            ))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_drains_the_callback_tail_without_exceeding_the_recording_limit() {
        let (mut producer, mut consumer) = RingBuffer::new(8);
        producer.push(1.0).unwrap();
        let mut samples = Vec::new();
        drain_capture(&mut consumer, &mut samples, 3);
        // A callback races the stop request; stream destruction finishes before final drain.
        for sample in [2.0, 3.0, 4.0] {
            producer.push(sample).unwrap();
        }
        drop(producer);
        drain_capture(&mut consumer, &mut samples, 3);
        assert_eq!(samples, [1.0, 2.0, 3.0]);
        assert!(consumer.pop().is_err());
    }
}
