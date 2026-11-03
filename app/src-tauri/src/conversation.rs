//! Conversations: what you said and what they said, both translated, kept on this computer.
//!
//! Lines are grouped into threads. A thread is one meeting from the calendar, or one stretch
//! of talk: a pause of [`GAP`] or a different meeting starts a new thread, so back-to-back
//! meetings stay apart while a call that runs over stays together.
use eyre::Result;
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};

/// A pause this long starts a new conversation.
const GAP: i64 = 10 * 60_000;
/// Kept on disk: the newest conversations and lines.
const MAX_THREADS: usize = 500;
const MAX_LINES: usize = 40_000;
/// Saved at most this often while lines arrive.
const SAVE_EVERY: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    /// You spoke; they heard the translation.
    Me,
    /// They spoke; you read (and maybe heard) the translation.
    Them,
}

/// One phrase.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    /// Unix time in milliseconds when it started.
    pub at: i64,
    pub side: Side,
    /// The language spoken and the language it was translated into (BCP-47).
    pub from: String,
    pub to: String,
    pub original: String,
    pub translated: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Thread {
    /// Unix time in milliseconds of the first line; also its id.
    pub id: i64,
    pub start: i64,
    pub end: i64,
    /// A name the user gave it.
    #[serde(default)]
    pub title: Option<String>,
    /// The calendar meeting it happened in.
    #[serde(default)]
    pub meeting: Option<String>,
    #[serde(default)]
    pub meeting_title: Option<String>,
    /// "meet", "zoom", … from the meeting's call link.
    #[serde(default)]
    pub app: Option<String>,
    #[serde(default)]
    pub starred: bool,
    pub lines: Vec<Line>,
}

/// A thread without its lines, for the list.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub id: i64,
    pub start: i64,
    pub end: i64,
    pub title: Option<String>,
    pub meeting: Option<String>,
    pub meeting_title: Option<String>,
    pub app: Option<String>,
    pub starred: bool,
    pub lines: usize,
    /// Lines by you and by them.
    pub mine: usize,
    pub theirs: usize,
    /// Every language spoken or translated into.
    pub languages: Vec<String>,
    /// The first words, to recognise it.
    pub preview: String,
}

impl Thread {
    fn summary(&self) -> Summary {
        let mut languages: Vec<String> = Vec::new();
        for line in &self.lines {
            for code in [&line.from, &line.to] {
                if !code.is_empty() && !languages.contains(code) {
                    languages.push(code.clone());
                }
            }
        }
        let preview = self
            .lines
            .iter()
            .map(|l| {
                if l.side == Side::Them && !l.translated.is_empty() {
                    &l.translated
                } else {
                    &l.original
                }
            })
            .find(|t| !t.is_empty())
            .map(|t| t.chars().take(120).collect())
            .unwrap_or_default();
        Summary {
            id: self.id,
            start: self.start,
            end: self.end,
            title: self.title.clone(),
            meeting: self.meeting.clone(),
            meeting_title: self.meeting_title.clone(),
            app: self.app.clone(),
            starred: self.starred,
            lines: self.lines.len(),
            mine: self.lines.iter().filter(|l| l.side == Side::Me).count(),
            theirs: self.lines.iter().filter(|l| l.side == Side::Them).count(),
            languages,
            preview,
        }
    }
}

/// Sent when a line is added.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Added<'a> {
    thread: Summary,
    line: &'a Line,
}

/// The meeting a new line happens in, if any.
pub struct Context {
    pub id: String,
    pub title: String,
    pub app: Option<String>,
}

/// Where a new line goes: the newest thread, or a new one.
fn belongs(thread: &Thread, at: i64, meeting: Option<&Context>) -> bool {
    if at - thread.end > GAP {
        return false;
    }
    match (meeting, &thread.meeting) {
        // A different meeting started: back-to-back calls stay apart.
        (Some(now), Some(then)) => now.id == *then,
        _ => true,
    }
}

/// Add a line to the threads (newest last); returns the thread's index.
fn place(threads: &mut Vec<Thread>, line: Line, meeting: Option<&Context>) -> usize {
    let fits = threads
        .last()
        .is_some_and(|thread| belongs(thread, line.at, meeting));
    if !fits {
        threads.push(Thread {
            id: line.at,
            start: line.at,
            end: line.at,
            title: None,
            meeting: None,
            meeting_title: None,
            app: None,
            starred: false,
            lines: Vec::new(),
        });
    }
    let index = threads.len() - 1;
    let thread = &mut threads[index];
    if let (Some(meeting), None) = (meeting, &thread.meeting) {
        thread.meeting = Some(meeting.id.clone());
        thread.meeting_title = Some(meeting.title.clone());
        thread.app = meeting.app.clone();
    }
    thread.end = thread.end.max(line.at);
    thread.lines.push(line);
    index
}

/// Keep the newest threads and lines; starred threads stay longest.
fn trim(threads: &mut Vec<Thread>) {
    let mut lines: usize = threads.iter().map(|t| t.lines.len()).sum();
    while threads.len() > MAX_THREADS || (lines > MAX_LINES && threads.len() > 1) {
        let oldest = threads.iter().position(|t| !t.starred).unwrap_or(0);
        lines -= threads[oldest].lines.len();
        threads.remove(oldest);
    }
}

struct Inner {
    /// Oldest first; `None` until first read.
    threads: Option<Vec<Thread>>,
    dirty: bool,
    saved: Instant,
}

pub struct Conversations {
    inner: Mutex<Inner>,
    path: PathBuf,
    /// The history of Hush AI 1.1, moved into conversations on first read.
    legacy: PathBuf,
}

/// A 1.1 history entry: what you said and what they heard.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyEntry {
    at: i64,
    from: String,
    to: String,
    heard: String,
    said: String,
}

impl Conversations {
    pub fn new(app: &AppHandle) -> Self {
        let dir = app.path().app_data_dir().unwrap_or_default();
        Self {
            inner: Mutex::new(Inner {
                threads: None,
                dirty: false,
                saved: Instant::now(),
            }),
            path: dir.join("conversations.json"),
            legacy: dir.join("translate-history.json"),
        }
    }

    fn loaded<'a>(&self, inner: &'a mut Inner) -> &'a mut Vec<Thread> {
        if inner.threads.is_none() {
            let threads = match std::fs::read(&self.path) {
                Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
                Err(_) => {
                    let migrated = self.migrate();
                    if !migrated.is_empty() {
                        inner.dirty = true;
                    }
                    migrated
                }
            };
            inner.threads = Some(threads);
        }
        inner.threads.as_mut().expect("loaded")
    }

    fn migrate(&self) -> Vec<Thread> {
        let Ok(bytes) = std::fs::read(&self.legacy) else {
            return Vec::new();
        };
        let entries: Vec<LegacyEntry> = serde_json::from_slice(&bytes).unwrap_or_default();
        let mut threads = Vec::new();
        for entry in entries {
            place(
                &mut threads,
                Line {
                    at: entry.at,
                    side: Side::Me,
                    from: entry.from,
                    to: entry.to,
                    original: entry.heard,
                    translated: entry.said,
                },
                None,
            );
        }
        threads
    }

    /// Add a finished phrase. The current calendar meeting names its thread.
    pub fn add(&self, app: &AppHandle, line: Line) {
        if line.original.is_empty() && line.translated.is_empty() {
            return;
        }
        let meeting = app
            .state::<std::sync::Arc<crate::calendar::Calendar>>()
            .current()
            .map(|m| Context {
                id: m.id,
                title: m.title,
                app: m.app.map(str::to_string),
            });
        let summary = {
            let mut inner = self.inner.lock().unwrap();
            let threads = self.loaded(&mut inner);
            let index = place(threads, line.clone(), meeting.as_ref());
            let summary = threads[index].summary();
            trim(threads);
            inner.dirty = true;
            summary
        };
        let _ = app.emit(
            "conversation-line",
            Added {
                thread: summary,
                line: &line,
            },
        );
        self.flush(false);
    }

    /// Write to disk if something changed, at most every few seconds unless `now`.
    pub fn flush(&self, now: bool) {
        let snapshot = {
            let mut inner = self.inner.lock().unwrap();
            if !inner.dirty || (!now && inner.saved.elapsed() < SAVE_EVERY) {
                return;
            }
            inner.dirty = false;
            inner.saved = Instant::now();
            inner.threads.clone().unwrap_or_default()
        };
        if let Err(error) = self.save(&snapshot) {
            log::warn!("Conversations: {error:#}");
        }
    }

    fn save(&self, threads: &[Thread]) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let temporary = self.path.with_extension("json.tmp");
        std::fs::write(&temporary, serde_json::to_vec(threads)?)?;
        std::fs::rename(&temporary, &self.path)?;
        Ok(())
    }

    /// Every conversation, newest first, without lines.
    pub fn list(&self) -> Vec<Summary> {
        let mut inner = self.inner.lock().unwrap();
        self.loaded(&mut inner)
            .iter()
            .rev()
            .map(Thread::summary)
            .collect()
    }

    pub fn get(&self, id: i64) -> Option<Thread> {
        let mut inner = self.inner.lock().unwrap();
        self.loaded(&mut inner).iter().find(|t| t.id == id).cloned()
    }

    /// Change one conversation; returns its new summary.
    fn change(&self, id: i64, edit: impl FnOnce(&mut Thread)) -> Option<Summary> {
        let summary = {
            let mut inner = self.inner.lock().unwrap();
            let thread = self.loaded(&mut inner).iter_mut().find(|t| t.id == id)?;
            edit(thread);
            let summary = thread.summary();
            inner.dirty = true;
            summary
        };
        self.flush(true);
        Some(summary)
    }

    pub fn star(&self, id: i64, starred: bool) -> Option<Summary> {
        self.change(id, |t| t.starred = starred)
    }

    pub fn rename(&self, id: i64, title: Option<String>) -> Option<Summary> {
        let title = title
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());
        self.change(id, |t| t.title = title)
    }

    pub fn delete(&self, id: i64) {
        {
            let mut inner = self.inner.lock().unwrap();
            self.loaded(&mut inner).retain(|t| t.id != id);
            inner.dirty = true;
        }
        self.flush(true);
    }

    pub fn clear(&self) {
        {
            let mut inner = self.inner.lock().unwrap();
            inner.threads = Some(Vec::new());
            inner.dirty = false;
        }
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(&self.legacy);
    }

    /// Write conversations to a file: "md", "txt" or "json".
    pub fn export(&self, ids: &[i64], format: &str, path: &std::path::Path) -> Result<()> {
        let threads: Vec<Thread> = {
            let mut inner = self.inner.lock().unwrap();
            self.loaded(&mut inner)
                .iter()
                .filter(|t| ids.is_empty() || ids.contains(&t.id))
                .cloned()
                .collect()
        };
        let text = match format {
            "json" => serde_json::to_string_pretty(&threads)?,
            "txt" => render(&threads, false),
            _ => render(&threads, true),
        };
        std::fs::write(path, text)?;
        Ok(())
    }
}

fn clock(ms: i64, format: &str) -> String {
    use chrono::TimeZone;
    chrono::Local
        .timestamp_millis_opt(ms)
        .single()
        .map(|t| t.format(format).to_string())
        .unwrap_or_default()
}

/// The conversations as Markdown or plain text, both languages of every line.
fn render(threads: &[Thread], markdown: bool) -> String {
    let mut out = String::new();
    for thread in threads {
        let title = thread
            .title
            .clone()
            .or_else(|| thread.meeting_title.clone())
            .unwrap_or_else(|| format!("Conversation, {}", clock(thread.start, "%-d %B %Y")));
        let when = format!(
            "{} – {}",
            clock(thread.start, "%a %-d %b %Y, %H:%M"),
            clock(thread.end, "%H:%M")
        );
        if markdown {
            out.push_str(&format!("## {title}\n\n_{when}_\n\n"));
        } else {
            out.push_str(&format!("{title}\n{when}\n\n"));
        }
        for line in &thread.lines {
            let who = match line.side {
                Side::Me => "You",
                Side::Them => "Them",
            };
            let time = clock(line.at, "%H:%M:%S");
            if markdown {
                out.push_str(&format!("**{who}** · {time} · {}\n", line.from));
                out.push_str(&format!("> {}\n", line.original.trim()));
                if !line.translated.trim().is_empty() {
                    out.push_str(&format!(">\n> _{}: {}_\n", line.to, line.translated.trim()));
                }
                out.push('\n');
            } else {
                out.push_str(&format!(
                    "[{time}] {who} ({}): {}\n",
                    line.from,
                    line.original.trim()
                ));
                if !line.translated.trim().is_empty() {
                    out.push_str(&format!(
                        "           ({}): {}\n",
                        line.to,
                        line.translated.trim()
                    ));
                }
            }
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(at: i64, side: Side) -> Line {
        Line {
            at,
            side,
            from: "en".into(),
            to: "es".into(),
            original: format!("line at {at}"),
            translated: "traducido".into(),
        }
    }

    fn meeting(id: &str) -> Context {
        Context {
            id: id.into(),
            title: format!("Meeting {id}"),
            app: Some("zoom".into()),
        }
    }

    #[test]
    fn a_pause_starts_a_new_conversation() {
        let mut threads = Vec::new();
        place(&mut threads, line(0, Side::Me), None);
        place(&mut threads, line(60_000, Side::Them), None);
        place(&mut threads, line(60_000 + GAP + 1, Side::Me), None);
        assert_eq!(threads.len(), 2);
        assert_eq!(threads[0].lines.len(), 2);
        assert_eq!(threads[0].end, 60_000);
    }

    #[test]
    fn back_to_back_meetings_stay_apart() {
        let mut threads = Vec::new();
        let a = meeting("a");
        let b = meeting("b");
        place(&mut threads, line(0, Side::Me), Some(&a));
        place(&mut threads, line(30_000, Side::Them), Some(&a));
        place(&mut threads, line(60_000, Side::Me), Some(&b));
        assert_eq!(threads.len(), 2);
        assert_eq!(threads[0].meeting_title.as_deref(), Some("Meeting a"));
        assert_eq!(threads[1].meeting.as_deref(), Some("b"));
        assert_eq!(threads[1].app.as_deref(), Some("zoom"));
    }

    #[test]
    fn talk_before_a_meeting_joins_it() {
        let mut threads = Vec::new();
        place(&mut threads, line(0, Side::Me), None);
        place(&mut threads, line(60_000, Side::Me), Some(&meeting("a")));
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].meeting.as_deref(), Some("a"));
    }

    #[test]
    fn trimming_keeps_starred_conversations() {
        let mut threads = Vec::new();
        for i in 0..(MAX_THREADS as i64 + 3) {
            place(&mut threads, line(i * (GAP + 1), Side::Me), None);
        }
        threads[0].starred = true;
        trim(&mut threads);
        assert_eq!(threads.len(), MAX_THREADS);
        assert!(threads[0].starred);
        assert_eq!(threads[1].id, 4 * (GAP + 1));
    }

    #[test]
    fn exports_both_languages() {
        let mut threads = Vec::new();
        place(&mut threads, line(0, Side::Me), Some(&meeting("a")));
        let text = render(&threads, true);
        assert!(text.contains("## Meeting a"));
        assert!(text.contains("**You**"));
        assert!(text.contains("traducido"));
        let plain = render(&threads, false);
        assert!(plain.contains("You (en): line at 0"));
    }
}
