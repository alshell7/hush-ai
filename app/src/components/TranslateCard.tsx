// Talk controls, shared by the Live tab, the Talk tab, the tray widget and the notch: the
// conversation switch (which asks first), which directions it turns on, "I speak" and "They
// speak", live captions for both sides, and what Hush does with their voice.
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { features, languageName, setFeatures, talking, TRANSLATE_SHORTCUT, useFeatures, type CleanLevel, type HearMode, type LaneInfo, type Side, type TalkMode } from "../lib/features";
import { openUrl } from "../lib/ipc";
import { saveSettings, useApp } from "../lib/store";
import { DevicePicker, Segmented, Spinner, Toggle } from "./Controls";
import { GeminiKeyConnect } from "./GeminiKeyConnect";
import { ArrowRightIcon, CloseIcon, SpeakerIcon, SwapIcon, TranslateIcon } from "./Icons";
import { LanguagePicker } from "./LanguagePicker";
import { toast } from "./Toast";

export const tail = (text: string, chars: number) => (text.length > chars ? `…${text.slice(-chars).replace(/^\S*\s/, "")}` : text);

export const TALK_MODES: { value: TalkMode; label: string; hint: string }[] = [
  { value: "both", label: "Both ways", hint: "They hear you in their language, and you read them in yours." },
  { value: "me", label: "My voice", hint: "They hear you in their language." },
  { value: "them", label: "Their voice", hint: "You read (and can hear) them in your language." },
];

/**
 * Turning the conversation on asks first, because voices go to Google; without a key it opens
 * the key setup. Turning it off is immediate. The tray menu and the shortcut skip the question:
 * pressing them is the answer.
 */
export function useTranslateSwitch() {
  const settings = useApp((s) => s.settings);
  const info = useFeatures((s) => s.translate);
  const requested = useFeatures((s) => s.translateSetup);
  const [asking, setAsking] = useState<null | { side: Side | "talk" }>(null);
  const [setupOpen, setSetupOpen] = useState(false);

  useEffect(() => {
    if (requested) setSetupOpen(true);
  }, [requested]);

  const run = async (side: Side | "talk", on: boolean) => {
    try {
      setFeatures({ translate: side === "talk" ? await features.talkSet(on) : await features.translateSetSide(side, on) });
    } catch (e) {
      toast(String(e), "error");
    }
  };

  const request = (on: boolean, side: Side | "talk" = "talk") => {
    if (!on) {
      setAsking(null);
      return run(side, false);
    }
    if (!info?.hasKey) return setSetupOpen(true);
    if (settings?.translateAsk ?? true) return setAsking({ side });
    return run(side, true);
  };

  return {
    on: talking(info),
    me: !!info?.me.on,
    them: !!info?.them.on,
    asking: asking?.side ?? null,
    request,
    confirm: async (dontAsk: boolean) => {
      const side = asking?.side ?? "talk";
      setAsking(null);
      if (dontAsk) await saveSettings({ translateAsk: false });
      await run(side, true);
    },
    cancel: () => setAsking(null),
    setupOpen,
    closeSetup: () => setSetupOpen(false),
    openSetup: () => setSetupOpen(true),
  };
}

/** The directions a request turns on, for the question. */
function sidesOf(side: Side | "talk" | null, mode: TalkMode): { me: boolean; them: boolean } {
  if (side === "me") return { me: true, them: false };
  if (side === "them") return { me: false, them: true };
  return { me: mode !== "them", them: mode !== "me" };
}

/** "Start translating now?" in place, with Start and Not now. */
export function TranslateConfirm({ side = "talk", onConfirm, onCancel, compact = false }: { side?: Side | "talk" | null; onConfirm: (dontAsk: boolean) => void; onCancel: () => void; compact?: boolean }) {
  const settings = useApp((s) => s.settings);
  const [dontAsk, setDontAsk] = useState(false);
  const theirs = languageName(settings?.translateTo ?? "es");
  const mine = languageName(settings?.translateFrom ?? "en");
  const { me, them } = sidesOf(side, settings?.talkMode ?? "both");
  const what = me && them ? `Both voices go to Google Gemini: they hear you in ${theirs}, you read them in ${mine}.` : me ? `Your voice goes to Google Gemini and the call hears you in ${theirs}.` : `Their voice goes to Google Gemini and you read them in ${mine}.`;
  return (
    <motion.div initial={{ height: 0, opacity: 0 }} animate={{ height: "auto", opacity: 1 }} exit={{ height: 0, opacity: 0 }} transition={{ type: "spring", stiffness: 380, damping: 32 }} className="overflow-hidden">
      <div className={`${compact ? "mt-2 p-2.5" : "mt-3 p-3.5"} rounded-2xl`} style={{ background: "linear-gradient(135deg, rgba(255,196,176,.45), rgba(255,224,166,.4) 50%, rgba(201,242,212,.45))", boxShadow: "inset 0 0 0 1px rgba(255,255,255,.7)" }}>
        <div className={`${compact ? "text-[12.5px]" : "text-[13.5px]"} font-[650]`}>Start translating now?</div>
        <p className={`mt-0.5 ${compact ? "text-[11px]" : "text-[12px]"} leading-snug text-ink-2`}>
          {what} {compact ? "" : "Noise cancellation stays on this computer."}
        </p>
        <div className="mt-2 flex flex-wrap items-center gap-2">
          <button className="btn btn-primary btn-sm h-8" onClick={() => onConfirm(dontAsk)}>
            Start translating
          </button>
          <button className="btn btn-ghost btn-sm h-8" onClick={onCancel}>
            Not now
          </button>
          <label className="ml-auto flex cursor-pointer items-center gap-1.5 text-[11px] text-ink-2">
            <input type="checkbox" className="h-3.5 w-3.5 accent-[#1e1b18]" checked={dontAsk} onChange={(e) => setDontAsk(e.target.checked)} />
            Don't ask again
          </label>
        </div>
      </div>
    </motion.div>
  );
}

/** "I speak" ⇄ "They speak". Both directions use the pair, and it is remembered. */
export function LanguagePair({ compact = true, inline = false }: { compact?: boolean; inline?: boolean }) {
  const settings = useApp((s) => s.settings)!;
  const from = settings.translateFrom ?? "en";
  const to = settings.translateTo ?? "es";
  return (
    <div className={`grid grid-cols-[1fr_auto_1fr] ${inline ? "items-center" : "items-end"} gap-2`}>
      <div className="min-w-0">
        {!inline && <div className="mb-1 px-1 text-[11px] font-semibold text-muted">I speak</div>}
        <LanguagePicker label="I speak" value={from} onChange={(translateFrom) => saveSettings({ translateFrom })} compact={compact} />
      </div>
      <motion.button whileTap={{ rotate: 180 }} transition={{ type: "spring", stiffness: 400, damping: 20 }} className={`btn btn-ghost btn-icon ${inline ? "" : "mb-0.5"} h-9 w-9`} aria-label="Swap languages" title="Swap languages" onClick={() => saveSettings({ translateFrom: to, translateTo: from })}>
        <SwapIcon size={16} />
      </motion.button>
      <div className="min-w-0">
        {!inline && <div className="mb-1 px-1 text-[11px] font-semibold text-muted">They speak</div>}
        <LanguagePicker label="They speak" value={to} onChange={(translateTo) => saveSettings({ translateTo })} compact={compact} />
      </div>
    </div>
  );
}

export function laneLabel(lane: LaneInfo, side: Side, language: string) {
  if (lane.status === "connecting") return "Connecting to Google…";
  if (lane.status === "error") return lane.message ?? "Something went wrong";
  return side === "me" ? `They hear you in ${languageName(language)}` : `You read them in ${languageName(language)}`;
}

/** One side's live caption: the translation large, what was said small. `pending` shows only
 * the phrase still being said (the chat already shows the finished ones). */
export function LaneCaption({ side, large = false, pending = false }: { side: Side; large?: boolean; pending?: boolean }) {
  const settings = useApp((s) => s.settings);
  const lane = useFeatures((s) => s.translate?.[side]);
  if (!lane?.on) return null;
  const language = side === "me" ? settings?.translateTo ?? "es" : settings?.translateFrom ?? "en";
  const color = lane.status === "error" ? "var(--h-red)" : lane.status === "on" ? "var(--h-green)" : "var(--h-amber)";
  // For their side the translation is what you need; for yours, what you said comes first.
  const original = pending ? lane.pendingOriginal : lane.original;
  const translated = pending ? lane.pendingTranslated : lane.translated;
  const main = side === "them" ? translated : original;
  const sub = side === "them" ? original : translated;
  const chars = large ? 220 : 110;
  // Between phrases the chat needs only a quiet sign that Hush is listening.
  if (pending && !original && !translated && lane.status === "on") {
    return (
      <div className={`flex ${side === "me" ? "justify-end" : "justify-start"}`}>
        <span className="flex items-center gap-1.5 rounded-full bg-white/70 px-3 py-1.5 text-[11px] font-semibold text-muted" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
          <span className="dot animate-pulse" style={{ color }} />
          {side === "me" ? "Listening to you" : "Listening to them"}
        </span>
      </div>
    );
  }
  return (
    <div className={`${large ? "space-y-1.5 p-3.5" : "space-y-1 p-3"} rounded-2xl ${side === "me" ? "bg-[rgba(30,27,24,.045)]" : "bg-white/80"}`} style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
      <div className="flex items-center gap-1.5 text-[11px] font-semibold" style={{ color }}>
        {lane.status === "connecting" ? <Spinner size={11} /> : <span className="dot" />}
        <span className="text-ink-2">{side === "me" ? "You" : "Them"}</span>
        <span className="truncate font-[520]">· {laneLabel(lane, side, language)}</span>
      </div>
      <p className={`${large ? "min-h-[24px] text-[16px]" : "min-h-[18px] text-[13.5px]"} font-[560] leading-snug text-ink`}>
        {tail(main, chars) || <span className="text-faint">{side === "me" ? "Speak, and the translation follows." : "Their words appear here in your language."}</span>}
      </p>
      {sub && <p className={`${large ? "text-[12.5px]" : "text-[11.5px]"} leading-snug text-muted`}>{tail(sub, chars)}</p>}
    </div>
  );
}

/** Captions for whichever sides are on. */
export function LiveCaptions({ large = false }: { large?: boolean }) {
  const info = useFeatures((s) => s.translate);
  if (!talking(info)) return null;
  return (
    <div className="space-y-2">
      <LaneCaption side="them" large={large} />
      <LaneCaption side="me" large={large} />
    </div>
  );
}

/** "Both ways · My voice · Their voice". */
export function ModePicker({ size = "sm" }: { size?: "sm" | "md" }) {
  const settings = useApp((s) => s.settings);
  const info = useFeatures((s) => s.translate);
  const mode = settings?.talkMode ?? "both";
  const change = async (talkMode: TalkMode) => {
    await saveSettings({ talkMode });
    // While talking, the directions follow the new mode at once.
    if (talking(info)) setFeatures({ translate: await features.talkSet(true) });
  };
  return <Segmented size={size} label="Translate" value={mode} options={TALK_MODES.map(({ value, label }) => ({ value, label }))} onChange={change} />;
}

const CLEAN: { value: CleanLevel; label: string }[] = [
  { value: "off", label: "Off" },
  { value: "light", label: "Light" },
  { value: "full", label: "Full" },
];

const HEAR: { value: HearMode; label: string; hint: string }[] = [
  { value: "original", label: "Their voice", hint: "Hear them as they are and read the translation." },
  { value: "voiceOver", label: "Voice-over", hint: "Their voice turns down while the translation speaks over it, like an interpreter on TV. Best for following along." },
  { value: "translation", label: "Translation", hint: "Hear only the translated voice." },
];

/** VB-Audio's page with Hi-Fi Cable, free for end users. Hush may not bundle it. */
const HIFI_CABLE_PAGE = "https://vb-audio.com/Cable/";

/**
 * Windows has no Hush AI Speaker of its own yet (VB-CABLE is already Hush AI Microphone), so
 * Hush names a second cable: download, install, then one click (one administrator prompt).
 */
function WindowsSpeakerSetup() {
  const [status, setStatus] = useState<"ready" | "installed" | "missing" | null>(null);
  const [busy, setBusy] = useState(false);
  // Notice the cable as soon as it is installed.
  useEffect(() => {
    let alive = true;
    const check = () => features.speakerStatus().then((s) => alive && setStatus(s)).catch(() => undefined);
    check();
    const timer = window.setInterval(check, 3000);
    window.addEventListener("focus", check);
    return () => {
      alive = false;
      clearInterval(timer);
      window.removeEventListener("focus", check);
    };
  }, []);
  const name = async () => {
    setBusy(true);
    try {
      setStatus(await features.speakerSetup());
      setFeatures({ listen: await features.listenInfo() });
      toast("Hush AI Speaker is ready. Choose it as the speaker in your meeting app.");
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setBusy(false);
    }
  };
  const step = (n: number, done: boolean, title: string, body: string, action?: ReactNode) => (
    <li className="flex gap-2.5">
      <span className={`grid h-5 w-5 shrink-0 place-items-center rounded-full text-[10.5px] font-bold ${done ? "bg-[var(--h-green)] text-white" : "bg-[rgba(30,27,24,.08)] text-ink-2"}`}>{done ? "✓" : n}</span>
      <div className="min-w-0 flex-1">
        <div className="text-[12.5px] font-[620]">{title}</div>
        <div className="text-[11.5px] leading-snug text-muted">{body}</div>
        {action && <div className="mt-1.5">{action}</div>}
      </div>
    </li>
  );
  const installed = status === "installed" || status === "ready";
  return (
    <div className="rounded-2xl bg-white/70 p-3.5" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
      <div className="mb-1 text-[13px] font-[650]">Set up Hush AI Speaker</div>
      <p className="mb-3 text-[11.5px] leading-snug text-muted">Windows needs one more free virtual cable for it (VB-CABLE is already your Hush AI Microphone). Until then, Hush translates their voice by listening along to your speakers.</p>
      <ol className="space-y-3">
        {step(
          1,
          installed,
          "Download Hi-Fi Cable from VB-Audio",
          "Free for personal use. On VB-Audio's page, find \u201cVB-Audio Hi-Fi Cable\u201d and download it.",
          !installed && (
            <button className="btn btn-soft btn-sm h-8" onClick={() => openUrl(HIFI_CABLE_PAGE)}>
              Open VB-Audio
            </button>
          ),
        )}
        {step(2, installed, "Install it, then restart Windows", "Extract the zip and run its setup as administrator. Hush notices it on its own.")}
        {step(
          3,
          status === "ready",
          "Name it Hush AI Speaker",
          "One click; Windows asks for permission once.",
          status === "installed" && (
            <button className="btn btn-primary btn-sm h-8" disabled={busy} onClick={name}>
              {busy ? <Spinner size={13} /> : null} Name it Hush AI Speaker
            </button>
          ),
        )}
      </ol>
    </div>
  );
}

/** Hush AI Speaker: where their voice comes from, cleaning, and what you hear. */
export function TheirVoiceOptions() {
  const settings = useApp((s) => s.settings);
  const devices = useApp((s) => s.devices);
  const listen = useFeatures((s) => s.listen);
  if (!settings) return null;
  const platform = useApp((s) => s.platform);
  const speaker = listen?.mode === "speaker";
  const outputs = (devices?.outputs ?? [{ name: null, label: "System default" }]).filter((d) => !/hush ai|cable|vb-audio/i.test(d.name ?? ""));
  return (
    <div className="space-y-4">
      <div className="flex items-start gap-2.5 rounded-2xl bg-white/70 p-3" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
        <SpeakerIcon size={18} className={listen?.active ? "mt-0.5 text-green" : "mt-0.5 text-ink-2"} />
        <div className="min-w-0 text-[12px] leading-snug">
          {speaker ? (
            <>
              <div className="font-[620]">{listen?.device ?? "Hush AI Speaker"} is ready</div>
              <div className="text-muted">In your meeting app, choose it as the speaker. Hush plays their voice on {listen?.output ?? "your speakers"}{listen?.active ? " · their voice is playing now" : ""}.</div>
            </>
          ) : listen?.mode === "loopback" ? (
            <>
              <div className="font-[620]">Listening along to your speakers</div>
              <div className="text-muted">Hush translates what your speakers play, with no setup. To also clean their voice or hear a voice-over, set up Hush AI Speaker below.</div>
            </>
          ) : (
            <>
              <div className="font-[620]">Hush AI Speaker is not set up</div>
              <div className="text-muted">{listen?.message ?? "Hush cannot hear what your meeting app plays on this computer yet."}</div>
            </>
          )}
          {listen?.message && speaker && <div className="mt-1 text-amber">{listen.message}</div>}
        </div>
      </div>
      {!speaker && platform === "windows" && <WindowsSpeakerSetup />}
      {speaker && (
        <>
          <div>
            <div className="mb-1.5 flex items-center justify-between">
              <span className="text-[12.5px] font-[620]">Clean their voice</span>
              {listen?.cleaning && <span className="chip h-5 text-[10.5px] text-green">Cleaning</span>}
            </div>
            <Segmented size="sm" label="Clean their voice" value={settings.listen ? settings.listenClean ?? "full" : "off"} options={CLEAN} onChange={(v) => saveSettings({ listen: v !== "off", listenClean: v === "off" ? settings.listenClean : v })} />
            <p className="mt-1 text-[11px] leading-snug text-muted">Removes their keyboard, traffic and room noise before you hear it. Light keeps a little room sound.</p>
          </div>
          <div>
            <div className="mb-1.5 text-[12.5px] font-[620]">While their voice is translated, you hear</div>
            <Segmented size="sm" label="You hear" value={settings.listenVoice ?? "voiceOver"} options={HEAR.map(({ value, label }) => ({ value, label }))} onChange={(listenVoice) => saveSettings({ listenVoice })} />
            <p className="mt-1 text-[11px] leading-snug text-muted">{HEAR.find((h) => h.value === (settings.listenVoice ?? "voiceOver"))?.hint}</p>
          </div>
          <div>
            <div className="mb-1.5 text-[12.5px] font-[620]">Play their voice on</div>
            <DevicePicker label="Play their voice on" icon={<SpeakerIcon size={16} />} devices={outputs} value={settings.listenOutput ?? null} onChange={(listenOutput) => saveSettings({ listenOutput })} />
          </div>
        </>
      )}
    </div>
  );
}

/** A small anchored panel with the options for their voice. */
export function TheirVoiceButton() {
  const listen = useFeatures((s) => s.listen);
  const [open, setOpen] = useState(false);
  return (
    <>
      <button className="btn btn-ghost btn-sm h-9 gap-1.5 px-3" onClick={() => setOpen(true)} title="Their voice: Hush AI Speaker">
        <span className="relative">
          <SpeakerIcon size={16} />
          {listen?.active && <span className="absolute -right-0.5 -top-0.5 h-1.5 w-1.5 rounded-full bg-[var(--h-green)]" />}
        </span>
        Their voice
      </button>
      {createPortal(
        <AnimatePresence>
          {open && (
            <motion.div className="fixed inset-0 z-50 grid place-items-center bg-[rgba(30,27,24,.18)] p-6 backdrop-blur-[2px]" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} onMouseDown={(e) => e.target === e.currentTarget && setOpen(false)}>
              <motion.div role="dialog" aria-label="Their voice" initial={{ y: 10, scale: 0.98 }} animate={{ y: 0, scale: 1 }} exit={{ y: 10, scale: 0.98 }} transition={{ type: "spring", stiffness: 420, damping: 34 }} className="w-full max-w-[440px] rounded-[26px] bg-[var(--h-bg-2,#fbf8f4)] p-5" style={{ boxShadow: "0 0 0 1px var(--h-line), var(--h-shadow-3, 0 30px 80px rgba(30,27,24,.25))" }}>
                <div className="mb-3 flex items-center justify-between">
                  <div className="text-[16px] font-[650] tracking-[-0.02em]">Their voice</div>
                  <button className="btn btn-ghost btn-icon h-8 w-8" aria-label="Close" onClick={() => setOpen(false)}>
                    <CloseIcon size={15} />
                  </button>
                </div>
                <TheirVoiceOptions />
              </motion.div>
            </motion.div>
          )}
        </AnimatePresence>,
        document.body,
      )}
    </>
  );
}

/** The Live tab's card: the conversation switch, mode, languages and captions. */
export function TranslateCard({ onOpen }: { onOpen?: () => void }) {
  const info = useFeatures((s) => s.translate);
  const toggle = useTranslateSwitch();

  return (
    <div className="glass shrink-0 p-5">
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          <TranslateIcon size={18} />
          <span className="text-[15px] font-semibold">Live Translate</span>
          <span className="chip h-5 text-[10.5px]">Preview</span>
        </div>
        <Toggle label="Live Translate" on={toggle.on} onChange={(on) => toggle.request(on)} />
      </div>
      <p className="mt-1 text-[12.5px] leading-snug text-muted">Talk with anyone in their language. Works with noise cancellation on or off.</p>
      <div className="mt-3">
        <ModePicker />
      </div>
      <div className="mt-3">
        <LanguagePair />
      </div>
      <AnimatePresence initial={false}>{toggle.asking && <TranslateConfirm side={toggle.asking} onConfirm={toggle.confirm} onCancel={toggle.cancel} />}</AnimatePresence>
      <AnimatePresence initial={false}>
        {toggle.on && (
          <motion.div initial={{ height: 0, opacity: 0 }} animate={{ height: "auto", opacity: 1 }} exit={{ height: 0, opacity: 0 }} className="overflow-hidden">
            <div className="mt-3">
              <LiveCaptions />
            </div>
          </motion.div>
        )}
      </AnimatePresence>
      <div className="mt-2.5 flex items-center justify-between gap-2 text-[11.5px] text-muted">
        {info?.hasKey ? (
          <span>
            <kbd className="font-sans font-semibold text-ink-2">{TRANSLATE_SHORTCUT}</kbd> during calls
          </span>
        ) : (
          <button className="btn btn-soft btn-sm" onClick={toggle.openSetup}>
            Add your Gemini API key
          </button>
        )}
        {onOpen && (
          <button className="btn btn-ghost btn-sm h-7 px-2.5 text-[12px]" onClick={onOpen}>
            Conversations <ArrowRightIcon size={13} />
          </button>
        )}
      </div>
      <GeminiKeyConnect open={toggle.setupOpen} onClose={toggle.closeSetup} />
    </div>
  );
}

/** The widget's one-line version: languages, swap and the switch, which asks in place. */
export function TranslateRow() {
  const settings = useApp((s) => s.settings);
  const info = useFeatures((s) => s.translate);
  const toggle = useTranslateSwitch();
  if (!settings) return null;
  const from = settings.translateFrom ?? "en";
  const to = settings.translateTo ?? "es";
  const short = (code: string) => code.split("-")[0].toUpperCase();
  const mode = TALK_MODES.find((m) => m.value === (settings.talkMode ?? "both"))!;
  const connecting = info?.me.status === "connecting" || info?.them.status === "connecting";
  return (
    <div className="relative mt-3 shrink-0 rounded-[18px] bg-white/80 px-3 py-2.5" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
      <div className="flex items-center gap-2.5">
        <TranslateIcon size={17} className={toggle.on ? "text-green" : "text-ink-2"} />
        <div className="min-w-0 flex-1">
          <div className="text-[12.5px] font-[620]">Translate · {mode.label}</div>
          <button className="flex items-center gap-1 text-[11.5px] font-semibold text-muted hover:text-ink" title="Swap languages" onClick={() => saveSettings({ translateFrom: to, translateTo: from })}>
            {short(from)} <SwapIcon size={11} /> {short(to)}
            {connecting && <Spinner size={10} />}
          </button>
        </div>
        <Toggle
          label="Live Translate"
          on={toggle.on}
          onChange={(next) => {
            // Without a key, the main window opens at the setup.
            if (next && !info?.hasKey) return features.talkSet(true);
            toggle.request(next);
          }}
        />
      </div>
      <AnimatePresence initial={false}>{toggle.asking && <TranslateConfirm compact side={toggle.asking} onConfirm={toggle.confirm} onCancel={toggle.cancel} />}</AnimatePresence>
      <AnimatePresence initial={false}>
        {toggle.on && (
          <motion.div initial={{ height: 0, opacity: 0 }} animate={{ height: "auto", opacity: 1 }} exit={{ height: 0, opacity: 0 }} className="overflow-hidden">
            <div className="mt-2">
              <LiveCaptions />
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
