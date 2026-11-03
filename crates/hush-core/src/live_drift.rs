//! Slow output-queue feedback for independent device clocks; no model-state changes.

pub(crate) const MAX_PPM: f64 = 1000.0;

pub(crate) struct DriftController {
    target_seconds: f64,
    filtered_error: f64,
    integral: f64,
    correction: f64,
}

impl DriftController {
    pub fn new(target_samples: f64, output_rate: u32) -> Self {
        Self {
            target_seconds: target_samples / output_rate as f64,
            filtered_error: 0.0,
            integral: 0.0,
            correction: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.filtered_error = 0.0;
        self.integral = 0.0;
        self.correction = 0.0;
    }

    /// Positive correction consumes source samples faster and produces fewer output samples.
    /// Call once per rendered block, after enqueueing, with its nominal duration.
    pub fn update(&mut self, queued: usize, output_rate: u32, seconds: f64) -> f64 {
        let error = queued as f64 / output_rate as f64 - self.target_seconds;
        // Smooth callback/worker scheduling jitter before a slow PI loop. Limit both the
        // integral and the slew; this is clock tracking, not a remedy for CPU overload.
        let alpha = seconds / (2.0 + seconds);
        self.filtered_error += alpha * (error - self.filtered_error);
        self.integral =
            (self.integral + 625.0 * self.filtered_error * seconds).clamp(-MAX_PPM, MAX_PPM);
        let desired = (50_000.0 * self.filtered_error + self.integral).clamp(-MAX_PPM, MAX_PPM);
        let limit = 100.0 * seconds;
        self.correction += (desired - self.correction).clamp(-limit, limit);
        self.correction
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_hour_long_clock_offsets_with_callback_jitter_and_a_rate_change() {
        for rate in [16000, 44100, 48000, 96000] {
            for offset in [-750.0, -100.0, 0.0, 100.0, 750.0] {
                let target = 0.050;
                let mut queue = target;
                let mut controller = DriftController::new(target * rate as f64, rate);
                let mut previous = 0.0;
                for tick in 0..180_000 {
                    let skew = if tick < 90_000 { offset } else { -offset };
                    // 20 ms producer frames; output callback phase contributes bounded jitter.
                    let jitter = 0.004 * (tick as f64 * 0.13).sin();
                    let observed = ((queue + jitter) * rate as f64).round() as usize;
                    let ppm = controller.update(observed, rate, 0.020);
                    assert!(ppm.abs() <= MAX_PPM);
                    assert!((ppm - previous).abs() <= 2.000_001);
                    queue += 0.020 * ((1.0 + skew * 1e-6) / (1.0 + ppm * 1e-6) - 1.0);
                    assert!(
                        (0.025..0.075).contains(&queue),
                        "rate={rate} skew={skew} tick={tick} queue={queue}"
                    );
                    previous = ppm;
                }
                assert!((queue - target).abs() < 0.001);
                assert!((previous + offset).abs() < 100.0);
            }
        }
    }

    #[test]
    fn bounds_sustained_overload_and_clears_feedback_on_reset() {
        let mut controller = DriftController::new(2400.0, 48000);
        for _ in 0..10_000 {
            assert!(controller.update(100_000, 48000, 0.02) <= MAX_PPM);
        }
        assert_eq!(controller.correction, MAX_PPM);
        controller.reset();
        assert_eq!(controller.update(2400, 48000, 0.02), 0.0);
        for _ in 0..10_000 {
            assert!(controller.update(0, 48000, 0.02) >= -MAX_PPM);
        }
        assert_eq!(controller.correction, -MAX_PPM);
    }
}
