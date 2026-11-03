// Self-test steps for Studio and Meetings (HUSH_SELFTEST=1): enhance a generated file and
// save it, and, when HUSH_TEST_ICAL holds a calendar address, check and connect it.
import { features, type CalendarInfo, type StudioEvent } from "./features";
import { invoke, on } from "./ipc";

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
