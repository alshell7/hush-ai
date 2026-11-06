# User guide

[Home](../README.md) · [Architecture](ARCHITECTURE.md) · [Performance](PERFORMANCE.md)

## Install

Download the file for your computer from the
[latest release](https://github.com/alshell7/hush-ai/releases/latest).

| Computer | File | First start |
| --- | --- | --- |
| Windows 10 or 11 | `…_x64-setup.exe` (ARM: `…_arm64-setup.exe`, NVIDIA RE-USE only for now: DeepFilterNet 3's inference library does not build for Windows on ARM yet) | SmartScreen: **More info › Run anyway** |
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

- **The notch.** Point at the island at the top of the screen: noise cancellation, Live
  Translate, their voice, captions and your next meeting.
- **Tray or menu bar.** Click the mochi to open the notch (or the widget, if you chose it).
  Turn noise cancellation or Live Translate on or off, swap languages, change the
  microphone, or join your next meeting. Double-click opens
  the window (Windows). Right-click for the menu.
- **Noise cancellation off** does not silence you: Hush AI Microphone passes your voice through
  as it is, so apps keep hearing you and you never have to switch microphones.
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

### Show one calendar

With more than one calendar, the chips above **Up next** show **All** or one calendar's
meetings. Hush remembers your choice.

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

## Talk: Live Translate both ways (Preview)

Talk with anyone in their language. They hear you in theirs through Hush AI Microphone; you
read them in yours, and can hear them translated too. Hush uses Google's **Gemini 3.5 Live
Translate** model with your own API key.

1. On the **Talk** tab, press **Add your key**. Hush opens Google AI Studio.
2. Press **Create API key** and copy it. Back in Hush, it is noticed, checked with Google and
   kept in Hush's settings folder.
3. Choose **I speak** and **They speak** (70+ languages). ⇄ swaps them. Hush remembers them.
4. Choose what to translate:
   - **Both ways**: they hear you in their language, you read them in yours.
   - **My voice**: only your voice is translated.
   - **Their voice**: only theirs.
5. Turn on **Live Translate**. Hush asks **Start translating now?** first, because voices go to
   Google. Tick **Don't ask again** to skip it, or turn it back on in Settings.

The Talk tab shows the call as a chat: their words in your language (what they said in small
print), yours with what they heard. During a call, use the notch, the tray menu
(**Translate my voice**, **Translate their voice**) or **Ctrl + Alt + T** (**⌘ ⌥ T** on
macOS) in any app; the shortcut turns on the directions you chose.

**With or without noise cancellation.** Your voice does not need noise cancellation. With it
on, Google gets your cleaned voice. With it off, Hush runs a quiet pass-through session.

### Their voice: Hush AI Speaker

Press **Their voice** on the Talk tab (or in Settings):

| Option | What it does |
| --- | --- |
| Clean their voice: Off, Light, Full | Removes their background noise before you hear it. Light keeps a little room sound. |
| You hear: Their voice | Their own voice; the translation shows as captions. |
| You hear: Voice-over | Their voice turns down while the translation speaks over it, like an interpreter on TV. The default: you hear who is talking and follow what they say. |
| You hear: Translation | Only the translated voice. |
| Play their voice on | Your real speakers or headphones. |

How Hush hears them:

- **macOS and Linux:** choose **Hush AI Speaker** as the speaker in your meeting app.
- **Windows:** VB-CABLE is already Hush AI Microphone, so Hush AI Speaker needs one more free
  virtual cable. Talk › **Their voice** walks you through it:
  1. **Open VB-Audio** and download **VB-Audio Hi-Fi Cable** (free for personal use).
  2. Extract it, run its setup as administrator, and restart Windows.
  3. Press **Name it Hush AI Speaker** (Windows asks for permission once). Hush renames the
     cable's playback side to **Hush AI Speaker** and its recording side to **Hush AI Speaker
     Monitor**.

  Then choose **Hush AI Speaker** as the speaker in your meeting app. VB-CABLE A or B works too.
  Until it is set up, Hush listens along to your speakers and translates them (it cannot clean
  or replace their voice that way). Settings › Hush AI Microphone › **Restore VB-CABLE names**
  puts the cable's own names back too.

When Hush quits, apps that played to Hush AI Speaker fall back to your speakers (Linux) or
stay silent until Hush runs again (macOS, Windows cable): choose your speakers again in the
meeting app if you stop using Hush.

### Conversations

Every call is kept as a conversation: one calendar meeting, or one stretch of talk (a pause
of 10 minutes starts a new one). Back-to-back meetings stay apart.

- **Search** finds words in either language, meeting names and titles.
- **Filters:** All, Starred, Meetings, and a language.
- **Rename** with a click on the title. **Star** the ones that matter; starred ones are kept
  longest.
- **Export** one conversation, or all of them, as Markdown, text or JSON.
- Conversations stay on this computer (`conversations.json` in Hush's data folder). Turn off
  **Keep conversations** in Settings and Hush stops saving new ones.

Good to know:

- **It uses the internet.** While a direction is on, that voice goes to Google; everything
  else in Hush stays on your computer.
- **It follows by a phrase or so.** Gemini translates phrase by phrase.
- **Cost:** Google offers a free tier; paid use is billed by Google to your key. Google's
  terms apply to the audio you send.

## The notch

Hush sits at the top of your screen as a small island (on a MacBook, it hides in the camera
notch).

- **Idle:** a sliver. It costs nothing to keep.
- **In a call** (noise cancellation on and a meeting, their voice playing, or Live Translate
  on): it shows your level and what is happening, with a softly turning rim.
- **Point at it** (or click the tray icon) and it opens: noise cancellation, Live Translate,
  their voice, both sides' captions and your next meeting with **Join**.

Clicks pass through everywhere except the island. Prefer the tray widget? Settings › Quick
controls › **Show Hush as** › **Tray widget**. **Hide from screen sharing** keeps Hush's
windows and the notch out of screenshots, screen sharing and recordings (Windows and macOS).

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
| Ctrl + Alt + T does nothing | Another app uses the same shortcut. Use the switch in the notch or widget instead. |
| I hear nothing from the meeting | The meeting app plays to Hush AI Speaker: keep Hush running, or choose your speakers in the meeting app. Check **Play their voice on** under Their voice. |
| Windows: Their voice says "Listening along" | Hush AI Speaker is not set up yet: translation works; to clean their voice, follow the steps under Talk › Their voice. |

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
send voices to Google while **Live Translate** is on (yours, theirs, or both, as you chose). It sends no usage data and no
crash reports, and it has no servers of its own.
