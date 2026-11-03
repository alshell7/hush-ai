# Build and release

[Home](../README.md) · [Architecture](ARCHITECTURE.md) · [Contributing](../CONTRIBUTING.md)

## Repository

| Path | Contents |
| --- | --- |
| `crates/hush-core` | Engine library: model, live worker, virtual microphone client, tests, benchmarks |
| `app` | Desktop app: React frontend (`src`), Tauri backend (`src-tauri`) |
| `virtual_mic` | Hush AI Microphone drivers: macOS plug-in, Windows SysVAD driver, shared reader tests |
| `site` | Website (GitHub Pages) |
| `docs` | Documentation |
| `assets` | Logo, icons, screenshots |

## Prerequisites

- [Rust](https://rustup.rs) (stable)
- [Node.js](https://nodejs.org) 20 or newer
- [Tauri prerequisites](https://tauri.app/start/prerequisites/): WebView2 (Windows), Xcode
  command line tools (macOS), `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev
  libasound2-dev patchelf` (Debian/Ubuntu)

## Run the app

```sh
cd app
npm install
npm run tauri dev
```

The app downloads the chosen model on first start. Or put the models in `models/` at the
repository root (a development build finds them there):

```sh
mkdir -p models
curl -fL -o models/DeepFilterNet3_onnx.tar.gz https://huggingface.co/alshell7/hush-ai/resolve/main/models/DeepFilterNet3_onnx-v0.5.6.tar.gz
curl -fL -o models/model.safetensors https://huggingface.co/alshell7/hush-ai/resolve/main/models/realtime-reuse-4c0c186.safetensors
```

| File | Model | SHA-256 |
| --- | --- | --- |
| `DeepFilterNet3_onnx.tar.gz` | DeepFilterNet 3 (tag v0.5.6) | `c94d91f70911001c946e0fabb4aa9adc37045f45a03b56008cb0c8244cb63616` |
| `model.safetensors` | NVIDIA Real-time RE-USE (revision 4c0c186) | `6eac4e8cc721f99a22cde7eac3c9f8e2826ced5dc8a4234fd1977de1ec0d3ade` |

`HUSH_DFN_MODEL` and `HUSH_MODEL` override the two paths.

**Design without the backend.** `npm run dev` and open http://localhost:1420. In a normal
browser the frontend uses a simulated backend (`app/src/lib/ipc.ts`). Query options:
`?platform=macos|windows|linux`, `?onboarded=0` (setup), `?vmic=install|download` (setup
states), `?models=deepFilter,reuse` (downloaded models), `#widget` (tray widget).

**End-to-end check.** With `HUSH_SELFTEST=1`, the app drives the real backend through every
command for each model (devices, download if missing, start, meters, quality change for
RE-USE, stop, test recording) and prints `[selftest] PASS` or `FAIL`, then quits:

```sh
cargo build -p hush-ai
HUSH_SELFTEST=1 target/debug/hush-ai
```

On macOS, start `npm run dev` first (debug builds load the frontend from port 1420).

## Tests

```sh
export HUSH_MODEL=$PWD/models/model.safetensors   # absolute paths
export HUSH_DFN_MODEL=$PWD/models/DeepFilterNet3_onnx.tar.gz
cargo fmt --all --check
cargo clippy --workspace --all-targets --release -- -D warnings
cargo test -p hush-core --release -- --test-threads=1
cargo test -p hush-ai
(cd app && npm run typecheck)
```

`hush-core` tests include NVIDIA's seven GPU regression cases ([Accuracy](ACCURACY.md)).
Never loosen a gate to make a change pass. The DeepFilterNet 3 tests check that Hush's
output is bit-identical to libDF called directly with the `deep-filter` settings.

Virtual microphone reader tests:

```sh
cc -std=c11 -O2 -Wall -Wextra -Werror -o test_ring virtual_mic/test_ring.c -lm && ./test_ring
```

## CI

| Workflow | When | What |
| --- | --- | --- |
| `ci.yml` | Push and pull request | Engine (fmt, clippy, tests) on Linux, macOS, Windows; app build on all three; driver reader tests |
| `release.yml` | Tag `v*` | Installers for Windows x64/ARM64, macOS Apple silicon/Intel, Linux; signed update files; GitHub release |
| `pages.yml` | Push to `main` that changes `site/` or the app frontend | Website plus the live app demo |

Caches: Cargo (`Swatinem/rust-cache`, one key per job type), npm (`setup-node`), both models
(`actions/cache`, keyed by their hashes). Runs on the same branch cancel older ones.

## Release

1. Update `version` in `Cargo.toml` (workspace) and `app/package.json`.
2. Commit, then tag and push:

   ```sh
   git tag v1.0.1
   git push origin v1.0.1
   ```

3. `release.yml` builds every installer and publishes the release with `latest.json`.
   Installed apps find it within six hours, download it quietly and offer **Restart to update**.

### Update signing key

Updates are signed with a minisign key. The public key is in
`app/src-tauri/tauri.conf.json` (`plugins.updater.pubkey`). The private key never goes in
the repository: it is in the GitHub secrets `TAURI_SIGNING_PRIVATE_KEY` and
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. If the key is lost, installed apps cannot update to
builds signed with a new key; users must install the new version by hand.

### Code signing (not yet)

- **Windows:** an Authenticode certificate removes the SmartScreen prompt. The Hush driver
  needs an EV certificate and Microsoft attestation signing ([virtual_mic](../virtual_mic/README.md)).
- **macOS:** a Developer ID certificate and notarization remove the Gatekeeper prompt. Set
  `APPLE_CERTIFICATE`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD` and
  `APPLE_TEAM_ID` secrets; `tauri-action` signs and notarizes automatically.

## Model downloads

[`alshell7/hush-ai`](https://huggingface.co/alshell7/hush-ai) holds the RE-USE weights the
app downloads (`models/realtime-reuse-4c0c186.safetensors`, NVIDIA revision `4c0c186`), the
NVIDIA license and the model card. The app falls back to NVIDIA's repository at the same
revision. Both are checked against the same SHA-256.

The same repository holds the DeepFilterNet 3 archive
(`models/DeepFilterNet3_onnx-v0.5.6.tar.gz`) with its MIT and Apache-2.0 licenses. The app
falls back to the DeepFilterNet repository at tag v0.5.6, then jsDelivr's copy of that tag.
All three are checked against one SHA-256.
