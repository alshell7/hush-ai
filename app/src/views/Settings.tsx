// Settings, as a panel that slides over the main screen.
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { DevicePicker, Segmented, Spinner, Tip, Toggle } from "../components/Controls";
import { CloseIcon, GitHubIcon, HeadphonesIcon, InfoIcon } from "../components/Icons";
import { Wordmark } from "../components/Mochi";
import { ModelDownload } from "../components/ModelDownload";
import { toast } from "../components/Toast";
import { ENGINE_ORDER, ENGINES } from "../lib/engines";
import { api, formatBytes, isTauri, openUrl, type Engine } from "../lib/ipc";
import { saveSettings, setStore, useApp } from "../lib/store";
import { checkForUpdate } from "../lib/updater";

const REPO = "https://github.com/alshell7/hush-ai";

export function SettingsPanel({ open, onClose }: { open: boolean; onClose: () => void }) {
  useEffect(() => {
    if (!open) return;
    const key = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [open, onClose]);

  return (
    <AnimatePresence>
      {open && (
        <motion.div className="fixed inset-0 z-30" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}>
          <div className="absolute inset-0 bg-[rgba(30,27,24,.16)]" onClick={onClose} />
          <motion.aside
            role="dialog"
            aria-label="Settings"
            initial={{ x: 40, opacity: 0 }}
            animate={{ x: 0, opacity: 1 }}
            exit={{ x: 40, opacity: 0 }}
            transition={{ type: "spring", stiffness: 340, damping: 34 }}
            className="scroll absolute bottom-3 right-3 top-3 w-[430px] rounded-[28px] bg-[#fdfcfa] p-6"
            style={{ boxShadow: "var(--h-shadow-3)" }}
          >
            <div className="mb-5 flex items-center justify-between">
              <h2 className="text-[21px] font-[650] tracking-[-0.025em]">Settings</h2>
              <button className="btn btn-ghost btn-icon h-9 w-9" aria-label="Close settings" onClick={onClose}>
                <CloseIcon size={18} />
              </button>
            </div>
            <SettingsBody />
          </motion.aside>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="mb-6">
      <h3 className="eyebrow mb-2 px-1">{title}</h3>
      <div className="divide-y divide-[var(--h-line)] rounded-3xl bg-white px-4" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
        {children}
      </div>
    </section>
  );
}

function Row({ title, detail, children }: { title: string; detail?: React.ReactNode; children?: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4 py-3.5">
      <div className="min-w-0">
        <div className="text-[14px] font-[560]">{title}</div>
        {detail && <div className="mt-0.5 text-[12.5px] leading-snug text-muted">{detail}</div>}
      </div>
      {children}
    </div>
  );
}

function SettingsBody() {
  const settings = useApp((s) => s.settings)!;
  const engine = useApp((s) => s.engine)!;
  const models = useApp((s) => s.models);
  const [adding, setAdding] = useState<Engine | null>(null);
  const devices = useApp((s) => s.devices);
  const vmic = useApp((s) => s.vmic);
  const platform = useApp((s) => s.platform);
  const version = useApp((s) => s.version);
  const [autostart, setAutostart] = useState<boolean | null>(null);
  const [checking, setChecking] = useState(false);

  useEffect(() => {
    if (!isTauri) return setAutostart(false);
    import("@tauri-apps/plugin-autostart").then(({ isEnabled }) => isEnabled().then(setAutostart));
  }, []);

  const setLaunch = async (on: boolean) => {
    setAutostart(on);
    if (!isTauri) return;
    const { enable, disable } = await import("@tauri-apps/plugin-autostart");
    await (on ? enable() : disable()).catch((e) => toast(String(e), "error"));
  };

  const check = async () => {
    setChecking(true);
    try {
      const result = await checkForUpdate(true);
      toast(result === "available" ? "Update downloaded. Restart to finish." : "Hush AI is up to date.");
    } catch (e) {
      toast(`Could not check for updates: ${e}`, "error");
    } finally {
      setChecking(false);
    }
  };

  return (
    <>
      <Section title="General">
        <Row title="Open at login" detail="Starts in the tray, ready for your next call.">
          <Toggle label="Open at login" on={!!autostart} disabled={autostart === null} onChange={setLaunch} />
        </Row>
        <Row title="Install updates automatically" detail={`Version ${version}`}>
          <Toggle label="Install updates" on={settings.checkUpdates} onChange={(checkUpdates) => saveSettings({ checkUpdates })} />
        </Row>
        <Row title="Check for updates">
          <button className="btn btn-soft btn-sm" disabled={checking} onClick={check}>
            {checking ? <Spinner size={14} /> : null} Check now
          </button>
        </Row>
      </Section>

      <Section title="Audio">
        <div className="py-3.5">
          <div className="mb-2 text-[14px] font-[560]">Headphones for Listen</div>
          <DevicePicker
            label="Headphones"
            icon={<HeadphonesIcon size={17} />}
            devices={devices?.outputs ?? [{ name: null, label: "System default" }]}
            value={settings.output}
            onChange={(output) => saveSettings({ output })}
          />
        </div>
        {settings.engine === "reuse" ? (
          <div className="py-3.5">
            <div className="mb-2 flex items-center gap-1.5 text-[14px] font-[560]">
              Processing threads
              <Tip content="Automatic picks the fewest CPU threads that keep up, and adds more only when needed.">
                <span className="text-faint">
                  <InfoIcon size={15} />
                </span>
              </Tip>
            </div>
            <Segmented
              size="sm"
              label="Threads"
              value={String(settings.threads)}
              options={["0", "1", "2", "4", "8"].map((v) => ({ value: v, label: v === "0" ? `Automatic${engine.status === "on" && engine.engine === "reuse" && settings.threads === 0 ? ` (${engine.threads})` : ""}` : v }))}
              onChange={(v) => saveSettings({ threads: Number(v) })}
            />
          </div>
        ) : (
          <Row title="Processing threads" detail={`${ENGINES.deepFilter.name} runs on one thread.`} />
        )}
        {platform === "windows" && (
          <Row title="Audio priority" detail="Runs the model as a Windows “Pro Audio” task, so other apps do not interrupt your voice.">
            <Toggle label="Audio priority" on={settings.audioPriority} onChange={(audioPriority) => saveSettings({ audioPriority })} />
          </Row>
        )}
      </Section>

      <Section title="Hush AI Microphone">
        <Row title={vmic?.state === "ready" ? "Ready" : "Not set up"} detail={vmic?.detail}>
          {vmic?.state === "ready" && vmic.driver !== "pulse" && (
            <button
              className="btn btn-soft btn-sm"
              onClick={async () => {
                try {
                  setStore({ vmic: await api.vmicUninstall() });
                  toast(vmic.driver === "vb-cable" ? "VB-CABLE names restored." : "Hush AI Microphone removed.");
                } catch (e) {
                  toast(String(e), "error");
                }
              }}
            >
              {vmic.driver === "vb-cable" ? "Restore VB-CABLE names" : "Remove"}
            </button>
          )}
        </Row>
      </Section>

      <Section title="Voice models">
        {ENGINE_ORDER.map((kind) => {
          const info = models?.[kind];
          const inUse = settings.engine === kind;
          return (
            <div key={kind} className="py-3.5">
              <div className="flex items-center justify-between gap-4">
                <div className="min-w-0">
                  <div className="flex items-center gap-2 text-[14px] font-[560]">
                    {ENGINES[kind].name}
                    {inUse && (
                      <span className="chip h-5 text-[11px] text-green" style={{ background: "rgba(31,157,114,.1)" }}>
                        In use
                      </span>
                    )}
                  </div>
                  <div className="mt-0.5 text-[12.5px] leading-snug text-muted">
                    {info?.installed ? `Installed · ${formatBytes(info.size)}` : "Not downloaded"} ·{" "}
                    <a className="link" onClick={(e) => (e.preventDefault(), openUrl(ENGINES[kind].licenseUrl))}>
                      {ENGINES[kind].license}
                    </a>
                  </div>
                </div>
                {info?.installed ? (
                  <button
                    className="btn btn-soft btn-sm"
                    onClick={async () => {
                      setStore({ models: await api.removeModel(kind) });
                      toast(inUse ? "Voice model removed. Download it again to use Hush." : "Voice model removed.");
                    }}
                  >
                    Remove
                  </button>
                ) : (
                  adding !== kind && (
                    <button className="btn btn-soft btn-sm" onClick={() => setAdding(kind)}>
                      Download
                    </button>
                  )
                )}
              </div>
              {!info?.installed && adding === kind && <ModelDownload engine={kind} />}
            </div>
          );
        })}
      </Section>

      <Section title="About">
        <div className="flex items-center justify-between py-4">
          <Wordmark size={24} />
          <span className="text-[12.5px] text-muted">v{version}</span>
        </div>
        <Row title="Open source" detail="Apache-2.0 app and engine. Star it, report bugs, send ideas.">
          <button className="btn btn-soft btn-sm" onClick={() => openUrl(REPO)}>
            <GitHubIcon size={15} /> GitHub
          </button>
        </Row>
        <Row title="Quit Hush AI" detail="Hush AI Microphone goes silent until you open Hush again.">
          <button className="btn btn-soft btn-sm" onClick={() => api.quit()}>
            Quit
          </button>
        </Row>
      </Section>
    </>
  );
}
