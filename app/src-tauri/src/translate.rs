//! Live Translate: your clean voice, translated as you speak, through Google's Gemini Live
//! Translate model. While it is on, the live session hands its output to this module and
//! sends the translated voice to the Hush AI Microphone instead.
//!
//! This is the one Hush feature that sends audio off the computer, to Google, and only while
//! it is on. It uses the user's own Gemini API key, stored in Hush's settings folder.
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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslateInfo {
    pub has_key: bool,
    /// The last four characters of the key, to recognize it.
    pub key_hint: Option<String>,
    pub on: bool,
    pub status: Status,
    pub message: Option<String>,
    /// What you said, and what the other side hears, as rolling captions.
    pub heard: String,
    pub said: String,
    /// The language Gemini hears you speaking.
    pub detected: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct Stored {
    key: Option<String>,
}

struct Inner {
    key: Option<String>,
    on: bool,
    status: Status,
    message: Option<String>,
    heard: String,
    said: String,
    detected: Option<String>,
    last_emit: Instant,
}

/// The app-wide translation state. The live session reads `on` and the languages from it.
pub struct Translator {
    inner: Mutex<Inner>,
    /// Where the API key is kept.
    path: PathBuf,
    /// Shared with the live session's audio tap.
    pub active: Arc<AtomicBool>,
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
                on: false,
                status: Status::Off,
                message: None,
                heard: String::new(),
                said: String::new(),
                detected: None,
                last_emit: Instant::now(),
            }),
            path,
            active: Arc::new(AtomicBool::new(false)),
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
            on: inner.on,
            status: inner.status,
            message: inner.message.clone(),
            heard: inner.heard.clone(),
            said: inner.said.clone(),
            detected: inner.detected.clone(),
        }
    }

    fn publish(&self, app: &AppHandle) {
        let _ = app.emit("translate", self.info());
        crate::tray::update_translate(app, self.inner.lock().unwrap().on);
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
        self.set_on(app, false);
        self.inner.lock().unwrap().key = None;
        self.publish(app);
        self.info()
    }

    /// Turn translation on or off. It works while noise cancellation runs.
    pub fn set_on(&self, app: &AppHandle, on: bool) {
        {
            let mut inner = self.inner.lock().unwrap();
            if on && inner.key.is_none() {
                inner.message = Some("Add your Gemini API key first.".into());
                inner.status = Status::Error;
                drop(inner);
                self.publish(app);
                return;
            }
            inner.on = on;
            inner.status = if on { Status::Connecting } else { Status::Off };
            inner.message = None;
            if on {
                inner.heard.clear();
                inner.said.clear();
                inner.detected = None;
            }
        }
        self.active.store(on, Ordering::Release);
        self.publish(app);
    }

    fn status(&self, app: &AppHandle, status: Status, message: Option<String>) {
        {
            let mut inner = self.inner.lock().unwrap();
            if !inner.on && status != Status::Off {
                return;
            }
            if inner.status == status && inner.message == message {
                return;
            }
            inner.status = status;
            inner.message = message;
        }
        self.publish(app);
    }

    /// Add caption text; sends at most 8 updates per second.
    fn caption(
        &self,
        app: &AppHandle,
        heard: Option<&str>,
        said: Option<&str>,
        language: Option<String>,
    ) {
        let send = {
            let mut inner = self.inner.lock().unwrap();
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
            if let Some(add) = heard {
                roll(&mut inner.heard, add);
            }
            if let Some(add) = said {
                roll(&mut inner.said, add);
            }
            if language.is_some() {
                inner.detected = language;
            }
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

// ---------------------------------------------------------------- Live session worker

/// The audio side of translation, used in the live session's output callback. While
/// translation is on, the clean voice goes to the translator and the translated voice
/// replaces it. Lock-free and allocation-free: two queues and a flag.
pub struct Tap {
    active: Arc<AtomicBool>,
    clean: Producer<f32>,
    translated: Consumer<f32>,
    /// The output device's rate, set once the session knows it.
    pub rate: Arc<AtomicU32>,
}

impl Tap {
    pub fn apply(&mut self, block: &mut [f32]) {
        if !self.active.load(Ordering::Relaxed) {
            // Let an old translation drain away, a little per callback.
            for _ in 0..block.len() * 4 {
                if self.translated.pop().is_err() {
                    break;
                }
            }
            return;
        }
        for sample in block.iter() {
            let _ = self.clean.push(*sample);
        }
        for sample in block.iter_mut() {
            *sample = self.translated.pop().unwrap_or(0.0);
        }
    }
}

/// The translator's side of a [`Tap`].
pub struct Link {
    clean: Consumer<f32>,
    translated: Producer<f32>,
    rate: Arc<AtomicU32>,
}

/// A connected tap and link for one live session.
pub fn tap(translator: &Translator) -> (Tap, Link) {
    // Two seconds of voice to send, and half a minute of translation to play.
    let (clean_in, clean_out) = rtrb::RingBuffer::new(96_000 * 2);
    let (translated_in, translated_out) = rtrb::RingBuffer::new(96_000 * 30);
    let rate = Arc::new(AtomicU32::new(0));
    (
        Tap {
            active: translator.active.clone(),
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

/// Runs for as long as the live session; connects to Gemini only while translation is on.
pub fn run(app: AppHandle, mut link: Link, stop: Arc<AtomicBool>) {
    let translator = app.state::<Arc<Translator>>().inner().clone();
    let mut session: Option<(Session, String)> = None;
    let mut resume: Option<String> = None;
    let mut retry_at = Instant::now();
    let mut failures = 0u32;
    let mut pending: Vec<f32> = Vec::with_capacity(CHUNK * 2);
    // The session sets the output rate once its devices are open.
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
        if !translator.active.load(Ordering::Acquire) {
            if session.take().is_some() {
                resume = None;
            }
            while link.clean.pop().is_ok() {}
            pending.clear();
            thread::sleep(Duration::from_millis(30));
            continue;
        }
        let target = app.state::<crate::AppState>().settings().translate_to;
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
            translator.status(&app, Status::Connecting, None);
            let Some(key) = translator.key() else {
                translator.set_on(&app, false);
                continue;
            };
            match Session::open(&key, &target, resume.as_deref()) {
                Ok(opened) => {
                    failures = 0;
                    session = Some((opened, target.clone()));
                    translator.status(&app, Status::On, None);
                }
                Err(error) => {
                    failures += 1;
                    resume = None;
                    let wait = [2, 4, 8, 15][(failures as usize - 1).min(3)];
                    retry_at = Instant::now() + Duration::from_secs(wait);
                    log::warn!("Translate: {error:#}");
                    translator.status(&app, Status::Error, Some(format!("{error}")));
                    continue;
                }
            }
        }
        let Some((live, _)) = session.as_mut() else {
            continue;
        };
        let mut broken = None;
        // Clean voice in, 100 ms at a time.
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
            log::info!("Translate: connection ended ({error}); reconnecting");
            session = None;
            retry_at = Instant::now();
        }
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
