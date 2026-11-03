//! End-to-end, one-hop-at-a-time CPU benchmark (including STFT and overlap-add).
//!
//! `cargo run --release --example bench_stream -- [rate] [layers] [lookahead] [frames] [warmup] [wav|-] [dump|-]`
//! The optional dump is raw little-endian f32 output, useful for comparing implementations.

use cpu_time::ProcessTime;
use eyre::{ensure, Result};
use hush_core::{audio, Config, Denoiser};
use std::{fs::File, io::Write, time::Instant};
mod bench_support;

fn value<T: std::str::FromStr>(args: &[String], index: usize, default: T) -> Result<T> {
    match args.get(index) {
        Some(value) => value
            .parse()
            .map_err(|_| eyre::eyre!("invalid argument {index}: {value}")),
        None => Ok(default),
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let sr = value(&args, 1, 16000u32)?;
    let layers = value(&args, 2, 4usize)?;
    let look_ahead = value(&args, 3, 0usize)?;
    let frames = value(&args, 4, 300usize)?;
    let warmup = value(&args, 5, 25usize)?;
    ensure!(
        frames > warmup && warmup > 0,
        "frames must exceed a positive warmup"
    );
    let model = std::env::var("HUSH_MODEL").unwrap_or_else(|_| "models/model.safetensors".into());
    let mut denoiser = Denoiser::with_config(
        &model,
        Config {
            sample_rate: sr,
            layers,
            look_ahead,
        },
    )?;
    let placement = bench_support::configure_workers(&mut denoiser)?;
    let worker_threads = denoiser.worker_threads();
    let scheduling = bench_support::scheduling_name(&denoiser);
    let _registration = denoiser.worker_scheduling().enter_current_thread()?;
    let hop = denoiser.frame();
    let input = if let Some(path) = args.get(6).filter(|s| s.as_str() != "-") {
        let (samples, file_sr) = audio::load_wav(path)?;
        ensure!(file_sr == sr, "input is {file_sr} Hz, requested {sr} Hz");
        ensure!(!samples.is_empty(), "input is empty");
        (0..frames * hop)
            .map(|i| samples[i % samples.len()])
            .collect::<Vec<_>>()
    } else {
        let mut seed = 0x1234_5678u32;
        (0..frames * hop)
            .map(|i| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let noise = ((seed >> 8) as f32 / 16_777_216.0 - 0.5) * 0.08;
                let tone = (std::f32::consts::TAU * 220.0 * i as f32 / sr as f32).sin() * 0.2;
                if (i / (sr as usize)) % 4 == 3 {
                    0.0
                } else {
                    tone + noise
                }
            })
            .collect::<Vec<_>>()
    };
    let mut times = Vec::with_capacity(frames - warmup);
    let mut output = Vec::with_capacity(input.len());
    let all_start = Instant::now();
    let mut cpu_start = ProcessTime::now();
    let mut measured_start = Instant::now();
    for (i, chunk) in input.chunks_exact(hop).enumerate() {
        if i == warmup {
            #[cfg(feature = "profile")]
            hush_core::profile::reset();
            cpu_start = ProcessTime::now();
            measured_start = Instant::now();
        }
        let start = Instant::now();
        let enhanced = denoiser.process(chunk)?;
        let elapsed = start.elapsed().as_secs_f64();
        if i >= warmup {
            times.push(elapsed * 1000.0);
        }
        output.extend_from_slice(&enhanced);
    }
    let measured_cpu = cpu_start.elapsed().as_secs_f64();
    let measured_wall = measured_start.elapsed().as_secs_f64();
    #[cfg(feature = "profile")]
    hush_core::profile::report(frames - warmup);
    let rss = memory_stats::memory_stats()
        .map(|m| m.physical_mem)
        .unwrap_or(0);
    output.extend_from_slice(&denoiser.flush()?);
    let wall = all_start.elapsed().as_secs_f64();
    ensure!(output.len() == input.len(), "unexpected output length");
    if let Some(path) = args.get(7).filter(|s| s.as_str() != "-") {
        let mut file = File::create(path)?;
        for sample in &output {
            file.write_all(&sample.to_le_bytes())?;
        }
    }
    let mean = times.iter().sum::<f64>() / times.len() as f64;
    let budget = hop as f64 * 1000.0 / sr as f64;
    let overruns = times.iter().filter(|v| **v > budget).count();
    times.sort_by(f64::total_cmp);
    let percentile = |p: usize| times[((times.len() * p).div_ceil(100)).saturating_sub(1)];
    let audio_seconds = times.len() as f64 * hop as f64 / sr as f64;
    println!("{{\"os\":\"{}\",\"arch\":\"{}\",\"precision\":\"fp32\",\"sample_rate\":{sr},\"layers\":{layers},\"look_ahead\":{look_ahead},\"frames\":{frames},\"warmup_frames\":{warmup},\"hop_samples\":{hop},\"frame_budget_ms\":{budget:.4},\"mean_ms\":{mean:.4},\"p50_ms\":{:.4},\"p95_ms\":{:.4},\"p99_ms\":{:.4},\"max_ms\":{:.4},\"overruns\":{overruns},\"rtf\":{:.4},\"cpu_busy_percent\":{:.2},\"cpu_per_audio_percent\":{:.2},\"rss_bytes\":{rss},\"wall_seconds_including_flush\":{wall:.3},\"rayon_threads\":{},\"worker_placement\":\"{placement}\",\"worker_scheduling\":\"{scheduling}\"}}", std::env::consts::OS, std::env::consts::ARCH, percentile(50), percentile(95), percentile(99), times[times.len()-1], mean / budget, measured_cpu / measured_wall * 100.0, measured_cpu / audio_seconds * 100.0, worker_threads);
    Ok(())
}
