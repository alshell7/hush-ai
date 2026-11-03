// Typed bridge to the Rust backend. In a plain browser (design previews, the website demo)
// a small simulation stands in for the backend, so every screen can render without Tauri.
import type { UnlistenFn } from "@tauri-apps/api/event";

export type Quality = "light" | "balanced" | "studio";
export type EngineStatus = "off" | "starting" | "on" | "error";
export type VmicState = "ready" | "install" | "download" | "unsupported";
export type Platform = "macos" | "windows" | "linux" | string;

export interface Settings {
  onboarded: boolean;
  licenseAccepted: boolean;
  enabled: boolean;
  input: string | null;
  output: string | null;
  quality: Quality;
  threads: number;
  audioPriority: boolean;
  checkUpdates: boolean;
  trayHintShown: boolean;
}

export interface EngineState {
  status: EngineStatus;
  message: string | null;
  threads: number;
  inputRate: number;
  modelRate: number;
  outputRate: number;
}

export interface ModelInfo {
  installed: boolean;
  path: string | null;
  size: number;
}

export interface Snapshot {
  settings: Settings;
  model: ModelInfo;
  engine: EngineState;
  platform: Platform;
  version: string;
}

export interface Device {
  name: string | null;
  label: string;
}

export interface Devices {
  inputs: Device[];
  outputs: Device[];
}

export interface Meters {
  input: number;
  output: number;
  load: number;
  dropped: number;
}

export interface VmicInfo {
  state: VmicState;
  driver: "hush" | "vb-cable" | "pulse" | "none";
  detail: string;
}

export interface ModelProgress {
  received: number;
  total: number;
  bytesPerSecond: number;
  source: number;
}

export type TestEvent =
  | { phase: "recording"; seconds: number }
  | { phase: "processing" }
  | { phase: "ready"; original: number[]; enhanced: number[]; seconds: number }
  | { phase: "playing"; enhanced: boolean }
  | { phase: "stopped" }
  | { phase: "error"; message: string };

export interface EventMap {
  engine: EngineState;
  meters: Meters;
  settings: Settings;
  "model-progress": ModelProgress;
  "mic-level": number;
  "mic-level-error": string;
  test: TestEvent;
  "tray-hint": null;
}

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

type Handler<K extends keyof EventMap> = (payload: EventMap[K]) => void;

// ---------------------------------------------------------------- Tauri

async function tauriInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
}

async function tauriListen<K extends keyof EventMap>(event: K, handler: Handler<K>): Promise<UnlistenFn> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<EventMap[K]>(event, (e) => handler(e.payload));
}

// ---------------------------------------------------------------- Simulation

const listeners = new Map<string, Set<(payload: unknown) => void>>();
export function emit<K extends keyof EventMap>(event: K, payload: EventMap[K]) {
  listeners.get(event)?.forEach((fn) => fn(payload));
}

const sim = {
  settings: {
    onboarded: new URLSearchParams(location.search).get("onboarded") !== "0",
    licenseAccepted: true,
    enabled: true,
    input: null,
    output: null,
    quality: "light",
    threads: 0,
    audioPriority: true,
    checkUpdates: true,
    trayHintShown: true,
  } as Settings,
  engine: {
    status: "on",
    message: null,
    threads: 2,
    inputRate: 48000,
    modelRate: 16000,
    outputRate: 48000,
  } as EngineState,
  model: { installed: true, path: "models/realtime-reuse-4c0c186.safetensors", size: 54055764 } as ModelInfo,
  vmic: {
    state: (new URLSearchParams(location.search).get("vmic") as VmicState | null) ?? "ready",
    driver: "hush",
    detail: "Hush AI Microphone",
  } as VmicInfo,
  meterTimer: 0 as number,
  levelTimer: 0 as number,
};

function simulateMeters(on: boolean) {
  clearInterval(sim.meterTimer);
  if (!on) return;
  let t = 0;
  sim.meterTimer = window.setInterval(() => {
    t += 0.05;
    const speech = Math.max(0, Math.sin(t * 1.7) * Math.sin(t * 0.37 + 1)) ** 1.6;
    const noise = 0.08 + Math.random() * 0.05;
    const input = Math.min(1, noise + speech * (0.55 + Math.random() * 0.25));
    const output = sim.engine.status === "on" ? Math.min(1, speech * (0.5 + Math.random() * 0.2)) : 0;
    emit("meters", { input, output, load: 0.34 + Math.random() * 0.06, dropped: 0 });
  }, 50);
}

function setEngine(patch: Partial<EngineState>) {
  sim.engine = { ...sim.engine, ...patch };
  emit("engine", sim.engine);
}

export const simulated: Record<string, (args: Record<string, unknown>) => unknown> = {
  snapshot: () => ({ settings: sim.settings, model: sim.model, engine: sim.engine, platform: guessPlatform(), version: "1.0.0" }),
  app_ready: () => undefined,
  devices: () => ({
    inputs: [
      { name: null, label: "System default" },
      { name: "MacBook Pro Microphone", label: "MacBook Pro Microphone" },
      { name: "AirPods Pro", label: "AirPods Pro" },
      { name: "Shure MV7", label: "Shure MV7" },
    ],
    outputs: [
      { name: null, label: "System default" },
      { name: "AirPods Pro", label: "AirPods Pro" },
    ],
  }),
  update_settings: ({ patch }) => {
    sim.settings = { ...sim.settings, ...(patch as Partial<Settings>) };
    emit("settings", sim.settings);
    return sim.settings;
  },
  set_enabled_command: ({ on }) => {
    sim.settings.enabled = on as boolean;
    emit("settings", sim.settings);
    if (on) {
      setEngine({ status: "starting" });
      setTimeout(() => setEngine({ status: "on" }), 700);
    } else setEngine({ status: "off" });
    return undefined;
  },
  watch_meters: ({ on }) => simulateMeters(on as boolean),
  model_download: () =>
    new Promise((resolve) => {
      let received = 0;
      const total = sim.model.size;
      const timer = setInterval(() => {
        received = Math.min(total, received + total / 40);
        emit("model-progress", { received, total, bytesPerSecond: 9.6e6, source: 0 });
        if (received >= total) {
          clearInterval(timer);
          sim.model = { ...sim.model, installed: true };
          resolve(sim.model);
        }
      }, 80);
    }),
  model_cancel: () => undefined,
  model_remove: () => ({ ...sim.model, installed: false }),
  vmic_status: () => sim.vmic,
  vmic_install: () => new Promise((r) => setTimeout(() => r((sim.vmic = { ...sim.vmic, state: "ready" })), 1200)),
  vmic_uninstall: () => ({ ...sim.vmic, state: "install" }),
  mic_monitor: ({ on }) => {
    clearInterval(sim.levelTimer);
    if (on) {
      let t = 0;
      sim.levelTimer = window.setInterval(() => {
        t += 0.05;
        emit("mic-level", Math.min(1, 0.06 + Math.abs(Math.sin(t * 2.3)) * 0.5 * Math.random()));
      }, 50);
    }
    return undefined;
  },
  test_record: ({ seconds }) => {
    emit("test", { phase: "recording", seconds: 0 });
    setTimeout(() => emit("test", { phase: "processing" }), (seconds as number) * 1000);
    setTimeout(() => {
      const original = Array.from({ length: 96 }, (_, i) => 0.12 + Math.random() * 0.12 + Math.max(0, Math.sin(i / 6)) * 0.5);
      const enhanced = original.map((v, i) => (Math.sin(i / 6) > 0.15 ? v * 0.9 : 0.015));
      emit("test", { phase: "ready", original, enhanced, seconds: seconds as number });
    }, (seconds as number) * 1000 + 900);
    return undefined;
  },
  test_play: ({ enhanced }) => {
    emit("test", { phase: "playing", enhanced: enhanced as boolean });
    setTimeout(() => emit("test", { phase: "stopped" }), 3000);
    return undefined;
  },
  test_stop: () => emit("test", { phase: "stopped" }),
  open_main: () => undefined,
  quit: () => undefined,
};

function guessPlatform(): Platform {
  const forced = new URLSearchParams(location.search).get("platform");
  if (forced) return forced;
  const ua = navigator.userAgent;
  if (ua.includes("Mac")) return "macos";
  if (ua.includes("Win")) return "windows";
  return "linux";
}

// ---------------------------------------------------------------- API

export async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri) return tauriInvoke<T>(command, args);
  const fn = simulated[command];
  if (!fn) throw new Error(`Unknown command ${command}`);
  return (await fn(args ?? {})) as T;
}

export async function on<K extends keyof EventMap>(event: K, handler: Handler<K>): Promise<UnlistenFn> {
  if (isTauri) return tauriListen(event, handler);
  const set = listeners.get(event) ?? new Set();
  set.add(handler as (payload: unknown) => void);
  listeners.set(event, set);
  return () => set.delete(handler as (payload: unknown) => void);
}

export const api = {
  snapshot: () => invoke<Snapshot>("snapshot"),
  ready: () => invoke<void>("app_ready"),
  devices: () => invoke<Devices>("devices"),
  updateSettings: (patch: Partial<Settings>) => invoke<Settings>("update_settings", { patch }),
  setEnabled: (on: boolean) => invoke<void>("set_enabled_command", { on }),
  watchMeters: (on: boolean) => invoke<void>("watch_meters", { on }),
  downloadModel: () => invoke<ModelInfo>("model_download"),
  cancelModel: () => invoke<void>("model_cancel"),
  removeModel: () => invoke<ModelInfo>("model_remove"),
  vmicStatus: () => invoke<VmicInfo>("vmic_status"),
  vmicInstall: () => invoke<VmicInfo>("vmic_install"),
  vmicUninstall: () => invoke<VmicInfo>("vmic_uninstall"),
  micMonitor: (on: boolean) => invoke<void>("mic_monitor", { on }),
  testRecord: (seconds: number) => invoke<void>("test_record", { seconds }),
  testPlay: (enhanced: boolean) => invoke<void>("test_play", { enhanced }),
  testStop: () => invoke<void>("test_stop"),
  openMain: () => invoke<void>("open_main"),
  quit: () => invoke<void>("quit"),
};

export async function openUrl(url: string) {
  if (isTauri) {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  } else window.open(url, "_blank", "noopener");
}

export function formatBytes(bytes: number) {
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(1)} MB`;
  if (bytes >= 1e3) return `${(bytes / 1e3).toFixed(0)} KB`;
  return `${bytes} B`;
}
