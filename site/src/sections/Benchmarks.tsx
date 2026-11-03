// Measured quality and CPU of Hush's two engines against RNNoise and the noisy input. The
// numbers live in data/benchmarks.ts. Bars and rows move with transforms only.
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { ArrowRightIcon, CheckIcon, PlayIcon } from "../components/Icons";
import { Reveal, SectionHeader } from "../components/ui";
import { best, DATASET, ENGINES, METRICS, NOTES, SYSTEMS, type MetricId, type System } from "../data/benchmarks";
import { useActive } from "../lib/hooks";
import { docHref } from "../lib/route";

const ROW = 68;
const COLOR: Record<System["id"], string> = {
  noisy: "rgba(30,27,24,.16)",
  rnnoise: "rgba(30,27,24,.32)",
  reuseLight: "linear-gradient(90deg,#ffd27a,#9fe8b8)",
  reuse8: "linear-gradient(90deg,#ffd27a,#6fd9d1)",
  deepfilter: "linear-gradient(90deg,#ffae94,#ffd27a 45%,#9fe8b8 80%,#6fd9d1)",
};

function value(s: System, m: MetricId): number | null {
  return m === "cpu" ? s.cpu : s[m];
}

function MetricChart() {
  const [metric, setMetric] = useState<MetricId>("pesq");
  const refs = useRef<(HTMLButtonElement | null)[]>([]);
  const m = METRICS.find((x) => x.id === metric)!;
  const top = best(metric);

  // Rank: best first; systems without a value go last.
  const ranked = [...SYSTEMS].sort((a, b) => {
    const va = value(a, metric);
    const vb = value(b, metric);
    if (va === null) return 1;
    if (vb === null) return -1;
    return m.higherIsBetter ? vb - va : va - vb;
  });
  const rank = new Map(ranked.map((s, i) => [s.id, i]));

  const onKey = (e: KeyboardEvent, i: number) => {
    const step = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const next = (i + step + METRICS.length) % METRICS.length;
    setMetric(METRICS[next].id);
    refs.current[next]?.focus();
  };

  return (
    <div className="card p-4 sm:p-7">
      <div role="radiogroup" aria-label="Metric" className="flex flex-wrap gap-1.5">
        {METRICS.map((x, i) => {
          const on = x.id === metric;
          return (
            <button
              key={x.id}
              ref={(el) => {
                refs.current[i] = el;
              }}
              type="button"
              role="radio"
              aria-checked={on}
              tabIndex={on ? 0 : -1}
              onClick={() => setMetric(x.id)}
              onKeyDown={(e) => onKey(e, i)}
              className="h-10 cursor-pointer rounded-full px-4 text-[14px] font-[620] transition-[background-color,color,box-shadow] duration-200"
              style={{
                background: on ? "var(--h-ink)" : "rgba(30,27,24,.05)",
                color: on ? "#fff" : "var(--h-ink-2)",
              }}
            >
              {x.label}
            </button>
          );
        })}
      </div>

      <div className="mt-5 min-h-[66px] sm:min-h-[48px]">
        <div className="text-[17px] font-[650] tracking-[-0.02em]">
          {m.name} <span className="font-[520] text-muted">· {m.higherIsBetter ? "higher is better" : "lower is better"}</span>
        </div>
        <p className="mt-1 text-[14.5px] leading-snug text-ink-2">{m.plain}</p>
      </div>

      {/* Rows sit at fixed slots and slide to their rank. */}
      <ol className="relative mt-5" style={{ height: SYSTEMS.length * ROW }} aria-label={`${m.label} by system`}>
        {SYSTEMS.map((s) => {
          const v = value(s, metric);
          const [lo, hi] = m.scale;
          const frac = v === null ? 0 : Math.min(1, Math.max(0.015, (v - lo) / (hi - lo)));
          const isBest = s.id === top && metric !== "cpu";
          const hush = s.kind === "hush";
          return (
            <li
              key={s.id}
              className="absolute inset-x-0 top-0 grid h-[68px] grid-cols-[1fr_auto] content-center gap-x-3 gap-y-1.5 sm:grid-cols-[34%_1fr_118px] sm:items-center"
              style={{ transform: `translateY(${rank.get(s.id)! * ROW}px)`, transition: "transform .6s var(--h-ease)" }}
            >
              <div className="min-w-0">
                <div className={`flex items-center gap-1.5 truncate text-[14px] leading-tight sm:text-[15px] ${hush ? "font-[650] text-ink" : "font-[560] text-ink-2"}`}>
                  {hush && (
                    <span className="rounded-full px-1.5 py-px text-[10.5px] font-[700] text-ink sm:hidden" style={{ background: "var(--h-opal)" }}>
                      Hush
                    </span>
                  )}
                  <span className="truncate sm:hidden">{s.short}</span>
                  <span className="hidden truncate sm:inline">{s.name}</span>
                </div>
                <div className="hidden truncate text-[12.5px] text-muted sm:block">{s.detail}</div>
              </div>
              <div className="relative col-span-2 row-start-2 h-2.5 overflow-hidden rounded-full bg-black/[.05] sm:col-span-1 sm:row-start-auto sm:h-3.5">
                <div
                  className="absolute inset-0 origin-left rounded-full"
                  style={{ background: COLOR[s.id], transform: `scaleX(${frac})`, transition: "transform .7s var(--h-ease)" }}
                />
              </div>
              <div className="col-start-2 row-start-1 flex items-center justify-end gap-1.5 sm:col-start-auto sm:row-start-auto">
                {isBest && (
                  <span className="hidden rounded-full px-2 py-0.5 text-[11px] font-[700] text-green sm:inline" style={{ background: "rgba(31,157,114,.12)" }}>
                    Best
                  </span>
                )}
                <span className={`text-[14.5px] tabular-nums sm:text-[16px] ${isBest ? "font-[700] text-ink" : "font-[600] text-ink-2"}`}>
                  {v === null ? "–" : m.format(v)}
                  {isBest && <span className="sr-only"> (best)</span>}
                </span>
              </div>
            </li>
          );
        })}
      </ol>
      <div className="mt-2 flex justify-between text-[11.5px] tabular-nums text-faint sm:pl-[calc(34%+12px)] sm:pr-[130px]">
        <span>{m.format(m.scale[0])}</span>
        <span>{m.format(m.scale[1])}</span>
      </div>
      {metric === "cpu" && (
        <p className="mt-3 text-[13.5px] leading-snug text-ink-2">
          DeepFilterNet 3 costs about as much CPU as RNNoise, with far higher quality on every metric.
        </p>
      )}
    </div>
  );
}

const SPEED = 10;

function CpuRace() {
  const ref = useRef<HTMLDivElement>(null);
  const visible = useActive(ref);
  const [run, setRun] = useState(0);
  useEffect(() => {
    if (visible && run === 0) setRun(1);
  }, [visible, run]);
  const racers = SYSTEMS.filter((s) => s.cpu !== null).sort((a, b) => a.cpu! - b.cpu!);
  return (
    <div ref={ref} className="card flex flex-col p-5 sm:p-7">
      <div className="flex items-start justify-between gap-3">
        <div>
          <h3 className="text-[18px] font-[650] tracking-[-0.02em]">One minute of audio, one CPU core</h3>
          <p className="mt-1 text-[14px] leading-snug text-muted">How long each engine needs to clean 60 seconds of speech on one thread. Played {SPEED}× faster.</p>
        </div>
        <button type="button" onClick={() => setRun((r) => r + 1)} className="btn btn-soft btn-sm shrink-0" aria-label="Run the race again">
          <PlayIcon size={13} /> Again
        </button>
      </div>
      <ul className="mt-6 space-y-4" key={run}>
        {racers.map((s) => {
          const seconds = 60 * s.cpu!;
          const dur = run ? seconds / SPEED : 0;
          return (
            <li key={s.id}>
              <div className="flex items-baseline justify-between gap-2 text-[13.5px]">
                <span className={s.kind === "hush" ? "font-[640]" : "font-[560] text-ink-2"}>{s.short}</span>
                <span className="race-done tabular-nums font-[650]" style={{ animationDelay: `${dur}s`, opacity: run ? undefined : 1 }}>
                  {seconds < 10 ? seconds.toFixed(1) : Math.round(seconds)} s{seconds > 60 ? " · slower than real time" : ""}
                </span>
              </div>
              <div className="relative mt-1.5 h-2.5 overflow-hidden rounded-full bg-black/[.05]">
                <div
                  className={`absolute inset-0 origin-left rounded-full ${run ? "race-bar" : ""}`}
                  style={{ background: COLOR[s.id], animationDuration: `${dur}s`, transform: run ? undefined : "scaleX(1)" }}
                />
              </div>
            </li>
          );
        })}
      </ul>
      <p className="mt-auto pt-5 text-[12.5px] leading-snug text-muted">
        In the app, Hush uses more threads when one is not enough. Times from the CPU column: 60 s × CPU per audio second.
      </p>
    </div>
  );
}

function EngineCard({ id }: { id: keyof typeof ENGINES }) {
  const e = ENGINES[id];
  const dfn = id === "deepfilter";
  return (
    <div
      className="card flex flex-col p-5 sm:p-7"
      style={dfn ? { boxShadow: "var(--h-inner), 0 0 0 1.5px rgba(255,174,148,.6), 0 30px 60px -30px rgba(232,115,90,.5)" } : undefined}
    >
      <div className="flex items-center gap-2">
        <span className="chip" style={dfn ? { background: "var(--h-opal)", color: "var(--h-ink)" } : undefined}>
          {e.role}
        </span>
      </div>
      <h3 className="mt-4 text-[22px] font-[660] tracking-[-0.03em]">{e.name}</h3>
      <ul className="mt-4 space-y-2.5">
        {e.points.map((p) => (
          <li key={p} className="flex items-start gap-2.5 text-[15px] text-ink-2">
            <span className="mt-0.5 grid h-5 w-5 shrink-0 place-items-center rounded-full" style={{ background: "rgba(31,157,114,.12)", color: "#157353" }}>
              <CheckIcon size={12} strokeWidth={2.8} />
            </span>
            {p}
          </li>
        ))}
      </ul>
      <p className="mt-auto pt-5 text-[13px] text-muted">
        License:{" "}
        {e.licenseUrl ? (
          <a className="link text-ink-2" href={e.licenseUrl} target="_blank" rel="noopener">
            {e.license}
          </a>
        ) : (
          <span className="font-[600] text-ink-2">{e.license}</span>
        )}
      </p>
    </div>
  );
}

export function Benchmarks() {
  return (
    <section id="benchmarks" aria-labelledby="bench-title" className="relative py-20 sm:py-28">
      <div className="wrap">
        <SectionHeader
          id="bench-title"
          eyebrow="Benchmarks"
          title={
            <>
              Two engines. <span className="opal-text">Measured.</span>
            </>
          }
          sub={`On the ${DATASET.name} test set (${DATASET.files} recordings, scored at ${DATASET.scoredAt}), the best score on every quality metric belongs to one of Hush's engines.`}
        />

        <div className="mt-12 grid gap-4 lg:grid-cols-[1.35fr_1fr]">
          <Reveal className="flex">
            <div className="w-full">
              <MetricChart />
            </div>
          </Reveal>
          <Reveal delay={0.08} className="flex">
            <CpuRace />
          </Reveal>
        </div>

        <Reveal className="mt-14">
          <h3 className="heading text-center text-[28px] sm:text-[36px]">Why two engines?</h3>
          <p className="lede mx-auto mt-3 max-w-[640px] text-center text-[16.5px] sm:text-[18px]">
            DeepFilterNet 3 is light, scores highest on PESQ, STOI and SI-SDR, and is free for any use, so it is the default. NVIDIA RE-USE, with 8 layers, scores
            the most natural sound (DNSMOS), but it needs much more CPU. You choose in Settings.
          </p>
        </Reveal>
        <div className="mx-auto mt-8 grid max-w-[920px] gap-4 md:grid-cols-2">
          <Reveal className="flex">
            <div className="flex w-full">
              <EngineCard id="deepfilter" />
            </div>
          </Reveal>
          <Reveal delay={0.08} className="flex">
            <div className="flex w-full">
              <EngineCard id="reuse" />
            </div>
          </Reveal>
        </div>

        <Reveal className="mx-auto mt-8 max-w-[920px]">
          <ul className="space-y-1 text-[13px] leading-relaxed text-muted">
            {NOTES.map((n) => (
              <li key={n}>{n}</li>
            ))}
          </ul>
          <a href={docHref(DATASET.doc)} className="link mt-3 inline-flex items-center gap-1.5 text-[14px] font-[580] text-ink">
            The method, every metric and the caveats <ArrowRightIcon size={15} />
          </a>
        </Reveal>
      </div>
    </section>
  );
}
