// Live level meters. Readings arrive 20 times per second and are drawn straight to a
// canvas, so React does not re-render for each one. Meters stop while the window is hidden.
import { useEffect, useRef } from "react";
import { api, on, type Meters } from "../lib/ipc";

type Listener = (m: Meters) => void;
const listeners = new Set<Listener>();
let started = false;
let watching = false;

function setWatching(next: boolean) {
  if (next === watching) return;
  watching = next;
  api.watchMeters(next).catch(() => {});
}

function update() {
  setWatching(listeners.size > 0 && document.visibilityState === "visible");
}

function start() {
  if (started) return;
  started = true;
  on("meters", (m) => listeners.forEach((fn) => fn(m)));
  document.addEventListener("visibilitychange", update);
  window.addEventListener("focus", update);
}

/** Receive meter readings while mounted and visible. */
export function useMeters(fn: Listener) {
  const ref = useRef(fn);
  ref.current = fn;
  useEffect(() => {
    start();
    const listener: Listener = (m) => ref.current(m);
    listeners.add(listener);
    update();
    return () => {
      listeners.delete(listener);
      update();
    };
  }, []);
}

/** Peak to a 0–1 display scale: about −60 dB to 0 dB. */
export function toDisplay(peak: number) {
  if (peak <= 0.001) return 0;
  return Math.max(0, Math.min(1, 1 + Math.log10(peak) / 3));
}

const HISTORY = 96;

/** A scrolling strip of level bars. Bars are about 3 px wide, as many as fit. */
export function LevelStrip({
  source,
  color,
  height = 40,
  idle = false,
}: {
  source: "input" | "output";
  color: string;
  height?: number;
  idle?: boolean;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const history = useRef<number[]>(new Array(HISTORY).fill(0));
  const frame = useRef(0);

  const draw = () => {
    frame.current = 0;
    const el = canvas.current;
    if (!el) return;
    const ctx = el.getContext("2d");
    if (!ctx) return;
    const dpr = window.devicePixelRatio || 1;
    const w = el.clientWidth;
    const h = el.clientHeight;
    // Not laid out yet (first frame, hidden window): nothing to draw.
    if (w < 16 || h < 4) return;
    if (el.width !== Math.round(w * dpr) || el.height !== Math.round(h * dpr)) {
      el.width = Math.round(w * dpr);
      el.height = Math.round(h * dpr);
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    const gap = 2;
    const count = Math.min(HISTORY, Math.max(4, Math.floor((w + gap) / 5)));
    const bar = (w - gap * (count - 1)) / count;
    const values = history.current.slice(HISTORY - count);
    ctx.fillStyle = color;
    values.forEach((v, i) => {
      const bh = Math.max(Math.min(3, h), v * (h - 2));
      const x = i * (bar + gap);
      const y = (h - bh) / 2;
      ctx.globalAlpha = 0.22 + 0.78 * (i / count);
      ctx.beginPath();
      ctx.roundRect(x, y, bar, bh, Math.max(0, Math.min(bar, bh) / 2));
      ctx.fill();
    });
    ctx.globalAlpha = 1;
  };

  useMeters((m) => {
    const value = idle ? 0 : toDisplay(source === "input" ? m.input : m.output);
    history.current.push(value);
    history.current.shift();
    if (!frame.current) frame.current = requestAnimationFrame(draw);
  });

  useEffect(() => {
    history.current.fill(0);
    draw();
    return () => cancelAnimationFrame(frame.current);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [idle]);

  return <canvas ref={canvas} className="block w-full" style={{ height }} aria-hidden />;
}

/** A small live dot that scales with the level. */
export function LevelDot({ source, size = 10 }: { source: "input" | "output"; size?: number }) {
  const dot = useRef<HTMLSpanElement>(null);
  useMeters((m) => {
    const v = toDisplay(source === "input" ? m.input : m.output);
    if (dot.current) dot.current.style.transform = `scale(${0.8 + v * 0.9})`;
  });
  return (
    <span
      ref={dot}
      className="inline-block rounded-full transition-transform duration-100"
      style={{ width: size, height: size, background: "var(--h-opal-strong)" }}
    />
  );
}

/** One level bar for the microphone check (mic-level events). */
export function MicLevel({ level }: { level: number }) {
  const v = toDisplay(level);
  const segments = 24;
  return (
    <div className="flex h-8 items-center gap-[3px]">
      {Array.from({ length: segments }, (_, i) => {
        const lit = i / segments < v;
        return (
          <span
            key={i}
            className="h-full flex-1 rounded-full transition-colors duration-75"
            style={{
              background: lit ? (i > segments * 0.85 ? "var(--h-coral)" : i > segments * 0.6 ? "#e7b244" : "var(--h-green)") : "rgba(30,27,24,0.08)",
            }}
          />
        );
      })}
    </div>
  );
}
