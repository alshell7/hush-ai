//! End-to-end check of the installed Hush AI Speaker, through the system audio stack.
//!
//! `virtual_speaker_check [--seconds 6] [--freq 997] [--amp 0.5] [--max-latency 2400]
//!                        [--poll-ms 10] [--stall-ms 300]`
//!
//! Plays a stereo tone to "Hush AI Speaker" with CPAL, like a meeting app, and reads it back with
//! [`VirtualSpeaker`], like the Hush app. Halfway through, the reader stalls for `--stall-ms`, so
//! the latency bound must skip audio once. Then the tone stops and `active()` must turn false.
//! Checks: frequency and level of the mono mix, breaks in the tone (only the stall may cause
//! one), buffered audio never above the bound after a read, and `active()`.
//!
//! Prints one JSON line. Needs the `devices` feature. Playing to a device needs no permission,
//! so this runs over SSH.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use eyre::{ensure, eyre, Result};
use hush_core::virtual_speaker::{VirtualSpeaker, RATE};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Options {
    seconds: f64,
    freq: f64,
    amp: f64,
    max_latency: usize,
    poll_ms: u64,
    stall_ms: u64,
}

fn options() -> Result<Options> {
    let mut o = Options {
        seconds: 6.0,
        freq: 997.0,
        amp: 0.5,
        max_latency: 2400,
        poll_ms: 10,
        stall_ms: 300,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let value: f64 = args
            .next()
            .ok_or_else(|| eyre!("{arg} needs a value"))?
            .parse()
            .map_err(|_| eyre!("invalid value for {arg}"))?;
        match arg.as_str() {
            "--seconds" => o.seconds = value,
            "--freq" => o.freq = value,
            "--amp" => o.amp = value,
            "--max-latency" => o.max_latency = value as usize,
            "--poll-ms" => o.poll_ms = value as u64,
            "--stall-ms" => o.stall_ms = value as u64,
            _ => return Err(eyre!("unknown option {arg}")),
        }
    }
    Ok(o)
}

/// Positions where the signal is not a sine of frequency `freq`, grouped into events.
/// For a sine, x[n-1] + x[n+1] = 2 cos(w) x[n].
fn breaks(x: &[f32], freq: f64, amp: f64) -> Vec<usize> {
    let c = 2.0 * (2.0 * std::f64::consts::PI * freq / f64::from(RATE)).cos();
    let mut events: Vec<usize> = Vec::new();
    for n in 1..x.len().saturating_sub(1) {
        let r = f64::from(x[n - 1]) + f64::from(x[n + 1]) - c * f64::from(x[n]);
        if r.abs() > 0.02 * amp && events.last().is_none_or(|&last| n > last + 64) {
            events.push(n);
        }
    }
    events
}

fn main() -> Result<()> {
    let o = options()?;
    let mut speaker = VirtualSpeaker::open()?;
    let idle_at_start = !speaker.active();

    let host = cpal::default_host();
    // Not `output_devices()`: CPAL 0.15 lists output formats through an AUHAL unit with input
    // enabled, which Core Audio refuses for an output-only virtual device. Playing works.
    let device = host
        .devices()?
        .find(|d| d.name().is_ok_and(|n| n == "Hush AI Speaker"))
        .ok_or_else(|| eyre!("no output device named Hush AI Speaker"))?;
    let default_output = host
        .default_output_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_default();
    let config = cpal::StreamConfig {
        channels: 2,
        sample_rate: cpal::SampleRate(RATE),
        buffer_size: cpal::BufferSize::Default,
    };
    let first_callback: Arc<Mutex<Option<Instant>>> = Arc::default();
    let first = first_callback.clone();
    let (freq, amp) = (o.freq, o.amp);
    let mut phase = 0.0f64;
    let step = 2.0 * std::f64::consts::PI * freq / f64::from(RATE);
    let stream = device.build_output_stream(
        &config,
        move |data: &mut [f32], _| {
            first.lock().unwrap().get_or_insert_with(Instant::now);
            for frame in data.chunks_mut(2) {
                let v = (amp * phase.sin()) as f32;
                phase = (phase + step) % (2.0 * std::f64::consts::PI);
                frame.fill(v);
            }
        },
        |e| eprintln!("stream error: {e}"),
        None,
    )?;
    stream.play()?;

    let start = Instant::now();
    let mut audio: Vec<f32> = Vec::new();
    let mut buffer = vec![0f32; 8192];
    let mut onset: Option<Instant> = None;
    let mut active_seen: Option<f64> = None;
    let mut max_after_read = 0usize;
    let mut max_before_read = 0usize;
    let mut stall_at: Option<usize> = None;
    let mut reads = 0u64;
    let poll = Duration::from_millis(o.poll_ms);
    while start.elapsed().as_secs_f64() < o.seconds {
        if stall_at.is_none() && start.elapsed().as_secs_f64() > o.seconds / 2.0 {
            stall_at = Some(audio.len());
            std::thread::sleep(Duration::from_millis(o.stall_ms));
        }
        max_before_read = max_before_read.max(speaker.buffered());
        let n = speaker.read(&mut buffer, o.max_latency);
        reads += 1;
        max_after_read = max_after_read.max(speaker.buffered());
        if onset.is_none() && buffer[..n].iter().any(|v| v.abs() > 0.01) {
            onset = Some(Instant::now());
        }
        if active_seen.is_none() && speaker.active() {
            active_seen = Some(start.elapsed().as_secs_f64());
        }
        if onset.is_some() {
            audio.extend_from_slice(&buffer[..n]);
        }
        std::thread::sleep(poll);
    }
    // CPAL 0.15 on macOS does not stop the device when a stream is dropped: pause it first.
    stream.pause()?;
    drop(stream);
    let stopped = Instant::now();
    let mut inactive_after = None;
    while stopped.elapsed() < Duration::from_secs(3) {
        let mut sink = [0f32; 4096];
        speaker.read(&mut sink, o.max_latency);
        if !speaker.active() {
            inactive_after = Some(stopped.elapsed().as_secs_f64());
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    // Analysis: skip the first 100 ms after the onset (the tone starts at phase 0).
    let skip = (RATE / 10) as usize;
    ensure!(
        audio.len() > skip * 4,
        "too little audio read: {} frames",
        audio.len()
    );
    let body = &audio[skip..];
    let rms = (body
        .iter()
        .map(|&v| f64::from(v) * f64::from(v))
        .sum::<f64>()
        / body.len() as f64)
        .sqrt();
    let crossings = body
        .windows(2)
        .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
        .count();
    let measured_freq = crossings as f64 / 2.0 / (body.len() as f64 / f64::from(RATE));
    let events: Vec<usize> = breaks(body, o.freq, o.amp)
        .iter()
        .map(|&n| n + skip)
        .collect();
    // The stall must cause exactly one break, where the reader skipped ahead.
    let stall_breaks = stall_at.map_or(0, |at| {
        events
            .iter()
            .filter(|&&n| n + 64 >= at && n <= at + 64)
            .count()
    });
    let other_breaks = events.len() - stall_breaks;
    let onset_ms = match (onset, *first_callback.lock().unwrap()) {
        (Some(heard), Some(played)) => {
            Some(heard.saturating_duration_since(played).as_secs_f64() * 1e3)
        }
        _ => None,
    };
    let expected_rms = o.amp / 2f64.sqrt();
    println!(
        "{{\"frames\":{},\"reads\":{},\"freq\":{:.2},\"rms\":{:.4},\"expected_rms\":{:.4},\
         \"breaks\":{},\"stall_breaks\":{},\"skipped\":{},\"max_buffered_before_read\":{},\
         \"max_buffered_after_read\":{},\"max_latency\":{},\"onset_ms\":{},\"active_at_s\":{},\
         \"inactive_after_s\":{},\"idle_at_start\":{},\"default_output\":\"{}\"}}",
        audio.len(),
        reads,
        measured_freq,
        rms,
        expected_rms,
        other_breaks,
        stall_breaks,
        speaker.skipped(),
        max_before_read,
        max_after_read,
        o.max_latency,
        onset_ms.map_or("null".into(), |v| format!("{v:.1}")),
        active_seen.map_or("null".into(), |v| format!("{v:.3}")),
        inactive_after.map_or("null".into(), |v| format!("{v:.3}")),
        idle_at_start,
        default_output.replace('"', "'"),
    );
    ensure!(
        (measured_freq - o.freq).abs() < o.freq * 0.005,
        "wrong frequency"
    );
    ensure!(
        (rms - expected_rms).abs() < expected_rms * 0.02,
        "wrong level"
    );
    ensure!(other_breaks == 0, "{other_breaks} breaks in the tone");
    ensure!(
        o.stall_ms == 0 || speaker.skipped() > 0,
        "the stall did not skip audio"
    );
    ensure!(
        max_after_read <= o.max_latency,
        "latency above the bound after a read"
    );
    ensure!(active_seen.is_some(), "active() never turned true");
    ensure!(
        inactive_after.is_some_and(|s| s < 1.5),
        "active() did not turn false after the tone stopped"
    );
    ensure!(
        default_output != "Hush AI Speaker",
        "Hush AI Speaker became the default output"
    );
    Ok(())
}
