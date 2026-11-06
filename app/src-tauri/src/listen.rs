//! Their voice: what the meeting app plays, cleaned if you like, translated for you.
//!
//! Two ways to hear it:
//! - **Hush AI Speaker.** The meeting app plays to a virtual speaker; Hush reads it, removes
//!   their background noise with DeepFilterNet 3 (light or full) and plays it to your real
//!   speakers or headphones. macOS: the Hush plug-in's speaker; Windows: a render endpoint
//!   named "Hush AI Speaker" or a VB-CABLE A/B cable, read by WASAPI loopback; Linux: a
//!   PulseAudio null sink that Hush creates.
//! - **Listening along** (no virtual speaker): Hush listens to what your speakers play, for
//!   translation only. Windows uses WASAPI loopback, Linux the default sink's monitor.
//!
//! The listener thread owns the streams (they are not `Send`). The capture callback only
//! queues samples; one worker loop resamples to 48 kHz, cleans, feeds the translation tap and
//! queues the result for the playback callback. Nothing in a callback allocates or locks.
use crate::conversation::Side;
use crate::settings::{Clean, Hear, Settings};
use crate::translate::{self, Tap};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, Stream};
use eyre::{eyre, Result};
use hush_core::resample::StreamingMono;
use rtrb::{Consumer, Producer, RingBuffer};
use serde::Serialize;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};

/// Everything inside runs at this rate.
const RATE: u32 = 48_000;
/// 10 ms blocks.
const BLOCK: usize = RATE as usize / 100;
/// Playback keeps at most this much queued, so their voice stays in step with the video.
const MAX_QUEUED: usize = RATE as usize * 80 / 1000;
/// Audio louder than this means someone is talking (about -60 dBFS).
const SIGNAL: f32 = 0.001;
/// Voice-over: their voice under the translation, and how fast it ducks and comes back
/// (per 10 ms block: about 40 ms down, 400 ms up, after a short hold).
const DUCKED: f32 = 0.22;
const DUCK_DOWN: f32 = 0.25;
const DUCK_UP: f32 = 0.025;
const DUCK_HOLD: u32 = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    /// A virtual speaker: Hush plays their voice, and can clean it.
    Speaker,
    /// Listening along to the speakers: translation only.
    Loopback,
    /// Nothing to listen to on this system.
    None,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ListenInfo {
    pub mode: Mode,
    /// The virtual speaker the meeting app should play to.
    pub device: Option<String>,
    /// Where Hush plays their voice (speaker mode).
    pub output: Option<String>,
    pub running: bool,
    /// Someone's voice arrived in the last moments: a call is playing.
    pub active: bool,
    /// Their voice is being cleaned right now.
    pub cleaning: bool,
    /// Why cleaning or listening cannot run, if it cannot.
    pub message: Option<String>,
}

impl Default for ListenInfo {
    fn default() -> Self {
        Self {
            mode: Mode::None,
            device: None,
            output: None,
            running: false,
            active: false,
            cleaning: false,
            message: None,
        }
    }
}

/// The app-wide listener.
pub struct Listener {
    info: Mutex<ListenInfo>,
    /// Ask the listener thread to look again (settings or devices changed).
    wake: AtomicBool,
    /// Their voice's level, 0..1, for the notch and the widget.
    pub level: AtomicU32,
}

impl Listener {
    pub fn new() -> Self {
        Self {
            info: Mutex::new(ListenInfo::default()),
            wake: AtomicBool::new(true),
            level: AtomicU32::new(0),
        }
    }

    pub fn info(&self) -> ListenInfo {
        self.info.lock().unwrap().clone()
    }

    /// Settings, devices or translation changed: reopen if needed.
    pub fn refresh(&self) {
        self.wake.store(true, Ordering::Release);
    }

    fn set(&self, app: &AppHandle, info: ListenInfo) {
        let changed = {
            let mut current = self.info.lock().unwrap();
            let changed = *current != info;
            *current = info;
            changed
        };
        if changed {
            let _ = app.emit("listen", self.info());
        }
    }
}

/// What the listener thread should open now.
#[derive(Clone, Debug, PartialEq)]
struct Plan {
    mode: Mode,
    /// The device to capture from, and whether to capture it by loopback.
    capture: Option<String>,
    loopback: bool,
    /// Where to play their voice; `None` in loopback mode.
    play: Option<String>,
    clean: Clean,
    /// What you hear while their voice is translated.
    voice: Hear,
}

/// Names of output devices that are virtual speakers, best first (Windows).
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn virtual_speaker_names(names: &[String]) -> Option<String> {
    let rank = |name: &str| {
        let lower = name.to_lowercase();
        if lower.contains("hush ai speaker") && !lower.contains("monitor") {
            Some(0)
        } else if lower.contains("hi-fi cable input") {
            Some(1)
        } else if lower.contains("cable-a input") {
            Some(2)
        } else if lower.contains("cable-b input") {
            Some(3)
        } else {
            None
        }
    };
    names
        .iter()
        .filter_map(|n| rank(n).map(|r| (r, n)))
        .min_by_key(|(r, _)| *r)
        .map(|(_, n)| n.clone())
}

fn output_names() -> Vec<String> {
    cpal::default_host()
        .output_devices()
        .map(|devices| devices.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}

/// Real speakers for their voice: the chosen ones, or the system default unless that is a
/// virtual device, or the first real one.
fn real_output(settings: &Settings, names: &[String], speaker: Option<&str>) -> Option<String> {
    let is_virtual = |name: &str| {
        let lower = name.to_lowercase();
        Some(name) == speaker
            || lower.contains("hush ai")
            || lower.contains("cable")
            || lower.contains("vb-audio")
    };
    if let Some(chosen) = &settings.listen_output {
        if names.contains(chosen) && !is_virtual(chosen) {
            return Some(chosen.clone());
        }
    }
    let default = cpal::default_host()
        .default_output_device()
        .and_then(|d| d.name().ok());
    if let Some(default) = default.filter(|d| !is_virtual(d)) {
        return Some(default);
    }
    names.iter().find(|n| !is_virtual(n)).cloned()
}

fn plan(app: &AppHandle) -> (Plan, ListenInfo) {
    let settings = app.state::<crate::AppState>().settings();
    let translating = app.state::<Arc<translate::Translator>>().is_on(Side::Them);
    let names = output_names();
    let speaker = speaker_device(&names);
    let mut info = ListenInfo::default();
    let none = Plan {
        mode: Mode::None,
        capture: None,
        loopback: false,
        play: None,
        clean: Clean::Off,
        voice: Hear::Original,
    };
    if let Some(device) = speaker {
        let play = real_output(&settings, &names, Some(&device));
        info.mode = Mode::Speaker;
        info.device = Some(device.clone());
        info.output = play.clone();
        if play.is_none() {
            info.message = Some("No speakers or headphones to play their voice on.".into());
            return (none, info);
        }
        if settings.listen {
            return (
                Plan {
                    mode: Mode::Speaker,
                    loopback: cfg!(target_os = "windows"),
                    capture: Some(device),
                    play,
                    clean: settings.listen_clean,
                    voice: settings.listen_voice,
                },
                info,
            );
        }
        // Off: their voice still has to reach the speakers, so pass it through.
        return (
            Plan {
                mode: Mode::Speaker,
                loopback: cfg!(target_os = "windows"),
                capture: Some(device),
                play,
                clean: Clean::Off,
                voice: settings.listen_voice,
            },
            info,
        );
    }
    if loopback_available() {
        info.mode = Mode::Loopback;
        if translating {
            return (
                Plan {
                    mode: Mode::Loopback,
                    capture: None,
                    loopback: true,
                    play: None,
                    clean: Clean::Off,
                    voice: Hear::Original,
                },
                info,
            );
        }
    }
    (none, info)
}

/// The virtual speaker on this system, if any.
fn speaker_device(names: &[String]) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let _ = names;
        hush_core::virtual_speaker::VirtualSpeaker::open()
            .ok()
            .map(|_| "Hush AI Speaker".to_string())
    }
    #[cfg(target_os = "linux")]
    {
        let _ = names;
        crate::audio::pulse::speaker_ready().then(|| "Hush AI Speaker".to_string())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        virtual_speaker_names(names)
    }
}

fn loopback_available() -> bool {
    cfg!(any(target_os = "windows", target_os = "linux"))
}

/// Runs for the whole app: keeps the right streams open.
pub fn run(app: AppHandle) {
    let listener = app.state::<Arc<Listener>>().inner().clone();
    let mut current: Option<(Plan, Running)> = None;
    let mut checked = Instant::now() - Duration::from_secs(60);
    loop {
        // Look again when asked, and every few seconds for devices that came or went.
        let due = listener.wake.swap(false, Ordering::AcqRel)
            || checked.elapsed() >= Duration::from_secs(4);
        if due {
            checked = Instant::now();
            let (wanted, mut info) = plan(&app);
            let same = current.as_ref().is_some_and(|(p, _)| *p == wanted);
            if !same {
                current = None;
                if wanted.mode != Mode::None {
                    match open(&app, &wanted) {
                        Ok(running) => current = Some((wanted, running)),
                        Err(error) => {
                            log::warn!("Listen: {error:#}");
                            info.message = Some(format!("{error}"));
                        }
                    }
                }
            }
            if let Some((_, running)) = &current {
                info.running = true;
                if let Some(message) = &running.clean_error {
                    info.message = Some(message.clone());
                }
            }
            listener.set(&app, merge(info, &listener.info(), current.as_ref()));
        }
        let failed = match current.as_mut() {
            Some((_, running)) => running.step(&listener).err(),
            None => None,
        };
        if let Some(error) = failed {
            log::warn!("Listen: {error:#}");
            current = None;
            listener.refresh();
        }
        match current.as_mut() {
            Some((_, running)) => {
                let mut info = listener.info();
                let active = running.last_signal.elapsed() < Duration::from_millis(1500);
                let cleaning = running.clean.is_some() && active;
                if info.active != active || info.cleaning != cleaning {
                    info.active = active;
                    info.cleaning = cleaning;
                    listener.set(&app, info);
                }
                thread::sleep(Duration::from_millis(5));
            }
            None => {
                listener.level.store(0, Ordering::Relaxed);
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

/// Keep the live parts of the previous state.
fn merge(mut info: ListenInfo, old: &ListenInfo, current: Option<&(Plan, Running)>) -> ListenInfo {
    if current.is_some() {
        info.active = old.active;
        info.cleaning = old.cleaning;
    }
    info
}

// ---------------------------------------------------------------- Streams

enum Capture {
    /// Kept open while it lives.
    #[allow(dead_code)]
    Stream(Stream),
    #[cfg(target_os = "macos")]
    Ring(hush_core::virtual_speaker::VirtualSpeaker),
}

struct Running {
    _capture: Capture,
    _play: Option<Stream>,
    captured: Consumer<f32>,
    to_rate: StreamingMono,
    from_rate: Option<StreamingMono>,
    queued: Option<Producer<f32>>,
    clean: Option<hush_core::DeepFilter>,
    clean_error: Option<String>,
    tap: Tap,
    voice: Hear,
    /// Voice-over: their voice's gain now, and blocks left before it comes back up.
    duck: f32,
    hold: u32,
    block: Vec<f32>,
    cleaned: Vec<f32>,
    translated: Vec<f32>,
    last_signal: Instant,
    peak: f32,
    #[cfg(target_os = "macos")]
    ring_scratch: Vec<f32>,
    /// Stops the translation worker when these streams close.
    stop: Arc<AtomicBool>,
}

fn find_device(input: bool, name: &str) -> Result<cpal::Device> {
    let host = cpal::default_host();
    let mut devices = if input {
        host.input_devices()?
    } else {
        host.output_devices()?
    };
    devices
        .find(|d| d.name().is_ok_and(|n| n == name))
        .ok_or_else(|| eyre!("'{name}' is not available"))
}

fn open(app: &AppHandle, plan: &Plan) -> Result<Running> {
    let translator = app.state::<Arc<translate::Translator>>();
    let (tap, link) = translate::tap(translator.flag(Side::Them));
    tap.rate.store(RATE, Ordering::Release);
    let (producer, captured) = RingBuffer::new(RATE as usize * 2);

    let (capture, capture_rate) = open_capture(plan, producer)?;

    let (play, queued, from_rate) = match &plan.play {
        Some(name) => {
            let device = find_device(false, name)?;
            let (stream, rate, queue) = open_playback(&device)?;
            (
                Some(stream),
                Some(queue),
                Some(StreamingMono::new(RATE, rate)?),
            )
        }
        None => (None, None, None),
    };

    let (clean, clean_error) = match plan.clean.limit() {
        Some(limit) => match deep_filter(app, limit) {
            Ok(model) => (Some(model), None),
            Err(error) => (None, Some(error)),
        },
        None => (None, None),
    };

    // The translation worker for their voice lives as long as these streams.
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let worker_app = app.clone();
    thread::Builder::new()
        .name("hush-translate-them".into())
        .spawn(move || translate::run(worker_app, link, worker_stop, Side::Them))?;

    Ok(Running {
        _capture: capture,
        _play: play,
        captured,
        to_rate: StreamingMono::new(capture_rate, RATE)?,
        from_rate,
        queued,
        clean,
        clean_error,
        tap,
        voice: plan.voice,
        duck: 1.0,
        hold: 0,
        block: Vec::with_capacity(BLOCK * 4),
        cleaned: Vec::with_capacity(BLOCK * 4),
        translated: vec![0.0; BLOCK],
        last_signal: Instant::now() - Duration::from_secs(10),
        peak: 0.0,
        #[cfg(target_os = "macos")]
        ring_scratch: vec![0.0; BLOCK * 4],
        stop,
    })
}

impl Running {
    /// Move everything captured so far through cleaning, translation and playback.
    fn step(&mut self, listener: &Listener) -> Result<()> {
        #[cfg(target_os = "macos")]
        if let Capture::Ring(ring) = &mut self._capture {
            let read = ring.read(&mut self.ring_scratch, BLOCK * 6);
            for sample in &self.ring_scratch[..read] {
                self.to_rate.push(*sample, |s| self.block.push(s));
            }
            if ring.active() {
                self.last_signal = Instant::now();
            }
        }
        while let Ok(sample) = self.captured.pop() {
            self.to_rate.push(sample, |s| self.block.push(s));
        }
        let blocks = self.block.len() / BLOCK;
        if blocks == 0 {
            return Ok(());
        }
        let take = blocks * BLOCK;
        let mut peak = 0.0f32;
        for s in &self.block[..take] {
            peak = peak.max(s.abs());
        }
        if peak > SIGNAL {
            self.last_signal = Instant::now();
        }
        self.peak = (self.peak * 0.85).max(peak);
        listener
            .level
            .store(self.peak.min(1.0).to_bits(), Ordering::Relaxed);

        self.cleaned.clear();
        match self.clean.as_mut() {
            Some(model) => model.process_into(&self.block[..take], &mut self.cleaned)?,
            None => self.cleaned.extend_from_slice(&self.block[..take]),
        }
        self.block.drain(..take);

        for chunk in self.cleaned.chunks_mut(BLOCK) {
            if self.voice == Hear::Original || self.queued.is_none() {
                self.tap.feed(chunk);
                continue;
            }
            let n = chunk.len().min(self.translated.len());
            let taken = self.tap.feed_and_take(chunk, &mut self.translated[..n]);
            if !self.tap.is_on() {
                self.duck = 1.0;
                continue;
            }
            let translated = &self.translated[..taken];
            match self.voice {
                Hear::Translation => {
                    chunk[..taken].copy_from_slice(translated);
                    chunk[taken..].fill(0.0);
                }
                _ => {
                    // Duck their voice while the translation speaks, then bring it back.
                    let speaking = translated.iter().any(|s| s.abs() > SIGNAL);
                    if speaking {
                        self.hold = DUCK_HOLD;
                    } else {
                        self.hold = self.hold.saturating_sub(1);
                    }
                    let target = if self.hold > 0 { DUCKED } else { 1.0 };
                    let step = if target < self.duck {
                        DUCK_DOWN
                    } else {
                        DUCK_UP
                    };
                    let from = self.duck;
                    self.duck += (target - self.duck) * step;
                    let len = chunk.len().max(1) as f32;
                    for (i, sample) in chunk.iter_mut().enumerate() {
                        let gain = from + (self.duck - from) * i as f32 / len;
                        let over = translated.get(i).copied().unwrap_or(0.0);
                        *sample = (*sample * gain + over).clamp(-1.0, 1.0);
                    }
                }
            }
        }
        if let (Some(queue), Some(resample)) = (self.queued.as_mut(), self.from_rate.as_mut()) {
            for sample in &self.cleaned {
                resample.push(*sample, |s| {
                    let _ = queue.push(s);
                });
            }
        }
        Ok(())
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

fn deep_filter(app: &AppHandle, limit: f32) -> std::result::Result<hush_core::DeepFilter, String> {
    if !hush_core::DEEP_FILTER_AVAILABLE {
        return Err(
            "Cleaning their voice needs DeepFilterNet 3, which this build does not have.".into(),
        );
    }
    let data_dir = app.path().app_data_dir().unwrap_or_default();
    let Some(path) = crate::model::find(&data_dir, crate::settings::Engine::DeepFilter) else {
        return Err("Download DeepFilterNet 3 in Settings to clean their voice.".into());
    };
    hush_core::DeepFilter::load_with_limit(&path, limit).map_err(|e| format!("{e}"))
}

fn open_capture(plan: &Plan, producer: Producer<f32>) -> Result<(Capture, u32)> {
    #[cfg(target_os = "macos")]
    if plan.mode == Mode::Speaker {
        let _ = producer;
        let ring = hush_core::virtual_speaker::VirtualSpeaker::open()?;
        return Ok((Capture::Ring(ring), hush_core::virtual_speaker::RATE));
    }
    let device = capture_device(plan)?;
    let config = device_config(&device, plan.loopback)?;
    let rate = config.sample_rate().0;
    let format = config.sample_format();
    let config: cpal::StreamConfig = config.into();
    macro_rules! build {
        ($t:ty) => {
            capture_stream::<$t>(&device, &config, producer)?
        };
    }
    let stream = match format {
        SampleFormat::F32 => build!(f32),
        SampleFormat::I16 => build!(i16),
        SampleFormat::I32 => build!(i32),
        SampleFormat::U16 => build!(u16),
        SampleFormat::F64 => build!(f64),
        SampleFormat::U8 => build!(u8),
        SampleFormat::I8 => build!(i8),
        other => return Err(eyre!("Unsupported audio format {other:?}")),
    };
    stream.play()?;
    Ok((Capture::Stream(stream), rate))
}

/// The device to capture: the virtual speaker or the speakers, by loopback on Windows; the
/// PulseAudio monitor on Linux.
fn capture_device(plan: &Plan) -> Result<cpal::Device> {
    #[cfg(target_os = "linux")]
    {
        crate::audio::pulse::capture_device(plan.mode == Mode::Speaker)
    }
    #[cfg(not(target_os = "linux"))]
    {
        match &plan.capture {
            // Loopback: an input stream on an output device.
            Some(name) => find_device(!plan.loopback, name),
            None => cpal::default_host()
                .default_output_device()
                .ok_or_else(|| eyre!("No speakers to listen to")),
        }
    }
}

fn device_config(device: &cpal::Device, loopback: bool) -> Result<cpal::SupportedStreamConfig> {
    // A loopback stream uses the output device's own format.
    Ok(if loopback {
        device.default_output_config()?
    } else {
        device.default_input_config()?
    })
}

fn capture_stream<T: SizedSample>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut producer: Producer<f32>,
) -> Result<Stream>
where
    f32: FromSample<T>,
{
    let channels = config.channels as usize;
    Ok(device.build_input_stream(
        config,
        move |data: &[T], _| {
            for frame in data.chunks_exact(channels) {
                let mono =
                    frame.iter().map(|s| f32::from_sample(*s)).sum::<f32>() / channels as f32;
                let _ = producer.push(mono);
            }
        },
        |error| log::warn!("Listen capture: {error}"),
        None,
    )?)
}

/// Play their voice: a stream that only empties a queue.
fn open_playback(device: &cpal::Device) -> Result<(Stream, u32, Producer<f32>)> {
    #[cfg(target_os = "linux")]
    let _guard = crate::audio::pulse::playback_env();
    let config = device.default_output_config()?;
    let rate = config.sample_rate().0;
    let format = config.sample_format();
    let config: cpal::StreamConfig = config.into();
    let (producer, consumer) = RingBuffer::new(rate as usize);
    let max = MAX_QUEUED * rate as usize / RATE as usize;
    macro_rules! build {
        ($t:ty) => {
            playback_stream::<$t>(device, &config, consumer, max)?
        };
    }
    let stream = match format {
        SampleFormat::F32 => build!(f32),
        SampleFormat::I16 => build!(i16),
        SampleFormat::I32 => build!(i32),
        SampleFormat::U16 => build!(u16),
        SampleFormat::F64 => build!(f64),
        SampleFormat::U8 => build!(u8),
        SampleFormat::I8 => build!(i8),
        other => return Err(eyre!("Unsupported audio format {other:?}")),
    };
    stream.play()?;
    Ok((stream, rate, producer))
}

fn playback_stream<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut queue: Consumer<f32>,
    max: usize,
) -> Result<Stream> {
    let channels = config.channels as usize;
    Ok(device.build_output_stream(
        config,
        move |data: &mut [T], _| {
            // Too much queued (the capture clock runs ahead): skip to stay in step.
            let extra = queue.slots().saturating_sub(max);
            for _ in 0..extra {
                let _ = queue.pop();
            }
            for frame in data.chunks_exact_mut(channels) {
                let sample = T::from_sample(queue.pop().unwrap_or(0.0));
                frame.fill(sample);
            }
        },
        |error| log::warn!("Listen playback: {error}"),
        None,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_virtual_speakers() {
        let names = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            virtual_speaker_names(&names(&[
                "Speakers (Realtek)",
                "CABLE Input (VB-Audio Virtual Cable)"
            ])),
            None
        );
        assert_eq!(
            virtual_speaker_names(&names(&[
                "CABLE-B Input (VB-Audio Cable B)",
                "CABLE-A Input (VB-Audio Cable A)"
            ]))
            .as_deref(),
            Some("CABLE-A Input (VB-Audio Cable A)")
        );
        assert_eq!(
            virtual_speaker_names(&names(&[
                "CABLE-A Input (VB-Audio Cable A)",
                "Hush AI Speaker"
            ]))
            .as_deref(),
            Some("Hush AI Speaker")
        );
    }
}
