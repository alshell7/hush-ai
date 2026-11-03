//! User settings, stored as JSON in the app's config folder.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Quality {
    /// 16 kHz, 3 layers: lowest delay and CPU.
    Light,
    /// 24 kHz, 4 layers.
    Balanced,
    /// 48 kHz full band, 3 layers: most CPU.
    Studio,
}

impl Quality {
    /// Model sample rate and layer count.
    pub fn model(self) -> (u32, usize) {
        match self {
            Self::Light => (16_000, 3),
            Self::Balanced => (24_000, 4),
            Self::Studio => (48_000, 3),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub onboarded: bool,
    pub license_accepted: bool,
    /// Noise cancellation is on.
    pub enabled: bool,
    /// Microphone name; `None` follows the system default.
    pub input: Option<String>,
    /// Headphones for "Listen"; `None` follows the system default.
    pub output: Option<String>,
    pub quality: Quality,
    /// Inference threads; 0 is automatic.
    pub threads: usize,
    /// Windows: run inference with "Pro Audio" priority (MMCSS).
    pub audio_priority: bool,
    pub check_updates: bool,
    pub tray_hint_shown: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            onboarded: false,
            license_accepted: false,
            enabled: true,
            input: None,
            output: None,
            quality: Quality::Light,
            threads: 0,
            audio_priority: true,
            check_updates: true,
            tray_hint_shown: false,
        }
    }
}

impl Settings {
    pub fn load(path: &PathBuf) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Write to a temporary file, then rename, so a crash never leaves half a file.
    pub fn save(&self, path: &PathBuf) -> eyre::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&temporary, path)?;
        Ok(())
    }
}
