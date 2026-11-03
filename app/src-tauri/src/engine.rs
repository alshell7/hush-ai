//! Noise cancellation on and off: one live session at a time, its meters, and recovery.
//!
//! The live session (`audio::live_io`) opens the microphone, runs the model on its own
//! thread and writes to the Hush AI Microphone, or to headphones while "Listen" is on.
//! This module starts and stops it, restarts it when settings change, retries after device
//! errors and sends state and meter events to the windows.
use crate::audio::audio_io::{devices, Message, Selection, Sender, Settings as AudioSettings};
use crate::audio::live_io::{self, control::StopReason, LiveSession, StopReport};
use crate::settings::Settings;
use hush_core::live::LiveMeters;
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Off,
    Starting,
    On,
    Error,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineState {
    pub status: Status,
    pub message: Option<String>,
    /// Output goes to headphones instead of the Hush AI Microphone.
    pub listen: bool,
    /// Listen to the unprocessed microphone (A/B).
    pub original: bool,
    pub threads: usize,
    pub input_rate: u32,
    pub model_rate: u32,
    pub output_rate: u32,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meters {
    /// Peak levels since the last reading, 0 to 1.
    pub input: f32,
    pub output: f32,
    /// Share of real time the model needs, 0 to 1 (above 1 cannot keep up).
    pub load: f32,
    pub dropped: u64,
}

struct Inner {
    session: Option<LiveSession>,
    id: u64,
    state: EngineState,
    meters: Option<LiveMeters>,
    /// Start again when the current session has closed.
    restart: bool,
    retries: u32,
}

pub struct Engine {
    app: AppHandle,
    inner: Mutex<Inner>,
    /// Windows that show meters. Meters are not read while this is 0.
    pub viewers: AtomicUsize,
    quitting: AtomicBool,
    /// The last session has released its devices.
    closed: AtomicBool,
}

/// Friendly text for a failed start.
fn explain(error: &str) -> String {
    let lower = error.to_lowercase();
    if lower.contains("in use by another hush") {
        "Another Hush AI window is using the Hush AI Microphone. Close it and try again.".into()
    } else if lower.contains("cable") && lower.contains("not found") {
        "Hush AI Microphone is not set up yet. Open Setup to finish it.".into()
    } else if lower.contains("cannot configure microphone") || lower.contains("input device") {
        "Your microphone is not available. Check that it is connected, or choose another one."
            .into()
    } else if lower.contains("safetensors")
        || lower.contains("model") && lower.contains("no such file")
    {
        "The voice model is missing. Download it in Settings.".into()
    } else {
        error.lines().next().unwrap_or(error).to_string()
    }
}

fn reason_text(reason: StopReason) -> &'static str {
    match reason {
        StopReason::Requested => "Stopped.",
        StopReason::InputUnavailable | StopReason::InputError => {
            "Your microphone stopped. Hush AI will try again in a moment."
        }
        StopReason::OutputUnavailable | StopReason::OutputError => {
            "The audio output stopped. Hush AI will try again in a moment."
        }
        StopReason::WorkerFailed => "Processing stopped unexpectedly. Hush AI will restart it.",
    }
}

impl Engine {
    pub fn new(app: AppHandle) -> Arc<Self> {
        let engine = Arc::new(Self {
            app,
            inner: Mutex::new(Inner {
                session: None,
                id: 0,
                state: EngineState {
                    status: Status::Off,
                    message: None,
                    listen: false,
                    original: false,
                    threads: 0,
                    input_rate: 0,
                    model_rate: 0,
                    output_rate: 0,
                },
                meters: None,
                restart: false,
                retries: 0,
            }),
            viewers: AtomicUsize::new(0),
            quitting: AtomicBool::new(false),
            closed: AtomicBool::new(true),
        });
        let weak = Arc::downgrade(&engine);
        thread::Builder::new()
            .name("hush-meters".into())
            .spawn(move || meter_loop(weak))
            .expect("meter thread");
        engine
    }

    pub fn state(&self) -> EngineState {
        self.inner.lock().unwrap().state.clone()
    }

    pub fn is_running(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.session.is_some()
    }

    fn publish(&self, inner: &Inner) {
        let _ = self.app.emit("engine", &inner.state);
        crate::tray::update(&self.app, inner.state.status);
    }

    fn sender(&self) -> Sender {
        let app = self.app.clone();
        Sender::new(move |message| {
            if let Some(engine) = app.try_state::<Arc<Engine>>() {
                engine.on_message(message);
            }
        })
    }

    /// Start, or restart with the current settings.
    pub fn start(&self) {
        let mut inner = self.inner.lock().unwrap();
        if self.quitting.load(Ordering::Relaxed) {
            return;
        }
        if inner.session.is_some() {
            inner.restart = true;
            let id = inner.id;
            if let Some(session) = &inner.session {
                session.request_stop(self.sender(), id);
            }
            return;
        }
        self.launch(&mut inner);
    }

    fn launch(&self, inner: &mut Inner) {
        inner.restart = false;
        inner.id += 1;
        inner.meters = None;
        let app = &self.app;
        let settings = app.state::<crate::AppState>().settings();
        let data_dir = app.path().app_data_dir().unwrap_or_default();
        let Some(model) = crate::model::find(&data_dir) else {
            inner.state.status = Status::Error;
            inner.state.message = Some("Download the voice model to turn on Hush AI.".into());
            self.publish(inner);
            return;
        };
        let (input, notice) = input_selection(&settings);
        let output = output_selection(&settings);
        let audio = audio_settings(&settings, model, !inner.state.listen);
        inner.state.status = Status::Starting;
        inner.state.message = notice;
        inner.state.original = false;
        self.closed.store(false, Ordering::Relaxed);
        match live_io::start(input, output, audio, self.sender(), inner.id) {
            Ok(session) => inner.session = Some(session),
            Err(error) => {
                inner.state.status = Status::Error;
                inner.state.message = Some(explain(&format!("{error:#}")));
            }
        }
        self.publish(inner);
    }

    /// Turn off. The session fades out and closes its devices on its own thread.
    pub fn stop(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.restart = false;
        inner.retries = 0;
        let id = inner.id;
        if let Some(session) = &inner.session {
            session.request_stop(self.sender(), id);
        } else {
            inner.state.status = Status::Off;
            inner.state.message = None;
            self.publish(&inner);
        }
    }

    /// Send output to headphones (true) or to the Hush AI Microphone (false).
    pub fn set_listen(&self, listen: bool) {
        let running = {
            let mut inner = self.inner.lock().unwrap();
            if inner.state.listen == listen {
                return;
            }
            inner.state.listen = listen;
            self.publish(&inner);
            inner.session.is_some()
        };
        if running || listen {
            self.start();
        }
    }

    pub fn set_original(&self, original: bool) {
        let mut inner = self.inner.lock().unwrap();
        inner.state.original = original;
        if let Some(meters) = &inner.meters {
            meters.set_original(original);
        }
        self.publish(&inner);
    }

    /// Stop for good before the app exits, and wait briefly for the devices to close.
    pub fn shutdown(&self) {
        self.quitting.store(true, Ordering::Relaxed);
        let session = self.inner.lock().unwrap().session.take();
        if session.is_none() {
            return;
        }
        drop(session);
        let deadline = Instant::now() + Duration::from_millis(600);
        while Instant::now() < deadline && !self.closed.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn on_message(&self, message: Message) {
        let mut inner = self.inner.lock().unwrap();
        match message {
            Message::LiveStarted(id, result) if id == inner.id => match result {
                Ok((rates, meters)) => {
                    meters.set_original(inner.state.original);
                    inner.meters = Some(meters);
                    inner.retries = 0;
                    let state = &mut inner.state;
                    state.status = Status::On;
                    state.threads = rates.threads;
                    state.input_rate = rates.input;
                    state.model_rate = rates.model;
                    state.output_rate = rates.output;
                    self.publish(&inner);
                }
                Err(error) => {
                    inner.session = None;
                    inner.meters = None;
                    inner.state.status = Status::Error;
                    inner.state.message = Some(explain(&error));
                    self.publish(&inner);
                    drop(inner);
                    self.retry_later();
                }
            },
            Message::LiveStopped(id, report) if id == inner.id => {
                if !report.devices_closed {
                    return;
                }
                self.closed.store(true, Ordering::Relaxed);
                inner.session = None;
                inner.meters = None;
                if inner.restart && !self.quitting.load(Ordering::Relaxed) {
                    self.launch(&mut inner);
                    return;
                }
                self.stopped(inner, report);
            }
            _ => {}
        }
    }

    fn stopped(&self, mut inner: std::sync::MutexGuard<'_, Inner>, report: StopReport) {
        if let Some(stats) = report.stats {
            log::info!(
                "Stopped ({:?}): {} frames, {} input and {} output samples dropped, {} underrun",
                report.reason,
                stats.frames_processed,
                stats.input_dropped_samples,
                stats.output_dropped_samples,
                stats.output_underrun_samples
            );
        }
        if report.reason == StopReason::Requested {
            inner.state.status = Status::Off;
            inner.state.message = None;
            // "Listen" ends with the session.
            inner.state.listen = false;
            self.publish(&inner);
            return;
        }
        inner.state.status = Status::Error;
        inner.state.message = Some(reason_text(report.reason).into());
        self.publish(&inner);
        drop(inner);
        self.retry_later();
    }

    /// After a device error, try again while noise cancellation is still wanted:
    /// after 2, 4 and 8 seconds, then every 15 seconds.
    fn retry_later(&self) {
        let delay = {
            let mut inner = self.inner.lock().unwrap();
            inner.retries += 1;
            Duration::from_secs(match inner.retries {
                1 => 2,
                2 => 4,
                3 => 8,
                _ => 15,
            })
        };
        let app = self.app.clone();
        thread::spawn(move || {
            thread::sleep(delay);
            let Some(engine) = app.try_state::<Arc<Engine>>() else {
                return;
            };
            let wanted = app.state::<crate::AppState>().settings().enabled;
            let state = engine.state();
            if wanted && state.status == Status::Error && !engine.is_running() {
                engine.start();
            }
        });
    }
}

fn input_selection(settings: &Settings) -> (Selection, Option<String>) {
    let Some(name) = &settings.input else {
        return (Selection::Default, None);
    };
    match devices::discover(true).and_then(|list| list.named(name)) {
        Ok(selection) => (selection, None),
        Err(_) => (
            Selection::Default,
            Some(format!(
                "{name} is not connected. Using the system microphone."
            )),
        ),
    }
}

fn output_selection(settings: &Settings) -> Selection {
    settings
        .output
        .as_ref()
        .and_then(|name| devices::discover(false).and_then(|l| l.named(name)).ok())
        .unwrap_or(Selection::Default)
}

pub fn audio_settings(settings: &Settings, model: PathBuf, virtual_mic: bool) -> AudioSettings {
    let (rate, layers) = settings.quality.model();
    AudioSettings {
        model,
        rate,
        layers,
        look_ahead: 0,
        threads: settings.threads,
        placement: hush_core::WorkerPlacement::Unrestricted,
        scheduling: if cfg!(target_os = "windows") && settings.audio_priority {
            hush_core::WorkerScheduling::WindowsAudio
        } else {
            hush_core::WorkerScheduling::OsDefault
        },
        bypass: false,
        prefill: 1,
        virtual_mic,
    }
}

/// Reads meters 20 times per second while a window shows them; otherwise sleeps.
fn meter_loop(engine: std::sync::Weak<Engine>) {
    let mut last: Option<(u64, Instant)> = None;
    loop {
        let Some(engine) = engine.upgrade() else {
            return;
        };
        if engine.viewers.load(Ordering::Relaxed) == 0 {
            last = None;
            drop(engine);
            thread::sleep(Duration::from_millis(250));
            continue;
        }
        let meters = engine.inner.lock().unwrap().meters.clone();
        if let Some(meters) = meters {
            let reading = meters.take();
            let now = Instant::now();
            let load = match last {
                Some((busy, at)) if now > at => {
                    (reading.busy_ns.saturating_sub(busy) as f64
                        / now.duration_since(at).as_nanos() as f64) as f32
                }
                _ => 0.0,
            };
            last = Some((reading.busy_ns, now));
            let _ = engine.app.emit(
                "meters",
                Meters {
                    input: reading.input_peak.min(1.0),
                    output: reading.output_peak.min(1.0),
                    load,
                    dropped: reading.stats.input_dropped_samples
                        + reading.stats.output_underrun_samples,
                },
            );
        } else {
            last = None;
        }
        drop(engine);
        thread::sleep(Duration::from_millis(50));
    }
}
