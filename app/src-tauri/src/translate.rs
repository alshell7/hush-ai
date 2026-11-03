//! Live Translate, both ways, through Google's Gemini Live Translate model:
//! - your voice, translated for them: the live session hands its output to this module and
//!   sends the translated voice to the Hush AI Microphone instead;
//! - their voice, translated for you: the listener (`listen.rs`) hands what the meeting app
//!   plays, and shows (and can play) the translation.
//!
//! This is the one Hush feature that sends audio off the computer, to Google, and only while
//! a direction is on. It uses the user's own Gemini API key, stored in Hush's settings folder.
use crate::conversation::{Line, Side};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use eyre::{eyre, Result};
use hush_core::resample::StreamingMono;
use rtrb::{Consumer, Producer};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    net::TcpStream,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};
use tungstenite::{stream::MaybeTlsStream, Message, WebSocket};

pub const MODEL: &str = "gemini-3.5-live-translate-preview";
const ENDPOINT: &str = "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent";
/// Gemini takes 16 kHz and answers at 24 kHz, both 16-bit mono PCM.
const SEND_RATE: u32 = 16_000;
const RECEIVE_RATE: u32 = 24_000;
/// 100 ms per message, as Google recommends.
const CHUNK: usize = SEND_RATE as usize / 10;
/// Captions keep the last few sentences.
const CAPTION_CHARS: usize = 220;
/// A pause this long ends a phrase.
const ENTRY_PAUSE: Duration = Duration::from_millis(2500);

/// Languages Gemini Live Translate speaks (BCP-47).
pub const LANGUAGES: &[&str] = &[
    "af", "ak", "sq", "am", "ar", "hy", "az", "eu", "be", "bn", "bg", "my", "ca", "zh-Hans",
    "zh-Hant", "hr", "cs", "da", "nl", "en", "et", "fil", "fi", "fr", "gl", "ka", "de", "el", "gu",
    "ha", "he", "hi", "hu", "is", "id", "it", "ja", "jv", "kn", "kk", "km", "rw", "ko", "lo", "lv",
    "lt", "mk", "ms", "ml", "mr", "mn", "ne", "nb", "fa", "pl", "pt-BR", "pt-PT", "pa", "ro", "ru",
    "sr", "sd", "si", "sk", "sl", "es", "su", "sw", "sv", "ta", "te", "th", "tr", "uk", "ur", "uz",
    "vi", "zu",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Off,
    Connecting,
    On,
    Error,
}

/// One direction of a conversation, as the windows show it.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaneInfo {
    pub on: bool,
    pub status: Status,
    pub message: Option<String>,
    /// What was said, and its translation, as rolling captions.
    pub original: String,
    pub translated: String,
    /// The language Gemini hears.
    pub detected: Option<String>,
    /// The phrase still being said: not yet in the conversation.
    pub pending_original: String,
    pub pending_translated: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslateInfo {
    pub has_key: bool,
    /// The last four characters of the key, to recognize it.
    pub key_hint: Option<String>,
    /// Your voice, translated for them.
    pub me: LaneInfo,
    /// Their voice, translated for you.
    pub them: LaneInfo,
}

#[derive(Default, Serialize, Deserialize)]
struct Stored {
    key: Option<String>,
}

struct Lane {
    on: bool,
    status: Status,
    message: Option<String>,
    original: String,
    translated: String,
    detected: Option<String>,
    /// The phrase being translated now, and when text last arrived for it.
    current: Option<Line>,
    last_text: Instant,
}

impl Lane {
    fn new() -> Self {
        Self {
            on: false,
            status: Status::Off,
            message: None,
            original: String::new(),
            translated: String::new(),
            detected: None,
            current: None,
            last_text: Instant::now(),
        }
    }

    fn info(&self) -> LaneInfo {
        LaneInfo {
            on: self.on,
            status: self.status,
            message: self.message.clone(),
            original: self.original.clone(),
            translated: self.translated.clone(),
            detected: self.detected.clone(),
            pending_original: self
                .current
                .as_ref()
                .map(|l| l.original.clone())
                .unwrap_or_default(),
            pending_translated: self
                .current
                .as_ref()
                .map(|l| l.translated.clone())
                .unwrap_or_default(),
        }
    }
}

struct Inner {
    key: Option<String>,
    lanes: [Lane; 2],
    last_emit: Instant,
}

fn index(side: Side) -> usize {
    match side {
        Side::Me => 0,
        Side::Them => 1,
    }
}

/// The languages of one direction: (spoken, translated into). You speak `translate_from`
/// and they hear `translate_to`; they speak `translate_to` and you read `translate_from`.
pub fn languages(settings: &crate::settings::Settings, side: Side) -> (String, String) {
    match side {
        Side::Me => (
            settings.translate_from.clone(),
            settings.translate_to.clone(),
        ),
        Side::Them => (
            settings.translate_to.clone(),
            settings.translate_from.clone(),
        ),
    }
}

/// The app-wide translation state. Each live session reads its direction's flag.
pub struct Translator {
    inner: Mutex<Inner>,
    /// Where the API key is kept.
    path: PathBuf,
    /// Shared with the audio taps: your voice, their voice.
    flags: [Arc<AtomicBool>; 2],
}

impl Translator {
    pub fn new(app: &AppHandle) -> Self {
        let path = app
            .path()
            .app_config_dir()
            .unwrap_or_default()
            .join("translate.json");
        let stored: Stored = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            inner: Mutex::new(Inner {
                key: stored.key,
                lanes: [Lane::new(), Lane::new()],
                last_emit: Instant::now(),
            }),
            path,
            flags: [
                Arc::new(AtomicBool::new(false)),
                Arc::new(AtomicBool::new(false)),
            ],
        }
    }

    pub fn info(&self) -> TranslateInfo {
        let inner = self.inner.lock().unwrap();
        TranslateInfo {
            has_key: inner.key.is_some(),
            key_hint: inner.key.as_deref().map(|k| {
                k.chars()
                    .rev()
                    .take(4)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect()
            }),
            me: inner.lanes[0].info(),
            them: inner.lanes[1].info(),
        }
    }

    /// Whether a direction is on.
    pub fn is_on(&self, side: Side) -> bool {
        self.inner.lock().unwrap().lanes[index(side)].on
    }

    /// The flag an audio tap follows.
    pub fn flag(&self, side: Side) -> Arc<AtomicBool> {
        self.flags[index(side)].clone()
    }

    fn publish(&self, app: &AppHandle) {
        let info = self.info();
        crate::tray::update_translate(app, info.me.on, info.them.on);
        let _ = app.emit("translate", info);
    }

    pub fn key(&self) -> Option<String> {
        self.inner.lock().unwrap().key.clone()
    }

    /// Check a key with a short session, then keep it.
    pub fn set_key(&self, app: &AppHandle, key: &str) -> Result<TranslateInfo> {
        let key = key.trim().to_string();
        if key.len() < 20 || key.contains(char::is_whitespace) {
            return Err(eyre!("This does not look like a Gemini API key."));
        }
        let target = app.state::<crate::AppState>().settings().translate_to;
        Session::open(&key, &target, None)?;
        std::fs::create_dir_all(self.path.parent().unwrap_or(&self.path))?;
        let temporary = self.path.with_extension("json.tmp");
        std::fs::write(
            &temporary,
            serde_json::to_vec(&Stored {
                key: Some(key.clone()),
            })?,
        )?;
        std::fs::rename(&temporary, &self.path)?;
        self.inner.lock().unwrap().key = Some(key);
        self.publish(app);
        Ok(self.info())
    }

    pub fn remove_key(&self, app: &AppHandle) -> TranslateInfo {
        let _ = std::fs::remove_file(&self.path);
        self.set_on(app, Side::Me, false);
        self.set_on(app, Side::Them, false);
        self.inner.lock().unwrap().key = None;
        self.publish(app);
        self.info()
    }

    /// Turn one direction on or off.
    pub fn set_on(&self, app: &AppHandle, side: Side, on: bool) {
        let ended = {
            let mut inner = self.inner.lock().unwrap();
            let no_key = inner.key.is_none();
            let lane = &mut inner.lanes[index(side)];
            if on && no_key {
                lane.message = Some("Add your Gemini API key first.".into());
                lane.status = Status::Error;
                drop(inner);
                self.publish(app);
                return;
            }
            lane.on = on;
            lane.status = if on { Status::Connecting } else { Status::Off };
            lane.message = None;
            if on {
                lane.original.clear();
                lane.translated.clear();
                lane.detected = None;
            }
            if on {
                None
            } else {
                lane.current.take()
            }
        };
        if let Some(line) = ended {
            self.keep(app, line);
        }
        self.flags[index(side)].store(on, Ordering::Release);
        self.publish(app);
    }

    fn status(&self, app: &AppHandle, side: Side, status: Status, message: Option<String>) {
        {
            let mut inner = self.inner.lock().unwrap();
            let lane = &mut inner.lanes[index(side)];
            if !lane.on && status != Status::Off {
                return;
            }
            if lane.status == status && lane.message == message {
                return;
            }
            lane.status = status;
            lane.message = message;
        }
        self.publish(app);
    }

    /// Add caption text; sends at most 8 updates per second.
    fn caption(
        &self,
        app: &AppHandle,
        side: Side,
        original: Option<&str>,
        translated: Option<&str>,
        language: Option<String>,
    ) {
        let send = {
            let mut inner = self.inner.lock().unwrap();
            let lane = &mut inner.lanes[index(side)];
            let roll = |text: &mut String, add: &str| {
                text.push_str(add);
                if text.len() > CAPTION_CHARS * 2 {
                    let cut = text.len() - CAPTION_CHARS;
                    let cut = (cut..text.len())
                        .find(|&i| text.is_char_boundary(i))
                        .unwrap_or(0);
                    text.replace_range(..cut, "…");
                }
            };
            if let Some(add) = original {
                roll(&mut lane.original, add);
            }
            if let Some(add) = translated {
                roll(&mut lane.translated, add);
            }
            if language.is_some() {
                lane.detected = language;
            }
            // The same text, phrase by phrase, for the conversation.
            if lane.current.is_none() {
                let (from, to) = languages(&app.state::<crate::AppState>().settings(), side);
                lane.current = Some(Line {
                    at: chrono::Utc::now().timestamp_millis(),
                    side,
                    from,
                    to,
                    original: String::new(),
                    translated: String::new(),
                });
            }
            let detected = lane.detected.clone();
            if let Some(line) = lane.current.as_mut() {
                line.original.push_str(original.unwrap_or(""));
                line.translated.push_str(translated.unwrap_or(""));
                if let Some(language) = detected {
                    line.from = language;
                }
            }
            lane.last_text = Instant::now();
            let due = inner.last_emit.elapsed() >= Duration::from_millis(125);
            if due {
                inner.last_emit = Instant::now();
            }
            due
        };
        if send {
            let _ = app.emit("translate", self.info());
        }
    }

    /// Ends the current phrase after a pause. Called often by the translator threads.
    fn tick(&self, app: &AppHandle, side: Side) {
        let ended = {
            let mut inner = self.inner.lock().unwrap();
            let lane = &mut inner.lanes[index(side)];
            if lane.current.is_some() && lane.last_text.elapsed() >= ENTRY_PAUSE {
                lane.current.take()
            } else {
                None
            }
        };
        if let Some(line) = ended {
            self.keep(app, line);
            let _ = app.emit("translate", self.info());
        }
    }

    /// Add a finished phrase to the conversations, if the user keeps them.
    fn keep(&self, app: &AppHandle, mut line: Line) {
        line.original = line.original.trim().to_string();
        line.translated = line.translated.trim().to_string();
        if line.original.is_empty() && line.translated.is_empty() {
            return;
        }
        if !app.state::<crate::AppState>().settings().translate_history {
            return;
        }
        app.state::<crate::conversation::Conversations>()
            .add(app, line);
    }
}

// ---------------------------------------------------------------- Gemini Live session

/// One WebSocket session with Gemini Live Translate.
pub struct Session {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
}

/// What a server message carried.
#[derive(Default, Debug)]
pub struct Received {
    pub audio: Vec<i16>,
    pub heard: Option<String>,
    pub said: Option<String>,
    pub language: Option<String>,
    pub resume: Option<String>,
    pub go_away: bool,
}

fn set_timeout(socket: &WebSocket<MaybeTlsStream<TcpStream>>, timeout: Option<Duration>) {
    let stream = match socket.get_ref() {
        MaybeTlsStream::Plain(stream) => stream,
        MaybeTlsStream::Rustls(stream) => stream.get_ref(),
        _ => return,
    };
    let _ = stream.set_read_timeout(timeout);
}

fn friendly(error: &str) -> String {
    let lower = error.to_lowercase();
    if lower.contains("api key not valid")
        || lower.contains("api_key_invalid")
        || lower.contains("permission")
        || lower.contains("401")
        || lower.contains("403")
    {
        "Google did not accept this API key.".into()
    } else if lower.contains("quota")
        || lower.contains("resource_exhausted")
        || lower.contains("429")
    {
        "Your Gemini quota is used up. Try again later or check your plan in Google AI Studio."
            .into()
    } else if lower.contains("dns")
        || lower.contains("connect")
        || lower.contains("timed out")
        || lower.contains("network")
    {
        "Cannot reach Google. Check your internet connection.".into()
    } else {
        error.lines().next().unwrap_or(error).to_string()
    }
}

impl Session {
    /// Connect, send the setup and wait for Google to accept it.
    pub fn open(key: &str, target: &str, resume: Option<&str>) -> Result<Self> {
        let url = format!("{ENDPOINT}?key={key}");
        let (mut socket, _) = tungstenite::connect(url.as_str())
            .map_err(|e| eyre!(friendly(&e.to_string().replace(key, "<key>"))))?;
        let mut setup = json!({
            "setup": {
                "model": format!("models/{MODEL}"),
                // Transcripts are setup fields here, not generationConfig as Google's guide shows.
                "inputAudioTranscription": {},
                "outputAudioTranscription": {},
                "generationConfig": {
                    "responseModalities": ["AUDIO"],
                    "translationConfig": {
                        "targetLanguageCode": target,
                        // Speech already in their language is repeated, so they always hear you.
                        "echoTargetLanguage": true,
                    },
                },
                // Sessions outlive Google's 10-minute connections.
                "sessionResumption": {},
                "contextWindowCompression": { "slidingWindow": {} },
            }
        });
        if let Some(handle) = resume {
            setup["setup"]["sessionResumption"] = json!({ "handle": handle });
        }
        socket.send(Message::text(setup.to_string()))?;
        set_timeout(&socket, Some(Duration::from_secs(10)));
        loop {
            let message = socket.read().map_err(|e| eyre!(friendly(&e.to_string())))?;
            let text = match message {
                Message::Text(text) => text.to_string(),
                Message::Binary(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
                Message::Close(frame) => {
                    let reason = frame.map(|f| f.reason.to_string()).unwrap_or_default();
                    return Err(eyre!(friendly(if reason.is_empty() {
                        "Google closed the session"
                    } else {
                        &reason
                    })));
                }
                _ => continue,
            };
            let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            if value.get("setupComplete").is_some() {
                break;
            }
        }
        set_timeout(&socket, Some(Duration::from_millis(5)));
        Ok(Self { socket })
    }

    /// Send 16 kHz mono samples.
    pub fn send(&mut self, samples: &[f32]) -> Result<()> {
        let mut bytes = Vec::with_capacity(samples.len() * 2);
        for sample in samples {
            bytes.extend_from_slice(
                &((sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes(),
            );
        }
        let message = json!({
            "realtimeInput": { "audio": { "data": BASE64.encode(bytes), "mimeType": "audio/pcm;rate=16000" } }
        });
        self.socket.send(Message::text(message.to_string()))?;
        Ok(())
    }

    /// The next server message, or `None` when nothing arrived yet.
    pub fn receive(&mut self) -> Result<Option<Received>> {
        let message = match self.socket.read() {
            Ok(message) => message,
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                return Ok(None)
            }
            Err(e) => return Err(eyre!(friendly(&e.to_string()))),
        };
        let text = match message {
            Message::Text(text) => text.to_string(),
            Message::Binary(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Message::Close(frame) => {
                let reason = frame.map(|f| f.reason.to_string()).unwrap_or_default();
                return Err(eyre!(friendly(if reason.is_empty() {
                    "Google closed the session"
                } else {
                    &reason
                })));
            }
            _ => return Ok(Some(Received::default())),
        };
        Ok(Some(parse(&text)))
    }
}

fn parse(text: &str) -> Received {
    let value: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let mut out = Received::default();
    if let Some(content) = value.get("serverContent") {
        if let Some(parts) = content
            .pointer("/modelTurn/parts")
            .and_then(Value::as_array)
        {
            for part in parts {
                if let Some(data) = part.pointer("/inlineData/data").and_then(Value::as_str) {
                    if let Ok(bytes) = BASE64.decode(data) {
                        let (pairs, _) = bytes.as_chunks::<2>();
                        out.audio
                            .extend(pairs.iter().map(|b| i16::from_le_bytes(*b)));
                    }
                }
            }
        }
        if let Some(t) = content.get("inputTranscription") {
            out.heard = t.get("text").and_then(Value::as_str).map(str::to_string);
            out.language = t
                .get("languageCode")
                .and_then(Value::as_str)
                .map(str::to_string);
        }
        if let Some(t) = content.get("outputTranscription") {
            out.said = t.get("text").and_then(Value::as_str).map(str::to_string);
        }
    }
    if let Some(update) = value.get("sessionResumptionUpdate") {
        if update
            .get("resumable")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            out.resume = update
                .get("newHandle")
                .and_then(Value::as_str)
                .map(str::to_string);
        }
    }
    out.go_away = value.get("goAway").is_some();
    out
}

// ---------------------------------------------------------------- Audio taps and workers

/// The audio side of one direction. While it is on, the voice goes to the translator, and
/// [`apply`](Self::apply) replaces it with the translated voice. Lock-free and allocation-free:
/// two queues and a flag.
pub struct Tap {
    active: Arc<AtomicBool>,
    clean: Producer<f32>,
    translated: Consumer<f32>,
    /// The device's rate, set once the session knows it.
    pub rate: Arc<AtomicU32>,
}

impl Tap {
    /// Send the voice and play the translation in its place.
    pub fn apply(&mut self, block: &mut [f32]) {
        if !self.feed(block) {
            return;
        }
        for sample in block.iter_mut() {
            *sample = self.translated.pop().unwrap_or(0.0);
        }
    }

    /// Whether the direction is on.
    pub fn is_on(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    /// Send the voice only; the translated voice drains away unheard. Returns whether the
    /// direction is on.
    pub fn feed(&mut self, block: &[f32]) -> bool {
        if !self.active.load(Ordering::Relaxed) {
            // Let an old translation drain away, a little per callback.
            for _ in 0..block.len() * 4 {
                if self.translated.pop().is_err() {
                    break;
                }
            }
            return false;
        }
        for sample in block {
            let _ = self.clean.push(*sample);
        }
        true
    }

    /// Feed the voice and take as much translated voice as is ready (for mixing or replacing).
    pub fn feed_and_take(&mut self, block: &[f32], out: &mut [f32]) -> usize {
        if !self.feed(block) {
            return 0;
        }
        let mut taken = 0;
        for slot in out.iter_mut() {
            match self.translated.pop() {
                Ok(sample) => {
                    *slot = sample;
                    taken += 1;
                }
                Err(_) => break,
            }
        }
        taken
    }
}

/// The translator's side of a [`Tap`].
pub struct Link {
    clean: Consumer<f32>,
    translated: Producer<f32>,
    rate: Arc<AtomicU32>,
}

/// A connected tap and link for one direction of one session.
pub fn tap(active: Arc<AtomicBool>) -> (Tap, Link) {
    // Two seconds of voice to send, and half a minute of translation to play.
    let (clean_in, clean_out) = rtrb::RingBuffer::new(96_000 * 2);
    let (translated_in, translated_out) = rtrb::RingBuffer::new(96_000 * 30);
    let rate = Arc::new(AtomicU32::new(0));
    (
        Tap {
            active,
            clean: clean_in,
            translated: translated_out,
            rate: rate.clone(),
        },
        Link {
            clean: clean_out,
            translated: translated_in,
            rate,
        },
    )
}

/// Runs for as long as its session; connects to Gemini only while its direction is on.
pub fn run(app: AppHandle, mut link: Link, stop: Arc<AtomicBool>, side: Side) {
    let translator = app.state::<Arc<Translator>>().inner().clone();
    let active = translator.flag(side);
    let mut session: Option<(Session, String)> = None;
    let mut resume: Option<String> = None;
    let mut retry_at = Instant::now();
    let mut failures = 0u32;
    let mut pending: Vec<f32> = Vec::with_capacity(CHUNK * 2);
    // The session sets the device rate once its devices are open.
    let rate = loop {
        let rate = link.rate.load(Ordering::Acquire);
        if rate > 0 || stop.load(Ordering::Acquire) {
            break rate;
        }
        thread::sleep(Duration::from_millis(20));
    };
    if rate == 0 {
        return;
    }
    let mut down = StreamingMono::new(rate, SEND_RATE).expect("rates");
    let mut up = StreamingMono::new(RECEIVE_RATE, rate).expect("rates");

    while !stop.load(Ordering::Acquire) {
        if !active.load(Ordering::Acquire) {
            if session.take().is_some() {
                resume = None;
            }
            while link.clean.pop().is_ok() {}
            pending.clear();
            thread::sleep(Duration::from_millis(30));
            continue;
        }
        let (_, target) = languages(&app.state::<crate::AppState>().settings(), side);
        // A new target language needs a new session.
        if session.as_ref().is_some_and(|(_, t)| *t != target) {
            session = None;
            resume = None;
        }
        if session.is_none() {
            if Instant::now() < retry_at {
                while link.clean.pop().is_ok() {}
                thread::sleep(Duration::from_millis(30));
                continue;
            }
            translator.status(&app, side, Status::Connecting, None);
            let Some(key) = translator.key() else {
                translator.set_on(&app, side, false);
                continue;
            };
            match Session::open(&key, &target, resume.as_deref()) {
                Ok(opened) => {
                    failures = 0;
                    session = Some((opened, target.clone()));
                    translator.status(&app, side, Status::On, None);
                }
                Err(error) => {
                    failures += 1;
                    resume = None;
                    let wait = [2, 4, 8, 15][(failures as usize - 1).min(3)];
                    retry_at = Instant::now() + Duration::from_secs(wait);
                    log::warn!("Translate ({side:?}): {error:#}");
                    translator.status(&app, side, Status::Error, Some(format!("{error}")));
                    continue;
                }
            }
        }
        let Some((live, _)) = session.as_mut() else {
            continue;
        };
        let mut broken = None;
        // Voice in, 100 ms at a time.
        while let Ok(sample) = link.clean.pop() {
            down.push(sample, |s| pending.push(s));
        }
        while pending.len() >= CHUNK {
            if let Err(error) = live.send(&pending[..CHUNK]) {
                broken = Some(error);
                break;
            }
            pending.drain(..CHUNK);
        }
        // Everything Google sent so far.
        for _ in 0..64 {
            match live.receive() {
                Ok(None) => break,
                Ok(Some(message)) => {
                    for sample in &message.audio {
                        up.push(*sample as f32 / i16::MAX as f32, |s| {
                            let _ = link.translated.push(s);
                        });
                    }
                    if message.heard.is_some() || message.said.is_some() {
                        translator.caption(
                            &app,
                            side,
                            message.heard.as_deref(),
                            message.said.as_deref(),
                            message.language,
                        );
                    }
                    if message.resume.is_some() {
                        resume = message.resume;
                    }
                    if message.go_away {
                        // Google ends this connection soon: continue on a new one.
                        broken = Some(eyre!("reconnecting"));
                        break;
                    }
                }
                Err(error) => {
                    broken = Some(error);
                    break;
                }
            }
        }
        if let Some(error) = broken {
            log::info!("Translate ({side:?}): connection ended ({error}); reconnecting");
            session = None;
            retry_at = Instant::now();
        }
        translator.tick(&app, side);
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_server_messages() {
        let audio = BASE64.encode([0x00, 0x40, 0xff, 0x7f]);
        let message = format!(
            r#"{{"serverContent":{{"modelTurn":{{"parts":[{{"inlineData":{{"mimeType":"audio/pcm;rate=24000","data":"{audio}"}}}}]}},"inputTranscription":{{"text":"Hola","languageCode":"es"}},"outputTranscription":{{"text":"Hello"}}}}}}"#
        );
        let received = parse(&message);
        assert_eq!(received.audio, vec![0x4000, 0x7fff]);
        assert_eq!(received.heard.as_deref(), Some("Hola"));
        assert_eq!(received.said.as_deref(), Some("Hello"));
        assert_eq!(received.language.as_deref(), Some("es"));
        let update = parse(r#"{"sessionResumptionUpdate":{"newHandle":"abc","resumable":true}}"#);
        assert_eq!(update.resume.as_deref(), Some("abc"));
        assert!(parse(r#"{"goAway":{"timeLeft":"10s"}}"#).go_away);
    }

    /// `HUSH_TEST_GEMINI_KEY=… HUSH_TEST_SPEECH=<16 kHz wav> cargo test -p hush-ai gemini_translates -- --ignored --nocapture`
    /// streams a recording to Gemini in real time and writes the translation next to it.
    #[test]
    #[ignore]
    fn gemini_translates() {
        let (Ok(key), Ok(speech)) = (
            std::env::var("HUSH_TEST_GEMINI_KEY"),
            std::env::var("HUSH_TEST_SPEECH"),
        ) else {
            return;
        };
        let mut reader = hound::WavReader::open(&speech).unwrap();
        let rate = reader.spec().sample_rate;
        let input: Vec<f32> = reader
            .samples::<i16>()
            .map(|s| s.unwrap() as f32 / i16::MAX as f32)
            .collect();
        let samples = hush_core::resample::mono(&input, rate, SEND_RATE).unwrap();
        let started = Instant::now();
        let mut session = Session::open(&key, "es", None).expect("setup");
        println!("setup complete in {:?}", started.elapsed());
        let (mut heard, mut said, mut audio) = (String::new(), String::new(), Vec::new());
        let mut first_audio = None;
        let silence = vec![0.0; SEND_RATE as usize * 4];
        let all: Vec<f32> = samples.iter().copied().chain(silence).collect();
        let begin = Instant::now();
        for (i, chunk) in all.chunks(CHUNK).enumerate() {
            session.send(chunk).unwrap();
            // Real time: one chunk every 100 ms.
            let due = begin + Duration::from_millis(100 * (i as u64 + 1));
            while Instant::now() < due {
                if let Some(message) = session.receive().unwrap() {
                    if !message.audio.is_empty() && first_audio.is_none() {
                        first_audio = Some(begin.elapsed());
                    }
                    audio.extend(message.audio);
                    heard.push_str(message.heard.as_deref().unwrap_or(""));
                    said.push_str(message.said.as_deref().unwrap_or(""));
                } else {
                    thread::sleep(Duration::from_millis(5));
                }
            }
        }
        println!(
            "input {:.1} s, translated audio {:.1} s, first audio after {:?}",
            samples.len() as f32 / SEND_RATE as f32,
            audio.len() as f32 / RECEIVE_RATE as f32,
            first_audio
        );
        println!("heard: {heard}\nsaid: {said}");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: RECEIVE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let out = std::path::Path::new(&speech).with_extension("es.wav");
        let mut writer = hound::WavWriter::create(&out, spec).unwrap();
        for s in audio {
            writer.write_sample(s).unwrap();
        }
        writer.finalize().unwrap();
        assert!(!said.is_empty(), "no translation came back");
    }
}
