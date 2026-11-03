// Studio (clean up a recording), Meetings (calendar) and Live Translate on top of the backend
// bridge: types, commands, a small store each window shares, and the browser simulation.
import { useSyncExternalStore } from "react";
import { emit, invoke, isTauri, on, simulated } from "./ipc";

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
  }
  interface EventMap {
    studio: StudioEvent;
    calendar: CalendarInfo;
    "meeting-soon": MeetingSoon;
    translate: TranslateInfo;
    "translate-setup": null;
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
}

export interface CalendarInfo {
  connected: boolean;
  status: "off" | "loading" | "ready" | "error";
  error: string | null;
  updated: number | null;
  source: string | null;
  name: string | null;
  meetings: Meeting[];
}

export interface CalendarPreview {
  source: string;
  name: string | null;
  meetings: number;
  calls: number;
  next: Meeting | null;
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

export interface TranslateInfo {
  hasKey: boolean;
  keyHint: string | null;
  on: boolean;
  status: "off" | "connecting" | "on" | "error";
  message: string | null;
  /** What you said, and what the other side hears. */
  heard: string;
  said: string;
  detected: string | null;
}

/** Shown in the UI; the backend registers the same combination. */
export const TRANSLATE_SHORTCUT = typeof navigator !== "undefined" && navigator.userAgent.includes("Mac") ? "⌘ ⌥ T" : "Ctrl + Alt + T";

/** A Gemini API key as Google AI Studio shows it. */
export const GEMINI_KEY_PATTERN = /^(?:AIza[0-9A-Za-z_-]{35}|AQ\.[0-9A-Za-z_.-]{20,})$/;

export const features = {
  studioOpen: (path: string) => invoke<void>("studio_open", { path }),
  studioRecord: () => invoke<void>("studio_record"),
  studioStop: () => invoke<void>("studio_stop"),
  studioCancel: () => invoke<void>("studio_cancel"),
  studioSave: (path: string) => invoke<void>("studio_save", { path }),
  calendarInfo: () => invoke<CalendarInfo>("calendar_info"),
  calendarCheck: (address: string) => invoke<CalendarPreview>("calendar_check", { address }),
  calendarConnect: (address: string) => invoke<CalendarInfo>("calendar_connect", { address }),
  calendarOpenSettings: () => invoke<void>("calendar_open_settings"),
  calendarDisconnect: () => invoke<CalendarInfo>("calendar_disconnect"),
  calendarRefresh: () => invoke<void>("calendar_refresh"),
  calendarJoin: (id: string) => invoke<void>("calendar_join", { id }),
  translateInfo: () => invoke<TranslateInfo>("translate_info"),
  translateLanguages: () => invoke<string[]>("translate_languages"),
  translateSetKey: (key: string) => invoke<TranslateInfo>("translate_set_key", { key }),
  translateRemoveKey: () => invoke<TranslateInfo>("translate_remove_key"),
  translateSet: (on: boolean) => invoke<TranslateInfo>("translate_set", { on }),
  translateOpenKeys: () => invoke<void>("translate_open_keys"),
};

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
  /** The latest Studio event; it survives switching tabs. */
  studio: StudioEvent | null;
  soon: MeetingSoon | null;
}

let state: FeatureState = { calendar: null, translate: null, translateSetup: 0, studio: null, soon: null };
const subscribers = new Set<() => void>();

export function setFeatures(patch: Partial<FeatureState>) {
  state = { ...state, ...patch };
  subscribers.forEach((fn) => fn());
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

export async function startFeatures() {
  if (started) return;
  started = true;
  await Promise.all([
    on("calendar", (calendar) => setFeatures({ calendar })),
    on("studio", (studio) => setFeatures({ studio })),
    on("meeting-soon", (soon) => setFeatures({ soon })),
    on("translate", (translate) => setFeatures({ translate })),
    on("translate-setup", () => setFeatures({ translateSetup: Date.now() })),
  ]);
  const [calendar, translate] = await Promise.allSettled([features.calendarInfo(), features.translateInfo()]);
  setFeatures({
    calendar: calendar.status === "fulfilled" ? calendar.value : null,
    translate: translate.status === "fulfilled" ? translate.value : null,
  });
}

// ---------------------------------------------------------------- Simulation

/** A believable week of meetings around now, for previews and the website demo. */
function simMeetings(): Meeting[] {
  const at = (minutes: number) => Math.round((Date.now() + minutes * 60_000) / 60_000) * 60_000;
  const day = 24 * 60;
  return [
    { id: "1", title: "Design review", start: at(12), end: at(57), allDay: false, app: "zoom" },
    { id: "2", title: "Team standup", start: at(95), end: at(110), allDay: false, app: "meet" },
    { id: "3", title: "Customer call · Northwind", start: at(190), end: at(235), allDay: false, app: "teams" },
    { id: "4", title: "Lunch with Sam", start: at(260), end: at(320), allDay: false, app: null },
    { id: "5", title: "Podcast recording", start: at(day + 30), end: at(day + 90), allDay: false, app: "webex" },
    { id: "6", title: "Sprint planning", start: at(day + 150), end: at(day + 210), allDay: false, app: "meet" },
    { id: "7", title: "Game night", start: at(2 * day + 120), end: at(2 * day + 240), allDay: false, app: "discord" },
  ];
}

function simCalendar(connected: boolean): CalendarInfo {
  return {
    connected,
    status: connected ? "ready" : "off",
    error: null,
    updated: connected ? Date.now() : null,
    source: connected ? "Google Calendar" : null,
    name: connected ? "Work" : null,
    meetings: connected ? simMeetings() : [],
  };
}

const LANGUAGES = ["af", "ak", "sq", "am", "ar", "hy", "az", "eu", "be", "bn", "bg", "my", "ca", "zh-Hans", "zh-Hant", "hr", "cs", "da", "nl", "en", "et", "fil", "fi", "fr", "gl", "ka", "de", "el", "gu", "ha", "he", "hi", "hu", "is", "id", "it", "ja", "jv", "kn", "kk", "km", "rw", "ko", "lo", "lv", "lt", "mk", "ms", "ml", "mr", "mn", "ne", "nb", "fa", "pl", "pt-BR", "pt-PT", "pa", "ro", "ru", "sr", "sd", "si", "sk", "sl", "es", "su", "sw", "sv", "ta", "te", "th", "tr", "uk", "ur", "uz", "vi", "zu"];

const CAPTIONS: [string, string][] = [
  ["Thanks for joining. Let's look at the launch plan for next week.", "Gracias por unirse. Veamos el plan de lanzamiento para la próxima semana."],
];

if (!isTauri) {
  const params = new URLSearchParams(typeof location === "undefined" ? "" : location.search);
  let captionTimer = 0;
  let translate: TranslateInfo = {
    hasKey: params.get("translate") !== "0",
    keyHint: params.get("translate") !== "0" ? "x9Qe" : null,
    on: false,
    status: "off",
    message: null,
    heard: "",
    said: "",
    detected: null,
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
          resolve({ source: "Google Calendar", name: "Work", meetings: meetings.length, calls: meetings.filter((m) => m.app).length, next: meetings[0] });
        }, 700),
      ),
    calendar_connect: () =>
      new Promise((resolve) =>
        setTimeout(() => {
          calendar = simCalendar(true);
          emit("calendar", calendar);
          resolve(calendar);
        }, 500),
      ),
    calendar_open_settings: () => window.open("https://calendar.google.com/calendar/u/0/r/settings", "_blank", "noopener"),
    calendar_disconnect: () => {
      calendar = simCalendar(false);
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
      translate = { ...translate, hasKey: false, keyHint: null, on: false, status: "off" };
      emit("translate", translate);
      return translate;
    },
    translate_set: ({ on }: Record<string, unknown>) => {
      clearInterval(captionTimer);
      if (!on) {
        translate = { ...translate, on: false, status: "off" };
        emit("translate", translate);
        return translate;
      }
      if (!translate.hasKey) {
        emit("translate-setup", null);
        return translate;
      }
      translate = { ...translate, on: true, status: "connecting", heard: "", said: "", message: null };
      emit("translate", translate);
      setTimeout(() => {
        translate = { ...translate, status: "on", detected: "en" };
        emit("translate", translate);
        let i = 0;
        captionTimer = window.setInterval(() => {
          const [heard, said] = CAPTIONS[i % CAPTIONS.length];
          translate = { ...translate, heard: heard.slice(0, ++i * 3 % (heard.length + 12)), said: said.slice(0, Math.max(0, (i * 3 % (said.length + 12)) - 10)) };
          emit("translate", translate);
        }, 160);
      }, 1100);
      return translate;
    },
    translate_open_keys: () => window.open("https://aistudio.google.com/apikey", "_blank", "noopener"),
  });
}
