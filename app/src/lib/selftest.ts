// End-to-end check of the real backend, run with HUSH_SELFTEST=1. It drives the same
// commands as the UI for each voice model (and downloads a missing one), prints each result
// to the terminal, then quits.
import { api, invoke, on, type Engine, type EngineState, type Meters, type TestEvent } from "./ipc";
import { selftestFeatures } from "./selftestFeatures";

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
    const state = { engine: () => engine, meters, test: () => test, clearTest: () => (test = null) };

    const snap = await api.snapshot();
    await log(`snapshot: platform=${snap.platform} version=${snap.version} engine=${snap.settings.engine} models=deepFilter:${snap.models.deepFilter.installed},reuse:${snap.models.reuse.installed} onboarded=${snap.settings.onboarded}`);
    const devices = await api.devices();
    await log(`devices: ${devices.inputs.length} inputs, ${devices.outputs.length} outputs: ${devices.inputs.map((d) => d.label).join(" | ")}`);
    const vmic = await api.vmicStatus();
    await log(`vmic: ${vmic.state} ${vmic.driver} ${vmic.detail}`);
    await api.updateSettings({ onboarded: true, licenseAccepted: true });
    await api.watchMeters(true);
    // Every engine, the user's own last, so it is the one left on.
    const own = snap.settings.engine;
    const order: Engine[] = own === "reuse" ? ["deepFilter", "reuse"] : ["reuse", "deepFilter"];
    for (const kind of order) await checkEngine(kind, state);
    await api.setEnabled(true);
    await until("engine on again", () => (engine.status === "on" && engine.engine === own ? true : undefined), 20000);
    await log(`engine on again: ${engine.engine}`);
    await selftestFeatures(log);
    await log("PASS");
  } catch (error) {
    await log(`FAIL: ${error}`);
  }
  await log("DONE");
}

interface State {
  engine: () => EngineState;
  meters: Meters[];
  test: () => TestEvent | null;
  clearTest: () => void;
}

async function checkEngine(kind: Engine, state: State) {
  await log(`== ${kind}`);
  const models = (await api.snapshot()).models;
  if (!models[kind].installed) {
    let last = 0;
    const stop = await on("model-progress", (p) => (last = p.received));
    const t = Date.now();
    const info = await api.downloadModel(kind);
    stop();
    await log(`model download: ${info[kind].installed} ${last} bytes in ${((Date.now() - t) / 1000).toFixed(1)} s`);
  }
  await api.setEnabled(false);
  // Off still carries the voice: a pass-through session, so apps keep hearing you.
  await until("pass-through", () => (state.engine().status === "on" && state.engine().noise === false ? true : undefined), 10000);
  await api.updateSettings({ engine: kind });
  await api.setEnabled(true);
  const failed = () => {
    throw new Error(state.engine().message ?? "error");
  };
  await until("engine on", () => {
    const e = state.engine();
    return e.status === "on" && e.engine === kind ? true : e.status === "error" ? failed() : undefined;
  }, 20000);
  const e = state.engine();
  await log(`engine on: ${e.engine} threads=${e.threads} in=${e.inputRate} model=${e.modelRate} out=${e.outputRate}`);
  state.meters.length = 0;
  await wait(5000);
  const peakIn = Math.max(...state.meters.map((m) => m.input));
  const loads = state.meters.map((m) => m.load).filter((v) => v > 0);
  if (loads.length === 0) throw new Error("no meter readings");
  await log(`meters: ${state.meters.length} readings in 5 s, input peak ${peakIn.toFixed(3)}, load ${(loads.reduce((a, b) => a + b, 0) / loads.length).toFixed(3)}, dropped ${state.meters.at(-1)?.dropped}`);
  if (kind === "reuse") {
    await api.updateSettings({ quality: "balanced" });
    await wait(400);
    await until("restart", () => (state.engine().status === "on" && state.engine().modelRate === 24000 ? true : undefined), 20000);
    await log(`quality change restarted: model rate ${state.engine().modelRate}`);
    await api.updateSettings({ quality: "light" });
    await until("restart light", () => (state.engine().status === "on" && state.engine().modelRate === 16000 ? true : undefined), 20000);
  }
  await api.setEnabled(false);
  // Off still carries the voice: a pass-through session, so apps keep hearing you.
  await until("pass-through", () => (state.engine().status === "on" && state.engine().noise === false ? true : undefined), 10000);
  await log("noise cancellation off: Hush AI Microphone passes the voice through");
  state.clearTest();
  await api.testRecord(2);
  await until("test ready", () => (state.test()?.phase === "ready" || state.test()?.phase === "error" ? true : undefined), 20000);
  const t = state.test();
  if (t?.phase === "error") throw new Error(`test record: ${t.message}`);
  await log(`test record: ${t?.phase} ${t?.phase === "ready" ? `${t.seconds.toFixed(2)} s, original peak ${Math.max(...t.original).toFixed(3)}, enhanced peak ${Math.max(...t.enhanced).toFixed(3)}` : ""}`);
}
