// Screenshot a page in headless Edge/Chrome over the DevTools protocol, in real time so
// animations finish. Usage: node scripts/screenshot.mjs <url> <out.png> <width> <height> [wait ms]
import { spawn } from "node:child_process";
import { writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const [url, out, width = "1280", height = "800", wait = "4500"] = process.argv.slice(2);
const browser =
  process.env.BROWSER ??
  (process.platform === "win32"
    ? "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"
    : process.platform === "darwin"
      ? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
      : "google-chrome");
const port = 9300 + Math.floor(Math.random() * 500);
const proc = spawn(browser, ["--headless=new", "--disable-gpu", "--hide-scrollbars", `--remote-debugging-port=${port}`, `--user-data-dir=${join(tmpdir(), `shot-${port}`)}`, "about:blank"], { stdio: "ignore" });

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let target;
for (let i = 0; i < 50 && !target; i++) {
  await sleep(200);
  try {
    target = (await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()).find((t) => t.type === "page");
  } catch {}
}
const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((r) => ws.addEventListener("open", r));
let id = 0;
const pending = new Map();
ws.addEventListener("message", (e) => {
  const msg = JSON.parse(e.data);
  if (msg.id && pending.has(msg.id)) pending.get(msg.id)(msg.result);
});
const send = (method, params = {}) => new Promise((r) => { pending.set(++id, r); ws.send(JSON.stringify({ id, method, params })); });

await send("Emulation.setDeviceMetricsOverride", { width: +width, height: +height, deviceScaleFactor: 2, mobile: !!process.env.MOBILE });
if (process.env.MOBILE) await send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
await send("Page.enable");
await send("Page.navigate", { url });
await sleep(+wait);
// STOPS="0,#features,2400" captures one image per scroll stop: out-0.png, out-1.png, …
const stops = process.env.STOPS ? process.env.STOPS.split(",") : null;
if (!stops) {
  const { data } = await send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
  writeFileSync(out, Buffer.from(data, "base64"));
} else {
  for (const [i, stop] of stops.entries()) {
    const target = stop.startsWith("#") ? `(document.querySelector(${JSON.stringify(stop)})?.getBoundingClientRect().top ?? 0) + scrollY - 70` : stop;
    await send("Runtime.evaluate", { expression: `document.documentElement.style.scrollBehavior='auto'; scrollTo(0, ${target})` });
    await sleep(1600);
    const { data } = await send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
    writeFileSync(out.replace(/\.png$/, `-${i}.png`), Buffer.from(data, "base64"));
  }
}
ws.close();
proc.kill();
console.log(`saved ${out}`);
