// The Live tab: the switch, your microphone, what apps hear, and the few settings people
// change often.
import { AnimatePresence, motion } from "motion/react";
import { createPortal } from "react-dom";
import { useEffect, useRef, useState } from "react";
import { AppGuideTip, GUIDES } from "../components/AppGuides";
import { AppIcon } from "../components/AppIcons";
import { DevicePicker, Segmented, Tip } from "../components/Controls";
import { AgentIcon, ArrowRightIcon, CalendarIcon, ChartIcon, LockIcon, MicIcon, SparkleIcon, TranslateIcon } from "../components/Icons";
import { LevelStrip, useMeters } from "../components/Meters";
import { Mark } from "../components/Mochi";
import { ModelDownload } from "../components/ModelDownload";
import { NoiseMorph } from "../components/NoiseMorph";
import { PowerHero } from "../components/PowerHero";
import { TranslateCard } from "../components/TranslateCard";
import { VmicSetup } from "../components/VmicSetup";
import { ENGINES, useEngineOrder } from "../lib/engines";
import { features, useFeatures } from "../lib/features";
import { api, type Quality } from "../lib/ipc";
import { refreshDevices, refreshVmic, saveSettings, useApp } from "../lib/store";
import { clockTime, relative, useNow } from "../lib/time";

const QUALITY: { value: Quality; label: string; hint: string }[] = [
  { value: "light", label: "Light", hint: "16 kHz voice band. Lowest delay and CPU. Best for calls." },
  { value: "balanced", label: "Balanced", hint: "24 kHz. Clearer voice; uses more CPU." },
  { value: "studio", label: "Studio", hint: "48 kHz full band. For recording and streaming. Needs a fast CPU." },
];

export const ROADMAP = [
  { icon: SparkleIcon, title: "Noise cancellation", state: "done" },
  { icon: TranslateIcon, title: "Voice translation", state: "preview" },
  { icon: AgentIcon, title: "Agent assist", state: "soon" },
  { icon: ChartIcon, title: "Speech analytics", state: "soon" },
  { icon: LockIcon, title: "Voice security", state: "soon" },
] as const;

export function Live({ onOpenMeetings, onOpenTranslate }: { onOpenMeetings: () => void; onOpenTranslate: () => void }) {
  const settings = useApp((s) => s.settings)!;
  const engine = useApp((s) => s.engine)!;
  const engineOrder = useEngineOrder();
  const devices = useApp((s) => s.devices);
  const vmic = useApp((s) => s.vmic);
  const [setupOpen, setSetupOpen] = useState(false);
  const load = useRef<HTMLSpanElement>(null);

  // A session can run for Live Translate alone; noise cancellation is what the setting says.
  const status = settings.enabled ? engine.status : "off";
  const on = status === "on";
  const busy = status === "starting";

  // Meters arrive 20 times a second; the load is averaged over about two seconds and shown
  // twice a second, so the number settles instead of flickering.
  const smooth = useRef({ value: -1, shown: 0 });
  useMeters((m) => {
    const s = smooth.current;
    const sample = Math.min(m.load, 9.99);
    s.value = s.value < 0 ? sample : s.value + (sample - s.value) * 0.05;
    const now = performance.now();
    if (load.current && now - s.shown > 500) {
      s.shown = now;
      load.current.textContent = `${Math.round(s.value * 100)}%`;
    }
  });

  // Devices come and go; refresh while the window is in front.
  useEffect(() => {
    const refresh = () => {
      if (document.visibilityState === "visible") {
        refreshDevices();
        refreshVmic();
      }
    };
    window.addEventListener("focus", refresh);
    const timer = setInterval(refresh, 8000);
    return () => {
      window.removeEventListener("focus", refresh);
      clearInterval(timer);
    };
  }, []);

  const toggle = () => api.setEnabled(!settings.enabled);

  const headline = status === "error" ? "Hush needs a moment" : on ? "Your voice, without the noise" : busy ? "Waking up…" : "Noise cancellation is off";
  const sub = status === "error" ? engine.message : on ? "Apps hear you through Hush AI Microphone." : "Your apps still hear you through Hush AI Microphone, just without the cleanup. No need to switch microphones.";

  return (
    <>
      <main className="grid min-h-0 flex-1 grid-cols-[1.08fr_1fr] gap-4">
        {/* Hero */}
        <section className="glass flex min-h-0 flex-col p-6">
          <div className="flex items-center justify-between">
            <span className="eyebrow">Noise cancellation</span>
            <StatusChip status={status} />
          </div>
          <div className="flex flex-1 flex-col items-center justify-center gap-5 py-2">
            <PowerHero status={status} onToggle={toggle} />
            <div className="text-center">
              <AnimatePresence mode="wait">
                <motion.h1
                  key={headline}
                  initial={{ opacity: 0, y: 6, filter: "blur(4px)" }}
                  animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
                  exit={{ opacity: 0, y: -6, filter: "blur(4px)" }}
                  transition={{ duration: 0.3 }}
                  className="text-[23px] font-[640] tracking-[-0.03em]"
                >
                  {headline}
                </motion.h1>
              </AnimatePresence>
              <p className="mx-auto mt-1 max-w-[330px] text-[13.5px] leading-snug text-muted">{sub}</p>
            </div>
          </div>
          <div className="space-y-3">
            <DevicePicker
              label="Microphone"
              icon={<MicIcon size={17} />}
              devices={devices?.inputs ?? [{ name: null, label: "System default" }]}
              value={settings.input}
              onChange={(input) => saveSettings({ input })}
            />
            <div className="grid grid-cols-2 gap-3">
              <MeterTile label="Your microphone" source="input" color="rgba(30,27,24,.5)" idle={!on} />
              <MeterTile label="Hush AI Microphone" source="output" color="#1f9d72" idle={!on} accent />
            </div>
          </div>
        </section>

        {/* Side */}
        <section className="scroll -mr-2 flex min-h-0 flex-col gap-4 pr-2">
          <Card>
            <div className="flex items-start justify-between gap-3">
              <div>
                <div className="flex items-center gap-2 text-[15px] font-semibold">
                  <Mark size={18} /> Hush AI Microphone
                </div>
                <p className="mt-1 text-[13px] leading-snug text-muted">Choose it as the microphone in your apps. Hover an app to watch where.</p>
              </div>
              {vmic && vmic.state !== "ready" ? (
                <button className="btn btn-primary btn-sm shrink-0" onClick={() => setSetupOpen(true)}>
                  Set up
                </button>
              ) : (
                <span className="chip shrink-0 text-green" style={{ background: "rgba(31,157,114,.1)" }}>
                  <span className="dot" /> Ready
                </span>
              )}
            </div>
            <div className="mt-3 flex flex-wrap gap-2">
              {GUIDES.map((guide) => (
                <AppGuideTip key={guide.app} app={guide.app} />
              ))}
            </div>
          </Card>

          <TranslateCard onOpen={onOpenTranslate} />

          <NextMeetingCard onOpenMeetings={onOpenMeetings} />

          <Card>
            <div className="mb-3 flex items-center justify-between">
              <span className="text-[15px] font-semibold">Voice model</span>
              <Tip content="How much of real time the model needs. Below 100% it keeps up.">
                <span className="chip cursor-default">
                  Load <span ref={load} className="tabular-nums">{on ? "…" : "0%"}</span>
                </span>
              </Tip>
            </div>
            <Segmented
              label="Voice model"
              value={settings.engine}
              options={engineOrder.map((engine) => ({ value: engine, label: ENGINES[engine].label, hint: ENGINES[engine].hint }))}
              onChange={(engine) => saveSettings({ engine })}
            />
            <AnimatePresence initial={false}>
              {settings.engine === "reuse" && (
                <motion.div initial={{ height: 0, opacity: 0 }} animate={{ height: "auto", opacity: 1 }} exit={{ height: 0, opacity: 0 }} className="overflow-hidden">
                  <div className="pt-3">
                    <Segmented size="sm" label="Quality" value={settings.quality} options={QUALITY.map((q) => ({ value: q.value, label: q.label, hint: q.hint }))} onChange={(quality) => saveSettings({ quality })} />
                  </div>
                </motion.div>
              )}
            </AnimatePresence>
            <ModelDownload key={settings.engine} engine={settings.engine} />
          </Card>

          <Card>
            <div className="mb-3 flex items-center justify-between">
              <span className="text-[15px] font-semibold">Noise in, voice out</span>
              <span className="eyebrow">{on ? "Working" : "When on"}</span>
            </div>
            <NoiseMorph />
            <p className="mt-2.5 text-[12px] leading-snug text-muted">Keyboards, fans, dogs and traffic stay with Hush. Your apps get only your voice.</p>
          </Card>

          <Card>
            <div className="mb-2 flex items-center justify-between">
              <span className="text-[15px] font-semibold">What's next</span>
              <span className="eyebrow">Roadmap</span>
            </div>
            <ul className="space-y-1">
              {ROADMAP.map(({ icon: Icon, title, state }) => (
                <li key={title} className="flex items-center gap-3 rounded-xl px-1 py-1.5">
                  <Icon size={18} className={state === "soon" ? "text-faint" : "text-green"} />
                  <span className={`flex-1 text-[13.5px] ${state === "soon" ? "text-ink-2" : "font-[560]"}`}>{title}</span>
                  <span className={`chip h-6 text-[11px] ${state === "soon" ? "text-muted" : "text-green"}`} style={state === "soon" ? undefined : { background: "rgba(31,157,114,.1)" }}>
                    {state === "done" ? "Available" : state === "preview" ? "Preview" : "Coming soon"}
                  </span>
                </li>
              ))}
            </ul>
          </Card>
        </section>
      </main>

      <AnimatePresence>
        {setupOpen && (
          <Modal onClose={() => setSetupOpen(false)} title="Set up Hush AI Microphone">
            <VmicSetup compact onReady={() => setTimeout(() => setSetupOpen(false), 900)} />
          </Modal>
        )}
      </AnimatePresence>
    </>
  );
}

/** The next call, or an invitation to connect the calendar. */
function NextMeetingCard({ onOpenMeetings }: { onOpenMeetings: () => void }) {
  const calendar = useFeatures((s) => s.calendar);
  const now = useNow(30_000);
  const next = calendar?.meetings.find((m) => !m.allDay && m.end > now && m.start - now < 18 * 3600_000);

  if (!calendar?.connected) {
    return (
      <button className="glass group flex shrink-0 items-center gap-3 p-4 text-left transition-transform duration-200 hover:-translate-y-0.5" onClick={onOpenMeetings}>
        <span className="grid h-10 w-10 place-items-center rounded-2xl bg-[#f4efe8]">
          <CalendarIcon size={19} />
        </span>
        <span className="min-w-0 flex-1">
          <span className="block text-[14px] font-semibold">Turn on before every call</span>
          <span className="block text-[12.5px] text-muted">Connect Google Calendar, no sign-in.</span>
        </span>
        <ArrowRightIcon size={17} className="text-muted transition-transform group-hover:translate-x-0.5" />
      </button>
    );
  }
  if (!next) return null;
  return (
    <div className="glass flex shrink-0 items-center gap-3 p-4">
      {next.app ? <AppIcon app={next.app} size={20} tile /> : <span className="grid h-[35px] w-[35px] place-items-center rounded-xl bg-[#f4efe8]"><CalendarIcon size={18} /></span>}
      <button className="min-w-0 flex-1 text-left" onClick={onOpenMeetings}>
        <span className="block truncate text-[14px] font-semibold">{next.title}</span>
        <span className="block text-[12.5px] text-muted">
          {clockTime(next.start)} · {relative(next.start, next.end, now)}
        </span>
      </button>
      {next.app && (
        <button className="btn btn-primary btn-sm" onClick={() => features.calendarJoin(next.id)}>
          Join
        </button>
      )}
    </div>
  );
}

function Card({ children }: { children: React.ReactNode }) {
  return <div className="glass shrink-0 p-5">{children}</div>;
}

function MeterTile({ label, source, color, idle, accent }: { label: string; source: "input" | "output"; color: string; idle: boolean; accent?: boolean }) {
  return (
    <div className="tile p-3" style={accent ? { background: "linear-gradient(180deg, rgba(201,242,212,.35), rgba(255,255,255,.7))" } : undefined}>
      <div className="mb-1.5 truncate text-[11.5px] font-semibold text-muted">{label}</div>
      <LevelStrip source={source} color={color} height={34} idle={idle} />
    </div>
  );
}

export function StatusChip({ status }: { status: string }) {
  const map: Record<string, { text: string; color: string; bg: string }> = {
    on: { text: "On", color: "var(--h-green)", bg: "rgba(31,157,114,.1)" },
    starting: { text: "Starting", color: "var(--h-amber)", bg: "rgba(194,124,18,.1)" },
    error: { text: "Needs attention", color: "var(--h-red)", bg: "rgba(214,69,75,.1)" },
    off: { text: "Off", color: "var(--h-muted)", bg: "rgba(30,27,24,.06)" },
  };
  const s = map[status] ?? map.off;
  return (
    <motion.span layout className="chip" style={{ color: s.color, background: s.bg }}>
      <span className="dot" />
      {s.text}
    </motion.span>
  );
}

export function Modal({ title, onClose, children }: { title: string; onClose: () => void; children: React.ReactNode }) {
  useEffect(() => {
    const key = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [onClose]);
  return createPortal(
    <motion.div className="fixed inset-0 z-40 grid place-items-center p-6" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}>
      <div className="absolute inset-0 bg-[rgba(30,27,24,.22)] backdrop-blur-[3px]" onClick={onClose} />
      <motion.div
        role="dialog"
        aria-label={title}
        initial={{ opacity: 0, y: 14, scale: 0.97 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        exit={{ opacity: 0, y: 8, scale: 0.98 }}
        transition={{ type: "spring", stiffness: 380, damping: 32 }}
        className="relative w-full max-w-[560px] rounded-[28px] bg-[#fdfcfa] p-6"
        style={{ boxShadow: "var(--h-shadow-3)" }}
      >
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-[19px] font-[640] tracking-[-0.02em]">{title}</h2>
          <button className="btn btn-ghost btn-icon h-8 w-8" aria-label="Close" onClick={onClose}>
            ✕
          </button>
        </div>
        {children}
      </motion.div>
    </motion.div>,
    document.body,
  );
}
