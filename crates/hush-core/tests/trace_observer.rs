#![cfg(feature = "trace")]

use hush_core::Streaming;
use std::collections::BTreeMap;

fn bits(values: &[f32]) -> Vec<u32> {
    values.iter().map(|v| v.to_bits()).collect()
}

#[test]
fn encoder_operator_replay_matches_stream_history() {
    use std::collections::VecDeque;
    let checkpoint = std::env::var("HUSH_MODEL").expect("encoder replay requires HUSH_MODEL");
    for ahead in 0..=2 {
        let mut model = Streaming::load(&checkpoint, 3, ahead).unwrap();
        let mut history: VecDeque<_> = (0..2 - ahead).map(|_| vec![0.; 2 * 963]).collect();
        for frame in 0..ahead + 4 {
            let mag: Vec<_> = (0..961)
                .map(|i| ((i * 13 + frame * 17) % 99) as f32 / 100.)
                .collect();
            let phase: Vec<_> = (0..961).map(|i| (i % 47) as f32 / 10. - 2.).collect();
            let mut input = vec![0.; 2 * 963];
            input[..961].copy_from_slice(&mag);
            input[963..963 + 961].copy_from_slice(&phase);
            history.push_back(input);
            let mut observed = None;
            let next = model.step_with_trace(&mag, &phase, &mut |name, _, values| {
                if name == "encoder.in" {
                    observed = Some(bits(values));
                }
            });
            if next.is_none() {
                assert!(observed.is_none());
                continue;
            }
            assert_eq!(history.len(), 3);
            let mut padded = vec![0.; 2 * 3 * 965];
            for c in 0..2 {
                for (t, past) in history.iter().enumerate() {
                    let at = (c * 3 + t) * 965 + 1;
                    padded[at..at + 963].copy_from_slice(&past[c * 963..(c + 1) * 963]);
                }
            }
            let conv = model
                .replay_encoder_operator("in.conv", &padded, &[1, 2, 3, 965])
                .unwrap();
            let output = model
                .replay_encoder_operator("in.act", &conv, &[1, 64, 1, 963])
                .unwrap();
            assert_eq!(bits(&output), observed.unwrap());
            let native_block = model
                .replay_encoder_operator("in.block", &padded, &[1, 2, 3, 965])
                .unwrap();
            assert_eq!(bits(&output), bits(&native_block));
            let norm = model
                .replay_encoder_operator("in.norm_cuda", &conv, &[1, 64, 1, 963])
                .unwrap();
            let prelu = model
                .replay_encoder_operator("in.prelu", &norm, &[1, 64, 1, 963])
                .unwrap();
            let combined = model
                .replay_encoder_operator("in.act_cuda", &conv, &[1, 64, 1, 963])
                .unwrap();
            assert_eq!(bits(&prelu), bits(&combined));
            let cuda_block = model
                .replay_encoder_operator("in.block_cuda", &padded, &[1, 2, 3, 965])
                .unwrap();
            assert_eq!(bits(&combined), bits(&cuda_block));
            for mode in ["mean_cuda", "rstd_cuda"] {
                assert_eq!(
                    model
                        .replay_encoder_operator(&format!("in.{mode}"), &conv, &[1, 64, 1, 963])
                        .unwrap()
                        .len(),
                    963
                );
            }
            let mut nonzero_pad = padded.clone();
            nonzero_pad[0] = 1.;
            assert!(model
                .replay_encoder_operator("in.conv", &nonzero_pad, &[1, 2, 3, 965])
                .is_err());
            history.pop_front();
        }
        for (name, values, shape) in [
            ("missing", vec![0.; 64], vec![1, 64, 1, 1]),
            ("dense.4.act", vec![0.; 64], vec![1, 64, 1, 1]),
            ("in.unknown", vec![0.; 64], vec![1, 64, 1, 1]),
            ("in.conv", vec![0.; 64], vec![1, 64, 1, 1]),
            ("dense.2.conv", vec![0.; 64], vec![1, 64, 1, 1]),
            ("out.act", vec![f32::NAN; 64], vec![1, 64, 1, 1]),
            ("in.act", vec![0.; 64], vec![1, 1, 1, 64]),
            ("in.act", vec![], vec![1, 64, 1, 0]),
            ("in.act", vec![0.; 64], vec![usize::MAX, 64, 1, 1]),
        ] {
            assert!(model
                .replay_encoder_operator(name, &values, &shape)
                .is_err());
        }
    }
}

#[test]
fn encoder_blocks_and_norm_match_controlled_t4() {
    let checkpoint = std::env::var("HUSH_MODEL").expect("encoder GPU replay requires HUSH_MODEL");
    let model = Streaming::load(checkpoint, 3, 0).unwrap();
    let check = |name: &str, shape: &[usize], input: &[u8], expected: &[u8]| {
        let input: Vec<_> = input
            .as_chunks::<4>()
            .0
            .iter()
            .map(|v| f32::from_le_bytes(*v))
            .collect();
        let expected: Vec<_> = expected
            .as_chunks::<4>()
            .0
            .iter()
            .map(|v| u32::from_le_bytes(*v))
            .collect();
        let actual = model.replay_encoder_operator(name, &input, shape).unwrap();
        assert_eq!(bits(&actual), expected, "{name}");
    };
    check(
        "in.block_cuda",
        &[1, 2, 3, 965],
        include_bytes!("fixtures/encoder-operators/00458-in.input.f32"),
        include_bytes!("fixtures/encoder-operators/00458-in.output.f32"),
    );
    check(
        "dense.2.norm_cuda",
        &[1, 64, 1, 963],
        include_bytes!("fixtures/encoder-operators/00050-dense.2.conv.f32"),
        include_bytes!("fixtures/encoder-operators/00050-dense.2.norm.f32"),
    );
}

#[test]
fn complete_frequency_blocks_match_controlled_t4() {
    use hush_core::trace::FrequencyMath::ExpNormSfu;
    let checkpoint = std::env::var("HUSH_MODEL").expect("GPU block replay requires HUSH_MODEL");
    let model = Streaming::load(checkpoint, 3, 0).unwrap();
    let cases: [(usize, &[u8], &[u8]); 3] = [
        (
            0,
            include_bytes!("fixtures/frequency-blocks/00000-layer0-input.f32"),
            include_bytes!("fixtures/frequency-blocks/00000-layer0-output.f32"),
        ),
        (
            1,
            include_bytes!("fixtures/frequency-blocks/01381-layer1-input.f32"),
            include_bytes!("fixtures/frequency-blocks/01381-layer1-output.f32"),
        ),
        (
            2,
            include_bytes!("fixtures/frequency-blocks/01781-layer2-input.f32"),
            include_bytes!("fixtures/frequency-blocks/01781-layer2-output.f32"),
        ),
    ];
    for (layer, input, expected) in cases {
        let input: Vec<_> = input
            .as_chunks::<4>()
            .0
            .iter()
            .map(|v| f32::from_le_bytes(*v))
            .collect();
        let expected: Vec<_> = expected
            .as_chunks::<4>()
            .0
            .iter()
            .map(|v| u32::from_le_bytes(*v))
            .collect();
        assert_eq!(input.len(), 481 * 64);
        assert_eq!(expected.len(), input.len());
        let actual = model
            .replay_frequency_block_with_math_diagnostic(layer, &input, ExpNormSfu)
            .unwrap();
        assert_eq!(
            bits(&actual),
            expected,
            "complete GPU frequency block {layer}"
        );
    }
}

#[test]
fn combined_frequency_replay_matches_stream_and_observer() {
    use hush_core::trace::FrequencyMath::ExpNormSfu;
    let checkpoint = std::env::var("HUSH_MODEL").expect("block replay requires HUSH_MODEL");
    for ahead in 0..=2 {
        let mut normal = Streaming::load(&checkpoint, 3, ahead).unwrap();
        let mut observed = Streaming::load(&checkpoint, 3, ahead).unwrap();
        for frame in 0..ahead + 3 {
            let mag: Vec<_> = (0..961)
                .map(|i| ((i * 17 + frame * 13) % 101) as f32 / 100.)
                .collect();
            let phase: Vec<_> = (0..961).map(|i| (i % 41) as f32 / 10. - 2.).collect();
            let expected = normal
                .step_with_frequency_math_diagnostic(&mag, &phase, ExpNormSfu)
                .unwrap();
            let mut tensors = BTreeMap::new();
            let actual = observed
                .step_with_frequency_math_trace_diagnostic(
                    &mag,
                    &phase,
                    ExpNormSfu,
                    &mut |name, shape, values| {
                        assert_eq!(shape.iter().product::<usize>(), values.len());
                        assert!(tensors.insert(name.to_owned(), bits(values)).is_none());
                    },
                )
                .unwrap();
            match (expected, actual) {
                (None, None) => assert!(tensors.is_empty()),
                (Some((m, p)), Some((a, b))) => {
                    assert_eq!(bits(&m), bits(&a));
                    assert_eq!(bits(&p), bits(&b));
                    for layer in 0..3 {
                        let input: Vec<_> = tensors[&format!("blocks.{layer}.time_residual")]
                            .iter()
                            .map(|v| f32::from_bits(*v))
                            .collect();
                        let replay = observed
                            .replay_frequency_block_with_math_diagnostic(layer, &input, ExpNormSfu)
                            .unwrap();
                        assert_eq!(bits(&replay), tensors[&format!("blocks.{layer}.freq")]);
                    }
                }
                _ => panic!("observer changed diagnostic look-ahead"),
            }
        }
        for (layer, input) in [
            (3, vec![0.; 481 * 64]),
            (0, vec![]),
            (0, vec![0.; 480 * 64]),
            (0, vec![f32::NAN; 481 * 64]),
        ] {
            assert!(observed
                .replay_frequency_block_with_math_diagnostic(layer, &input, ExpNormSfu)
                .is_err());
        }
    }
}

#[test]
fn sliced_projection_matches_t4_witnesses() {
    let checkpoint = std::env::var("HUSH_MODEL").expect("projection test requires HUSH_MODEL");
    let model = Streaming::load(checkpoint, 3, 0).unwrap();
    let expand = |raw: &[u8], width: usize| -> Vec<f32> {
        let values: Vec<_> = raw
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect();
        assert_eq!(values.len(), 5 * width);
        values
            .chunks_exact(width)
            .cycle()
            .take(481)
            .flatten()
            .copied()
            .collect()
    };
    for (name, input, expected, width, output) in [
        (
            "fw.x_proj_sliced",
            include_bytes!("fixtures/sliced-projections/fw-x-input.f32").as_slice(),
            include_bytes!("fixtures/sliced-projections/fw-x-output.f32").as_slice(),
            256,
            36,
        ),
        (
            "bw.out_proj_sliced",
            include_bytes!("fixtures/sliced-projections/bw-out-input.f32").as_slice(),
            include_bytes!("fixtures/sliced-projections/bw-out-output.f32").as_slice(),
            256,
            64,
        ),
        (
            "projection_sliced",
            include_bytes!("fixtures/sliced-projections/block-input.f32").as_slice(),
            include_bytes!("fixtures/sliced-projections/block-output.f32").as_slice(),
            128,
            64,
        ),
    ] {
        let actual = model
            .replay_frequency_operator(0, name, &expand(input, width))
            .unwrap();
        assert_eq!(bits(&actual), bits(&expand(expected, output)), "{name}");
    }
}

#[test]
fn frequency_sfu_diagnostic_checks_shape_and_resets() {
    check_frequency_reset(hush_core::trace::FrequencyMath::ScanSfu);
    check_frequency_reset(hush_core::trace::FrequencyMath::ConvGateSfu);
    check_frequency_reset(hush_core::trace::FrequencyMath::SlicedSfu);
    check_frequency_reset(hush_core::trace::FrequencyMath::ExpNormSfu);
}

fn check_frequency_reset(math: hush_core::trace::FrequencyMath) {
    let checkpoint = std::env::var("HUSH_MODEL").expect("diagnostic test requires HUSH_MODEL");
    let mut model = Streaming::load(checkpoint, 3, 1).unwrap();
    let mag: Vec<_> = (0..961).map(|i| (i % 19) as f32 / 100.).collect();
    let phase = vec![0.25; 961];
    let mut first = Vec::new();
    for segment in 0..2 {
        model.reset();
        assert!(model
            .step_with_frequency_math_diagnostic(&mag[..960], &phase, math)
            .is_err());
        assert!(model
            .step_with_frequency_math_diagnostic(&mag, &[f32::NAN; 961], math)
            .is_err());
        for frame in 0..3 {
            let output = model
                .step_with_frequency_math_diagnostic(&mag, &phase, math)
                .unwrap();
            assert_eq!(output.is_none(), frame == 0);
            let value = output.map(|(m, p)| {
                assert!(m.iter().chain(&p).all(|x| x.is_finite()));
                (bits(&m), bits(&p))
            });
            if segment == 0 {
                first.push(value);
            } else {
                assert_eq!(value, first[frame]);
            }
        }
    }
}

#[test]
fn observer_preserves_outputs_states_lookahead_and_resets() {
    let checkpoint = std::env::var("HUSH_MODEL").expect("trace identity test requires HUSH_MODEL");
    for ahead in 0..=2 {
        let mut normal = Streaming::load(&checkpoint, 3, ahead).unwrap();
        let mut traced = Streaming::load(&checkpoint, 3, ahead).unwrap();
        for segment in 0..2 {
            normal.reset();
            traced.reset();
            for t in 0..8 {
                let mag: Vec<_> = (0..161)
                    .map(|f| ((17 * f + 13 * t + segment) % 101) as f32 / 100.)
                    .collect();
                let phase: Vec<_> = (0..161)
                    .map(|f| ((23 * f + 31 * t) % 200) as f32 / 100. - 1.)
                    .collect();
                let expected = normal.step(&mag, &phase);
                let mut tensors = BTreeMap::new();
                let actual = traced.step_with_trace(&mag, &phase, &mut |name, shape, values| {
                    assert_eq!(shape.iter().product::<usize>(), values.len(), "{name}");
                    assert!(values.iter().all(|v| v.is_finite()), "{name}");
                    assert!(
                        tensors.insert(name.to_string(), bits(values)).is_none(),
                        "duplicate {name}"
                    );
                });
                match (expected, actual) {
                    (None, None) => assert!(tensors.is_empty()),
                    (Some((m, p)), Some((tm, tp))) => {
                        assert_eq!(tensors.len(), 26);
                        assert_eq!(bits(&m), bits(&tm));
                        assert_eq!(bits(&p), bits(&tp));
                        assert_eq!(tensors["output.mag"], bits(&m));
                        assert_eq!(tensors["output.phase"], bits(&p));
                        assert_eq!(tensors["decoder.mag"][..161], bits(&m));
                        for layer in 0..3 {
                            let input: Vec<_> = tensors[&format!("blocks.{layer}.time_residual")]
                                .iter()
                                .map(|v| f32::from_bits(*v))
                                .collect();
                            let replay = traced.replay_frequency_block(layer, &input).unwrap();
                            assert_eq!(bits(&replay), tensors[&format!("blocks.{layer}.freq")]);
                            let mut inner = BTreeMap::new();
                            let inspected = traced
                                .replay_frequency_block_with_trace(
                                    layer,
                                    &input,
                                    &mut |name, shape, data| {
                                        assert_eq!(shape.iter().product::<usize>(), data.len());
                                        assert!(data.iter().all(|v| v.is_finite()));
                                        assert!(inner
                                            .insert(name.to_string(), bits(data))
                                            .is_none());
                                    },
                                )
                                .unwrap();
                            assert_eq!(inner.len(), 31);
                            assert_eq!(bits(&inspected), bits(&replay));
                            assert_eq!(inner["output"], bits(&replay));
                            if t == ahead {
                                let value = |name: &str| -> Vec<f32> {
                                    inner[name].iter().map(|v| f32::from_bits(*v)).collect()
                                };
                                for (op, input, expected) in [
                                    ("projection", "cat", "projection_replay"),
                                    ("norm", "projection_replay", "output"),
                                ] {
                                    let output = traced
                                        .replay_frequency_operator(layer, op, &value(input))
                                        .unwrap();
                                    assert_eq!(bits(&output), inner[expected]);
                                }
                                for direction in ["fw", "bw"] {
                                    let data = |name: &str| -> Vec<f32> {
                                        inner[&format!("{direction}.{name}")]
                                            .iter()
                                            .map(|v| f32::from_bits(*v))
                                            .collect()
                                    };
                                    let d = data("d");
                                    let x = data("xc");
                                    for (op, input, expected) in [
                                        ("x_proj", "xc", "dbl"),
                                        ("delta_logits", "dbl", "delta_logits_replay"),
                                        ("out_proj", "gate", "out"),
                                    ] {
                                        let output = traced
                                            .replay_frequency_operator(
                                                layer,
                                                &format!("{direction}.{op}"),
                                                &data(input),
                                            )
                                            .unwrap();
                                        assert_eq!(bits(&output), bits(&data(expected)));
                                    }
                                    let a = traced
                                        .replay_frequency_operator(
                                            layer,
                                            &format!("{direction}.a"),
                                            &[],
                                        )
                                        .unwrap();
                                    assert_eq!(bits(&a), bits(&data("a")));
                                    let raw = traced
                                        .replay_frequency_operator(
                                            layer,
                                            &format!("{direction}.delta_raw"),
                                            &data("dbl"),
                                        )
                                        .unwrap();
                                    let bias = data("delta_bias");
                                    let logits: Vec<_> = raw
                                        .iter()
                                        .enumerate()
                                        .map(|(i, v)| v + bias[i % bias.len()])
                                        .collect();
                                    assert_eq!(bits(&logits), bits(&data("delta_logits_replay")));
                                    let output = hush_core::trace::replay_scan(
                                        hush_core::trace::ScanInputs {
                                            rows: x.len() / d.len(),
                                            channels: d.len(),
                                            delta: &data("dt"),
                                            x: &x,
                                            a: &data("a"),
                                            b: &data("b"),
                                            c: &data("c"),
                                            d: &d,
                                        },
                                    )
                                    .unwrap();
                                    assert_eq!(
                                        bits(&output),
                                        inner[&format!("{direction}.scan_plus_d_replay")]
                                    );
                                }
                            }
                            let (nc, ns) = normal.temporal_state(layer).unwrap();
                            let (tc, ts) = traced.temporal_state(layer).unwrap();
                            assert_eq!(bits(nc), bits(tc));
                            assert_eq!(bits(ns), bits(ts));
                            assert_eq!(tensors[&format!("blocks.{layer}.conv_state")], bits(tc));
                            assert_eq!(tensors[&format!("blocks.{layer}.ssm_state")], bits(ts));
                        }
                    }
                    _ => panic!("trace changed lookahead readiness"),
                }
            }
        }
        assert!(traced.replay_frequency_block(3, &[0.; 64]).is_err());
        assert!(traced.replay_frequency_block(0, &[]).is_err());
        assert!(traced.replay_frequency_block(0, &[0.; 63]).is_err());
        assert!(traced.replay_frequency_block(0, &[f32::NAN; 64]).is_err());
        for (layer, name, input) in [
            (3, "norm", vec![0.; 64]),
            (0, "unknown", vec![0.; 64]),
            (0, "norm", vec![]),
            (0, "norm", vec![0.; 63]),
            (0, "projection", vec![f32::NAN; 128]),
            (0, "fw.x_proj", vec![0.; 255]),
            (0, "bw.delta_raw", vec![0.; 4]),
            (0, "fw.a", vec![0.]),
            (0, "norm_sliced", vec![0.; 64]),
            (0, "fw.x_proj_sliced", vec![0.; 256]),
        ] {
            assert!(traced
                .replay_frequency_operator(layer, name, &input)
                .is_err());
        }
    }
}
