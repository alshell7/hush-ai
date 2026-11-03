// Live Translate controls: the switch (which asks first), "I speak" and "They hear", swap,
// and live captions. Used on the Live tab, the Translate tab and the tray widget.
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { features, languageName, setFeatures, TRANSLATE_SHORTCUT, useFeatures } from "../lib/features";
import { saveSettings, useApp } from "../lib/store";
import { Spinner, Toggle } from "./Controls";
import { GeminiKeyConnect } from "./GeminiKeyConnect";
import { ArrowRightIcon, SwapIcon, TranslateIcon } from "./Icons";
import { LanguagePicker } from "./LanguagePicker";
import { toast } from "./Toast";

export const tail = (text: string, chars: number) => (text.length > chars ? `…${text.slice(-chars).replace(/^\S*\s/, "")}` : text);

/**
 * Turning Live Translate on asks first, because it sends your voice to Google; without a key
 * it opens the key setup. Turning it off is immediate. The tray menu and the shortcut skip the
 * question: pressing them is the answer.
 */
export function useTranslateSwitch() {
  const settings = useApp((s) => s.settings);
  const info = useFeatures((s) => s.translate);
  const requested = useFeatures((s) => s.translateSetup);
  const [asking, setAsking] = useState(false);
  const [setupOpen, setSetupOpen] = useState(false);

  useEffect(() => {
    if (requested) setSetupOpen(true);
  }, [requested]);

  const set = async (on: boolean) => {
    try {
      setFeatures({ translate: await features.translateSet(on) });
    } catch (e) {
      toast(String(e), "error");
    }
  };

  const request = (on: boolean) => {
    if (!on) {
      setAsking(false);
      return set(false);
    }
    if (!info?.hasKey) return setSetupOpen(true);
    if (settings?.translateAsk ?? true) return setAsking(true);
    return set(true);
  };

  return {
    on: !!info?.on,
    asking,
    request,
    confirm: async (dontAsk: boolean) => {
      setAsking(false);
      if (dontAsk) await saveSettings({ translateAsk: false });
      await set(true);
    },
    cancel: () => setAsking(false),
    setupOpen,
    closeSetup: () => setSetupOpen(false),
    openSetup: () => setSetupOpen(true),
  };
}

/** "Start translating now?" in place, with Start and Not now. */
export function TranslateConfirm({ onConfirm, onCancel, compact = false }: { onConfirm: (dontAsk: boolean) => void; onCancel: () => void; compact?: boolean }) {
  const settings = useApp((s) => s.settings);
  const [dontAsk, setDontAsk] = useState(false);
  const to = languageName(settings?.translateTo ?? "es");
  return (
    <motion.div
      initial={{ height: 0, opacity: 0 }}
      animate={{ height: "auto", opacity: 1 }}
      exit={{ height: 0, opacity: 0 }}
      transition={{ type: "spring", stiffness: 380, damping: 32 }}
      className="overflow-hidden"
    >
      <div className={`${compact ? "mt-2 p-2.5" : "mt-3 p-3.5"} rounded-2xl`} style={{ background: "linear-gradient(135deg, rgba(255,196,176,.45), rgba(255,224,166,.4) 50%, rgba(201,242,212,.45))", boxShadow: "inset 0 0 0 1px rgba(255,255,255,.7)" }}>
        <div className={`${compact ? "text-[12.5px]" : "text-[13.5px]"} font-[650]`}>Start translating now?</div>
        <p className={`mt-0.5 ${compact ? "text-[11px]" : "text-[12px]"} leading-snug text-ink-2`}>
          Your voice goes to Google Gemini and the call hears you in {to}. {compact ? "" : "Noise cancellation stays on this computer."}
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

/** "I speak" ⇄ "They hear". */
export function LanguagePair({ compact = true }: { compact?: boolean }) {
  const settings = useApp((s) => s.settings)!;
  const from = settings.translateFrom ?? "en";
  const to = settings.translateTo ?? "es";
  return (
    <div className="grid grid-cols-[1fr_auto_1fr] items-end gap-2">
      <div className="min-w-0">
        <div className="mb-1 px-1 text-[11px] font-semibold text-muted">I speak</div>
        <LanguagePicker label="I speak" value={from} onChange={(translateFrom) => saveSettings({ translateFrom })} compact={compact} />
      </div>
      <motion.button
        whileTap={{ rotate: 180 }}
        transition={{ type: "spring", stiffness: 400, damping: 20 }}
        className="btn btn-ghost btn-icon mb-0.5 h-9 w-9"
        aria-label="Swap languages"
        title="Swap languages"
        onClick={() => saveSettings({ translateFrom: to, translateTo: from })}
      >
        <SwapIcon size={16} />
      </motion.button>
      <div className="min-w-0">
        <div className="mb-1 px-1 text-[11px] font-semibold text-muted">They hear</div>
        <LanguagePicker label="They hear" value={to} onChange={(translateTo) => saveSettings({ translateTo })} compact={compact} />
      </div>
    </div>
  );
}

/** Status and the live captions while translating. */
export function LiveCaptions({ large = false }: { large?: boolean }) {
  const settings = useApp((s) => s.settings);
  const info = useFeatures((s) => s.translate);
  if (!info) return null;
  const to = settings?.translateTo ?? "es";
  return (
    <div className={`${large ? "space-y-2.5 p-4" : "space-y-1.5 p-3"} rounded-2xl bg-white/75`} style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
      <div className="flex items-center gap-1.5 text-[11px] font-semibold" style={{ color: info.status === "error" ? "var(--h-red)" : info.status === "on" ? "var(--h-green)" : "var(--h-amber)" }}>
        {info.status === "connecting" ? <Spinner size={11} /> : <span className="dot" />}
        {info.status === "connecting" ? "Connecting to Google…" : info.status === "error" ? info.message : `Live · ${languageName(to)}`}
      </div>
      <p className={`${large ? "min-h-[48px] text-[17px]" : "min-h-[18px] text-[13.5px]"} font-[560] leading-snug text-ink`}>
        {tail(info.said, large ? 220 : 120) || <span className="text-faint">The translation appears here as you talk.</span>}
      </p>
      {info.heard && <p className={`${large ? "text-[13px]" : "text-[12px]"} leading-snug text-muted`}>{tail(info.heard, large ? 220 : 120)}</p>}
    </div>
  );
}

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
        <Toggle label="Live Translate" on={toggle.on} onChange={toggle.request} />
      </div>
      <p className="mt-1 text-[12.5px] leading-snug text-muted">Speak your language; the call hears you in theirs. Works with noise cancellation on or off.</p>

      <div className="mt-3">
        <LanguagePair />
      </div>

      <AnimatePresence initial={false}>
        {toggle.asking && <TranslateConfirm onConfirm={toggle.confirm} onCancel={toggle.cancel} />}
      </AnimatePresence>

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
            History <ArrowRightIcon size={13} />
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
  return (
    <div className="relative mt-3 shrink-0 rounded-[18px] bg-white/80 px-3 py-2.5" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
      <div className="flex items-center gap-2.5">
        <TranslateIcon size={17} className={info?.on ? "text-green" : "text-ink-2"} />
        <div className="min-w-0 flex-1">
          <div className="text-[12.5px] font-[620]">Translate my voice</div>
          <button className="flex items-center gap-1 text-[11.5px] font-semibold text-muted hover:text-ink" title="Swap languages" onClick={() => saveSettings({ translateFrom: to, translateTo: from })}>
            {short(from)} <SwapIcon size={11} /> {short(to)}
            {info?.on && info.status === "connecting" && <Spinner size={10} />}
          </button>
        </div>
        <Toggle
          label="Translate my voice"
          on={toggle.on}
          onChange={(next) => {
            // Without a key, the main window opens at the setup.
            if (next && !info?.hasKey) return features.translateSet(true);
            toggle.request(next);
          }}
        />
      </div>
      <AnimatePresence initial={false}>{toggle.asking && <TranslateConfirm compact onConfirm={toggle.confirm} onCancel={toggle.cancel} />}</AnimatePresence>
    </div>
  );
}
