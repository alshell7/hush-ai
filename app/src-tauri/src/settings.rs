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

/// How much of their background noise Hush removes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Clean {
    Off,
    /// Up to 12 dB: quieter rooms, natural sound.
    Light,
    /// All it can.
    Full,
}

impl Clean {
    /// DeepFilterNet's attenuation limit in dB; `None` when off.
    pub fn limit(self) -> Option<f32> {
        match self {
            Self::Off => None,
            Self::Light => Some(12.0),
            Self::Full => Some(100.0),
        }
    }
}

/// What you hear while their voice is translated (with Hush AI Speaker).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Hear {
    /// Their own voice; the translation shows as captions.
    Original,
    /// Their voice turned down while the translation speaks over it, like an interpreter.
    VoiceOver,
    /// Only the translated voice.
    Translation,
}

/// Which directions the conversation switch (and its shortcut) turns on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TalkMode {
    /// Your voice, translated for them.
    Me,
    /// Their voice, translated for you.
    Them,
    Both,
}

/// Quick controls: the tray widget, or the notch at the top of the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Overlay {
    Widget,
    Notch,
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
    /// Speakers or headphones for the microphone test; `None` follows the system default.
    pub output: Option<String>,
    pub quality: Quality,
    /// Inference threads; 0 is automatic.
    pub threads: usize,
    /// Windows: run inference with "Pro Audio" priority (MMCSS).
    pub audio_priority: bool,
    pub check_updates: bool,
    pub tray_hint_shown: bool,
    /// Turn noise cancellation on before calendar meetings that have a call link.
    pub calendar_auto: bool,
    /// Minutes before a meeting to turn it on.
    pub calendar_lead: u32,
    /// Turn it off after the meeting, if Hush turned it on.
    pub calendar_auto_off: bool,
    /// Pop up the tray widget shortly before a meeting.
    pub calendar_popup: bool,
    /// Minutes between calendar reads.
    pub calendar_sync: u32,
    /// Live Translate: the language you speak, and the one the other side hears (BCP-47).
    pub translate_from: String,
    pub translate_to: String,
    /// Ask before Live Translate starts sending your voice to Google.
    pub translate_ask: bool,
    /// Keep conversations (both languages) on this computer.
    pub translate_history: bool,
    /// Hush AI Speaker: clean their voice before you hear it.
    pub listen: bool,
    pub listen_clean: Clean,
    /// What you hear while their voice is translated.
    pub listen_voice: Hear,
    /// Where Hush plays their voice; `None` follows the system default.
    pub listen_output: Option<String>,
    pub talk_mode: TalkMode,
    pub overlay: Overlay,
    /// Keep Hush's windows out of screenshots, screen sharing and recordings.
    pub hide_from_capture: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            onboarded: false,
            license_accepted: false,
            engine: if hush_core::DEEP_FILTER_AVAILABLE {
                Engine::DeepFilter
            } else {
                Engine::Reuse
            },
            enabled: true,
            input: None,
            output: None,
            quality: Quality::Light,
            threads: 0,
            audio_priority: true,
            check_updates: true,
            tray_hint_shown: false,
            calendar_auto: true,
            calendar_lead: 1,
            calendar_auto_off: true,
            calendar_popup: true,
            calendar_sync: 5,
            translate_from: "en".into(),
            translate_to: "es".into(),
            translate_ask: true,
            translate_history: true,
            listen: true,
            listen_clean: Clean::Full,
            listen_voice: Hear::VoiceOver,
            listen_output: None,
            talk_mode: TalkMode::Both,
            overlay: Overlay::Notch,
            hide_from_capture: false,
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
        if (value.get("engine").is_none() && settings.onboarded)
            || !hush_core::DEEP_FILTER_AVAILABLE
        {
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
