//! Live microphone-to-speaker monitoring. All allocation and inference is outside callbacks.
use super::audio_io::{devices, Message, Selection, Sender, Settings};
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, Stream};
use eyre::{bail, ensure, Result, WrapErr};
use hush_core::{
    live::{LiveInput, LiveOutput, LiveWorker},
    virtual_mic::{Feeder, VirtualMic},
    Config, Denoiser,
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

#[path = "live_control.rs"]
pub mod control;
#[cfg(target_os = "windows")]
#[path = "vb_cable.rs"]
pub mod vb_cable;
pub use control::StopReport;
use control::StopSignal;

struct RunningSession {
    // Streams are dropped before the worker on the control thread.
    _input: Stream,
    _output: Output,
    worker: LiveWorker,
    faded: Arc<AtomicBool>,
    rates: LiveRates,
}

#[derive(Clone, Copy)]
pub struct LiveRates {
    pub input: u32,
    pub model: u32,
    pub output: u32,
    /// Inference workers in use.
    pub threads: usize,
}

pub struct LiveSession {
    stopping: Arc<StopSignal>,
}

impl LiveSession {
    pub fn request_stop(&self, sender: Sender, id: u64) {
        self.stopping.request_stop();
        // A device API may block while starting. Report it without claiming teardown completed.
        watch_shutdown(self.stopping.clone(), sender, id);
    }
}

impl Drop for LiveSession {
    fn drop(&mut self) {
        self.stopping.request_stop();
    }
}

fn watch_shutdown(signal: Arc<StopSignal>, sender: Sender, id: u64) {
    if signal.begin_watchdog() {
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(1));
            let report = signal.report(None);
            if !report.devices_closed {
                sender.send(Message::LiveStopped(id, report));
            }
        });
    }
}

fn config(
    device: &cpal::Device,
    input: bool,
    rate: u32,
) -> Result<(cpal::StreamConfig, SampleFormat)> {
    let configs: Vec<_> = if input {
        device.supported_input_configs()?.collect()
    } else {
        device.supported_output_configs()?.collect()
    };
    let chosen = configs
        .into_iter()
        .filter(|c| c.min_sample_rate().0 <= rate && c.max_sample_rate().0 >= rate)
        .min_by_key(|c| {
            (
                usize::from(c.channels() != if input { 1 } else { 2 }),
                usize::from(c.sample_format() != SampleFormat::F32),
            )
        });
    // Model rate and device rate are independent. Shared-mode devices commonly expose only
    // their current mix rate (e.g. 48 kHz); convert on the inference worker when necessary.
    let chosen = if let Some(chosen) = chosen {
        chosen.with_sample_rate(cpal::SampleRate(rate))
    } else if input {
        device.default_input_config()?
    } else {
        device.default_output_config()?
    };
    ensure!(
        chosen.channels() > 0 && chosen.sample_rate().0 > 0,
        "Invalid audio device configuration"
    );
    Ok((chosen.config(), chosen.sample_format()))
}

fn input_stream<T: SizedSample>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut input: LiveInput,
    stopping: Arc<StopSignal>,
) -> Result<Stream>
where
    f32: FromSample<T>,
{
    let channels = config.channels as usize;
    let mut scratch = [0.0f32; 1024];
    let failed = stopping.clone();
    Ok(device.build_input_stream(
        config,
        move |data: &[T], _| {
            if stopping.is_stopping() {
                return;
            }
            for block in data.chunks(channels * scratch.len()) {
                let mut count = 0;
                for frame in block.chunks_exact(channels) {
                    scratch[count] =
                        frame.iter().map(|s| f32::from_sample(*s)).sum::<f32>() / channels as f32;
                    count += 1;
                }
                input.push(&scratch[..count]);
            }
        },
        move |error| {
            failed.stream_error(true, &error);
        },
        None,
    )?)
}

fn output_stream<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut output: LiveOutput,
    stopping: Arc<StopSignal>,
    faded: Arc<AtomicBool>,
) -> Result<Stream> {
    let channels = config.channels as usize;
    let ramp = (config.sample_rate.0 as usize / 200).max(1);
    let mut scratch = [0.0f32; 1024];
    let mut fade_remaining = ramp;
    let failed = faded.clone();
    let failure_stop = stopping.clone();
    Ok(device.build_output_stream(
        config,
        move |data: &mut [T], _| {
            for block in data.chunks_mut(channels * scratch.len()) {
                let count = block.len() / channels;
                output.read(&mut scratch[..count]);
                let mut frames = block.chunks_exact_mut(channels);
                for (frame, sample) in frames.by_ref().zip(&scratch[..count]) {
                    let gain = if stopping.is_stopping() {
                        fade_remaining = fade_remaining.saturating_sub(1);
                        fade_remaining as f32 / ramp as f32
                    } else {
                        1.0
                    };
                    frame.fill(T::from_sample((sample * gain).clamp(-1.0, 1.0)));
                }
                frames.into_remainder().fill(T::from_sample(0.0));
            }
            if fade_remaining == 0 {
                faded.store(true, Ordering::Release);
            }
        },
        move |error| {
            failed.store(true, Ordering::Release);
            failure_stop.stream_error(false, &error);
        },
        None,
    )?)
}

/// Where live output goes.
enum Output {
    Speakers(Stream),
    // Held for its drop, which stops the writer thread.
    VirtualMic(#[allow(dead_code)] Feeder),
}

/// Feed the Hush AI Microphone with live output, with the same stop fade as the speakers.
fn virtual_mic_feeder(
    mic: VirtualMic,
    mut output: LiveOutput,
    stopping: Arc<StopSignal>,
    faded: Arc<AtomicBool>,
) -> Result<Feeder> {
    let ramp = (hush_core::virtual_mic::RATE as usize / 200).max(1);
    let mut fade_remaining = ramp;
    Feeder::start(mic, move |block| {
        output.read(block);
        for sample in block.iter_mut() {
            if stopping.is_stopping() {
                fade_remaining = fade_remaining.saturating_sub(1);
                *sample *= fade_remaining as f32 / ramp as f32;
            }
            *sample = sample.clamp(-1.0, 1.0);
        }
        if fade_remaining == 0 {
            faded.store(true, Ordering::Release);
        }
    })
}

fn queue_capacities(denoiser: &Denoiser, rates: LiveRates, bypass: bool) -> Result<(usize, usize)> {
    // NVIDIA rounds the 22.05 kHz hop to 442 samples, slightly longer than 20 ms.
    // Match LiveWorker's conversion of the actual hop, including its device-rate bypass path.
    let (frame, rate) = if bypass {
        (rates.input.div_ceil(50) as u64, rates.input)
    } else {
        (denoiser.frame() as u64, denoiser.sample_rate())
    };
    let capacity = |device_rate: u32| -> Result<usize> {
        let frame = (frame * u64::from(device_rate)).div_ceil(u64::from(rate));
        usize::try_from(frame)
            .ok()
            .and_then(|frame| frame.checked_mul(4))
            .ok_or_else(|| eyre::eyre!("Audio queue capacity exceeds addressable memory"))
    };
    Ok((capacity(rates.input)?, capacity(rates.output)?))
}

fn start_streams(
    input_selection: Selection,
    output_selection: Selection,
    settings: Settings,
    stopping: Arc<StopSignal>,
) -> Result<RunningSession> {
    let input_device = devices::resolve(true, &input_selection)?;
    let (input_config, input_format) = config(&input_device, true, settings.rate)
        .wrap_err_with(|| format!("Cannot configure microphone '{}'. Choose an input device and refresh devices if it was disconnected", input_device.name().unwrap_or_default()))?;
    // The Hush driver first; on Windows, a renamed VB-CABLE otherwise (played like speakers).
    let virtual_mic = if settings.virtual_mic {
        match VirtualMic::open() {
            Ok(mic) => Some(mic),
            // Windows: VB-CABLE; Linux: the PulseAudio feed sink. Both play like speakers.
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            Err(_) => None,
            #[cfg(not(any(target_os = "windows", target_os = "linux")))]
            Err(error) => return Err(error.wrap_err("Cannot open the Hush AI Microphone")),
        }
    } else {
        None
    };
    let speakers = if virtual_mic.is_some() {
        None
    } else {
        #[cfg(target_os = "windows")]
        let device = if settings.virtual_mic {
            vb_cable::feed_device()?
        } else {
            devices::resolve(false, &output_selection)?
        };
        #[cfg(target_os = "linux")]
        let device = if settings.virtual_mic {
            super::pulse::feed_device()?
        } else {
            devices::resolve(false, &output_selection)?
        };
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        let device = devices::resolve(false, &output_selection)?;
        let (config, format) = config(&device, false, settings.rate)
            .wrap_err_with(|| format!("Cannot configure speakers '{}'. Choose an output device and refresh devices if it was disconnected", device.name().unwrap_or_default()))?;
        Some((device, config, format))
    };
    let rates = LiveRates {
        input: input_config.sample_rate.0,
        model: settings.rate,
        output: speakers
            .as_ref()
            .map_or(hush_core::virtual_mic::RATE, |(_, config, _)| {
                config.sample_rate.0
            }),
        threads: 0,
    };
    let mut denoiser = Denoiser::with_config(
        &settings.model,
        Config {
            layers: settings.layers,
            sample_rate: settings.rate,
            look_ahead: settings.look_ahead,
        },
    )?;
    let threads = if settings.threads == 0 {
        denoiser.set_automatic_worker_threads(settings.placement, settings.scheduling)?
    } else {
        denoiser.set_worker_threads_with_options(
            settings.threads,
            settings.placement,
            settings.scheduling,
        )?;
        settings.threads
    };
    let rates = LiveRates { threads, ..rates };
    let (input_capacity, output_capacity) = queue_capacities(&denoiser, rates, settings.bypass)?;
    let (worker, input, output) = LiveWorker::start_clocked_with_prefill(
        denoiser,
        rates.input,
        rates.output,
        input_capacity,
        output_capacity,
        settings.bypass,
        settings.prefill,
    )?;
    let faded = Arc::new(AtomicBool::new(false));
    macro_rules! build_input {
        ($t:ty) => {
            input_stream::<$t>(&input_device, &input_config, input, stopping.clone())?
        };
    }
    let input_stream = match input_format {
        SampleFormat::F32 => build_input!(f32),
        SampleFormat::F64 => build_input!(f64),
        SampleFormat::I8 => build_input!(i8),
        SampleFormat::I16 => build_input!(i16),
        SampleFormat::I32 => build_input!(i32),
        SampleFormat::I64 => build_input!(i64),
        SampleFormat::U8 => build_input!(u8),
        SampleFormat::U16 => build_input!(u16),
        SampleFormat::U32 => build_input!(u32),
        SampleFormat::U64 => build_input!(u64),
        other => {
            bail!("Unsupported microphone format: {other:?}");
        }
    };
    let output_stream = match (speakers, virtual_mic) {
        (Some((device, config, format)), _) => Output::Speakers(speaker_stream(
            &device,
            &config,
            format,
            output,
            stopping.clone(),
            faded.clone(),
        )?),
        (None, Some(mic)) => Output::VirtualMic(virtual_mic_feeder(
            mic,
            output,
            stopping.clone(),
            faded.clone(),
        )?),
        (None, None) => unreachable!("one output is always configured"),
    };
    if stopping.is_stopping() {
        bail!("Live monitoring was stopped");
    }
    if let Output::Speakers(stream) = &output_stream {
        stream.play()?;
    }
    if stopping.is_stopping() {
        bail!("Live monitoring was stopped");
    }
    input_stream.play()?;
    Ok(RunningSession {
        _input: input_stream,
        _output: output_stream,
        worker,
        faded,
        rates,
    })
}

fn speaker_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    format: SampleFormat,
    output: LiveOutput,
    stopping: Arc<StopSignal>,
    faded: Arc<AtomicBool>,
) -> Result<Stream> {
    macro_rules! build_output {
        ($t:ty) => {
            output_stream::<$t>(device, config, output, stopping, faded)?
        };
    }
    Ok(match format {
        SampleFormat::F32 => build_output!(f32),
        SampleFormat::F64 => build_output!(f64),
        SampleFormat::I8 => build_output!(i8),
        SampleFormat::I16 => build_output!(i16),
        SampleFormat::I32 => build_output!(i32),
        SampleFormat::I64 => build_output!(i64),
        SampleFormat::U8 => build_output!(u8),
        SampleFormat::U16 => build_output!(u16),
        SampleFormat::U32 => build_output!(u32),
        SampleFormat::U64 => build_output!(u64),
        other => {
            bail!("Unsupported speaker format: {other:?}");
        }
    })
}

/// Device discovery, stream creation and teardown happen on a control thread because platform
/// audio APIs can block. The inference worker and the device callbacks remain separate.
pub fn start(
    input_selection: Selection,
    output_selection: Selection,
    settings: Settings,
    sender: Sender,
    id: u64,
) -> Result<LiveSession> {
    let stopping = Arc::new(StopSignal::default());
    let signal = stopping.clone();
    thread::Builder::new()
        .name("hush-audio-control".into())
        .spawn(move || {
            match start_streams(input_selection, output_selection, settings, signal.clone()) {
                Ok(running) => {
                    if !signal.is_stopping() {
                        sender.send(Message::LiveStarted(
                            id,
                            Ok((running.rates, running.worker.meters())),
                        ));
                    }
                    while !signal.is_stopping() {
                        if running.worker.stats().worker_failed {
                            signal.worker_failed();
                            break;
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
                    watch_shutdown(signal.clone(), sender.clone(), id);
                    let deadline = Instant::now() + Duration::from_millis(300);
                    while !running.faded.load(Ordering::Acquire) && Instant::now() < deadline {
                        thread::sleep(Duration::from_millis(5));
                    }
                    let stats = running.worker.stats();
                    if stats.worker_failed {
                        signal.worker_failed();
                    }
                    drop(running);
                    // A final report means device handles are released and the worker joined.
                    // The separate timeout notification reports that the device is still closing.
                    signal.mark_closed();
                    sender.send(Message::LiveStopped(id, signal.report(Some(stats))));
                }
                Err(error) => {
                    signal.mark_closed();
                    if let Some(error) = signal.startup_failure(&error) {
                        sender.send(Message::LiveStarted(id, Err(error)));
                    } else {
                        sender.send(Message::LiveStopped(id, signal.report(None)));
                    }
                }
            }
        })?;
    Ok(LiveSession { stopping })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clocked_22050_profile_starts_with_actual_hop_capacity() {
        let Some(path) =
            std::env::var_os("HUSH_MODEL").filter(|p| std::path::Path::new(p).is_file())
        else {
            eprintln!("HUSH_MODEL absent; 22.05 kHz device-capacity regression skipped");
            return;
        };
        let mut model = Denoiser::with_config(
            path,
            Config {
                layers: 3,
                sample_rate: 22050,
                look_ahead: 1,
            },
        )
        .unwrap();
        model.set_worker_threads(2).unwrap();
        assert_eq!(model.frame(), 442);
        let rates = LiveRates {
            input: 48000,
            model: 22050,
            output: 48000,
            threads: 2,
        };
        let capacities = queue_capacities(&model, rates, false).unwrap();
        assert_eq!(capacities, (3852, 3852));
        assert_eq!(
            queue_capacities(
                &model,
                LiveRates {
                    output: 44100,
                    ..rates
                },
                false
            )
            .unwrap(),
            (3852, 3536)
        );
        // Bypass uses the capture-device hop rather than the selected model's even hop.
        assert_eq!(queue_capacities(&model, rates, true).unwrap(), (3840, 3840));
        assert_eq!(
            queue_capacities(
                &model,
                LiveRates {
                    input: 22050,
                    ..rates
                },
                true
            )
            .unwrap(),
            (1764, 3840)
        );
        let (worker, _, _) = LiveWorker::start_clocked(
            model,
            rates.input,
            rates.output,
            capacities.0,
            capacities.1,
            false,
        )
        .unwrap();
        assert!(!worker.stats().worker_failed);
        eprintln!("22.05 kHz clocked startup executed with four actual-hop device frames");
    }
}
