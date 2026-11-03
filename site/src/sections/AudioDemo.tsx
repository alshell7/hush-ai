// "Hear the difference": an A/B player for several scenes. All versions of a scene play in
// sync through Web Audio and the toggle crossfades between them, so the switch is instant and
// gapless. Scenes come from samples/demos.json; without it, the original sample plays.
import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { BoltIcon, CarIcon, CupIcon, FanIcon, HeartIcon, KeyboardIcon, MicIcon, PauseIcon, PawIcon, PlayIcon, SparkleIcon } from "../components/Icons";
import { Mochi } from "../components/Mochi";
import { Reveal, SectionHeader, Segmented } from "../components/ui";
import { AbPlayer, type Version } from "../lib/abplayer";
import { useActive, useMediaQuery } from "../lib/hooks";
import { docHref } from "../lib/route";

const BASE = import.meta.env.BASE_URL;
const SAMPLES = `${BASE}samples/`;

type EngineId = "deepfilter" | "reuse";
type Scores = { pesq: number; stoi: number; sisdr: number };
export type Scene = {
  id: string;
  title: string;
  place?: string;
  speaker?: string;
  duration?: number;
  snr?: number;
  files: Partial<Record<Version, string>>;
  peaks: Partial<Record<Version, number[]>>;
  metrics?: Partial<Record<Version, Scores>>;
};
type Manifest = { version: number; engines?: Partial<Record<EngineId, unknown>>; scenes: Scene[] };

const FALLBACK: Scene = {
  id: "mic-test",
  title: "Mic test",
  place: "NVIDIA's sample recording",
  duration: 6.5,
  files: { noisy: "noisy.mp3", reuse: "hush.mp3" },
  peaks: {},
};
const ENGINE_NAMES: Record<EngineId, string> = { deepfilter: "DeepFilterNet 3", reuse: "NVIDIA RE-USE" };

function engineName(manifest: Manifest | null, id: EngineId): string {
  const e = manifest?.engines?.[id];
  if (typeof e === "string" && e.trim()) return e;
  if (e && typeof e === "object") {
    const o = e as { name?: unknown; label?: unknown };
    if (typeof o.name === "string") return o.name;
    if (typeof o.label === "string") return o.label;
  }
  return ENGINE_NAMES[id];
}

function validScene(s: unknown): s is Scene {
  const x = s as Scene;
  return !!x && typeof x.id === "string" && typeof x.title === "string" && !!x.files && typeof x.files.noisy === "string" && (!!x.files.deepfilter || !!x.files.reuse);
}

function sceneIcon(scene: Scene): ReactNode {
  const k = `${scene.id} ${scene.title}`.toLowerCase();
  if (/dog|bark|pet/.test(k)) return <PawIcon size={16} />;
  if (/key|typing/.test(k)) return <KeyboardIcon size={16} />;
  if (/caf|restaurant|chatter|bar\b|crowd/.test(k)) return <CupIcon size={16} />;
  if (/street|traffic|car|road|city/.test(k)) return <CarIcon size={16} />;
  if (/fan|hvac|air|hum/.test(k)) return <FanIcon size={16} />;
  if (/baby|child|cry/.test(k)) return <HeartIcon size={16} />;
  if (/construct|hammer|drill|bang|build/.test(k)) return <BoltIcon size={16} />;
  if (/mic|test/.test(k)) return <MicIcon size={16} />;
  return <SparkleIcon size={16} />;
}

/** Downsamples peaks to `count` bars, keeping the loudest value in each bucket. */
function bucket(values: number[] | undefined, count: number, scale: number): number[] {
  if (!values?.length) return Array(count).fill(0.06);
  const out: number[] = [];
  for (let i = 0; i < count; i++) {
    const a = Math.floor((i * values.length) / count);
    const b = Math.max(a + 1, Math.floor(((i + 1) * values.length) / count));
    let m = 0;
    for (let j = a; j < b; j++) m = Math.max(m, Math.abs(values[j] ?? 0));
    out.push(Math.min(1, Math.max(0.035, m * scale)));
  }
  return out;
}

const fmt = (s: number) => {
  const t = Math.max(0, Math.floor(s));
  return `${Math.floor(t / 60)}:${String(t % 60).padStart(2, "0")}`;
};

export function AudioDemo() {
  const sectionRef = useRef<HTMLElement>(null);
  const near = useActive(sectionRef, "700px");
  const narrow = useMediaQuery("(max-width: 639px)");
  const count = narrow ? 56 : 110;

  const player = useRef<AbPlayer | null>(null);
  const getPlayer = () => (player.current ??= new AbPlayer());
  const playedRef = useRef<HTMLDivElement>(null);
  const innerRef = useRef<HTMLDivElement>(null);
  const timeRef = useRef<HTMLSpanElement>(null);
  const seekRef = useRef<HTMLInputElement>(null);
  const chipRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const raf = useRef(0);

  const [manifest, setManifest] = useState<Manifest | null>(null);
  const [scenes, setScenes] = useState<Scene[]>([FALLBACK]);
  const [sceneId, setSceneId] = useState(FALLBACK.id);
  const [hush, setHush] = useState(true);
  const [engine, setEngine] = useState<EngineId>("reuse");
  const [state, setState] = useState<"idle" | "loading" | "playing" | "paused" | "error">("idle");
  const [ready, setReady] = useState<string | null>(null);
  const [duration, setDuration] = useState(FALLBACK.duration ?? 0);

  const scene = scenes.find((s) => s.id === sceneId) ?? scenes[0];
  const engines = (["deepfilter", "reuse"] as EngineId[]).filter((e) => !!scene.files[e]);
  const activeEngine: EngineId = engines.includes(engine) ? engine : engines[0];
  const version: Version = hush ? activeEngine : "noisy";

  // Scenes: the manifest when the build saw one, otherwise the original sample's peaks.
  const requested = useRef(false);
  useEffect(() => {
    if (!near || requested.current) return;
    requested.current = true;
    let alive = true;
    if (__HAS_DEMOS__) {
      fetch(`${SAMPLES}demos.json`)
        .then((r) => (r.ok ? r.json() : Promise.reject(r.status)))
        .then((m: Manifest) => {
          const list = Array.isArray(m?.scenes) ? m.scenes.filter(validScene) : [];
          if (!alive || !list.length) return;
          setManifest(m);
          setScenes(list);
          setSceneId(list[0].id);
          setEngine(list[0].files.deepfilter ? "deepfilter" : "reuse");
          setDuration(list[0].duration ?? 0);
        })
        .catch(() => undefined);
    } else {
      fetch(`${SAMPLES}peaks.json`)
        .then((r) => (r.ok ? r.json() : Promise.reject(r.status)))
        .then((p: { noisy?: number[]; hush?: number[]; seconds?: number }) => {
          if (!alive || !Array.isArray(p.noisy) || !Array.isArray(p.hush)) return;
          setScenes([{ ...FALLBACK, duration: Number(p.seconds) || FALLBACK.duration, peaks: { noisy: p.noisy, reuse: p.hush } }]);
          setDuration(Number(p.seconds) || FALLBACK.duration!);
        })
        .catch(() => undefined);
    }
    return () => {
      alive = false;
      requested.current = false;
    };
  }, [near]);

  const urls = useMemo(() => {
    const out: Partial<Record<Version, string>> = {};
    for (const [v, f] of Object.entries(scene.files) as [Version, string][]) out[v] = `${SAMPLES}${f}`;
    return out;
  }, [scene]);

  // Prefetch the current scene once the section is close; decoding never plays anything.
  useEffect(() => {
    if (!near) return;
    for (const url of Object.values(urls)) getPlayer().decode(url!).catch(() => undefined);
  }, [near, urls]);

  const paint = useCallback(
    (t: number) => {
      const d = player.current?.duration || duration || 1;
      const p = Math.min(1, Math.max(0, t / d));
      // The played part is a window that slides over a second, coloured waveform. Both
      // layers move with transforms only.
      if (playedRef.current) playedRef.current.style.transform = `translateX(${(p - 1) * 100}%)`;
      if (innerRef.current) innerRef.current.style.transform = `translateX(${(1 - p) * 100}%)`;
      if (timeRef.current) timeRef.current.textContent = fmt(t);
      if (seekRef.current) {
        seekRef.current.value = String(t);
        seekRef.current.setAttribute("aria-valuetext", `${fmt(t)} of ${fmt(d)}`);
      }
    },
    [duration],
  );

  const stopLoop = () => cancelAnimationFrame(raf.current);
  const loop = useCallback(() => {
    const p = player.current;
    if (!p?.playing) return;
    paint(p.position);
    raf.current = requestAnimationFrame(loop);
  }, [paint]);

  useEffect(() => {
    paint(player.current?.position ?? 0);
  }, [paint, sceneId]);

  useEffect(
    () => () => {
      stopLoop();
      player.current?.dispose();
    },
    [],
  );

  useEffect(() => {
    player.current?.select(version);
  }, [version]);

  const ensureLoaded = async () => {
    const p = getPlayer();
    if (ready === scene.id) return p;
    await p.load(urls);
    p.onEnded = () => {
      stopLoop();
      setState("paused");
      paint(0);
    };
    setReady(scene.id);
    setDuration(p.duration);
    return p;
  };

  const play = async () => {
    getPlayer().unlock();
    setState("loading");
    try {
      const p = await ensureLoaded();
      p.select(version);
      await p.play();
      setState("playing");
      stopLoop();
      raf.current = requestAnimationFrame(loop);
    } catch {
      setState("error");
    }
  };

  const pause = () => {
    player.current?.pause();
    stopLoop();
    setState("paused");
    paint(player.current?.position ?? 0);
  };

  const chooseScene = async (id: string) => {
    if (id === sceneId) return;
    const wasPlaying = player.current?.playing ?? false;
    player.current?.pause();
    stopLoop();
    setSceneId(id);
    setReady(null);
    const chip = chipRefs.current[scenes.findIndex((s) => s.id === id)];
    const row = chip?.parentElement;
    if (chip && row) row.scrollTo({ left: chip.offsetLeft - row.clientWidth / 2 + chip.clientWidth / 2, behavior: "smooth" });
    const next = scenes.find((s) => s.id === id)!;
    if (!next.files[engine]) setEngine(next.files.deepfilter ? "deepfilter" : "reuse");
    setDuration(next.duration ?? 0);
    setState(wasPlaying ? "loading" : "idle");
    paint(0);
    if (wasPlaying) {
      // Keep playing: load the new scene and start it from the beginning.
      try {
        const p = getPlayer();
        p.unlock();
        const files: Partial<Record<Version, string>> = {};
        for (const [v, f] of Object.entries(next.files) as [Version, string][]) files[v] = `${SAMPLES}${f}`;
        await p.load(files);
        setReady(next.id);
        setDuration(p.duration);
        p.select(hush ? (next.files[engine] ? engine : next.files.deepfilter ? "deepfilter" : "reuse") : "noisy");
        await p.play();
        setState("playing");
        raf.current = requestAnimationFrame(loop);
      } catch {
        setState("error");
      }
    }
  };

  const seek = (t: number) => {
    const p = player.current;
    if (p && ready === scene.id) p.seek(t);
    paint(t);
  };

  const onSeekKey = (e: KeyboardEvent<HTMLInputElement>) => {
    const now = player.current?.position ?? 0;
    const d = player.current?.duration || duration;
    const map: Record<string, number> = { ArrowRight: now + 1, ArrowUp: now + 1, ArrowLeft: now - 1, ArrowDown: now - 1, Home: 0, End: d };
    if (e.key in map) {
      e.preventDefault();
      seek(map[e.key]);
    }
  };

  const onChipKey = (e: KeyboardEvent, i: number) => {
    const step = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const next = (i + step + scenes.length) % scenes.length;
    chooseScene(scenes[next].id);
    chipRefs.current[next]?.focus();
  };

  // One scale for every version of the scene keeps their loudness comparable.
  const scale = useMemo(() => {
    const all = Object.values(scene.peaks).flatMap((v) => v ?? []);
    const max = all.reduce((m, v) => Math.max(m, Math.abs(v)), 0);
    return max > 0 ? 1 / max : 1;
  }, [scene]);
  const bars = useMemo(() => bucket(scene.peaks[version], count, scale), [scene, version, count, scale]);

  const scores = scene.metrics;
  const gains =
    scores?.noisy && scores[activeEngine]
      ? [
          { label: "PESQ", hint: "sound quality", from: scores.noisy.pesq, to: scores[activeEngine]!.pesq, digits: 2, unit: "" },
          { label: "STOI", hint: "clarity", from: scores.noisy.stoi, to: scores[activeEngine]!.stoi, digits: 2, unit: "" },
          { label: "SI-SDR", hint: "noise removed", from: scores.noisy.sisdr, to: scores[activeEngine]!.sisdr, digits: 1, unit: " dB" },
        ]
      : null;

  const playing = state === "playing" || state === "loading";
  const engineOptions = (["deepfilter", "reuse"] as EngineId[]).map((id) => ({
    value: id,
    label: <span className={scene.files[id] ? "" : "opacity-40"}>{id === "deepfilter" ? "DeepFilterNet 3" : "RE-USE"}</span>,
  }));

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
          sub="Pick a scene and press play. Switch between the original and Hush while it plays: both run in sync, so you hear the same word, with and without the noise."
        />

        <Reveal className="mx-auto mt-12 max-w-[960px]">
          <div className="glass p-4 sm:p-7">
            {/* Scenes */}
            <div
              role="radiogroup"
              aria-label="Scene"
              className="chip-row relative -mx-1 flex h-11 items-center gap-2 overflow-x-auto px-1 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
            >
              {scenes.map((s, i) => {
                const selected = s.id === scene.id;
                return (
                  <button
                    key={s.id}
                    ref={(el) => {
                      chipRefs.current[i] = el;
                    }}
                    type="button"
                    role="radio"
                    aria-checked={selected}
                    tabIndex={selected ? 0 : -1}
                    onClick={() => chooseScene(s.id)}
                    onKeyDown={(e) => onChipKey(e, i)}
                    className="flex h-10 shrink-0 cursor-pointer items-center gap-2 rounded-full pl-2 pr-3.5 text-[14px] font-[590] transition-[background-color,box-shadow,color] duration-200"
                    style={{
                      background: selected ? "#fff" : "rgba(30,27,24,.045)",
                      color: selected ? "var(--h-ink)" : "var(--h-ink-2)",
                      boxShadow: selected ? "inset 0 0 0 1px rgba(30,27,24,.1), 0 8px 20px -12px rgba(30,27,24,.45)" : "none",
                    }}
                  >
                    <span
                      className="grid h-7 w-7 place-items-center rounded-full transition-colors duration-300"
                      style={{ background: selected ? "var(--h-opal)" : "rgba(255,255,255,.7)" }}
                    >
                      {sceneIcon(s)}
                    </span>
                    {s.title}
                  </button>
                );
              })}
            </div>

            <div className="mt-5 flex flex-col gap-5 sm:flex-row sm:items-center">
              <div className="flex min-w-0 items-center gap-4">
                <button
                  type="button"
                  onClick={playing ? pause : play}
                  aria-label={playing ? "Pause" : `Play “${scene.title}”`}
                  className="relative grid h-16 w-16 shrink-0 cursor-pointer place-items-center rounded-full bg-ink text-white transition-transform duration-200 active:scale-95 sm:h-[72px] sm:w-[72px]"
                  style={{ boxShadow: "inset 0 1px 0 rgba(255,255,255,.18), 0 14px 30px -12px rgba(30,27,24,.7)" }}
                >
                  {state === "loading" && (
                    <span
                      aria-hidden
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
                  <Mochi size={44} mood={hush ? "happy" : "oops"} breathe={false} />
                  <div className="min-w-0">
                    <div className="truncate text-[17px] font-[640] tracking-[-0.02em]">{hush ? `With Hush · ${engineName(manifest, activeEngine)}` : "Original"}</div>
                    <div className="truncate text-[13.5px] tabular-nums text-muted">
                      <span ref={timeRef}>0:00</span> / {fmt(duration)}
                      {scene.place ? ` · ${scene.place}` : ""}
                      {typeof scene.snr === "number" ? ` · ${scene.snr} dB SNR` : ""}
                    </div>
                  </div>
                </div>
              </div>
              <div className="sm:ml-auto sm:w-[280px] sm:shrink-0">
                <Segmented
                  label="Which version to hear"
                  size="lg"
                  value={hush ? "hush" : "noisy"}
                  onChange={(v) => setHush(v === "hush")}
                  options={[
                    { value: "noisy", label: "Original" },
                    { value: "hush", label: "With Hush" },
                  ]}
                />
              </div>
            </div>

            {/* Waveform: grey bars, plus a coloured copy revealed up to the playhead. */}
            <div className="relative mt-6 h-[110px] overflow-hidden rounded-[16px] sm:h-[150px] [&:has(input:focus-visible)]:outline-2 [&:has(input:focus-visible)]:outline-offset-4 [&:has(input:focus-visible)]:outline-[var(--focus)]">
              <Bars bars={bars} tone="base" />
              <div ref={playedRef} aria-hidden className="absolute inset-0 overflow-hidden" style={{ transform: "translateX(-100%)" }}>
                <div ref={innerRef} className="absolute inset-0" style={{ transform: "translateX(100%)" }}>
                  <Bars bars={bars} tone={hush ? "hush" : "noisy"} />
                </div>
                <div className="absolute inset-y-1 right-0 w-[2px] rounded-full bg-ink/70" />
              </div>
              <input
                ref={seekRef}
                type="range"
                className="seek absolute inset-0 h-full w-full cursor-pointer"
                min={0}
                max={duration || 1}
                step="any"
                defaultValue={0}
                aria-label="Position in the recording"
                onInput={(e) => seek(Number(e.currentTarget.value))}
                onKeyDown={onSeekKey}
              />
            </div>

            <div className="mt-6 grid gap-4 md:grid-cols-[minmax(0,320px)_1fr] md:items-center">
              <div>
                <div className="mb-2 text-[12px] font-[600] uppercase tracking-[0.07em] text-muted">Hush engine</div>
                <Segmented
                  label="Hush engine"
                  value={activeEngine}
                  onChange={(v) => scene.files[v] && setEngine(v)}
                  options={engineOptions}
                />
              </div>
              <ul className="grid grid-cols-3 gap-2" aria-label={`Scores for this scene, ${engineName(manifest, activeEngine)}`}>
                {(gains ?? [
                  { label: "PESQ", hint: "sound quality", from: NaN, to: NaN, digits: 2, unit: "" },
                  { label: "STOI", hint: "clarity", from: NaN, to: NaN, digits: 2, unit: "" },
                  { label: "SI-SDR", hint: "noise removed", from: NaN, to: NaN, digits: 1, unit: " dB" },
                ]).map((g) => {
                  const has = Number.isFinite(g.to);
                  const delta = g.to - g.from;
                  return (
                    <li key={g.label} className="tile min-w-0 px-2.5 py-2 sm:px-3 sm:py-2.5">
                      <div className="flex flex-wrap items-baseline justify-between gap-x-1">
                        <span className="whitespace-nowrap text-[11.5px] font-[650] tracking-[0.02em] text-ink-2 sm:text-[12px]">{g.label}</span>
                        <span className={`text-[11.5px] font-[650] tabular-nums sm:text-[12px] ${has && delta >= 0 ? "text-green" : "text-muted"}`}>
                          {has ? `${delta >= 0 ? "+" : "−"}${Math.abs(delta).toFixed(g.digits)}` : "–"}
                        </span>
                      </div>
                      <div className="mt-0.5 truncate text-[15px] font-[650] tabular-nums sm:text-[16px]">
                        {has ? `${g.to.toFixed(g.digits)}${g.unit}` : <span className="text-[13px] font-[560] text-muted">{g.hint}</span>}
                      </div>
                      <div className="truncate text-[11.5px] tabular-nums text-muted">{has ? `from ${g.from.toFixed(g.digits)}` : " "}</div>
                    </li>
                  );
                })}
              </ul>
            </div>

            <div className="mt-5 flex flex-col gap-2 text-[13px] leading-relaxed text-muted sm:flex-row sm:items-start sm:justify-between">
              <p>
                {manifest ? (
                  <>
                    Scores compare each version with the clean recording.{" "}
                    {__HAS_DEMO_CREDITS__ && (
                      <a className="link text-ink-2" href={`${SAMPLES}demos/CREDITS.md`} target="_blank" rel="noopener">
                        Recording credits
                      </a>
                    )}
                    {__HAS_DEMO_CREDITS__ && <span className="text-faint"> · </span>}
                  </>
                ) : (
                  <>
                    NVIDIA's sample recording, before and after Hush (NVIDIA RE-USE). Sample © NVIDIA (
                    <a className="link text-ink-2" href={`${SAMPLES}LICENSE`} target="_blank" rel="noopener">
                      license
                    </a>
                    ).
                  </>
                )}{" "}
                <a className="link text-ink-2" href={docHref("comparison")}>
                  How we measure
                </a>
              </p>
              <p role="status" className="shrink-0 font-[560] text-amber">
                {state === "error" ? "This recording could not be loaded. Try again." : ""}
              </p>
            </div>
          </div>
        </Reveal>
      </div>
    </section>
  );
}

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

function Bars({ bars, tone }: { bars: number[]; tone: "base" | "noisy" | "hush" }) {
  const n = bars.length;
  const colors = useMemo(() => (tone === "hush" ? Array.from({ length: n }, (_, i) => opalAt(i / Math.max(1, n - 1))) : null), [tone, n]);
  const color = tone === "base" ? "rgba(30,27,24,.12)" : "#4a453f";
  return (
    <div aria-hidden className="absolute inset-0 flex items-center gap-[2px] sm:gap-[3px]">
      {bars.map((v, i) => (
        <span
          key={i}
          className="h-full flex-1 rounded-full"
          style={{
            transform: `scaleY(${v})`,
            background: colors ? colors[i] : color,
            transition: `transform .5s var(--h-ease) ${(i % 40) * 4}ms, background-color .3s`,
          }}
        />
      ))}
    </div>
  );
}
