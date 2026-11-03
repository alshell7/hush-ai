// The tray / menu bar widget: the switch, the microphone and a glance at the levels.
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import { Segmented, Toggle } from "../components/Controls";
import { ExternalIcon, MicIcon } from "../components/Icons";
import { MeetingSoon, NextUp, useSoon } from "../components/MeetingSoon";
import { LevelStrip } from "../components/Meters";
import { Mark, Mochi } from "../components/Mochi";
import { api, invoke, isTauri, on } from "../lib/ipc";
import { refreshDevices, saveSettings, useApp } from "../lib/store";

export function Widget() {
  const settings = useApp((s) => s.settings);
  const engine = useApp((s) => s.engine);
  const devices = useApp((s) => s.devices);
  const [hint, setHint] = useState(false);
  const soon = useSoon();
  const card = useRef<HTMLDivElement>(null);

  // The window follows the card's height, so a meeting pop-up or the next meeting never
  // squeezes the levels, and there is no empty space when they are gone.
  useEffect(() => {
    const el = card.current;
    if (!el || !isTauri) return;
    let last = 0;
    const observer = new ResizeObserver(() => {
      const height = Math.ceil(el.getBoundingClientRect().height) + 24;
      if (Math.abs(height - last) > 1) {
        last = height;
        invoke("widget_fit", { height });
      }
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, [settings === null]);

  useEffect(() => {
    document.documentElement.classList.add("widget");
    const refresh = () => document.visibilityState === "visible" && refreshDevices();
    window.addEventListener("focus", refresh);
    const stop = on("tray-hint", () => setHint(true));
    return () => {
      window.removeEventListener("focus", refresh);
      stop.then((fn) => fn());
    };
  }, []);

  if (!settings || !engine) return null;
  const isOn = engine.status === "on" || engine.status === "starting";
  const mic = devices?.inputs.find((d) => d.name === settings.input)?.label ?? settings.input ?? "System default";

  return (
    <div className="p-3">
      <div
        ref={card}
        className="relative flex flex-col overflow-hidden rounded-[22px] p-4"
        style={{
          background: "rgba(253,252,250,.97)",
          boxShadow: "inset 0 0 0 1px rgba(255,255,255,.9), 0 0 0 1px rgba(30,27,24,.1), 0 10px 24px -10px rgba(30,27,24,.35)",
        }}
      >
        <div className="pointer-events-none absolute -right-16 -top-20 h-48 w-48 rounded-full blur-3xl transition-opacity duration-700" style={{ background: "var(--h-opal)", opacity: isOn ? 0.9 : 0.3 }} />
        <header className="relative flex items-center justify-between">
          <div className="flex items-center gap-2 text-[14px] font-[650] tracking-[-0.02em]">
            <Mark size={20} /> Hush AI
          </div>
          <button className="btn btn-ghost btn-icon h-8 w-8" aria-label="Open Hush AI" onClick={() => api.openMain()}>
            <ExternalIcon size={16} />
          </button>
        </header>

        <div className="relative mt-3 flex items-center gap-3 rounded-[18px] bg-white/80 p-3.5" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
          <Mochi size={50} mood={engine.status === "error" ? "oops" : isOn ? "happy" : "sleep"} />
          <div className="min-w-0 flex-1">
            <div className="whitespace-nowrap text-[14.5px] font-[640] tracking-[-0.02em]">Noise cancellation</div>
            <div className="truncate text-[12px] text-muted" title={engine.message ?? undefined}>
              {engine.status === "error" ? engine.message : engine.status === "starting" ? "Starting…" : isOn ? "On · Hush AI Microphone" : "Off"}
            </div>
          </div>
          <Toggle label="Noise cancellation" size="lg" on={isOn} onChange={(v) => api.setEnabled(v)} />
        </div>

        <MeetingSoon />

        <AnimatePresence>
          {hint && (
            <motion.div initial={{ opacity: 0, y: -4 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="relative mt-3 rounded-2xl px-3 py-2 text-[12.5px] leading-snug text-ink" style={{ background: "var(--h-opal)" }}>
              Hush keeps running here. Click the icon any time.
              <button className="ml-2 font-semibold underline" onClick={() => setHint(false)}>
                Got it
              </button>
            </motion.div>
          )}
        </AnimatePresence>

        {/* A meeting pop-up takes the levels' place while it shows. */}
        {!soon && (
        <div className="relative mt-3 shrink-0 space-y-2 rounded-[18px] bg-white/70 p-3" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
          <div className="flex items-center gap-2 text-[12px] font-semibold text-muted">
            <MicIcon size={15} />
            <span className="truncate">{mic}</span>
          </div>
          <LevelStrip source="input" color="rgba(30,27,24,.45)" height={22} idle={!isOn} />
          <div className="flex items-center gap-2 pt-1 text-[12px] font-semibold text-muted">
            <Mark size={14} /> Hush AI Microphone
          </div>
          <LevelStrip source="output" color="#1f9d72" height={22} idle={!isOn} />
        </div>
        )}

        <NextUp />

        <div className="relative mt-3">
          <Segmented
            size="sm"
            label="Quality"
            value={settings.quality}
            options={[
              { value: "light", label: "Light" },
              { value: "balanced", label: "Balanced" },
              { value: "studio", label: "Studio" },
            ]}
            onChange={(quality) => saveSettings({ quality })}
          />
        </div>

        <footer className="relative flex items-center justify-between pt-3">
          <button className="btn btn-ghost btn-sm" onClick={() => api.openMain()}>
            Open Hush AI
          </button>
          <button className="btn btn-ghost btn-sm text-muted" onClick={() => api.quit()}>
            Quit
          </button>
        </footer>
      </div>
    </div>
  );
}
