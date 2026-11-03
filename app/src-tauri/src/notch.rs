//! The notch: Hush's quick controls at the top of the screen, like the iPhone's Dynamic Island.
//!
//! One transparent, always-on-top window covers the top centre of the main screen. Only the
//! pill the page draws takes the mouse: the page reports the pill's rectangle, and a light
//! poll of the cursor (while the window shows) lets clicks pass through everywhere else and
//! tells the page when the cursor comes near, so it can expand. On a Mac with a camera notch
//! the pill sits around it, above the menu bar.
use serde::Serialize;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    thread,
    time::Duration,
};
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

pub const LABEL: &str = "notch";
/// The window, in logical pixels: room for the expanded notch and its shadow.
const WIDTH: f64 = 720.0;
const HEIGHT: f64 = 440.0;

/// The screen's top edge, for the page to shape the pill.
#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Geometry {
    /// A camera notch in the screen (MacBooks), and its size in logical pixels.
    pub has_notch: bool,
    pub notch_width: f64,
    pub notch_height: f64,
    /// The macOS menu bar's height; 0 elsewhere.
    pub menu_bar: f64,
}

#[derive(Clone, Copy, Debug, Default)]
struct Region {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

pub struct Notch {
    /// The pill, in logical pixels inside the window.
    region: Mutex<Region>,
    hovered: AtomicBool,
    ignoring: AtomicBool,
    geometry: Mutex<Geometry>,
}

impl Notch {
    fn new() -> Self {
        Self {
            region: Mutex::new(Region::default()),
            hovered: AtomicBool::new(false),
            ignoring: AtomicBool::new(false),
            geometry: Mutex::new(Geometry::default()),
        }
    }
}

/// Create the notch window (hidden) and the cursor poll. Call once at start-up.
pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    app.manage(Notch::new());
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html#notch".into()))
        .title("Hush AI")
        .inner_size(WIDTH, HEIGHT)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .visible_on_all_workspaces(true)
        .build()?;
    place(&window);
    let handle = app.clone();
    thread::Builder::new()
        .name("hush-notch".into())
        .spawn(move || poll(handle))?;
    Ok(())
}

/// Top centre of the main screen.
fn place(window: &WebviewWindow) {
    let Some(monitor) = window
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| window.current_monitor().ok().flatten())
    else {
        return;
    };
    let scale = monitor.scale_factor();
    let position = monitor.position().to_logical::<f64>(scale);
    let size = monitor.size().to_logical::<f64>(scale);
    let _ = window.set_size(LogicalSize::new(WIDTH, HEIGHT));
    let _ = window.set_position(LogicalPosition::new(
        position.x + (size.width - WIDTH) / 2.0,
        position.y,
    ));
    #[cfg(target_os = "macos")]
    mac::prepare(window);
}

/// Show or hide the notch and the widget for the settings, and keep windows out of screen
/// captures if asked.
pub fn apply(app: &AppHandle) {
    let settings = app.state::<crate::AppState>().settings();
    let notch = settings.overlay == crate::settings::Overlay::Notch && settings.onboarded;
    if let Some(window) = app.get_webview_window(LABEL) {
        if notch {
            if !window.is_visible().unwrap_or(false) {
                place(&window);
                let _ = window.set_ignore_cursor_events(true);
                app.state::<Notch>().ignoring.store(true, Ordering::Release);
                let _ = window.show();
            }
        } else {
            let _ = window.hide();
        }
    }
    for label in ["main", "widget", LABEL] {
        if let Some(window) = app.get_webview_window(label) {
            let _ = window.set_content_protected(settings.hide_from_capture);
        }
    }
}

/// Tray click in notch mode: open (or close) the notch as if hovered.
pub fn toggle_pin(app: &AppHandle) {
    let _ = app.emit_to(LABEL, "notch-pin", ());
}

/// Follow the cursor while the notch shows: pass clicks through outside the pill, and tell the
/// page when the cursor is near it.
fn poll(app: AppHandle) {
    let notch = app.state::<Notch>();
    loop {
        let Some(window) = app.get_webview_window(LABEL) else {
            thread::sleep(Duration::from_millis(500));
            continue;
        };
        if !window.is_visible().unwrap_or(false) {
            thread::sleep(Duration::from_millis(300));
            continue;
        }
        let inside = (|| {
            let cursor = app.cursor_position().ok()?;
            let origin = window.outer_position().ok()?;
            let scale = window.scale_factor().ok()?;
            let region = *notch.region.lock().unwrap();
            // A little margin so the pill wakes up just before the cursor reaches it.
            let margin = 6.0;
            let x = (cursor.x - f64::from(origin.x)) / scale;
            let y = (cursor.y - f64::from(origin.y)) / scale;
            Some(
                x >= region.x - margin
                    && x <= region.x + region.width + margin
                    && y >= region.y - margin
                    && y <= region.y + region.height + margin,
            )
        })()
        .unwrap_or(false);
        if notch.hovered.swap(inside, Ordering::AcqRel) != inside {
            let _ = window.emit_to(LABEL, "notch-hover", inside);
        }
        if notch.ignoring.swap(!inside, Ordering::AcqRel) == inside {
            let _ = window.set_ignore_cursor_events(!inside);
        }
        thread::sleep(Duration::from_millis(if inside { 40 } else { 60 }));
    }
}

/// The pill's rectangle inside the notch window, in logical pixels.
#[tauri::command]
pub fn notch_region(app: AppHandle, x: f64, y: f64, width: f64, height: f64) {
    *app.state::<Notch>().region.lock().unwrap() = Region {
        x,
        y,
        width,
        height,
    };
}

#[tauri::command]
pub fn notch_geometry(app: AppHandle) -> Geometry {
    app.state::<Notch>().geometry.lock().unwrap().clone()
}

#[cfg(target_os = "macos")]
mod mac {
    use super::{Geometry, Notch, LABEL};
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSScreen, NSWindow, NSWindowCollectionBehavior};
    use tauri::{Emitter, Manager, WebviewWindow};

    /// Above the menu bar, on every Space and over full-screen apps; measures the screen's
    /// notch. Runs on the main thread later, never blocking the caller (which may be it).
    pub fn prepare(window: &WebviewWindow) {
        let target = window.clone();
        let _ = window.run_on_main_thread(move || {
            let mut geometry = Geometry::default();
            if let Ok(pointer) = target.ns_window() {
                // SAFETY: Tauri hands out its live NSWindow; we are on the main thread.
                let ns: &NSWindow = unsafe { &*(pointer as *const NSWindow) };
                // NSMainMenuWindowLevel + 1: above the menu bar.
                ns.setLevel(25);
                ns.setCollectionBehavior(
                    NSWindowCollectionBehavior::CanJoinAllSpaces
                        | NSWindowCollectionBehavior::Stationary
                        | NSWindowCollectionBehavior::FullScreenAuxiliary
                        | NSWindowCollectionBehavior::IgnoresCycle,
                );
                ns.setHasShadow(false);
            }
            if let Some(mtm) = MainThreadMarker::new() {
                if let Some(screen) = NSScreen::mainScreen(mtm) {
                    let frame = screen.frame();
                    let visible = screen.visibleFrame();
                    geometry.menu_bar = (frame.origin.y + frame.size.height)
                        - (visible.origin.y + visible.size.height);
                    let insets = screen.safeAreaInsets();
                    if insets.top > 0.0 {
                        let left = screen.auxiliaryTopLeftArea();
                        let right = screen.auxiliaryTopRightArea();
                        geometry.has_notch = true;
                        geometry.notch_height = insets.top;
                        geometry.notch_width =
                            frame.size.width - left.size.width - right.size.width;
                    }
                }
            }
            let app = target.app_handle();
            *app.state::<Notch>().geometry.lock().unwrap() = geometry.clone();
            let _ = app.emit_to(LABEL, "notch-geometry", geometry);
        });
    }
}
