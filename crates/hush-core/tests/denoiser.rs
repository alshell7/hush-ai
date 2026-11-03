//! `Denoiser` streaming behaviour. Tests that need the model run only when `HUSH_MODEL` points at
//! `model.safetensors` from huggingface.co/nvidia/Real-time_RE-USE; otherwise they are skipped.

use hush_core::{audio, Config, Denoiser};

const LAYERS: usize = 4;

fn model() -> Option<String> {
    let path = std::env::var("HUSH_MODEL").ok();
    if path.is_none() {
        eprintln!("HUSH_MODEL not set, skipping");
    }
    path
}

fn denoiser(path: &str, sample_rate: u32, look_ahead: usize) -> Denoiser {
    let config = Config {
        layers: LAYERS,
        sample_rate,
        look_ahead,
    };
    Denoiser::with_config(path, config).expect("failed to load HUSH_MODEL")
}

/// Every supported sample rate with every look-ahead.
fn configs() -> impl Iterator<Item = (u32, usize)> {
    audio::SAMPLE_RATES
        .into_iter()
        .flat_map(|sr| (0..=2).map(move |la| (sr, la)))
}

/// A tone with noise, deterministic.
fn signal(len: usize) -> Vec<f32> {
    let mut seed = 1u32;
    (0..len)
        .map(|i| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let noise = (seed >> 8) as f32 / (1 << 24) as f32 - 0.5;
            0.3 * (i as f32 * 0.07).sin() + 0.05 * noise
        })
        .collect()
}

fn run(denoiser: &mut Denoiser, input: &[f32], chunk: usize) -> Vec<f32> {
    let mut out = Vec::new();
    for piece in input.chunks(chunk) {
        out.extend(denoiser.process(piece).unwrap());
    }
    out.extend(denoiser.flush().unwrap());
    out
}

#[test]
fn missing_model_is_an_error() {
    assert!(Denoiser::load("does/not/exist.safetensors", LAYERS).is_err());
}

#[test]
fn unsupported_settings_are_errors() {
    assert!(audio::stft_sizes(11025).is_err());
    let Some(path) = model() else {
        return;
    };
    let bad = [
        Config {
            sample_rate: 11025,
            ..Config::default()
        },
        Config {
            look_ahead: 3,
            ..Config::default()
        },
        Config {
            layers: 13,
            ..Config::default()
        },
    ];
    for config in bad {
        assert!(Denoiser::with_config(&path, config).is_err(), "{config:?}");
    }
}

#[test]
fn stft_sizes_match_nvidia() {
    let sizes = audio::SAMPLE_RATES.map(|sr| audio::stft_sizes(sr).unwrap());
    assert_eq!(
        sizes,
        [
            (320, 160),
            (640, 320),
            (882, 442),
            (960, 480),
            (1280, 640),
            (1764, 882),
            (1920, 960)
        ]
    );
}

#[test]
fn output_lags_input_by_latency() {
    let Some(path) = model() else {
        return;
    };
    for (sr, la) in configs() {
        let mut d = denoiser(&path, sr, la);
        assert_eq!(d.frame(), audio::stft_sizes(sr).unwrap().1);
        let input = signal(10 * d.frame());
        let out = d.process(&input).unwrap();
        // Output comes in whole frames, so it can trail by up to one frame more.
        let ready = input.len() - d.latency();
        assert!(
            out.len() <= ready && out.len() + d.frame() > ready,
            "{sr} Hz, look-ahead {la}"
        );
    }
}

#[test]
fn flush_returns_every_sample() {
    let Some(path) = model() else {
        return;
    };
    for (sr, la) in configs() {
        let mut d = denoiser(&path, sr, la);
        for len in [
            0,
            1,
            d.frame() - 1,
            d.frame(),
            d.frame() + 1,
            1000,
            sr as usize / 5,
        ] {
            let out = run(&mut d, &signal(len), 777);
            assert_eq!(out.len(), len, "{sr} Hz, look-ahead {la}, {len} samples");
        }
    }
}

#[test]
fn chunk_size_does_not_change_output() {
    let Some(path) = model() else {
        return;
    };
    for (sr, la) in configs() {
        let mut d = denoiser(&path, sr, la);
        let input = signal(sr as usize / 5);
        let whole = run(&mut d, &input, input.len());
        for chunk in [1, 160, 999] {
            assert_eq!(
                run(&mut d, &input, chunk),
                whole,
                "{sr} Hz, look-ahead {la}, chunks of {chunk}"
            );
        }
    }
}

#[test]
fn reset_starts_a_new_stream() {
    let Some(path) = model() else {
        return;
    };
    for (sr, la) in configs() {
        let mut d = denoiser(&path, sr, la);
        let input = signal(4000);
        let first = run(&mut d, &input, 500);
        d.process(&signal(1234)).unwrap();
        d.reset();
        assert_eq!(run(&mut d, &input, 500), first, "{sr} Hz, look-ahead {la}");
    }
}

#[test]
fn preparation_preserves_first_frame_timing_and_audio_after_partial_streams() {
    let Some(path) = model() else {
        return;
    };
    for (sr, la) in [(8000, 0), (16000, 1), (44100, 2), (48000, 0)] {
        let mut d = denoiser(&path, sr, la);
        let input = signal(7 * d.frame() + 13);
        let mut expected = Vec::new();
        for part in input.chunks(d.frame()) {
            expected.push(d.process(part).unwrap());
        }
        let tail = d.flush().unwrap();
        // Preparing must discard both incomplete audio and already-updated model states.
        d.process(&input[..3 * d.frame() + 7]).unwrap();
        d.prepare().unwrap();
        d.prepare().unwrap();
        for (part, expected) in input.chunks(d.frame()).zip(expected) {
            assert_eq!(
                d.process(part).unwrap(),
                expected,
                "{sr} Hz, look-ahead {la}"
            );
        }
        assert_eq!(d.flush().unwrap(), tail);
    }
}

#[test]
fn automatic_workers_reset_the_stream_and_preserve_samples() {
    let Some(path) = model() else {
        return;
    };
    // Longer than one automatic load window, so a worker may be added mid-stream.
    let input = signal(24000);
    let mut reference = denoiser(&path, 16000, 0);
    let expected = run(&mut reference, &input, 320);
    let mut actual = denoiser(&path, 16000, 0);
    actual.process(&input[..5000]).unwrap();
    let limit = std::thread::available_parallelism().map_or(1, |n| (n.get() / 2).max(1));
    let chosen = actual
        .set_automatic_worker_threads(Default::default(), Default::default())
        .unwrap();
    assert!((1..=limit).contains(&chosen));
    assert_eq!(actual.worker_threads(), chosen);
    assert_eq!(run(&mut actual, &input, 320), expected);
    assert!((chosen..=limit).contains(&actual.worker_threads()));
}

#[test]
fn reusable_output_and_dedicated_pool_preserve_samples() {
    let Some(path) = model() else {
        return;
    };
    let input = signal(6400);
    let mut reference = denoiser(&path, 16000, 1);
    let expected = run(&mut reference, &input, 319);
    let mut actual = denoiser(&path, 16000, 1);
    actual.set_worker_threads(2).unwrap();
    let mut out = vec![123.0];
    for chunk in input.chunks(319) {
        actual.process_into(chunk, &mut out).unwrap();
    }
    out.extend(actual.flush().unwrap());
    assert_eq!(out[0], 123.0);
    assert_eq!(&out[1..], expected);
}

#[test]
fn edge_signals_stay_finite_and_reset_deterministically() {
    let Some(path) = model() else {
        return;
    };
    let mut d = denoiser(&path, 16000, 0);
    let mut input = vec![0.0; 6400];
    input[0] = 1.0;
    input[319] = -1.0;
    input[3200..4000].fill(1.0);
    let first = run(&mut d, &input, 97);
    assert_eq!(first.len(), input.len());
    assert!(first.iter().all(|x| x.is_finite()));
    assert_eq!(first, run(&mut d, &input, 641));
}

#[test]
fn scheduling_changes_and_failed_changes_preserve_stream_state() {
    use hush_core::{WorkerPlacement as Placement, WorkerScheduling as Scheduling};
    let Some(path) = model() else {
        return;
    };
    for (rate, ahead) in [(16000, 1), (48000, 2)] {
        let mut reference = denoiser(&path, rate, ahead);
        reference.set_worker_threads(2).unwrap();
        let input = signal(reference.frame() * 8 + 13);
        let expected = run(&mut reference, &input, 319);
        let mut actual = denoiser(&path, rate, ahead);
        actual.set_worker_threads(2).unwrap();
        let split = actual.frame() * 2 + 13;
        let mut output = actual.process(&input[..split]).unwrap();
        let enabled = actual
            .set_worker_threads_with_options(2, Placement::Unrestricted, Scheduling::WindowsAudio)
            .is_ok();
        if !cfg!(target_os = "windows") {
            assert!(!enabled);
        }
        let selected = if enabled {
            Scheduling::WindowsAudio
        } else {
            Scheduling::OsDefault
        };
        assert_eq!(actual.worker_scheduling(), selected);
        assert!(actual
            .set_worker_threads_with_options(3, Placement::PhysicalCores, Scheduling::WindowsAudio)
            .is_err());
        assert_eq!(actual.worker_scheduling(), selected);
        assert_eq!(actual.worker_threads(), 2);
        {
            let _guard = selected.enter_current_thread().unwrap();
            actual
                .process_into(&input[split..2 * split], &mut output)
                .unwrap();
        }
        actual.set_worker_threads(2).unwrap();
        assert_eq!(actual.worker_scheduling(), Scheduling::OsDefault);
        actual
            .process_into(&input[2 * split..], &mut output)
            .unwrap();
        output.extend(actual.flush().unwrap());
        assert_eq!(output, expected, "{rate} Hz scheduling enabled={enabled}");
        if enabled {
            actual
                .set_worker_threads_with_options(
                    0,
                    Placement::Unrestricted,
                    Scheduling::WindowsAudio,
                )
                .unwrap();
            assert!(actual.worker_threads() > 0);
            assert_eq!(actual.worker_scheduling(), Scheduling::WindowsAudio);
            actual.set_worker_threads(2).unwrap();
            assert_eq!(actual.worker_scheduling(), Scheduling::OsDefault);
        }
        eprintln!(
            "Scheduling stream identity verified: {rate} Hz, Windows Audio enabled={enabled}"
        );
    }
}

#[cfg(all(target_os = "windows", target_pointer_width = "64"))]
#[test]
fn physical_worker_placement_preserves_audio_and_failed_changes_preserve_pool() {
    let Some(path) = model() else {
        return;
    };
    // Eight kHz exercises the scalar/small-product paths; 48 kHz exercises packed matrices.
    for (sr, ahead) in [(8000, 0), (48000, 2)] {
        let mut d = denoiser(&path, sr, ahead);
        d.set_worker_threads_with_placement(0, hush_core::WorkerPlacement::PhysicalCores)
            .unwrap();
        let workers = d.worker_threads();
        d.set_worker_threads(workers).unwrap();
        let input = signal(d.frame() * 10 + 13);
        let expected = run(&mut d, &input, 319);
        d.set_worker_threads_with_placement(0, hush_core::WorkerPlacement::PhysicalCores)
            .unwrap();
        assert!(d
            .set_worker_threads_with_placement(
                usize::MAX,
                hush_core::WorkerPlacement::PhysicalCores
            )
            .is_err());
        assert_eq!(d.worker_threads(), workers);
        assert_eq!(run(&mut d, &input, 319), expected);
        d.set_worker_threads(workers).unwrap();
        assert_eq!(run(&mut d, &input, 319), expected);
    }
}

#[test]
fn upstream_fp32_synthetic_regression() {
    let Some(path) = model() else {
        return;
    };
    let sr = 16000u32;
    let mut seed = 0x1234_5678u32;
    let input: Vec<f32> = (0..60 * 320)
        .map(|i| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let noise = ((seed >> 8) as f32 / 16_777_216.0 - 0.5) * 0.08;
            let tone = (std::f32::consts::TAU * 220.0 * i as f32 / sr as f32).sin() * 0.2;
            if (i / sr as usize) % 4 == 3 {
                0.0
            } else {
                tone + noise
            }
        })
        .collect();
    let bytes = include_bytes!("fixtures/upstream-fp32-16k-l4.f32");
    let expected: Vec<f32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect();
    let mut d = denoiser(&path, sr, 0);
    let actual = run(&mut d, &input, 320);
    assert_eq!(actual.len(), expected.len());
    let mut error = 0.0f64;
    let mut power = 0.0f64;
    let mut max_error = 0.0f32;
    for (&a, &b) in actual.iter().zip(&expected) {
        let delta = a - b;
        error += (delta as f64).powi(2);
        power += (b as f64).powi(2);
        max_error = max_error.max(delta.abs());
    }
    let snr = if error == 0.0 {
        f64::INFINITY
    } else {
        10.0 * (power / error).log10()
    };
    assert!(
        snr >= 70.0 && max_error < 0.002,
        "FP32 drift: SNR {snr:.1} dB, max error {max_error}"
    );
}
