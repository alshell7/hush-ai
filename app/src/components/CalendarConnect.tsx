// Connect Google Calendar without signing in. The popup opens Google Calendar's settings,
// shows where the secret iCal address is, notices it on the clipboard when you come back,
// checks it with Google and shows what it found before anything is saved.
import { AnimatePresence, motion, useAnimate, useReducedMotion } from "motion/react";
import { useCallback, useEffect, useRef, useState } from "react";
import { clipboardText, features, ICAL_PATTERN, type CalendarPreview } from "../lib/features";
import { clockTime, dayLabel } from "../lib/time";
import { AppIcon, GOOGLE_CALENDAR_LOGO } from "./AppIcons";
import { Spinner } from "./Controls";
import { CheckIcon, CloseIcon, CopyIcon, ExternalIcon, LockIcon } from "./Icons";

type State =
  | { kind: "waiting" }
  | { kind: "checking"; address: string }
  | { kind: "found"; address: string; preview: CalendarPreview }
  | { kind: "error"; address: string; message: string };

/** The address with its secret part hidden. */
export function maskAddress(address: string) {
  return address.replace(/^(?:https|webcal):\/\//, "").replace(/\/ical\/[^/]+\//, "/ical/…/").replace(/private-[^/]+/, "private-••••••");
}

export function CalendarConnect({ open, onClose }: { open: boolean; onClose: () => void }) {
  return (
    <AnimatePresence>
      {open && (
        <motion.div className="fixed inset-0 z-40 grid place-items-center p-5" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}>
          <div className="absolute inset-0 bg-[rgba(30,27,24,.24)] backdrop-blur-[3px]" onClick={onClose} />
          <motion.div
            role="dialog"
            aria-label="Connect Google Calendar"
            initial={{ opacity: 0, y: 18, scale: 0.96 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 10, scale: 0.98 }}
            transition={{ type: "spring", stiffness: 360, damping: 30 }}
            className="relative w-full max-w-[760px] overflow-hidden rounded-[30px] bg-[#fdfcfa]"
            style={{ boxShadow: "var(--h-shadow-3)" }}
          >
            <div className="pointer-events-none absolute -left-24 -top-28 h-64 w-64 rounded-full opacity-60 blur-3xl" style={{ background: "var(--h-opal)" }} />
            <ConnectBody onClose={onClose} />
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function ConnectBody({ onClose }: { onClose: () => void }) {
  const [state, setState] = useState<State>({ kind: "waiting" });
  const [manual, setManual] = useState("");
  const [connecting, setConnecting] = useState(false);
  const tried = useRef(new Set<string>());

  const check = useCallback(async (address: string) => {
    const clean = address.trim();
    if (!clean) return;
    tried.current.add(clean);
    setState({ kind: "checking", address: clean });
    try {
      const preview = await features.calendarCheck(clean);
      setState({ kind: "found", address: clean, preview });
    } catch (e) {
      setState({ kind: "error", address: clean, message: String(e) });
    }
  }, []);

  // When you come back from Google Calendar with the address copied, Hush picks it up.
  // The clipboard is read only while this popup is open, and only an iCal address is used.
  const look = useCallback(async () => {
    if (document.visibilityState !== "visible") return;
    const text = (await clipboardText())?.trim();
    if (text && ICAL_PATTERN.test(text) && !tried.current.has(text)) check(text);
  }, [check]);

  useEffect(() => {
    look();
    window.addEventListener("focus", look);
    document.addEventListener("visibilitychange", look);
    return () => {
      window.removeEventListener("focus", look);
      document.removeEventListener("visibilitychange", look);
    };
  }, [look]);

  useEffect(() => {
    const key = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [onClose]);

  const connect = async () => {
    if (state.kind !== "found") return;
    setConnecting(true);
    try {
      await features.calendarConnect(state.address);
      onClose();
    } catch (e) {
      setState({ kind: "error", address: state.address, message: String(e) });
    } finally {
      setConnecting(false);
    }
  };

  return (
    <div className="relative grid grid-cols-[316px_1fr] gap-6 p-6">
      <div className="space-y-3">
        <div className="flex items-center gap-2.5">
          <img src={GOOGLE_CALENDAR_LOGO} alt="" width={30} height={30} />
          <div>
            <h2 className="text-[18px] font-[650] leading-tight tracking-[-0.02em]">Connect Google Calendar</h2>
            <div className="text-[12px] text-muted">No sign-in, no password</div>
          </div>
        </div>
        <CalendarGuide />
        <p className="flex items-start gap-1.5 text-[11.5px] leading-snug text-muted">
          <LockIcon size={14} className="mt-px shrink-0" />
          The address stays on this computer. Hush reads it straight from Google and never sees your account.
        </p>
      </div>

      <div className="flex flex-col">
        <div className="flex justify-end">
          <button className="btn btn-ghost btn-icon -mr-2 -mt-2 h-8 w-8" aria-label="Close" onClick={onClose}>
            <CloseIcon size={16} />
          </button>
        </div>
        <ol className="space-y-3">
          <Step n={1} title="Open Google Calendar settings">
            <button className="btn btn-soft btn-sm mt-1.5" onClick={() => features.calendarOpenSettings()}>
              Open settings <ExternalIcon size={14} />
            </button>
          </Step>
          <Step n={2} title="Pick your calendar, then Integrate calendar">
            <span className="text-[12px] text-muted">Under “Settings for my calendars” on the left.</span>
          </Step>
          <Step n={3} title="Copy “Secret address in iCal format”">
            <span className="text-[12px] text-muted">Come back here: Hush notices it by itself.</span>
          </Step>
        </ol>

        <div className="mt-4 flex-1">
          <AnimatePresence mode="wait" initial={false}>
            {state.kind === "waiting" && (
              <motion.div key="waiting" {...fade} className="space-y-2">
                <div className="relative flex h-12 items-center gap-2 overflow-hidden rounded-2xl bg-white px-3.5 text-[13px] text-muted" style={{ boxShadow: "inset 0 0 0 1px var(--h-line-2)" }}>
                  <span className="absolute inset-0 opacity-60" style={{ background: "linear-gradient(90deg, transparent, rgba(255,224,166,.45), transparent)", backgroundSize: "200% 100%", animation: "h-shimmer 2.4s linear infinite" }} />
                  <CopyIcon size={16} className="relative" />
                  <span className="relative">Waiting for the copied address…</span>
                </div>
                <div className="flex gap-2">
                  <input
                    value={manual}
                    onChange={(e) => setManual(e.target.value)}
                    onPaste={(e) => {
                      const text = e.clipboardData.getData("text");
                      if (text) setTimeout(() => check(text));
                    }}
                    onKeyDown={(e) => e.key === "Enter" && check(manual)}
                    placeholder="…or paste it here"
                    spellCheck={false}
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
                <div>
                  <div className="text-[13.5px] font-semibold">Checking with Google…</div>
                  <div className="text-[11.5px] text-muted">{maskAddress(state.address)}</div>
                </div>
              </motion.div>
            )}
            {state.kind === "found" && (
              <motion.div key="found" {...fade} className="space-y-3">
                <motion.div layout className="rounded-2xl bg-white p-4" style={{ boxShadow: "inset 0 0 0 1px rgba(31,157,114,.35), 0 10px 26px -18px rgba(31,157,114,.8)" }}>
                  <div className="flex items-center gap-2 text-[13.5px] font-semibold">
                    <span className="grid h-6 w-6 place-items-center rounded-full bg-green text-white">
                      <CheckIcon size={14} draw />
                    </span>
                    {state.preview.name ?? state.preview.source}
                  </div>
                  <div className="mt-1.5 text-[12.5px] text-ink-2">
                    {state.preview.meetings} {state.preview.meetings === 1 ? "event" : "events"} this week · {state.preview.calls} with a call link
                  </div>
                  {state.preview.next && (
                    <div className="mt-2.5 flex items-center gap-2 rounded-xl bg-[#f7f5f1] px-2.5 py-2 text-[12px]">
                      {state.preview.next.app && <AppIcon app={state.preview.next.app} size={15} />}
                      <span className="truncate font-[560]">{state.preview.next.title}</span>
                      <span className="ml-auto shrink-0 text-muted">
                        {dayLabel(state.preview.next.start)} {clockTime(state.preview.next.start)}
                      </span>
                    </div>
                  )}
                </motion.div>
                <div className="flex items-center gap-2">
                  <button className="btn btn-primary" disabled={connecting} onClick={connect}>
                    {connecting ? <Spinner size={14} /> : null} Connect
                  </button>
                  <button className="btn btn-ghost btn-sm" onClick={() => setState({ kind: "waiting" })}>
                    Use another address
                  </button>
                </div>
              </motion.div>
            )}
            {state.kind === "error" && (
              <motion.div key="error" {...fade} className="space-y-2">
                <div className="rounded-2xl bg-[#fdecea] px-3.5 py-3 text-[12.5px] leading-snug text-red">{state.message}</div>
                <div className="flex gap-2">
                  <button className="btn btn-soft btn-sm" onClick={() => check(state.address)}>
                    Try again
                  </button>
                  <button className="btn btn-ghost btn-sm" onClick={() => setState({ kind: "waiting" })}>
                    Use another address
                  </button>
                </div>
              </motion.div>
            )}
          </AnimatePresence>
        </div>
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

function Step({ n, title, children }: { n: number; title: string; children?: React.ReactNode }) {
  return (
    <li className="flex gap-3">
      <span className="grid h-6 w-6 shrink-0 place-items-center rounded-full bg-white text-[12px] font-bold" style={{ boxShadow: "inset 0 0 0 1px var(--h-line-2)" }}>
        {n}
      </span>
      <div>
        <div className="text-[13.5px] font-[600] leading-6">{title}</div>
        {children}
      </div>
    </li>
  );
}

/** A miniature of Google Calendar's settings: the cursor finds and copies the secret address. */
function CalendarGuide() {
  const [scope, animate] = useAnimate<HTMLDivElement>();
  const reduce = useReducedMotion();

  useEffect(() => {
    if (reduce) return;
    let alive = true;
    const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
    const at = (name: string) => {
      const root = scope.current;
      const el = root?.querySelector<HTMLElement>(`[data-c="${name}"]`);
      if (!root || !el) return { x: 0, y: 0 };
      let x = el.offsetWidth / 2;
      let y = el.offsetHeight / 2;
      let node: HTMLElement | null = el;
      while (node && node !== root) {
        x += node.offsetLeft;
        y += node.offsetTop;
        node = node.offsetParent as HTMLElement | null;
      }
      return { x, y };
    };
    // Offsets ignore transforms, so `shift` adds the scroll of the settings pane.
    const move = (name: string, duration = 0.7, shift = 0) => {
      const { x, y } = at(name);
      return animate("[data-c=cursor]", { x: x - 3, y: y - 2 + shift }, { duration, ease: [0.45, 0, 0.2, 1] });
    };
    const click = async () => {
      animate("[data-c=ring]", { scale: [0.3, 1.6], opacity: [0.55, 0] }, { duration: 0.45 });
      await animate("[data-c=cursor]", { scale: 0.82 }, { duration: 0.08 });
      await animate("[data-c=cursor]", { scale: 1 }, { duration: 0.14 });
    };
    const run = async () => {
      while (alive) {
        if (document.visibilityState !== "visible") {
          await sleep(700);
          continue;
        }
        await Promise.all([
          animate("[data-c=sub]", { height: 0, opacity: 0 }, { duration: 0 }),
          animate("[data-c=pane]", { y: 0 }, { duration: 0 }),
          animate("[data-c=integrate]", { backgroundColor: "rgba(26,115,232,0)" }, { duration: 0 }),
          animate("[data-c=toast]", { opacity: 0, y: 6 }, { duration: 0 }),
          animate("[data-c=cursor]", { x: 250, y: 190, opacity: 0 }, { duration: 0 }),
        ]);
        await sleep(300);
        if (!alive) return;
        await animate("[data-c=cursor]", { opacity: 1 }, { duration: 0.25 });
        await move("calendar", 0.8);
        await click();
        await animate("[data-c=sub]", { height: "auto", opacity: 1 }, { duration: 0.3 });
        await move("integrate", 0.5);
        await click();
        animate("[data-c=integrate]", { backgroundColor: "rgba(26,115,232,.12)" }, { duration: 0.2 });
        await animate("[data-c=pane]", { y: -54 }, { duration: 0.7, ease: [0.45, 0, 0.2, 1] });
        await move("copy", 0.6, -54);
        await click();
        await animate("[data-c=toast]", { opacity: 1, y: 0 }, { duration: 0.25 });
        await animate("[data-c=cursor]", { opacity: 0 }, { duration: 0.5, delay: 0.6 });
        await sleep(1800);
      }
    };
    run();
    return () => {
      alive = false;
    };
  }, [animate, scope, reduce]);

  const row = (label: string, value: string, copy?: boolean) => (
    <div className="space-y-1">
      <div className="text-[9.5px] font-semibold" style={{ color: "#3c4043" }}>
        {label}
      </div>
      <div className="flex h-[22px] items-center gap-1 rounded-md px-1.5 text-[9px]" style={{ background: "#f1f3f4", color: "#5f6368" }}>
        <span className="flex-1 truncate">{value}</span>
        {copy && (
          <span data-c="copy" className="grid h-4 w-4 place-items-center rounded" style={{ color: "#1a73e8" }}>
            <CopyIcon size={11} />
          </span>
        )}
      </div>
    </div>
  );

  return (
    <div ref={scope} className="relative h-[232px] overflow-hidden rounded-[14px] bg-white" style={{ boxShadow: "0 0 0 1px rgba(30,27,24,.08), 0 14px 30px -20px rgba(30,27,24,.5)" }} aria-hidden>
      <div className="flex h-7 items-center gap-1.5 border-b border-line px-2.5">
        <img src={GOOGLE_CALENDAR_LOGO} alt="" width={13} height={13} />
        <span className="text-[10.5px] font-semibold" style={{ color: "#3c4043" }}>
          Settings
        </span>
      </div>
      <div className="flex h-[204px]">
        <div className="w-[118px] shrink-0 space-y-0.5 border-r border-line p-1.5 text-[9.5px]" style={{ color: "#3c4043" }}>
          <div className="rounded px-1.5 py-1">General</div>
          <div className="rounded px-1.5 py-1">Add calendar</div>
          <div className="px-1.5 pb-0.5 pt-1.5 text-[8.5px] font-semibold uppercase tracking-wide" style={{ color: "#5f6368" }}>
            Settings for my calendars
          </div>
          <div data-c="calendar" className="flex items-center gap-1 rounded px-1.5 py-1 font-semibold">
            <span className="h-2 w-2 rounded-sm" style={{ background: "#039be5" }} /> Your calendar
          </div>
          <div data-c="sub" className="overflow-hidden pl-3" style={{ height: reduce ? "auto" : 0, opacity: reduce ? 1 : 0 }}>
            <div className="rounded px-1.5 py-1">Calendar settings</div>
            <div className="rounded px-1.5 py-1">Share with people</div>
            <div data-c="integrate" className="rounded px-1.5 py-1 font-semibold" style={{ color: "#1a73e8" }}>
              Integrate calendar
            </div>
          </div>
        </div>
        <div className="relative flex-1 overflow-hidden p-2.5">
          <div data-c="pane" className="space-y-2.5" style={{ transform: reduce ? "translateY(-54px)" : undefined }}>
            <div className="text-[11px] font-semibold" style={{ color: "#3c4043" }}>
              Integrate calendar
            </div>
            {row("Calendar ID", "you@gmail.com")}
            {row("Public URL to this calendar", "calendar.google.com/…/embed")}
            {row("Public address in iCal format", "…/public/basic.ics")}
            <div className="rounded-md p-1" style={{ background: "rgba(255,224,166,.45)" }}>
              {row("Secret address in iCal format", "…/private-••••••/basic.ics", true)}
            </div>
          </div>
          <div data-c="toast" className="absolute bottom-2 left-2.5 right-2.5 rounded-md px-2 py-1.5 text-[9.5px] text-white" style={{ background: "#202124", opacity: reduce ? 1 : 0 }}>
            Copied to clipboard
          </div>
        </div>
      </div>
      <div data-c="cursor" className="pointer-events-none absolute left-0 top-0 z-10" style={{ opacity: 0 }}>
        <span data-c="ring" className="absolute -left-2 -top-2 h-5 w-5 rounded-full" style={{ background: "rgba(255,174,148,.7)", opacity: 0 }} />
        <svg width="15" height="15" viewBox="0 0 24 24" className="relative">
          <path d="M5 3l14 8-6.5 1.5L9 19z" fill="#1e1b18" stroke="#fff" strokeWidth="1.6" strokeLinejoin="round" />
        </svg>
      </div>
    </div>
  );
}
