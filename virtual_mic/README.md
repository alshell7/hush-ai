# Hush AI Microphone

A virtual microphone. Hush writes enhanced speech into it, and any app (Google Meet, Zoom,
Teams, Discord, a browser) can choose **Hush AI Microphone** as its microphone. No
VB-CABLE or BlackHole is needed.

```
real microphone ─▶ Hush app (model) ─▶ ring buffer ─▶ driver ─▶ "Hush AI Microphone" ─▶ meeting app
```

## Design

| Rule | Why |
| --- | --- |
| The driver only copies audio. The model runs in the app | A meeting app cannot stall the model, and the driver stays small |
| 48 kHz mono float in a ring of 16384 frames (341 ms) | One fixed format: no conversion in the driver |
| Reads are indexed by the device clock | Several apps can record at once and get the same audio |
| The driver reads 10 ms behind the newest sample | Margin for the app's writer thread |
| More than 100 ms off: the driver jumps to the newest audio | Bounded delay after a pause or restart |
| Writer idle for 100 ms, or since the reader started: silence | Old audio never plays or repeats when the app stops, crashes or starts late |
| Samples are made finite and clipped to [-1, 1] in the driver | A bad value cannot reach a meeting app |
| One writer at a time (macOS: process ID slot; Windows: exclusive device) | Two Hush windows cannot mix their audio |
| No data: silence. No locks, no waiting, no allocation in the audio path | Audio deadlines are never missed by the driver |

Both drivers use the same reader, [`ring.h`](ring.h). [`test_ring.c`](test_ring.c) tests it
without a driver on Windows and macOS: silence before any write, no stale audio, writer
stop and restart, late writer, two clients, read sizes from 1 to 4096 frames with writer
jitter, long writer gaps, positions above 2^40, bursts, device time jumps, NaN and
infinities. Without the idle rule, the test finds 39 repeated blocks and 38,688 samples of
old audio after the app stops.

The app side is `hush_core::virtual_mic` (`VirtualMic`, `Feeder`). The feeder thread keeps one
driver read plus 10 ms buffered and follows the driver's clock through the live worker's clock
correction. It runs with real-time priority: Mach time-constraint policy on macOS, MMCSS on
Windows. In the app, turn noise cancellation on; the output goes to Hush AI Microphone.

## macOS

`macos/` is a Core Audio server plug-in, structured like Apple's NullAudio sample: one
plug-in, one device, one input stream. The plug-in creates the POSIX shared memory
`/hush.mic.v1` (layout in `shared.h`); the app maps it.

```sh
virtual_mic/macos/build.sh                      # universal arm64 + x86_64, ad-hoc signed
clang -framework CoreAudio -framework CoreFoundation -o target/test_plugin virtual_mic/macos/test_plugin.c
target/test_plugin target/HushMic.driver/Contents/MacOS/HushMic
sudo cp -R target/HushMic.driver /Library/Audio/Plug-Ins/HAL/ && sudo killall coreaudiod
cargo run --release -p hush-core --features devices --example virtual_mic_check 30
```

Remove: `sudo rm -rf /Library/Audio/Plug-Ins/HAL/HushMic.driver && sudo killall coreaudiod`.
`virtual_mic_check` needs microphone permission. Run it from Terminal on the Mac, not over
SSH: macOS gives SSH sessions silence.

Results (M1 Mac mini, macOS 26.6): the device appears in System Settings and in
`system_profiler SPAudioDataType` (48 kHz, 1 channel, Virtual). `test_plugin` passes.
`virtual_mic_check` records a 440 Hz tone through Core Audio. `macos/edge_tests.command`
runs these cases; all pass:

| Case | Result |
| --- | --- |
| Baseline (20 s; earlier also 6 × 30 s) | 0 glitches, no silence, longest writer gap 2.1 ms |
| Recorder buffer 64 frames / 4096 frames | 0 glitches |
| App stops while recording | 0 samples after 250 ms |
| App starts 4 s after the recorder | No audio before; audio at once after |
| Old audio in the ring, no app | Silence |
| Two recording apps at once | Both clean; the driver reads 17.0 s in 17 s (not twice) |
| A second writer | Refused: "in use by another Hush window" |
| `coreaudiod` restarts while the app writes | New recordings clean |
| 8 busy processes on the 8 cores | 0 glitches |

Release builds need a Developer ID signature and notarization.

## Windows with VB-CABLE (now)

Until the Hush driver has Microsoft's signature (see below), Hush uses **VB-CABLE**, the free
virtual cable by VB-Audio, signed by Microsoft. Hush does not bundle or download it: VB-Audio's
terms do not allow bundling.

During setup (and in **Set up** on the home screen), Hush checks for a Hush AI Microphone:

1. Not installed: Hush shows the steps and opens VB-Audio's page
   ([vb-audio.com/Cable](https://vb-audio.com/Cable/)). Download the Driver Pack, extract
   it, run the setup program as administrator, press **Install Driver**, restart Windows.
2. Installed: Hush offers to rename it. One administrator prompt starts
   `hush-ai.exe --vb-cable setup`, which renames the endpoints in the Windows property store:

   | VB-CABLE endpoint | Apps now see |
   | --- | --- |
   | CABLE Output (VB-Audio Virtual Cable), recording | **Hush AI Microphone (Hush)** |
   | CABLE Input (VB-Audio Virtual Cable), playback | Hush AI Microphone Feed (Hush) |

   The endpoint IDs go to `%ProgramData%\Hush\vb-cable-endpoints.txt`, errors to
   `vb-cable.log` in the same folder. `hush-ai.exe --vb-cable restore` (administrator) restores
   the original names of those endpoints only. Reinstalling VB-CABLE also restores them.
3. Noise cancellation on: Hush plays its output into the feed. In Meet, Zoom or any app, choose
   **Hush AI Microphone**. If the Hush driver is installed, Hush uses it instead.

The driver stays VB-Audio's (Device Manager shows it), and VB-CABLE's unused 16-channel
playback device ("CABLE In 16ch") keeps its name.

Results on the Windows laptop (VB-CABLE Driver Pack45):

| Test | Result |
| --- | --- |
| Rename (`--vb-cable setup`) | Both endpoints renamed, IDs recorded |
| `virtual_mic_check --cable`, 20 s + 3 × 30 s | 2 glitches in the first run after the rename, then 0 |
| App stops while recording | Silence afterwards |
| `loop_latency` feed → microphone | **83.7 ms** median (82.6–84.6), on top of Hush's own delay |
| Live app path (headset → model → feed), 30 s, while another app records | No lost audio, 82% of one core |

VB-CABLE adds about 84 ms. The Hush driver reads 10 ms behind the newest sample, so it will
cut most of that when it is signed.

## Windows driver (after signing)

`windows/` is Microsoft's SysVAD sample (`audio/sysvad`, commit `f4c1ff8`, MS-PL, see
`LICENSE-SysVAD.txt`), reduced to one capture endpoint:

- `TabletAudioSample/minipairs.h`: only the MicIn endpoint; no render endpoints.
- `TabletAudioSample/micinwavtable.h`: 48 kHz 16-bit mono only.
- `hushmic.h`, `hushmic.cpp`: the kernel ring and the control device `\\.\HushAIMicrophone`.
  The app writes with `IOCTL_HUSH_MIC_WRITE`; one writer at a time.
- `EndpointsCommon/minwavertstream.cpp`: `WriteBytes` copies from the ring instead of the
  test tone.
- `adapter.cpp`: creates the control device with the adapter, deletes it on removal;
  Bluetooth, USB and A2DP sideband paths are off by default.
- `TabletAudioSample/HushMic.inx`: a new INF for this one device (`Root\HushAIMicrophone`).

Windows lists it as **Hush AI Microphone (Hush)**.

```powershell
virtual_mic\windows\build.ps1                    # x64 and ARM64 packages in virtual_mic\windows\out\
virtual_mic\windows\install.ps1                  # administrator; test-signing mode only
virtual_mic\windows\install.ps1 -Uninstall
```

`build.ps1` needs Visual Studio 2022 with the **Windows Driver Kit** component and WDK
10.0.26100. It runs ApiValidator (both packages are "Universal") and InfVerif (valid).
Code analysis (`/p:RunCodeAnalysis=true`) reports no issues in the Hush changes except
floating point in the audio timer path (`ring.h`), which SysVAD's own tone generator also
uses there. The control device handles every request type itself; PortCls never sees its
requests, even after the device is deleted while the app still holds a handle.
Build ARM64 and x64 drivers separately; never run the x64 driver on Windows ARM.

The packages are signed with the WDK test certificate. They load only in test-signing mode:
`bcdedit /set testsigning on`, Secure Boot off, restart. Release packages need Microsoft
attestation signing through the Hardware Dev Center.

## Not tested yet

- Windows: installing and loading the driver, and audio through it. Needs test-signing mode
  on a physical x64 PC and on an ARM64 device. Then run `windows\edge_tests.ps1` (the same
  cases as macOS) and a meeting app. The shared reader is already tested on Windows by
  `test_ring.c`; the kernel parts (control device, IOCTL, WaveRT copy) are not.
- macOS: Google Meet or Zoom in a browser selecting Hush AI Microphone; Intel Macs.
- Both: the full app path with a real microphone into a meeting, for an hour.
