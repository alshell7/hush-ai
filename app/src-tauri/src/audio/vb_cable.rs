//! Windows: use VB-CABLE (by VB-Audio, installed by the user) as the Hush AI Microphone until
//! the Hush driver is signed. Hush renames the two cable endpoints and plays its output into
//! the cable; apps record the other end.
//!
//! | VB-CABLE endpoint | Renamed to |
//! | --- | --- |
//! | CABLE Output (recording) | Hush AI Microphone (Hush) |
//! | CABLE Input (playback) | Hush AI Microphone Feed (Hush) |
//!
//! Renaming writes the endpoint property store, which needs administrator rights: the app
//! starts itself elevated with `--vb-cable setup` or `--vb-cable restore` (one UAC prompt).
//! Only the endpoint names change. The driver stays VB-Audio's; reinstalling VB-CABLE restores
//! the names. Hush never downloads or installs VB-CABLE itself: VB-Audio's terms do not allow
//! bundling it, so the app opens VB-Audio's page and the user installs it.

use cpal::traits::{DeviceTrait, HostTrait};
use eyre::{eyre, Result};

/// VB-Audio's official page for VB-CABLE.
pub const DOWNLOAD_PAGE: &str = "https://vb-audio.com/Cable/";
const CABLE_CAPTURE: &str = "CABLE Output";
const CABLE_RENDER: &str = "CABLE Input";
const CABLE_INTERFACE: &str = "VB-Audio Virtual Cable";
const MIC_NAME: &str = "Hush AI Microphone";
const FEED_NAME: &str = "Hush AI Microphone Feed";
const INTERFACE_NAME: &str = "Hush";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// VB-CABLE is not installed (or not enabled).
    Missing,
    /// VB-CABLE is installed with its own names.
    Installed,
    /// VB-CABLE is renamed and ready.
    Ready,
}

/// Device names as apps see them: "<endpoint> (<interface>)".
fn named(name: &str, endpoint: &str) -> bool {
    name.starts_with(&format!("{endpoint} ("))
}

pub fn status() -> Status {
    let host = cpal::default_host();
    let outputs: Vec<String> = host
        .output_devices()
        .map(|devices| devices.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default();
    if outputs.iter().any(|n| named(n, FEED_NAME)) {
        Status::Ready
    } else if outputs.iter().any(|n| named(n, CABLE_RENDER)) {
        Status::Installed
    } else {
        Status::Missing
    }
}

/// The playback side of the renamed cable, where Hush sends its output.
pub fn feed_device() -> Result<cpal::Device> {
    cpal::default_host()
        .output_devices()?
        .find(|d| d.name().is_ok_and(|n| named(&n, FEED_NAME)))
        .ok_or_else(|| {
            eyre!("Hush AI Microphone is not set up. Turn on Send to Hush AI Microphone again to set it up")
        })
}

/// Open VB-Audio's download page in the default browser.
pub fn open_download_page() -> Result<()> {
    shell::open(DOWNLOAD_PAGE)
}

/// Rename the cable (or restore its names) in an elevated copy of this app. Blocks until the
/// copy exits; call it off the GUI thread. Fails if the user declines the UAC prompt.
pub fn run_elevated(restore: bool) -> Result<()> {
    let exe = std::env::current_exe()?;
    let code = shell::run_as_admin(
        &exe,
        if restore {
            "--vb-cable restore"
        } else {
            "--vb-cable setup"
        },
    )?;
    match code {
        0 => Ok(()),
        2 => Err(eyre!("VB-CABLE was not found. Install it, then try again")),
        code => {
            let detail = std::fs::read_to_string(endpoints::log()).unwrap_or_default();
            Err(eyre!(
                "Renaming the VB-CABLE devices failed (code {code}). {}",
                detail.trim()
            ))
        }
    }
}

/// Entry point for `hush --vb-cable setup|restore`, run elevated. Returns the process exit code.
pub fn elevated_main(action: &str) -> i32 {
    let restore = action == "restore";
    let _ = std::fs::remove_file(endpoints::log());
    match endpoints::rename(restore) {
        Ok(0) if !restore => 2,
        Ok(_) => 0,
        Err(error) => {
            let log = endpoints::log();
            let _ = std::fs::create_dir_all(log.parent().unwrap_or(&log));
            let _ = std::fs::write(&log, format!("{error:#}"));
            1
        }
    }
}

mod endpoints {
    use super::*;
    use windows::core::{GUID, HSTRING, PROPVARIANT};
    use windows::Win32::Media::Audio::{
        eAll, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
    };
    use windows::Win32::System::Com::StructuredStorage::{
        PropVariantChangeType, PROPVAR_CHANGE_FLAGS,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ,
        STGM_READWRITE,
    };
    use windows::Win32::System::Variant::VT_LPWSTR;
    use windows::Win32::UI::Shell::PropertiesSystem::PROPERTYKEY;

    /// The full name apps see, "<endpoint> (<interface>)" (PKEY_Device_FriendlyName).
    const FULL_NAME: PROPERTYKEY = PROPERTYKEY {
        fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
        pid: 14,
    };
    /// The endpoint's own name, as in Sound settings (PKEY_Device_DeviceDesc).
    const ENDPOINT_NAME: PROPERTYKEY = PROPERTYKEY {
        fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
        pid: 2,
    };
    /// The name in brackets, stored per endpoint (the audio endpoint's interface name).
    const INTERFACE: PROPERTYKEY = PROPERTYKEY {
        fmtid: GUID::from_u128(0xb3f8fa53_0004_438e_9003_51a46e139bfc),
        pid: 6,
    };

    fn folder() -> std::path::PathBuf {
        let base = std::env::var_os("ProgramData").unwrap_or_else(|| r"C:\ProgramData".into());
        std::path::Path::new(&base).join("Hush")
    }

    /// Endpoint IDs renamed by setup, so that restore never touches the Hush driver's own
    /// "Hush AI Microphone".
    fn record() -> std::path::PathBuf {
        folder().join("vb-cable-endpoints.txt")
    }

    /// The elevated helper's last error, for the app to show.
    pub fn log() -> std::path::PathBuf {
        folder().join("vb-cable.log")
    }

    fn text(value: &str) -> windows::core::Result<PROPVARIANT> {
        let mut out = PROPVARIANT::default();
        // SAFETY: both pointers refer to live PROPVARIANTs.
        unsafe {
            PropVariantChangeType(
                &mut out,
                &PROPVARIANT::from(value),
                PROPVAR_CHANGE_FLAGS(0),
                VT_LPWSTR,
            )?
        };
        Ok(out)
    }

    unsafe fn set_names(
        device: &IMMDevice,
        endpoint: &str,
        interface: &str,
    ) -> windows::core::Result<()> {
        let store = device.OpenPropertyStore(STGM_READWRITE)?;
        store.SetValue(&ENDPOINT_NAME, &text(endpoint)?)?;
        // The bracket name is cosmetic: if Windows refuses it, keep the endpoint name.
        let _ = store.SetValue(&INTERFACE, &text(interface)?);
        store.Commit()
    }

    fn remember(kind: &str, device: &IMMDevice) -> eyre::Result<()> {
        // SAFETY: GetId returns a CoTaskMem string that is freed after copying.
        let id = unsafe {
            let raw = device.GetId()?;
            let id = raw.to_string()?;
            windows::Win32::System::Com::CoTaskMemFree(Some(raw.0 as _));
            id
        };
        let earlier = std::fs::read_to_string(record()).unwrap_or_default();
        if !earlier.lines().any(|line| line.ends_with(&id)) {
            std::fs::create_dir_all(folder())?;
            std::fs::write(record(), earlier + &format!("{kind} {id}\n"))?;
        }
        Ok(())
    }

    /// Rename the cable endpoints (or restore them); returns how many were changed.
    pub fn rename(restore: bool) -> eyre::Result<usize> {
        let mut count = 0;
        // SAFETY: standard COM use on this thread; every interface is released when dropped.
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            if restore {
                let Ok(ids) = std::fs::read_to_string(record()) else {
                    return Ok(0);
                };
                for line in ids.lines() {
                    let Some((kind, id)) = line.split_once(' ') else {
                        continue;
                    };
                    let Ok(device) = enumerator.GetDevice(&HSTRING::from(id)) else {
                        continue;
                    };
                    let endpoint = if kind == "capture" {
                        CABLE_CAPTURE
                    } else {
                        CABLE_RENDER
                    };
                    set_names(&device, endpoint, CABLE_INTERFACE)?;
                    count += 1;
                }
                let _ = std::fs::remove_file(record());
                return Ok(count);
            }
            let devices = enumerator.EnumAudioEndpoints(eAll, DEVICE_STATE_ACTIVE)?;
            for index in 0..devices.GetCount()? {
                let device = devices.Item(index)?;
                let name = device
                    .OpenPropertyStore(STGM_READ)?
                    .GetValue(&FULL_NAME)?
                    .to_string();
                // A feed renamed by an earlier, interrupted setup is the cable too.
                let (kind, endpoint) = if named(&name, CABLE_CAPTURE) {
                    ("capture", MIC_NAME)
                } else if named(&name, CABLE_RENDER) || named(&name, FEED_NAME) {
                    ("render", FEED_NAME)
                } else {
                    continue;
                };
                // Remember first: restore must find every endpoint that was touched.
                remember(kind, &device)?;
                set_names(&device, endpoint, INTERFACE_NAME)?;
                count += 1;
            }
        }
        Ok(count)
    }
}

mod shell {
    use eyre::Result;
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};
    use windows::Win32::UI::Shell::{
        ShellExecuteExW, ShellExecuteW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
    };
    use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_SHOWNORMAL};

    pub fn open(url: &str) -> Result<()> {
        // SAFETY: valid NUL-terminated strings for the duration of the call.
        let result = unsafe {
            ShellExecuteW(
                None,
                &HSTRING::from("open"),
                &HSTRING::from(url),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        eyre::ensure!(result.0 > 32, "Could not open {url}");
        Ok(())
    }

    /// Start `exe` with `arguments` as administrator and wait for its exit code.
    pub fn run_as_admin(exe: &std::path::Path, arguments: &str) -> Result<u32> {
        let file = HSTRING::from(exe.as_os_str());
        let parameters = HSTRING::from(arguments);
        let verb = HSTRING::from("runas");
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS,
            lpVerb: PCWSTR(verb.as_ptr()),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: PCWSTR(parameters.as_ptr()),
            nShow: SW_HIDE.0,
            ..Default::default()
        };
        // SAFETY: `info` and its strings live until the process handle is closed below.
        unsafe {
            ShellExecuteExW(&mut info)
                .map_err(|e| eyre::eyre!("Administrator permission was not given ({e})"))?;
            WaitForSingleObject(info.hProcess, INFINITE);
            let mut code = 0;
            let result = GetExitCodeProcess(info.hProcess, &mut code);
            let _ = CloseHandle(info.hProcess);
            result?;
            Ok(code)
        }
    }
}
