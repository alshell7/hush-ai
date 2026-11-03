use hush_core::{live::LiveWorker, Config, Denoiser};
use std::time::{Duration, Instant};

#[test]
fn device_rates_preserve_model_profile_and_reset_both_filter_histories() {
    check_device_rates_and_reset(false, false);
}

#[test]
fn clocked_worker_preserves_model_and_resets_rate_feedback_with_filter_histories() {
    check_device_rates_and_reset(true, false);
}

#[cfg(target_os = "windows")]
#[test]
fn windows_audio_worker_preserves_audio_and_discontinuity_reset() {
    check_device_rates_and_reset(true, true);
}

fn check_device_rates_and_reset(clocked: bool, audio_scheduling: bool) {
    let Ok(path) = std::env::var("HUSH_MODEL") else {
        return;
    };
    let config = Config {
        layers: 3,
        sample_rate: 16000,
        look_ahead: 0,
    };
    let source: Vec<f32> = (0..960 * 12)
        .map(|i| (i as f32 * 0.025).sin() * 0.1)
        .collect();
    // Independent offline reference includes each causal converter's 16-source-sample delay.
    let convert = |samples: &[f32], from, to| {
        let mut delayed = vec![0.0; 16];
        delayed.extend_from_slice(samples);
        let mut out = hush_core::resample::mono(&delayed, from, to).unwrap();
        out.truncate((samples.len() as u64 * to as u64).div_ceil(from as u64) as usize);
        out
    };
    let model_input = convert(&source, 48000, 16000);
    let mut direct = Denoiser::with_config(&path, config).unwrap();
    direct.set_worker_threads(2).unwrap();
    let expected = convert(&direct.process(&model_input).unwrap(), 16000, 44100);
    let mut model = Denoiser::with_config(&path, config).unwrap();
    model.set_worker_threads(2).unwrap();
    if audio_scheduling {
        if let Err(error) = model.set_worker_threads_with_options(
            2,
            hush_core::WorkerPlacement::Unrestricted,
            hush_core::WorkerScheduling::WindowsAudio,
        ) {
            eprintln!(
                "MMCSS unavailable on this host; live scheduling integration case skipped: {error}"
            );
            return;
        }
    }
    let start = if clocked {
        LiveWorker::start_clocked
    } else {
        LiveWorker::start_at_rates
    };
    let (worker, mut producer, mut consumer) = start(
        model,
        48000,
        44100,
        source.len() * 2,
        source.len() * 2,
        false,
    )
    .unwrap();
    for round in 1..=2 {
        if round == 2 {
            producer.discontinuity();
        }
        for chunk in source.chunks(173) {
            assert_eq!(producer.push(chunk), chunk.len());
        }
        let deadline = Instant::now() + Duration::from_secs(15);
        while worker.stats().frames_processed < round * 12 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(worker.stats().frames_processed, round * 12);
        let mut actual = vec![0.0; expected.len()];
        assert_eq!(consumer.read(&mut actual), expected.len());
        let fade = 44100 / 200;
        let previous = if round == 1 {
            0.0
        } else {
            *expected.last().unwrap()
        };
        let mut faded = expected.clone();
        for (i, sample) in faded[..fade].iter_mut().enumerate() {
            let old = (fade - i) as f32 / fade as f32;
            *sample = previous * old + *sample * (1.0 - old);
        }
        // Include the fade region: it must not conceal leftover resampler history on reset.
        assert_eq!(actual, faded);
    }
    assert_eq!(worker.stats().discontinuities, 1);
    assert_eq!(worker.stats().input_dropped_samples, 0);
    assert_eq!(worker.stats().output_dropped_samples, 0);
    assert!(!worker.stats().worker_failed);
    assert_eq!(worker.stats().clock_correction_ppm, 0);
}

#[test]
fn bypass_at_device_rate_preserves_frequencies_above_the_model_nyquist() {
    let Ok(path) = std::env::var("HUSH_MODEL") else {
        return;
    };
    let model = Denoiser::with_config(
        path,
        Config {
            layers: 3,
            sample_rate: 16000,
            look_ahead: 0,
        },
    )
    .unwrap();
    let source: Vec<f32> = (0..960 * 4)
        .map(|i| (std::f32::consts::TAU * 12000.0 * i as f32 / 48000.0).sin() * 0.1)
        .collect();
    let (worker, mut producer, mut consumer) = LiveWorker::start_at_rates(
        model,
        48000,
        48000,
        source.len() * 2,
        source.len() * 2,
        true,
    )
    .unwrap();
    assert_eq!(producer.push(&source), source.len());
    let deadline = Instant::now() + Duration::from_secs(15);
    while consumer.available() < source.len() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut actual = vec![0.0; source.len()];
    assert_eq!(consumer.read(&mut actual), source.len());
    assert_eq!(&actual[240..], &source[240..]);
    assert_eq!(worker.stats().input_dropped_samples, 0);
    assert_eq!(worker.stats().output_dropped_samples, 0);
}

#[test]
fn worker_matches_direct_stream_after_startup_fade() {
    let Ok(path) = std::env::var("HUSH_MODEL") else {
        return;
    };
    let config = Config {
        layers: 3,
        sample_rate: 8000,
        look_ahead: 0,
    };
    let input: Vec<f32> = (0..160 * 12)
        .map(|i| (i as f32 * 0.08).sin() * 0.1)
        .collect();
    let mut direct = Denoiser::with_config(&path, config).unwrap();
    direct.set_worker_threads(2).unwrap();
    let expected = direct.process(&input).unwrap();
    let mut model = Denoiser::with_config(&path, config).unwrap();
    model.set_worker_threads(2).unwrap();
    let (worker, mut producer, mut consumer) =
        LiveWorker::start(model, input.len() * 2, input.len() * 2).unwrap();
    assert_eq!(producer.push(&input), input.len());
    let deadline = Instant::now() + Duration::from_secs(15);
    while consumer.available() < expected.len() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut output = vec![0.0; expected.len()];
    assert_eq!(consumer.read(&mut output), expected.len());
    assert_eq!(&output[40..], &expected[40..]);
    assert!(!worker.stats().worker_failed);
    assert_eq!(worker.stats().input_dropped_samples, 0);
    assert_eq!(worker.stats().output_dropped_samples, 0);
}

#[test]
fn discontinuity_discards_stale_output_and_resets_model() {
    let Ok(path) = std::env::var("HUSH_MODEL") else {
        return;
    };
    let config = Config {
        layers: 3,
        sample_rate: 8000,
        look_ahead: 0,
    };
    let segment: Vec<f32> = (0..160 * 6)
        .map(|i| (i as f32 * 0.05).sin() * 0.15)
        .collect();
    let mut direct = Denoiser::with_config(&path, config).unwrap();
    direct.set_worker_threads(2).unwrap();
    let expected = direct.process(&segment).unwrap();
    let mut model = Denoiser::with_config(path, config).unwrap();
    model.set_worker_threads(2).unwrap();
    let (worker, mut producer, mut consumer) = LiveWorker::start(model, 4096, 4096).unwrap();
    assert_eq!(producer.push(&segment), segment.len());
    let deadline = Instant::now() + Duration::from_secs(15);
    while worker.stats().frames_processed < 6 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(worker.stats().frames_processed, 6);
    producer.discontinuity();
    assert_eq!(producer.push(&segment), segment.len());
    while worker.stats().frames_processed < 12 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(worker.stats().discontinuities, 1);
    assert_eq!(worker.stats().frames_processed, 12);
    let mut output = vec![0.0; expected.len()];
    assert_eq!(consumer.read(&mut output), expected.len());
    assert_eq!(&output[40..], &expected[40..]);
}

#[test]
fn meters_report_work_and_original_mode_plays_delay_matched_input() {
    let Ok(path) = std::env::var("HUSH_MODEL") else {
        return;
    };
    let config = Config {
        layers: 3,
        sample_rate: 8000,
        look_ahead: 0,
    };
    let input: Vec<f32> = (0..160 * 12)
        .map(|i| (i as f32 * 0.08).sin() * 0.25)
        .collect();
    let mut model = Denoiser::with_config(&path, config).unwrap();
    model.set_worker_threads(2).unwrap();
    let latency = model.latency();
    let (worker, mut producer, mut consumer) =
        LiveWorker::start(model, input.len() * 2, input.len() * 2).unwrap();
    let meters = worker.meters();
    meters.set_original(true);
    assert_eq!(producer.push(&input), input.len());
    let expected = input.len() - latency;
    let deadline = Instant::now() + Duration::from_secs(15);
    while consumer.available() < expected && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut output = vec![0.0; expected];
    assert_eq!(consumer.read(&mut output), expected);
    // After the 10 ms (80-sample) crossfade, output is the input delayed by the model latency.
    for (i, value) in output.iter().enumerate().skip(80) {
        assert!((value - input[i]).abs() < 1e-6, "sample {i}");
    }
    let reading = meters.take();
    assert!(reading.busy_ns > 0 && reading.peak_frame_ns > 0);
    assert!((reading.input_peak - 0.25).abs() < 1e-3);
    assert!(reading.output_peak > 0.2);
    assert_eq!(meters.take().peak_frame_ns, 0, "peaks reset on read");
}
