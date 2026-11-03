//! Band-limited mono sample-rate conversion. Run on a worker, outside audio callbacks.

use eyre::{ensure, Result};

const TAPS: usize = 32;
const PHASES: usize = 1024;

/// Resample a mono recording using a 32-tap Hann-windowed sinc polyphase filter.
/// The output duration is rounded to the nearest sample. Equal rates are copied exactly.
pub fn mono(input: &[f32], from: u32, to: u32) -> Result<Vec<f32>> {
    ensure!(from > 0 && to > 0, "sample rates must be positive");
    if input.is_empty() || from == to {
        return Ok(input.to_vec());
    }
    let kernels = kernels(from, to);
    let count = ((input.len() as u64 * to as u64 + from as u64 / 2) / from as u64) as usize;
    let mut output = Vec::with_capacity(count);
    for index in 0..count {
        // Integer arithmetic avoids accumulating fractional position drift over long recordings.
        let pos = index as u64 * from as u64;
        let base = (pos / to as u64) as isize;
        let phase = ((pos % to as u64) * PHASES as u64 / to as u64) as usize;
        let mut sample = 0.0;
        for (tap, weight) in kernels[phase].iter().enumerate() {
            let at = (base + tap as isize - (TAPS / 2 - 1) as isize)
                .clamp(0, input.len() as isize - 1) as usize;
            sample += input[at] * weight;
        }
        output.push(sample);
    }
    Ok(output)
}

fn kernels(from: u32, to: u32) -> Vec<[f32; TAPS]> {
    let cutoff = 0.94 * (to as f64 / from as f64).min(1.0);
    let mut kernels = vec![[0f32; TAPS]; PHASES];
    for (phase, kernel) in kernels.iter_mut().enumerate() {
        let frac = phase as f64 / PHASES as f64;
        let mut sum = 0.0;
        for (tap, coeff) in kernel.iter_mut().enumerate() {
            let x = (tap as f64 - (TAPS / 2 - 1) as f64) - frac;
            let sinc = if x.abs() < 1e-12 {
                cutoff
            } else {
                (std::f64::consts::PI * cutoff * x).sin() / (std::f64::consts::PI * x)
            };
            let window = 0.5 + 0.5 * (std::f64::consts::PI * x / (TAPS / 2) as f64).cos();
            let weight = if x.abs() < (TAPS / 2) as f64 {
                sinc * window
            } else {
                0.0
            };
            *coeff = weight as f32;
            sum += weight;
        }
        for coeff in kernel {
            *coeff /= sum as f32;
        }
    }
    kernels
}

/// Causal counterpart of `mono`, with 16 input samples of filter delay when rates differ.
/// Rational phase and filter history persist across arbitrary device callback boundaries.
/// Kernel allocation is confined to construction; `push` and `reset` retain all storage.
pub struct StreamingMono {
    from: u64,
    to: u64,
    step: u64,
    adaptive: bool,
    position: u64,
    history: [f32; TAPS],
    head: usize,
    kernels: Vec<[f32; TAPS]>,
}

impl StreamingMono {
    pub fn new(from: u32, to: u32) -> Result<Self> {
        ensure!(from > 0 && to > 0, "sample rates must be positive");
        Ok(Self {
            from: from.into(),
            to: to.into(),
            step: from.into(),
            adaptive: false,
            position: 0,
            history: [0.0; TAPS],
            head: 0,
            kernels: if from == to {
                Vec::new()
            } else {
                kernels(from, to)
            },
        })
    }

    /// Variable output clock, including equal nominal rates. The filter is always active;
    /// a million phase units per rate unit allow sub-ppm updates without resetting history.
    pub fn adaptive(from: u32, to: u32) -> Result<Self> {
        let mut converter = Self::new(from, to)?;
        if converter.kernels.is_empty() {
            converter.kernels = kernels(from, to);
        }
        converter.from *= 1_000_000;
        converter.to *= 1_000_000;
        converter.step = converter.from;
        converter.adaptive = true;
        Ok(converter)
    }

    pub fn set_correction_ppm(&mut self, ppm: f64) {
        assert!(self.adaptive && ppm.is_finite());
        let ppm = ppm.clamp(-crate::live_drift::MAX_PPM, crate::live_drift::MAX_PPM);
        self.step = (self.from as f64 * (1.0 + ppm * 1e-6)).round() as u64;
    }

    pub fn reset(&mut self) {
        self.position = 0;
        self.step = self.from;
        self.history.fill(0.0);
        self.head = 0;
    }

    pub fn push(&mut self, sample: f32, mut emit: impl FnMut(f32)) {
        if self.from == self.to && !self.adaptive {
            emit(sample);
            return;
        }
        self.history[self.head] = sample;
        self.head = (self.head + 1) % TAPS;
        while self.position < self.to {
            let phase = (self.position * PHASES as u64 / self.to) as usize;
            let mut value = 0.0;
            for (tap, weight) in self.kernels[phase].iter().enumerate() {
                value += self.history[(self.head + tap) % TAPS] * weight;
            }
            emit(value);
            self.position += self.step;
        }
        self.position -= self.to;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adaptive_rate_changes_keep_a_tone_continuous() {
        let mut converter = StreamingMono::adaptive(16000, 48000).unwrap();
        let mut previous = 0.0f32;
        let mut samples = 0;
        let mut max_step = 0.0f32;
        for i in 0..16000 {
            if i % 320 == 0 {
                converter.set_correction_ppm(if i % 640 == 0 { 1000.0 } else { -1000.0 });
            }
            let value = (std::f32::consts::TAU * 1000.0 * i as f32 / 16000.0).sin() * 0.25;
            converter.push(value, |s| {
                if samples > 100 {
                    max_step = max_step.max((s - previous).abs());
                }
                previous = s;
                samples += 1;
            });
        }
        assert!(
            max_step < 0.035,
            "phase discontinuity: maximum step {max_step}"
        );
        assert!((samples as i64 - 48000).abs() <= 2);
    }

    #[test]
    fn adaptive_counts_follow_clock_correction_without_resetting_phase_or_history() {
        for (from, to) in [(16000, 48000), (48000, 48000), (44100, 48000)] {
            let mut converter = StreamingMono::adaptive(from, to).unwrap();
            let mut total = 0;
            let mut expected = 0.0;
            for ppm in [0.0, 750.0, -750.0, 1000.0, -1000.0, 0.0] {
                converter.set_correction_ppm(ppm);
                for i in 0..from {
                    converter.push(0.25, |s| {
                        total += 1;
                        if total > 100 && i > TAPS as u32 {
                            assert!((s - 0.25).abs() < 1e-5);
                        }
                    });
                }
                expected += to as f64 / (1.0 + ppm * 1e-6);
                assert!((total as f64 - expected).abs() < 2.0);
            }
            converter.reset();
            let mut actual = Vec::new();
            for _ in 0..from / 10 {
                converter.push(0.25, |s| actual.push(s));
            }
            let mut delayed = vec![0.0; TAPS / 2];
            delayed.extend(vec![0.25; from as usize / 10]);
            let expected = mono(&delayed, from, to).unwrap();
            // At equal rates the adaptive filter also runs; compare against its FIR directly.
            if from != to {
                assert_eq!(actual, expected[..actual.len()]);
            } else {
                assert_eq!(actual.len(), from as usize / 10);
                assert!(actual[100..].iter().all(|s| (*s - 0.25).abs() < 1e-5));
                assert!(actual[0].abs() < 0.01);
            }
        }
    }

    #[test]
    fn streaming_preserves_phase_dc_counts_and_reset_across_chunks() {
        for (from, to) in [
            (48000, 16000),
            (16000, 48000),
            (44100, 16000),
            (16000, 44100),
            (192000, 16000),
            (48000, 22050),
            (22050, 48000),
            (48000, 48000),
        ] {
            let input = vec![0.25; from as usize];
            let mut converter = StreamingMono::new(from, to).unwrap();
            let mut first = Vec::new();
            for chunk in input.chunks(137) {
                for &sample in chunk {
                    converter.push(sample, |s| first.push(s));
                }
            }
            assert_eq!(first.len(), to as usize);
            let settled = (TAPS as u64 * to as u64).div_ceil(from as u64) as usize;
            assert!(first[settled..].iter().all(|s| (*s - 0.25).abs() < 1e-5));
            // Leave a fractional converter between phases, with different tail history.
            converter.push(-0.9, |_| {});
            converter.reset();
            let mut second = Vec::new();
            for &sample in &input {
                converter.push(sample, |s| second.push(s));
            }
            assert_eq!(first, second);
        }
        assert!(StreamingMono::new(0, 48000).is_err());
        assert!(StreamingMono::new(48000, 0).is_err());
    }

    #[test]
    fn streaming_matches_delayed_offline_filter_and_rejects_aliases() {
        for (from, to) in [(48000, 16000), (16000, 48000), (44100, 22050)] {
            for hz in [1000.0, 12000.0] {
                let input: Vec<f32> = (0..from / 10)
                    .map(|n| (std::f32::consts::TAU * hz * n as f32 / from as f32).sin())
                    .collect();
                let mut converter = StreamingMono::new(from, to).unwrap();
                let mut actual = Vec::new();
                for &s in &input {
                    converter.push(s, |s| actual.push(s));
                }
                let mut delayed = vec![0.0; TAPS / 2];
                delayed.extend_from_slice(&input);
                let expected = mono(&delayed, from, to).unwrap();
                assert_eq!(actual, expected[..actual.len()]);
                let rms = (actual[100..].iter().map(|s| s * s).sum::<f32>()
                    / (actual.len() - 100) as f32)
                    .sqrt();
                if hz == 1000.0 {
                    assert!(rms > 0.65);
                }
                if from == 48000 && to == 16000 && hz == 12000.0 {
                    assert!(rms < 0.01);
                }
            }
        }
        let mut identity = StreamingMono::new(16000, 16000).unwrap();
        let input = [1.0, -0.4, 0.0, 0.8];
        let mut output = Vec::new();
        for s in input {
            identity.push(s, |s| output.push(s));
        }
        assert_eq!(output, input);
    }

    #[test]
    fn preserves_length_and_dc() {
        let out = mono(&vec![0.25; 4800], 48000, 16000).unwrap();
        assert_eq!(out.len(), 1600);
        assert!(out.iter().all(|x| (*x - 0.25).abs() < 1e-5));
        assert_eq!(mono(&[1.0, -0.5], 16000, 16000).unwrap(), [1.0, -0.5]);
        assert!(mono(&[0.0], 0, 16000).is_err());
    }

    #[test]
    fn suppresses_frequencies_above_new_nyquist() {
        let tone = |hz: f32| {
            (0..4800)
                .map(|n| (std::f32::consts::TAU * hz * n as f32 / 48000.0).sin())
                .collect::<Vec<_>>()
        };
        let rms = |x: &[f32]| {
            (x[50..x.len() - 50].iter().map(|v| v * v).sum::<f32>() / (x.len() - 100) as f32).sqrt()
        };
        let pass = mono(&tone(1000.0), 48000, 16000).unwrap();
        let reject = mono(&tone(12000.0), 48000, 16000).unwrap();
        assert!(rms(&pass) > 0.65);
        assert!(rms(&reject) < 0.01);
    }
}
