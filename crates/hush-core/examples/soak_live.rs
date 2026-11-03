//! Long-run check of the live path: memory, CPU and audio counters over time (no audio device).
//! Usage: soak_live [minutes=60] [rate=16000] [layers=3] [threads=0] [report_seconds=60]
//! Zero threads selects the count automatically, as the app does.
//!
//! Synthetic 48 kHz microphone and speaker callbacks drive the clocked `LiveWorker`, with the
//! speaker clock 100 ppm fast, so resampling and drift correction run as in the app.
//! Prints one JSON line per report. Fails if memory grows after the first report, or if audio
//! is lost after it.

use cpu_time::ProcessTime;
use eyre::{ensure, Result};
use hush_core::{live::LiveWorker, Config, Denoiser};
use std::time::{Duration, Instant};

const DEVICE_RATE: u32 = 48_000;
const CALLBACK: usize = 480; // 10 ms
const SPEAKER_PPM: f64 = 100.0;
/// Allowed RSS growth between the first report and the end.
const MAX_GROWTH_BYTES: u64 = 8 << 20;

fn value<T: std::str::FromStr>(args: &[String], i: usize, default: T) -> Result<T> {
    args.get(i).map_or(Ok(default), |v| {
        v.parse()
            .map_err(|_| eyre::eyre!("invalid argument {i}: {v}"))
    })
}

fn rss() -> u64 {
    memory_stats::memory_stats().map_or(0, |m| m.physical_mem as u64)
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let minutes = value(&args, 1, 60f64)?;
    let rate = value(&args, 2, 16000u32)?;
    let layers = value(&args, 3, 3usize)?;
    let threads = value(&args, 4, 0usize)?;
    let report = Duration::from_secs(value(&args, 5, 60u64)?);
    let model = std::env::var("HUSH_MODEL").unwrap_or_else(|_| "models/model.safetensors".into());
    let mut denoiser = Denoiser::with_config(
        model,
        Config {
            sample_rate: rate,
            layers,
            look_ahead: 0,
        },
    )?;
    let threads = if threads == 0 {
        denoiser.set_automatic_worker_threads(Default::default(), Default::default())?
    } else {
        denoiser.set_worker_threads(threads)?;
        threads
    };
    println!("{{\"threads\":{threads}}}");
    let device_frame = (denoiser.frame() * DEVICE_RATE as usize).div_ceil(rate as usize);
    let (worker, mut input, mut output) = LiveWorker::start_clocked_with_prefill(
        denoiser,
        DEVICE_RATE,
        DEVICE_RATE,
        device_frame * 4,
        device_frame * 4,
        false,
        2,
    )?;
    let meters = worker.meters();

    let mut seed = 0x1234_5678u32;
    let mut phase = 0f32;
    let mut mic = vec![0f32; CALLBACK];
    let mut speaker = vec![0f32; CALLBACK];
    let period = Duration::from_secs_f64(CALLBACK as f64 / DEVICE_RATE as f64);
    let speaker_period = period.div_f64(1.0 + SPEAKER_PPM * 1e-6);
    let start = Instant::now();
    let end = start + Duration::from_secs_f64(minutes * 60.0);
    let (mut next_mic, mut next_speaker) = (start, start + period * 4);
    let (mut next_report, mut last_report) = (start + report, start);
    let mut cpu = ProcessTime::now();
    let mut base = worker.stats();
    let mut busy = 0u64;
    let mut after_first = None;
    let (mut first_rss, mut peak_rss) = (None, 0u64);
    let mut finite = true;
    let mut written = 0u64;
    // Real device callbacks run at real-time priority; give the simulated ones the same.
    let _audio = hush_core::audio_thread::periodic(period);
    while Instant::now() < end {
        let next = next_mic.min(next_speaker).min(next_report);
        if let Some(wait) = next.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
        let now = Instant::now();
        if now >= next_mic {
            for v in &mut mic {
                // Tone and noise for 3 s, then 1 s of silence.
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let noise = ((seed >> 8) as f32 / 16_777_216.0 - 0.5) * 0.08;
                phase = (phase + 220.0 / DEVICE_RATE as f32).fract();
                let on = (written / DEVICE_RATE as u64) % 4 != 3;
                *v = if on {
                    (std::f32::consts::TAU * phase).sin() * 0.2 + noise
                } else {
                    0.0
                };
                written += 1;
            }
            input.push(&mic);
            next_mic += period;
        }
        if now >= next_speaker {
            output.read(&mut speaker);
            finite &= speaker.iter().all(|v| v.is_finite());
            next_speaker += speaker_period;
        }
        if now >= next_report {
            let interval = now.duration_since(last_report).as_secs_f64();
            let reading = meters.take();
            let stats = reading.stats;
            let memory = rss();
            first_rss.get_or_insert(memory);
            peak_rss = peak_rss.max(memory);
            println!(
                "{{\"minutes\":{:.2},\"rss_mb\":{:.2},\"cpu_percent\":{:.1},\"busy_percent\":{:.1},\"peak_frame_ms\":{:.2},\"frames\":{},\"input_dropped\":{},\"output_dropped\":{},\"underruns\":{},\"discontinuities\":{},\"clock_correction_ppm\":{}}}",
                now.duration_since(start).as_secs_f64() / 60.0,
                memory as f64 / 1048576.0,
                cpu.elapsed().as_secs_f64() / interval * 100.0,
                (reading.busy_ns - busy) as f64 / 1e7 / interval,
                reading.peak_frame_ns as f64 / 1e6,
                stats.frames_processed - base.frames_processed,
                stats.input_dropped_samples - base.input_dropped_samples,
                stats.output_dropped_samples - base.output_dropped_samples,
                stats.output_underrun_samples - base.output_underrun_samples,
                stats.discontinuities - base.discontinuities,
                stats.clock_correction_ppm,
            );
            cpu = ProcessTime::now();
            busy = reading.busy_ns;
            after_first.get_or_insert(stats);
            base = stats;
            last_report = now;
            next_report += report;
        }
    }
    let total = worker.stats();
    drop(worker);
    let growth = rss().saturating_sub(first_rss.unwrap_or(0));
    println!(
        "{{\"summary\":true,\"rss_growth_mb\":{:.2},\"peak_rss_mb\":{:.2},\"input_dropped\":{},\"output_dropped\":{},\"underruns\":{},\"discontinuities\":{},\"worker_failed\":{},\"output_finite\":{finite}}}",
        growth as f64 / 1048576.0,
        peak_rss as f64 / 1048576.0,
        total.input_dropped_samples,
        total.output_dropped_samples,
        total.output_underrun_samples,
        total.discontinuities,
        total.worker_failed,
    );
    ensure!(
        !total.worker_failed && finite,
        "worker failed or output was not finite"
    );
    ensure!(growth <= MAX_GROWTH_BYTES, "memory grew by {growth} bytes");
    if let Some(first) = after_first {
        let lost = (total.input_dropped_samples - first.input_dropped_samples)
            + (total.output_dropped_samples - first.output_dropped_samples)
            + (total.output_underrun_samples - first.output_underrun_samples);
        ensure!(
            lost == 0,
            "{lost} samples were dropped or missing after the first report"
        );
    }
    Ok(())
}
