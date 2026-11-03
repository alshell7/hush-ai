// Automatic updates from GitHub Releases, signed with the Hush AI update key.
// Checks shortly after start and every six hours; downloads quietly; asks before restarting.
import { isTauri } from "./ipc";
import { getStore, setStore } from "./store";

type Update = Awaited<ReturnType<typeof import("@tauri-apps/plugin-updater").check>>;
let pending: Update = null;
let timer = 0;

export async function checkForUpdate(manual = false): Promise<"none" | "available" | "error"> {
  if (!isTauri) return "none";
  try {
    const { check } = await import("@tauri-apps/plugin-updater");
    const update = await check();
    if (!update) return "none";
    pending = update;
    const notes = update.body ?? null;
    setStore({ update: { version: update.version, state: "downloading", notes } });
    await update.download();
    setStore({ update: { version: update.version, state: "ready", notes } });
    return "available";
  } catch (error) {
    if (manual) throw error;
    return "error";
  }
}

export async function installUpdate() {
  if (!pending) return;
  await pending.install();
  const { relaunch } = await import("@tauri-apps/plugin-process");
  await relaunch();
}

export function startUpdateChecks() {
  if (!isTauri || timer) return;
  const run = () => {
    if (getStore().settings?.checkUpdates && getStore().update?.state !== "ready") checkForUpdate();
  };
  setTimeout(run, 8000);
  timer = window.setInterval(run, 6 * 60 * 60 * 1000);
}
