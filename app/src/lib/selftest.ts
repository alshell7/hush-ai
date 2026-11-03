// End-to-end check of the real backend, run with HUSH_SELFTEST=1. It drives the same
// commands as the UI and prints each result to the terminal, then quits.
import { api, invoke, on, type Meters, type TestEvent } from "./ipc";

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
const log = (line: string) => invoke("selftest_report", { line });

async function until<T>(what: string, check: () => T | undefined, ms: number): Promise<T> {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    const value = check();
    if (value !== undefined) return value;
    await wait(100);
  }
  throw new Error(`timeout: ${what}`);
}

let started = false;

export async function selftest() {
  if (started) return;
  started = true;
  try {
    let engine = (await api.snapshot()).engine;
    on("engine", (e) => {
      if (e.status !== engine.status) log(`  engine: ${e.status}${e.message ? ` (${e.message})` : ""}`);
      engine = e;
    });
    const meters: Meters[] = [];
    on("meters", (m) => meters.push(m));
    let test: TestEvent | null = null;
    on("test", (e) => (test = e));

    const snap = await api.snapshot();
    await log(`snapshot: platform=${snap.platform} version=${snap.version} model=${snap.model.installed} onboarded=${snap.settings.onboarded}`);
    const devices = await api.devices();
    await log(`devices: ${devices.inputs.length} inputs, ${devices.outputs.length} outputs: ${devices.inputs.map((d) => d.label).join(" | ")}`);
    const vmic = await api.vmicStatus();
    await log(`vmic: ${vmic.state} ${vmic.driver} ${vmic.detail}`);
    if (!snap.model.installed) {
      let last = 0;
      on("model-progress", (p) => (last = p.received));
      const t = Date.now();
      const info = await api.downloadModel();
      await log(`model download: ${info.installed} ${last} bytes in ${((Date.now() - t) / 1000).toFixed(1)} s`);
    }
    await api.updateSettings({ onboarded: true, licenseAccepted: true });
    await api.watchMeters(true);
    await api.setEnabled(true);
    await until("engine on", () => (engine.status === "on" ? true : engine.status === "error" ? (() => { throw new Error(engine.message ?? "error"); })() : undefined), 20000);
    await log(`engine on: threads=${engine.threads} in=${engine.inputRate} model=${engine.modelRate} out=${engine.outputRate}`);
    meters.length = 0;
    await wait(5000);
    const peakIn = Math.max(...meters.map((m) => m.input));
    const loads = meters.map((m) => m.load).filter((v) => v > 0);
    await log(`meters: ${meters.length} readings in 5 s, input peak ${peakIn.toFixed(3)}, load ${(loads.reduce((a, b) => a + b, 0) / Math.max(1, loads.length)).toFixed(2)}, dropped ${meters.at(-1)?.dropped}`);
    await api.updateSettings({ quality: "balanced" });
    await wait(400);
    await until("restart", () => (engine.status === "on" && engine.modelRate === 24000 ? true : undefined), 20000);
    await log(`quality change restarted: model rate ${engine.modelRate}`);
    await api.updateSettings({ quality: "light" });
    await until("restart light", () => (engine.status === "on" && engine.modelRate === 16000 ? true : undefined), 20000);
    await api.setEnabled(false);
    await until("engine off", () => (engine.status === "off" ? true : undefined), 5000);
    await log("engine off");
    await api.testRecord(2);
    await until("test ready", () => ((test as TestEvent | null)?.phase === "ready" || (test as TestEvent | null)?.phase === "error" ? true : undefined), 20000);
    const t = test as TestEvent | null;
    await log(`test record: ${t?.phase} ${t?.phase === "ready" ? `${t.seconds.toFixed(2)} s, original peak ${Math.max(...t.original).toFixed(3)}, enhanced peak ${Math.max(...t.enhanced).toFixed(3)}` : t?.phase === "error" ? t.message : ""}`);
    await api.setEnabled(true);
    await until("engine on again", () => (engine.status === "on" ? true : undefined), 20000);
    await log("engine on again");
    await log("PASS");
  } catch (error) {
    await log(`FAIL: ${error}`);
  }
  await log("DONE");
}
