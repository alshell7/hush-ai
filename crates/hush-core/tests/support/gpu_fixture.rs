//! Reader/comparator shared by the fused-reference tests and measurement example.
use eyre::{ensure, Result};
use hush_core::Streaming;

#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    pub min_snr: f64,
    pub max_error: f64,
}

impl Metrics {
    fn new() -> Self {
        Self {
            min_snr: f64::INFINITY,
            max_error: 0.0,
        }
    }

    fn update(&mut self, expected: &[f32], actual: &[f32], phase: bool) -> Result<()> {
        ensure!(expected.len() == actual.len(), "fixture shape mismatch");
        let mut power = 0.0;
        let mut error = 0.0;
        for (&a, &b) in expected.iter().zip(actual) {
            ensure!(a.is_finite() && b.is_finite(), "non-finite trace value");
            let delta = b as f64 - a as f64;
            let delta = if phase {
                delta.sin().atan2(delta.cos())
            } else {
                delta
            };
            power += (a as f64).powi(2);
            error += delta * delta;
            self.max_error = self.max_error.max(delta.abs());
        }
        let snr = if error == 0.0 {
            f64::INFINITY
        } else if power == 0.0 {
            f64::NEG_INFINITY
        } else {
            10.0 * (power / error).log10()
        };
        self.min_snr = self.min_snr.min(snr);
        Ok(())
    }
}

struct Reader<'a>(&'a [u8]);
impl Reader<'_> {
    fn word(&mut self) -> Result<[u8; 4]> {
        ensure!(self.0.len() >= 4, "truncated GPU fixture");
        let (word, rest) = self.0.split_at(4);
        self.0 = rest;
        Ok(word.try_into()?)
    }

    fn int(&mut self) -> Result<usize> {
        Ok(u32::from_le_bytes(self.word()?) as usize)
    }

    fn floats(&mut self, count: usize) -> Result<Vec<f32>> {
        ensure!(count <= self.0.len() / 4, "truncated GPU fixture array");
        (0..count)
            .map(|_| Ok(f32::from_le_bytes(self.word()?)))
            .collect()
    }
}

/// Replay model inputs and resets, compare every emitted spectrum and each layer's sampled caches.
pub fn compare(path: &str, bytes: &[u8]) -> Result<[Metrics; 4]> {
    ensure!(bytes.starts_with(b"HUSHGPU1"), "unknown GPU fixture format");
    let mut reader = Reader(&bytes[8..]);
    let bins = reader.int()?;
    let layers = reader.int()?;
    let ahead = reader.int()?;
    let frames = reader.int()?;
    let reset = reader.int()?;
    ensure!(bins > 0 && frames > 0, "empty GPU fixture");
    let mut model = Streaming::load(path, layers, ahead)?;
    let mut metrics = [Metrics::new(); 4];
    let mut outputs = 0;
    for frame in 0..frames {
        if frame == reset {
            model.reset();
        }
        let mag = reader.floats(bins)?;
        let phase = reader.floats(bins)?;
        let present = reader.int()?;
        ensure!(present <= 1, "invalid output-present flag");
        let output = model.step(&mag, &phase);
        ensure!(
            output.is_some() == (present == 1),
            "frame {frame}: look-ahead readiness mismatch"
        );
        if let Some((mag, phase)) = output {
            outputs += 1;
            metrics[0].update(&reader.floats(bins)?, &mag, false)?;
            metrics[1].update(&reader.floats(bins)?, &phase, true)?;
            for layer in 0..layers {
                let (conv, ssm) = model.temporal_state(layer).unwrap();
                for (index, state) in [(2, conv), (3, ssm)] {
                    let count = reader.int()?;
                    let expected = reader.floats(count)?;
                    let actual: Vec<_> = state.iter().step_by(257).copied().collect();
                    metrics[index].update(&expected, &actual, false)?;
                }
            }
        }
    }
    ensure!(
        outputs > 0 && reader.0.is_empty(),
        "incomplete fixture replay"
    );
    Ok(metrics)
}
