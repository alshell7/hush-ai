//! The voice model: NVIDIA Real-time RE-USE weights, downloaded once and checked by SHA-256.
use eyre::{bail, eyre, Result, WrapErr};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

/// SHA-256 of `model.safetensors` at NVIDIA revision 4c0c186.
pub const SHA256: &str = "6eac4e8cc721f99a22cde7eac3c9f8e2826ced5dc8a4234fd1977de1ec0d3ade";
pub const SIZE: u64 = 54_055_764;
const FILE: &str = "realtime-reuse-4c0c186.safetensors";
/// The Hush AI mirror first, then NVIDIA's repository at the pinned revision.
const SOURCES: [&str; 2] = [
    "https://huggingface.co/alshell7/hush-ai/resolve/main/models/realtime-reuse-4c0c186.safetensors",
    "https://huggingface.co/nvidia/Real-time_RE-USE/resolve/4c0c186272ca853cd18d832fc5c28777b62f32d1/model.safetensors",
];

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub installed: bool,
    pub path: Option<String>,
    pub size: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub received: u64,
    pub total: u64,
    pub bytes_per_second: f64,
    pub source: usize,
}

/// Where a downloaded model lives.
pub fn install_path(data_dir: &Path) -> PathBuf {
    data_dir.join("models").join(FILE)
}

/// A usable model: `HUSH_MODEL`, the downloaded file, or one next to the app (portable builds).
pub fn find(data_dir: &Path) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("HUSH_MODEL").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(path));
    }
    let mut candidates = vec![install_path(data_dir)];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("model.safetensors"));
            // Development: <repo>/target/<profile>/hush-ai(.exe) and <repo>/models.
            if let Some(repo) = dir.parent().and_then(Path::parent) {
                candidates.push(repo.join("models/model.safetensors"));
            }
        }
    }
    // The size check is enough at start-up: downloads are hashed before they are kept.
    candidates.into_iter().find(|path| {
        path.metadata()
            .is_ok_and(|m| m.is_file() && m.len() == SIZE)
    })
}

pub fn info(data_dir: &Path) -> ModelInfo {
    let path = find(data_dir);
    ModelInfo {
        installed: path.is_some(),
        path: path.map(|p| p.display().to_string()),
        size: SIZE,
    }
}

/// Download, hash and keep the model. Calls `progress` about ten times per second.
pub fn download(
    data_dir: &Path,
    cancel: &AtomicBool,
    progress: impl Fn(Progress),
) -> Result<PathBuf> {
    let target = install_path(data_dir);
    std::fs::create_dir_all(target.parent().unwrap())?;
    let mut errors = Vec::new();
    for (source, url) in SOURCES.iter().enumerate() {
        match fetch(url, &target, cancel, |received, total, rate| {
            progress(Progress {
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
        .unwrap_or(SIZE);
    if total != SIZE {
        bail!("Unexpected size: {total} bytes");
    }
    let mut reader = response.body_mut().with_config().limit(SIZE + 1).reader();
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
    if received != SIZE || hash != SHA256 {
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

pub fn remove(data_dir: &Path) -> Result<()> {
    let path = install_path(data_dir);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}
