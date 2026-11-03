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
        if line.contains(SINK) || line.contains(SOURCE) {
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
