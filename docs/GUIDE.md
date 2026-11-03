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
   - **NVIDIA RE-USE** (54 MB): more CPU. Agree to its license terms first.
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

- **Tray or menu bar.** Click the mochi: turn noise cancellation or Live Translate on or
  off, swap languages, change the microphone, or join your next meeting. Double-click opens
  the window (Windows). Right-click for the menu.
- **Closing the window** keeps Hush running in the tray. Use **Quit** to stop it. When Hush
  is not running, Hush AI Microphone is silent.
- **Open at login** (Settings) starts Hush in the tray when you log in.

### Voice model

| Model | Use it for | Cost |
| --- | --- | --- |
| **Efficient** (DeepFilterNet 3) | Everything (48 kHz full band) | Lowest CPU, one thread |
| **NVIDIA RE-USE** | Light, Balanced or Studio, below | More CPU |

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

## Studio: clean up a recording

Open the **Studio** tab.

1. Drop a file on the window, press **Choose a file**, or press **Record yourself**. Pick
   the microphone to record (the bar shows its level) and press **Start recording** (up to
   30 minutes). Hush records the microphone as it is and cleans the recording afterwards;
   your calls and Hush AI Microphone are not affected.
2. Hush cleans it with your voice model. Long files are fine; the progress ring shows the
   time left.
3. Press play and switch between **Original** and **With Hush** while it plays (the **A**
   key switches, **Space** plays).
4. **Save clean WAV** writes a 48 kHz mono file. **Show in folder** opens where you saved it.

Files: WAV, MP3, M4A/AAC, FLAC, OGG, AIFF, CAF, and the sound of MP4, MOV and MKV. Stereo
is mixed to mono. Everything happens on your computer.

## Meetings: Google Calendar

Open the **Meetings** tab and press **Connect Google Calendar**.

1. **Open settings** opens Google Calendar's settings in your browser.
2. Under **Settings for my calendars**, pick your calendar, then **Integrate calendar**.
3. Copy **Secret address in iCal format**.
4. Come back to Hush. It notices the address on the clipboard, checks it with Google and
   shows the calendar's name and the meetings it found. Press **Connect**.

You can also paste the address into the field. Any calendar that publishes an iCal
(`.ics`) address over HTTPS works the same way.

| Setting | What it does |
| --- | --- |
| Check for changes every 5, 15, 30 or 60 min | How often Hush reads the calendar |
| Turn on noise cancellation 1, 2 or 5 min before | For meetings with a Meet, Zoom, Teams, Webex, Slack or Discord link |
| Turn it off after the call | Only if Hush turned it on, and you did not turn it off yourself |
| Pop up with a Join button | The tray widget opens a minute before the call |

### More than one calendar

Press **+ Add a calendar** on the Meetings tab and connect another secret address, up to 12.
The **Calendars** card lists each with its colour, the meetings it has, and whether it is
syncing or needs attention.

- **The switch** pauses a calendar without forgetting it.
- **✕, then Remove** forgets it.
- **Coloured dots** on a meeting show which calendars it comes from.
- **The same meeting in two calendars** (an invite on your work and personal calendar) shows
  once, and Hush pops up once.

If one calendar fails (its address was reset), the others keep working, and its row says
what to do.

**Join** opens the meeting's own link. **Remove** makes Hush forget an address. The
address is a secret link to your calendar: Hush keeps it only in its settings folder and
never writes it to logs. If it leaks, reset it in Google Calendar and connect again.

## Live Translate (Preview)

Speak your language and the call hears you in theirs, in a natural voice, through Hush AI
Microphone. Hush uses Google's **Gemini 3.5 Live Translate** model with your own API key.

1. On the **Translate** tab, press **Add your Gemini API key**. Hush opens Google AI Studio.
2. Press **Create API key** and copy it. Back in Hush, it is noticed, checked with Google and
   kept in Hush's settings folder.
3. Choose **I speak** and **They hear** (70+ languages). ⇄ swaps them.
4. Turn on **Live Translate**. Hush asks **Start translating now?** first, because your voice
   goes to Google. Tick **Don't ask again** to skip it, or turn it back on in Settings.
5. Captions show what you said and what they hear.

During a call, use the switch in the tray widget, **Translate my voice** in the tray menu,
or **Ctrl + Alt + T** (**⌘ ⌥ T** on macOS) in any app.

**With or without noise cancellation.** Live Translate does not need noise cancellation. With
it on, Google gets your cleaned voice. With it off, Hush runs a quiet pass-through session:
your voice goes to Google as it is, and the call still hears the translation through Hush AI
Microphone. Turning noise cancellation on or off does not stop the translation.

### History

The **History** on the Translate tab keeps what you said and what they heard, phrase by
phrase, grouped by day, with the two languages side by side.

- **Search** finds words in either language. **Copy** copies a translation.
- **Clear** deletes the history, after you confirm.
- It stays on this computer (`translate-history.json` in Hush's data folder), up to the last
  1,000 phrases. Turn off **Keep a history** in Settings and Hush stops saving new phrases.

Good to know:

- **It uses the internet.** While Live Translate is on, your voice goes to Google;
  everything else in Hush stays on your computer.
- **It follows you by a phrase or so.** Gemini translates phrase by phrase, so the other side
  hears you a moment after you speak.
- **Cost:** Google offers a free tier; paid use is billed by Google to your key. Google's
  terms apply to the audio you send.
- If you already speak their language, Gemini repeats you in that language, so they always
  hear you.

## Fix problems

| Problem | Fix |
| --- | --- |
| Apps hear nothing | Check that Hush is **On** and the app uses **Hush AI Microphone**. |
| My voice sounds robotic or cut | Choose **Efficient**, or **Light** with RE-USE. Close heavy programs. On Windows, keep **Audio priority** on. |
| "Your microphone is not available" | Reconnect it or choose another. Hush retries on its own. |
| Windows: Hush AI Microphone missing after a VB-CABLE update | Open Hush › Set up › **Rename** again. |
| macOS: Hush AI Microphone missing | Settings › Hush AI Microphone, or run setup again (it restarts Core Audio). |
| Delay is noticeable | Windows with VB-CABLE adds about 84 ms. In VB-CABLE's control panel, a smaller buffer lowers it. |
| Studio: "Hush cannot read this file" | Convert it to WAV or MP3. Opus files (for example WhatsApp voice notes) are not supported yet. |
| Meetings: "The calendar did not accept this address" | The secret address was reset. Copy it again from Google Calendar. |
| Live Translate: "Google did not accept this API key" | Create a new key in Google AI Studio and add it again. |
| Live Translate is on but they hear silence | Check that the call app uses **Hush AI Microphone**, speak a full sentence and wait a moment. Gemini answers phrase by phrase. |
| Meetings: one calendar in the list shows an error | Its secret address was reset. Remove it and add the new address; the other calendars keep working. |
| Ctrl + Alt + T does nothing | Another app uses the same shortcut. Use the switch in the widget instead. |

## Uninstall

1. Settings › Hush AI Microphone › **Remove** (macOS) or **Restore VB-CABLE names** (Windows).
2. Uninstall the app like any other app. On Windows, uninstall VB-CABLE too if you no
   longer need it.
3. Optional: delete the models and settings folder:
   - Windows: `%APPDATA%\ai.hush.desktop`
   - macOS: `~/Library/Application Support/ai.hush.desktop`
   - Linux: `~/.local/share/ai.hush.desktop` and `~/.config/ai.hush.desktop`

## Privacy

Noise cancellation and Studio run only on your computer. Hush connects to the internet to
download a model (Hugging Face, or the model's original source as a fallback), to check for
updates (GitHub), to read your calendar's iCal address if you connect one (Google), and to
send your voice to Google while **Live Translate** is on. It sends no usage data and no
crash reports, and it has no servers of its own.
