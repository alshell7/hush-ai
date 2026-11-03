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
    translate: CheckMenuItem<Wry>,
    listen: CheckMenuItem<Wry>,
    /// When the widget last hid because it lost focus. A click on the icon right after
    /// that must not show it again.
    hidden_at: Mutex<Option<Instant>>,
}

/// The tray icon. macOS tints its template icons for the menu bar itself; Windows and Linux
/// get a pair drawn for the taskbar's theme, so the icon is easy to see on either.
fn icon(on: bool) -> Image<'static> {
    let bytes: &'static [u8] = if cfg!(target_os = "macos") {
        if on {
            include_bytes!("../icons/tray-on-template.png")
        } else {
            include_bytes!("../icons/tray-off-template.png")
        }
    } else {
        match (light_taskbar(), on) {
            (true, true) => include_bytes!("../icons/tray-on-light.png"),
            (true, false) => include_bytes!("../icons/tray-off-light.png"),
            (false, true) => include_bytes!("../icons/tray-on-dark.png"),
            (false, false) => include_bytes!("../icons/tray-off-dark.png"),
        }
    };
    Image::from_bytes(bytes).expect("tray icon")
}

/// Windows: whether the taskbar uses the light theme. Elsewhere panels are usually dark.
#[cfg(target_os = "windows")]
fn light_taskbar() -> bool {
    use windows::core::w;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    let mut value: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: a DWORD read into a local of the size we pass.
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut std::ffi::c_void),
            Some(&mut size),
        )
    };
    result == ERROR_SUCCESS && value == 1
}

#[cfg(not(target_os = "windows"))]
fn light_taskbar() -> bool {
    false
}

/// Windows: redraw the icon when the taskbar switches between light and dark.
pub fn follow_theme(app: &AppHandle) {
    if !cfg!(target_os = "windows") {
        return;
    }
    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("hush-tray-theme".into())
        .spawn(move || {
            let mut light = light_taskbar();
            loop {
                std::thread::sleep(Duration::from_secs(3));
                let now = light_taskbar();
                if now != light {
                    light = now;
                    let status = app.state::<Arc<Engine>>().state().status;
                    update(&app, status);
                }
            }
        });
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
    let translate = CheckMenuItem::with_id(
        app,
        "translate",
        "Translate my voice",
        true,
        false,
        None::<&str>,
    )?;
    let listen = CheckMenuItem::with_id(
        app,
        "listen",
        "Translate their voice",
        true,
        false,
        None::<&str>,
    )?;
    let open = MenuItem::with_id(app, "open", "Open Hush AI", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Hush AI", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &toggle,
            &translate,
            &listen,
            &open,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
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
            "translate" => {
                let translator = app.state::<Arc<crate::translate::Translator>>();
                let on = translator.is_on(crate::conversation::Side::Me);
                crate::set_translate(app, crate::conversation::Side::Me, !on);
            }
            "listen" => {
                let translator = app.state::<Arc<crate::translate::Translator>>();
                let on = translator.is_on(crate::conversation::Side::Them);
                crate::set_translate(app, crate::conversation::Side::Them, !on);
            }
            "open" => crate::show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } => {
                let app = tray.app_handle();
                if app.state::<crate::AppState>().settings().overlay
                    == crate::settings::Overlay::Notch
                {
                    crate::notch::toggle_pin(app);
                } else {
                    toggle_widget(app, rect);
                }
            }
            // Windows users double-click tray icons to open the app.
            TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => crate::show_main(tray.app_handle()),
            _ => {}
        })
        .build(app)?;
    app.manage(TrayItems {
        toggle,
        translate,
        listen,
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

/// Show which directions of Live Translate are on in the tray menu.
pub fn update_translate(app: &AppHandle, me: bool, them: bool) {
    if let Some(items) = app.try_state::<TrayItems>() {
        let _ = items.translate.set_checked(me);
        let _ = items.listen.set_checked(them);
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

/// Fit the widget's height to its content (logical pixels). The edge next to the tray stays
/// where it is: the bottom above a Windows taskbar, the top below the macOS menu bar.
pub fn fit_widget(app: &AppHandle, height: f64) {
    let Some(window) = app.get_webview_window("widget") else {
        return;
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    let target = (height.clamp(240.0, 760.0) * scale).round() as u32;
    let (Ok(size), Ok(position)) = (window.outer_size(), window.outer_position()) else {
        return;
    };
    if size.height == target {
        return;
    }
    let above_tray = window
        .current_monitor()
        .ok()
        .flatten()
        .map(|m| {
            let area = m.work_area();
            let middle = f64::from(area.position.y) + f64::from(area.size.height) / 2.0;
            f64::from(position.y) + f64::from(size.height) / 2.0 > middle
        })
        .unwrap_or(false);
    if above_tray && window.is_visible().unwrap_or(false) {
        let y = position.y + size.height as i32 - target as i32;
        let _ = window.set_position(PhysicalPosition::new(position.x, y));
    }
    let _ = window.set_size(tauri::PhysicalSize::new(size.width, target));
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
