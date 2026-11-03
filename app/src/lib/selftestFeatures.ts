// Self-test steps for Studio, Meetings and Live Translate (HUSH_SELFTEST=1): enhance a
// generated file and save it; with HUSH_TEST_ICAL, check and connect a calendar; with
// HUSH_TEST_GEMINI_KEY, save the key and open a live translation in the running session.
import { features, type CalendarInfo, type StudioEvent, type TranslateInfo } from "./features";
import { api, invoke, on } from "./ipc";

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

export async function selftestFeatures(log: (line: string) => Promise<unknown>) {
  let studio: StudioEvent | null = null;
  const stop = await on("studio", (e) => (studio = e));
  try {
    const input = await invoke<string>("selftest_fixture", { kind: "studio" });
    await features.studioOpen(input);
    const end = Date.now() + 120_000;
    let last = "";
    while (Date.now() < end) {
      const e = studio as StudioEvent | null;
      if (e?.phase === "ready" || e?.phase === "error") break;
      if (e && e.phase !== last) last = e.phase;
      await wait(100);
    }
    const e = studio as StudioEvent | null;
    if (e?.phase !== "ready") throw new Error(`studio: ${e?.phase === "error" ? e.message : "timeout"}`);
    const peak = (values: number[]) => Math.max(...values).toFixed(3);
    await log(`studio: ${e.seconds.toFixed(2)} s with ${e.engine} in ${e.elapsed.toFixed(2)} s, peaks original ${peak(e.originalPeaks)} enhanced ${peak(e.enhancedPeaks)}`);
    const target = await invoke<string>("selftest_fixture", { kind: "save" });
    await features.studioSave(target);
    await log("studio: saved");
  } finally {
    stop();
  }

  await selftestTranslate(log);

  const address = await invoke<string | null>("selftest_fixture", { kind: "ical" });
  if (!address) {
    await log("calendar: skipped (no HUSH_TEST_ICAL)");
    return;
  }
  const preview = await features.calendarCheck(address);
  await log(`calendar check: named=${preview.name !== null} ${preview.meetings} events this week, ${preview.calls} with call links`);
  const info: CalendarInfo = await features.calendarConnect(address);
  await log(`calendar connect: ${info.status}, ${info.meetings.length} upcoming, apps ${[...new Set(info.meetings.map((m) => m.app).filter(Boolean))].join(",") || "none"}`);
  const after = await features.calendarDisconnect();
  await log(`calendar disconnect: connected=${after.connected}`);
}

async function selftestTranslate(log: (line: string) => Promise<unknown>) {
  const key = await invoke<string | null>("selftest_fixture", { kind: "gemini" });
  if (!key) {
    await log("translate: skipped (no HUSH_TEST_GEMINI_KEY)");
    return;
  }
  const saved = await features.translateSetKey(key);
  await log(`translate key: saved=${saved.hasKey}`);
  let info: TranslateInfo | null = null;
  const stop = await on("translate", (e) => (info = e));
  try {
    await features.translateSet(true);
    const end = Date.now() + 30_000;
    while (Date.now() < end) {
      const now = info as TranslateInfo | null;
      if (now?.me.status === "on" || now?.me.status === "error") break;
      await wait(100);
    }
    const now = info as TranslateInfo | null;
    if (now?.me.status !== "on") throw new Error(`translate: ${now?.me.message ?? now?.me.status ?? "no state"}`);
    await log("translate: live session open with Google");
    await wait(3000);
    const after = (info as TranslateInfo | null)!;
    await log(`translate: still ${after.me.status}, captions ${after.me.original.length}/${after.me.translated.length} chars`);
    // Live Translate keeps going with noise cancellation off: the session passes the voice through.
    await api.setEnabled(false);
    await wait(4000);
    const bypass = await api.snapshot();
    const still = (info as TranslateInfo | null)!;
    if (still.me.status !== "on" || bypass.engine.status !== "on") throw new Error(`translate without noise cancellation: translate ${still.me.status}, session ${bypass.engine.status}`);
    await log("translate: still on with noise cancellation off (pass-through session)");
    await features.translateSet(false);
    await wait(1500);
    await log(`translate off: session ${(await api.snapshot()).engine.status}`);
    await api.setEnabled(true);
    // Their voice: Hush AI Speaker, or listening along to the speakers.
    const listen = await features.listenInfo();
    await log(`listen: mode ${listen.mode}${listen.device ? ` (${listen.device})` : ""}, running ${listen.running}`);
    if (listen.mode !== "none") {
      await features.translateSetSide("them", true);
      const until = Date.now() + 30_000;
      while (Date.now() < until && !["on", "error"].includes((info as TranslateInfo | null)?.them.status ?? "")) await wait(100);
      const them = (info as TranslateInfo | null)!.them;
      if (them.status !== "on") throw new Error(`their voice: ${them.message ?? them.status}`);
      const running = (await features.listenInfo()).running;
      if (!running) throw new Error("their voice: the listener is not running");
      await log("their voice: live session open with Google, listener running");
      await features.translateSetSide("them", false);
    }
    const list = await features.conversationsList();
    await log(`conversations: ${list.length} kept`);
  } finally {
    stop();
    const removed = await features.translateRemoveKey();
    await log(`translate key removed: ${!removed.hasKey}`);
  }
}
