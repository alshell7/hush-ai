// Before/after player. Both recordings play together and the toggle only swaps which one
// is muted, so switching is instant and keeps the position. Nothing loads until it is near.
import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { PauseIcon, PlayIcon } from "../components/Icons";
import { Mochi } from "../components/Mochi";
import { Reveal, SectionHeader, Segmented } from "../components/ui";
import { useActive, useMediaQuery } from "../lib/hooks";

type Peaks = { noisy: number[]; hush: number[]; seconds: number };
type Mode = "noisy" | "hush";
type State = "idle" | "loading" | "playing" | "paused" | "error";

const BASE = import.meta.env.BASE_URL;
const FLAT: Peaks = { noisy: Array(200).fill(0.06), hush: Array(200).fill(0.06), seconds: 10 };
const STOPS: [number, [number, number, number]][] = [
  [0, [232, 115, 90]],
  [0.4, [217, 154, 30]],
  [0.75, [31, 157, 114]],
  [1, [23, 143, 138]],
];

function opalAt(t: number): string {
  for (let i = 1; i < STOPS.length; i++) {
    const [p1, c1] = STOPS[i];
    const [p0, c0] = STOPS[i - 1];
    if (t <= p1) {
      const k = (t - p0) / (p1 - p0);
      const c = c0.map((v, j) => Math.round(v + (c1[j] - v) * k));
      return `rgb(${c[0]},${c[1]},${c[2]})`;
    }
  }
  return "rgb(23,143,138)";
}

/** Downsample peaks to `count` bars, keeping the loudest value in each bucket. */
function bucket(values: number[], count: number): number[] {
  if (!values.length) return Array(count).fill(0.05);
  const out: number[] = [];
  for (let i = 0; i < count; i++) {
    const a = Math.floor((i * values.length) / count);
    const b = Math.max(a + 1, Math.floor(((i + 1) * values.length) / count));
    let m = 0;
    for (let j = a; j < b; j++) m = Math.max(m, values[j] ?? 0);
    out.push(Math.min(1, Math.max(0.04, m)));
  }
  return out;
}

const fmt = (s: number) => {
  const t = Math.max(0, Math.floor(s));
  return `${Math.floor(t / 60)}:${String(t % 60).padStart(2, "0")}`;
};

export function AudioDemo() {
  const sectionRef = useRef<HTMLElement>(null);
  const noisyRef = useRef<HTMLAudioElement>(null);
  const hushRef = useRef<HTMLAudioElement>(null);
  const progressRef = useRef<HTMLDivElement>(null);
  const headRef = useRef<HTMLDivElement>(null);
  const timeRef = useRef<HTMLSpanElement>(null);
  const seekRef = useRef<HTMLInputElement>(null);
  const modeRef = useRef<Mode>("noisy");
  const raf = useRef(0);
  const lastSync = useRef(0);

  const [peaks, setPeaks] = useState<Peaks>(FLAT);
  const [mode, setModeState] = useState<Mode>("noisy");
  const [state, setState] = useState<State>("idle");
  const near = useActive(sectionRef, "600px");
  const narrow = useMediaQuery("(max-width: 639px)");
  const count = narrow ? 64 : 120;
  const duration = peaks.seconds || 10;

  // Peaks are tiny; fetch them when the section is close.
  useEffect(() => {
    if (!near || peaks !== FLAT) return;
    fetch(`${BASE}samples/peaks.json`)
      .then((r) => (r.ok ? r.json() : Promise.reject(r.status)))
      .then((p: Peaks) => {
        if (Array.isArray(p.noisy) && Array.isArray(p.hush)) setPeaks({ noisy: p.noisy, hush: p.hush, seconds: Number(p.seconds) || 10 });
      })
      .catch(() => undefined);
  }, [near, peaks]);

  const bars = useMemo(() => bucket(mode === "hush" ? peaks.hush : peaks.noisy, count), [mode, peaks, count]);
  const colors = useMemo(() => Array.from({ length: count }, (_, i) => opalAt(i / (count - 1))), [count]);

  const els = () => [noisyRef.current!, hushRef.current!] as const;
  const active = () => (modeRef.current === "hush" ? hushRef.current! : noisyRef.current!);
  const other = () => (modeRef.current === "hush" ? noisyRef.current! : hushRef.current!);

  const paint = useCallback((t: number) => {
    const pct = Math.min(100, (t / duration) * 100);
    if (progressRef.current) progressRef.current.style.clipPath = `inset(0 ${100 - pct}% 0 0)`;
    if (headRef.current) headRef.current.style.left = `${pct}%`;
    if (timeRef.current) timeRef.current.textContent = fmt(t);
    if (seekRef.current) {
      seekRef.current.value = String(t);
      seekRef.current.setAttribute("aria-valuetext", `${fmt(t)} of ${fmt(duration)}`);
    }
  }, [duration]);

  const stopLoop = () => cancelAnimationFrame(raf.current);
  const loop = useCallback(() => {
    const a = active();
    paint(a.currentTime);
    // Keep the muted twin within a few milliseconds of the one you hear: a big gap is
    // closed with a seek, a small one by nudging its speed (it is muted, so nobody hears it).
    const now = performance.now();
    if (now - lastSync.current > 200) {
      lastSync.current = now;
      const b = other();
      const drift = b.currentTime - a.currentTime;
      if (b.seeking) {
        /* wait for the seek */
      } else if (Math.abs(drift) > 0.15) {
        b.currentTime = a.currentTime;
      } else {
        b.playbackRate = Math.abs(drift) < 0.008 ? 1 : 1 - Math.max(-0.1, Math.min(0.1, drift * 2));
      }
    }
    raf.current = requestAnimationFrame(loop);
  }, [paint]);

  useEffect(() => stopLoop, []);
  useEffect(() => paint(active().currentTime), [paint]);

  const setMode = (next: Mode) => {
    modeRef.current = next;
    setModeState(next);
    const [n, h] = els();
    n.playbackRate = h.playbackRate = 1;
    n.muted = next !== "noisy";
    h.muted = next !== "hush";
  };

  const play = async () => {
    const [n, h] = els();
    if (state === "error") {
      // Try again from scratch, in case the files have appeared or the network is back.
      n.load();
      h.load();
    }
    n.muted = modeRef.current !== "noisy";
    h.muted = modeRef.current !== "hush";
    other().currentTime = active().currentTime;
    setState("loading");
    try {
      // Both calls happen inside the click, so browsers allow sound.
      await Promise.all([n.play(), h.play()]);
      // Line up the muted twin; never seek the one you hear.
      other().currentTime = active().currentTime;
      setState("playing");
      stopLoop();
      raf.current = requestAnimationFrame(loop);
    } catch (e) {
      n.pause();
      h.pause();
      setState((e as DOMException)?.name === "NotSupportedError" ? "error" : "paused");
    }
  };

  const pause = () => {
    const [n, h] = els();
    n.pause();
    h.pause();
    n.playbackRate = h.playbackRate = 1;
    stopLoop();
    setState("paused");
    paint(active().currentTime);
  };

  const seek = (t: number) => {
    const clamped = Math.max(0, Math.min(duration, t));
    const [n, h] = els();
    n.currentTime = clamped;
    h.currentTime = clamped;
    paint(clamped);
  };

  const onSeekKey = (e: KeyboardEvent<HTMLInputElement>) => {
    const now = active().currentTime;
    const map: Record<string, number> = { ArrowRight: now + 1, ArrowUp: now + 1, ArrowLeft: now - 1, ArrowDown: now - 1, Home: 0, End: duration };
    if (e.key in map) {
      e.preventDefault();
      seek(map[e.key]);
    }
  };

  const onEnded = () => {
    const [n, h] = els();
    n.pause();
    h.pause();
    n.playbackRate = h.playbackRate = 1;
    stopLoop();
    seek(0);
    setState("paused");
  };

  const playing = state === "playing" || state === "loading";

  return (
    <section ref={sectionRef} id="demo" aria-labelledby="demo-title" className="relative py-20 sm:py-28">
      <div className="wrap">
        <SectionHeader
          id="demo-title"
          eyebrow="Listen"
          title={
            <>
              Hear the <span className="opal-text">difference</span>
            </>
          }
          sub="Press play, then switch between the original and Hush while it plays."
        />

        <Reveal className="mx-auto mt-12 max-w-[920px]">
          <div className="glass p-5 sm:p-8">
            <div className="flex flex-col gap-5 sm:flex-row sm:items-center">
              <div className="flex items-center gap-4">
                <button
                  type="button"
                  onClick={playing ? pause : play}
                  aria-label={playing ? "Pause sample" : "Play sample"}
                  className="relative grid h-16 w-16 shrink-0 cursor-pointer place-items-center rounded-full bg-ink text-white transition-transform duration-200 active:scale-95 disabled:cursor-not-allowed disabled:opacity-40 sm:h-[72px] sm:w-[72px]"
                  style={{ boxShadow: "inset 0 1px 0 rgba(255,255,255,.18), 0 14px 30px -12px rgba(30,27,24,.7)" }}
                >
                  {state === "loading" && (
                    <span
                      className="absolute -inset-1 rounded-full"
                      style={{
                        background: "var(--h-opal-conic)",
                        animation: "h-spin 1.2s linear infinite",
                        WebkitMask: "radial-gradient(farthest-side, transparent calc(100% - 3px), #000 calc(100% - 2.5px))",
                        mask: "radial-gradient(farthest-side, transparent calc(100% - 3px), #000 calc(100% - 2.5px))",
                      }}
                    />
                  )}
                  {playing ? <PauseIcon size={26} /> : <PlayIcon size={26} className="translate-x-[1.5px]" />}
                </button>
                <div className="flex min-w-0 items-center gap-3">
                  <Mochi size={44} mood={mode === "hush" ? "happy" : "oops"} breathe={false} />
                  <div className="min-w-0">
                    <div className="text-[17px] font-[640] tracking-[-0.02em]">{mode === "hush" ? "With Hush" : "Original"}</div>
                    <div className="text-[13.5px] tabular-nums text-muted">
                      <span ref={timeRef}>0:00</span> / {fmt(duration)}
                    </div>
                  </div>
                </div>
              </div>
              <div className="sm:ml-auto sm:w-[300px]">
                <Segmented
                  label="Which recording to hear"
                  size="lg"
                  value={mode}
                  onChange={setMode}
                  options={[
                    { value: "noisy", label: "Original" },
                    { value: "hush", label: "With Hush" },
                  ]}
                />
              </div>
            </div>

            <div className="group relative mt-7 h-[110px] rounded-[18px] sm:h-[150px] [&:has(input:focus-visible)]:outline-2 [&:has(input:focus-visible)]:outline-offset-4 [&:has(input:focus-visible)]:outline-[var(--focus)]">
              <Bars bars={bars} colors={null} />
              <div ref={progressRef} className="absolute inset-0" style={{ clipPath: "inset(0 100% 0 0)" }}>
                <Bars bars={bars} colors={mode === "hush" ? colors : null} strong />
              </div>
              <div ref={headRef} aria-hidden className="pointer-events-none absolute inset-y-1 left-0 w-[2px] -translate-x-1/2 rounded-full bg-ink/70" />
              <input
                ref={seekRef}
                type="range"
                className="seek absolute inset-0 h-full w-full cursor-pointer"
                min={0}
                max={duration}
                step="any"
                defaultValue={0}
                aria-label="Position in the recording"
                onInput={(e) => seek(Number(e.currentTarget.value))}
                onKeyDown={onSeekKey}
              />
            </div>

            <div className="mt-5 flex flex-col gap-2 text-[13px] leading-relaxed text-muted sm:flex-row sm:items-center sm:justify-between">
              <p>NVIDIA's sample recording, before and after Hush. Sample © NVIDIA, NVIDIA One-Way Noncommercial License.</p>
              {state === "error" && (
                <p role="status" className="shrink-0 font-[560] text-amber">
                  The sample audio isn't available yet.
                </p>
              )}
            </div>
          </div>
        </Reveal>
      </div>

      <audio ref={noisyRef} src={`${BASE}samples/noisy.mp3`} preload="none" onEnded={onEnded} onError={() => setState("error")} />
      <audio ref={hushRef} src={`${BASE}samples/hush.mp3`} preload="none" muted onError={() => setState("error")} />
    </section>
  );
}

function Bars({ bars, colors, strong }: { bars: number[]; colors: string[] | null; strong?: boolean }) {
  return (
    <div aria-hidden className="absolute inset-0 flex items-center gap-[2px] sm:gap-[3px]">
      {bars.map((v, i) => (
        <span
          key={i}
          className="min-h-[3px] flex-1 rounded-full"
          style={{
            height: `${v * 100}%`,
            background: colors ? colors[i] : strong ? "#4a453f" : "rgba(30,27,24,.13)",
            transition: `height .45s var(--h-ease) ${i * 3}ms, background-color .3s`,
          }}
        />
      ))}
    </div>
  );
}
