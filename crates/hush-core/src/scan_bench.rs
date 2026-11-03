//! Isolated development timing; absent from production and ignored by normal tests.
use super::*;
use std::{
    hint::black_box,
    time::{Duration, Instant},
};

#[repr(C, align(64))]
struct Samples([f32; STATE * 2]);

#[inline(never)]
fn run(kernel: Scan16, updates: usize, offset: usize) -> (Duration, [f32; STATE], f32) {
    let mut states = Samples([0.0; STATE * 2]);
    let aa = Samples(std::array::from_fn(|i| -((i % STATE) as f32 + 1.0) * 0.25));
    let bb = Samples(std::array::from_fn(|i| ((i % STATE) as f32 * 0.17).sin()));
    let cc = Samples(std::array::from_fn(|i| ((i % STATE) as f32 * 0.11).cos()));
    let state: &mut [f32; STATE] = (&mut states.0[offset..offset + STATE]).try_into().unwrap();
    let a = aa.0[offset..offset + STATE].try_into().unwrap();
    let b = bb.0[offset..offset + STATE].try_into().unwrap();
    let c = cc.0[offset..offset + STATE].try_into().unwrap();
    let deltas = [0.001, 0.1, 0.5, 10.0];
    let kernel = black_box(kernel);
    let mut output = 0.0;
    let start = Instant::now();
    for i in 0..updates {
        let delta = black_box(deltas[i % deltas.len()]);
        // SAFETY: the caller checks CPU support and all slices have STATE elements.
        output = unsafe { kernel(state, a, delta, delta * 0.1, b, c) };
        black_box(output);
    }
    (start.elapsed(), *state, output)
}

#[test]
#[ignore = "timing experiment: run alone with --release --lib temporal_scan_kernels -- --ignored --nocapture"]
#[allow(clippy::assertions_on_constants)]
fn temporal_scan_kernels() {
    assert!(!cfg!(feature = "profile"), "benchmark without profiling");
    if !(std::is_x86_feature_detected!("avx2") && std::is_x86_feature_detected!("avx512f")) {
        println!("temporal_scan_kernels requires AVX2 and AVX-512");
        return;
    }
    let kernels: [(&str, Scan16); 2] = [("avx2", scan16_avx2), ("avx512", scan16_avx512)];
    for offset in [0, 4, 8, 12] {
        for (_, kernel) in kernels {
            black_box(run(kernel, 50_000, offset));
        }
        let mut reference = None;
        for pair in 0..6 {
            for index in if pair % 2 == 0 { [0, 1] } else { [1, 0] } {
                let (name, kernel) = kernels[index];
                let updates = 2_000_000;
                let (elapsed, state, output) = run(kernel, updates, offset);
                let bits = (state.map(f32::to_bits), output.to_bits());
                if let Some(expected) = reference {
                    assert_eq!(bits, expected);
                } else {
                    reference = Some(bits);
                }
                println!(
                "scan16_bench offset_bytes={} pair={} kernel={name} updates={updates} mean_ns={:.3}",
                offset * std::mem::size_of::<f32>(),
                pair + 1,
                elapsed.as_secs_f64() * 1e9 / updates as f64
            );
            }
        }
    }
}

unsafe fn per_channel_avx2(
    state: &mut [f32],
    a: &[f32],
    delta: &[f32],
    x: &[f32],
    b: &[f32; STATE],
    c: &[f32; STATE],
    y: &mut [f32],
) {
    let kernel = black_box(scan16_avx2 as Scan16);
    for ch in 0..y.len() {
        // SAFETY: the benchmark checks AVX2 and supplies complete state rows.
        y[ch] = unsafe {
            kernel(
                (&mut state[ch * STATE..][..STATE]).try_into().unwrap(),
                state16(&a[ch * STATE..]),
                delta[ch],
                delta[ch] * x[ch],
                b,
                c,
            )
        };
    }
}

#[repr(C, align(64))]
struct RowSamples([f32; 128 * STATE + STATE]);

#[inline(never)]
fn run_rows(kernel: ScanRow, updates: usize, offset: usize) -> (Duration, Vec<u32>, Vec<u32>) {
    let mut storage = RowSamples([0.0; 128 * STATE + STATE]);
    let weights = RowSamples(std::array::from_fn(|i| -((i % STATE) as f32 + 1.0) * 0.25));
    let state = &mut storage.0[offset..offset + 128 * STATE];
    let a = &weights.0[offset..offset + 128 * STATE];
    let delta: [f32; 128] = std::array::from_fn(|i| [0.001, 0.01, 0.1, 10.0][i % 4]);
    let x: [f32; 128] = std::array::from_fn(|i| (i as f32 * 0.17).sin());
    let b = std::array::from_fn(|i| (i as f32 * 0.17).sin());
    let c = std::array::from_fn(|i| (i as f32 * 0.11).cos());
    let mut output = [0.0; 128];
    let kernel = black_box(kernel);
    let start = Instant::now();
    for _ in 0..updates {
        // SAFETY: the caller checks feature support, all row lengths agree.
        unsafe {
            kernel(state, a, &delta, &x, &b, &c, &mut output);
        }
        black_box(&output);
    }
    let elapsed = start.elapsed();
    (
        elapsed,
        state.iter().map(|v| v.to_bits()).collect(),
        output.map(f32::to_bits).to_vec(),
    )
}

#[test]
#[ignore = "timing experiment: run alone with --release --lib temporal_scan_rows -- --ignored --nocapture"]
#[allow(clippy::assertions_on_constants)]
fn temporal_scan_rows() {
    assert!(!cfg!(feature = "profile"), "benchmark without profiling");
    if !(std::is_x86_feature_detected!("avx2") && std::is_x86_feature_detected!("avx512f")) {
        println!("temporal_scan_rows requires AVX2 and AVX-512");
        return;
    }
    let kernels: [(&str, ScanRow); 3] = [
        ("per_channel", per_channel_avx2),
        ("row_avx2", scan_row_avx2),
        ("row_avx512", scan_row_avx512),
    ];
    for offset in [0, 4, 8, 12] {
        for (_, kernel) in kernels {
            black_box(run_rows(kernel, 1000, offset));
        }
        let mut reference = None;
        // Rotate forward and reverse so each kernel occupies every order slot.
        let orders = [
            [0, 1, 2],
            [2, 1, 0],
            [1, 2, 0],
            [0, 2, 1],
            [2, 0, 1],
            [1, 0, 2],
        ];
        for (round, order) in orders.into_iter().enumerate() {
            for index in order {
                let (name, kernel) = kernels[index];
                let updates = 20_000;
                let (duration, state, output) = run_rows(kernel, updates, offset);
                let bits = (state, output);
                if let Some(expected) = reference.as_ref() {
                    assert_eq!(&bits, expected);
                } else {
                    reference = Some(bits);
                }
                println!("scan_row_bench offset_bytes={} round={} kernel={name} rows={updates} mean_ns_per_channel={:.3}", offset * 4, round + 1, duration.as_secs_f64() * 1e9 / (updates * 128) as f64);
            }
        }
    }
}

type BlockScan = unsafe fn(&Mamba, &[f32], &[f32], &[f32], usize, usize, &mut [f32]);

struct FrequencySamples {
    dt: Vec<f32>,
    dbl: Vec<f32>,
    x: Vec<f32>,
    len: usize,
    start: usize,
}

#[inline(never)]
fn run_frequency(
    model: &Mamba,
    samples: &FrequencySamples,
    width: usize,
    kernel: BlockScan,
    repetitions: usize,
) -> (Duration, Vec<u32>) {
    let di = model.d_inner;
    let len = samples.len;
    let mut blocked = vec![0.0; len * di];
    let kernel = black_box(kernel);
    let start = Instant::now();
    for _ in 0..repetitions {
        for (block, output) in blocked.chunks_exact_mut(len * width).enumerate() {
            // SAFETY: the benchmark checks feature support and supplies complete blocks.
            unsafe {
                kernel(
                    model,
                    &samples.dt,
                    &samples.dbl,
                    &samples.x,
                    samples.start,
                    block * width,
                    output,
                );
            }
        }
        black_box(&blocked);
    }
    let elapsed = start.elapsed();
    let mut bits = vec![0; len * di];
    for (block, output) in blocked.chunks_exact(len * width).enumerate() {
        for row in 0..len {
            for ch in 0..width {
                bits[row * di + block * width + ch] = output[row * width + ch].to_bits();
            }
        }
    }
    (elapsed, bits)
}

#[test]
#[ignore = "timing experiment: run alone with --release --lib frequency_scan_widths -- --ignored --nocapture"]
#[allow(clippy::assertions_on_constants)]
fn frequency_scan_widths() {
    assert!(!cfg!(feature = "profile"), "benchmark without profiling");
    if !(std::is_x86_feature_detected!("avx2") && std::is_x86_feature_detected!("avx512f")) {
        println!("frequency_scan_widths requires AVX2 and AVX-512");
        return;
    }
    let (di, rank, len, start) = (256, 4, 481, 3);
    let nd = rank + 2 * STATE;
    let empty_linear = || Linear {
        weight: Vec::new(),
        packed: None,
        bias: None,
        out: nd,
        inp: 0,
    };
    let a: Vec<_> = (0..di * STATE)
        .map(|i| -((i % 23 + 1) as f32) * 0.25)
        .collect();
    let model = Mamba {
        scan: scan_kernel(),
        scan_row: scan_row_kernel(),
        in_proj: empty_linear(),
        x_proj: empty_linear(),
        dt_proj: empty_linear(),
        out_proj: empty_linear(),
        conv_w: Vec::new(),
        conv_taps: Vec::new(),
        conv_b: Vec::new(),
        d: Vec::new(),
        a_t: (0..di * STATE)
            .map(|i| a[(i % di) * STATE + i / di])
            .collect(),
        #[cfg(feature = "trace")]
        a_cuda: a.clone(),
        a,
        d_inner: di,
        d_state: STATE,
        d_conv: 4,
        dt_rank: rank,
    };
    let samples = FrequencySamples {
        dt: (0..(start + len) * di)
            .map(|i| [0.001, 0.01, 0.1, 10.0][i % 4])
            .collect(),
        dbl: (0..(start + len) * nd)
            .map(|i| (i as f32 * 0.13).cos())
            .collect(),
        x: (0..(start + len) * di)
            .map(|i| (i as f32 * 0.071).sin())
            .collect(),
        len,
        start,
    };
    let kernels: [(&str, usize, BlockScan); 8] = [
        ("avx2", 8, Mamba::scan_block_avx2::<8>),
        ("avx2", 16, Mamba::scan_block_avx2::<16>),
        ("avx2", 32, Mamba::scan_block_avx2::<32>),
        ("avx2", 64, Mamba::scan_block_avx2::<64>),
        ("avx512", 8, Mamba::scan_block_avx512::<8>),
        ("avx512", 16, Mamba::scan_block_avx512::<16>),
        ("avx512", 32, Mamba::scan_block_avx512::<32>),
        ("avx512", 64, Mamba::scan_block_avx512::<64>),
    ];
    for (_, width, kernel) in kernels {
        black_box(run_frequency(&model, &samples, width, kernel, 1));
    }
    let mut reference = None;
    for round in 0..8 {
        for slot in 0..8 {
            let (isa, width, kernel) = kernels[(round + slot) % 8];
            let (elapsed, bits) = run_frequency(&model, &samples, width, kernel, 10);
            if let Some(expected) = reference.as_ref() {
                assert_eq!(&bits, expected);
            } else {
                reference = Some(bits);
            }
            println!("frequency_width_bench round={} isa={isa} width={width} positions={} repetitions=10 mean_ns_per_channel={:.3}",round+1,len,elapsed.as_secs_f64()*1e9/(10*len*di) as f64);
        }
    }
}
