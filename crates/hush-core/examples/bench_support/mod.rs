use eyre::Result;
use hush_core::{Denoiser, WorkerPlacement, WorkerScheduling};

/// Default remains global Rayon for historical comparability. Both explicit modes use a
/// dedicated pool, so their paired comparison isolates placement rather than pool ownership.
pub fn configure_workers(denoiser: &mut Denoiser) -> Result<&'static str> {
    let scheduling = scheduling_from_env()?;
    let Ok(mode) = std::env::var("HUSH_WORKER_PLACEMENT") else {
        eyre::ensure!(
            scheduling == WorkerScheduling::OsDefault,
            "Windows Audio scheduling requires HUSH_WORKER_PLACEMENT=unrestricted"
        );
        return Ok("global");
    };
    let placement = match mode.as_str() {
        "unrestricted" => WorkerPlacement::Unrestricted,
        "physical" => WorkerPlacement::PhysicalCores,
        _ => {
            eyre::bail!("HUSH_WORKER_PLACEMENT must be unrestricted or physical");
        }
    };
    let threads: usize = std::env::var("RAYON_NUM_THREADS")?.parse()?;
    eyre::ensure!(
        threads > 0,
        "Set a positive RAYON_NUM_THREADS for a dedicated pool"
    );
    denoiser.set_worker_threads_with_options(threads, placement, scheduling)?;
    Ok(if placement == WorkerPlacement::PhysicalCores {
        "physical"
    } else {
        "unrestricted"
    })
}

/// `HUSH_WORKER_SCHEDULING`: `os-default` (or unset) or `windows-audio`.
pub fn scheduling_from_env() -> Result<WorkerScheduling> {
    match std::env::var("HUSH_WORKER_SCHEDULING").as_deref() {
        Ok("windows-audio") => Ok(WorkerScheduling::WindowsAudio),
        Ok("os-default") | Err(std::env::VarError::NotPresent) => Ok(WorkerScheduling::OsDefault),
        _ => {
            eyre::bail!("HUSH_WORKER_SCHEDULING must be os-default or windows-audio");
        }
    }
}

pub fn scheduling_name(scheduling: WorkerScheduling) -> &'static str {
    match scheduling {
        WorkerScheduling::OsDefault => "os-default",
        WorkerScheduling::WindowsAudio => "windows-audio",
    }
}
