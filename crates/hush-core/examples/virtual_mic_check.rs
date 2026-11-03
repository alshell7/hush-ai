//! End-to-end checks of the installed Hush AI Microphone, through the system audio stack.
//!
//! `virtual_mic_check [--seconds 30] [--record-only | --write-only] [--expect-tone]
//!                    [--stop-writer-after S] [--start-writer-after S] [--buffer FRAMES] [--cable]`
//!
//! The writer plays a 440 Hz tone into the virtual microphone, like the app. The recorder
//! opens "Hush AI Microphone" with CPAL, like a meeting app. With both (the default), or with
//! `--record-only --expect-tone` while another process writes, the check fails on glitches
//! (breaks in the tone) and on audio where there must be silence:
//! - before `--start-writer-after` (old audio must not play),
//! - from 250 ms after `--stop-writer-after` (old audio must not repeat), and
//! - anywhere with `--record-only` and no `--expect-tone`.
//!
//! `--cable` writes like the app does with VB-CABLE on Windows: a normal output stream into
//! "Hush AI Microphone Feed" instead of the Hush driver.
//!
//! Prints one JSON line. Needs the `desktop` feature and microphone permission (on macOS, run
//! it in Terminal on the Mac, not over SSH).

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use eyre::{ensure, eyre, Result};
use hush_core::virtual_mic::{Feeder, VirtualMic, RATE};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TONE: f64 = 440.0;

#[derive(Default)]
struct Options {
    seconds: f64,
    record: bool,
    write: bool,
    expect_tone: bool,
    stop_writer_after: Option<f64>,
    start_writer_after: Option<f64>,
    buffer: Option<u32>,
    cable: bool,
}

fn options() -> Result<Options> {
    let mut o = Options {
        seconds: 30.0,
        record: true,
        write: true,
        ..Default::default()
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || -> Result<f64> {
            args.next()
                .ok_or_else(|| eyre!("{arg} needs a value"))?
                .parse()
                .map_err(|_| eyre!("invalid value for {arg}"))
        };
        match arg.as_str() {
            "--seconds" => o.seconds = value()?,
            "--stop-writer-after" => o.stop_writer_after = Some(value()?),
            "--start-writer-after" => o.start_writer_after = Some(value()?),
            "--buffer" => o.buffer = Some(value()? as u32),
            "--record-only" => o.write = false,
            "--write-only" => o.record = false,
            "--expect-tone" => o.expect_tone = true,
            "--cable" => o.cable = true,
            // A bare number is the duration.
            other => {
                o.seconds = other
                    .parse()
                    .map_err(|_| eyre!("unknown argument {other}"))?
            }
        }
    }
    if o.write && o.record {
        o.expect_tone = true;
    }
    Ok(o)
}

/// The tone source; dropping it stops writing.
enum Writer {
    Driver(Feeder),
    // Held for its drop, which stops the stream.
    Cable(#[allow(dead_code)] cpal::Stream),
}

impl Writer {
    fn max_gap(&self) -> Duration {
        match self {
            Writer::Driver(feeder) => feeder.max_gap(),
            Writer::Cable(_) => Duration::ZERO,
        }
    }
}

fn tone(rate: u32) -> impl FnMut() -> f32 {
    let mut phase = 0f64;
    let step = TONE / f64::from(rate);
    move || {
        let v = (std::f64::consts::TAU * phase).sin() as f32 * 0.5;
        phase = (phase + step).fract();
        v
    }
}

fn start_writer(cable: bool) -> Result<Writer> {
    if !cable {
        let mut next = tone(RATE);
        return Ok(Writer::Driver(Feeder::start(
            VirtualMic::open()?,
            move |block| block.iter_mut().for_each(|s| *s = next()),
        )?));
    }
    let device = cpal::default_host()
        .output_devices()?
        .find(|d| {
            d.name()
                .is_ok_and(|n| n.starts_with("Hush AI Microphone Feed"))
        })
        .ok_or_else(|| eyre!("Hush AI Microphone Feed is not in the output device list"))?;
    let supported = device.default_output_config()?;
    ensure!(
        supported.sample_format() == cpal::SampleFormat::F32,
        "unexpected format {:?}",
        supported.sample_format()
    );
    let channels = supported.channels() as usize;
    let mut next = tone(supported.sample_rate().0);
    let stream = device.build_output_stream(
        &supported.into(),
        move |data: &mut [f32], _| {
            for frame in data.chunks_mut(channels) {
                frame.fill(next());
            }
        },
        |error| eprintln!("stream error: {error}"),
        None,
    )?;
    stream.play()?;
    Ok(Writer::Cable(stream))
}

type Recording = (cpal::Stream, Arc<Mutex<Vec<f32>>>, u32);

fn start_recording(o: &Options) -> Result<Recording> {
    let device = cpal::default_host()
        .input_devices()?
        .find(|d| d.name().is_ok_and(|n| n.starts_with("Hush AI Microphone")))
        .ok_or_else(|| eyre!("Hush AI Microphone is not in the input device list"))?;
    let supported = device.default_input_config()?;
    ensure!(
        supported.sample_format() == cpal::SampleFormat::F32,
        "unexpected format {:?}",
        supported.sample_format()
    );
    let channels = supported.channels() as usize;
    let rate = supported.sample_rate().0;
    let mut config: cpal::StreamConfig = supported.into();
    if let Some(frames) = o.buffer {
        config.buffer_size = cpal::BufferSize::Fixed(frames);
    }
    // Reserve all the space now: growing a vector in the audio callback can miss a deadline.
    let capacity = ((o.seconds + 2.0) * f64::from(rate)) as usize;
    let samples = Arc::new(Mutex::new(Vec::<f32>::with_capacity(capacity)));
    let sink = samples.clone();
    let stream = device.build_input_stream(
        &config,
        move |data: &[f32], _| {
            if let Ok(mut out) = sink.try_lock() {
                let room = out.capacity() - out.len();
                out.extend(data.iter().step_by(channels).take(room));
            }
        },
        |error| eprintln!("stream error: {error}"),
        None,
    )?;
    stream.play()?;
    Ok((stream, samples, rate))
}

fn main() -> Result<()> {
    let o = options()?;
    let mut writer = if o.write && o.start_writer_after.is_none() {
        Some(start_writer(o.cable)?)
    } else {
        None
    };
    // Watching the driver's reads needs a second mapping (macOS only).
    let watch = VirtualMic::open_observer().ok();
    let recording = if o.record {
        Some(start_recording(&o)?)
    } else {
        None
    };

    let start = Instant::now();
    let before = watch.as_ref().map(VirtualMic::position);
    let mut max_gap = Duration::ZERO;
    while start.elapsed().as_secs_f64() < o.seconds {
        let t = start.elapsed().as_secs_f64();
        if writer.is_none() && o.write && o.start_writer_after.is_some_and(|s| t >= s) {
            writer = Some(start_writer(o.cable)?);
        }
        if o.stop_writer_after.is_some_and(|s| t >= s) {
            if let Some(feeder) = writer.take() {
                max_gap = max_gap.max(feeder.max_gap());
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let after = watch.as_ref().map(VirtualMic::position);
    if let Some(feeder) = writer.take() {
        max_gap = max_gap.max(feeder.max_gap());
    }
    let mut report = format!(
        "\"seconds\":{:.1},\"feeder_max_gap_ms\":{:.2}",
        o.seconds,
        max_gap.as_secs_f64() * 1000.0
    );
    if let (Some(before), Some(after)) = (before, after) {
        report += &format!(
            ",\"driver_read_seconds\":{:.2}",
            (after.read - before.read) as f64 / f64::from(RATE)
        );
    }
    let mut failures = Vec::new();
    if let Some((stream, samples, rate)) = recording {
        drop(stream);
        let audio = samples.lock().unwrap().clone();
        let at = |seconds: f64| ((seconds * f64::from(rate)) as usize).min(audio.len());
        let nonzero = |from: usize, to: usize| {
            audio[from..to.max(from)]
                .iter()
                .filter(|v| **v != 0.0)
                .count()
        };
        let from = at(o.start_writer_after.unwrap_or(0.0) + 0.5);
        let to = at(o.stop_writer_after.unwrap_or(o.seconds) - 0.05).max(from);
        let tone = &audio[from..to];
        // A clean sine satisfies x[n] = 2cos(w)x[n-1] - x[n-2]. Large residuals are glitches.
        let c = 2.0 * (std::f64::consts::TAU * TONE / f64::from(rate)).cos();
        let (mut glitches, mut last_glitch) = (0usize, 0usize);
        for n in 2..tone.len() {
            let residual = f64::from(tone[n]) - c * f64::from(tone[n - 1]) + f64::from(tone[n - 2]);
            if residual.abs() > 0.05 && n > last_glitch + rate as usize / 100 {
                glitches += 1;
                last_glitch = n;
            }
        }
        let (mut gap, mut longest_gap) = (0usize, 0usize);
        for &v in tone {
            gap = if v == 0.0 { gap + 1 } else { 0 };
            longest_gap = longest_gap.max(gap);
        }
        let rms = (tone.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>()
            / tone.len().max(1) as f64)
            .sqrt();
        report += &format!(
            ",\"device_rate\":{rate},\"recorded_seconds\":{:.2},\"tone_rms\":{rms:.4},\"glitches\":{glitches},\"longest_silence_ms\":{:.2}",
            audio.len() as f64 / f64::from(rate),
            longest_gap as f64 * 1000.0 / f64::from(rate)
        );
        if o.expect_tone {
            if !(0.3..0.4).contains(&rms) {
                failures.push("tone level is wrong".to_owned());
            }
            if glitches > 0 {
                failures.push(format!("{glitches} glitches"));
            }
        }
        if let Some(s) = o.start_writer_after {
            let early = nonzero(at(0.25), at(s));
            let first = audio[at(s)..].iter().position(|v| *v != 0.0);
            report += &format!(
                ",\"audio_before_writer\":{early},\"first_audio_after_writer_ms\":{:.1}",
                first.map_or(-1.0, |i| i as f64 * 1000.0 / f64::from(rate))
            );
            if early > 0 {
                failures.push(format!("{early} samples before the writer started"));
            }
            if first.is_none_or(|i| i > at(0.3)) {
                failures.push("no audio within 300 ms of the writer starting".to_owned());
            }
        }
        if let Some(s) = o.stop_writer_after {
            let late = nonzero(at(s + 0.25), audio.len());
            report += &format!(",\"audio_after_writer_stopped\":{late}");
            if late > 0 {
                failures.push(format!("{late} samples after the writer stopped"));
            }
        }
        if !o.write && !o.expect_tone {
            let any = nonzero(at(0.25), audio.len());
            report += &format!(",\"audio_without_writer\":{any}");
            if any > 0 {
                failures.push(format!("{any} samples with no writer"));
            }
        }
    }
    println!("{{{report},\"failures\":{failures:?}}}");
    ensure!(failures.is_empty(), "{}", failures.join("; "));
    Ok(())
}
