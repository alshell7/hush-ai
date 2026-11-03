//! Enhance many mono wavs with Real-time RE-USE or DeepFilterNet 3, loading the model once.
//!
//! cargo run --release --example enhance_dir -- <in> <out_dir> [layers] [look_ahead] [model.safetensors]
//!
//! A model that ends in `.tar.gz` is a DeepFilterNet archive: the wavs must be 48 kHz, and
//! `layers` and `look_ahead` are ignored.
//!
//! `<in>` is a folder of wavs, or a text file with one wav path per line. Every wav must have
//! the sample rate of the first one. Each file is a fresh stream, fed in the same small chunks
//! as `enhance`, and is written to `<out_dir>` with the same name. Output sample `i` matches
//! input sample `i`. For each file, the example prints a tab-separated line:
//! `file  audio_seconds  wall_seconds  cpu_seconds` (CPU time of the whole process).

use hush_core::{audio, Config, DeepFilter, Denoiser, Enhancer};
use std::path::Path;
use std::time::Instant;

/// Samples handed to the denoiser per call, like one buffer from an audio device.
const CHUNK: usize = 1000;

fn main() -> eyre::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    eyre::ensure!(
        args.len() >= 3,
        "usage: enhance_dir <in_dir|list.txt> <out_dir> [layers] [look_ahead] [model.safetensors]"
    );
    let files = wav_list(Path::new(&args[1]))?;
    let out_dir = Path::new(&args[2]);
    let layers: usize = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(8);
    let look_ahead: usize = args.get(4).and_then(|v| v.parse().ok()).unwrap_or(0);
    let model = args
        .get(5)
        .map(String::as_str)
        .unwrap_or("models/model.safetensors");
    eyre::ensure!(!files.is_empty(), "no wav files in {}", args[1]);
    std::fs::create_dir_all(out_dir)?;

    let (_, sample_rate) = audio::load_wav(&files[0])?;
    let mut denoiser: Box<dyn Enhancer> = if model.ends_with(".tar.gz") {
        let model = DeepFilter::load(model)?;
        eyre::ensure!(
            sample_rate == model.sample_rate(),
            "DeepFilterNet needs 48 kHz wavs, not {sample_rate} Hz"
        );
        eprintln!(
            "{} files, DeepFilterNet, {sample_rate} Hz, 1 thread",
            files.len()
        );
        Box::new(model)
    } else {
        let config = Config {
            layers,
            sample_rate,
            look_ahead,
        };
        let model = Denoiser::with_config(model, config)?;
        eprintln!(
            "{} files, {layers} layers, look-ahead {look_ahead}, {sample_rate} Hz, {} worker threads",
            files.len(),
            model.worker_threads()
        );
        Box::new(model)
    };

    for path in &files {
        let (wav, rate) = audio::load_wav(path)?;
        eyre::ensure!(
            rate == sample_rate,
            "{path}: {rate} Hz, expected {sample_rate} Hz"
        );
        let wall = Instant::now();
        let cpu = cpu_time::ProcessTime::now();
        let mut enhanced = Vec::with_capacity(wav.len());
        for chunk in wav.chunks(CHUNK) {
            denoiser.process_into(chunk, &mut enhanced)?;
        }
        // Flush also resets the stream, so the next file starts fresh.
        enhanced.extend(denoiser.flush()?);
        let (wall, cpu) = (wall.elapsed().as_secs_f64(), cpu.elapsed().as_secs_f64());

        let name = Path::new(path)
            .file_name()
            .ok_or_else(|| eyre::eyre!("bad path {path}"))?
            .to_string_lossy();
        let out = out_dir.join(&*name);
        audio::save_wav(&out.to_string_lossy(), &enhanced, sample_rate)?;
        let seconds = wav.len() as f64 / f64::from(sample_rate);
        println!("{name}\t{seconds:.4}\t{wall:.4}\t{cpu:.4}");
    }
    Ok(())
}

/// Wav files in a folder (sorted by name), or the paths listed in a text file.
fn wav_list(input: &Path) -> eyre::Result<Vec<String>> {
    if input.is_dir() {
        let mut files: Vec<String> = std::fs::read_dir(input)?
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("wav")))
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        files.sort();
        Ok(files)
    } else {
        Ok(std::fs::read_to_string(input)?
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(String::from)
            .collect())
    }
}
