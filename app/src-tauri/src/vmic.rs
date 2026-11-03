//! The "Hush AI Microphone": where it comes from on each system, and how to set it up.
//!
//! - macOS: the Hush Core Audio plug-in (`HushMic.driver`), installed with one password prompt.
//! - Windows: the Hush driver when installed; otherwise VB-CABLE, renamed by Hush.
//! - Linux: a PulseAudio/PipeWire virtual source that Hush creates while it runs.
use eyre::Result;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum State {
    /// Apps can choose Hush AI Microphone.
    Ready,
    /// One step with administrator rights is needed (install, update or rename).
    #[cfg_attr(target_os = "linux", allow(dead_code))]
    Install,
    /// Windows: VB-CABLE must be downloaded and installed first.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    Download,
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Unsupported,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub state: State,
    /// "hush", "vb-cable" or "pulse".
    pub driver: &'static str,
    pub detail: String,
}

fn info(state: State, driver: &'static str, detail: impl Into<String>) -> Info {
    Info {
        state,
        driver,
        detail: detail.into(),
    }
}

/// Current state. `running` means Hush writes to the microphone now.
pub fn status(running: bool) -> Info {
    platform::status(running)
}

/// Do the setup step for the current state. May show a system password or UAC prompt.
pub fn install(resources: &std::path::Path) -> Result<()> {
    platform::install(resources)
}

/// Remove the macOS plug-in or restore VB-CABLE's names.
pub fn uninstall() -> Result<()> {
    platform::uninstall()
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use std::path::Path;

    const INSTALLED: &str = "/Library/Audio/Plug-Ins/HAL/HushMic.driver";
    const BINARY: &str = "Contents/MacOS/HushMic";

    fn bundled(resources: &Path) -> std::path::PathBuf {
        resources.join("HushMic.driver")
    }

    fn resources() -> Option<std::path::PathBuf> {
        let exe = std::env::current_exe().ok()?;
        Some(exe.parent()?.parent()?.join("Resources"))
    }

    pub fn status(running: bool) -> Info {
        let installed = Path::new(INSTALLED);
        if !installed.exists() {
            return info(State::Install, "hush", "Not installed");
        }
        // A newer plug-in in the app: offer the update.
        if let Some(resources) = resources() {
            let new = std::fs::read(bundled(&resources).join(BINARY)).ok();
            let old = std::fs::read(installed.join(BINARY)).ok();
            if new.is_some() && new != old {
                return info(State::Install, "hush", "Update available");
            }
        }
        if running || hush_core::virtual_mic::VirtualMic::open_observer().is_ok() {
            info(State::Ready, "hush", "Hush AI Microphone")
        } else {
            info(
                State::Install,
                "hush",
                "Installed, but Core Audio has not loaded it",
            )
        }
    }

    fn admin(script: &str) -> Result<()> {
        let applescript = format!(
            "do shell script \"{}\" with administrator privileges",
            script.replace('\\', "\\\\").replace('"', "\\\"")
        );
        let output = std::process::Command::new("osascript")
            .args(["-e", &applescript])
            .output()?;
        if !output.status.success() {
            let text = String::from_utf8_lossy(&output.stderr);
            if text.contains("-128") {
                eyre::bail!("Canceled");
            }
            eyre::bail!("{}", text.trim());
        }
        Ok(())
    }

    fn quote(path: &Path) -> String {
        format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
    }

    pub fn install(resources: &Path) -> Result<()> {
        let source = bundled(resources);
        eyre::ensure!(source.exists(), "HushMic.driver is missing from the app");
        admin(&format!(
            "rm -rf {INSTALLED} && cp -R {} /Library/Audio/Plug-Ins/HAL/ && chown -R root:wheel {INSTALLED} && (killall coreaudiod || true)",
            quote(&source)
        ))?;
        // Core Audio restarts in about a second and loads the plug-in.
        for _ in 0..40 {
            if hush_core::virtual_mic::VirtualMic::open_observer().is_ok() {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        Ok(())
    }

    pub fn uninstall() -> Result<()> {
        admin(&format!(
            "rm -rf {INSTALLED} && (killall coreaudiod || true)"
        ))
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use crate::audio::live_io::vb_cable::{self, Status};

    /// The Hush driver accepts one writer, so only probe it when Hush is not writing.
    fn hush_driver() -> bool {
        hush_core::virtual_mic::VirtualMic::open().is_ok()
    }

    pub fn status(running: bool) -> Info {
        if !running && hush_driver() {
            return info(State::Ready, "hush", "Hush AI Microphone (Hush driver)");
        }
        match vb_cable::status() {
            Status::Ready => info(State::Ready, "vb-cable", "Hush AI Microphone (VB-CABLE)"),
            Status::Installed => info(
                State::Install,
                "vb-cable",
                "VB-CABLE is installed. Rename it to Hush AI Microphone",
            ),
            Status::Missing => info(
                State::Download,
                "vb-cable",
                "Install VB-CABLE, then come back",
            ),
        }
    }

    pub fn install(_resources: &std::path::Path) -> Result<()> {
        match vb_cable::status() {
            Status::Missing => vb_cable::open_download_page(),
            _ => vb_cable::run_elevated(false),
        }
    }

    pub fn uninstall() -> Result<()> {
        vb_cable::run_elevated(true)
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::*;

    pub fn status(_running: bool) -> Info {
        if crate::audio::pulse::available() {
            info(
                State::Ready,
                "pulse",
                "Hush AI Microphone (PipeWire/PulseAudio)",
            )
        } else {
            info(
                State::Unsupported,
                "pulse",
                "Needs PipeWire or PulseAudio (pactl)",
            )
        }
    }

    pub fn install(_resources: &std::path::Path) -> Result<()> {
        crate::audio::pulse::ensure()
    }

    pub fn uninstall() -> Result<()> {
        crate::audio::pulse::remove();
        Ok(())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod platform {
    use super::*;

    pub fn status(_running: bool) -> Info {
        info(State::Unsupported, "none", "Not available on this system")
    }

    pub fn install(_resources: &std::path::Path) -> Result<()> {
        Err(eyre::eyre!("Not available on this system"))
    }

    pub fn uninstall() -> Result<()> {
        Ok(())
    }
}
