//! Paced synthetic callbacks through the real bounded LiveWorker (no audio device).
//! Usage: bench_live [rate=48000] [layers=3] [lookahead=0] [frames=600]
//!                   [warmup=100] [buffer_frames=4] [queue_frames=8]
//! Startup waits model latency plus buffer_frames; warmup counters are reported separately.

use cpu_time::ProcessTime;
use eyre::{ensure, Result};
use hush_core::{live::LiveWorker, Config, Denoiser};
use std::time::{Duration, Instant};
mod bench_support;

fn value<T: std::str::FromStr>(args: &[String], i: usize, default: T) -> Result<T> {
    args.get(i).map_or(Ok(default), |v| {
        v.parse()
            .map_err(|_| eyre::eyre!("invalid argument {i}: {v}"))
    })
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let rate = value(&args, 1, 48000u32)?;
    let layers = value(&args, 2, 3usize)?;
    let look_ahead = value(&args, 3, 0usize)?;
    let frames = value(&args, 4, 600usize)?;
    let warmup = value(&args, 5, 100usize)?;
    let buffer_frames = value(&args, 6, 4usize)?;
    let queue_frames = value(&args, 7, 8usize)?;
    ensure!(
        buffer_frames > 0 && queue_frames > buffer_frames,
        "queue must exceed positive startup buffering"
    );
    let model = std::env::var("HUSH_MODEL").unwrap_or_else(|_| "models/model.safetensors".into());
    let mut denoiser = Denoiser::with_config(
        model,
        Config {
            sample_rate: rate,
            layers,
            look_ahead,
        },
    )?;
    let placement = bench_support::configure_workers(&mut denoiser)?;
    let worker_threads = denoiser.worker_threads();
    let scheduling = bench_support::scheduling_name(&denoiser);
    let hop = denoiser.frame();
    let latency_frames = denoiser.latency().div_ceil(hop);
    let delay = latency_frames + buffer_frames;
    ensure!(
        frames > warmup && warmup > delay,
        "frames must exceed warmup, which must exceed startup delay"
    );
    let (worker, mut input, mut output) =
        LiveWorker::start(denoiser, queue_frames * hop, queue_frames * hop)?;
    let mut seed = 0x1234_5678u32;
    let signal: Vec<_> = (0..frames * hop)
        .map(|i| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let noise = ((seed >> 8) as f32 / 16_777_216.0 - 0.5) * 0.08;
            if (i / rate as usize) % 4 == 3 {
                0.0
            } else {
                (std::f32::consts::TAU * 220.0 * i as f32 / rate as f32).sin() * 0.2 + noise
            }
        })
        .collect();
    let mut sink = vec![0.0; hop];
    let ticks = frames + buffer_frames;
    let period = Duration::from_secs_f64(hop as f64 / rate as f64);
    let start = Instant::now();
    let mut cpu_start = ProcessTime::now();
    let mut measured_start = start;
    let mut base = worker.stats();
    let mut late = Vec::with_capacity(ticks - warmup);
    let mut accepted = 0usize;
    let mut max_pending = 0usize;
    let mut max_output = 0usize;
    let mut consumed = 0usize;
    let mut finite = true;
    for tick in 0..ticks {
        let deadline = start + period.mul_f64(tick as f64);
        if let Some(wait) = deadline.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
        if tick == warmup {
            base = worker.stats();
            cpu_start = ProcessTime::now();
            measured_start = Instant::now();
        }
        if tick >= warmup {
            late.push(
                Instant::now()
                    .saturating_duration_since(deadline)
                    .as_secs_f64()
                    * 1000.0,
            );
        }
        if tick < frames {
            accepted += input.push(&signal[tick * hop..][..hop]);
        }
        if tick >= delay {
            consumed += output.read(&mut sink);
            finite &= sink.iter().all(|v| v.is_finite());
        }
        if tick >= warmup {
            max_pending = max_pending
                .max(accepted.saturating_sub(worker.stats().frames_processed as usize * hop));
            max_output = max_output.max(output.available());
        }
    }
    let cpu = cpu_start.elapsed().as_secs_f64();
    let wall = measured_start.elapsed().as_secs_f64();
    // Shutdown joins the worker; exclude join time from the paced interval.
    drop(worker);
    let end = input.stats();
    late.sort_by(f64::total_cmp);
    let p99 = late[(late.len() * 99).div_ceil(100) - 1];
    let max_late = late[late.len() - 1];
    let late_hops = late
        .iter()
        .filter(|&&ms| ms >= period.as_secs_f64() * 1000.0)
        .count();
    println!(concat!(
        "{{\"mode\":\"synthetic-paced-worker\",\"sample_rate\":{rate},\"layers\":{layers},\"look_ahead\":{look_ahead},",
        "\"frames\":{frames},\"warmup_frames\":{warmup},\"buffer_frames\":{buffer_frames},\"queue_frames\":{queue_frames},",
        "\"hop_samples\":{hop},\"model_latency_frames\":{latency_frames},\"rayon_threads\":{},\"worker_placement\":\"{placement}\",\"worker_scheduling\":\"{scheduling}\",",
        "\"input_dropped_samples\":{},\"output_dropped_samples\":{},\"output_underrun_samples\":{},\"discontinuities\":{},",
        "\"warmup_input_dropped_samples\":{},\"warmup_output_dropped_samples\":{},\"warmup_output_underrun_samples\":{},",
        "\"frames_processed_total\":{},\"output_samples_consumed_total\":{consumed},\"max_pending_input_samples_estimate\":{max_pending},",
        "\"max_queued_output_samples\":{max_output},\"callback_lateness_p99_ms\":{p99:.4},\"callback_lateness_max_ms\":{max_late:.4},",
        "\"callbacks_late_by_a_hop\":{late_hops},\"cpu_busy_percent\":{:.2},\"measured_wall_seconds\":{wall:.3},",
        "\"worker_failed\":{},\"output_finite\":{finite}}}"
    ), worker_threads,
        end.input_dropped_samples - base.input_dropped_samples,
        end.output_dropped_samples - base.output_dropped_samples,
        end.output_underrun_samples - base.output_underrun_samples,
        end.discontinuities - base.discontinuities,
        base.input_dropped_samples, base.output_dropped_samples, base.output_underrun_samples,
        end.frames_processed, cpu / wall * 100.0, end.worker_failed,
        rate=rate, layers=layers, look_ahead=look_ahead, frames=frames, warmup=warmup,
        buffer_frames=buffer_frames, queue_frames=queue_frames, hop=hop, latency_frames=latency_frames,
        consumed=consumed, max_pending=max_pending, max_output=max_output, p99=p99, max_late=max_late,
        late_hops=late_hops, wall=wall, finite=finite, placement=placement, scheduling=scheduling);
    ensure!(
        !end.worker_failed && finite,
        "worker failed or produced non-finite output"
    );
    Ok(())
}
