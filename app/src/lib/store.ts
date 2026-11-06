// One small app store shared by every component of a window. Updates come from backend
// events; components read slices with `useApp`.
import { useSyncExternalStore } from "react";
import { api, isTauri, on, type Devices, type EngineState, type Models, type Platform, type Settings, type VmicInfo } from "./ipc";

export interface AppStore {
  ready: boolean;
  settings: Settings | null;
  engine: EngineState | null;
  models: Models | null;
  vmic: VmicInfo | null;
  devices: Devices | null;
  platform: Platform;
  version: string;
  /** A newer version: its notes come from the release. */
  update: { version: string; state: "available" | "downloading" | "ready"; notes?: string | null } | null;
}

let state: AppStore = {
  ready: false,
  settings: null,
  engine: null,
  models: null,
  vmic: null,
  devices: null,
  platform: "windows",
  version: "",
  update: null,
};

const subscribers = new Set<() => void>();

export function setStore(patch: Partial<AppStore>) {
  state = { ...state, ...patch };
  subscribers.forEach((fn) => fn());
}

export function getStore() {
  return state;
}

function subscribe(fn: () => void) {
  subscribers.add(fn);
  return () => subscribers.delete(fn);
}

export function useApp<T>(select: (s: AppStore) => T): T {
  return useSyncExternalStore(subscribe, () => select(state));
}

let started = false;

/** Load the first snapshot and follow backend events. Safe to call more than once. */
export async function startStore() {
  if (started) return;
  started = true;
  // Subscribe first, then read the snapshot, so no state change falls in between.
  await Promise.all([
    on("engine", (engine) => setStore({ engine })),
    on("settings", (settings) => setStore({ settings })),
    on("models", (models) => setStore({ models })),
  ]);
  const snapshot = await api.snapshot();
  setStore({
    ready: true,
    settings: snapshot.settings,
    engine: snapshot.engine,
    models: snapshot.models,
    platform: snapshot.platform,
    version: snapshot.version,
  });
  document.documentElement.dataset.platform = snapshot.platform;
  // Previews: `?update=1` shows a downloaded update and its notes.
  if (!isTauri && new URLSearchParams(location.search).get("update") === "1") {
    setStore({ update: { version: "1.3.0", state: "ready", notes: "- **Smoother notch.** It morphs without a glitch.\n- **Hush AI Speaker on Windows.** Hush names a free cable for you.\n\n<!-- downloads -->\n**Download** below." } });
  }
  refreshDevices();
  refreshVmic();
}

export async function refreshDevices() {
  try {
    setStore({ devices: await api.devices() });
  } catch {
    /* devices stay as they were; the next refresh tries again */
  }
}

export async function refreshVmic() {
  try {
    setStore({ vmic: await api.vmicStatus() });
  } catch {
    /* unknown until the next refresh */
  }
}

export async function saveSettings(patch: Partial<Settings>) {
  const current = state.settings;
  if (current) setStore({ settings: { ...current, ...patch } });
  setStore({ settings: await api.updateSettings(patch) });
}
