//! Bounded physical-device check using the desktop's actual stream/control implementation.
//! Outputs counters only. This is manual hardware validation, never an automatic CI audio test.

// These shared modules also contain recording/playback/UI helpers unused by this example.
#[allow(dead_code)]
#[path = "../src/audio/audio_io.rs"]
mod audio_io;
#[allow(dead_code)]
#[path = "../src/audio/live_io.rs"]
mod live_io;
#[cfg(target_os = "linux")]
#[path = "../src/audio/pulse.rs"]
mod pulse;

use audio_io::{Message, Settings};
use eyre::{bail, ensure, eyre, Result};
use hush_core::{WorkerPlacement, WorkerScheduling};
use serde_json::json;
use std::{
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

const HELP: &str = "Physical headphone monitoring; no microphone audio is saved.
live_device --list
live_device --input NAME --output NAME [--model PATH] [--rate 48000]
  [--layers 3] [--look-ahead 0] [--threads 12] [--seconds 30] [--report-seconds 0]
  [--scheduling os-default|windows-audio] [--bypass] [--virtual-mic] [--expect-device-loss]
Use exact endpoint names from --list. Seconds must be 1..=14400. Threads 0 is Automatic.
--report-seconds N prints one JSON line every N seconds: memory, CPU and audio counters.
--virtual-mic sends output to the Hush AI Microphone; --output is then ignored.
With --expect-device-loss, unplug the named device after Monitoring appears; a normal timed stop fails.
JSON goes to stdout; diagnostics go to stderr. Exit 2 means queue loss or insufficient progress.
Startup/stop failures and a watchdog timeout exit unsuccessfully.";

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.is_empty() || args == ["--help"] {
        println!("{HELP}");
        return Ok(());
    }
    // Device discovery can block in a driver too. A process watchdog bounds that operation.
    let mut settings = Settings {
        model: PathBuf::from(
            std::env::var("HUSH_MODEL").unwrap_or_else(|_| "models/model.safetensors".into()),
        ),
        rate: 48000,
        layers: 3,
        look_ahead: 0,
        threads: 12,
        placement: WorkerPlacement::Unrestricted,
        scheduling: WorkerScheduling::OsDefault,
        bypass: false,
        prefill: 2,
        virtual_mic: false,
    };
    let mut input_name = None;
    let mut output_name = None;
    let mut seconds = 30u64;
    let mut report_seconds = 0u64;
    let mut expect_device_loss = false;
    let list = args == ["--list"];
    if !list {
        let mut args = args.iter();
        while let Some(key) = args.next() {
            if key == "--bypass" {
                settings.bypass = true;
                continue;
            }
            if key == "--virtual-mic" {
                settings.virtual_mic = true;
                continue;
            }
            if key == "--expect-device-loss" {
                expect_device_loss = true;
                continue;
            }
            let value = args
                .next()
                .ok_or_else(|| eyre!("Missing value for {key}"))?;
            match key.as_str() {
                "--input" => input_name = Some(value.clone()),
                "--output" => output_name = Some(value.clone()),
                "--model" => settings.model = value.into(),
                "--rate" => settings.rate = value.parse()?,
                "--layers" => settings.layers = value.parse()?,
                "--look-ahead" => settings.look_ahead = value.parse()?,
                "--threads" => settings.threads = value.parse()?,
                "--seconds" => seconds = value.parse()?,
                "--report-seconds" => report_seconds = value.parse()?,
                "--scheduling" => {
                    settings.scheduling = match value.as_str() {
                        "os-default" => WorkerScheduling::OsDefault,
                        "windows-audio" => WorkerScheduling::WindowsAudio,
                        _ => {
                            bail!("Unknown scheduling policy: {value}");
                        }
                    }
                }
                _ => {
                    bail!("Unknown argument: {key}");
                }
            }
        }
        ensure!(
            input_name.is_some() && output_name.is_some(),
            "--input and --output are required; use headphones"
        );
        ensure!((1..=14400).contains(&seconds), "seconds must be 1..=14400");
        ensure!(
            [8000, 16000, 22050, 24000, 32000, 44100, 48000].contains(&settings.rate),
            "Unsupported model rate"
        );
        ensure!(
            (3..=12).contains(&settings.layers) && settings.look_ahead <= 2,
            "Invalid model profile"
        );
        ensure!(settings.threads <= 256, "threads must be 0..=256");
    }
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(if list { 30 } else { seconds + 60 }));
        eprintln!("Device-test watchdog expired; closing this test process");
        std::process::exit(1);
    });
    let inputs = audio_io::devices::discover(true)?;
    let outputs = audio_io::devices::discover(false)?;
    if list {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "inputs": inputs.names(),
                "outputs": outputs.names(),
            }))?
        );
        return Ok(());
    }
    let input_name = input_name.unwrap();
    let output_name = output_name.unwrap();
    let input = inputs.named(&input_name)?;
    let output = outputs.named(&output_name)?;
    let mut report = json!({
        "input": input_name, "output": output_name,
        "requested_model_rate": settings.rate, "layers": settings.layers,
        "look_ahead": settings.look_ahead, "threads": settings.threads,
        "worker_placement": "unrestricted",
        "worker_scheduling": match settings.scheduling {
            WorkerScheduling::OsDefault => "os-default",
            WorkerScheduling::WindowsAudio => "windows-audio",
        },
        "bypass": settings.bypass, "precision": "fp32", "requested_seconds": seconds,
        "microphone_recording_saved": false,
        "validation_mode": if expect_device_loss { "device-loss" } else { "steady" },
        "method": "Desktop live_io module, physical CPAL streams; no GUI input or acoustic-latency measurement",
    });
    let (deliver, receiver) = std::sync::mpsc::channel::<Message>();
    let deliver = std::sync::Mutex::new(deliver);
    let sender = audio_io::Sender::new(move |message| {
        let _ = deliver.lock().unwrap().send(message);
    });
    let start = Instant::now();
    let session = live_io::start(input, output, settings, sender.clone(), 1)?;
    let mut active = None;
    let mut meters = None;
    let mut next_report = None;
    let mut interval = (Instant::now(), cpu_time::ProcessTime::now(), 0u64);
    let mut first_rss = None;
    let mut last_rss = 0;
    let mut stopping = None;
    let mut closing_wait = None;
    loop {
        while let Ok(message) = receiver.try_recv() {
            match message {
                Message::LiveStarted(1, Ok((rates, live_meters))) => {
                    ensure!(active.is_none(), "Duplicate startup notification");
                    active = Some(Instant::now());
                    report["worker_threads"] = json!(rates.threads);
                    meters = Some(live_meters);
                    if report_seconds > 0 {
                        next_report = Some(Instant::now() + Duration::from_secs(report_seconds));
                        interval = (Instant::now(), cpu_time::ProcessTime::now(), 0);
                    }
                    report["startup_seconds"] = json!(start.elapsed().as_secs_f64());
                    report["input_rate"] = json!(rates.input);
                    report["model_rate"] = json!(rates.model);
                    report["output_rate"] = json!(rates.output);
                    eprintln!(
                        "Monitoring: {} -> {} -> {} Hz for {seconds}s",
                        rates.input, rates.model, rates.output
                    );
                }
                Message::LiveStarted(1, Err(error)) => {
                    bail!("Live startup failed: {error}");
                }
                Message::LiveStopped(1, stop_report) => {
                    if !stop_report.devices_closed {
                        eprintln!(
                            "Waiting for actual device teardown: {:?}",
                            stop_report.reason
                        );
                        closing_wait.get_or_insert_with(Instant::now);
                        continue;
                    }
                    let stats = stop_report.stats.ok_or_else(|| {
                        eyre!("Device closed before monitoring: {:?}", stop_report.reason)
                    })?;
                    let expected_reason = if expect_device_loss {
                        matches!(
                            stop_report.reason,
                            live_io::control::StopReason::InputUnavailable
                                | live_io::control::StopReason::OutputUnavailable
                        )
                    } else {
                        stop_report.reason == live_io::control::StopReason::Requested
                            && stopping.is_some()
                    };
                    let minimum_frames = if expect_device_loss { 1 } else { seconds * 40 };
                    let progressed = active.is_some() && stats.frames_processed >= minimum_frames;
                    let clean = stats.input_dropped_samples == 0
                        && stats.output_dropped_samples == 0
                        && stats.output_underrun_samples == 0
                        && stats.discontinuities == 0;
                    report["shutdown_seconds"] =
                        json!(stopping.map(|t: Instant| t.elapsed().as_secs_f64()));
                    report["shutdown_measurement"] = json!("Request-to-final-report when manually stopped; null for external device loss. Physical unplug time is not observed.");
                    if stopping.is_none() {
                        report["observed_seconds"] =
                            json!(active.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.));
                    }
                    report["device_handles_closed"] = json!(true);
                    if let Some(first) = first_rss {
                        // First to last periodic report, while the model was loaded.
                        report["rss_growth_mb"] =
                            json!((last_rss as f64 - first as f64) / 1048576.0);
                    }
                    report["stop_reason"] = json!(format!("{:?}", stop_report.reason));
                    report["stats"] = json!({
                        "input_dropped_samples": stats.input_dropped_samples,
                        "output_dropped_samples": stats.output_dropped_samples,
                        "output_underrun_samples": stats.output_underrun_samples,
                        "output_buffering_samples": stats.output_buffering_samples,
                        "clock_correction_ppm": stats.clock_correction_ppm,
                        "discontinuities": stats.discontinuities,
                        "frames_processed": stats.frames_processed, "worker_failed": stats.worker_failed,
                    });
                    let passed = expected_reason
                        && progressed
                        && !stats.worker_failed
                        && (expect_device_loss || clean);
                    report["status"] = json!(if stats.worker_failed {
                        "worker-failed"
                    } else if !expected_reason {
                        "unexpected-stop-reason"
                    } else if !progressed {
                        "insufficient-progress"
                    } else if passed {
                        "pass"
                    } else {
                        "queue-loss"
                    });
                    report["minimum_frames"] = json!(minimum_frames);
                    report["counter_scope"] = json!(
                        "Cumulative from startup through stop fade, captured before teardown"
                    );
                    println!("{}", serde_json::to_string_pretty(&report)?);
                    if !passed {
                        std::process::exit(2);
                    }
                    return Ok(());
                }
                _ => {
                    bail!("Unexpected desktop audio message");
                }
            }
        }
        if let Some(stopped) = closing_wait.or(stopping) {
            ensure!(
                stopped.elapsed() < Duration::from_secs(10),
                "Device teardown exceeded ten seconds"
            );
        } else if let Some(active) = active {
            if let (Some(due), Some(meters)) = (next_report, &meters) {
                if Instant::now() >= due {
                    let (since, cpu, busy) = interval;
                    let wall = since.elapsed().as_secs_f64();
                    let reading = meters.take();
                    let stats = reading.stats;
                    let memory = rss();
                    first_rss.get_or_insert(memory);
                    last_rss = memory;
                    println!(
                        "{}",
                        json!({
                            "minutes": active.elapsed().as_secs_f64() / 60.0,
                            "rss_mb": memory as f64 / 1048576.0,
                            "cpu_percent": cpu.elapsed().as_secs_f64() / wall * 100.0,
                            "busy_percent": (reading.busy_ns - busy) as f64 / 1e7 / wall,
                            "peak_frame_ms": reading.peak_frame_ns as f64 / 1e6,
                            "frames": stats.frames_processed,
                            "input_dropped": stats.input_dropped_samples,
                            "output_dropped": stats.output_dropped_samples,
                            "underruns": stats.output_underrun_samples,
                            "discontinuities": stats.discontinuities,
                            "clock_correction_ppm": stats.clock_correction_ppm,
                        })
                    );
                    interval = (
                        Instant::now(),
                        cpu_time::ProcessTime::now(),
                        reading.busy_ns,
                    );
                    next_report = Some(due + Duration::from_secs(report_seconds));
                }
            }
            if active.elapsed() >= Duration::from_secs(seconds) {
                report["observed_seconds"] = json!(active.elapsed().as_secs_f64());
                stopping = Some(Instant::now());
                session.request_stop(sender.clone(), 1);
            }
        } else {
            ensure!(
                start.elapsed() < Duration::from_secs(30),
                "Device startup exceeded thirty seconds"
            );
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn rss() -> u64 {
    memory_stats::memory_stats().map_or(0, |m| m.physical_mem as u64)
}
