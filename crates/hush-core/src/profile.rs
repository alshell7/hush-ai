//! Opt-in inclusive wall-time counters for development, not audio callbacks.
//! Nested categories overlap; parallel categories can sum to more than frame wall time.
use std::{
    collections::BTreeMap,
    sync::Mutex,
    time::{Duration, Instant},
};

static TIMES: Mutex<BTreeMap<&'static str, (u64, f64)>> = Mutex::new(BTreeMap::new());

pub(crate) struct Scope(&'static str, Instant);
pub(crate) fn scope(name: &'static str) -> Scope {
    Scope(name, Instant::now())
}
impl Drop for Scope {
    fn drop(&mut self) {
        record(self.0, 1, self.1.elapsed());
    }
}

/// Batch worker-local timing samples before taking the shared profiling lock. Totals from
/// parallel workers are summed wall intervals (including preemption), not CPU time or elapsed
/// time on the inference thread.
pub(crate) fn record(name: &'static str, calls: u64, elapsed: Duration) {
    let mut times = TIMES.lock().unwrap();
    let entry = times.entry(name).or_default();
    entry.0 += calls;
    entry.1 += elapsed.as_secs_f64();
}
/// Call only between completed inference calls, after warm-up.
pub fn reset() {
    TIMES.lock().unwrap().clear();
}
/// Print inclusive totals, normalized to measured frames, before flushing the stream.
pub fn report(frames: usize) {
    for (name, (calls, seconds)) in TIMES.lock().unwrap().iter() {
        eprintln!(
            "profile {name}: calls={calls} ms_per_frame={:.4}",
            seconds * 1000.0 / frames as f64
        );
    }
}
