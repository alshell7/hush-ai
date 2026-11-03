# Security

Hush AI runs with your microphone and, during setup, installs an audio driver with
administrator rights. Please report security problems privately.

## Report a problem

Use [GitHub private vulnerability reporting](https://github.com/alshell7/hush-ai/security/advisories/new).
Do not open a public issue. Include the version, your system and the steps to reproduce.

## What Hush AI does

- Audio stays on your computer. The app makes network requests only to download the model
  (Hugging Face) and to check for updates (GitHub).
- The model download is checked against a fixed SHA-256 before use.
- Updates are signed; the app checks the signature before it installs one.
- Administrator rights are used only for: installing or removing the macOS plug-in in
  `/Library/Audio/Plug-Ins/HAL`, and renaming or restoring VB-CABLE's endpoint names on Windows.
