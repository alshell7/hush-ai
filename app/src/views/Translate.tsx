// The Translate tab: Live Translate in full, and the history of what you said and what the
// other side heard. The history stays on this computer.
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useMemo, useState } from "react";
import { Toggle } from "../components/Controls";
import { GeminiKeyConnect } from "../components/GeminiKeyConnect";
import { CheckIcon, CopyIcon, LockIcon, TranslateIcon } from "../components/Icons";
import { Mochi } from "../components/Mochi";
import { toast } from "../components/Toast";
import { LanguagePair, LiveCaptions, TranslateConfirm, useTranslateSwitch } from "../components/TranslateCard";
import { features, languageName, loadHistory, setFeatures, TRANSLATE_SHORTCUT, useFeatures, type TranslateEntry } from "../lib/features";
import { saveSettings, useApp } from "../lib/store";
import { clockTime, dayLabel, useNow } from "../lib/time";

export function Translate() {
  const info = useFeatures((s) => s.translate);
  const toggle = useTranslateSwitch();

  return (
    <main className="grid min-h-0 flex-1 grid-cols-[1fr_1.05fr] gap-4">
      <section className="glass flex min-h-0 flex-col p-6">
        <div className="flex items-center justify-between gap-3">
          <div className="flex items-center gap-2.5">
            <span className="grid h-10 w-10 place-items-center rounded-2xl bg-white" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
              <TranslateIcon size={20} className={toggle.on ? "text-green" : undefined} />
            </span>
            <div>
              <div className="flex items-center gap-2 text-[17px] font-[650] tracking-[-0.02em]">
                Live Translate <span className="chip h-5 text-[10.5px]">Preview</span>
              </div>
              <div className="text-[12px] text-muted">Speak your language. The call hears theirs.</div>
            </div>
          </div>
          <Toggle label="Live Translate" size="lg" on={toggle.on} onChange={toggle.request} />
        </div>

        <div className="mt-5">
          <LanguagePair compact={false} />
        </div>

        <AnimatePresence initial={false}>{toggle.asking && <TranslateConfirm onConfirm={toggle.confirm} onCancel={toggle.cancel} />}</AnimatePresence>

        <div className="mt-4 flex-1">
          <AnimatePresence mode="wait" initial={false}>
            {toggle.on ? (
              <motion.div key="live" initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -6 }}>
                <LiveCaptions large />
              </motion.div>
            ) : (
              <motion.ul key="how" initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -6 }} className="space-y-2 text-[12.5px] leading-snug text-ink-2">
                <li className="tile px-3 py-2.5">Works in every app through Hush AI Microphone, with noise cancellation on or off.</li>
                <li className="tile px-3 py-2.5">
                  In a call, switch it from the tray, or press <kbd className="font-sans font-semibold">{TRANSLATE_SHORTCUT}</kbd>.
                </li>
                <li className="tile px-3 py-2.5">The translation follows you by a phrase or so.</li>
              </motion.ul>
            )}
          </AnimatePresence>
        </div>

        <div className="mt-4 flex items-start gap-1.5 text-[11.5px] leading-snug text-muted">
          <LockIcon size={13} className="mt-px shrink-0" />
          {info?.hasKey ? (
            <span>While it is on, your voice goes to Google Gemini with your API key. Everything else stays on this computer.</span>
          ) : (
            <span>
              Needs your own Gemini API key.{" "}
              <button className="link" onClick={toggle.openSetup}>
                Add it
              </button>
            </span>
          )}
        </div>
        <GeminiKeyConnect open={toggle.setupOpen} onClose={toggle.closeSetup} />
      </section>

      <History />
    </main>
  );
}

function History() {
  const history = useFeatures((s) => s.history);
  const keep = useApp((s) => s.settings?.translateHistory ?? true);
  const [query, setQuery] = useState("");
  const [confirmClear, setConfirmClear] = useState(false);
  const now = useNow(60_000);

  useEffect(() => {
    loadHistory();
  }, []);

  // Newest first, grouped by day, filtered by the search.
  const groups = useMemo(() => {
    const q = query.trim().toLowerCase();
    const list = [...(history ?? [])].reverse().filter((e) => !q || e.heard.toLowerCase().includes(q) || e.said.toLowerCase().includes(q));
    const map = new Map<string, TranslateEntry[]>();
    for (const entry of list) {
      const key = dayLabel(entry.at, now);
      map.set(key, [...(map.get(key) ?? []), entry]);
    }
    return [...map.entries()];
  }, [history, query, now]);

  const clear = async () => {
    await features.translateHistoryClear();
    setFeatures({ history: [] });
    setConfirmClear(false);
    toast("Translation history cleared.");
  };

  return (
    <section className="glass flex min-h-0 flex-col p-5">
      <div className="mb-3 flex items-center justify-between gap-3">
        <div>
          <div className="text-[15px] font-semibold">History</div>
          <div className="text-[11.5px] text-muted">{history?.length ? `${history.length} phrases · on this computer` : "Kept on this computer"}</div>
        </div>
        <div className="flex items-center gap-2">
          <span className="text-[11.5px] text-muted">Keep</span>
          <Toggle label="Keep translation history" on={keep} onChange={(translateHistory) => saveSettings({ translateHistory })} />
        </div>
      </div>
      {!!history?.length && (
        <div className="mb-2 flex gap-2">
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search what was said"
            className="h-9 min-w-0 flex-1 rounded-xl bg-white/80 px-3 text-[12.5px] outline-none placeholder:text-faint focus:bg-white"
            style={{ boxShadow: "inset 0 0 0 1px var(--h-line-2)", userSelect: "text" }}
          />
          {confirmClear ? (
            <button className="btn btn-sm h-9 bg-[#fdecea] text-red" onClick={clear}>
              Clear all
            </button>
          ) : (
            <button className="btn btn-ghost btn-sm h-9" onClick={() => setConfirmClear(true)} onBlur={() => setTimeout(() => setConfirmClear(false), 200)}>
              Clear
            </button>
          )}
        </div>
      )}
      <div className="scroll -mr-2 min-h-0 flex-1 space-y-4 pr-2">
        {history && history.length === 0 && (
          <div className="flex flex-col items-center gap-3 py-10 text-center">
            <Mochi size={56} mood="sleep" />
            <div className="max-w-[260px] text-[12.5px] leading-snug text-muted">
              {keep ? "What you say and what the other side hears appear here, phrase by phrase." : "History is off. Turn on Keep to save translations on this computer."}
            </div>
          </div>
        )}
        {groups.map(([day, entries]) => (
          <div key={day}>
            <div className="eyebrow mb-1.5 px-1">{day}</div>
            <ul className="space-y-2">
              {entries.map((entry) => (
                <Phrase key={entry.at} entry={entry} />
              ))}
            </ul>
          </div>
        ))}
        {history && history.length > 0 && groups.length === 0 && <div className="py-6 text-center text-[12.5px] text-muted">Nothing matches “{query}”.</div>}
      </div>
    </section>
  );
}

function Phrase({ entry }: { entry: TranslateEntry }) {
  const [copied, setCopied] = useState(false);
  const short = (code: string) => code.split("-")[0].toUpperCase();
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(entry.said);
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    } catch {
      toast("Could not copy.", "error");
    }
  };
  return (
    <motion.li layout initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} className="group rounded-2xl bg-white/80 p-3" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)", userSelect: "text" }}>
      <div className="mb-1 flex items-center gap-2 text-[10.5px] font-semibold text-muted">
        <span className="tabular-nums">{clockTime(entry.at)}</span>
        <span className="chip h-5 text-[10px]" title={`${languageName(entry.from)} → ${languageName(entry.to)}`}>
          {short(entry.from)} → {short(entry.to)}
        </span>
        <button className="ml-auto opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100" aria-label="Copy the translation" onClick={copy}>
          {copied ? <CheckIcon size={14} className="text-green" /> : <CopyIcon size={14} />}
        </button>
      </div>
      {entry.heard && <p className="text-[12.5px] leading-snug text-muted">{entry.heard}</p>}
      {entry.said && <p className="mt-0.5 text-[14px] font-[560] leading-snug text-ink">{entry.said}</p>}
    </motion.li>
  );
}
