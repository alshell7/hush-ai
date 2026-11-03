// The main screen: the switch, your microphone, what apps hear, and the few settings
// people change often.
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import { DevicePicker, Segmented, Tip, Toggle } from "../components/Controls";
import { AgentIcon, ChartIcon, GearIcon, HeadphonesIcon, LockIcon, MicIcon, SparkleIcon, TranslateIcon, WarningIcon } from "../components/Icons";
import { LevelStrip, useMeters } from "../components/Meters";
import { APPS, AppHint } from "../components/Mockups";
import { Mark, Wordmark } from "../components/Mochi";
import { PowerHero } from "../components/PowerHero";
import { TitleBar } from "../components/TitleBar";
import { VmicSetup } from "../components/VmicSetup";
import { api, type Quality } from "../lib/ipc";
import { refreshDevices, refreshVmic, saveSettings, useApp } from "../lib/store";
import { installUpdate } from "../lib/updater";
import { SettingsPanel } from "./Settings";

const QUALITY: { value: Quality; label: string; hint: string }[] = [
  { value: "light", label: "Light", hint: "16 kHz voice band. Lowest delay and CPU. Best for calls." },
  { value: "balanced", label: "Balanced", hint: "24 kHz. Clearer voice; uses more CPU." },
  { value: "studio", label: "Studio", hint: "48 kHz full band. For recording and streaming. Needs a fast CPU." },
];

export const ROADMAP = [
  { icon: SparkleIcon, title: "Noise cancellation", state: "done" },
  { icon: TranslateIcon, title: "Voice translation", state: "soon" },
  { icon: AgentIcon, title: "Agent assist", state: "soon" },
  { icon: ChartIcon, title: "Speech analytics", state: "soon" },
  { icon: LockIcon, title: "Voice security", state: "soon" },
] as const;

export function Home() {
  const settings = useApp((s) => s.settings)!;
  const engine = useApp((s) => s.engine)!;
  const devices = useApp((s) => s.devices);
  const vmic = useApp((s) => s.vmic);
  const update = useApp((s) => s.update);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [setupOpen, setSetupOpen] = useState(false);
  const load = useRef<HTMLSpanElement>(null);

  const on = engine.status === "on";
  const busy = engine.status === "starting";

  useMeters((m) => {
    if (load.current) load.current.textContent = `${Math.round(Math.min(m.load, 9.99) * 100)}%`;
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

  const toggle = () => api.setEnabled(!(on || busy));

  const headline = engine.status === "error" ? "Hush needs a moment" : on ? "Your voice, without the noise" : busy ? "Waking up…" : "Hush is resting";
  const sub =
    engine.status === "error"
      ? engine.message
      : on
        ? engine.listen
          ? "You hear yourself in your headphones. Apps get your voice again when Listen is off."
          : "Apps hear you through Hush AI Microphone."
        : "Turn on to remove background noise in every app.";

  return (
    <div className="relative flex h-full flex-col">
      <TitleBar>
        <Wordmark size={26} />
        <div data-tauri-drag-region className="flex-1" />
        <AnimatePresence>
          {update && (
            <motion.button
              initial={{ opacity: 0, scale: 0.9 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0 }}
              className="btn btn-opal btn-sm no-drag"
              disabled={update.state !== "ready"}
              onClick={() => installUpdate()}
            >
              {update.state === "ready" ? `Restart to update to ${update.version}` : `Downloading ${update.version}…`}
            </motion.button>
          )}
        </AnimatePresence>
        <button className="btn btn-ghost btn-icon group no-drag" aria-label="Settings" onClick={() => setSettingsOpen(true)}>
          <GearIcon size={19} />
        </button>
      </TitleBar>

      <main className="grid min-h-0 flex-1 grid-cols-[1.08fr_1fr] gap-4 px-5 pb-5">
        {/* Hero */}
        <section className="glass flex min-h-0 flex-col p-6">
          <div className="flex items-center justify-between">
            <span className="eyebrow">Noise cancellation</span>
            <StatusChip status={engine.status} />
          </div>
          <div className="flex flex-1 flex-col items-center justify-center gap-5 py-2">
            <PowerHero status={engine.status} onToggle={toggle} />
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
              <MeterTile label={engine.listen ? "In your headphones" : "Hush AI Microphone"} source="output" color="#1f9d72" idle={!on} accent />
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
                <p className="mt-1 text-[13px] leading-snug text-muted">Choose it as the microphone in your apps. Hover an app to see where.</p>
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
            <div className="mt-3 flex flex-wrap gap-1.5">
              {APPS.map((app) => (
                <Tip key={app.name} content={<AppHint {...app} />}>
                  <span className="chip cursor-default bg-white/80 transition-colors hover:bg-white" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
                    {app.name}
                  </span>
                </Tip>
              ))}
            </div>
          </Card>

          <Card>
            <div className="mb-3 flex items-center justify-between">
              <span className="text-[15px] font-semibold">Quality</span>
              <Tip content="How much of real time the model needs. Below 100% it keeps up.">
                <span className="chip cursor-default">
                  Load <span ref={load} className="tabular-nums">{on ? "…" : "0%"}</span>
                </span>
              </Tip>
            </div>
            <Segmented label="Quality" value={settings.quality} options={QUALITY.map((q) => ({ value: q.value, label: q.label, hint: q.hint }))} onChange={(quality) => saveSettings({ quality })} />
          </Card>

          <Card>
            <div className="flex items-center justify-between gap-3">
              <div className="flex items-center gap-3">
                <span className="grid h-10 w-10 place-items-center rounded-2xl bg-[#f4efe8] text-ink-2">
                  <HeadphonesIcon size={20} />
                </span>
                <div>
                  <div className="text-[15px] font-semibold">Listen to yourself</div>
                  <div className="text-[12.5px] text-muted">Hear Hush in your headphones</div>
                </div>
              </div>
              <Toggle label="Listen to yourself" on={engine.listen} onChange={(v) => api.setListen(v)} />
            </div>
            <AnimatePresence initial={false}>
              {engine.listen && (
                <motion.div initial={{ height: 0, opacity: 0 }} animate={{ height: "auto", opacity: 1 }} exit={{ height: 0, opacity: 0 }} className="overflow-hidden">
                  <div className="space-y-2 pt-3">
                    <Segmented
                      size="sm"
                      label="Compare"
                      value={engine.original ? "original" : "hush"}
                      options={[
                        { value: "hush", label: "With Hush" },
                        { value: "original", label: "Original" },
                      ]}
                      onChange={(v) => api.setOriginal(v === "original")}
                    />
                    <p className="flex items-center gap-1.5 text-[12px] text-amber">
                      <WarningIcon size={14} /> Use headphones, or your speakers will echo.
                    </p>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>
          </Card>

          <Card>
            <div className="mb-2 flex items-center justify-between">
              <span className="text-[15px] font-semibold">What's next</span>
              <span className="eyebrow">Roadmap</span>
            </div>
            <ul className="space-y-1">
              {ROADMAP.map(({ icon: Icon, title, state }) => (
                <li key={title} className="flex items-center gap-3 rounded-xl px-1 py-1.5">
                  <Icon size={18} className={state === "done" ? "text-green" : "text-faint"} />
                  <span className={`flex-1 text-[13.5px] ${state === "done" ? "font-[560]" : "text-ink-2"}`}>{title}</span>
                  <span className={`chip h-6 text-[11px] ${state === "done" ? "text-green" : "text-muted"}`} style={state === "done" ? { background: "rgba(31,157,114,.1)" } : undefined}>
                    {state === "done" ? "Available" : "Coming soon"}
                  </span>
                </li>
              ))}
            </ul>
          </Card>
        </section>
      </main>

      <SettingsPanel open={settingsOpen} onClose={() => setSettingsOpen(false)} />

      <AnimatePresence>
        {setupOpen && (
          <Modal onClose={() => setSetupOpen(false)} title="Set up Hush AI Microphone">
            <VmicSetup compact onReady={() => setTimeout(() => setSetupOpen(false), 900)} />
          </Modal>
        )}
      </AnimatePresence>
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
  return (
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
    </motion.div>
  );
}
