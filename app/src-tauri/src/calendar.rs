//! Meetings from a calendar's secret iCal address (Google Calendar: Settings › your calendar ›
//! Integrate calendar › Secret address in iCal format). No sign-in: Hush reads the address
//! directly from the calendar service on a schedule the user picks. The address is stored
//! only on this computer and never logged.
//!
//! Hush shows the next meetings with a Join button. Before a meeting with a call link it can
//! pop up the tray widget, turn noise cancellation on, and turn it off again afterwards if
//! Hush turned it on.
use chrono::{DateTime, Duration as Span, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use eyre::{eyre, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};

/// How often the start of a meeting is checked.
const TICK: Duration = Duration::from_secs(10);
/// The pop-up shows at least this long before a meeting.
const POPUP_LEAD: i64 = 60;
/// Meetings shown: from 12 hours ago to a week ahead.
const PAST: i64 = 12 * 3600;
const AHEAD: i64 = 8 * 24 * 3600;
/// Largest calendar file read.
const MAX_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Off,
    Loading,
    Ready,
    Error,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meeting {
    pub id: String,
    pub title: String,
    /// Unix time in milliseconds.
    pub start: i64,
    pub end: i64,
    pub all_day: bool,
    /// "meet", "zoom", "teams", "webex", "slack", "discord" or "other" when there is a call link.
    pub app: Option<&'static str>,
    #[serde(skip)]
    link: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarInfo {
    pub connected: bool,
    pub status: Status,
    pub error: Option<String>,
    /// Unix time in milliseconds of the last successful read.
    pub updated: Option<i64>,
    /// "Google Calendar" or the address's host.
    pub source: Option<String>,
    /// The calendar's own name, when it has one.
    pub name: Option<String>,
    pub meetings: Vec<Meeting>,
}

/// What an address holds, shown before it is connected.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub source: String,
    pub name: Option<String>,
    /// Timed events in the next seven days, and how many have a call link.
    pub meetings: usize,
    pub calls: usize,
    pub next: Option<Meeting>,
}

/// Sent to the windows shortly before a meeting with a call link.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Soon {
    pub meeting: Meeting,
    /// Hush turned noise cancellation on for it.
    pub turned_on: bool,
}

#[derive(Default, Serialize, Deserialize)]
struct Stored {
    url: Option<String>,
}

#[derive(Default)]
struct Inner {
    url: Option<String>,
    name: Option<String>,
    status: Option<Status>,
    error: Option<String>,
    updated: Option<i64>,
    meetings: Vec<Meeting>,
    fetched: Option<Instant>,
    refresh: bool,
    /// Meetings whose start was already handled, so Hush acts once per meeting.
    handled: HashSet<String>,
    /// Meetings already announced with the pop-up.
    announced: HashSet<String>,
    /// Hush turned itself on for a meeting that ends at this time (ms).
    started: Option<i64>,
}

pub struct Calendar {
    inner: Mutex<Inner>,
    wake: Condvar,
    path: PathBuf,
}

impl Calendar {
    pub fn new(app: &AppHandle) -> Arc<Self> {
        let path = app
            .path()
            .app_config_dir()
            .unwrap_or_default()
            .join("calendar.json");
        let stored: Stored = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let calendar = Arc::new(Self {
            inner: Mutex::new(Inner {
                url: stored.url,
                refresh: true,
                ..Default::default()
            }),
            wake: Condvar::new(),
            path,
        });
        let app = app.clone();
        let worker = calendar.clone();
        thread::Builder::new()
            .name("hush-calendar".into())
            .spawn(move || worker.run(&app))
            .expect("calendar thread");
        calendar
    }

    pub fn info(&self) -> CalendarInfo {
        let inner = self.inner.lock().unwrap();
        let now = Utc::now().timestamp_millis();
        CalendarInfo {
            connected: inner.url.is_some(),
            status: if inner.url.is_none() {
                Status::Off
            } else {
                inner.status.clone().unwrap_or(Status::Loading)
            },
            error: inner.error.clone(),
            updated: inner.updated,
            source: inner.url.as_deref().map(source_name),
            name: inner.name.clone(),
            meetings: inner
                .meetings
                .iter()
                .filter(|m| m.end > now)
                .take(40)
                .cloned()
                .collect(),
        }
    }

    fn publish(&self, app: &AppHandle) {
        let _ = app.emit("calendar", self.info());
    }

    /// Read an address without keeping it, to show what it holds.
    pub fn check(address: &str) -> Result<Preview> {
        let url = normalize(address)?;
        let (name, events) =
            fetch(&url).map_err(|e| eyre!(e.to_string().replace(&url, "the address")))?;
        let now = Utc::now();
        let week = (now + Span::days(7)).timestamp_millis();
        let upcoming: Vec<Meeting> = expand(&events, now)
            .into_iter()
            .filter(|m| !m.all_day && m.end > now.timestamp_millis() && m.start < week)
            .collect();
        Ok(Preview {
            source: source_name(&url),
            name,
            meetings: upcoming.len(),
            calls: upcoming.iter().filter(|m| m.link.is_some()).count(),
            next: upcoming
                .into_iter()
                .find(|m| m.start > now.timestamp_millis()),
        })
    }

    /// Check and save a new address. The calendar is read once before it is kept.
    pub fn connect(&self, app: &AppHandle, address: &str) -> Result<CalendarInfo> {
        let url = normalize(address)?;
        let (name, events) =
            fetch(&url).map_err(|e| eyre!(e.to_string().replace(&url, "the address")))?;
        let meetings = expand(&events, Utc::now());
        std::fs::create_dir_all(self.path.parent().unwrap_or(&self.path))?;
        let temporary = self.path.with_extension("json.tmp");
        std::fs::write(
            &temporary,
            serde_json::to_vec(&Stored {
                url: Some(url.clone()),
            })?,
        )?;
        std::fs::rename(&temporary, &self.path)?;
        {
            let mut inner = self.inner.lock().unwrap();
            inner.url = Some(url);
            inner.name = name;
            inner.meetings = meetings;
            inner.status = Some(Status::Ready);
            inner.error = None;
            inner.updated = Some(Utc::now().timestamp_millis());
            inner.fetched = Some(Instant::now());
            // Just read: the next read waits for the sync interval.
            inner.refresh = false;
        }
        self.publish(app);
        Ok(self.info())
    }

    pub fn disconnect(&self, app: &AppHandle) -> CalendarInfo {
        let _ = std::fs::remove_file(&self.path);
        {
            let mut inner = self.inner.lock().unwrap();
            *inner = Inner::default();
        }
        self.publish(app);
        self.info()
    }

    /// Read the calendar again now.
    pub fn refresh(&self) {
        self.inner.lock().unwrap().refresh = true;
        self.wake.notify_all();
    }

    /// The call link of a meeting, to open it.
    pub fn link(&self, id: &str) -> Option<String> {
        let inner = self.inner.lock().unwrap();
        inner
            .meetings
            .iter()
            .find(|m| m.id == id)
            .and_then(|m| m.link.clone())
    }

    fn run(&self, app: &AppHandle) {
        loop {
            let every = Duration::from_secs(
                u64::from(
                    app.state::<crate::AppState>()
                        .settings()
                        .calendar_sync
                        .clamp(1, 240),
                ) * 60,
            );
            let due = {
                let inner = self.inner.lock().unwrap();
                let inner = self
                    .wake
                    .wait_timeout_while(inner, TICK, |i| !i.refresh)
                    .unwrap()
                    .0;
                inner.url.clone().filter(|_| {
                    inner.refresh || inner.fetched.is_none_or(|at| at.elapsed() >= every)
                })
            };
            if let Some(url) = due {
                {
                    let mut inner = self.inner.lock().unwrap();
                    inner.refresh = false;
                    inner.status = Some(Status::Loading);
                }
                self.publish(app);
                let result = fetch(&url).map(|(name, events)| (name, expand(&events, Utc::now())));
                let mut inner = self.inner.lock().unwrap();
                // Disconnected or replaced while reading: drop the result.
                if inner.url.as_deref() == Some(&url) {
                    inner.fetched = Some(Instant::now());
                    match result {
                        Ok((name, meetings)) => {
                            inner.name = name;
                            inner.meetings = meetings;
                            inner.status = Some(Status::Ready);
                            inner.error = None;
                            inner.updated = Some(Utc::now().timestamp_millis());
                        }
                        Err(error) => {
                            let text = error.to_string().replace(&url, "the address");
                            log::warn!("Calendar: {text}");
                            inner.status = Some(Status::Error);
                            inner.error = Some(text);
                        }
                    }
                    drop(inner);
                    self.publish(app);
                }
            }
            self.automatic(app);
        }
    }

    /// Before a meeting with a call link: pop up the widget and turn noise cancellation on.
    /// After it: turn it off again, if Hush turned it on.
    fn automatic(&self, app: &AppHandle) {
        let settings = app.state::<crate::AppState>().settings();
        let now = Utc::now().timestamp_millis();
        let lead = i64::from(settings.calendar_lead.clamp(1, 15)) * 60_000;
        let mut turn = None;
        let mut announce = None;
        {
            let mut inner = self.inner.lock().unwrap();
            if inner.url.is_none() || !settings.onboarded {
                inner.started = None;
                return;
            }
            if inner.started.is_some() && !settings.enabled {
                // Turned off by hand: leave it off.
                inner.started = None;
            }
            let calls = || {
                inner
                    .meetings
                    .iter()
                    .filter(|m| m.link.is_some() && !m.all_day && now < m.end)
            };
            let current = calls()
                .find(|m| m.start - lead <= now)
                .map(|m| (m.id.clone(), m.end));
            let soon = calls()
                .find(|m| {
                    m.start - lead.max(POPUP_LEAD * 1000) <= now && now < m.start + 5 * 60_000
                })
                .cloned();
            if let Some((id, end)) = current {
                if settings.calendar_auto && inner.handled.insert(id) && !settings.enabled {
                    inner.started = Some(end);
                    turn = Some(true);
                } else if let Some(until) = inner.started.as_mut() {
                    // Back-to-back meetings keep it on.
                    *until = (*until).max(end);
                }
            } else if let Some(end) = inner.started {
                if now >= end {
                    inner.started = None;
                    if settings.enabled && settings.calendar_auto_off {
                        turn = Some(false);
                    }
                }
            }
            if let Some(meeting) = soon {
                if settings.calendar_popup && inner.announced.insert(meeting.id.clone()) {
                    announce = Some(meeting);
                }
            }
            if inner.handled.len() > 500 {
                inner.handled.clear();
            }
            if inner.announced.len() > 500 {
                inner.announced.clear();
            }
        }
        if let Some(on) = turn {
            log::info!(
                "Calendar: turning noise cancellation {}",
                if on { "on" } else { "off" }
            );
            crate::set_enabled(app, on);
        }
        if let Some(meeting) = announce {
            let _ = app.emit(
                "meeting-soon",
                Soon {
                    meeting,
                    turned_on: turn == Some(true),
                },
            );
            crate::tray::show_widget(app);
        }
    }
}

/// `webcal://` and plain addresses become `https://`. Only HTTPS is read.
fn normalize(address: &str) -> Result<String> {
    let trimmed = address.trim();
    let url = if let Some(rest) = trimmed.strip_prefix("webcal://") {
        format!("https://{rest}")
    } else {
        trimmed.to_string()
    };
    if !url.starts_with("https://") || url.contains(char::is_whitespace) {
        return Err(eyre!(
            "Paste the address that starts with https://calendar.google.com/calendar/ical/"
        ));
    }
    if source_name(&url) == "Google Calendar" && !url.contains("/calendar/ical/") {
        return Err(eyre!(
            "This is a Google Calendar page, not its iCal address. Copy “Secret address in iCal format” instead."
        ));
    }
    Ok(url)
}

fn source_name(url: &str) -> String {
    let host = url
        .trim_start_matches("https://")
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .to_lowercase();
    match host.as_str() {
        "calendar.google.com" | "www.google.com" => "Google Calendar".into(),
        "outlook.office365.com" | "outlook.live.com" => "Outlook".into(),
        _ if host.ends_with("icloud.com") => "iCloud Calendar".into(),
        _ => host,
    }
}

fn fetch(url: &str) -> Result<(Option<String>, Vec<Event>)> {
    let mut response = ureq::get(url)
        .header("User-Agent", concat!("HushAI/", env!("CARGO_PKG_VERSION")))
        .config()
        .timeout_global(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .build()
        .call()
        .map_err(|e| eyre!("Could not reach the calendar: {e}"))?;
    let status = response.status().as_u16();
    if status == 404 || status == 401 || status == 403 {
        return Err(eyre!(
            "The calendar did not accept this address. Copy the secret address again; it changes if it was reset."
        ));
    }
    if status >= 400 {
        return Err(eyre!(
            "The calendar service answered with error {status}. Hush will try again."
        ));
    }
    let text = response
        .body_mut()
        .with_config()
        .limit(MAX_BYTES)
        .read_to_string()
        .map_err(|e| eyre!("Could not read the calendar: {e}"))?;
    if !text.trim_start().starts_with("BEGIN:VCALENDAR") {
        return Err(eyre!(
            "This address is not an iCal calendar. Use “Secret address in iCal format”."
        ));
    }
    Ok((calendar_name(&text), parse(&text)))
}

/// `X-WR-CALNAME`, the name Google and others give the calendar.
fn calendar_name(text: &str) -> Option<String> {
    unfold(text)
        .into_iter()
        .take_while(|line| !line.starts_with("BEGIN:VEVENT"))
        .filter_map(|line| property(&line))
        .find(|(name, _, _)| name == "X-WR-CALNAME")
        .map(|(_, _, value)| unescape(&value).trim().to_string())
        .filter(|name| !name.is_empty())
}

// ---------------------------------------------------------------- iCalendar (RFC 5545)

#[derive(Clone, Debug, PartialEq)]
enum Time {
    Utc(NaiveDateTime),
    Zoned(NaiveDateTime, chrono_tz::Tz),
    /// No time zone: the computer's.
    Floating(NaiveDateTime),
    Date(NaiveDate),
}

impl Time {
    fn parse(value: &str, params: &[(String, String)]) -> Option<Self> {
        let value = value.trim();
        let date_only = params
            .iter()
            .any(|(k, v)| k == "VALUE" && v.eq_ignore_ascii_case("DATE"))
            || value.len() == 8;
        if date_only {
            return NaiveDate::parse_from_str(&value[..8.min(value.len())], "%Y%m%d")
                .ok()
                .map(Self::Date);
        }
        if let Some(utc) = value.strip_suffix('Z') {
            return NaiveDateTime::parse_from_str(utc, "%Y%m%dT%H%M%S")
                .ok()
                .map(Self::Utc);
        }
        let naive = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S").ok()?;
        let zone = params
            .iter()
            .find(|(k, _)| k == "TZID")
            .and_then(|(_, v)| v.trim_matches('"').parse::<chrono_tz::Tz>().ok());
        Some(match zone {
            Some(zone) => Self::Zoned(naive, zone),
            None => Self::Floating(naive),
        })
    }

    fn instant(&self) -> Option<DateTime<Utc>> {
        match self {
            Self::Utc(naive) => Some(Utc.from_utc_datetime(naive)),
            Self::Zoned(naive, zone) => zone
                .from_local_datetime(naive)
                .earliest()
                .map(|t| t.with_timezone(&Utc)),
            Self::Floating(naive) => Local
                .from_local_datetime(naive)
                .earliest()
                .map(|t| t.with_timezone(&Utc)),
            Self::Date(date) => Local
                .from_local_datetime(&date.and_hms_opt(0, 0, 0)?)
                .earliest()
                .map(|t| t.with_timezone(&Utc)),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Event {
    uid: String,
    title: String,
    start: Option<Time>,
    /// The DTSTART line as written, for the recurrence rule parser.
    start_line: String,
    end: Option<Time>,
    duration: Option<Span>,
    rrule: Option<String>,
    exdates: Vec<String>,
    rdates: Vec<String>,
    recurrence_id: Option<Time>,
    cancelled: bool,
    link: Option<String>,
}

/// Lines with RFC 5545 folding undone.
fn unfold(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if let Some(rest) = line.strip_prefix([' ', '\t']) {
            if let Some(last) = lines.last_mut() {
                last.push_str(rest);
                continue;
            }
        }
        lines.push(line.to_string());
    }
    lines
}

/// A property's name, its parameters and its value.
type Property = (String, Vec<(String, String)>, String);

/// `NAME;PARAM=V;PARAM="V":value` into its parts.
fn property(line: &str) -> Option<Property> {
    let mut quoted = false;
    let mut split = None;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ':' if !quoted => {
                split = Some(i);
                break;
            }
            _ => {}
        }
    }
    let split = split?;
    let (head, value) = (&line[..split], &line[split + 1..]);
    let mut parts = Vec::new();
    let mut current = String::new();
    quoted = false;
    for c in head.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                current.push(c);
            }
            ';' if !quoted => parts.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    parts.push(current);
    let name = parts.remove(0).to_ascii_uppercase();
    let params = parts
        .into_iter()
        .filter_map(|p| {
            let (k, v) = p.split_once('=')?;
            Some((k.to_ascii_uppercase(), v.trim_matches('"').to_string()))
        })
        .collect();
    Some((name, params, value.to_string()))
}

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n' | 'N') => out.push('\n'),
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// "PT1H30M", "P1D" and the like.
fn parse_duration(value: &str) -> Option<Span> {
    let (negative, value) = match value.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, value.strip_prefix('+').unwrap_or(value)),
    };
    let value = value.strip_prefix('P')?;
    let mut seconds = 0i64;
    let mut number = String::new();
    let mut time = false;
    for c in value.chars() {
        match c {
            'T' => time = true,
            '0'..='9' => number.push(c),
            unit => {
                let n: i64 = number.parse().ok()?;
                number.clear();
                seconds += n * match (unit, time) {
                    ('W', _) => 7 * 86_400,
                    ('D', _) => 86_400,
                    ('H', true) => 3600,
                    ('M', true) => 60,
                    ('S', true) => 1,
                    _ => return None,
                };
            }
        }
    }
    Some(Span::seconds(if negative { -seconds } else { seconds }))
}

fn parse(text: &str) -> Vec<Event> {
    let mut events = Vec::new();
    let mut event: Option<Event> = None;
    // Nested components inside an event (alarms) are skipped.
    let mut nested = 0usize;
    let mut description = String::new();
    let mut location = String::new();
    let mut conference = String::new();
    let mut url = String::new();
    for line in unfold(text) {
        let Some((name, params, value)) = property(&line) else {
            continue;
        };
        match (name.as_str(), value.to_ascii_uppercase().as_str()) {
            ("BEGIN", "VEVENT") => {
                event = Some(Event::default());
                nested = 0;
                description.clear();
                location.clear();
                conference.clear();
                url.clear();
                continue;
            }
            ("BEGIN", _) if event.is_some() => {
                nested += 1;
                continue;
            }
            ("END", "VEVENT") => {
                if let Some(mut done) = event.take() {
                    done.link = [&conference, &location, &description, &url]
                        .into_iter()
                        .find_map(|text| call_link(text));
                    if done.start.is_some() {
                        events.push(done);
                    }
                }
                continue;
            }
            ("END", _) if event.is_some() && nested > 0 => {
                nested -= 1;
                continue;
            }
            _ => {}
        }
        let Some(current) = event.as_mut().filter(|_| nested == 0) else {
            continue;
        };
        match name.as_str() {
            "UID" => current.uid = value,
            "SUMMARY" => current.title = unescape(&value),
            "DTSTART" => {
                current.start = Time::parse(&value, &params);
                current.start_line = line.clone();
            }
            "DTEND" => current.end = Time::parse(&value, &params),
            "DURATION" => current.duration = parse_duration(&value),
            "RRULE" => current.rrule = Some(value),
            "EXDATE" => current.exdates.push(line.clone()),
            "RDATE" => current.rdates.push(line.clone()),
            "RECURRENCE-ID" => current.recurrence_id = Time::parse(&value, &params),
            "STATUS" => current.cancelled = value.eq_ignore_ascii_case("CANCELLED"),
            "DESCRIPTION" => description = unescape(&value),
            "LOCATION" => location = unescape(&value),
            "X-GOOGLE-CONFERENCE" => conference = value,
            "URL" => url = value,
            _ => {}
        }
    }
    events
}

/// The first video-call link in `text`, if any.
fn call_link(text: &str) -> Option<String> {
    let mut rest = text;
    while let Some(index) = rest.find("https://") {
        let candidate: String = rest[index..]
            .chars()
            .take_while(|c| {
                !c.is_whitespace() && !matches!(c, '"' | '\'' | '<' | '>' | ')' | ']' | '\\' | ',')
            })
            .collect();
        rest = &rest[index + 8..];
        if call_app(&candidate).is_some() {
            return Some(candidate.trim_end_matches(['.', ';']).to_string());
        }
    }
    None
}

fn call_app(link: &str) -> Option<&'static str> {
    let host = link
        .trim_start_matches("https://")
        .split(['/', '?', '#'])
        .next()?
        .to_lowercase();
    let path = link.to_lowercase();
    let is = |domain: &str| host == domain || host.ends_with(&format!(".{domain}"));
    Some(if is("meet.google.com") {
        "meet"
    } else if is("zoom.us") || is("zoomgov.com") {
        "zoom"
    } else if is("teams.microsoft.com") || is("teams.live.com") {
        "teams"
    } else if is("webex.com") {
        "webex"
    } else if is("slack.com") && path.contains("huddle") {
        "slack"
    } else if is("discord.com") || is("discord.gg") {
        "discord"
    } else if is("whereby.com") || is("meet.jit.si") || is("chime.aws") || is("gotomeeting.com") {
        "other"
    } else {
        return None;
    })
}

/// Every occurrence from `PAST` before `now` to `AHEAD` after it, sorted by start.
fn expand(events: &[Event], now: DateTime<Utc>) -> Vec<Meeting> {
    let from = now - Span::seconds(PAST);
    let until = now + Span::seconds(AHEAD);
    // Single occurrences that replace part of a series.
    let moved: HashSet<(String, i64)> = events
        .iter()
        .filter_map(|e| {
            let id = e.recurrence_id.as_ref()?.instant()?.timestamp();
            Some((e.uid.clone(), id))
        })
        .collect();
    let mut meetings = Vec::new();
    for event in events.iter().filter(|e| !e.cancelled) {
        let Some(start) = event.start.as_ref().and_then(Time::instant) else {
            continue;
        };
        let all_day = matches!(event.start, Some(Time::Date(_)));
        let length = match (&event.end, event.duration) {
            (Some(end), _) => end.instant().map(|end| end - start),
            (None, Some(duration)) => Some(duration),
            (None, None) => Some(if all_day { Span::days(1) } else { Span::zero() }),
        }
        .unwrap_or_else(Span::zero)
        .max(Span::zero());
        let mut starts = Vec::new();
        match (&event.rrule, &event.recurrence_id) {
            (Some(rule), None) => match occurrences(event, rule, from - length, until) {
                Some(found) => starts.extend(
                    found
                        .into_iter()
                        .filter(|t| !moved.contains(&(event.uid.clone(), t.timestamp()))),
                ),
                // A rule Hush cannot read: show the first occurrence only.
                None => starts.push(start),
            },
            _ => starts.push(start),
        }
        for begin in starts {
            let end = begin + length;
            if end < from || begin > until {
                continue;
            }
            meetings.push(Meeting {
                id: format!("{}@{}", event.uid, begin.timestamp()),
                title: if event.title.trim().is_empty() {
                    "Busy".into()
                } else {
                    event.title.trim().to_string()
                },
                start: begin.timestamp_millis(),
                end: end.timestamp_millis(),
                all_day,
                app: event.link.as_deref().and_then(call_app),
                link: event.link.clone(),
            });
        }
    }
    meetings.sort_by_key(|m| (m.start, m.all_day));
    meetings.dedup_by(|a, b| a.id == b.id);
    meetings
}

fn occurrences(
    event: &Event,
    rule: &str,
    from: DateTime<Utc>,
    until: DateTime<Utc>,
) -> Option<Vec<DateTime<Utc>>> {
    let mut text = format!("{}\nRRULE:{rule}", event.start_line);
    for line in event.exdates.iter().chain(&event.rdates) {
        text.push('\n');
        text.push_str(line);
    }
    let set: rrule::RRuleSet = text.parse().ok()?;
    let set = set
        .after(from.with_timezone(&rrule::Tz::UTC))
        .before(until.with_timezone(&rrule::Tz::UTC));
    let result = set.all(500);
    Some(
        result
            .dates
            .into_iter()
            .map(|t| t.with_timezone(&Utc))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "BEGIN:VCALENDAR\r
VERSION:2.0\r
BEGIN:VEVENT\r
DTSTART;TZID=Europe/Berlin:20261102T100000\r
DTEND;TZID=Europe/Berlin:20261102T103000\r
RRULE:FREQ=WEEKLY;BYDAY=MO,WE\r
EXDATE;TZID=Europe/Berlin:20261104T100000\r
UID:standup@google.com\r
SUMMARY:Team standup\r
X-GOOGLE-CONFERENCE:https://meet.google.com/abc-defg-hij\r
BEGIN:VALARM\r
ACTION:DISPLAY\r
DESCRIPTION:This is an event reminder\r
END:VALARM\r
END:VEVENT\r
BEGIN:VEVENT\r
DTSTART;TZID=Europe/Berlin:20261109T110000\r
DTEND;TZID=Europe/Berlin:20261109T113000\r
RECURRENCE-ID;TZID=Europe/Berlin:20261109T100000\r
UID:standup@google.com\r
SUMMARY:Team standup (moved)\r
END:VEVENT\r
BEGIN:VEVENT\r
DTSTART:20261103T150000Z\r
DTEND:20261103T160000Z\r
UID:review@google.com\r
SUMMARY:Design review\\, Q4\r
DESCRIPTION:Join Zoom Meeting\\nhttps://us02web.zoom.us/j/8123456789?pwd=abc\r
 def\\nMeeting ID: 812 345 6789\r
END:VEVENT\r
BEGIN:VEVENT\r
DTSTART;VALUE=DATE:20261105\r
DTEND;VALUE=DATE:20261106\r
UID:holiday@google.com\r
SUMMARY:Holiday\r
END:VEVENT\r
BEGIN:VEVENT\r
DTSTART:20261104T090000Z\r
DURATION:PT45M\r
UID:gone@google.com\r
SUMMARY:Cancelled sync\r
STATUS:CANCELLED\r
LOCATION:https://teams.microsoft.com/l/meetup-join/xyz\r
END:VEVENT\r
END:VCALENDAR\r
";

    #[test]
    fn reads_google_style_calendars() {
        let events = parse(SAMPLE);
        assert_eq!(events.len(), 5);
        let review = events
            .iter()
            .find(|e| e.uid == "review@google.com")
            .unwrap();
        assert_eq!(review.title, "Design review, Q4");
        assert_eq!(
            review.link.as_deref(),
            Some("https://us02web.zoom.us/j/8123456789?pwd=abcdef")
        );
        let standup = &events[0];
        assert_eq!(
            standup.link.as_deref(),
            Some("https://meet.google.com/abc-defg-hij")
        );
        assert_eq!(standup.title, "Team standup");
    }

    #[test]
    fn expands_series_with_exceptions_and_moves() {
        let now = Utc.with_ymd_and_hms(2026, 11, 2, 7, 0, 0).unwrap();
        let meetings = expand(&parse(SAMPLE), now);
        let titles: Vec<_> = meetings
            .iter()
            .map(|m| {
                (
                    m.title.as_str(),
                    DateTime::from_timestamp_millis(m.start).unwrap(),
                )
            })
            .collect();
        // Mon 2 Nov 10:00 Berlin = 09:00 UTC; Wed 4 Nov is excluded; Mon 9 Nov moved to 11:00.
        assert!(titles.contains(&(
            "Team standup",
            Utc.with_ymd_and_hms(2026, 11, 2, 9, 0, 0).unwrap()
        )));
        assert!(!titles
            .iter()
            .any(|(_, t)| *t == Utc.with_ymd_and_hms(2026, 11, 4, 9, 0, 0).unwrap()));
        assert!(titles.contains(&(
            "Team standup (moved)",
            Utc.with_ymd_and_hms(2026, 11, 9, 10, 0, 0).unwrap()
        )));
        assert!(!titles
            .iter()
            .any(|(_, t)| *t == Utc.with_ymd_and_hms(2026, 11, 9, 9, 0, 0).unwrap()));
        // The window ends 8 days after `now`: only Mon 2 Nov is left of the series itself.
        assert_eq!(
            titles.iter().filter(|(t, _)| *t == "Team standup").count(),
            1
        );
        assert!(!meetings.iter().any(|m| m.title == "Cancelled sync"));
        let review = meetings
            .iter()
            .find(|m| m.title.starts_with("Design"))
            .unwrap();
        assert_eq!(review.app, Some("zoom"));
        assert_eq!(review.end - review.start, 3_600_000);
        let holiday = meetings.iter().find(|m| m.title == "Holiday").unwrap();
        assert!(holiday.all_day && holiday.app.is_none());
        assert!(meetings.windows(2).all(|w| w[0].start <= w[1].start));
    }

    #[test]
    fn reads_the_calendar_name() {
        let text = SAMPLE.replace(
            "VERSION:2.0\r\n",
            "VERSION:2.0\r\nX-WR-CALNAME:Work\\, Owais\r\n",
        );
        assert_eq!(calendar_name(&text).as_deref(), Some("Work, Owais"));
        assert_eq!(calendar_name(SAMPLE), None);
    }

    /// `HUSH_TEST_ICAL=<secret address> cargo test -p hush-ai real_calendar -- --ignored --nocapture`
    /// reads a real calendar and prints counts only.
    #[test]
    #[ignore]
    fn real_calendar() {
        let Ok(address) = std::env::var("HUSH_TEST_ICAL") else {
            return;
        };
        let started = Instant::now();
        let url = normalize(&address).unwrap();
        let (name, events) = fetch(&url).unwrap();
        let meetings = expand(&events, Utc::now());
        let calls = meetings.iter().filter(|m| m.link.is_some()).count();
        let mut apps: Vec<_> = meetings.iter().filter_map(|m| m.app).collect();
        apps.sort();
        apps.dedup();
        println!(
            "named: {}, events: {}, recurring: {}, occurrences in window: {}, with call links: {calls}, apps: {apps:?}, all-day: {}, took {:?}",
            name.is_some(),
            events.len(),
            events.iter().filter(|e| e.rrule.is_some()).count(),
            meetings.len(),
            meetings.iter().filter(|m| m.all_day).count(),
            started.elapsed()
        );
        let preview = Calendar::check(&address).unwrap();
        println!(
            "preview: {} meetings this week, {} with calls, next in {:?} min",
            preview.meetings,
            preview.calls,
            preview
                .next
                .map(|m| (m.start - Utc::now().timestamp_millis()) / 60_000)
        );
    }

    #[test]
    fn checks_addresses() {
        assert!(
            normalize("webcal://calendar.google.com/calendar/ical/x/private-y/basic.ics").is_ok()
        );
        assert!(normalize("http://example.com/a.ics").is_err());
        assert!(normalize("https://calendar.google.com/calendar/u/0/r").is_err());
        assert_eq!(parse_duration("PT1H30M"), Some(Span::minutes(90)));
        assert_eq!(parse_duration("P1D"), Some(Span::days(1)));
        assert_eq!(call_app("https://example.com/meet"), None);
        assert_eq!(
            call_app("https://app.slack.com/huddle/T1/C2"),
            Some("slack")
        );
    }
}
