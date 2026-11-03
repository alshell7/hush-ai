// Live Translate controls: the switch, "I speak" and "They hear", swap, and live captions.
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { features, languageName, setFeatures, TRANSLATE_SHORTCUT, useFeatures } from "../lib/features";
import { saveSettings, useApp } from "../lib/store";
import { Spinner, Toggle } from "./Controls";
import { GeminiKeyConnect } from "./GeminiKeyConnect";
import { SwapIcon, TranslateIcon } from "./Icons";
import { LanguagePicker } from "./LanguagePicker";
import { toast } from "./Toast";

const tail = (text: string, chars: number) => (text.length > chars ? `…${text.slice(-chars).replace(/^\S*\s/, "")}` : text);

/** Opens the key setup when the tray menu or the shortcut asks for it. */
function useSetupRequests(open: () => void) {
  const requested = useFeatures((s) => s.translateSetup);
  useEffect(() => {
    if (requested) open();
  }, [requested, open]);
}

export function TranslateCard() {
  const settings = useApp((s) => s.settings)!;
  const info = useFeatures((s) => s.translate);
  const [setupOpen, setSetupOpen] = useState(false);
  useSetupRequests(() => setSetupOpen(true));
  const from = settings.translateFrom ?? "en";
  const to = settings.translateTo ?? "es";
  const on = !!info?.on;

  const toggle = async (next: boolean) => {
    if (next && !info?.hasKey) return setSetupOpen(true);
    try {
      setFeatures({ translate: await features.translateSet(next) });
    } catch (e) {
      toast(String(e), "error");
    }
  };

  return (
    <div className="glass shrink-0 p-5">
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          <TranslateIcon size={18} />
          <span className="text-[15px] font-semibold">Live Translate</span>
          <span className="chip h-5 text-[10.5px]">Preview</span>
        </div>
        <Toggle label="Live Translate" on={on} onChange={toggle} />
      </div>
      <p className="mt-1 text-[12.5px] leading-snug text-muted">Speak your language; the call hears you in theirs.</p>

      <div className="mt-3 grid grid-cols-[1fr_auto_1fr] items-end gap-2">
        <div className="min-w-0">
          <div className="mb-1 px-1 text-[11px] font-semibold text-muted">I speak</div>
          <LanguagePicker label="I speak" value={from} onChange={(translateFrom) => saveSettings({ translateFrom })} compact />
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
          <LanguagePicker label="They hear" value={to} onChange={(translateTo) => saveSettings({ translateTo })} compact />
        </div>
      </div>

      <AnimatePresence initial={false}>
        {on && info && (
          <motion.div initial={{ height: 0, opacity: 0 }} animate={{ height: "auto", opacity: 1 }} exit={{ height: 0, opacity: 0 }} className="overflow-hidden">
            <div className="mt-3 space-y-1.5 rounded-2xl bg-white/75 p-3" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
              <div className="flex items-center gap-1.5 text-[11px] font-semibold" style={{ color: info.status === "error" ? "var(--h-red)" : info.status === "on" ? "var(--h-green)" : "var(--h-amber)" }}>
                {info.status === "connecting" ? <Spinner size={11} /> : <span className="dot" />}
                {info.status === "connecting" ? "Connecting to Google…" : info.status === "error" ? info.message : `Live · ${languageName(to)}`}
              </div>
              <p className="min-h-[18px] text-[13.5px] font-[560] leading-snug text-ink">{tail(info.said, 120) || <span className="text-faint">The translation appears here as you talk.</span>}</p>
              {info.heard && <p className="text-[12px] leading-snug text-muted">{tail(info.heard, 120)}</p>}
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      <div className="mt-2.5 flex items-center justify-between gap-2 text-[11.5px] text-muted">
        {info?.hasKey ? (
          <span>
            Uses Google Gemini · <kbd className="font-sans font-semibold text-ink-2">{TRANSLATE_SHORTCUT}</kbd> during calls
          </span>
        ) : (
          <button className="btn btn-soft btn-sm" onClick={() => setSetupOpen(true)}>
            Add your Gemini API key
          </button>
        )}
      </div>
      <GeminiKeyConnect open={setupOpen} onClose={() => setSetupOpen(false)} />
    </div>
  );
}

/** The widget's one-line version: languages, swap and the switch. */
export function TranslateRow() {
  const settings = useApp((s) => s.settings);
  const info = useFeatures((s) => s.translate);
  if (!settings) return null;
  const from = settings.translateFrom ?? "en";
  const to = settings.translateTo ?? "es";
  const short = (code: string) => code.split("-")[0].toUpperCase();
  return (
    <div className="relative mt-3 flex shrink-0 items-center gap-2.5 rounded-[18px] bg-white/80 px-3 py-2.5" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
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
        on={!!info?.on}
        onChange={async (next) => {
          // Without a key, the main window opens at the setup.
          setFeatures({ translate: await features.translateSet(next) });
        }}
      />
    </div>
  );
}
