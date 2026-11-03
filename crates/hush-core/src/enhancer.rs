//! One streaming interface for every model, so [`LiveWorker`](crate::live::LiveWorker) can run
//! either one.
use eyre::Result;

/// A streaming speech enhancer for mono audio: [`Denoiser`](crate::Denoiser) (NVIDIA RE-USE) or
/// [`DeepFilter`](crate::DeepFilter) (DeepFilterNet 3).
///
/// Output sample `i` is the enhanced input sample `i`, delayed by [`latency`](Self::latency)
/// samples. Inference allocates: call these methods on a worker or control thread, never in an
/// audio callback.
pub trait Enhancer: Send {
    /// Mono sample rate of the model.
    fn sample_rate(&self) -> u32;
    /// Samples consumed and produced per model step.
    fn frame(&self) -> usize;
    /// Delay between an input sample and its enhanced output, in samples. Waiting for a full
    /// [`frame`](Self::frame) adds one more frame.
    fn latency(&self) -> usize;
    /// Clears all stream state; the next sample starts a new stream.
    fn reset(&mut self);
    /// Initialize inference buffers on a control thread before audio starts, then return to a
    /// fresh stream.
    fn prepare(&mut self) -> Result<()>;
    /// Append every output sample that is complete to `out`.
    fn process_into(&mut self, input: &[f32], out: &mut Vec<f32>) -> Result<()>;
    /// Returns the samples still owed for the input so far, then resets.
    fn flush(&mut self) -> Result<Vec<f32>>;
    /// Scheduling policy for the thread that drives inference.
    fn worker_scheduling(&self) -> crate::WorkerScheduling;
}

impl Enhancer for crate::Denoiser {
    fn sample_rate(&self) -> u32 {
        crate::Denoiser::sample_rate(self)
    }
    fn frame(&self) -> usize {
        crate::Denoiser::frame(self)
    }
    fn latency(&self) -> usize {
        crate::Denoiser::latency(self)
    }
    fn reset(&mut self) {
        crate::Denoiser::reset(self)
    }
    fn prepare(&mut self) -> Result<()> {
        crate::Denoiser::prepare(self)
    }
    fn process_into(&mut self, input: &[f32], out: &mut Vec<f32>) -> Result<()> {
        crate::Denoiser::process_into(self, input, out)
    }
    fn flush(&mut self) -> Result<Vec<f32>> {
        crate::Denoiser::flush(self)
    }
    fn worker_scheduling(&self) -> crate::WorkerScheduling {
        crate::Denoiser::worker_scheduling(self)
    }
}

impl<E: Enhancer + ?Sized> Enhancer for Box<E> {
    fn sample_rate(&self) -> u32 {
        (**self).sample_rate()
    }
    fn frame(&self) -> usize {
        (**self).frame()
    }
    fn latency(&self) -> usize {
        (**self).latency()
    }
    fn reset(&mut self) {
        (**self).reset()
    }
    fn prepare(&mut self) -> Result<()> {
        (**self).prepare()
    }
    fn process_into(&mut self, input: &[f32], out: &mut Vec<f32>) -> Result<()> {
        (**self).process_into(input, out)
    }
    fn flush(&mut self) -> Result<Vec<f32>> {
        (**self).flush()
    }
    fn worker_scheduling(&self) -> crate::WorkerScheduling {
        (**self).worker_scheduling()
    }
}
