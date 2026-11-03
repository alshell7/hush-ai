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
          7%
        </text>
        <text x="0" y="52" fontSize="10.5" fontWeight="560" fill="#736c63">
          of one core
        </text>
        <text x="0" y="86" fontSize="9.5" fontWeight="600" fill="#736c63">
          Of an 8-core laptop: under 1%
        </text>
        <rect x="0" y="92" width="96" height="5" rx="2.5" fill={FAINT} />
        <rect x="0" y="92" width="96" height="5" rx="2.5" fill="#1f9d72" className="la-fill-box la-origin-left" style={{ transform: "scaleX(.03)" }} />
        <text x="0" y="114" fontSize="9.5" fontWeight="600" fill="#736c63">
          Free for your call: 99%
        </text>
        <rect x="0" y="120" width="96" height="5" rx="2.5" fill="#9be6e0" opacity=".8" className="la-fill-box la-origin-left" style={{ transform: "scaleX(.99)" }} />
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
          <g transform="translate(37 18) scale(.92)">
            <path d="M12.152 6.896c-.948 0-2.415-1.078-3.96-1.04-2.04.027-3.91 1.183-4.961 3.014-2.117 3.675-.546 9.103 1.519 12.09 1.013 1.454 2.208 3.09 3.792 3.039 1.52-.065 2.09-.987 3.935-.987 1.831 0 2.35.987 3.96.948 1.637-.026 2.676-1.48 3.676-2.948 1.156-1.688 1.636-3.325 1.662-3.415-.039-.013-3.182-1.221-3.22-4.857-.026-3.04 2.48-4.494 2.597-4.559-1.429-2.09-3.623-2.324-4.39-2.376-2-.156-3.675 1.09-4.61 1.09zM15.53 3.83c.843-1.012 1.4-2.427 1.245-3.83-1.207.052-2.662.805-3.532 1.818-.78.896-1.454 2.338-1.273 3.714 1.338.104 2.715-.688 3.559-1.701" fill="#1e1b18" opacity=".82" />
          </g>
          <path d="M0 58 H96 Q96 64 90 64 H6 Q0 64 0 58 Z" fill="#fff" stroke={STROKE} />
        </svg>
      </Device>
      <Device label="Linux" delay={1.2}>
        <svg viewBox="0 0 96 70" className="h-auto w-[78px] sm:w-[96px]">
          <rect x="4" y="4" width="88" height="60" rx="8" fill="#fff" stroke={STROKE} />
          <path d="M4 16 H92" stroke={FAINT} />
          <g transform="translate(36 22) scale(1)">
            <path d="M12.504 0c-.155 0-.315.008-.48.021-4.226.333-3.105 4.807-3.17 6.298-.076 1.092-.3 1.953-1.05 3.02-.885 1.051-2.127 2.75-2.716 4.521-.278.832-.41 1.684-.287 2.489a.424.424 0 00-.11.135c-.26.268-.45.6-.663.839-.199.199-.485.267-.797.4-.313.136-.658.269-.864.68-.09.189-.136.394-.132.602 0 .199.027.4.055.536.058.399.116.728.04.97-.249.68-.28 1.145-.106 1.484.174.334.535.47.94.601.81.2 1.91.135 2.774.6.926.466 1.866.67 2.616.47.526-.116.97-.464 1.208-.946.587-.003 1.23-.269 2.26-.334.699-.058 1.574.267 2.577.2.025.134.063.198.114.333l.003.003c.391.778 1.113 1.132 1.884 1.071.771-.06 1.592-.536 2.257-1.306.631-.765 1.683-1.084 2.378-1.503.348-.199.629-.469.649-.853.023-.4-.2-.811-.714-1.376v-.097l-.003-.003c-.17-.2-.25-.535-.338-.926-.085-.401-.182-.786-.492-1.046h-.003c-.059-.054-.123-.067-.188-.135a.357.357 0 00-.19-.064c.431-1.278.264-2.55-.173-3.694-.533-1.41-1.465-2.638-2.175-3.483-.796-1.005-1.576-1.957-1.56-3.368.026-2.152.236-6.133-3.544-6.139zm.529 3.405h.013c.213 0 .396.062.584.198.19.135.33.332.438.533.105.259.158.459.166.724 0-.02.006-.04.006-.06v.105a.086.086 0 01-.004-.021l-.004-.024a1.807 1.807 0 01-.15.706.953.953 0 01-.213.335.71.71 0 00-.088-.042c-.104-.045-.198-.064-.284-.133a1.312 1.312 0 00-.22-.066c.05-.06.146-.133.183-.198.053-.128.082-.264.088-.402v-.02a1.21 1.21 0 00-.061-.4c-.045-.134-.101-.2-.183-.333-.084-.066-.167-.132-.267-.132h-.016c-.093 0-.176.03-.262.132a.8.8 0 00-.205.334 1.18 1.18 0 00-.09.4v.019c.002.089.008.179.02.267-.193-.067-.438-.135-.607-.202a1.635 1.635 0 01-.018-.2v-.02a1.772 1.772 0 01.15-.768c.082-.22.232-.406.43-.533a.985.985 0 01.594-.2zm-2.962.059h.036c.142 0 .27.048.399.135.146.129.264.288.344.465.09.199.14.4.153.667v.004c.007.134.006.2-.002.266v.08c-.03.007-.056.018-.083.024-.152.055-.274.135-.393.2.012-.09.013-.18.003-.267v-.015c-.012-.133-.04-.2-.082-.333a.613.613 0 00-.166-.267.248.248 0 00-.183-.064h-.021c-.071.006-.13.04-.186.132a.552.552 0 00-.12.27.944.944 0 00-.023.33v.015c.012.135.037.2.08.334.046.134.098.2.166.268.01.009.02.018.034.024-.07.057-.117.07-.176.136a.304.304 0 01-.131.068 2.62 2.62 0 01-.275-.402 1.772 1.772 0 01-.155-.667 1.759 1.759 0 01.08-.668 1.43 1.43 0 01.283-.535c.128-.133.26-.2.418-.2zm1.37 1.706c.332 0 .733.065 1.216.399.293.2.523.269 1.052.468h.003c.255.136.405.266.478.399v-.131a.571.571 0 01.016.47c-.123.31-.516.643-1.063.842v.002c-.268.135-.501.333-.775.465-.276.135-.588.292-1.012.267a1.139 1.139 0 01-.448-.067 3.566 3.566 0 01-.322-.198c-.195-.135-.363-.332-.612-.465v-.005h-.005c-.4-.246-.616-.512-.686-.71-.07-.268-.005-.47.193-.6.224-.135.38-.271.483-.336.104-.074.143-.102.176-.131h.002v-.003c.169-.202.436-.47.839-.601.139-.036.294-.065.466-.065zm2.8 2.142c.358 1.417 1.196 3.475 1.735 4.473.286.534.855 1.659 1.102 3.024.156-.005.33.018.513.064.646-1.671-.546-3.467-1.089-3.966-.22-.2-.232-.335-.123-.335.59.534 1.365 1.572 1.646 2.757.13.535.16 1.104.021 1.67.067.028.135.06.205.067 1.032.534 1.413.938 1.23 1.537v-.043c-.06-.003-.12 0-.18 0h-.016c.151-.467-.182-.825-1.065-1.224-.915-.4-1.646-.336-1.77.465-.008.043-.013.066-.018.135-.068.023-.139.053-.209.064-.43.268-.662.669-.793 1.187-.13.533-.17 1.156-.205 1.869v.003c-.02.334-.17.838-.319 1.35-1.5 1.072-3.58 1.538-5.348.334a2.645 2.645 0 00-.402-.533 1.45 1.45 0 00-.275-.333c.182 0 .338-.03.465-.067a.615.615 0 00.314-.334c.108-.267 0-.697-.345-1.163-.345-.467-.931-.995-1.788-1.521-.63-.4-.986-.87-1.15-1.396-.165-.534-.143-1.085-.015-1.645.245-1.07.873-2.11 1.274-2.763.107-.065.037.135-.408.974-.396.751-1.14 2.497-.122 3.854a8.123 8.123 0 01.647-2.876c.564-1.278 1.743-3.504 1.836-5.268.048.036.217.135.289.202.218.133.38.333.59.465.21.201.477.335.876.335.039.003.075.006.11.006.412 0 .73-.134.997-.268.29-.134.52-.334.74-.4h.005c.467-.135.835-.402 1.044-.7zm2.185 8.958c.037.6.343 1.245.882 1.377.588.134 1.434-.333 1.791-.765l.211-.01c.315-.007.577.01.847.268l.003.003c.208.199.305.53.391.876.085.4.154.78.409 1.066.486.527.645.906.636 1.14l.003-.007v.018l-.003-.012c-.015.262-.185.396-.498.595-.63.401-1.746.712-2.457 1.57-.618.737-1.37 1.14-2.036 1.191-.664.053-1.237-.2-1.574-.898l-.005-.003c-.21-.4-.12-1.025.056-1.69.176-.668.428-1.344.463-1.897.037-.714.076-1.335.195-1.814.12-.465.308-.797.641-.984l.045-.022zm-10.814.049h.01c.053 0 .105.005.157.014.376.055.706.333 1.023.752l.91 1.664.003.003c.243.533.754 1.064 1.189 1.637.434.598.77 1.131.729 1.57v.006c-.057.744-.48 1.148-1.125 1.294-.645.135-1.52.002-2.395-.464-.968-.536-2.118-.469-2.857-.602-.369-.066-.61-.2-.723-.4-.11-.2-.113-.602.123-1.23v-.004l.002-.003c.117-.334.03-.752-.027-1.118-.055-.401-.083-.71.043-.94.16-.334.396-.4.69-.533.294-.135.64-.202.915-.47h.002v-.002c.256-.268.445-.601.668-.838.19-.201.38-.336.663-.336zm7.159-9.074c-.435.201-.945.535-1.488.535-.542 0-.97-.267-1.28-.466-.154-.134-.28-.268-.373-.335-.164-.134-.144-.333-.074-.333.109.016.129.134.199.2.096.066.215.2.36.333.292.2.68.467 1.167.467.485 0 1.053-.267 1.398-.466.195-.135.445-.334.648-.467.156-.136.149-.267.279-.267.128.016.034.134-.147.332a8.097 8.097 0 01-.69.468zm-1.082-1.583V5.64c-.006-.02.013-.042.029-.05.074-.043.18-.027.26.004.063 0 .16.067.15.135-.006.049-.085.066-.135.066-.055 0-.092-.043-.141-.068-.052-.018-.146-.008-.163-.065zm-.551 0c-.02.058-.113.049-.166.066-.047.025-.086.068-.14.068-.05 0-.13-.02-.136-.068-.01-.066.088-.133.15-.133.08-.031.184-.047.259-.005.019.009.036.03.03.05v.02h.003z" fill="#1e1b18" opacity=".82" />
          </g>
        </svg>
      </Device>
    </div>
  );
}
