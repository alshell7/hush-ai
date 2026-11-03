//! Linux: a virtual source "Hush AI Microphone" fed by a null sink, through `pactl`.
//! Works with PulseAudio and with PipeWire's PulseAudio server.
use eyre::{bail, Result};
use std::process::Command;

const SINK: &str = "hush_ai_feed";
const SOURCE: &str = "hush_ai_microphone";

fn pactl(args: &[&str]) -> Result<String> {
    let output = Command::new("pactl").args(args).output()?;
    if !output.status.success() {
        bail!(
            "pactl {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn available() -> bool {
    pactl(&["info"]).is_ok()
}

fn exists(kind: &str, name: &str) -> bool {
    pactl(&["list", "short", kind]).is_ok_and(|list| {
        list.lines()
            .any(|line| line.split('\t').nth(1) == Some(name))
    })
}

/// Create the feed sink and the microphone if they are missing.
pub fn ensure() -> Result<()> {
    if !exists("sinks", SINK) {
        pactl(&[
            "load-module",
            "module-null-sink",
            &format!("sink_name={SINK}"),
            "rate=48000",
            "channels=1",
            "sink_properties=device.description=Hush-AI-Microphone-Feed",
        ])?;
    }
    if !exists("sources", SOURCE) {
        pactl(&[
            "load-module",
            "module-remap-source",
            &format!("master={SINK}.monitor"),
            &format!("source_name={SOURCE}"),
            "source_properties=device.description=Hush-AI-Microphone",
        ])?;
    }
    Ok(())
}

/// Unload the modules that Hush created.
pub fn remove() {
    let Ok(list) = pactl(&["list", "short", "modules"]) else {
        return;
    };
    for line in list.lines() {
        // Without Hush, apps that played to Hush AI Speaker fall back to the real speakers.
        if line.contains(SINK) || line.contains(SOURCE) || line.contains(SPEAKER) {
            if let Some(id) = line.split('\t').next() {
                let _ = pactl(&["unload-module", id]);
            }
        }
    }
}

/// The output device that plays into the feed sink. The ALSA "pulse" plugin reads
/// `PULSE_SINK` when a stream opens.
pub fn feed_device() -> Result<cpal::Device> {
    use cpal::traits::{DeviceTrait, HostTrait};
    ensure()?;
    std::env::set_var("PULSE_SINK", SINK);
    let host = cpal::default_host();
    let mut devices = host.output_devices()?;
    devices
        .find(|d| d.name().is_ok_and(|n| n == "pulse"))
        .or_else(|| host.default_output_device())
        .ok_or_else(|| eyre::eyre!("No PulseAudio output device"))
}

/// Hush AI Speaker: a null sink that meeting apps play to. Hush reads its monitor.
const SPEAKER: &str = "hush_ai_speaker";

/// Create Hush AI Speaker if PulseAudio is there; whether it exists.
pub fn speaker_ready() -> bool {
    if !available() {
        return false;
    }
    if !exists("sinks", SPEAKER) {
        let _ = pactl(&[
            "load-module",
            "module-null-sink",
            &format!("sink_name={SPEAKER}"),
            "rate=48000",
            "channels=2",
            "sink_properties=device.description=Hush-AI-Speaker",
        ]);
    }
    exists("sinks", SPEAKER)
}

/// Opening a stream reads `PULSE_SINK`/`PULSE_SOURCE`: one at a time.
static ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The real default sink (not one of Hush's).
fn real_sink() -> Option<String> {
    let name = pactl(&["get-default-sink"]).ok()?.trim().to_string();
    (!name.is_empty() && name != SPEAKER && name != SINK).then_some(name)
}

/// What to listen to: Hush AI Speaker's monitor, or the default sink's (listening along).
pub fn capture_device(speaker: bool) -> Result<cpal::Device> {
    use cpal::traits::{DeviceTrait, HostTrait};
    let _guard = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let source = if speaker {
        format!("{SPEAKER}.monitor")
    } else {
        format!(
            "{}.monitor",
            real_sink().ok_or_else(|| eyre::eyre!("No speakers to listen to"))?
        )
    };
    std::env::set_var("PULSE_SOURCE", source);
    let host = cpal::default_host();
    let mut devices = host.input_devices()?;
    devices
        .find(|d| d.name().is_ok_and(|n| n == "pulse"))
        .ok_or_else(|| eyre::eyre!("No PulseAudio input device"))
}

/// While alive, streams that open play to the real speakers; afterwards `PULSE_SINK` points
/// at the microphone feed again, as [`feed_device`] expects.
pub struct PlaybackEnv(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

impl Drop for PlaybackEnv {
    fn drop(&mut self) {
        std::env::set_var("PULSE_SINK", SINK);
    }
}

/// Hold this while their voice's playback stream opens.
pub fn playback_env() -> PlaybackEnv {
    let guard = ENV.lock().unwrap_or_else(|e| e.into_inner());
    match real_sink() {
        Some(sink) => std::env::set_var("PULSE_SINK", sink),
        None => std::env::remove_var("PULSE_SINK"),
    }
    PlaybackEnv(guard)
}
