//! DeepFilterNet 3 behind the streaming interface. Needs `HUSH_DFN_MODEL`: an absolute path to
//! `DeepFilterNet3_onnx.tar.gz`. Without it, every case is skipped.
use df::tract::{DfParams, DfTract, ReduceMask, RuntimeParams};
use hush_core::{live::LiveWorker, DeepFilter, Enhancer};
use ndarray::{ArrayView2, ArrayViewMut2};
use std::time::{Duration, Instant};

fn model_path() -> Option<String> {
    let path = std::env::var("HUSH_DFN_MODEL")
        .ok()
        .filter(|p| !p.is_empty());
    if path.is_none() {
        eprintln!("HUSH_DFN_MODEL absent; DeepFilterNet case skipped");
    }
    path
}

/// Speech-like tones in noise, `seconds` long at 48 kHz.
fn signal(seconds: f32) -> Vec<f32> {
    let mut seed = 0x1234_5678u32;
    (0..(seconds * 48000.0) as usize)
        .map(|i| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let noise = ((seed >> 8) as f32 / 16_777_216.0 - 0.5) * 0.05;
            let t = i as f32 / 48000.0;
            let envelope = (t * 3.0 * std::f32::consts::TAU).sin().max(0.0);
            let voice = (t * 180.0 * std::f32::consts::TAU).sin() * 0.3
                + (t * 540.0 * std::f32::consts::TAU).sin() * 0.1;
            voice * envelope + noise
        })
        .collect()
}

#[test]
fn matches_libdf_with_deep_filter_command_settings() {
    let Some(path) = model_path() else { return };
    let input = signal(1.5);
    // libDF called directly, as `deep-filter` 0.5.6 does with its default options.
    let runtime = RuntimeParams::default_with_ch(1)
        .with_atten_lim(100.0)
        .with_thresholds(-15.0, 35.0, 35.0)
        .with_mask_reduce(ReduceMask::MAX);
    let mut reference =
        DfTract::new(DfParams::new(path.clone().into()).unwrap(), &runtime).unwrap();
    let hop = reference.hop_size;
    let mut expected = vec![0f32; input.len() / hop * hop];
    for (noisy, enhanced) in input.chunks_exact(hop).zip(expected.chunks_exact_mut(hop)) {
        reference
            .process(
                ArrayView2::from_shape((1, hop), noisy).unwrap(),
                ArrayViewMut2::from_shape((1, hop), enhanced).unwrap(),
            )
            .unwrap();
    }

    let mut model = DeepFilter::load(&path).unwrap();
    assert_eq!((model.sample_rate(), model.frame()), (48000, 480));
    assert_eq!(model.latency(), 1440);
    let mut actual = Vec::new();
    // Chunks that do not line up with frames.
    for chunk in input.chunks(173) {
        model.process_into(chunk, &mut actual).unwrap();
    }
    // Output sample i is input sample i: libDF's output, without its 30 ms delay.
    let delay = model.latency();
    assert_eq!(actual.len(), expected.len() - delay);
    assert_eq!(actual, expected[delay..]);
    assert!(actual.iter().any(|v| v.abs() > 0.01));
}

#[test]
fn reset_and_flush_restart_the_stream() {
    let Some(path) = model_path() else { return };
    let input = signal(1.0);
    let mut model = DeepFilter::load(path).unwrap();
    model.prepare().unwrap();
    let mut first = Vec::new();
    model.process_into(&input, &mut first).unwrap();
    first.extend(model.flush().unwrap());
    assert_eq!(first.len(), input.len());
    let mut second = Vec::new();
    model.process_into(&input[..1000], &mut second).unwrap();
    model.reset();
    second.clear();
    model.process_into(&input, &mut second).unwrap();
    second.extend(model.flush().unwrap());
    assert_eq!(first, second);
}

#[test]
fn live_worker_runs_deep_filter_on_its_thread() {
    let Some(path) = model_path() else { return };
    let input = signal(0.5);
    let mut direct = DeepFilter::load(&path).unwrap();
    let mut expected = Vec::new();
    direct.process_into(&input, &mut expected).unwrap();
    // A boxed model, as the app passes it.
    let model: Box<dyn Enhancer> = Box::new(DeepFilter::load(&path).unwrap());
    let (worker, mut producer, mut consumer) =
        LiveWorker::start(model, input.len() * 2, input.len() * 2).unwrap();
    for chunk in input.chunks(441) {
        assert_eq!(producer.push(chunk), chunk.len());
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    while consumer.available() < expected.len() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut output = vec![0.0; expected.len()];
    assert_eq!(consumer.read(&mut output), expected.len());
    // The first 5 ms fade in.
    assert_eq!(&output[240..], &expected[240..]);
    assert_eq!(worker.stats().frames_processed, (input.len() / 480) as u64);
    assert!(!worker.stats().worker_failed);
    assert_eq!(worker.stats().input_dropped_samples, 0);
    assert_eq!(worker.stats().output_dropped_samples, 0);
}
