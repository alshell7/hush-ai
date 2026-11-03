// Studio (clean up a recording), Meetings (calendar), Talk (Live Translate both ways, and the
// conversations it keeps) and the notch, on top of the backend bridge: types, commands, a small
// store each window shares, and the browser simulation.
import { useSyncExternalStore } from "react";
import { emit, invoke, isTauri, on, simSettings, simulated } from "./ipc";

export type AppId = "zoom" | "meet" | "teams" | "slack" | "discord" | "webex" | "obs" | "other";

declare module "./ipc" {
  interface Settings {
    calendarAuto: boolean;
    calendarLead: number;
    calendarAutoOff: boolean;
    calendarPopup: boolean;
    calendarSync: number;
    translateFrom: string;
    translateTo: string;
    translateAsk: boolean;
    translateHistory: boolean;
    listen: boolean;
    listenClean: CleanLevel;
    listenVoice: HearMode;
    listenOutput: string | null;
    talkMode: TalkMode;
    overlay: "widget" | "notch";
    hideFromCapture: boolean;
  }
  interface EventMap {
    studio: StudioEvent;
    calendar: CalendarInfo;
    "meeting-soon": MeetingSoon;
    translate: TranslateInfo;
    "translate-setup": null;
    "conversation-line": { thread: ThreadSummary; line: Line };
    listen: ListenInfo;
    "notch-hover": boolean;
    "notch-pin": null;
    "notch-geometry": NotchGeometry;
  }
}

export interface Meeting {
  id: string;
  title: string;
  /** Unix time in milliseconds. */
  start: number;
  end: number;
  allDay: boolean;
  app: AppId | null;
  /** The calendars it is in (their ids). */
  calendars: string[];
}

export interface CalendarView {
  id: string;
  name: string | null;
  source: string;
  enabled: boolean;
  status: "off" | "loading" | "ready" | "error";
  error: string | null;
  updated: number | null;
  /** A colour slot, 0 to 5. */
  color: number;
  meetings: number;
}

export interface CalendarInfo {
  connected: boolean;
  status: "off" | "loading" | "ready" | "error";
  error: string | null;
  updated: number | null;
  source: string | null;
  name: string | null;
  calendars: CalendarView[];
  meetings: Meeting[];
}

/** Calendar colours, by slot. */
export const CALENDAR_COLORS = ["#e8735a", "#1f9d72", "#d99a1e", "#178f8a", "#d6457a", "#6b7cc9"];

export interface CalendarPreview {
  source: string;
  name: string | null;
  meetings: number;
  calls: number;
  next: Meeting | null;
  connected: boolean;
}

export interface MeetingSoon {
  meeting: Meeting;
  turnedOn: boolean;
}

export interface StudioResult {
  name: string;
  seconds: number;
  original: string;
  enhanced: string;
  originalPeaks: number[];
  enhancedPeaks: number[];
  engine: string;
  elapsed: number;
}

export type StudioEvent =
  | { phase: "recording"; seconds: number; level: number }
  | { phase: "decoding"; name: string }
  | { phase: "processing"; name: string; progress: number; eta: number | null }
  | ({ phase: "ready" } & StudioResult)
  | { phase: "cancelled" }
  | { phase: "error"; message: string };

export type Side = "me" | "them";
export type CleanLevel = "off" | "light" | "full";
/** What you hear while their voice is translated. */
export type HearMode = "original" | "voiceOver" | "translation";
/** Which directions the conversation switch turns on. */
export type TalkMode = "me" | "them" | "both";
export type LaneStatus = "off" | "connecting" | "on" | "error";

/** One direction of the conversation. */
export interface LaneInfo {
  on: boolean;
  status: LaneStatus;
  message: string | null;
  /** What was said and its translation, as rolling captions. */
  original: string;
  translated: string;
  detected: string | null;
  /** The phrase still being said: not yet in the conversation. */
  pendingOriginal: string;
  pendingTranslated: string;
}

export interface TranslateInfo {
  hasKey: boolean;
  keyHint: string | null;
  /** Your voice, translated for them. */
  me: LaneInfo;
  /** Their voice, translated for you. */
  them: LaneInfo;
}

/** One phrase of a conversation. */
export interface Line {
  at: number;
  side: Side;
  from: string;
  to: string;
  original: string;
  translated: string;
}

export interface ThreadSummary {
  id: number;
  start: number;
  end: number;
  title: string | null;
  meeting: string | null;
  meetingTitle: string | null;
  app: AppId | null;
  starred: boolean;
  lines: number;
  mine: number;
  theirs: number;
  languages: string[];
  preview: string;
}

export interface Thread {
  id: number;
  start: number;
  end: number;
  title: string | null;
  meeting: string | null;
  meetingTitle: string | null;
  app: AppId | null;
  starred: boolean;
  lines: Line[];
}

/** Their voice: where it comes from and what Hush does with it. */
export interface ListenInfo {
  /** "speaker": Hush AI Speaker, Hush plays (and can clean) their voice; "loopback": Hush
   * listens along to the speakers, for translation only. */
  mode: "speaker" | "loopback" | "none";
  device: string | null;
  output: string | null;
  running: boolean;
  /** Someone's voice is arriving: a call is playing. */
  active: boolean;
  cleaning: boolean;
  message: string | null;
}

export interface NotchGeometry {
  hasNotch: boolean;
  notchWidth: number;
  notchHeight: number;
  menuBar: number;
}

/** A conversation's name: the user's, the meeting's, or when it happened. */
export function threadTitle(t: { title: string | null; meetingTitle: string | null; start: number }) {
  if (t.title) return t.title;
  if (t.meetingTitle) return t.meetingTitle;
  const date = new Date(t.start);
  const today = new Date().toDateString() === date.toDateString();
  return `${today ? "Conversation" : date.toLocaleDateString(undefined, { weekday: "long" })} at ${date.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}`;
}

/** Shown in the UI; the backend registers the same combination. */
export const TRANSLATE_SHORTCUT = typeof navigator !== "undefined" && navigator.userAgent.includes("Mac") ? "⌘ ⌥ T" : "Ctrl + Alt + T";

/** A Gemini API key as Google AI Studio shows it. */
export const GEMINI_KEY_PATTERN = /^(?:AIza[0-9A-Za-z_-]{35}|AQ\.[0-9A-Za-z_.-]{20,})$/;

export const features = {
  studioOpen: (path: string) => invoke<void>("studio_open", { path }),
  studioRecord: (input: string | null) => invoke<void>("studio_record", { input }),
  studioMonitor: (on: boolean, input: string | null) => invoke<void>("studio_monitor", { on, input }),
  studioStop: () => invoke<void>("studio_stop"),
  studioCancel: () => invoke<void>("studio_cancel"),
  studioSave: (path: string) => invoke<void>("studio_save", { path }),
  calendarInfo: () => invoke<CalendarInfo>("calendar_info"),
  calendarCheck: (address: string) => invoke<CalendarPreview>("calendar_check", { address }),
  calendarConnect: (address: string) => invoke<CalendarInfo>("calendar_connect", { address }),
  calendarOpenSettings: () => invoke<void>("calendar_open_settings"),
  calendarDisconnect: (id?: string) => invoke<CalendarInfo>("calendar_disconnect", { id: id ?? null }),
  calendarSetEnabled: (id: string, enabled: boolean) => invoke<CalendarInfo>("calendar_set_enabled", { id, enabled }),
  calendarRefresh: () => invoke<void>("calendar_refresh"),
  calendarJoin: (id: string) => invoke<void>("calendar_join", { id }),
  translateInfo: () => invoke<TranslateInfo>("translate_info"),
  translateLanguages: () => invoke<string[]>("translate_languages"),
  translateSetKey: (key: string) => invoke<TranslateInfo>("translate_set_key", { key }),
  translateRemoveKey: () => invoke<TranslateInfo>("translate_remove_key"),
  translateSet: (on: boolean) => invoke<TranslateInfo>("translate_set", { on }),
  translateSetSide: (side: Side, on: boolean) => invoke<TranslateInfo>("translate_set_side", { side, on }),
  talkSet: (on: boolean) => invoke<TranslateInfo>("talk_set", { on }),
  translateOpenKeys: () => invoke<void>("translate_open_keys"),
  conversationsList: () => invoke<ThreadSummary[]>("conversations_list"),
  conversationGet: (id: number) => invoke<Thread | null>("conversation_get", { id }),
  conversationStar: (id: number, starred: boolean) => invoke<ThreadSummary | null>("conversation_star", { id, starred }),
  conversationRename: (id: number, title: string | null) => invoke<ThreadSummary | null>("conversation_rename", { id, title }),
  conversationDelete: (id: number) => invoke<void>("conversation_delete", { id }),
  conversationsClear: () => invoke<void>("conversations_clear"),
  conversationsExport: (ids: number[], format: "md" | "txt" | "json", path: string) => invoke<void>("conversations_export", { ids, format, path }),
  listenInfo: () => invoke<ListenInfo>("listen_info"),
  listenLevel: () => invoke<number>("listen_level"),
  notchRegion: (x: number, y: number, width: number, height: number) => invoke<void>("notch_region", { x, y, width, height }),
  notchGeometry: () => invoke<NotchGeometry>("notch_geometry"),
};

/** Whether any direction of the conversation is on. */
export const talking = (t: TranslateInfo | null) => !!t && (t.me.on || t.them.on);

const names = typeof Intl !== "undefined" && "DisplayNames" in Intl ? new Intl.DisplayNames(undefined, { type: "language" }) : null;

/** "Spanish", "Chinese (Simplified)", in the user's own language. */
export function languageName(code: string) {
  try {
    return names?.of(code) ?? code;
  } catch {
    return code;
  }
}

/** A Google Calendar iCal address, as copied from its settings. */
export const ICAL_PATTERN = /^(?:https|webcal):\/\/calendar\.google\.com\/calendar\/ical\/[^\s/]+\/(?:private-[^\s/]+|public)\/basic\.ics$/;

/** A URL the page can play for a file the backend wrote. */
export async function mediaUrl(path: string) {
  if (!isTauri) return new URL(path, location.href).href;
  const { convertFileSrc } = await import("@tauri-apps/api/core");
  return convertFileSrc(path);
}

/** Text on the clipboard, or null when it cannot be read. */
export async function clipboardText(): Promise<string | null> {
  try {
    if (isTauri) {
      const { readText } = await import("@tauri-apps/plugin-clipboard-manager");
      return await readText();
    }
    return await navigator.clipboard.readText();
  } catch {
    return null;
  }
}

// ---------------------------------------------------------------- Store

interface FeatureState {
  calendar: CalendarInfo | null;
  translate: TranslateInfo | null;
  /** Set when something asks for the Gemini key setup (the tray menu or the shortcut). */
  translateSetup: number;
  /** Conversations, newest first, without lines; `null` until a view asks. */
  conversations: ThreadSummary[] | null;
  /** Lines of conversations that were opened, by id; live lines are added as they arrive. */
  lines: Record<number, Line[]>;
  listen: ListenInfo | null;
  /** The latest Studio event; it survives switching tabs. */
  studio: StudioEvent | null;
  soon: MeetingSoon | null;
}

let state: FeatureState = { calendar: null, translate: null, translateSetup: 0, conversations: null, lines: {}, listen: null, studio: null, soon: null };
const subscribers = new Set<() => void>();

export function setFeatures(patch: Partial<FeatureState>) {
  state = { ...state, ...patch };
  subscribers.forEach((fn) => fn());
}

export function getFeatures() {
  return state;
}

export function useFeatures<T>(select: (s: FeatureState) => T): T {
  return useSyncExternalStore(
    (fn) => {
      subscribers.add(fn);
      return () => subscribers.delete(fn);
    },
    () => select(state),
  );
}

let started = false;

/** Load the conversation list the first time a view needs it. */
export async function loadConversations(force = false) {
  if (state.conversations && !force) return;
  try {
    setFeatures({ conversations: await features.conversationsList() });
  } catch {
    setFeatures({ conversations: [] });
  }
}

/** Load one conversation's lines. */
export async function loadThread(id: number) {
  if (state.lines[id]) return;
  const thread = await features.conversationGet(id).catch(() => null);
  setFeatures({ lines: { ...state.lines, [id]: thread?.lines ?? [] } });
}

/** Keep a conversation's summary in the list (newest first). */
export function putSummary(summary: ThreadSummary) {
  if (!state.conversations) return;
  const rest = state.conversations.filter((t) => t.id !== summary.id);
  setFeatures({ conversations: [summary, ...rest].sort((a, b) => b.start - a.start) });
}

export function dropThread(id: number) {
  const lines = { ...state.lines };
  delete lines[id];
  setFeatures({ conversations: state.conversations?.filter((t) => t.id !== id) ?? null, lines });
}

export async function startFeatures() {
  if (started) return;
  started = true;
  await Promise.all([
    on("calendar", (calendar) => setFeatures({ calendar })),
    on("studio", (studio) => setFeatures({ studio })),
    on("meeting-soon", (soon) => setFeatures({ soon })),
    on("translate", (translate) => setFeatures({ translate })),
    on("listen", (listen) => setFeatures({ listen })),
    on("translate-setup", () => setFeatures({ translateSetup: Date.now() })),
    on("conversation-line", ({ thread, line }) => {
      const known = state.lines[thread.id];
      // A new conversation starts with its first line; a known one grows.
      setFeatures({ lines: { ...state.lines, [thread.id]: known ? [...known, line] : [line] } });
      putSummary(thread);
    }),
  ]);
  const [calendar, translate, listen] = await Promise.allSettled([features.calendarInfo(), features.translateInfo(), features.listenInfo()]);
  setFeatures({
    calendar: calendar.status === "fulfilled" ? calendar.value : null,
    translate: translate.status === "fulfilled" ? translate.value : null,
    listen: listen.status === "fulfilled" ? listen.value : null,
  });
}

// ---------------------------------------------------------------- Simulation

/** A believable week of meetings around now, for previews and the website demo. */
function simMeetings(): Meeting[] {
  const at = (minutes: number) => Math.round((Date.now() + minutes * 60_000) / 60_000) * 60_000;
  const day = 24 * 60;
  const list: Omit<Meeting, "calendars">[] = [
    { id: "1", title: "Design review", start: at(12), end: at(57), allDay: false, app: "zoom" },
    { id: "2", title: "Team standup", start: at(95), end: at(110), allDay: false, app: "meet" },
    { id: "3", title: "Customer call · Northwind", start: at(190), end: at(235), allDay: false, app: "teams" },
    { id: "4", title: "Lunch with Sam", start: at(260), end: at(320), allDay: false, app: null },
    { id: "5", title: "Podcast recording", start: at(day + 30), end: at(day + 90), allDay: false, app: "webex" },
    { id: "6", title: "Sprint planning", start: at(day + 150), end: at(day + 210), allDay: false, app: "meet" },
    { id: "7", title: "Game night", start: at(2 * day + 120), end: at(2 * day + 240), allDay: false, app: "discord" },
  ];
  return list.map((m) => ({ ...m, calendars: ["c0"] }));
}

function simCalendar(connected: boolean, count = 2): CalendarInfo {
  const names = ["Work", "Personal", "Team", "Family"];
  const calendars: CalendarView[] = connected
    ? names.slice(0, count).map((name, i) => ({ id: `c${i}`, name, source: "Google Calendar", enabled: true, status: "ready", error: null, updated: Date.now(), color: i, meetings: 0 }))
    : [];
  const meetings = connected ? simMeetings().map((m, i) => ({ ...m, calendars: i === 0 && count > 1 ? ["c0", "c1"] : [`c${i % count}`] })) : [];
  calendars.forEach((c) => (c.meetings = meetings.filter((m) => m.calendars.includes(c.id)).length));
  return {
    connected,
    status: connected ? "ready" : "off",
    error: null,
    updated: connected ? Date.now() : null,
    source: connected ? "Google Calendar" : null,
    name: connected ? "Work" : null,
    calendars,
    meetings,
  };
}

const LANGUAGES = ["af", "ak", "sq", "am", "ar", "hy", "az", "eu", "be", "bn", "bg", "my", "ca", "zh-Hans", "zh-Hant", "hr", "cs", "da", "nl", "en", "et", "fil", "fi", "fr", "gl", "ka", "de", "el", "gu", "ha", "he", "hi", "hu", "is", "id", "it", "ja", "jv", "kn", "kk", "km", "rw", "ko", "lo", "lv", "lt", "mk", "ms", "ml", "mr", "mn", "ne", "nb", "fa", "pl", "pt-BR", "pt-PT", "pa", "ro", "ru", "sr", "sd", "si", "sk", "sl", "es", "su", "sw", "sv", "ta", "te", "th", "tr", "uk", "ur", "uz", "vi", "zu"];

/** Simulated phrases: [side, original, translated]. */
const CAPTIONS: [Side, string, string][] = [
  ["me", "Thanks for joining. Let's look at the launch plan for next week.", "Gracias por unirse. Veamos el plan de lanzamiento para la próxima semana."],
  ["them", "Perfecto. ¿Podemos empezar por el presupuesto?", "Perfect. Can we start with the budget?"],
  ["me", "Sure. We kept it the same as last quarter.", "Claro. Lo mantuvimos igual que el trimestre pasado."],
  ["them", "Muy bien, entonces solo falta la fecha.", "Great, then only the date is left."],
];

if (!isTauri) {
  const params = new URLSearchParams(typeof location === "undefined" ? "" : location.search);
  let captionTimer = 0;
  let levelTimer = 0;
  const minute = 60_000;
  const line = (at: number, side: Side, from: string, to: string, original: string, translated: string): Line => ({ at, side, from, to, original, translated });
  const ago = (m: number) => Date.now() - m * minute;
  const THREADS: Thread[] = [
    {
      id: ago(26 * 60), start: ago(26 * 60), end: ago(26 * 60 - 3), title: null, meeting: "1", meetingTitle: "Design review · Madrid office", app: "zoom", starred: true,
      lines: [
        line(ago(26 * 60), "me", "en", "es", "Thanks everyone for joining today's review.", "Gracias a todos por unirse a la revisión de hoy."),
        line(ago(26 * 60 - 1), "them", "es", "en", "Gracias a ti. ¿Empezamos con las pantallas nuevas?", "Thank you. Shall we start with the new screens?"),
        line(ago(26 * 60 - 2), "me", "en", "es", "Yes, let's start with the onboarding.", "Sí, empecemos con la bienvenida."),
        line(ago(26 * 60 - 3), "them", "es", "en", "Me encanta cómo quedó el flujo.", "I love how the flow turned out."),
      ],
    },
    {
      id: ago(50), start: ago(50), end: ago(47), title: null, meeting: null, meetingTitle: null, app: null, starred: false,
      lines: [
        line(ago(50), "me", "en", "de", "Could you share the latest numbers before Friday?", "Könnten Sie die neuesten Zahlen vor Freitag teilen?"),
        line(ago(49), "them", "de", "en", "Ja, ich schicke sie Ihnen morgen früh.", "Yes, I'll send them to you tomorrow morning."),
        line(ago(47), "me", "en", "de", "That works for me, thank you.", "Das passt mir, danke."),
      ],
    },
    {
      id: ago(6), start: ago(6), end: ago(5), title: "Intro with Aiko", meeting: null, meetingTitle: null, app: "meet", starred: false,
      lines: [
        line(ago(6), "me", "en", "ja", "Nice to meet you. I'm looking forward to working together.", "はじめまして。一緒に働けるのを楽しみにしています。"),
        line(ago(5), "them", "ja", "en", "こちらこそ、よろしくお願いします。", "Likewise, I look forward to working with you."),
      ],
    },
  ];
  const summary = (t: Thread): ThreadSummary => ({
    id: t.id, start: t.start, end: t.end, title: t.title, meeting: t.meeting, meetingTitle: t.meetingTitle, app: t.app, starred: t.starred,
    lines: t.lines.length, mine: t.lines.filter((l) => l.side === "me").length, theirs: t.lines.filter((l) => l.side === "them").length,
    languages: [...new Set(t.lines.flatMap((l) => [l.from, l.to]))], preview: t.lines[0]?.side === "them" ? t.lines[0].translated : t.lines[0]?.original ?? "",
  });
  const lane = (): LaneInfo => ({ on: false, status: "off", message: null, original: "", translated: "", detected: null, pendingOriginal: "", pendingTranslated: "" });
  let translate: TranslateInfo = {
    hasKey: params.get("translate") !== "0",
    keyHint: params.get("translate") !== "0" ? "x9Qe" : null,
    me: lane(),
    them: lane(),
  };
  let listen: ListenInfo = {
    mode: params.get("speaker") === "0" ? "loopback" : "speaker",
    device: params.get("speaker") === "0" ? null : "Hush AI Speaker",
    output: "Speakers (Realtek(R) Audio)",
    running: true,
    active: params.get("call") === "1",
    cleaning: params.get("call") === "1",
    message: null,
  };
  let live: Thread | null = null;
  // Simulated speech: captions roll, then each phrase lands in the conversation.
  const speak = () => {
    clearInterval(captionTimer);
    if (!translate.me.on && !translate.them.on) return;
    let phrase = 0;
    let step = 0;
    captionTimer = window.setInterval(() => {
      const [side, original, translated] = CAPTIONS[phrase % CAPTIONS.length];
      const key = side === "me" ? "me" : "them";
      if (!translate[key].on) {
        phrase++;
        return;
      }
      step++;
      const shown = Math.min(original.length, step * 4);
      const now = { original: original.slice(0, shown), translated: translated.slice(0, Math.max(0, shown - 14)) };
      translate = { ...translate, [key]: { ...translate[key], ...now, pendingOriginal: now.original, pendingTranslated: now.translated } };
      emit("translate", translate);
      if (shown >= original.length && step * 4 >= original.length + 30) {
        translate = { ...translate, [key]: { ...translate[key], translated, pendingOriginal: "", pendingTranslated: "" } };
        emit("translate", translate);
        const mine = simSettings().translateFrom ?? "en";
        const theirs = simSettings().translateTo ?? "es";
        const added = line(Date.now(), side, side === "me" ? mine : theirs, side === "me" ? theirs : mine, original, translated);
        if (!live || Date.now() - live.end > 10 * minute) {
          live = { id: added.at, start: added.at, end: added.at, title: null, meeting: null, meetingTitle: null, app: null, starred: false, lines: [] };
          THREADS.push(live);
        }
        live.lines.push(added);
        live.end = added.at;
        emit("conversation-line", { thread: summary(live), line: added });
        phrase++;
        step = 0;
      }
    }, 120);
  };
  const setSide = (side: Side, on: boolean): TranslateInfo => {
    if (on && !translate.hasKey) {
      emit("translate-setup", null);
      return translate;
    }
    translate = { ...translate, [side]: on ? { ...lane(), on: true, status: "connecting" } : { ...translate[side], on: false, status: "off" } };
    emit("translate", translate);
    if (on) {
      setTimeout(() => {
        if (!translate[side].on) return;
        translate = { ...translate, [side]: { ...translate[side], status: "on", detected: side === "me" ? "en" : "es" } };
        emit("translate", translate);
        if (side === "them") emit("listen", (listen = { ...listen, active: true, cleaning: listen.mode === "speaker" }));
        speak();
      }, 900);
    } else {
      speak();
    }
    return translate;
  };
  let calendar = simCalendar(params.get("calendar") !== "0");
  let timer = 0;
  // `?soon=1` shows the meeting pop-up, as the calendar does a minute before a call.
  if (params.get("soon") === "1") {
    setTimeout(() => {
      const meeting = { ...calendar.meetings[0], start: Date.now() + 58_000, end: Date.now() + 45 * 60_000 };
      emit("meeting-soon", { meeting, turnedOn: true });
    }, 600);
  }

  const peaks = (clean: boolean) =>
    Array.from({ length: 480 }, (_, i) => {
      const voice = Math.max(0, Math.sin(i / 9) * Math.sin(i / 37 + 1)) ** 1.4 * 0.85;
      const noise = 0.14 + 0.1 * Math.abs((Math.sin(i * 12.9898) * 43758.5453) % 1);
      return clean ? (voice > 0.05 ? voice : 0.012) : Math.min(1, voice + noise);
    });

  const process = (name: string) => {
    clearInterval(timer);
    emit("studio", { phase: "decoding", name });
    let progress = 0;
    timer = window.setInterval(() => {
      progress = Math.min(1, progress + 0.04);
      emit("studio", { phase: "processing", name, progress, eta: (1 - progress) * 2.6 });
      if (progress < 1) return;
      clearInterval(timer);
      emit("studio", {
        phase: "ready",
        name,
        seconds: 9.8,
        original: "../samples/noisy.mp3",
        enhanced: "../samples/hush.mp3",
        originalPeaks: peaks(false),
        enhancedPeaks: peaks(true),
        engine: "DeepFilterNet 3",
        elapsed: 2.6,
      });
    }, 100);
  };

  Object.assign(simulated, {
    studio_open: ({ path }: Record<string, unknown>) => process(String(path).split(/[\\/]/).pop()!.replace(/\.[^.]+$/, "")),
    studio_record: () => {
      clearInterval(timer);
      let seconds = 0;
      timer = window.setInterval(() => {
        seconds += 0.08;
        emit("studio", { phase: "recording", seconds, level: Math.min(1, 0.08 + Math.abs(Math.sin(seconds * 3.1)) * 0.6 * Math.random()) });
      }, 80);
    },
    studio_stop: () => process("Recording"),
    studio_cancel: () => {
      clearInterval(timer);
      emit("studio", { phase: "cancelled" });
    },
    studio_save: () => new Promise((r) => setTimeout(r, 300)),
    calendar_info: () => calendar,
    calendar_check: ({ address }: Record<string, unknown>) =>
      new Promise((resolve, reject) =>
        setTimeout(() => {
          const text = String(address).trim();
          if (!/^(https|webcal):\/\//.test(text)) return reject("Paste the address that starts with https://calendar.google.com/calendar/ical/");
          const meetings = simMeetings();
          resolve({ source: "Google Calendar", name: calendar.calendars.length ? "Personal" : "Work", meetings: meetings.length, calls: meetings.filter((m) => m.app).length, next: meetings[0], connected: false });
        }, 700),
      ),
    calendar_connect: () =>
      new Promise((resolve) =>
        setTimeout(() => {
          calendar = simCalendar(true, Math.min(4, calendar.calendars.length + 1));
          emit("calendar", calendar);
          resolve(calendar);
        }, 500),
      ),
    calendar_open_settings: () => window.open("https://calendar.google.com/calendar/u/0/r/settings", "_blank", "noopener"),
    calendar_set_enabled: ({ id, enabled }: Record<string, unknown>) => {
      calendar = { ...calendar, calendars: calendar.calendars.map((c) => (c.id === id ? { ...c, enabled: !!enabled, status: enabled ? "ready" : "off" } : c)) };
      const on = new Set(calendar.calendars.filter((c) => c.enabled).map((c) => c.id));
      calendar = { ...calendar, meetings: simCalendar(true, calendar.calendars.length).meetings.filter((m) => m.calendars.some((c) => on.has(c))) };
      emit("calendar", calendar);
      return calendar;
    },
    calendar_disconnect: ({ id }: Record<string, unknown>) => {
      const left = id ? calendar.calendars.length - 1 : 0;
      calendar = left > 0 ? simCalendar(true, left) : simCalendar(false);
      emit("calendar", calendar);
      return calendar;
    },
    calendar_refresh: () => {
      emit("calendar", { ...calendar, status: "loading" });
      setTimeout(() => emit("calendar", (calendar = { ...calendar, status: "ready", updated: Date.now() })), 900);
    },
    calendar_join: () => undefined,
    translate_info: () => translate,
    translate_languages: () => LANGUAGES,
    translate_set_key: ({ key }: Record<string, unknown>) =>
      new Promise((resolve, reject) =>
        setTimeout(() => {
          if (String(key).trim().length < 20) return reject("This does not look like a Gemini API key.");
          translate = { ...translate, hasKey: true, keyHint: String(key).trim().slice(-4) };
          emit("translate", translate);
          resolve(translate);
        }, 900),
      ),
    translate_remove_key: () => {
      clearInterval(captionTimer);
      translate = { ...translate, hasKey: false, keyHint: null, me: lane(), them: lane() };
      emit("translate", translate);
      return translate;
    },
    translate_set: ({ on }: Record<string, unknown>) => setSide("me", !!on),
    translate_set_side: ({ side, on }: Record<string, unknown>) => setSide(side as Side, !!on),
    talk_set: ({ on }: Record<string, unknown>) => {
      const mode: TalkMode = simSettings().talkMode ?? "both";
      setSide("me", !!on && mode !== "them");
      return setSide("them", !!on && mode !== "me");
    },
    conversations_list: () => [...THREADS].sort((a, b) => b.start - a.start).map(summary),
    conversation_get: ({ id }: Record<string, unknown>) => THREADS.find((t) => t.id === id) ?? null,
    conversation_star: ({ id, starred }: Record<string, unknown>) => {
      const t = THREADS.find((x) => x.id === id);
      if (!t) return null;
      t.starred = !!starred;
      return summary(t);
    },
    conversation_rename: ({ id, title }: Record<string, unknown>) => {
      const t = THREADS.find((x) => x.id === id);
      if (!t) return null;
      t.title = title ? String(title) : null;
      return summary(t);
    },
    conversation_delete: ({ id }: Record<string, unknown>) => {
      const i = THREADS.findIndex((x) => x.id === id);
      if (i >= 0) THREADS.splice(i, 1);
    },
    conversations_clear: () => {
      THREADS.length = 0;
    },
    conversations_export: () => new Promise((r) => setTimeout(r, 300)),
    listen_info: () => listen,
    listen_level: () => (listen.active ? 0.15 + Math.abs(Math.sin(Date.now() / 260)) * 0.5 * Math.random() : 0),
    notch_region: () => undefined,
    notch_geometry: () => ({ hasNotch: params.get("macnotch") === "1", notchWidth: 186, notchHeight: 32, menuBar: params.get("platform") === "macos" ? 24 : 0 }),
    studio_monitor: ({ on }: Record<string, unknown>) => {
      clearInterval(levelTimer);
      if (on) levelTimer = window.setInterval(() => emit("mic-level", Math.min(1, 0.05 + Math.abs(Math.sin(Date.now() / 300)) * 0.5 * Math.random())), 60);
    },
    translate_open_keys: () => window.open("https://aistudio.google.com/apikey", "_blank", "noopener"),
  });
}
