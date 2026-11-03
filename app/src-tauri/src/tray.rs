//! Menu bar (macOS) and notification area (Windows, Linux) icon, its menu and the widget.
use crate::engine::{Engine, Status};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, PhysicalPosition, Rect, Wry,
};

const TRAY: &str = "hush";

pub struct TrayItems {
    toggle: CheckMenuItem<Wry>,
    /// When the widget last hid because it lost focus. A click on the icon right after
    /// that must not show it again.
    hidden_at: Mutex<Option<Instant>>,
}

fn icon(on: bool) -> Image<'static> {
    let bytes: &'static [u8] = match (cfg!(target_os = "macos"), on) {
        (true, true) => include_bytes!("../icons/tray-on-template.png"),
        (true, false) => include_bytes!("../icons/tray-off-template.png"),
        (false, true) => include_bytes!("../icons/tray-on.png"),
        (false, false) => include_bytes!("../icons/tray-off.png"),
    };
    Image::from_bytes(bytes).expect("tray icon")
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let toggle = CheckMenuItem::with_id(
        app,
        "toggle",
        "Noise cancellation",
        true,
        false,
        None::<&str>,
    )?;
    let open = MenuItem::with_id(app, "open", "Open Hush AI", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Hush AI", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&toggle, &open, &PredefinedMenuItem::separator(app)?, &quit],
    )?;
    TrayIconBuilder::with_id(TRAY)
        .icon(icon(false))
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("Hush AI: off")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => {
                let on = app.state::<Arc<Engine>>().state().status != Status::Off;
                crate::set_enabled(app, !on);
            }
            "open" => crate::show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                toggle_widget(tray.app_handle(), rect);
            }
        })
        .build(app)?;
    app.manage(TrayItems {
        toggle,
        hidden_at: Mutex::new(None),
    });
    Ok(())
}

/// Show the state in the icon, its tooltip and the menu.
pub fn update(app: &AppHandle, status: Status) {
    let on = matches!(status, Status::On | Status::Starting);
    if let Some(tray) = app.tray_by_id(TRAY) {
        let _ = tray.set_icon(Some(icon(on)));
        let _ = tray.set_icon_as_template(cfg!(target_os = "macos"));
        let _ = tray.set_tooltip(Some(match status {
            Status::On => "Hush AI: on",
            Status::Starting => "Hush AI: starting",
            Status::Error => "Hush AI: needs attention",
            Status::Off => "Hush AI: off",
        }));
    }
    if let Some(items) = app.try_state::<TrayItems>() {
        let _ = items.toggle.set_checked(status != Status::Off);
    }
}

/// Show the widget at the tray icon with a hint, the first time the main window closes.
pub fn show_hint(app: &AppHandle) {
    show_widget(app);
}

/// Open the widget at the tray icon unless it is already open.
pub fn show_widget(app: &AppHandle) {
    let Some(rect) = app
        .tray_by_id(TRAY)
        .and_then(|tray| tray.rect().ok().flatten())
    else {
        return;
    };
    if let Some(window) = app.get_webview_window("widget") {
        if !window.is_visible().unwrap_or(false) {
            toggle_widget(app, rect);
        }
    }
}

/// The widget lost focus: hide it.
pub fn widget_blurred(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("widget") {
        let _ = window.hide();
    }
    if let Some(items) = app.try_state::<TrayItems>() {
        *items.hidden_at.lock().unwrap() = Some(Instant::now());
    }
}

/// Open the widget next to the icon: below it when the icon is at the top of the screen
/// (macOS menu bar), above it otherwise (Windows taskbar).
fn toggle_widget(app: &AppHandle, rect: Rect) {
    let Some(window) = app.get_webview_window("widget") else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }
    if let Some(items) = app.try_state::<TrayItems>() {
        let recent = items
            .hidden_at
            .lock()
            .unwrap()
            .is_some_and(|at| at.elapsed() < Duration::from_millis(300));
        if recent {
            return;
        }
    }
    let scale = window.scale_factor().unwrap_or(1.0);
    let anchor = rect.position.to_physical::<f64>(scale);
    let anchor_size = rect.size.to_physical::<f64>(scale);
    let size = window.outer_size().unwrap_or_default();
    let (width, height) = (f64::from(size.width), f64::from(size.height));
    let monitor = window
        .monitor_from_point(anchor.x, anchor.y)
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());
    let gap = 8.0 * scale;
    let mut x = anchor.x + anchor_size.width / 2.0 - width / 2.0;
    let mut y = anchor.y + anchor_size.height + gap;
    if let Some(monitor) = monitor {
        let area = monitor.work_area();
        let (left, top) = (f64::from(area.position.x), f64::from(area.position.y));
        let (right, bottom) = (
            left + f64::from(area.size.width),
            top + f64::from(area.size.height),
        );
        if anchor.y > (top + bottom) / 2.0 {
            y = anchor.y - height - gap;
        }
        x = x.clamp(left + gap, (right - width - gap).max(left));
        y = y.clamp(top + gap, (bottom - height - gap).max(top));
    }
    let _ = window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
    let _ = window.show();
    let _ = window.set_focus();
}
