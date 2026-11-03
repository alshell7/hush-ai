// Set up the Hush AI Microphone: one step on macOS, a guided VB-CABLE install on Windows,
// nothing on Linux.
import { motion, AnimatePresence } from "motion/react";
import { useEffect, useState } from "react";
import { api, openUrl } from "../lib/ipc";
import { refreshVmic, setStore, useApp } from "../lib/store";
import { Spinner } from "./Controls";
import { CheckIcon, DownloadIcon, ExternalIcon, ShieldIcon } from "./Icons";
import { Mark } from "./Mochi";

const VB_STEPS = [
  { title: "Download the Driver Pack", text: "On VB-Audio's page, download “VBCABLE_Driver_Pack” for Windows." },
  { title: "Extract the ZIP", text: "Right-click the file › Extract All. Keep the folder open." },
  { title: "Run the setup as administrator", text: "Right-click “VBCABLE_Setup_x64” › Run as administrator." },
  { title: "Install Driver, then restart", text: "Press Install Driver. Restart Windows when it finishes." },
];

export function VmicSetup({ onReady, compact = false }: { onReady?: () => void; compact?: boolean }) {
  const vmic = useApp((s) => s.vmic);
  const platform = useApp((s) => s.platform);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [active, setActive] = useState(0);

  useEffect(() => {
    refreshVmic();
  }, []);

  // Walk through the VB-CABLE steps, slowly, so the eye follows.
  useEffect(() => {
    if (vmic?.state !== "download") return;
    const timer = setInterval(() => setActive((a) => (a + 1) % VB_STEPS.length), 2600);
    return () => clearInterval(timer);
  }, [vmic?.state]);

  // While the user installs VB-CABLE, look for it every few seconds.
  useEffect(() => {
    if (vmic?.state !== "download") return;
    const timer = setInterval(refreshVmic, 4000);
    return () => clearInterval(timer);
  }, [vmic?.state]);

  const install = async () => {
    setBusy(true);
    setError(null);
    try {
      const next = await api.vmicInstall();
      setStore({ vmic: next });
      if (next.state === "ready") onReady?.();
    } catch (e) {
      const text = String(e);
      setError(text.includes("Canceled") ? "The permission prompt was closed. Try again when ready." : text);
    } finally {
      setBusy(false);
    }
  };

  if (!vmic) {
    return (
      <div className="grid h-40 place-items-center">
        <Spinner />
      </div>
    );
  }

  return (
    <AnimatePresence mode="wait">
      <motion.div
        key={vmic.state}
        initial={{ opacity: 0, y: 8 }}
        animate={{ opacity: 1, y: 0 }}
        exit={{ opacity: 0, y: -8 }}
        transition={{ duration: 0.28, ease: [0.2, 0.9, 0.1, 1] }}
        className="space-y-4"
      >
        {vmic.state === "ready" && (
          <div className="flex items-center gap-4 rounded-3xl bg-white/70 p-4" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
            <span className="grid h-12 w-12 place-items-center rounded-2xl text-green" style={{ background: "rgba(31,157,114,.12)" }}>
              <CheckIcon size={24} draw />
            </span>
            <div>
              <div className="flex items-center gap-1.5 text-[15px] font-semibold">
                <Mark size={16} /> Hush AI Microphone is ready
              </div>
              <div className="text-[13px] text-muted">
                {vmic.driver === "vb-cable"
                  ? "Using VB-CABLE, renamed for Hush. Choose it in any app."
                  : vmic.driver === "pulse"
                    ? "Created by Hush on PipeWire/PulseAudio while it runs."
                    : "Choose it as the microphone in any app."}
              </div>
            </div>
          </div>
        )}

        {vmic.state === "install" && platform === "macos" && (
          <Explainer
            title={vmic.detail === "Update available" ? "Update Hush AI Microphone" : "Install Hush AI Microphone"}
            text="Hush adds a small audio driver so every app can choose Hush AI Microphone. macOS asks for your password once. Audio stays on your Mac."
          >
            <button className="btn btn-primary group" disabled={busy} onClick={install}>
              {busy ? <Spinner size={16} /> : <DownloadIcon size={18} />}
              {busy ? "Installing…" : vmic.detail === "Update available" ? "Update" : "Install"}
            </button>
          </Explainer>
        )}

        {vmic.state === "install" && platform !== "macos" && (
          <Explainer
            title="Rename VB-CABLE to Hush AI Microphone"
            text="VB-CABLE is installed. Hush renames it so apps list it as Hush AI Microphone. Windows asks for permission once."
          >
            <button className="btn btn-primary" disabled={busy} onClick={install}>
              {busy ? <Spinner size={16} /> : <ShieldIcon size={18} />}
              {busy ? "Waiting for Windows…" : "Rename"}
            </button>
          </Explainer>
        )}

        {vmic.state === "download" && (
          <div className="space-y-4">
            <p className="text-[13.5px] leading-relaxed text-ink-2">
              On Windows, Hush AI Microphone uses <b>VB-CABLE</b>, a free virtual cable by VB-Audio. Its license does not let Hush include it, so it takes about a minute to install yourself.
            </p>
            <ol className={`grid gap-2 ${compact ? "" : "sm:grid-cols-2"}`}>
              {VB_STEPS.map((step, i) => (
                <motion.li
                  key={step.title}
                  onMouseEnter={() => setActive(i)}
                  animate={{
                    backgroundColor: active === i ? "rgba(255,255,255,0.95)" : "rgba(255,255,255,0.5)",
                    boxShadow: active === i ? "0 0 0 1.5px rgba(232,115,90,.45), 0 12px 26px -16px rgba(30,27,24,.35)" : "0 0 0 1px rgba(30,27,24,.08)",
                  }}
                  transition={{ duration: 0.35 }}
                  className="flex gap-3 rounded-2xl p-3"
                >
                  <motion.span
                    animate={{ scale: active === i ? 1.08 : 1, background: active === i ? "var(--h-opal-strong)" : "rgba(30,27,24,0.07)" }}
                    className="grid h-7 w-7 shrink-0 place-items-center rounded-full text-[12px] font-bold"
                  >
                    {i + 1}
                  </motion.span>
                  <div>
                    <div className="text-[13px] font-semibold">{step.title}</div>
                    <div className="text-[12px] leading-snug text-muted">{step.text}</div>
                  </div>
                </motion.li>
              ))}
            </ol>
            <div className="flex flex-wrap items-center gap-2">
              <button className="btn btn-primary group" onClick={() => openUrl("https://vb-audio.com/Cable/")}>
                Open VB-Audio download page <ExternalIcon size={16} />
              </button>
              <button className="btn btn-soft" onClick={() => refreshVmic()}>
                I installed it, check again
              </button>
            </div>
            <p className="text-[12px] text-muted">Hush checks every few seconds. After the restart, open Hush AI and it continues here.</p>
          </div>
        )}

        {vmic.state === "unsupported" && (
          <Explainer title="Hush AI Microphone is not available" text={vmic.detail}>
            <span />
          </Explainer>
        )}

        {error && <div className="rounded-2xl bg-[#fdecea] px-3 py-2 text-[12.5px] text-red">{error}</div>}
      </motion.div>
    </AnimatePresence>
  );
}

function Explainer({ title, text, children }: { title: string; text: string; children: React.ReactNode }) {
  return (
    <div className="space-y-3">
      <div className="text-[15px] font-semibold">{title}</div>
      <p className="text-[13.5px] leading-relaxed text-ink-2">{text}</p>
      {children}
    </div>
  );
}
