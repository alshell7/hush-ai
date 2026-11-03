//! User settings, stored as JSON in the app's config folder.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The noise-cancellation model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Engine {
    /// DeepFilterNet 3: 48 kHz, one thread, MIT or Apache-2.0.
    DeepFilter,
    /// NVIDIA Real-time RE-USE at the chosen [`Quality`]: NVIDIA One-Way Noncommercial License.
    Reuse,
}

/// RE-USE settings. DeepFilterNet 3 has one setting only.
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
    /// The NVIDIA license of the RE-USE weights is accepted.
    pub license_accepted: bool,
    pub engine: Engine,
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
            engine: Engine::DeepFilter,
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
            .and_then(|bytes| Self::parse(&bytes))
            .unwrap_or_default()
    }

    /// Settings saved by Hush AI 1.0 have no engine. Those users set up RE-USE and have its
    /// model, so they keep it until they choose another engine.
    fn parse(bytes: &[u8]) -> Option<Self> {
        let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
        let mut settings: Self = serde_json::from_value(value.clone()).ok()?;
        if value.get("engine").is_none() && settings.onboarded {
            settings.engine = Engine::Reuse;
        }
        Some(settings)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_1_0_users_keep_reuse_and_new_users_get_deep_filter() {
        let onboarded = br#"{"onboarded": true, "licenseAccepted": true, "quality": "studio"}"#;
        let settings = Settings::parse(onboarded).unwrap();
        assert_eq!(settings.engine, Engine::Reuse);
        assert_eq!(settings.quality, Quality::Studio);
        let unfinished = Settings::parse(br#"{"onboarded": false}"#).unwrap();
        assert_eq!(unfinished.engine, Engine::DeepFilter);
        let chosen = Settings::parse(br#"{"onboarded": true, "engine": "deepFilter"}"#).unwrap();
        assert_eq!(chosen.engine, Engine::DeepFilter);
        assert_eq!(Settings::default().engine, Engine::DeepFilter);
    }
}
