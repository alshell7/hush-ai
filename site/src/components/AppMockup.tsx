// Static pictures of the app, drawn with the app's own tokens. Used on small screens and
// while the live demo loads (or when it is not there, as in local development).
import { m } from "motion/react";
import { useState } from "react";
import { ExternalIcon, MicIcon } from "./Icons";
import { Mark, Mochi, Wordmark } from "./Mochi";

const METER_IN = [0.3, 0.5, 0.42, 0.7, 0.55, 0.36, 0.62, 0.8, 0.46, 0.38, 0.58, 0.66, 0.4, 0.52, 0.3, 0.44];
const METER_OUT = [0.12, 0.42, 0.36, 0.6, 0.5, 0.2, 0.54, 0.7, 0.4, 0.16, 0.5, 0.58, 0.22, 0.46, 0.1, 0.3];

function PowerDisc({ size = 168, on = true }: { size?: number; on?: boolean }) {
  const inner = size - 22;
  return (
    <div className="relative grid shrink-0 place-items-center rounded-full" style={{ width: size, height: size }}>
      <span
        className="absolute inset-0 rounded-full transition-opacity duration-500"
        style={{
          background: "var(--h-opal-conic)",
          opacity: on ? 1 : 0,
          WebkitMask: "radial-gradient(farthest-side, transparent calc(100% - 4px), #000 calc(100% - 3.5px))",
          mask: "radial-gradient(farthest-side, transparent calc(100% - 4px), #000 calc(100% - 3.5px))",
        }}
      />
      <span
        className="absolute inset-0 rounded-full border-2 border-dashed transition-opacity duration-500"
        style={{ borderColor: "rgba(30,27,24,.16)", opacity: on ? 0 : 1 }}
      />
      <span className="absolute rounded-full blur-2xl transition-opacity duration-700" style={{ inset: 14, background: "var(--h-opal-strong)", opacity: on ? 0.5 : 0 }} />
      <span
        className="relative grid place-items-center rounded-full transition-[background] duration-500"
        style={{
          width: inner,
          height: inner,
          background: on ? "var(--h-opal)" : "linear-gradient(180deg,#ffffff,#f4f1ec)",
          boxShadow: on
            ? "inset 0 2px 0 rgba(255,255,255,.8), inset 0 -10px 24px rgba(232,115,90,.12), 0 18px 40px -16px rgba(232,115,90,.55)"
            : "inset 0 2px 0 #fff, 0 14px 34px -18px rgba(30,27,24,.4)",
        }}
      >
        <Mochi size={inner * 0.62} mood={on ? "happy" : "sleep"} breathe={false} />
      </span>
    </div>
  );
}

function Meter({ values, color, label }: { values: number[]; color: string; label: string }) {
  return (
    <div className="tile p-3">
      <div className="text-[11.5px] font-[560] text-muted">{label}</div>
      <div className="mt-2 flex h-7 items-end gap-[3px]">
        {values.map((v, i) => (
          <span key={i} className="flex-1 rounded-full" style={{ height: `${Math.max(12, v * 100)}%`, background: color }} />
        ))}
      </div>
    </div>
  );
}

function MicField({ name = "MacBook Pro Microphone" }: { name?: string }) {
  return (
    <div className="flex h-11 items-center gap-2.5 rounded-[14px] bg-white px-3.5 text-[13.5px]" style={{ boxShadow: "inset 0 0 0 1px var(--h-line-2)" }}>
      <MicIcon size={17} className="text-muted" />
      <span className="text-[11.5px] font-[560] text-muted">Microphone</span>
      <span className="ml-auto truncate font-[560] text-ink">{name}</span>
      <span className="text-faint">▾</span>
    </div>
  );
}

function Switch({ on, size = "md" }: { on: boolean; size?: "md" | "lg" }) {
  const w = size === "lg" ? 58 : 44;
  const h = size === "lg" ? 34 : 26;
  const knob = h - 6;
  return (
    <span
      className="relative block shrink-0 rounded-full transition-[background] duration-300"
      style={{ width: w, height: h, background: on ? "var(--h-opal-strong)" : "rgba(30,27,24,0.14)" }}
    >
      <span
        className="absolute top-[3px] rounded-full bg-white transition-[left] duration-300"
        style={{ width: knob, height: knob, left: on ? w - knob - 3 : 3, boxShadow: "0 2px 6px rgba(30,27,24,.25)" }}
      />
    </span>
  );
}

/** The left card of the main window: the switch and your microphone. */
export function AppHeroCard({ className = "" }: { className?: string }) {
  return (
    <div className={`glass flex flex-col p-6 ${className}`}>
      <div className="flex items-center justify-between">
        <span className="text-[11px] font-semibold uppercase tracking-[0.08em] text-muted">Noise cancellation</span>
        <span className="chip chip-green h-6 text-[11.5px]">
          <span className="dot" /> On
        </span>
      </div>
      <div className="flex flex-1 flex-col items-center justify-center gap-5 py-5">
        <PowerDisc />
        <div className="text-center">
          <div className="text-[23px] font-[640] tracking-[-0.03em]">Your voice, without the noise</div>
          <p className="mx-auto mt-1 max-w-[330px] text-[13.5px] leading-snug text-muted">Apps hear you through Hush AI Microphone.</p>
        </div>
      </div>
      <div className="space-y-3">
        <MicField />
        <div className="grid grid-cols-2 gap-3">
          <Meter label="Your microphone" values={METER_IN} color="rgba(30,27,24,.45)" />
          <Meter label="Hush AI Microphone" values={METER_OUT} color="#1f9d72" />
        </div>
      </div>
    </div>
  );
}

/** The whole main window at its real size, 940 × 640. */
export function AppScreen() {
  return (
    <div className="relative h-[640px] w-[940px] overflow-hidden bg-paper text-ink">
      <div className="aurora" />
      <div className="relative flex h-14 items-center px-5" style={{ paddingLeft: 86 }}>
        <Wordmark size={26} />
      </div>
      <div className="relative grid h-[584px] grid-cols-[1.08fr_1fr] gap-4 px-5 pb-5">
        <AppHeroCard />
        <div className="flex min-h-0 flex-col gap-4">
          <div className="glass p-5">
            <div className="flex items-start justify-between gap-3">
              <div>
                <div className="flex items-center gap-2 text-[15px] font-semibold">
                  <Mark size={18} /> Hush AI Microphone
                </div>
                <p className="mt-1 text-[13px] leading-snug text-muted">Choose it as the microphone in your apps. Hover an app to see where.</p>
              </div>
              <span className="chip chip-green shrink-0">
                <span className="dot" /> Ready
              </span>
            </div>
            <div className="mt-3 flex flex-wrap gap-1.5">
              {["Zoom", "Google Meet", "Microsoft Teams", "Discord", "Slack", "OBS Studio"].map((n) => (
                <span key={n} className="chip bg-white/80" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
                  {n}
                </span>
              ))}
            </div>
          </div>
          <div className="glass p-5">
            <div className="mb-3 flex items-center justify-between">
              <span className="text-[15px] font-semibold">Quality</span>
            </div>
            <div className="flex rounded-full p-[3px]" style={{ background: "rgba(30,27,24,0.06)" }}>
              {["Light", "Balanced", "Studio"].map((q, i) => (
                <span
                  key={q}
                  className="flex h-[38px] flex-1 items-center justify-center rounded-full text-[13.5px] font-[560]"
                  style={i === 0 ? { background: "#fff", boxShadow: "0 1px 2px rgba(30,27,24,.08), 0 4px 12px -4px rgba(30,27,24,.16)" } : { color: "var(--h-muted)" }}
                >
                  {q}
                </span>
              ))}
            </div>
            <p className="mt-2 text-[12.5px] text-muted">16 kHz voice band. Lowest delay and CPU. Best for calls.</p>
          </div>
          <div className="glass flex items-center justify-between gap-3 p-5">
            <div>
              <div className="text-[15px] font-semibold">Listen to yourself</div>
              <div className="text-[12.5px] text-muted">Hear Hush in your headphones</div>
            </div>
            <Switch on={false} />
          </div>
        </div>
      </div>
    </div>
  );
}

/** The tray / menu-bar widget. Its switch works, for fun. */
export function WidgetMock() {
  const [on, setOn] = useState(true);
  return (
    <div
      className="relative w-full max-w-[340px] overflow-hidden rounded-[24px] p-4 text-left"
      style={{
        background: "rgba(253,252,250,.94)",
        boxShadow: "inset 0 0 0 1px rgba(255,255,255,.9), 0 0 0 1px rgba(30,27,24,.08), 0 30px 60px -24px rgba(30,27,24,.45)",
      }}
    >
      <div
        className="pointer-events-none absolute -right-16 -top-20 h-48 w-48 rounded-full blur-3xl transition-opacity duration-700"
        style={{ background: "var(--h-opal)", opacity: on ? 0.9 : 0.3 }}
      />
      <div className="relative flex items-center justify-between">
        <div className="flex items-center gap-2 text-[14px] font-[650] tracking-[-0.02em]">
          <Mark size={20} /> Hush AI
        </div>
        <span className="grid h-8 w-8 place-items-center rounded-full text-ink-2" aria-hidden>
          <ExternalIcon size={16} />
        </span>
      </div>
      <div className="relative mt-3 flex items-center gap-4 rounded-[18px] bg-white/80 p-4" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
        <Mochi size={54} mood={on ? "happy" : "sleep"} breathe={false} />
        <div className="min-w-0 flex-1">
          <div className="text-[15px] font-[640] tracking-[-0.02em]">Noise cancellation</div>
          <div className="truncate text-[12px] text-muted">{on ? "On · apps hear Hush AI Microphone" : "Off"}</div>
        </div>
        <button
          type="button"
          role="switch"
          aria-checked={on}
          aria-label="Noise cancellation (demo)"
          onClick={() => setOn((v) => !v)}
          className="cursor-pointer rounded-full"
        >
          <Switch on={on} size="lg" />
        </button>
      </div>
      <div className="relative mt-3 flex items-center gap-2 rounded-[14px] bg-white/70 px-3 py-2.5 text-[12.5px]" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
        <MicIcon size={15} className="text-muted" />
        <span className="truncate font-[560]">MacBook Pro Microphone</span>
        <span className="ml-auto text-faint">▾</span>
      </div>
      <div className="relative mt-3 flex h-6 items-end gap-[3px] px-1">
        {METER_OUT.concat(METER_IN).map((v, i) => (
          <m.span
            key={i}
            className="flex-1 rounded-full"
            initial={false}
            animate={{ height: `${on ? Math.max(14, v * 100) : 12}%`, backgroundColor: on ? "#1f9d72" : "rgba(30,27,24,.16)" }}
            transition={{ duration: 0.4, delay: i * 0.01 }}
          />
        ))}
      </div>
    </div>
  );
}
