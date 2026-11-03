// Thin line-art pictures for the feature cards, each with a small loop. Everything moves with
// CSS transforms and opacity (keyframes in styles.css, prefixed la-), and the cards pause
// them off-screen. Shapes are computed once at module load, so the server and the browser
// draw the same thing.
import type { ReactNode } from "react";
import { APPS, AppLogo } from "./AppGuides";
import { CheckIcon, ShieldIcon } from "./Icons";
import { Mark, Mochi } from "./Mochi";

const STROKE = "rgba(30,27,24,.22)";
const FAINT = "rgba(30,27,24,.1)";

/** A tiny deterministic random sequence. */
function rand(seed: number) {
  let s = seed;
  return () => {
    s = (s * 1664525 + 1013904223) % 4294967296;
    return s / 4294967296;
  };
}

/** A wave over two periods of `period` px, so translating by one period loops seamlessly. */
function wave(period: number, amp: number, noisy: boolean, seed = 7): string {
  const r = rand(seed);
  const step = noisy ? 3 : 2;
  const pts: string[] = [];
  const jitter: number[] = [];
  for (let x = 0; x <= period; x += step) jitter.push(noisy ? (r() - 0.5) * amp * 1.3 : 0);
  for (let x = 0; x <= period * 2; x += step) {
    const t = (x % period) / period;
    // Two syllable-like bursts per period.
    const env = Math.max(0, Math.sin(t * Math.PI * 2)) ** 1.6 * 0.9 + 0.1;
    const voice = Math.sin(x * 0.42) * env * amp * 0.85;
    const y = voice + jitter[Math.round((x % period) / step)];
    pts.push(`${x.toFixed(1)} ${y.toFixed(2)}`);
  }
  return `M${pts.join(" L")}`;
}

const NOISY_120 = wave(120, 14, true, 11);
const CLEAN_120 = wave(120, 14, false);
const NOISY_80 = wave(80, 10, true, 5);
const CLEAN_80 = wave(80, 10, false);

function Scroll({ d, x, y, w, color, width = 1.6, period, speed = 2.4, id }: { d: string; x: number; y: number; w: number; color: string; width?: number; period: number; speed?: number; id: string }) {
  return (
    <g transform={`translate(${x} ${y})`}>
      <clipPath id={id}>
        <rect x={0} y={-30} width={w} height={60} />
      </clipPath>
      <g clipPath={`url(#${id})`}>
        <path d={d} fill="none" stroke={color} strokeWidth={width} strokeLinejoin="round" strokeLinecap="round" style={{ animation: `la-scroll-${period} ${speed}s linear infinite` }} />
      </g>
    </g>
  );
}

// ------------------------------------------------------------------ On-device and private

export function PrivateArt() {
  return (
    <svg viewBox="0 0 520 230" className="h-full w-full" aria-hidden>
      <defs>
        <linearGradient id="la-opal" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#ffc4b0" />
          <stop offset=".35" stopColor="#ffe0a6" />
          <stop offset=".7" stopColor="#c9f2d4" />
          <stop offset="1" stopColor="#9be6e0" />
        </linearGradient>
      </defs>
      {/* Desk shadow */}
      <ellipse cx="262" cy="200" rx="190" ry="8" fill="rgba(30,27,24,.05)" />
      {/* Microphone */}
      <g transform="translate(34 92)">
        <rect x="0" y="0" width="52" height="52" rx="16" fill="#fff" stroke={STROKE} />
        <rect x="20" y="11" width="12" height="20" rx="6" fill="none" stroke="#4a453f" strokeWidth="1.6" />
        <path d="M14 27a12 12 0 0 0 24 0M26 39v4" fill="none" stroke="#4a453f" strokeWidth="1.6" strokeLinecap="round" />
      </g>
      <path d="M90 118 H150" stroke={STROKE} strokeDasharray="2 4" />
      {/* Laptop */}
      <rect x="150" y="34" width="224" height="146" rx="11" fill="#fff" stroke={STROKE} />
      <rect x="160" y="44" width="204" height="124" rx="5" fill="#fbfaf7" stroke={FAINT} />
      <path d="M126 180 H398 L414 191 Q416 194 412 194 H112 Q108 194 110 191 Z" fill="#fff" stroke={STROKE} />
      <path d="M240 180 h44 v3 q0 2 -2 2 h-40 q-2 0 -2 -2 z" fill={FAINT} />
      {/* Inside: noisy in, Hush, clean out */}
      <Scroll id="la-p-n" d={NOISY_80} x={170} y={106} w={60} color="#e8735a" period={80} />
      <Scroll id="la-p-c" d={CLEAN_80} x={294} y={106} w={60} color="#1f9d72" width={1.9} period={80} speed={2.4} />
      <circle cx="262" cy="106" r="24" fill="url(#la-opal)" stroke="rgba(255,255,255,.9)" strokeWidth="2" />
      <g transform="translate(244 88) scale(.56)">
        <path d="M4 40C4 21 16.5 8 32 8C47.5 8 60 21 60 40C60 50.5 52 56 32 56C12 56 4 50.5 4 40Z" fill="#fff" />
        <path d="M19 38.5Q23 33.5 27 38.5M37 38.5Q41 33.5 45 38.5" fill="none" stroke="#231f1c" strokeWidth="3.4" strokeLinecap="round" />
        <path d="M29.4 43.4Q32 46.4 34.6 43.4" fill="none" stroke="#231f1c" strokeWidth="2.6" strokeLinecap="round" />
      </g>
      <text x="262" y="152" textAnchor="middle" fontSize="10" fontWeight="600" fill="#736c63" fontFamily="var(--font-sans)">
        on this computer
      </text>
      {/* Cloud, not reached */}
      <path
        d="M440 70 h40 a16 16 0 0 0 -2 -31.8 a22 22 0 0 0 -41 -6 a15 15 0 0 0 -21 13.5 a12 12 0 0 0 24 24.3 z"
        fill="#fff"
        stroke={STROKE}
        strokeDasharray="3 3"
      />
      <path d="M374 64 L414 50" stroke={STROKE} strokeDasharray="2 4" />
      {[0, 1, 2].map((i) => (
        <circle key={i} cx="376" cy="63" r="3" fill="#e8735a" style={{ animation: `la-blocked 3s ease-in ${i}s infinite`, opacity: 0 }} />
      ))}
      <g transform="translate(397 56)">
        <g className="la-fill-box" style={{ animation: "la-bump 1s ease-out .8s infinite" }}>
          <circle r="10" fill="#fff" stroke="rgba(232,115,90,.6)" strokeWidth="1.4" />
          <path d="M-3.5 -3.5 L3.5 3.5 M3.5 -3.5 L-3.5 3.5" stroke="#e8735a" strokeWidth="1.8" strokeLinecap="round" />
        </g>
      </g>
      <text x="456" y="92" textAnchor="middle" fontSize="10" fontWeight="600" fill="#aaa398" fontFamily="var(--font-sans)">
        no cloud
      </text>
    </svg>
  );
}

// ------------------------------------------------------------------ Real time

export function RealtimeArt() {
  const frames = Array.from({ length: 12 }, (_, i) => i);
  return (
    <svg viewBox="0 0 300 200" className="h-full w-full" aria-hidden>
      <text x="16" y="38" fontSize="10" fontWeight="600" fill="#736c63" fontFamily="var(--font-sans)">
        You say
      </text>
      <Scroll id="la-r-n" d={NOISY_120} x={74} y={34} w={210} color="#e8735a" period={120} speed={3} />
      {/* 10 ms frames: one lights up at a time */}
      <g transform="translate(16 82)">
        {frames.map((i) => (
          <rect key={i} x={i * 23} y="0" width="19" height="26" rx="6" fill="#fff" stroke={STROKE} />
        ))}
        <rect x="0" y="0" width="19" height="26" rx="6" fill="url(#la-rt-opal)" style={{ animation: "la-step 1.2s steps(12) infinite" }} />
        <defs>
          <linearGradient id="la-rt-opal" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stopColor="#ffc4b0" />
            <stop offset="1" stopColor="#9be6e0" />
          </linearGradient>
        </defs>
        <text x="0" y="44" fontSize="9.5" fill="#aaa398" fontFamily="var(--font-sans)">
          10–20 ms frames
        </text>
      </g>
      <text x="16" y="160" fontSize="10" fontWeight="600" fill="#736c63" fontFamily="var(--font-sans)">
        They hear
      </text>
      <Scroll id="la-r-c" d={CLEAN_120} x={74} y={156} w={210} color="#1f9d72" width={1.9} period={120} speed={3} />
      {/* The delay, as a bracket */}
      <g transform="translate(74 178)">
        <path d="M0 0 v6 h54 v-6" fill="none" stroke="#c27c12" strokeWidth="1.2" />
        <text x="62" y="9" fontSize="10.5" fontWeight="650" fill="#a8650a" fontFamily="var(--font-sans)">
          ≈ 60 ms
        </text>
      </g>
    </svg>
  );
}

// ------------------------------------------------------------------ Light on the CPU

export function CpuArt() {
  const pins = Array.from({ length: 6 }, (_, i) => 22 + i * 14);
  return (
    <svg viewBox="0 0 300 200" className="h-full w-full" aria-hidden>
      <g transform="translate(26 30)">
        {pins.map((p) => (
          <g key={p} stroke={STROKE} strokeLinecap="round">
            <path d={`M${p} 0 v-10`} />
            <path d={`M${p} 120 v10`} />
            <path d={`M0 ${p} h-10`} />
            <path d={`M120 ${p} h10`} />
          </g>
        ))}
        <rect x="0" y="0" width="120" height="120" rx="16" fill="#fff" stroke={STROKE} />
        {[0, 1, 2, 3].map((i) => {
          const x = 14 + (i % 2) * 50;
          const y = 14 + Math.floor(i / 2) * 50;
          return (
            <g key={i} transform={`translate(${x} ${y})`}>
              <rect width="42" height="42" rx="9" fill="#fbfaf7" stroke={FAINT} />
              {i === 0 && (
                <rect x="4" y="4" width="34" height="34" rx="6" fill="#1f9d72" opacity=".9" className="la-fill-box la-origin-bottom" style={{ transform: "scaleY(.06)", animation: "la-core 2.6s ease-in-out infinite" }} />
              )}
            </g>
          );
        })}
      </g>
      <g transform="translate(186 44)" fontFamily="var(--font-sans)">
        <text x="0" y="34" fontSize="38" fontWeight="680" fill="#1e1b18" letterSpacing="-1.5">
          6%
        </text>
        <text x="0" y="52" fontSize="10.5" fontWeight="560" fill="#736c63">
          of one core
        </text>
        <text x="0" y="86" fontSize="9.5" fontWeight="600" fill="#736c63">
          DeepFilterNet 3
        </text>
        <rect x="0" y="92" width="96" height="5" rx="2.5" fill={FAINT} />
        <rect x="0" y="92" width="96" height="5" rx="2.5" fill="#1f9d72" className="la-fill-box la-origin-left" style={{ transform: "scaleX(.06)" }} />
        <text x="0" y="114" fontSize="9.5" fontWeight="600" fill="#aaa398">
          RE-USE Light 79%
        </text>
        <rect x="0" y="120" width="96" height="5" rx="2.5" fill={FAINT} />
        <rect x="0" y="120" width="96" height="5" rx="2.5" fill="#d99a1e" opacity=".8" className="la-fill-box la-origin-left" style={{ transform: "scaleX(.79)" }} />
      </g>
    </svg>
  );
}

// ------------------------------------------------------------------ Every app

export function AppsArt() {
  return (
    <div className="relative mx-auto h-full w-full max-w-[300px]" aria-hidden>
      <div className="absolute inset-x-4 top-5 rounded-[14px] bg-white" style={{ boxShadow: `inset 0 0 0 1px ${STROKE}` }}>
        <div className="flex h-7 items-center gap-1.5 border-b border-line px-3">
          <span className="h-2 w-2 rounded-full" style={{ boxShadow: `inset 0 0 0 1px ${STROKE}` }} />
          <span className="h-2 w-2 rounded-full" style={{ boxShadow: `inset 0 0 0 1px ${STROKE}` }} />
          <span className="h-2 w-2 rounded-full" style={{ boxShadow: `inset 0 0 0 1px ${STROKE}` }} />
          <span className="ml-1.5 text-[10px] font-[600] text-muted">Audio settings</span>
        </div>
        <div className="p-3">
          <div className="text-[9.5px] font-[650] uppercase tracking-[0.08em] text-faint">Microphone</div>
          <div className="relative mt-1.5 h-8 overflow-hidden rounded-[9px] text-[11.5px]" style={{ boxShadow: `inset 0 0 0 1px ${STROKE}` }}>
            <span className="absolute inset-0 flex items-center px-2.5 text-ink-2" style={{ animation: "la-swap-a 5s ease-in-out infinite" }}>
              Built-in Microphone
            </span>
            <span className="absolute inset-0 flex items-center gap-1.5 px-2.5 font-[640] text-ink" style={{ animation: "la-swap-b 5s ease-in-out infinite", opacity: 0 }}>
              <Mark size={13} /> Hush AI Microphone
              <CheckIcon size={12} strokeWidth={2.6} className="ml-auto text-green" />
            </span>
          </div>
        </div>
      </div>
      <div className="absolute inset-x-0 bottom-3 flex justify-center gap-2">
        {APPS.slice(0, 6).map((a, i) => (
          <span
            key={a.id}
            className="grid h-10 w-10 place-items-center rounded-[12px] bg-white"
            style={{ boxShadow: `inset 0 0 0 1px ${FAINT}, 0 8px 18px -12px rgba(30,27,24,.4)`, animation: `la-float 5s ease-in-out ${i * 0.35}s infinite` }}
          >
            <AppLogo app={a.id} size={20} />
          </span>
        ))}
      </div>
    </div>
  );
}

// ------------------------------------------------------------------ Tray and menu bar

export function TrayArt() {
  return (
    <div className="relative mx-auto h-full w-full max-w-[300px]" aria-hidden>
      <div className="absolute inset-x-3 top-4 flex h-7 items-center justify-end gap-3 rounded-[9px] bg-white px-3 text-[10px] font-[600] text-muted" style={{ boxShadow: `inset 0 0 0 1px ${FAINT}` }}>
        <span className="h-2.5 w-4 rounded-[3px]" style={{ boxShadow: `inset 0 0 0 1px ${STROKE}` }} />
        <span className="grid h-5 w-5 place-items-center rounded-[6px]" style={{ background: "rgba(255,224,166,.6)" }}>
          <Mark size={13} />
        </span>
        <span>9:41</span>
      </div>
      <div className="absolute right-3 top-14 w-[210px] rounded-[16px] bg-white p-3" style={{ boxShadow: `inset 0 0 0 1px ${STROKE}, 0 18px 36px -20px rgba(30,27,24,.45)` }}>
        <div className="flex items-center gap-2.5">
          <span className="relative h-9 w-9">
            <span className="absolute inset-0" style={{ animation: "la-swap-b 5s ease-in-out infinite", opacity: 0 }}>
              <Mochi size={36} mood="happy" breathe={false} />
            </span>
            <span className="absolute inset-0" style={{ animation: "la-swap-a 5s ease-in-out infinite" }}>
              <Mochi size={36} mood="sleep" breathe={false} />
            </span>
          </span>
          <span className="min-w-0 flex-1">
            <span className="block text-[11.5px] font-[650]">Noise cancellation</span>
            <span className="relative block h-3.5 text-[9.5px] text-muted">
              <span className="absolute inset-0" style={{ animation: "la-swap-a 5s ease-in-out infinite" }}>
                Off
              </span>
              <span className="absolute inset-0" style={{ animation: "la-swap-b 5s ease-in-out infinite", opacity: 0 }}>
                On · apps hear Hush
              </span>
            </span>
          </span>
          <span className="relative h-[22px] w-[38px] shrink-0 rounded-full" style={{ background: "rgba(30,27,24,.14)" }}>
            <span className="absolute inset-0 rounded-full" style={{ background: "var(--h-opal-strong)", animation: "la-swap-b 5s ease-in-out infinite", opacity: 0 }} />
            <span className="absolute left-[3px] top-[3px] h-4 w-4 rounded-full bg-white" style={{ boxShadow: "0 1px 3px rgba(30,27,24,.3)", animation: "la-knob 5s ease-in-out infinite" }} />
          </span>
        </div>
        <div className="mt-2.5 flex h-4 items-end gap-[2px]">
          {Array.from({ length: 22 }, (_, i) => (
            <span
              key={i}
              className="h-full flex-1 origin-bottom rounded-full bg-green/70"
              style={{ ["--s" as string]: (0.25 + ((i * 37) % 10) / 13).toFixed(2), transform: "scaleY(.12)", animation: "la-meter 5s ease-in-out infinite" }}
            />
          ))}
        </div>
      </div>
      <svg className="absolute right-[22px] top-[84px]" width="16" height="16" viewBox="0 0 24 24" style={{ animation: "la-cursor 5s ease-in-out infinite" }}>
        <path d="M5 3l14 8-6.5 1.5L9 19z" fill="#1e1b18" stroke="#fff" strokeWidth="1.5" strokeLinejoin="round" />
      </svg>
    </div>
  );
}

// ------------------------------------------------------------------ Automatic updates

export function UpdateArt() {
  return (
    <div className="relative mx-auto grid h-full w-full max-w-[300px] place-items-center" aria-hidden>
      <div className="w-[230px] rounded-[16px] bg-white p-3.5" style={{ boxShadow: `inset 0 0 0 1px ${STROKE}, 0 18px 36px -22px rgba(30,27,24,.45)` }}>
        <div className="flex items-center gap-2">
          <Mark size={18} />
          <span className="text-[11.5px] font-[650]">Hush AI</span>
          <span className="ml-auto flex items-center gap-1 rounded-full px-1.5 py-0.5 text-[9.5px] font-[650] text-green" style={{ background: "rgba(31,157,114,.1)" }}>
            <ShieldIcon size={11} /> Signed
          </span>
        </div>
        <div className="relative mt-3 h-3.5 text-[10.5px] text-ink-2">
          <span className="absolute inset-0" style={{ animation: "la-swap-a 6s ease-in-out infinite" }}>
            Downloading the new version…
          </span>
          <span className="absolute inset-0 font-[600] text-ink" style={{ animation: "la-swap-b 6s ease-in-out infinite", opacity: 0 }}>
            Ready. It installs when you restart.
          </span>
        </div>
        <div className="mt-2 h-1.5 overflow-hidden rounded-full" style={{ background: FAINT }}>
          <div className="h-full w-full origin-left rounded-full" style={{ background: "var(--h-opal-strong)", animation: "la-fill 6s ease-in-out infinite", transform: "scaleX(0)" }} />
        </div>
        <div className="mt-3 flex justify-end">
          <span className="rounded-full bg-ink px-3 py-1.5 text-[10.5px] font-[620] text-white" style={{ animation: "la-pop 6s ease-in-out infinite", opacity: 0 }}>
            Restart to update
          </span>
        </div>
      </div>
    </div>
  );
}

// ------------------------------------------------------------------ Every desktop

function Device({ label, children, delay }: { label: string; children: ReactNode; delay: number }) {
  return (
    <div className="flex flex-col items-center gap-2" style={{ animation: `la-float 6s ease-in-out ${delay}s infinite` }}>
      {children}
      <span className="text-[10.5px] font-[650] text-muted">{label}</span>
    </div>
  );
}

export function PlatformsArt() {
  return (
    <div className="la-devices flex h-full w-full items-end justify-center gap-3 pb-2 sm:gap-8" aria-hidden>
      <Device label="Windows" delay={0}>
        <svg viewBox="0 0 96 70" className="h-auto w-[78px] sm:w-[96px]">
          <rect x="8" y="2" width="80" height="54" rx="6" fill="#fff" stroke={STROKE} />
          <g transform="translate(38 19)" fill="rgba(23,143,138,.75)">
            <rect width="9" height="9" rx="1" />
            <rect x="11" width="9" height="9" rx="1" />
            <rect y="11" width="9" height="9" rx="1" />
            <rect x="11" y="11" width="9" height="9" rx="1" />
          </g>
          <path d="M0 58 H96 L92 66 H4 Z" fill="#fff" stroke={STROKE} />
        </svg>
      </Device>
      <Device label="macOS" delay={0.6}>
        <svg viewBox="0 0 96 70" className="h-auto w-[78px] sm:w-[96px]">
          <rect x="8" y="2" width="80" height="54" rx="7" fill="#fff" stroke={STROKE} />
          <rect x="14" y="8" width="68" height="5" rx="2" fill={FAINT} />
          <g transform="translate(36 18) scale(.38)">
            <path d="M4 40C4 21 16.5 8 32 8C47.5 8 60 21 60 40C60 50.5 52 56 32 56C12 56 4 50.5 4 40Z" fill="url(#la-pf-opal)" stroke={STROKE} strokeWidth="2" />
            <defs>
              <linearGradient id="la-pf-opal" x1="0" y1="0" x2="1" y2="1">
                <stop offset="0" stopColor="#ffc4b0" />
                <stop offset="1" stopColor="#9be6e0" />
              </linearGradient>
            </defs>
          </g>
          <path d="M0 58 H96 Q96 64 90 64 H6 Q0 64 0 58 Z" fill="#fff" stroke={STROKE} />
        </svg>
      </Device>
      <Device label="Linux" delay={1.2}>
        <svg viewBox="0 0 96 70" className="h-auto w-[78px] sm:w-[96px]">
          <rect x="4" y="4" width="88" height="60" rx="8" fill="#fff" stroke={STROKE} />
          <path d="M4 16 H92" stroke={FAINT} />
          <path d="M14 30 l6 5 -6 5" fill="none" stroke="#1f9d72" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
          <rect x="26" y="38" width="10" height="2.4" rx="1.2" fill="#1f9d72" style={{ animation: "la-blink 1.1s steps(2) infinite" }} />
          <rect x="14" y="48" width="44" height="3" rx="1.5" fill={FAINT} />
        </svg>
      </Device>
    </div>
  );
}
