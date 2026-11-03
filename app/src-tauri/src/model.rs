//! The voice models: DeepFilterNet 3 and NVIDIA Real-time RE-USE, each downloaded once and
//! checked by SHA-256.
use crate::settings::Engine;
use eyre::{bail, eyre, Result, WrapErr};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

/// One downloadable model file.
pub struct Spec {
    pub sha256: &'static str,
    pub size: u64,
    /// File name in the app's model folder.
    file: &'static str,
    /// File name next to the app and in `<repo>/models` (development).
    local: &'static str,
    /// Environment variable that overrides the path.
    env: &'static str,
    /// Tried in order.
    sources: &'static [&'static str],
}

/// `DeepFilterNet3_onnx.tar.gz` from the DeepFilterNet repository at tag v0.5.6 (MIT or
/// Apache-2.0, like the rest of that repository): the Hush AI mirror first, then upstream.
pub const DEEP_FILTER: Spec = Spec {
    sha256: "c94d91f70911001c946e0fabb4aa9adc37045f45a03b56008cb0c8244cb63616",
    size: 7_983_136,
    file: "deepfilternet3-v0.5.6.tar.gz",
    local: "DeepFilterNet3_onnx.tar.gz",
    env: "HUSH_DFN_MODEL",
    sources: &[
        "https://huggingface.co/alshell7/hush-ai/resolve/main/models/DeepFilterNet3_onnx-v0.5.6.tar.gz",
        "https://raw.githubusercontent.com/Rikorose/DeepFilterNet/v0.5.6/models/DeepFilterNet3_onnx.tar.gz",
        "https://cdn.jsdelivr.net/gh/Rikorose/DeepFilterNet@v0.5.6/models/DeepFilterNet3_onnx.tar.gz",
    ],
};

/// `model.safetensors` at NVIDIA revision 4c0c186: the Hush AI mirror first, then NVIDIA's
/// repository at the pinned revision.
pub const REUSE: Spec = Spec {
    sha256: "6eac4e8cc721f99a22cde7eac3c9f8e2826ced5dc8a4234fd1977de1ec0d3ade",
    size: 54_055_764,
    file: "realtime-reuse-4c0c186.safetensors",
    local: "model.safetensors",
    env: "HUSH_MODEL",
    sources: &[
        "https://huggingface.co/alshell7/hush-ai/resolve/main/models/realtime-reuse-4c0c186.safetensors",
        "https://huggingface.co/nvidia/Real-time_RE-USE/resolve/4c0c186272ca853cd18d832fc5c28777b62f32d1/model.safetensors",
    ],
};

pub fn spec(engine: Engine) -> &'static Spec {
    match engine {
        Engine::DeepFilter => &DEEP_FILTER,
        Engine::Reuse => &REUSE,
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    /// False when this build cannot run the engine (DeepFilterNet 3 on Windows on ARM).
    pub available: bool,
    pub installed: bool,
    pub path: Option<String>,
    pub size: u64,
}

/// Both models, for the frontend.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Models {
    pub deep_filter: ModelInfo,
    pub reuse: ModelInfo,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub engine: Engine,
    pub received: u64,
    pub total: u64,
    pub bytes_per_second: f64,
    pub source: usize,
}

/// Where a downloaded model lives.
pub fn install_path(data_dir: &Path, engine: Engine) -> PathBuf {
    data_dir.join("models").join(spec(engine).file)
}

/// A usable model: its environment variable, the downloaded file, or one next to the app
/// (portable builds).
pub fn find(data_dir: &Path, engine: Engine) -> Option<PathBuf> {
    let spec = spec(engine);
    if let Some(path) = std::env::var_os(spec.env).filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(path));
    }
    let mut candidates = vec![install_path(data_dir, engine)];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(spec.local));
            // Development: <repo>/target/<profile>/hush-ai(.exe) and <repo>/models.
            if let Some(repo) = dir.parent().and_then(Path::parent) {
                candidates.push(repo.join("models").join(spec.local));
            }
        }
    }
    // The size check is enough at start-up: downloads are hashed before they are kept.
    candidates.into_iter().find(|path| {
        path.metadata()
            .is_ok_and(|m| m.is_file() && m.len() == spec.size)
    })
}

pub fn info(data_dir: &Path, engine: Engine) -> ModelInfo {
    let path = find(data_dir, engine);
    ModelInfo {
        available: engine != Engine::DeepFilter || hush_core::DEEP_FILTER_AVAILABLE,
        installed: path.is_some(),
        path: path.map(|p| p.display().to_string()),
        size: spec(engine).size,
    }
}

pub fn all(data_dir: &Path) -> Models {
    Models {
        deep_filter: info(data_dir, Engine::DeepFilter),
        reuse: info(data_dir, Engine::Reuse),
    }
}

/// Download, hash and keep a model. Calls `progress` about ten times per second.
pub fn download(
    data_dir: &Path,
    engine: Engine,
    cancel: &AtomicBool,
    progress: impl Fn(Progress),
) -> Result<PathBuf> {
    let spec = spec(engine);
    let target = install_path(data_dir, engine);
    std::fs::create_dir_all(target.parent().unwrap())?;
    let mut errors = Vec::new();
    for (source, url) in spec.sources.iter().enumerate() {
        match fetch(url, spec, &target, cancel, |received, total, rate| {
            progress(Progress {
                engine,
                received,
                total,
                bytes_per_second: rate,
                source,
            })
        }) {
            Ok(()) => return Ok(target),
            Err(error) if cancel.load(Ordering::Relaxed) => return Err(error),
            Err(error) => errors.push(format!("{url}: {error:#}")),
        }
    }
    Err(eyre!(
        "Could not download the voice model.\n{}",
        errors.join("\n")
    ))
}

fn fetch(
    url: &str,
    spec: &Spec,
    target: &Path,
    cancel: &AtomicBool,
    progress: impl Fn(u64, u64, f64),
) -> Result<()> {
    let partial = target.with_extension("part");
    let mut response = ureq::get(url)
        .header("User-Agent", concat!("HushAI/", env!("CARGO_PKG_VERSION")))
        .call()
        .wrap_err("Request failed")?;
    let total = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or(spec.size);
    if total != spec.size {
        bail!("Unexpected size: {total} bytes");
    }
    let mut reader = response
        .body_mut()
        .with_config()
        .limit(spec.size + 1)
        .reader();
    let mut file = std::fs::File::create(&partial)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 256 * 1024];
    let (mut received, start) = (0u64, Instant::now());
    let mut last = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) {
            drop(file);
            let _ = std::fs::remove_file(&partial);
            bail!("Download canceled");
        }
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        file.write_all(&buffer[..count])?;
        received += count as u64;
        if last.elapsed().as_millis() >= 100 {
            last = Instant::now();
            progress(
                received,
                total,
                received as f64 / start.elapsed().as_secs_f64().max(0.001),
            );
        }
    }
    file.sync_all()?;
    drop(file);
    let hash: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if received != spec.size || hash != spec.sha256 {
        let _ = std::fs::remove_file(&partial);
        bail!("Checksum mismatch ({received} bytes, SHA-256 {hash})");
    }
    progress(
        received,
        total,
        received as f64 / start.elapsed().as_secs_f64().max(0.001),
    );
    std::fs::rename(&partial, target).map_err(|e| eyre!("Cannot save the model: {e}"))
}

pub fn remove(data_dir: &Path, engine: Engine) -> Result<()> {
    let path = install_path(data_dir, engine);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}
