// First run: meet Hush, get the voice model, check the microphone, set up the
// Hush AI Microphone, hear the difference, then use it everywhere.
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import { DevicePicker, Segmented, Spinner } from "../components/Controls";
import { ArrowLeftIcon, ArrowRightIcon, CheckIcon, DownloadIcon, ExternalIcon, LockIcon, MicIcon, PlayIcon, RecordIcon, ShieldIcon, SparkleIcon, StopIcon } from "../components/Icons";
import { LicenseNote } from "../components/ModelDownload";
import { MicLevel } from "../components/Meters";
import { AppGuidePicker } from "../components/AppGuides";
import { Mochi, Wordmark } from "../components/Mochi";
import { TitleBar } from "../components/TitleBar";
import { VmicSetup } from "../components/VmicSetup";
import { ENGINE_ORDER, ENGINES, useModelDownload } from "../lib/engines";
import { api, formatBytes, on, openUrl, type TestEvent } from "../lib/ipc";
import { saveSettings, useApp } from "../lib/store";

const STEPS = ["welcome", "model", "microphone", "vmic", "test", "done"] as const;
type Step = (typeof STEPS)[number];

export function Onboarding() {
  const [step, setStep] = useState<Step>("welcome");
  const [direction, setDirection] = useState(1);
  const index = STEPS.indexOf(step);

  const go = (next: Step) => {
    setDirection(STEPS.indexOf(next) > index ? 1 : -1);
    setStep(next);
  };
  const forward = () => go(STEPS[Math.min(index + 1, STEPS.length - 1)]);
  const back = () => go(STEPS[Math.max(index - 1, 0)]);

  return (
    <div className="relative flex h-full flex-col">
      <TitleBar>
        <Wordmark size={24} />
      </TitleBar>
      {/* Centered when it fits; scrolls from the top when the window is small, so nothing
          slides under the title bar, whose drag region would take the clicks. */}
      <div className="no-scrollbar relative flex min-h-0 flex-1 justify-center overflow-y-auto px-8 py-2">
        <AnimatePresence mode="wait" custom={direction}>
          <motion.div
            key={step}
            custom={direction}
            initial={{ opacity: 0, x: 28 * direction, filter: "blur(6px)" }}
            animate={{ opacity: 1, x: 0, filter: "blur(0px)" }}
            exit={{ opacity: 0, x: -28 * direction, filter: "blur(6px)" }}
            transition={{ duration: 0.38, ease: [0.2, 0.9, 0.1, 1] }}
            className="my-auto w-full max-w-[760px]"
          >
            {step === "welcome" && <Welcome onNext={forward} />}
            {step === "model" && <ModelStep onNext={forward} />}
            {step === "microphone" && <MicrophoneStep onNext={forward} />}
            {step === "vmic" && <VmicStep onNext={forward} />}
            {step === "test" && <TestStep onNext={forward} />}
            {step === "done" && <DoneStep />}
          </motion.div>
        </AnimatePresence>
      </div>
      <footer className="flex h-16 shrink-0 items-center justify-between px-8 pb-4">
        <div className="w-28">
          {index > 0 && step !== "done" && (
            <button className="btn btn-ghost btn-sm" onClick={back}>
              <ArrowLeftIcon size={16} /> Back
            </button>
          )}
        </div>
        <div className="flex items-center gap-2" aria-label={`Step ${index + 1} of ${STEPS.length}`}>
          {STEPS.map((s, i) => (
            <motion.span
              key={s}
              className="h-2 rounded-full"
              animate={{ width: i === index ? 26 : 8, background: i <= index ? "var(--h-ink)" : "rgba(30,27,24,.15)" }}
              transition={{ type: "spring", stiffness: 400, damping: 30 }}
            />
          ))}
        </div>
        <div className="w-28" />
      </footer>
    </div>
  );
}

/** True in short windows (the app's minimum is 560 px tall), where steps get compact. */
function useShort() {
  const query = "(max-height: 700px)";
  const [short, setShort] = useState(() => matchMedia(query).matches);
  useEffect(() => {
    const list = matchMedia(query);
    const change = () => setShort(list.matches);
    list.addEventListener("change", change);
    return () => list.removeEventListener("change", change);
  }, []);
  return short;
}

function StepCard({ eyebrow, title, text, children, art }: { eyebrow: string; title: string; text: React.ReactNode; children: React.ReactNode; art?: React.ReactNode }) {
  return (
    <div className="glass grid grid-cols-[1fr_auto] items-center gap-8 p-6 [@media(min-height:700px)]:p-9">
      <div className="min-w-0 space-y-4 [@media(min-height:700px)]:space-y-5">
        <div className="space-y-2">
          <div className="eyebrow">{eyebrow}</div>
          <h1 className="text-[26px] font-[660] leading-[1.08] tracking-[-0.035em] [@media(min-height:700px)]:text-[30px]">{title}</h1>
          <p className="max-w-[440px] text-[14.5px] leading-relaxed text-ink-2">{text}</p>
        </div>
        {children}
      </div>
      {art && <div className="hidden md:block">{art}</div>}
    </div>
  );
}

// ------------------------------------------------------------------ 1. Welcome

function Welcome({ onNext }: { onNext: () => void }) {
  const features = [
    { icon: ShieldIcon, title: "Private by design", text: "Noise cancellation runs on this computer." },
    { icon: SparkleIcon, title: "Works in every app", text: "Zoom, Meet, Teams, Discord, OBS and more." },
    { icon: LockIcon, title: "Free and open source", text: "No account. No subscription. No limits." },
  ];
  const short = useShort();
  return (
    <div className="flex flex-col items-center text-center">
      <motion.div initial={{ scale: 0.6, opacity: 0, y: 20 }} animate={{ scale: 1, opacity: 1, y: 0 }} transition={{ type: "spring", stiffness: 180, damping: 14, delay: 0.05 }} className="relative">
        <div className="absolute inset-0 -z-10 scale-150 rounded-full blur-3xl" style={{ background: "var(--h-opal)", opacity: 0.8 }} />
        <Mochi size={short ? 78 : 132} mood="awake" />
      </motion.div>
      <motion.h1 initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: 0.18, duration: 0.5 }} className={`${short ? "mt-3 text-[34px]" : "mt-6 text-[44px]"} font-[680] leading-[1.02] tracking-[-0.045em]`}>
        Silence the noise.
        <br />
        <span className="opal-text">Keep your voice.</span>
      </motion.h1>
      <motion.p initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: 0.28, duration: 0.5 }} className={`${short ? "mt-2 text-[14px]" : "mt-4 text-[15.5px]"} max-w-[480px] leading-relaxed text-ink-2`}>
        Hush AI removes background noise from your microphone in real time: keyboards, fans, dogs, traffic, cafés. Every app hears only you.
      </motion.p>
      <div className={`${short ? "mt-4" : "mt-8"} grid w-full max-w-[680px] grid-cols-3 gap-3`}>
        {features.map((f, i) => (
          <motion.div key={f.title} initial={{ opacity: 0, y: 14 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: 0.36 + i * 0.07, duration: 0.45 }} className={`tile ${short ? "p-3" : "p-4"} text-left`}>
            <f.icon size={20} className="mb-2 text-coral" />
            <div className="text-[13.5px] font-semibold">{f.title}</div>
            <div className="text-[12.5px] leading-snug text-muted">{f.text}</div>
          </motion.div>
        ))}
      </div>
      <motion.button initial={{ opacity: 0 }} animate={{ opacity: 1 }} transition={{ delay: 0.6 }} className={`btn btn-primary group ${short ? "mt-5" : "mt-8"} h-12 px-7 text-[15px]`} onClick={onNext}>
        Get started <ArrowRightIcon size={18} />
      </motion.button>
    </div>
  );
}

// ------------------------------------------------------------------ 2. Model

function ModelStep({ onNext }: { onNext: () => void }) {
  const settings = useApp((s) => s.settings)!;
  const engine = settings.engine;
  const [agreed, setAgreed] = useState(settings.licenseAccepted);
  const model = useModelDownload(engine);
  const [done, setDone] = useState(model.installed);
  const downloading = model.state === "downloading";

  useEffect(() => setDone(model.installed), [engine, model.installed]);

  const download = async () => {
    if (await model.download()) {
      setDone(true);
      setTimeout(onNext, 900);
    }
  };

  // The license terms must be accepted, also when the model is already on this computer.
  const blocked = !agreed;
  const accepted = settings.licenseAccepted;
  const next = async () => {
    if (!settings.licenseAccepted) await saveSettings({ licenseAccepted: true });
    onNext();
  };

  const fraction = model.progress ? model.progress.received / model.progress.total : 0;
  const state = done ? "done" : downloading ? "downloading" : model.state;
  return (
    <StepCard
      eyebrow="Step 1 · Voice model"
      title="Choose your voice model"
      text="Both run on your CPU, so your voice never leaves this computer. You can switch later."
      art={<ProgressRing fraction={done ? 1 : fraction} state={state} size={model.size} />}
    >
      <div role="radiogroup" aria-label="Voice model" className="grid grid-cols-2 gap-2.5">
        {ENGINE_ORDER.map((kind) => {
          const active = kind === engine;
          return (
            <button
              key={kind}
              role="radio"
              aria-checked={active}
              disabled={downloading}
              onClick={() => saveSettings({ engine: kind })}
              className="tile cursor-pointer p-3.5 text-left transition-shadow disabled:cursor-default"
              style={{ boxShadow: active ? "inset 0 0 0 2px var(--h-ink)" : "inset 0 0 0 1px var(--h-line)" }}
            >
              <div className="flex items-center justify-between gap-2">
                <span className="text-[13.5px] font-semibold">{ENGINES[kind].label}</span>
                {kind === "deepFilter" && <span className="chip h-5 text-[10.5px] text-green" style={{ background: "rgba(31,157,114,.1)" }}>Recommended</span>}
              </div>
              <div className="mt-1 text-[12px] leading-snug text-muted">{ENGINES[kind].hint}</div>
            </button>
          );
        })}
      </div>
      <div className="rounded-2xl bg-white/70 p-3.5" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
        <LicenseNote agreed={agreed} onAgree={setAgreed} disabled={downloading || (done && accepted)} />
      </div>
      <div className="flex items-center gap-3">
        {done ? (
          <button className="btn btn-primary group" disabled={blocked} onClick={next}>
            Continue <ArrowRightIcon size={18} />
          </button>
        ) : downloading ? (
          <button className="btn btn-soft" onClick={model.cancel}>
            Cancel
          </button>
        ) : (
          <button className="btn btn-primary group" disabled={blocked} onClick={download}>
            <DownloadIcon size={18} /> {model.state === "error" ? "Try again" : `Download ${formatBytes(model.size)}`}
          </button>
        )}
        <span className="text-[12.5px] tabular-nums text-muted">
          {downloading && model.progress && `${formatBytes(model.progress.received)} of ${formatBytes(model.progress.total)} · ${formatBytes(model.progress.bytesPerSecond)}/s`}
          {done && "Downloaded and verified"}
        </span>
      </div>
      {model.state === "error" && <div className="rounded-2xl bg-[#fdecea] px-3 py-2 text-[12.5px] text-red">{model.error}</div>}
    </StepCard>
  );
}

function ProgressRing({ fraction, state, size }: { fraction: number; state: string; size: number }) {
  const r = 70;
  const c = 2 * Math.PI * r;
  return (
    <div className="relative grid h-[180px] w-[180px] place-items-center">
      <svg width="180" height="180" className="absolute -rotate-90">
        <defs>
          <linearGradient id="ring" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stopColor="#ffae94" />
            <stop offset="0.5" stopColor="#ffd27a" />
            <stop offset="1" stopColor="#6fd9d1" />
          </linearGradient>
        </defs>
        <circle cx="90" cy="90" r={r} stroke="rgba(30,27,24,.07)" strokeWidth="10" fill="none" />
        <motion.circle cx="90" cy="90" r={r} stroke="url(#ring)" strokeWidth="10" strokeLinecap="round" fill="none" strokeDasharray={c} animate={{ strokeDashoffset: c * (1 - fraction) }} transition={{ ease: "linear", duration: 0.15 }} />
      </svg>
      <AnimatePresence mode="wait">
        {state === "done" ? (
          <motion.div key="done" initial={{ scale: 0.5, opacity: 0 }} animate={{ scale: 1, opacity: 1 }} className="grid h-16 w-16 place-items-center rounded-full text-green" style={{ background: "rgba(31,157,114,.12)" }}>
            <CheckIcon size={32} draw />
          </motion.div>
        ) : (
          <motion.div key="pct" className="text-center">
            <div className="text-[30px] font-[650] tabular-nums tracking-[-0.03em]">{Math.round(fraction * 100)}%</div>
            <div className="text-[11.5px] text-muted">{state === "downloading" ? "downloading" : formatBytes(size)}</div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

// ------------------------------------------------------------------ 3. Microphone

function MicrophoneStep({ onNext }: { onNext: () => void }) {
  const settings = useApp((s) => s.settings)!;
  const devices = useApp((s) => s.devices);
  const [level, setLevel] = useState(0);
  const [failed, setFailed] = useState("");
  const heard = useRef(0);
  const [speaking, setSpeaking] = useState(false);

  useEffect(() => {
    const a = on("mic-level", (v) => {
      setLevel(v);
      if (v > 0.08) heard.current = Date.now();
      setSpeaking(Date.now() - heard.current < 400);
    });
    const b = on("mic-level-error", setFailed);
    api.micMonitor(true).catch((e) => setFailed(String(e)));
    return () => {
      a.then((fn) => fn());
      b.then((fn) => fn());
      api.micMonitor(false);
    };
  }, [settings.input]);

  return (
    <StepCard
      eyebrow="Step 2 · Microphone"
      title="Which microphone do you use?"
      text="Say a few words. The bar moves when Hush hears you."
      art={
        <div className="grid h-[180px] w-[180px] place-items-center">
          <motion.div animate={{ scale: 1 + Math.min(0.12, level * 0.3) }} transition={{ type: "spring", stiffness: 300, damping: 18 }}>
            <Mochi size={130} mood={speaking ? "happy" : "awake"} />
          </motion.div>
        </div>
      }
    >
      <DevicePicker
        label="Microphone"
        icon={<MicIcon size={17} />}
        devices={devices?.inputs ?? [{ name: null, label: "System default" }]}
        value={settings.input}
        onChange={(input) => {
          setFailed("");
          saveSettings({ input });
        }}
      />
      <div className="rounded-2xl bg-white/70 p-3" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
        <MicLevel level={level} />
      </div>
      {failed && <div className="rounded-2xl bg-[#fdecea] px-3 py-2 text-[12.5px] text-red">This microphone cannot be opened. Choose another one. ({failed.split("\n")[0]})</div>}
      <button className="btn btn-primary group" onClick={onNext}>
        Sounds good <ArrowRightIcon size={18} />
      </button>
    </StepCard>
  );
}

// ------------------------------------------------------------------ 4. Hush AI Microphone

function VmicStep({ onNext }: { onNext: () => void }) {
  const vmic = useApp((s) => s.vmic);
  return (
    <StepCard
      eyebrow="Step 3 · Hush AI Microphone"
      title="One microphone for every app"
      text="Hush cleans your voice and sends it to a new microphone, Hush AI Microphone. Apps choose it like any other microphone."
    >
      <VmicSetup />
      <div className="flex items-center gap-3">
        <button className="btn btn-primary group" disabled={vmic?.state !== "ready"} onClick={onNext}>
          Continue <ArrowRightIcon size={18} />
        </button>
        {vmic?.state !== "ready" && (
          <button className="btn btn-ghost btn-sm" onClick={onNext}>
            Set up later
          </button>
        )}
      </div>
    </StepCard>
  );
}

// ------------------------------------------------------------------ 5. Hear the difference

const SECONDS = 6;

function TestStep({ onNext }: { onNext: () => void }) {
  const [event, setEvent] = useState<TestEvent | null>(null);
  const [result, setResult] = useState<Extract<TestEvent, { phase: "ready" }> | null>(null);
  const [which, setWhich] = useState<"original" | "hush">("hush");
  const [count, setCount] = useState(0);
  const [error, setError] = useState("");

  useEffect(() => {
    const stop = on("test", (e) => {
      setEvent(e);
      if (e.phase === "ready") setResult(e);
      if (e.phase === "error") setError(e.message);
    });
    return () => {
      stop.then((fn) => fn());
      api.testStop();
    };
  }, []);

  useEffect(() => {
    if (event?.phase !== "recording") return;
    setCount(SECONDS);
    const timer = setInterval(() => setCount((c) => Math.max(0, c - 1)), 1000);
    return () => clearInterval(timer);
  }, [event]);

  const record = async () => {
    setError("");
    setResult(null);
    try {
      await api.testRecord(SECONDS);
    } catch (e) {
      setError(String(e));
    }
  };
  const playing = event?.phase === "playing";
  const recording = event?.phase === "recording";
  const processing = event?.phase === "processing";

  return (
    <StepCard eyebrow="Step 4 · Try it" title="Hear the difference" text="Record a few seconds while something noisy happens: type, tap the desk, play music. Then compare.">
      <div className="rounded-3xl bg-white/75 p-4" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
        <div className="mb-3 flex items-center justify-between">
          <Segmented
            size="sm"
            label="Version"
            value={which}
            options={[
              { value: "original", label: "Original" },
              { value: "hush", label: "With Hush" },
            ]}
            onChange={(v) => {
              setWhich(v);
              if (playing) api.testPlay(v === "hush");
            }}
          />
          <span className="text-[12px] tabular-nums text-muted">{recording ? `Recording… ${count}s` : processing ? "Cleaning…" : result ? `${result.seconds.toFixed(1)} s` : "Not recorded yet"}</span>
        </div>
        <Waveform values={result ? (which === "hush" ? result.enhanced : result.original) : null} busy={recording || processing} tone={which} />
        <div className="mt-3 flex items-center gap-2">
          <button className="btn btn-primary btn-sm" disabled={recording || processing} onClick={record}>
            {processing ? <Spinner size={14} /> : <RecordIcon size={14} className="text-[#ff8a7a]" />}
            {result ? "Record again" : "Record"}
          </button>
          <button className="btn btn-soft btn-sm" disabled={!result || recording || processing} onClick={() => (playing ? api.testStop() : api.testPlay(which === "hush"))}>
            {playing ? <StopIcon size={14} /> : <PlayIcon size={14} />} {playing ? "Stop" : "Play"}
          </button>
        </div>
      </div>
      {error && <div className="rounded-2xl bg-[#fdecea] px-3 py-2 text-[12.5px] text-red">{error.split("\n")[0]}</div>}
      <div className="flex items-center gap-3">
        {result ? (
          <button className="btn btn-primary group" onClick={onNext}>
            Love it <ArrowRightIcon size={18} />
          </button>
        ) : (
          <button className="btn btn-ghost btn-sm" onClick={onNext}>
            Skip for now <ArrowRightIcon size={16} />
          </button>
        )}
      </div>
    </StepCard>
  );
}

function Waveform({ values, busy, tone }: { values: number[] | null; busy: boolean; tone: "original" | "hush" }) {
  const bars = values ?? new Array(96).fill(0);
  return (
    <div className="flex h-20 items-center gap-[2px]">
      {bars.map((v, i) => (
        <motion.span
          key={i}
          className="flex-1 rounded-full"
          initial={false}
          animate={{
            height: busy ? `${18 + 14 * Math.abs(Math.sin(i / 3))}%` : `${Math.max(4, Math.min(100, v * 120))}%`,
            background: values ? (tone === "hush" ? "#1f9d72" : "#b9b2a7") : "rgba(30,27,24,.1)",
          }}
          transition={{ type: "spring", stiffness: 260, damping: 24, delay: i * 0.003 }}
        />
      ))}
    </div>
  );
}

// ------------------------------------------------------------------ 6. Done

function DoneStep() {
  const finish = async () => {
    await saveSettings({ onboarded: true });
    api.setEnabled(true);
  };
  return (
    <div className="glass grid grid-cols-[1fr_auto] items-center gap-8 p-7 [@media(min-height:620px)]:p-9">
      <div className="space-y-5">
        <div className="space-y-2">
          <div className="eyebrow">All set</div>
          <h1 className="text-[30px] font-[660] leading-[1.08] tracking-[-0.035em]">
            Choose <span className="opal-text">Hush AI Microphone</span> in your apps
          </h1>
          <p className="max-w-[420px] text-[14.5px] leading-relaxed text-ink-2">
            In Zoom, Meet, Teams, Discord or OBS, open the audio settings and pick Hush AI Microphone. Hush keeps running in the {navigator.userAgent.includes("Mac") ? "menu bar" : "tray"}, so it is always ready.
          </p>
        </div>
        <button className="btn btn-primary group h-12 px-7 text-[15px]" onClick={finish}>
          Turn on Hush AI <ArrowRightIcon size={18} />
        </button>
        <button className="btn btn-ghost btn-sm block" onClick={() => openUrl("https://alshell7.github.io/hush-ai/#guides")}>
          App-by-app guides <ExternalIcon size={14} />
        </button>
      </div>
      <AppGuidePicker />
    </div>
  );
}
