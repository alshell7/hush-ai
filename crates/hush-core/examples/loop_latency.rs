//! Delay from an output device to an input device, for example a virtual cable.
//!
//! `loop_latency --output "Hush AI Microphone Feed" --input "Hush AI Microphone" [--seconds 10]`
//!
//! Plays a click every 500 ms and records the input. The delay of each click is its capture
//! time minus its playback time, both from the audio system's stream timestamps, so device
//! buffers on both sides are included. Prints one JSON line. Needs the `desktop` feature.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use eyre::{ensure, eyre, Result};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const PERIOD_SECONDS: f64 = 0.5;

fn find(input: bool, prefix: &str) -> Result<cpal::Device> {
    let host = cpal::default_host();
    let mut devices: Box<dyn Iterator<Item = cpal::Device>> = if input {
        Box::new(host.input_devices()?)
    } else {
        Box::new(host.output_devices()?)
    };
    devices
        .find(|d| d.name().is_ok_and(|n| n.starts_with(prefix)))
        .ok_or_else(|| eyre!("no device starting with {prefix:?}"))
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let (mut output, mut input, mut seconds) = (String::new(), String::new(), 10.0);
    while let Some(arg) = args.next() {
        let value = args.next().ok_or_else(|| eyre!("{arg} needs a value"))?;
        match arg.as_str() {
            "--output" => output = value,
            "--input" => input = value,
            "--seconds" => seconds = value.parse()?,
            _ => {
                eyre::bail!("unknown argument {arg}");
            }
        }
    }
    ensure!(
        !output.is_empty() && !input.is_empty(),
        "--output and --input are required"
    );

    // Playback: (stream time of each click, in seconds).
    let played = Arc::new(Mutex::new(Vec::<cpal::StreamInstant>::new()));
    let out_device = find(false, &output)?;
    let out_config = out_device.default_output_config()?;
    ensure!(
        out_config.sample_format() == cpal::SampleFormat::F32,
        "output is not f32"
    );
    let (out_rate, out_channels) = (out_config.sample_rate().0, out_config.channels() as usize);
    let period = (PERIOD_SECONDS * f64::from(out_rate)) as u64;
    let mut written = 0u64;
    let clicks = played.clone();
    let out_stream = out_device.build_output_stream(
        &out_config.into(),
        move |data: &mut [f32], info| {
            let start = info.timestamp().playback;
            for (i, frame) in data.chunks_mut(out_channels).enumerate() {
                let click = written.is_multiple_of(period) && written > 0;
                frame.fill(if click { 0.9 } else { 0.0 });
                if click {
                    let at = start.add(Duration::from_secs_f64(i as f64 / f64::from(out_rate)));
                    if let (Some(at), Ok(mut list)) = (at, clicks.try_lock()) {
                        list.push(at);
                    }
                }
                written += 1;
            }
        },
        |error| eprintln!("output error: {error}"),
        None,
    )?;

    // Capture: (stream time of each detected click).
    let heard = Arc::new(Mutex::new(Vec::<cpal::StreamInstant>::new()));
    let in_device = find(true, &input)?;
    let in_config = in_device.default_input_config()?;
    ensure!(
        in_config.sample_format() == cpal::SampleFormat::F32,
        "input is not f32"
    );
    let (in_rate, in_channels) = (in_config.sample_rate().0, in_config.channels() as usize);
    let hold = (0.2 * f64::from(in_rate)) as u64;
    let mut quiet = hold;
    let detected = heard.clone();
    let in_stream = in_device.build_input_stream(
        &in_config.into(),
        move |data: &[f32], info| {
            let start = info.timestamp().capture;
            for (i, frame) in data.chunks(in_channels).enumerate() {
                quiet += 1;
                if frame[0].abs() > 0.5 && quiet > hold {
                    quiet = 0;
                    let at = start.add(Duration::from_secs_f64(i as f64 / f64::from(in_rate)));
                    if let (Some(at), Ok(mut list)) = (at, detected.try_lock()) {
                        list.push(at);
                    }
                }
            }
        },
        |error| eprintln!("input error: {error}"),
        None,
    )?;
    in_stream.play()?;
    out_stream.play()?;
    std::thread::sleep(Duration::from_secs_f64(seconds));
    drop(out_stream);
    drop(in_stream);

    let played = played.lock().unwrap().clone();
    let heard = heard.lock().unwrap().clone();
    // Match each click to the first capture after it, within one period.
    let mut delays: Vec<f64> = played
        .iter()
        .filter_map(|p| {
            heard
                .iter()
                .filter_map(|h| h.duration_since(p))
                .find(|d| d.as_secs_f64() < PERIOD_SECONDS)
                .map(|d| d.as_secs_f64() * 1000.0)
        })
        .collect();
    ensure!(
        !delays.is_empty(),
        "no clicks came through ({} played, {} heard)",
        played.len(),
        heard.len()
    );
    delays.sort_by(f64::total_cmp);
    println!(
        "{{\"clicks\":{},\"matched\":{},\"delay_ms_min\":{:.1},\"delay_ms_median\":{:.1},\"delay_ms_max\":{:.1},\"output_rate\":{out_rate},\"input_rate\":{in_rate}}}",
        played.len(),
        delays.len(),
        delays[0],
        delays[delays.len() / 2],
        delays[delays.len() - 1]
    );
    Ok(())
}
