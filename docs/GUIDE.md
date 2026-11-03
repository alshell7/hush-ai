# User guide

[Home](../README.md) · [Architecture](ARCHITECTURE.md) · [Performance](PERFORMANCE.md)

## Install

Download the file for your computer from the
[latest release](https://github.com/alshell7/hush-ai/releases/latest).

| Computer | File | First start |
| --- | --- | --- |
| Windows 10 or 11 | `…_x64-setup.exe` (ARM: `…_arm64-setup.exe`) | SmartScreen: **More info › Run anyway** |
| Mac with Apple silicon | `…_aarch64.dmg` | Right-click the app › **Open** (or System Settings › Privacy & Security › **Open Anyway**) |
| Intel Mac | `…_x64.dmg` | Same as above |
| Linux | `.AppImage`, `.deb` or `.rpm` | Needs PipeWire or PulseAudio (`pactl`) |

The warnings appear because the apps are not code-signed yet. They appear only once.

## First start

1. **Voice model.** Choose a model and download it. Hush checks the file before it uses it.
   - **Efficient** (DeepFilterNet 3, 8 MB, recommended): little CPU, MIT or Apache-2.0
     license, so you can use it at work too.
   - **NVIDIA RE-USE** (54 MB): more CPU. Accept NVIDIA's noncommercial license first.
2. **Microphone.** Choose your microphone and say something. The bar moves when Hush hears you.
3. **Hush AI Microphone.**
   - **macOS:** press **Install**. macOS asks for your password once.
   - **Windows:** Hush shows four steps: download the *VBCABLE_Driver_Pack* from
     [vb-audio.com/Cable](https://vb-audio.com/Cable/), extract it, run
     *VBCABLE_Setup_x64* as administrator, press **Install Driver**, restart Windows.
     Open Hush again and press **Rename**. Windows asks for permission once.
   - **Linux:** nothing to do.
4. **Hear the difference.** Record six seconds with some noise around you (type, tap the
   desk), then play **Original** and **With Hush**.
5. **Use it.** Choose **Hush AI Microphone** in your apps.

## Choose Hush AI Microphone in your apps

| App | Where |
| --- | --- |
| Zoom | Settings › Audio › Microphone |
| Google Meet | More options › Settings › Audio › Microphone |
| Microsoft Teams | Settings › Devices › Microphone |
| Discord | User Settings › Voice & Video › Input Device |
| Slack | Preferences › Audio & video › Microphone |
| OBS Studio | Settings › Audio › Mic/Auxiliary Audio |
| Browser apps | The browser's site settings, or the app's own audio settings |
| Windows | Settings › System › Sound › Input |
| macOS | System Settings › Sound › Input |

Do not choose Hush AI Microphone as your microphone **inside Hush**. Hush hides it from its
own list.

## Every day

- **Tray or menu bar.** Click the mochi: turn noise cancellation on or off, change the
  microphone or the RE-USE quality. Right-click for the menu.
- **Closing the window** keeps Hush running in the tray. Use **Quit** to stop it. When Hush
  is not running, Hush AI Microphone is silent.
- **Open at login** (Settings) starts Hush in the tray when you log in.

### Voice model

| Model | Use it for | Cost | License |
| --- | --- | --- | --- |
| **Efficient** (DeepFilterNet 3) | Everything (48 kHz full band) | Lowest CPU, one thread | MIT or Apache-2.0 |
| **NVIDIA RE-USE** | Light, Balanced or Studio, below | More CPU | Non-commercial use only |

Switch on the main screen. Settings › Voice models downloads or removes each model. On the
[VoiceBank-DEMAND test](COMPARISON.md), DeepFilterNet 3 scored higher than RE-USE Light on
every metric.

### RE-USE quality

| Quality | Use it for | Cost |
| --- | --- | --- |
| **Light** | Calls and meetings (16 kHz voice band) | Lowest delay and CPU of RE-USE |
| **Balanced** | Clearer voice (24 kHz) | More CPU |
| **Studio** | Recording, streaming (48 kHz full band) | Needs a fast CPU |

The **Load** chip shows how much of real time the model needs. Below 100% it keeps up. If
it stays high, choose Efficient, or Light with RE-USE.

### Listen to yourself

Turn on **Listen to yourself** to hear Hush in your headphones and switch between
**Original** and **With Hush**. While Listen is on, apps do not get your voice. Use
headphones, or your speakers will echo.

## Fix problems

| Problem | Fix |
| --- | --- |
| Apps hear nothing | Check that Hush is **On** and the app uses **Hush AI Microphone**. |
| My voice sounds robotic or cut | Choose **Efficient**, or **Light** with RE-USE. Close heavy programs. On Windows, keep **Audio priority** on. |
| "Your microphone is not available" | Reconnect it or choose another. Hush retries on its own. |
| Windows: Hush AI Microphone missing after a VB-CABLE update | Open Hush › Set up › **Rename** again. |
| macOS: Hush AI Microphone missing | Settings › Hush AI Microphone, or run setup again (it restarts Core Audio). |
| Delay is noticeable | Windows with VB-CABLE adds about 84 ms. In VB-CABLE's control panel, a smaller buffer lowers it. |

## Uninstall

1. Settings › Hush AI Microphone › **Remove** (macOS) or **Restore VB-CABLE names** (Windows).
2. Uninstall the app like any other app. On Windows, uninstall VB-CABLE too if you no
   longer need it.
3. Optional: delete the models and settings folder:
   - Windows: `%APPDATA%\ai.hush.desktop`
   - macOS: `~/Library/Application Support/ai.hush.desktop`
   - Linux: `~/.local/share/ai.hush.desktop` and `~/.config/ai.hush.desktop`

## Privacy

Hush processes audio only on your computer. It connects to the internet to download a
model (Hugging Face, or GitHub as a fallback) and to check for updates (GitHub). It sends no audio, no usage data and
no crash reports.
