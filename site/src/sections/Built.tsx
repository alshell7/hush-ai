// "How Hush was built": an isometric stack drawn in SVG, one layer per step, with data
// flowing up through it. The step you are reading lifts its layer. Transforms and opacity only.
import { useEffect, useRef, useState, type ReactNode } from "react";
import { APPS } from "../components/AppGuides";
import { ArrowRightIcon } from "../components/Icons";
import { SectionHeader } from "../components/ui";
import { usePauseOffscreen } from "../lib/hooks";
import { docHref } from "../lib/route";

// ------------------------------------------------------------------ Isometric geometry

const C = Math.cos(Math.PI / 6);
const S = 0.5;
const W = 200; // footprint, both directions
const T = 14; // slab thickness
const GAP = 52; // vertical distance between layers
const CX = 210; // screen centre
const BASE_Y = 340;
/** Layers above the one you are reading move up by this much, so its top shows. */
const LIFT = 74; // screen y of the bottom layer's centre (top face)

const iso = (x: number, y: number, z: number): [number, number] => [CX + (x - y) * C, BASE_Y + (x + y) * S - z];
const pts = (list: [number, number][]) => list.map(([x, y]) => `${x.toFixed(1)},${y.toFixed(1)}`).join(" ");

/** SVG matrix that maps a W×W plane drawing onto the top face at height z. */
const faceMatrix = (z: number) => `matrix(${C} ${S} ${-C} ${S} ${CX} ${BASE_Y - W * S - z})`;

type Layer = {
  title: string;
  short: string;
  tint: [string, string, string];
  face: ReactNode;
};

const h = W / 2;

function Slab({ z, tint }: { z: number; tint: [string, string, string] }) {
  const top: [number, number][] = [iso(-h, -h, z), iso(h, -h, z), iso(h, h, z), iso(-h, h, z)];
  const right: [number, number][] = [iso(h, -h, z), iso(h, h, z), iso(h, h, z - T), iso(h, -h, z - T)];
  const left: [number, number][] = [iso(-h, h, z), iso(h, h, z), iso(h, h, z - T), iso(-h, h, z - T)];
  return (
    <>
      <polygon points={pts(left)} fill={tint[1]} stroke="rgba(30,27,24,.16)" strokeWidth=".8" strokeLinejoin="round" />
      <polygon points={pts(right)} fill={tint[2]} stroke="rgba(30,27,24,.16)" strokeWidth=".8" strokeLinejoin="round" />
      <polygon points={pts(top)} fill={tint[0]} stroke="rgba(30,27,24,.2)" strokeWidth=".8" strokeLinejoin="round" />
    </>
  );
}

const ink = "#4a453f";
const line = "rgba(30,27,24,.28)";

const LAYERS: Layer[] = [
  {
    title: "Two research models",
    short: "Research models",
    tint: ["#fff4ee", "#ffe2d6", "#ffd3c2"],
    face: (
      <g>
        {/* GPU chip */}
        <rect x="22" y="26" width="70" height="70" rx="8" fill="#fff" stroke={line} />
        {[0, 1, 2, 3].map((i) => (
          <g key={i} stroke={line}>
            <path d={`M${34 + i * 15} 26 v-8 M${34 + i * 15} 96 v8 M22 ${38 + i * 15} h-8 M92 ${38 + i * 15} h8`} />
          </g>
        ))}
        <text x="57" y="66" textAnchor="middle" fontSize="14" fontWeight="700" fill={ink} fontFamily="var(--font-sans)">
          GPU
        </text>
        {/* A small network */}
        {[
          [120, 30],
          [120, 62],
          [120, 94],
          [160, 46],
          [160, 78],
          [186, 62],
        ].map(([x, y], i) => (
          <circle key={i} cx={x} cy={y} r="6" fill="#fff" stroke={line} />
        ))}
        <path d="M126 30 L154 46 M126 62 L154 46 M126 62 L154 78 M126 94 L154 78 M126 30 L154 78 M166 46 L180 62 M166 78 L180 62" stroke={line} />
        <rect x="22" y="124" width="74" height="30" rx="15" fill="#fff" stroke={line} />
        <text x="59" y="144" textAnchor="middle" fontSize="12" fontWeight="650" fill={ink} fontFamily="var(--font-sans)">
          RE-USE
        </text>
        <rect x="106" y="124" width="74" height="30" rx="15" fill="#fff" stroke={line} />
        <text x="143" y="144" textAnchor="middle" fontSize="12" fontWeight="650" fill={ink} fontFamily="var(--font-sans)">
          DFN3
        </text>
        <text x="22" y="182" fontSize="11" fontWeight="600" fill="#736c63" fontFamily="var(--font-sans)">
          PyTorch
        </text>
      </g>
    ),
  },
  {
    title: "Ported to plain Rust, on the CPU",
    short: "Rust on the CPU",
    tint: ["#fff8ea", "#ffecc4", "#ffe0a6"],
    face: (
      <g>
        {Array.from({ length: 8 }, (_, i) => (
          <rect key={i} x={20 + i * 21} y="28" width="16" height="26" rx="4" fill={i === 3 ? "#ffd27a" : "#fff"} stroke={line} />
        ))}
        <text x="20" y="74" fontSize="11" fontWeight="600" fill="#736c63" fontFamily="var(--font-sans)">
          10–20 ms frames
        </text>
        {["Windows", "macOS", "Linux"].map((os, i) => (
          <g key={os} transform={`translate(20 ${92 + i * 30})`}>
            <rect width="160" height="22" rx="11" fill="#fff" stroke={line} />
            <circle cx="12" cy="11" r="6" fill="#1f9d72" />
            <path d="M9 11 l2 2 4 -4" fill="none" stroke="#fff" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
            <text x="26" y="15" fontSize="11" fontWeight="600" fill={ink} fontFamily="var(--font-sans)">
              {os} · 100+ dB
            </text>
          </g>
        ))}
      </g>
    ),
  },
  {
    title: "A real-time engine",
    short: "Real-time engine",
    tint: ["#f1fbf3", "#dcf6e3", "#c9f2d4"],
    face: (
      <g>
        {/* Ring buffer */}
        {Array.from({ length: 12 }, (_, i) => {
          const a0 = (i / 12) * Math.PI * 2;
          const a1 = ((i + 0.8) / 12) * Math.PI * 2;
          const r0 = 34;
          const r1 = 52;
          const p = (a: number, r: number) => `${(70 + Math.cos(a) * r).toFixed(1)} ${(100 + Math.sin(a) * r).toFixed(1)}`;
          return <path key={i} d={`M${p(a0, r0)} L${p(a0, r1)} L${p(a1, r1)} L${p(a1, r0)} Z`} fill={i < 7 ? "#9fe8b8" : "#fff"} stroke={line} strokeWidth=".8" />;
        })}
        <text x="70" y="104" textAnchor="middle" fontSize="10" fontWeight="650" fill={ink} fontFamily="var(--font-sans)">
          ring
        </text>
        {["audio thread", "worker", "output"].map((t, i) => (
          <g key={t} transform={`translate(132 ${50 + i * 34})`}>
            <rect width="54" height="22" rx="6" fill="#fff" stroke={line} />
            <text x="27" y="15" textAnchor="middle" fontSize="8.5" fontWeight="650" fill={ink} fontFamily="var(--font-sans)">
              {t}
            </text>
          </g>
        ))}
      </g>
    ),
  },
  {
    title: "The app and a virtual microphone",
    short: "App + Hush AI Microphone",
    tint: ["#eefaf8", "#d3f3ef", "#b9ece6"],
    face: (
      <g>
        <rect x="20" y="20" width="160" height="92" rx="10" fill="#fff" stroke={line} />
        <path d="M20 36 H180" stroke={line} />
        <circle cx="30" cy="28" r="2.5" fill="#ff8a7a" />
        <circle cx="39" cy="28" r="2.5" fill="#ffd27a" />
        <circle cx="48" cy="28" r="2.5" fill="#8fdc9f" />
        <circle cx="100" cy="74" r="24" fill="url(#iso-opal)" stroke="#fff" strokeWidth="2" />
        <path d="M91 73 q3 -4 6 0 M103 73 q3 -4 6 0" fill="none" stroke="#231f1c" strokeWidth="2" strokeLinecap="round" />
        <path d="M97.5 79 q2.5 2.5 5 0" fill="none" stroke="#231f1c" strokeWidth="1.6" strokeLinecap="round" />
        {["Core Audio", "VB-CABLE", "PulseAudio"].map((t, i) => (
          <g key={t} transform={`translate(${20 + i * 55} 130)`}>
            <rect width="50" height="44" rx="8" fill="#fff" stroke={line} />
            <text x="25" y="26" textAnchor="middle" fontSize="7.6" fontWeight="650" fill={ink} fontFamily="var(--font-sans)">
              {t}
            </text>
          </g>
        ))}
      </g>
    ),
  },
  {
    title: "Every meeting app",
    short: "Every meeting app",
    tint: ["#ffffff", "#f3efe9", "#e9e3da"],
    face: (
      <g>
        {APPS.slice(0, 6).map((a, i) => (
          <g key={a.id} transform={`translate(${24 + (i % 3) * 56} ${34 + Math.floor(i / 3) * 70})`}>
            <rect width="44" height="44" rx="11" fill="#fff" stroke={line} />
            <image href={a.logo} x="10" y="10" width="24" height="24" />
          </g>
        ))}
      </g>
    ),
  },
];

const STEPS: { title: string; body: ReactNode; chips: string[]; doc?: { slug: string; label: string } }[] = [
  {
    title: "Two research models",
    body: (
      <>
        <strong className="font-[630] text-ink">NVIDIA Real-time RE-USE</strong>, a Mamba speech model from NVIDIA research, written in PyTorch for GPUs. And{" "}
        <strong className="font-[630] text-ink">DeepFilterNet 3</strong>, an open model built for real-time noise suppression.
      </>
    ),
    chips: ["PyTorch", "GPU", "Mamba", "DeepFilterNet 3"],
    doc: { slug: "comparison", label: "How the two compare" },
  },
  {
    title: "Ported to plain Rust, on the CPU",
    body: (
      <>
        RE-USE was rewritten in plain Rust with SIMD; DeepFilterNet 3 runs through its own Rust library, libDF. Both run in real time on the CPU, in 10–20 ms
        frames. On every change, CI checks RE-USE against NVIDIA's GPU output on Windows, macOS and Linux: the waveforms match to more than 100 dB.
      </>
    ),
    chips: ["Rust", "SIMD", "No Python", "No GPU", "100+ dB in CI"],
    doc: { slug: "accuracy", label: "Accuracy" },
  },
  {
    title: "A real-time engine",
    body: (
      <>
        The audio thread never waits. Lock-free ring buffers carry your voice from the microphone to a worker thread that runs the model, and on to the output. A
        slow frame can never glitch the device.
      </>
    ),
    chips: ["Lock-free queues", "Inference on a worker", "Clock-drift control"],
    doc: { slug: "architecture", label: "Architecture" },
  },
  {
    title: "The app and a virtual microphone",
    body: (
      <>
        A small Tauri app with a tray widget, and a virtual microphone for each system: Hush's own Core Audio plug-in on macOS, VB-CABLE on Windows (until Hush's
        own driver is signed), and a PulseAudio virtual source on Linux.
      </>
    ),
    chips: ["Tauri", "Core Audio plug-in", "VB-CABLE", "PulseAudio"],
    doc: { slug: "virtual-microphone", label: "Virtual microphone" },
  },
  {
    title: "Every meeting app",
    body: <>Apps see Hush AI Microphone like any other microphone. Zoom, Google Meet, Teams, Slack, Discord, Webex, OBS: choose it once, and they hear only you.</>,
    chips: ["Zoom", "Meet", "Teams", "Slack", "Discord", "Webex", "OBS"],
    doc: { slug: "performance", label: "Performance" },
  },
];

function Stack({ active }: { active: number }) {
  const ref = usePauseOffscreen<HTMLDivElement>();
  return (
    <div ref={ref} className="iso relative mx-auto aspect-[420/560] w-full max-w-[300px] sm:max-w-[400px] lg:max-w-[460px]">
      <svg viewBox="0 -80 420 560" className="h-full w-full overflow-visible" role="img" aria-label="Five layers stacked: research models, Rust on the CPU, the real-time engine, the app with its virtual microphone, and meeting apps on top">
        <defs>
          <linearGradient id="iso-opal" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stopColor="#ffc4b0" />
            <stop offset=".4" stopColor="#ffe0a6" />
            <stop offset=".7" stopColor="#c9f2d4" />
            <stop offset="1" stopColor="#9be6e0" />
          </linearGradient>
          <radialGradient id="iso-glow" cx=".5" cy=".5" r=".5">
            <stop offset="0" stopColor="#ffd27a" stopOpacity=".55" />
            <stop offset="1" stopColor="#ffd27a" stopOpacity="0" />
          </radialGradient>
        </defs>
        {/* Shadow on the floor */}
        <ellipse cx={CX} cy={BASE_Y + T + 6} rx="180" ry="96" fill="rgba(30,27,24,.05)" />
        {LAYERS.map((layer, i) => {
          const z = i * GAP;
          const on = i === active;
          const [px, py] = iso(0, 0, z + 2);
          return (
            <g key={layer.title}>
              <g className="iso-layer" style={{ transform: `translateY(${on ? -14 : i > active ? -LIFT : 0}px)`, opacity: i <= active ? 1 : 0.5 }}>
                <ellipse cx={CX} cy={BASE_Y - z} rx="190" ry="96" fill="url(#iso-glow)" className="iso-glow" style={{ opacity: on ? 1 : 0 }} />
                <Slab z={z} tint={layer.tint} />
                <g transform={faceMatrix(z)}>{layer.face}</g>
                {/* Step number on the right corner */}
                <g transform={`translate(${(iso(h, -h, z)[0] + 14).toFixed(1)} ${(iso(h, -h, z)[1] + 6).toFixed(1)})`}>
                  <circle r="12" fill={on ? "#1e1b18" : "#fff"} stroke="rgba(30,27,24,.2)" />
                  <text y="4" textAnchor="middle" fontSize="12" fontWeight="700" fill={on ? "#fff" : "#4a453f"} fontFamily="var(--font-sans)">
                    {i + 1}
                  </text>
                </g>
              </g>
              {/* Data rising to the next layer, drawn before it so the layer above covers its end. */}
              {i < LAYERS.length - 1 && (
                <g className="iso-layer" style={{ transform: `translateY(${on ? -14 : i > active ? -LIFT : 0}px)`, opacity: i <= active ? 1 : 0 }}>
                  {[0, 1].map((k) => (
                    <g key={k} transform={`translate(${(px + (k ? 22 : -22)).toFixed(1)} ${py.toFixed(1)})`}>
                      <g className="iso-packet" style={{ animationDelay: `${(i * 0.35 + k * 0.9).toFixed(2)}s` }}>
                        <path d="M0 -5 L6 -2 L0 1 L-6 -2 Z" fill={k ? "#1f9d72" : "#e8735a"} />
                      </g>
                    </g>
                  ))}
                </g>
              )}
            </g>
          );
        })}
      </svg>
    </div>
  );
}

export function Built() {
  const [active, setActive] = useState(0);
  const stepRefs = useRef<(HTMLElement | null)[]>([]);

  useEffect(() => {
    // On small screens the picture is pinned over the top half, so read the step below it.
    const wide = window.matchMedia("(min-width: 1024px)").matches;
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) if (e.isIntersecting) setActive(Number((e.target as HTMLElement).dataset.step));
      },
      { rootMargin: wide ? "-45% 0px -45% 0px" : "-66% 0px -24% 0px" },
    );
    for (const el of stepRefs.current) if (el) io.observe(el);
    return () => io.disconnect();
  }, []);

  return (
    <section id="built" aria-labelledby="built-title" className="relative py-20 sm:py-28">
      <div className="wrap">
        <SectionHeader
          id="built-title"
          eyebrow="How Hush was built"
          title={
            <>
              From a research paper <span className="opal-text">to your call</span>
            </>
          }
          sub="Five layers, from GPU research code to a microphone every app can use. Scroll through them."
        />
        <div className="mt-12 grid gap-6 lg:mt-16 lg:grid-cols-[1.05fr_1fr] lg:gap-14">
          <div className="sticky top-[76px] z-10 -mx-4 self-start bg-[linear-gradient(180deg,var(--h-paper)_86%,transparent)] px-4 pb-4 lg:top-[110px] lg:mx-0 lg:bg-none lg:px-0 lg:pb-0">
            <Stack active={active} />
          </div>
          <ol className="relative">
            {STEPS.map((s, i) => (
              <li
                key={s.title}
                ref={(el) => {
                  stepRefs.current[i] = el;
                }}
                data-step={i}
                className="flex min-h-[46vh] items-center py-6 lg:min-h-[62vh]"
              >
                <div className="built-step w-full rounded-[26px] p-5 transition-[opacity,transform] duration-500 sm:p-7" style={{ opacity: i === active ? 1 : 0.42, transform: i === active ? "none" : "scale(.98)" }}>
                  <div className="flex items-center gap-3">
                    <span className="grid h-9 w-9 shrink-0 place-items-center rounded-full bg-ink text-[14px] font-[650] text-white">{i + 1}</span>
                    <h3 className="text-[21px] font-[650] tracking-[-0.025em] sm:text-[24px]">{s.title}</h3>
                  </div>
                  <p className="mt-4 text-[16px] leading-relaxed text-ink-2 sm:text-[17px]">{s.body}</p>
                  <ul className="mt-4 flex flex-wrap gap-1.5">
                    {s.chips.map((c) => (
                      <li key={c} className="chip bg-white" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
                        {c}
                      </li>
                    ))}
                  </ul>
                  {s.doc && (
                    <a href={docHref(s.doc.slug)} className="link mt-5 inline-flex items-center gap-1.5 text-[14.5px] font-[580]">
                      {s.doc.label} <ArrowRightIcon size={15} />
                    </a>
                  )}
                </div>
              </li>
            ))}
          </ol>
        </div>
      </div>
    </section>
  );
}
