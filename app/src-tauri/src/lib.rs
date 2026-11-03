//! Hush AI desktop app: Tauri commands, windows, tray and start-up.
mod audio;
mod calendar;
mod engine;
mod mictest;
mod model;
mod settings;
mod studio;
mod tray;
mod vmic;

use calendar::{Calendar, CalendarInfo};
use engine::{Engine, EngineState, Status};
use mictest::MicTest;
use serde::Serialize;
use settings::Settings;
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use studio::Studio;
use tauri::{AppHandle, Emitter, Manager, RunEvent, WindowEvent};

/// Launched at login: stay in the tray.
const BACKGROUND_ARG: &str = "--background";

pub struct AppState {
    settings: Mutex<Settings>,
    path: PathBuf,
    data_dir: PathBuf,
    download: Mutex<Option<Arc<AtomicBool>>>,
    viewers: Mutex<HashSet<String>>,
    background: bool,
}

impl AppState {
    pub fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    fn update(&self, change: impl FnOnce(&mut Settings)) -> Settings {
        let mut settings = self.settings.lock().unwrap();
        change(&mut settings);
        if let Err(error) = settings.save(&self.path) {
            log::warn!("Cannot save settings: {error:#}");
        }
        settings.clone()
    }
}

type Reply<T> = Result<T, String>;

fn text(error: eyre::Report) -> String {
    format!("{error:#}")
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    settings: Settings,
    model: model::ModelInfo,
    engine: EngineState,
    calendar: CalendarInfo,
    platform: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
struct Device {
    /// `None` is the system default.
    name: Option<String>,
    label: String,
}

#[derive(Serialize)]
struct Devices {
    inputs: Vec<Device>,
    outputs: Vec<Device>,
}

pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
    if let Some(window) = app.get_webview_window("widget") {
        let _ = window.hide();
    }
}

/// Turn noise cancellation on or off, and remember it.
pub fn set_enabled(app: &AppHandle, on: bool) {
    let settings = app.state::<AppState>().update(|s| s.enabled = on);
    let _ = app.emit("settings", &settings);
    let engine = app.state::<Arc<Engine>>().inner().clone();
    if on {
        app.state::<MicTest>().stop_monitor();
        std::thread::spawn(move || engine.start());
    } else {
        engine.stop();
    }
}

#[tauri::command]
fn snapshot(app: AppHandle, state: tauri::State<AppState>) -> Snapshot {
    Snapshot {
        settings: state.settings(),
        model: model::info(&state.data_dir),
        engine: app.state::<Arc<Engine>>().state(),
        calendar: app.state::<Arc<Calendar>>().info(),
        platform: std::env::consts::OS,
        version: env!("CARGO_PKG_VERSION"),
    }
}

/// The frontend has drawn its first frame: show the main window without a white flash.
#[tauri::command]
fn app_ready(app: AppHandle, state: tauri::State<AppState>) {
    if !state.background || !state.settings().onboarded {
        show_main(&app);
    }
}

#[tauri::command]
async fn devices() -> Reply<Devices> {
    tauri::async_runtime::spawn_blocking(|| {
        let list = |input: bool| -> eyre::Result<Vec<Device>> {
            let found = audio::audio_io::devices::discover(input)?;
            let mut devices: Vec<Device> = found
                .names()
                .into_iter()
                // Never offer Hush's own devices as a microphone or headphones.
                .filter(|name| {
                    !name.starts_with("Hush AI Microphone") && !name.starts_with("CABLE")
                })
                .map(|name| Device {
                    name: Some(name.to_string()),
                    label: name.to_string(),
                })
                .collect();
            devices.insert(
                0,
                Device {
                    name: None,
                    label: "System default".into(),
                },
            );
            Ok(devices)
        };
        Ok(Devices {
            inputs: list(true).map_err(text)?,
            outputs: list(false).map_err(text)?,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn update_settings(app: AppHandle, patch: serde_json::Value) -> Reply<Settings> {
    let state = app.state::<AppState>();
    let before = state.settings();
    let mut merged = serde_json::to_value(&before).map_err(|e| e.to_string())?;
    if let (Some(target), Some(source)) = (merged.as_object_mut(), patch.as_object()) {
        for (key, value) in source {
            target.insert(key.clone(), value.clone());
        }
    }
    let next: Settings = serde_json::from_value(merged).map_err(|e| e.to_string())?;
    let saved = state.update(|s| *s = next.clone());
    let _ = app.emit("settings", &saved);
    let restart = before.input != saved.input
        || before.quality != saved.quality
        || before.threads != saved.threads
        || before.audio_priority != saved.audio_priority;
    if before.calendar_sync != saved.calendar_sync {
        app.state::<Arc<Calendar>>().refresh();
    }
    if before.enabled != saved.enabled {
        set_enabled(&app, saved.enabled);
    } else if restart && app.state::<Arc<Engine>>().is_running() {
        let engine = app.state::<Arc<Engine>>().inner().clone();
        std::thread::spawn(move || engine.start());
    }
    Ok(saved)
}

#[tauri::command]
fn set_enabled_command(app: AppHandle, on: bool) {
    set_enabled(&app, on);
}

/// A window starts or stops showing meters.
#[tauri::command]
fn watch_meters(
    window: tauri::Window,
    state: tauri::State<AppState>,
    engine: tauri::State<Arc<Engine>>,
    on: bool,
) {
    let mut viewers = state.viewers.lock().unwrap();
    if on {
        viewers.insert(window.label().to_string());
    } else {
        viewers.remove(window.label());
    }
    engine.viewers.store(viewers.len(), Ordering::Relaxed);
}

#[tauri::command]
async fn model_download(app: AppHandle) -> Reply<model::ModelInfo> {
    let state = app.state::<AppState>();
    let cancel = Arc::new(AtomicBool::new(false));
    {
        // One download at a time: a second one would race on the same file.
        let mut slot = state.download.lock().unwrap();
        if slot.is_some() {
            return Err("The voice model is already downloading".into());
        }
        *slot = Some(cancel.clone());
    }
    let data_dir = state.data_dir.clone();
    let events = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        model::download(&data_dir, &cancel, |progress| {
            let _ = events.emit("model-progress", progress);
        })
    })
    .await;
    *state.download.lock().unwrap() = None;
    result.map_err(|e| e.to_string())?.map_err(text)?;
    Ok(model::info(&state.data_dir))
}

#[tauri::command]
fn model_cancel(state: tauri::State<AppState>) {
    if let Some(cancel) = state.download.lock().unwrap().as_ref() {
        cancel.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
fn model_remove(app: AppHandle) -> Reply<model::ModelInfo> {
    app.state::<Arc<Engine>>().stop();
    let state = app.state::<AppState>();
    model::remove(&state.data_dir).map_err(text)?;
    Ok(model::info(&state.data_dir))
}

#[tauri::command]
async fn vmic_status(app: AppHandle) -> Reply<vmic::Info> {
    let running = app.state::<Arc<Engine>>().is_running();
    tauri::async_runtime::spawn_blocking(move || vmic::status(running))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn vmic_install(app: AppHandle) -> Reply<vmic::Info> {
    let resources = app.path().resource_dir().unwrap_or_default();
    let engine = app.state::<Arc<Engine>>().inner().clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        vmic::install(&resources)?;
        Ok::<_, eyre::Report>(vmic::status(engine.is_running()))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(text)?;
    // A new microphone may need a fresh session.
    if result.state == vmic::State::Ready && app.state::<AppState>().settings().enabled {
        let engine = app.state::<Arc<Engine>>().inner().clone();
        if engine.state().status == Status::Error {
            std::thread::spawn(move || engine.start());
        }
    }
    Ok(result)
}

#[tauri::command]
async fn vmic_uninstall(app: AppHandle) -> Reply<vmic::Info> {
    app.state::<Arc<Engine>>().stop();
    tauri::async_runtime::spawn_blocking(|| {
        vmic::uninstall()?;
        Ok::<_, eyre::Report>(vmic::status(false))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(text)
}

#[tauri::command]
fn mic_monitor(app: AppHandle, on: bool) -> Reply<()> {
    let test = app.state::<MicTest>();
    if on && !app.state::<Arc<Engine>>().is_running() {
        let input = app.state::<AppState>().settings().input;
        test.start_monitor(&app, input).map_err(text)
    } else {
        test.stop_monitor();
        Ok(())
    }
}

#[tauri::command]
fn test_record(app: AppHandle, seconds: f32) -> Reply<()> {
    app.state::<MicTest>()
        .record(&app, seconds.clamp(1.0, 15.0))
        .map_err(text)
}

#[tauri::command]
fn test_play(app: AppHandle, enhanced: bool) -> Reply<()> {
    app.state::<MicTest>().play(&app, enhanced).map_err(text)
}

#[tauri::command]
fn test_stop(test: tauri::State<MicTest>) {
    test.stop();
}

/// Studio: enhance a file the user dropped or chose.
#[tauri::command]
fn studio_open(app: AppHandle, path: String) {
    app.state::<Studio>().open(&app, PathBuf::from(path), None);
}

/// Studio: record from the microphone; `studio_stop` ends the recording and enhances it.
#[tauri::command]
fn studio_record(app: AppHandle) -> Reply<()> {
    app.state::<Studio>().record(&app).map_err(text)
}

#[tauri::command]
fn studio_stop(studio: tauri::State<Studio>) {
    studio.stop_recording();
}

#[tauri::command]
fn studio_cancel(app: AppHandle) {
    app.state::<Studio>().cancel(&app);
}

#[tauri::command]
async fn studio_save(app: AppHandle, path: String) -> Reply<()> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<Studio>().save(std::path::Path::new(&path))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(text)
}

#[tauri::command]
fn calendar_info(calendar: tauri::State<Arc<Calendar>>) -> CalendarInfo {
    calendar.info()
}

#[tauri::command]
async fn calendar_connect(app: AppHandle, address: String) -> Reply<CalendarInfo> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<Arc<Calendar>>().connect(&app, &address)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

/// What an address holds, without keeping it.
#[tauri::command]
async fn calendar_check(address: String) -> Reply<calendar::Preview> {
    tauri::async_runtime::spawn_blocking(move || Calendar::check(&address))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Open Google Calendar's settings, where the secret address is.
#[tauri::command]
fn calendar_open_settings(app: AppHandle) -> Reply<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(
            "https://calendar.google.com/calendar/u/0/r/settings",
            None::<&str>,
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn calendar_disconnect(app: AppHandle) -> CalendarInfo {
    app.state::<Arc<Calendar>>().disconnect(&app)
}

#[tauri::command]
fn calendar_refresh(calendar: tauri::State<Arc<Calendar>>) {
    calendar.refresh();
}

/// Open a meeting's call link. Only links read from the calendar can be opened.
#[tauri::command]
fn calendar_join(app: AppHandle, id: String) -> Reply<()> {
    use tauri_plugin_opener::OpenerExt;
    let link = app
        .state::<Arc<Calendar>>()
        .link(&id)
        .ok_or("This meeting has no call link")?;
    app.opener()
        .open_url(link, None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn open_main(app: AppHandle) {
    show_main(&app);
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

/// `HUSH_SELFTEST=1`: the frontend drives an end-to-end check and reports here.
#[tauri::command]
fn selftest_mode() -> bool {
    std::env::var_os("HUSH_SELFTEST").is_some()
}

/// Self-test inputs: a noisy WAV for Studio, where to save its result, and the calendar
/// address from `HUSH_TEST_ICAL`. Only in self-test mode.
#[tauri::command]
fn selftest_fixture(app: AppHandle, kind: String) -> Reply<Option<String>> {
    if std::env::var_os("HUSH_SELFTEST").is_none() {
        return Err("Only in self-test mode".into());
    }
    let dir = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    match kind.as_str() {
        "studio" => {
            // 4 s of 44.1 kHz stereo: a voice-like tone that comes and goes, over noise.
            let path = dir.join("selftest-input.wav");
            let spec = hound::WavSpec {
                channels: 2,
                sample_rate: 44_100,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };
            let mut writer = hound::WavWriter::create(&path, spec).map_err(|e| e.to_string())?;
            let mut seed = 0x2545_f491_u32;
            for i in 0..44_100 * 4 {
                let t = i as f32 / 44_100.0;
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let noise = (seed as f32 / u32::MAX as f32 - 0.5) * 0.2;
                let voice = (t * 2.0 * std::f32::consts::PI * 180.0).sin()
                    * (t * std::f32::consts::PI * 2.0).sin().max(0.0)
                    * 0.4;
                let sample = ((voice + noise) * i16::MAX as f32) as i16;
                writer.write_sample(sample).map_err(|e| e.to_string())?;
                writer.write_sample(sample).map_err(|e| e.to_string())?;
            }
            writer.finalize().map_err(|e| e.to_string())?;
            Ok(Some(path.display().to_string()))
        }
        "save" => Ok(Some(dir.join("selftest-clean.wav").display().to_string())),
        "ical" => Ok(std::env::var("HUSH_TEST_ICAL").ok().filter(|v| !v.is_empty())),
        _ => Err(format!("Unknown fixture {kind}")),
    }
}

#[tauri::command]
fn selftest_report(app: AppHandle, line: String) {
    println!("[selftest] {line}");
    if line == "DONE" {
        app.exit(0);
    }
}

pub fn run() {
    // The administrator helper that renames VB-CABLE (started by Hush itself through UAC).
    #[cfg(target_os = "windows")]
    {
        let args: Vec<String> = std::env::args().collect();
        if let Some(index) = args.iter().position(|a| a == "--vb-cable") {
            let action = args.get(index + 1).map_or("setup", String::as_str);
            std::process::exit(audio::live_io::vb_cable::elevated_main(action));
        }
    }
    let background = std::env::args().any(|a| a == BACKGROUND_ARG);

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![BACKGROUND_ARG]),
        ))
        .setup(move |app| {
            let handle = app.handle();
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            let path = config_dir.join("settings.json");
            let settings = Settings::load(&path);
            let start = settings.onboarded && settings.enabled;
            app.manage(AppState {
                settings: Mutex::new(settings),
                path,
                data_dir,
                download: Mutex::new(None),
                viewers: Mutex::new(HashSet::new()),
                background,
            });
            app.manage(MicTest::default());
            app.manage(Studio::default());
            Studio::startup(handle);
            app.manage(Calendar::new(handle));
            let engine = Engine::new(handle.clone());
            app.manage(engine.clone());
            tray::create(handle)?;
            if start {
                std::thread::spawn(move || engine.start());
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } if window.label() == "main" => {
                api.prevent_close();
                let _ = window.hide();
                let app = window.app_handle();
                let state = app.state::<AppState>();
                if !state.settings().tray_hint_shown {
                    state.update(|s| s.tray_hint_shown = true);
                    let _ = app.emit("tray-hint", ());
                    tray::show_hint(app);
                }
            }
            WindowEvent::Focused(false) if window.label() == "widget" => {
                tray::widget_blurred(window.app_handle());
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            app_ready,
            devices,
            update_settings,
            set_enabled_command,
            watch_meters,
            model_download,
            model_cancel,
            model_remove,
            vmic_status,
            vmic_install,
            vmic_uninstall,
            mic_monitor,
            test_record,
            test_play,
            test_stop,
            studio_open,
            studio_record,
            studio_stop,
            studio_cancel,
            studio_save,
            calendar_info,
            calendar_check,
            calendar_connect,
            calendar_open_settings,
            calendar_disconnect,
            calendar_refresh,
            calendar_join,
            open_main,
            quit,
            selftest_mode,
            selftest_fixture,
            selftest_report,
        ])
        .build(tauri::generate_context!())
        .expect("Hush AI could not start")
        .run(|app, event| match event {
            #[cfg(target_os = "macos")]
            RunEvent::Reopen { .. } => show_main(app),
            RunEvent::Exit => {
                app.state::<Arc<Engine>>().shutdown();
                #[cfg(target_os = "linux")]
                audio::pulse::remove();
            }
            _ => {}
        });
}
