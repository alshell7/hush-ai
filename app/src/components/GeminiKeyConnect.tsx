// Add a Gemini API key for Live Translate. Like the calendar popup: open the page, copy, and
// Hush notices the key on the clipboard when you come back, checks it with Google, keeps it.
import { AnimatePresence, motion } from "motion/react";
import { useCallback, useEffect, useRef, useState } from "react";
import { clipboardText, features, GEMINI_KEY_PATTERN } from "../lib/features";
import { Spinner } from "./Controls";
import { CheckIcon, CloseIcon, CopyIcon, ExternalIcon, KeyIcon, LockIcon, TranslateIcon } from "./Icons";

type State = { kind: "waiting" } | { kind: "checking" } | { kind: "saved" } | { kind: "error"; message: string };

export function GeminiKeyConnect({ open, onClose }: { open: boolean; onClose: () => void }) {
  return (
    <AnimatePresence>
      {open && (
        <motion.div className="fixed inset-0 z-40 grid place-items-center p-5" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}>
          <div className="absolute inset-0 bg-[rgba(30,27,24,.24)] backdrop-blur-[3px]" onClick={onClose} />
          <motion.div
            role="dialog"
            aria-label="Set up Live Translate"
            initial={{ opacity: 0, y: 18, scale: 0.96 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 10, scale: 0.98 }}
            transition={{ type: "spring", stiffness: 360, damping: 30 }}
            className="relative w-full max-w-[560px] overflow-hidden rounded-[30px] bg-[#fdfcfa] p-6"
            style={{ boxShadow: "var(--h-shadow-3)" }}
          >
            <div className="pointer-events-none absolute -right-20 -top-24 h-56 w-56 rounded-full opacity-60 blur-3xl" style={{ background: "var(--h-opal)" }} />
            <Body onClose={onClose} />
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function Body({ onClose }: { onClose: () => void }) {
  const [state, setState] = useState<State>({ kind: "waiting" });
  const [manual, setManual] = useState("");
  const tried = useRef(new Set<string>());

  const check = useCallback(
    async (key: string) => {
      const clean = key.trim();
      if (!clean) return;
      tried.current.add(clean);
      setState({ kind: "checking" });
      try {
        await features.translateSetKey(clean);
        setState({ kind: "saved" });
        setTimeout(onClose, 1100);
      } catch (e) {
        setState({ kind: "error", message: String(e) });
      }
    },
    [onClose],
  );

  // The clipboard is read only while this popup is open, and only a key is used.
  const look = useCallback(async () => {
    if (document.visibilityState !== "visible") return;
    const text = (await clipboardText())?.trim();
    if (text && GEMINI_KEY_PATTERN.test(text) && !tried.current.has(text)) check(text);
  }, [check]);

  useEffect(() => {
    look();
    window.addEventListener("focus", look);
    document.addEventListener("visibilitychange", look);
    const key = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", key);
    return () => {
      window.removeEventListener("focus", look);
      document.removeEventListener("visibilitychange", look);
      window.removeEventListener("keydown", key);
    };
  }, [look, onClose]);

  return (
    <div className="relative space-y-4">
      <div className="flex items-start justify-between gap-4">
        <div className="flex items-center gap-3">
          <span className="grid h-11 w-11 place-items-center rounded-2xl bg-white" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
            <TranslateIcon size={21} />
          </span>
          <div>
            <h2 className="text-[18px] font-[650] leading-tight tracking-[-0.02em]">Set up Live Translate</h2>
            <div className="text-[12px] text-muted">With your own Google Gemini API key</div>
          </div>
        </div>
        <button className="btn btn-ghost btn-icon -mr-2 -mt-1 h-8 w-8" aria-label="Close" onClick={onClose}>
          <CloseIcon size={16} />
        </button>
      </div>

      <ol className="space-y-2.5 text-[13.5px]">
        <li className="flex items-center gap-3">
          <Num n={1} />
          <span className="flex-1 font-[600]">Open Google AI Studio and sign in</span>
          <button className="btn btn-soft btn-sm" onClick={() => features.translateOpenKeys()}>
            Open <ExternalIcon size={14} />
          </button>
        </li>
        <li className="flex items-center gap-3">
          <Num n={2} />
          <span className="font-[600]">
            Press <b>Create API key</b>, then copy it
          </span>
        </li>
        <li className="flex items-center gap-3">
          <Num n={3} />
          <span className="font-[600]">Come back here: Hush notices it by itself</span>
        </li>
      </ol>

      <AnimatePresence mode="wait" initial={false}>
        {state.kind === "waiting" && (
          <motion.div key="waiting" {...fade} className="space-y-2">
            <div className="relative flex h-12 items-center gap-2 overflow-hidden rounded-2xl bg-white px-3.5 text-[13px] text-muted" style={{ boxShadow: "inset 0 0 0 1px var(--h-line-2)" }}>
              <span className="absolute inset-0 opacity-60" style={{ background: "linear-gradient(90deg, transparent, rgba(255,224,166,.45), transparent)", backgroundSize: "200% 100%", animation: "h-shimmer 2.4s linear infinite" }} />
              <CopyIcon size={16} className="relative" />
              <span className="relative">Waiting for the copied key…</span>
            </div>
            <div className="flex gap-2">
              <input
                type="password"
                value={manual}
                onChange={(e) => setManual(e.target.value)}
                onPaste={(e) => {
                  const text = e.clipboardData.getData("text");
                  if (text) setTimeout(() => check(text));
                }}
                onKeyDown={(e) => e.key === "Enter" && check(manual)}
                placeholder="…or paste it here"
                spellCheck={false}
                autoComplete="off"
                className="h-10 min-w-0 flex-1 rounded-xl bg-white/80 px-3 text-[12.5px] outline-none placeholder:text-faint focus:bg-white"
                style={{ boxShadow: "inset 0 0 0 1px var(--h-line-2)", userSelect: "text" }}
              />
              <button className="btn btn-soft btn-sm h-10" disabled={!manual.trim()} onClick={() => check(manual)}>
                Check
              </button>
            </div>
          </motion.div>
        )}
        {state.kind === "checking" && (
          <motion.div key="checking" {...fade} className="flex h-[92px] items-center gap-3 rounded-2xl bg-white px-4" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
            <Spinner />
            <div className="text-[13.5px] font-semibold">Checking the key with Google…</div>
          </motion.div>
        )}
        {state.kind === "saved" && (
          <motion.div key="saved" {...fade} className="flex h-[92px] items-center gap-3 rounded-2xl bg-white px-4" style={{ boxShadow: "inset 0 0 0 1px rgba(31,157,114,.35)" }}>
            <span className="grid h-7 w-7 place-items-center rounded-full bg-green text-white">
              <CheckIcon size={15} draw />
            </span>
            <div>
              <div className="text-[13.5px] font-semibold">The key works. Live Translate is ready.</div>
              <div className="text-[12px] text-muted">Pick your languages and switch it on during a call.</div>
            </div>
          </motion.div>
        )}
        {state.kind === "error" && (
          <motion.div key="error" {...fade} className="space-y-2">
            <div className="rounded-2xl bg-[#fdecea] px-3.5 py-3 text-[12.5px] leading-snug text-red">{state.message}</div>
            <button className="btn btn-soft btn-sm" onClick={() => setState({ kind: "waiting" })}>
              Try another key
            </button>
          </motion.div>
        )}
      </AnimatePresence>

      <div className="space-y-1.5 rounded-2xl bg-white/60 p-3 text-[11.5px] leading-snug text-muted" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
        <p className="flex items-start gap-1.5">
          <LockIcon size={13} className="mt-px shrink-0" />
          The key stays in Hush's settings folder on this computer. Noise cancellation never leaves your computer; Live Translate sends your voice to Google while it is on.
        </p>
        <p className="flex items-start gap-1.5">
          <KeyIcon size={13} className="mt-px shrink-0" />
          Google offers a free tier. Usage and data terms are Google's, shown in AI Studio.
        </p>
      </div>
    </div>
  );
}

const fade = {
  initial: { opacity: 0, y: 8, filter: "blur(4px)" },
  animate: { opacity: 1, y: 0, filter: "blur(0px)" },
  exit: { opacity: 0, y: -6, filter: "blur(4px)" },
  transition: { duration: 0.26 },
};

function Num({ n }: { n: number }) {
  return (
    <span className="grid h-6 w-6 shrink-0 place-items-center rounded-full bg-white text-[12px] font-bold" style={{ boxShadow: "inset 0 0 0 1px var(--h-line-2)" }}>
      {n}
    </span>
  );
}
