//! Studio: clean up a recording. A dropped file, or a new recording from the microphone, is
//! decoded, mixed to mono, converted to 48 kHz and run through the voice model on a worker
//! thread. The original and the enhanced version are written as WAV files for playback in the
//! window and for saving.
use crate::audio::audio_io::{self, devices, Selection};
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::SampleFormat;
use eyre::{eyre, Result, WrapErr};
use hush_core::resample::StreamingMono;
use rtrb::RingBuffer;
use serde::Serialize;
use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use symphonia::core::{
    audio::{SampleBuffer, SignalSpec},
    codecs::DecoderOptions,
    errors::Error as DecodeError,
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
};
use tauri::{AppHandle, Emitter, Manager};

/// Studio files are written at 48 kHz, the rate of both models' full-band settings.
const RATE: u32 = 48_000;
/// Waveform resolution sent to the window.
const PEAKS: usize = 480;
/// Longest microphone recording.
const MAX_RECORDING: Duration = Duration::from_secs(30 * 60);

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "phase", rename_all = "camelCase")]
pub enum StudioEvent {
    Recording {
        seconds: f32,
        level: f32,
    },
    Decoding {
        name: String,
    },
    Processing {
        name: String,
        progress: f32,
        eta: Option<f32>,
    },
    Ready(StudioResult),
    Cancelled,
    Error {
        message: String,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioResult {
    pub name: String,
    pub seconds: f32,
    /// WAV files in the app's cache folder, for playback.
    pub original: String,
    pub enhanced: String,
    pub original_peaks: Vec<f32>,
    pub enhanced_peaks: Vec<f32>,
    pub engine: String,
    /// Processing time in seconds.
    pub elapsed: f32,
}

#[derive(Default)]
pub struct Studio {
    next: AtomicUsize,
    /// The job that may report: older jobs stop quietly.
    current: AtomicUsize,
    cancel: Mutex<Option<Arc<AtomicBool>>>,
    recorder: Mutex<Option<Arc<AtomicBool>>>,
    result: Mutex<Option<StudioResult>>,
}

fn folder(app: &AppHandle) -> Result<PathBuf> {
    let dir = app.path().app_cache_dir()?.join("studio");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Remove the files of earlier jobs, except `keep`.
fn clean(dir: &Path, keep: &[&Path]) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !keep.iter().any(|k| *k == path) {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

impl Studio {
    /// Delete what earlier sessions left behind.
    pub fn startup(app: &AppHandle) {
        if let Ok(dir) = folder(app) {
            clean(&dir, &[]);
        }
    }

    fn emit(&self, app: &AppHandle, job: usize, event: StudioEvent) {
        if self.current.load(Ordering::Relaxed) == job {
            let _ = app.emit("studio", event);
        }
    }

    fn begin(&self) -> (usize, Arc<AtomicBool>) {
        self.cancel_job();
        let job = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        self.current.store(job, Ordering::Relaxed);
        let cancel = Arc::new(AtomicBool::new(false));
        *self.cancel.lock().unwrap() = Some(cancel.clone());
        (job, cancel)
    }

    fn cancel_job(&self) {
        if let Some(cancel) = self.cancel.lock().unwrap().take() {
            cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Stop the recording or the processing in progress.
    pub fn cancel(&self, app: &AppHandle) {
        if let Some(stop) = self.recorder.lock().unwrap().take() {
            stop.store(true, Ordering::Relaxed);
        }
        self.cancel_job();
        let _ = app.emit("studio", StudioEvent::Cancelled);
        self.current.fetch_add(1, Ordering::Relaxed);
    }

    /// Enhance a file in the background; progress arrives as "studio" events.
    pub fn open(&self, app: &AppHandle, path: PathBuf, name: Option<String>) {
        if let Some(stop) = self.recorder.lock().unwrap().take() {
            stop.store(true, Ordering::Relaxed);
        }
        let (job, cancel) = self.begin();
        let name = name.unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Recording".into())
        });
        let app = app.clone();
        thread::Builder::new()
            .name("hush-studio".into())
            .spawn(move || {
                let studio = app.state::<Studio>();
                let outcome = process(&app, job, &path, &name, &cancel);
                match outcome {
                    Ok(Some(result)) => {
                        *studio.result.lock().unwrap() = Some(result.clone());
                        studio.emit(&app, job, StudioEvent::Ready(result));
                    }
                    Ok(None) => {}
                    Err(error) => {
                        log::warn!("Studio: {error:#}");
                        studio.emit(
                            &app,
                            job,
                            StudioEvent::Error {
                                message: friendly(&error),
                            },
                        );
                    }
                }
            })
            .expect("studio thread");
    }

    /// Record from the chosen microphone until `stop_recording`, then enhance the recording.
    /// `input` is a microphone name; `None` follows the system default.
    pub fn record(&self, app: &AppHandle, input: Option<String>) -> Result<()> {
        self.cancel_job();
        let job = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        self.current.store(job, Ordering::Relaxed);
        let stop = Arc::new(AtomicBool::new(false));
        if let Some(previous) = self.recorder.lock().unwrap().replace(stop.clone()) {
            previous.store(true, Ordering::Relaxed);
        }
        let dir = folder(app)?;
        let path = dir.join(format!("recording-{job}.wav"));
        // cpal streams cannot move between threads: the recorder thread opens the microphone
        // and reports back whether it started.
        let (ready, started) = std::sync::mpsc::channel::<Result<()>>();
        let app = app.clone();
        thread::Builder::new()
            .name("hush-studio-recorder".into())
            .spawn(move || {
                let opened = (|| -> Result<_> {
                    let selection = match &input {
                        Some(name) => devices::discover(true)?
                            .named(name)
                            .unwrap_or(Selection::Default),
                        None => Selection::Default,
                    };
                    let device = devices::resolve(true, &selection)?;
                    let supported = device.default_input_config()?;
                    let format = supported.sample_format();
                    let config: cpal::StreamConfig = supported.into();
                    let rate = config.sample_rate.0;
                    let (producer, consumer) = RingBuffer::new(rate as usize * 2);
                    let dropped = Arc::new(AtomicUsize::new(0));
                    let failed = Arc::new(AtomicBool::new(false));
                    macro_rules! build {
                        ($t:ty) => {
                            audio_io::input_stream::<$t>(
                                &device,
                                &config,
                                producer,
                                dropped,
                                failed.clone(),
                            )?
                        };
                    }
                    let stream = match format {
                        SampleFormat::F32 => build!(f32),
                        SampleFormat::I16 => build!(i16),
                        SampleFormat::I32 => build!(i32),
                        SampleFormat::U16 => build!(u16),
                        SampleFormat::U8 => build!(u8),
                        other => return Err(eyre!("Unsupported microphone format: {other:?}")),
                    };
                    let writer = hound::WavWriter::create(&path, wav_spec(rate))?;
                    stream.play()?;
                    Ok((stream, consumer, writer, rate, failed))
                })();
                let (stream, mut consumer, mut writer, rate, failed) = match opened {
                    Ok(parts) => {
                        let _ = ready.send(Ok(()));
                        parts
                    }
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return;
                    }
                };
                let studio = app.state::<Studio>();
                let began = Instant::now();
                let mut written = 0u64;
                let mut peak = 0f32;
                let mut last = Instant::now();
                let mut result = Ok(());
                while !stop.load(Ordering::Relaxed)
                    && !failed.load(Ordering::Relaxed)
                    && began.elapsed() < MAX_RECORDING
                {
                    while let Ok(sample) = consumer.pop() {
                        peak = peak.max(sample.abs());
                        written += 1;
                        if let Err(error) = writer.write_sample(to_i16(sample)) {
                            result = Err(error);
                        }
                    }
                    if last.elapsed() >= Duration::from_millis(80) {
                        last = Instant::now();
                        studio.emit(
                            &app,
                            job,
                            StudioEvent::Recording {
                                seconds: written as f32 / rate as f32,
                                level: peak.min(1.0),
                            },
                        );
                        peak = 0.0;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                // Dropping the stream closes the microphone.
                drop(stream);
                while let Ok(sample) = consumer.pop() {
                    written += 1;
                    let _ = writer.write_sample(to_i16(sample));
                }
                let finished = writer.finalize();
                let canceled = studio.current.load(Ordering::Relaxed) != job;
                if canceled {
                    let _ = std::fs::remove_file(&path);
                    return;
                }
                let failure = if failed.load(Ordering::Relaxed) && written == 0 {
                    Some("The microphone stopped. Check that it is connected and try again.".into())
                } else if written < rate as u64 / 2 {
                    Some("The recording is too short. Record at least a second.".to_string())
                } else if let Err(error) = result.and(finished) {
                    Some(format!("Could not save the recording: {error}"))
                } else {
                    None
                };
                if let Some(message) = failure {
                    let _ = std::fs::remove_file(&path);
                    studio.emit(&app, job, StudioEvent::Error { message });
                    return;
                }
                studio.recorder.lock().unwrap().take();
                let name = format!("Recording {}", chrono::Local::now().format("%-d %b, %H.%M"));
                studio.open(&app, path, Some(name));
            })?;
        started
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| eyre!("The microphone did not start"))?
    }

    /// Finish the recording and start enhancing it.
    pub fn stop_recording(&self) {
        if let Some(stop) = self.recorder.lock().unwrap().as_ref() {
            stop.store(true, Ordering::Relaxed);
        }
    }

    /// Copy the enhanced file of the last result to `target`.
    pub fn save(&self, target: &Path) -> Result<()> {
        let result = self.result.lock().unwrap().clone();
        let result = result.ok_or_else(|| eyre!("Enhance a recording first"))?;
        std::fs::copy(&result.enhanced, target)
            .wrap_err_with(|| format!("Could not save {}", target.display()))?;
        Ok(())
    }
}

fn friendly(error: &eyre::Report) -> String {
    let text = format!("{error:#}");
    let lower = text.to_lowercase();
    if lower.contains("unsupported") || lower.contains("no suitable") || lower.contains("probe") {
        "Hush cannot read this file. Try WAV, MP3, M4A, FLAC or OGG.".into()
    } else if lower.contains("safetensors") || lower.contains("voice model") {
        "Download the voice model first.".into()
    } else {
        text.lines().next().unwrap_or(&text).to_string()
    }
}

fn wav_spec(rate: u32) -> hound::WavSpec {
    hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    }
}

fn to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16
}

/// Peak level per 10 ms block, reduced to `PEAKS` bins at the end.
#[derive(Default)]
struct Envelope {
    blocks: Vec<f32>,
    peak: f32,
    count: usize,
}

impl Envelope {
    const BLOCK: usize = RATE as usize / 100;

    fn push(&mut self, samples: &[f32]) {
        for sample in samples {
            self.peak = self.peak.max(sample.abs());
            self.count += 1;
            if self.count == Self::BLOCK {
                self.blocks.push(self.peak);
                self.peak = 0.0;
                self.count = 0;
            }
        }
    }

    fn finish(mut self) -> Vec<f32> {
        if self.count > 0 {
            self.blocks.push(self.peak);
        }
        if self.blocks.is_empty() {
            return vec![0.0; PEAKS];
        }
        let size = self.blocks.len().div_ceil(PEAKS).max(1);
        self.blocks
            .chunks(size)
            .map(|chunk| chunk.iter().fold(0f32, |m, v| m.max(*v)).min(1.0))
            .collect()
    }
}

/// An open audio file, decoded block by block and mixed to mono.
struct Source {
    format: Box<dyn symphonia::core::formats::FormatReader>,
    decoder: Box<dyn symphonia::core::codecs::Decoder>,
    track: u32,
    rate: u32,
    /// Frames in the file, when the container says.
    frames: Option<u64>,
    /// Reused between packets: the frames and layout it holds, and the buffer.
    buffer: Option<(u64, SignalSpec, SampleBuffer<f32>)>,
}

impl Source {
    fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).wrap_err_with(|| format!("Cannot open {}", path.display()))?;
        let stream = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(extension);
        }
        let probed = symphonia::default::get_probe()
            .format(
                &hint,
                stream,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .map_err(|e| eyre!("Unsupported file: {e}"))?;
        let format = probed.format;
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
            .ok_or_else(|| eyre!("Unsupported file: no audio track"))?;
        let rate = track
            .codec_params
            .sample_rate
            .ok_or_else(|| eyre!("Unsupported file: unknown sample rate"))?;
        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
            .map_err(|e| eyre!("Unsupported file: {e}"))?;
        Ok(Self {
            track: track.id,
            frames: track.codec_params.n_frames,
            rate,
            format,
            decoder,
            buffer: None,
        })
    }

    /// The next block of mono samples, or `false` at the end of the file.
    fn next(&mut self, out: &mut Vec<f32>) -> Result<bool> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(packet) => packet,
                Err(DecodeError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Ok(false)
                }
                Err(DecodeError::ResetRequired) => return Ok(false),
                Err(error) => return Err(eyre!("Cannot read the file: {error}")),
            };
            if packet.track_id() != self.track {
                continue;
            }
            let decoded = match self.decoder.decode(&packet) {
                Ok(decoded) => decoded,
                // A damaged frame: skip it, as players do.
                Err(DecodeError::DecodeError(_)) => continue,
                Err(error) => return Err(eyre!("Cannot decode the file: {error}")),
            };
            let spec = *decoded.spec();
            let channels = spec.channels.count().max(1);
            let frames = decoded.capacity() as u64;
            let fits = matches!(&self.buffer, Some((held, held_spec, _)) if *held >= frames && *held_spec == spec);
            if !fits {
                self.buffer = Some((frames, spec, SampleBuffer::new(frames, spec)));
            }
            let (_, _, buffer) = self.buffer.as_mut().expect("buffer");
            buffer.copy_interleaved_ref(decoded);
            out.extend(
                buffer
                    .samples()
                    .chunks_exact(channels)
                    .map(|frame| frame.iter().sum::<f32>() / channels as f32),
            );
            return Ok(true);
        }
    }
}

type Writer = hound::WavWriter<std::io::BufWriter<File>>;

fn write(samples: &[f32], writer: &mut Writer, envelope: &mut Envelope) -> Result<()> {
    envelope.push(samples);
    for sample in samples {
        writer.write_sample(to_i16(*sample))?;
    }
    Ok(())
}

/// Decode, convert and enhance `path`. `None` when canceled.
fn process(
    app: &AppHandle,
    job: usize,
    path: &Path,
    name: &str,
    cancel: &AtomicBool,
) -> Result<Option<StudioResult>> {
    let studio = app.state::<Studio>();
    let started = Instant::now();
    studio.emit(
        app,
        job,
        StudioEvent::Decoding {
            name: name.to_string(),
        },
    );
    let dir = folder(app)?;
    let original_path = dir.join(format!("original-{job}.wav"));
    let enhanced_path = dir.join(format!("enhanced-{job}.wav"));
    clean(&dir, &[path, &original_path, &enhanced_path]);

    let mut source = Source::open(path)?;
    let mut model = crate::engine::offline_model(app, RATE)?;
    let total = source.frames.map(|f| f as f64);
    let mut resampler = StreamingMono::new(source.rate, RATE)?;
    let mut original = hound::WavWriter::create(&original_path, wav_spec(RATE))?;
    let mut enhanced = hound::WavWriter::create(&enhanced_path, wav_spec(RATE))?;
    let (mut original_envelope, mut enhanced_envelope) = (Envelope::default(), Envelope::default());
    let (mut decoded, mut converted, mut output) = (Vec::new(), Vec::new(), Vec::new());
    let (mut read, mut written) = (0u64, 0u64);
    let mut last = Instant::now();

    while source.next(&mut decoded)? {
        if cancel.load(Ordering::Relaxed) {
            drop((original, enhanced));
            clean(&dir, &[path]);
            return Ok(None);
        }
        read += decoded.len() as u64;
        for sample in decoded.drain(..) {
            resampler.push(sample, |value| converted.push(value));
        }
        write(&converted, &mut original, &mut original_envelope)?;
        model.process_into(&converted, &mut output)?;
        converted.clear();
        written += output.len() as u64;
        write(&output, &mut enhanced, &mut enhanced_envelope)?;
        output.clear();
        if last.elapsed() >= Duration::from_millis(120) {
            last = Instant::now();
            let progress = total.map_or(0.0, |t| (read as f64 / t).min(1.0)) as f32;
            let elapsed = started.elapsed().as_secs_f32();
            let eta = (progress > 0.02).then(|| elapsed / progress * (1.0 - progress));
            studio.emit(
                app,
                job,
                StudioEvent::Processing {
                    name: name.to_string(),
                    progress,
                    eta,
                },
            );
        }
    }
    let tail = model.flush()?;
    written += tail.len() as u64;
    write(&tail, &mut enhanced, &mut enhanced_envelope)?;
    original.finalize()?;
    enhanced.finalize()?;
    if written == 0 {
        return Err(eyre!("The file has no audio"));
    }
    Ok(Some(StudioResult {
        name: name.to_string(),
        seconds: written as f32 / RATE as f32,
        original: original_path.display().to_string(),
        enhanced: enhanced_path.display().to_string(),
        original_peaks: original_envelope.finish(),
        enhanced_peaks: enhanced_envelope.finish(),
        engine: model.label().into(),
        elapsed: started.elapsed().as_secs_f32(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_has_fixed_resolution() {
        let mut envelope = Envelope::default();
        envelope.push(&vec![0.5; RATE as usize * 7]);
        let peaks = envelope.finish();
        assert!(peaks.len() <= PEAKS && peaks.len() > PEAKS / 2);
        assert!(peaks.iter().all(|p| (*p - 0.5).abs() < 1e-6));
    }

    #[test]
    fn decodes_and_mixes_wav_to_mono() {
        let dir = std::env::temp_dir().join(format!("hush-studio-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("stereo.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 44_100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        for _ in 0..44_100 {
            writer.write_sample(i16::MAX / 2).unwrap();
            writer.write_sample(0i16).unwrap();
        }
        writer.finalize().unwrap();
        let mut source = Source::open(&path).unwrap();
        assert_eq!(source.rate, 44_100);
        let mut samples = Vec::new();
        while source.next(&mut samples).unwrap() {}
        assert_eq!(samples.len(), 44_100);
        assert!(samples.iter().all(|s| (*s - 0.25).abs() < 1e-3));
        let _ = std::fs::remove_dir_all(dir);
    }
}
