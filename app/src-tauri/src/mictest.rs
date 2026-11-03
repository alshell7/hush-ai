//! Microphone checks for setup and settings: a live level meter, and a short recording
//! that plays back with and without Hush so the user can hear the difference.
use crate::audio::audio_io::{
    self, devices, AudioTask, Message, PlaybackEnd, Recording, Selection, Sender,
};
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use serde::Serialize;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "phase", rename_all = "camelCase")]
pub enum TestEvent {
    Recording {
        seconds: f32,
    },
    Processing,
    Ready {
        original: Vec<f32>,
        enhanced: Vec<f32>,
        seconds: f32,
    },
    Playing {
        enhanced: bool,
    },
    Stopped,
    Error {
        message: String,
    },
}

#[derive(Default)]
pub struct MicTest {
    monitor: Mutex<Option<Monitor>>,
    task: Mutex<Option<AudioTask>>,
    recording: Mutex<Option<Recording>>,
    id: Mutex<u64>,
}

struct Monitor {
    stop: Arc<AtomicBool>,
}

impl Drop for Monitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Peaks in `bins` buckets, for drawing a waveform.
fn envelope(samples: &[f32], bins: usize) -> Vec<f32> {
    if samples.is_empty() {
        return vec![0.0; bins];
    }
    let size = samples.len().div_ceil(bins);
    samples
        .chunks(size)
        .map(|chunk| chunk.iter().fold(0f32, |m, v| m.max(v.abs())).min(1.0))
        .collect()
}

fn input_device(name: &Option<String>) -> eyre::Result<(Selection, cpal::Device)> {
    let selection = match name {
        Some(name) => devices::discover(true)?
            .named(name)
            .unwrap_or(Selection::Default),
        None => Selection::Default,
    };
    let device = devices::resolve(true, &selection)?;
    Ok((selection, device))
}

fn level_stream<T: SizedSample>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    peak: Arc<AtomicU32>,
) -> eyre::Result<cpal::Stream>
where
    f32: FromSample<T>,
{
    Ok(device.build_input_stream(
        config,
        move |data: &[T], _| {
            let block = data
                .iter()
                .fold(0f32, |m, s| m.max(f32::from_sample(*s).abs()));
            let old = f32::from_bits(peak.load(Ordering::Relaxed));
            if block > old {
                peak.store(block.to_bits(), Ordering::Relaxed);
            }
        },
        |_| {},
        None,
    )?)
}

impl MicTest {
    /// Show the microphone level ("mic-level" events) until `stop_monitor` or 2 minutes.
    pub fn start_monitor(&self, app: &AppHandle, name: Option<String>) -> eyre::Result<()> {
        let stop = Arc::new(AtomicBool::new(false));
        *self.monitor.lock().unwrap() = Some(Monitor { stop: stop.clone() });
        let app = app.clone();
        thread::Builder::new()
            .name("hush-mic-level".into())
            .spawn(move || {
                let result = (|| -> eyre::Result<()> {
                    let (_, device) = input_device(&name)?;
                    let supported = device.default_input_config()?;
                    let format = supported.sample_format();
                    let config: cpal::StreamConfig = supported.into();
                    let peak = Arc::new(AtomicU32::new(0));
                    let stream = match format {
                        SampleFormat::F32 => level_stream::<f32>(&device, &config, peak.clone())?,
                        SampleFormat::I16 => level_stream::<i16>(&device, &config, peak.clone())?,
                        SampleFormat::I32 => level_stream::<i32>(&device, &config, peak.clone())?,
                        SampleFormat::U16 => level_stream::<u16>(&device, &config, peak.clone())?,
                        SampleFormat::U8 => level_stream::<u8>(&device, &config, peak.clone())?,
                        other => {
                            eyre::bail!("Unsupported microphone format: {other:?}");
                        }
                    };
                    stream.play()?;
                    for _ in 0..(120 * 20) {
                        if stop.load(Ordering::Relaxed) {
                            break;
                        }
                        thread::sleep(Duration::from_millis(50));
                        let level = f32::from_bits(peak.swap(0, Ordering::Relaxed));
                        let _ = app.emit("mic-level", level.min(1.0));
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    let _ = app.emit("mic-level-error", format!("{error:#}"));
                }
            })?;
        Ok(())
    }

    pub fn stop_monitor(&self) {
        self.monitor.lock().unwrap().take();
    }

    fn sender(app: &AppHandle) -> Sender {
        let app = app.clone();
        Sender::new(move |message| {
            if let Some(test) = app.try_state::<MicTest>() {
                test.on_message(&app, message);
            }
        })
    }

    fn next_id(&self) -> u64 {
        let mut id = self.id.lock().unwrap();
        *id += 1;
        *id
    }

    /// Record `seconds` from the chosen microphone, then enhance it with the chosen quality.
    pub fn record(&self, app: &AppHandle, seconds: f32) -> eyre::Result<()> {
        self.stop_monitor();
        self.stop();
        let settings = app.state::<crate::AppState>().settings();
        let data_dir = app.path().app_data_dir()?;
        let model = crate::model::find(&data_dir)
            .ok_or_else(|| eyre::eyre!("Download the voice model first"))?;
        let mut audio = crate::engine::audio_settings(&settings, model, false);
        audio.threads = thread::available_parallelism()
            .map_or(2, |n| n.get() / 2)
            .clamp(1, 4);
        audio.scheduling = hush_core::WorkerScheduling::OsDefault;
        let (selection, _) = input_device(&settings.input)?;
        let id = self.next_id();
        let task = audio_io::record(selection, audio, Self::sender(app), id)?;
        *self.task.lock().unwrap() = Some(task);
        let app = app.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_secs_f32(seconds));
            if let Some(test) = app.try_state::<MicTest>() {
                if *test.id.lock().unwrap() == id {
                    if let Some(task) = test.task.lock().unwrap().as_ref() {
                        task.request_stop();
                    }
                }
            }
        });
        Ok(())
    }

    pub fn play(&self, app: &AppHandle, enhanced: bool) -> eyre::Result<()> {
        self.stop();
        let settings = app.state::<crate::AppState>().settings();
        let output = settings
            .output
            .as_ref()
            .and_then(|name| devices::discover(false).and_then(|l| l.named(name)).ok())
            .unwrap_or(Selection::Default);
        let recording = self.recording.lock().unwrap();
        let recording = recording
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Record a test first"))?;
        let id = self.next_id();
        let task = audio_io::playback(output, recording, enhanced, Self::sender(app), id)?;
        *self.task.lock().unwrap() = Some(task);
        Ok(())
    }

    pub fn stop(&self) {
        if let Some(task) = self.task.lock().unwrap().take() {
            task.request_stop();
        }
    }

    fn on_message(&self, app: &AppHandle, message: Message) {
        let current = *self.id.lock().unwrap();
        let event = match message {
            Message::CaptureStarted(id) if id == current => {
                Some(TestEvent::Recording { seconds: 0.0 })
            }
            Message::CaptureStopped(id) if id == current => Some(TestEvent::Processing),
            Message::CaptureFinished(id, result) if id == current => Some(match result {
                Ok(Some(recording)) if recording.device_error && recording.original.is_empty() => {
                    TestEvent::Error {
                        message: "The microphone stopped during the recording.".into(),
                    }
                }
                Ok(Some(recording)) => {
                    if recording.dropped > 0 {
                        log::warn!("Test recording lost {} samples", recording.dropped);
                    }
                    let event = TestEvent::Ready {
                        original: envelope(&recording.original, 96),
                        enhanced: envelope(&recording.enhanced, 96),
                        seconds: recording.original.len() as f32 / recording.rate as f32,
                    };
                    *self.recording.lock().unwrap() = Some(recording);
                    event
                }
                Ok(None) => TestEvent::Stopped,
                Err(message) => TestEvent::Error { message },
            }),
            Message::PlaybackStarted(id, enhanced) if id == current => {
                Some(TestEvent::Playing { enhanced })
            }
            Message::PlaybackFinished(id, result) if id == current => Some(match result {
                Ok(PlaybackEnd::Completed | PlaybackEnd::Stopped) => TestEvent::Stopped,
                Err(message) => TestEvent::Error { message },
            }),
            _ => None,
        };
        if let Some(event) = event {
            let _ = app.emit("test", event);
        }
    }
}
