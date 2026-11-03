// Google Calendar without signing in: the connect popup, step by step, the sync settings and
// the "meeting starting" pop-up. The steps advance on their own while the section is on screen.
import { useReducedMotion } from "motion/react";
import { useEffect, useRef, useState, type ReactNode } from "react";
import gcal from "../assets/apps/google-calendar.svg";
import { AppLogo, type AppId } from "../components/AppGuides";
import { CheckIcon, ExternalIcon, LockIcon, PauseIcon, PlayIcon } from "../components/Icons";
import { Mark, Mochi } from "../components/Mochi";
import { Reveal, SectionHeader, Segmented } from "../components/ui";
import { useActive, useLoop } from "../lib/hooks";

const STEPS = [
  { title: "Open Google Calendar settings", body: "One button in Hush opens the right page in your browser." },
  { title: "Copy the secret address", body: "Under your calendar, Integrate calendar › Secret address in iCal format." },
  { title: "Hush fills it in", body: "While the popup is open, Hush notices the copied address and pastes it for you." },
  { title: "Hush checks it", body: "It reads the calendar once and shows its name and how many meetings it found." },
  { title: "Connect", body: "Your next meetings appear with the right app and a Join button." },
];
const STEP_MS = 3800;

const MEETINGS: { title: string; time: string; app: AppId | null }[] = [
  { title: "Design review", time: "10:00", app: "meet" },
  { title: "Customer call", time: "11:30", app: "zoom" },
  { title: "Team stand-up", time: "14:00", app: "teams" },
];

function Popup({ children }: { children: ReactNode }) {
  return (
    <div className="mx-auto w-full max-w-[380px] rounded-[20px] bg-white p-4 sm:p-5" style={{ boxShadow: "0 0 0 1px rgba(30,27,24,.08), 0 24px 50px -24px rgba(30,27,24,.45)" }}>
      <div className="flex items-center gap-2.5">
        <img src={gcal} alt="" width={26} height={26} className="h-[26px] w-[26px]" />
        <div className="min-w-0">
          <div className="text-[14px] font-[650] tracking-[-0.01em]">Connect Google Calendar</div>
          <div className="text-[11.5px] text-muted">No sign-in. The address stays on this computer.</div>
        </div>
      </div>
      <div className="mt-4">{children}</div>
    </div>
  );
}

function Cursor({ className }: { className: string }) {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" className={`pointer-events-none absolute ${className}`}>
      <path d="M5 3l14 8-6.5 1.5L9 19z" fill="#1e1b18" stroke="#fff" strokeWidth="1.5" strokeLinejoin="round" />
    </svg>
  );
}

const ADDRESS = "https://calendar.google.com/calendar/ical/you%40gmail.com/private-3f9c…/basic.ics";

function Field({ filled, className = "" }: { filled: boolean; className?: string }) {
  return (
    <div className={`relative h-10 overflow-hidden rounded-[11px] px-3 text-[11.5px] leading-10 ${className}`} style={{ boxShadow: "inset 0 0 0 1px var(--h-line-2)" }}>
      <span className={`block truncate font-mono text-ink-2 ${filled ? "" : "text-faint"}`}>{filled ? ADDRESS : "Secret address in iCal format"}</span>
      {filled && <span className="cal-reveal absolute inset-y-0 left-0 right-0 bg-white" />}
    </div>
  );
}

const PANELS: ReactNode[] = [
  // 0. The button
  <Popup key="0">
    <p className="text-[12.5px] leading-snug text-ink-2">Hush reads your meetings from Google directly, from a private address only you have.</p>
    <div className="relative mt-4">
      <span className="cal-press flex h-10 items-center justify-center gap-2 rounded-full bg-ink text-[13px] font-[620] text-white">
        Open Google Calendar settings <ExternalIcon size={14} />
      </span>
      <Cursor className="cal-cursor-0 left-[60%] top-[22px]" />
    </div>
    <Field filled={false} className="mt-3" />
  </Popup>,
  // 1. Google Calendar's settings page
  <div key="1" className="mx-auto w-full max-w-[440px] overflow-hidden rounded-[16px] bg-white text-left" style={{ boxShadow: "0 0 0 1px rgba(30,27,24,.08), 0 24px 50px -24px rgba(30,27,24,.45)" }}>
    <div className="flex h-8 items-center gap-2 border-b border-line bg-[#f7f8fa] px-3 text-[10.5px] text-muted">
      <img src={gcal} alt="" width={14} height={14} className="h-[14px] w-[14px]" /> calendar.google.com › Settings
    </div>
    <div className="grid grid-cols-[110px_1fr] text-[11px] sm:grid-cols-[130px_1fr]">
      <div className="space-y-1 border-r border-line p-2.5 text-muted">
        <div className="font-[650] text-ink-2">Settings for my calendars</div>
        <div className="rounded-md bg-[#e8f0fe] px-2 py-1 font-[620] text-[#1a56c4]">Your calendar</div>
        <div className="pl-2">Calendar settings</div>
        <div className="pl-2">Notifications</div>
        <div className="cal-glow rounded-md pl-2 font-[620] text-ink">Integrate calendar</div>
      </div>
      <div className="relative p-3">
        <div className="text-[10px] font-[650] uppercase tracking-[0.06em] text-muted">Integrate calendar</div>
        <div className="mt-2 text-[10.5px] text-muted">Public address in iCal format</div>
        <div className="mt-1 h-6 rounded-md bg-[#f1f3f4]" />
        <div className="cal-glow mt-2.5 rounded-md text-[11px] font-[650] text-ink">Secret address in iCal format</div>
        <div className="mt-1 flex items-center gap-1.5">
          <div className="h-7 min-w-0 flex-1 truncate rounded-md bg-[#f1f3f4] px-2 font-mono text-[10px] leading-7 text-muted">••••••••••••••••••••</div>
          <span className="cal-copy grid h-7 w-7 shrink-0 place-items-center rounded-md text-[#1a56c4]" style={{ boxShadow: "inset 0 0 0 1px #c6d6f5" }}>
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <rect x="9" y="9" width="11" height="11" rx="2" />
              <path d="M5 15V5a2 2 0 0 1 2-2h10" />
            </svg>
          </span>
        </div>
        <span className="cal-toast absolute bottom-2 right-3 rounded-md bg-[#202124] px-2 py-1 text-[10px] text-white">Copied</span>
        <Cursor className="cal-cursor-1 right-[18px] top-[118px]" />
      </div>
    </div>
  </div>,
  // 2. Filled in from the clipboard
  <Popup key="2">
    <div className="text-[11px] font-[650] uppercase tracking-[0.06em] text-muted">Secret address</div>
    <Field filled className="mt-1.5" />
    <span className="cal-chip mt-2.5 inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[11.5px] font-[620] text-green" style={{ background: "rgba(31,157,114,.1)" }}>
      <CheckIcon size={13} strokeWidth={2.6} /> Found on your clipboard
    </span>
  </Popup>,
  // 3. Checked
  <Popup key="3">
    <div className="relative h-[86px]">
      <div className="cal-check-a absolute inset-0 flex items-center gap-3 rounded-[14px] px-3" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
        <span className="h-6 w-6 rounded-full" style={{ background: "var(--h-opal-conic)", animation: "h-spin 1s linear infinite", WebkitMask: "radial-gradient(farthest-side, transparent calc(100% - 3px), #000 calc(100% - 2.5px))", mask: "radial-gradient(farthest-side, transparent calc(100% - 3px), #000 calc(100% - 2.5px))" }} />
        <span className="text-[13px] font-[600] text-ink-2">Checking the address…</span>
      </div>
      <div className="cal-check-b absolute inset-0 flex items-center gap-3 rounded-[14px] px-3" style={{ background: "linear-gradient(100deg, rgba(201,242,212,.6), rgba(155,230,224,.3))" }}>
        <span className="grid h-8 w-8 place-items-center rounded-full bg-green text-white">
          <CheckIcon size={16} strokeWidth={2.6} />
        </span>
        <span>
          <span className="block text-[14px] font-[650]">Work</span>
          <span className="block text-[12px] text-ink-2">12 meetings in the next week</span>
        </span>
      </div>
    </div>
    <span className="mt-3 flex h-10 items-center justify-center rounded-full bg-ink text-[13px] font-[620] text-white">Connect</span>
  </Popup>,
  // 4. Connected
  <Popup key="4">
    <div className="flex items-center justify-between text-[11px] font-[650] uppercase tracking-[0.06em] text-muted">
      Today
      <span className="chip chip-green h-6 text-[11px] normal-case tracking-normal">
        <span className="dot" /> Connected
      </span>
    </div>
    <ul className="mt-2 space-y-1.5">
      {MEETINGS.map((m, i) => (
        <li key={m.title} className="cal-row flex items-center gap-2.5 rounded-[12px] bg-[#faf8f5] px-2.5 py-2" style={{ animationDelay: `${i * 0.12}s` }}>
          {m.app && <AppLogo app={m.app} size={18} />}
          <span className="min-w-0 flex-1">
            <span className="block truncate text-[12.5px] font-[620]">{m.title}</span>
            <span className="block text-[11px] text-muted">{m.time}</span>
          </span>
          <span className="rounded-full bg-ink px-2.5 py-1 text-[11px] font-[620] text-white">Join</span>
        </li>
      ))}
    </ul>
  </Popup>,
];

function Flow() {
  const stageRef = useRef<HTMLDivElement>(null);
  const visible = useActive(stageRef);
  const [step, setStep] = useState(0);
  const [auto, setAuto] = useState(true);
  const reduce = useReducedMotion();
  // With reduced motion the walkthrough waits for a click.
  useEffect(() => {
    if (reduce) setAuto(false);
  }, [reduce]);

  useEffect(() => {
    if (!visible || !auto) return;
    const t = window.setTimeout(() => setStep((s) => (s + 1) % STEPS.length), STEP_MS);
    return () => clearTimeout(t);
  }, [visible, auto, step]);

  return (
    <div className="grid grid-cols-[minmax(0,1fr)] gap-6 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.15fr)] lg:items-center">
      <ol className="space-y-1.5" aria-label="Connect Google Calendar, step by step">
        {STEPS.map((s, i) => {
          const on = i === step;
          return (
            <li key={s.title}>
              <button
                type="button"
                aria-current={on ? "step" : undefined}
                onClick={() => {
                  setStep(i);
                  setAuto(false);
                }}
                className="relative flex w-full cursor-pointer items-start gap-3.5 overflow-hidden rounded-[18px] p-3.5 text-left transition-[background-color,box-shadow] duration-300"
                style={{ background: on ? "#fff" : "transparent", boxShadow: on ? "inset 0 0 0 1px rgba(30,27,24,.08), 0 14px 30px -20px rgba(30,27,24,.45)" : "none" }}
              >
                <span
                  className="grid h-8 w-8 shrink-0 place-items-center rounded-full text-[13px] font-[650] transition-colors duration-300"
                  style={{ background: on ? "var(--h-ink)" : "rgba(30,27,24,.06)", color: on ? "#fff" : "var(--h-ink-2)" }}
                >
                  {i + 1}
                </span>
                <span className="min-w-0">
                  <span className="block text-[15.5px] font-[630] tracking-[-0.01em]">{s.title}</span>
                  <span className={`block text-[14px] leading-snug transition-colors duration-300 ${on ? "text-ink-2" : "text-muted"}`}>{s.body}</span>
                </span>
                {on && auto && visible && (
                  <span aria-hidden className="cal-timer absolute bottom-0 left-0 h-[2px] w-full origin-left" style={{ background: "var(--h-opal-strong)", animationDuration: `${STEP_MS}ms` }} key={step} />
                )}
              </button>
            </li>
          );
        })}
        <li className="pl-3.5 pt-1">
          <button type="button" onClick={() => setAuto((a) => !a)} className="btn btn-ghost btn-sm -ml-3 gap-1.5 text-[13px]">
            {auto ? <PauseIcon size={13} /> : <PlayIcon size={13} />}
            {auto ? "Pause the walkthrough" : "Play the walkthrough"}
          </button>
        </li>
      </ol>

      <div
        ref={stageRef}
        aria-hidden
        className="relative order-first h-[330px] overflow-hidden lg:order-none rounded-[28px] px-3 sm:h-[340px] sm:px-6"
        style={{ background: "linear-gradient(150deg, #fff1ea, #fff7e6 40%, #eefaf0 75%, #e5f7f5)", boxShadow: "var(--h-inner), var(--h-shadow-2)" }}
      >
        {PANELS.map((panel, i) => (
          <div key={i} className={`cal-panel absolute inset-0 grid place-items-center px-3 sm:px-6 ${i === step ? "is-on" : ""}`}>
            {panel}
          </div>
        ))}
        <div className="absolute bottom-3 left-1/2 flex -translate-x-1/2 gap-1.5">
          {STEPS.map((s, i) => (
            <span
              key={s.title}
              className="h-1.5 w-[18px] rounded-full transition-[transform,background-color] duration-300"
              style={{ transform: i === step ? "none" : "scaleX(.34)", background: i === step ? "var(--h-ink)" : "rgba(30,27,24,.18)" }}
            />
          ))}
        </div>
      </div>
    </div>
  );
}

function SyncSettings() {
  const [every, setEvery] = useState("5");
  const [before, setBefore] = useState("1");
  const [after, setAfter] = useState(true);
  return (
    <div className="card p-5 sm:p-6">
      <h3 className="text-[18px] font-[650] tracking-[-0.02em]">Sync settings</h3>
      <p className="mt-1 text-[14px] text-muted">Try them: this is how they look in Hush.</p>
      <div className="mt-5 space-y-5">
        <div>
          <div className="mb-2 text-[13.5px] font-[600] text-ink-2">Read the calendar every</div>
          <Segmented
            label="Read the calendar every"
            value={every}
            onChange={setEvery}
            options={["5", "15", "30", "60"].map((v) => ({ value: v, label: `${v} min` }))}
          />
        </div>
        <div>
          <div className="mb-2 text-[13.5px] font-[600] text-ink-2">Turn Hush on before meetings with a call link</div>
          <Segmented label="Turn Hush on before meetings" value={before} onChange={setBefore} options={["1", "2", "5"].map((v) => ({ value: v, label: `${v} min` }))} />
        </div>
        <button
          type="button"
          role="switch"
          aria-checked={after}
          onClick={() => setAfter((v) => !v)}
          className="flex w-full cursor-pointer items-center gap-4 rounded-[16px] p-3 text-left transition-colors hover:bg-black/[.03]"
          style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}
        >
          <span className="min-w-0 flex-1">
            <span className="block text-[14.5px] font-[620]">Turn it off afterwards</span>
            <span className="block text-[13px] text-muted">Only if Hush turned it on for the meeting.</span>
          </span>
          <span className="relative block h-[26px] w-[44px] shrink-0 rounded-full transition-[background] duration-300" style={{ background: after ? "var(--h-opal-strong)" : "rgba(30,27,24,.14)" }}>
            <span className="absolute left-[3px] top-[3px] h-5 w-5 rounded-full bg-white transition-transform duration-300" style={{ transform: after ? "translateX(18px)" : "none", boxShadow: "0 2px 6px rgba(30,27,24,.25)" }} />
          </span>
        </button>
      </div>
    </div>
  );
}

function MeetingPopup() {
  const { ref } = useLoop<HTMLDivElement>();
  return (
    <div ref={ref} aria-hidden className="relative h-[200px] overflow-hidden rounded-[28px] p-4" style={{ background: "linear-gradient(160deg,#f3f7f6,#fbf6ee)", boxShadow: "var(--h-inner), var(--h-shadow-2)" }}>
      <div className="flex h-7 items-center justify-end gap-3 rounded-[10px] bg-white/80 px-3 text-[10.5px] font-[600] text-muted" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
        <span className="grid h-5 w-5 place-items-center rounded-[6px]" style={{ background: "rgba(255,224,166,.7)" }}>
          <Mark size={13} />
        </span>
        <span>9:59</span>
      </div>
      <div className="meet-pop absolute right-4 top-14 w-[min(300px,calc(100%-32px))] rounded-[18px] bg-white p-3.5" style={{ boxShadow: "0 0 0 1px rgba(30,27,24,.08), 0 24px 44px -22px rgba(30,27,24,.5)" }}>
        <div className="flex items-center gap-2.5">
          <Mochi size={34} mood="awake" breathe={false} />
          <div className="min-w-0 flex-1">
            <div className="text-[11px] font-[650] uppercase tracking-[0.06em] text-coral">Meeting starting</div>
            <div className="truncate text-[14px] font-[650]">Design review · 10:00</div>
          </div>
        </div>
        <div className="mt-3 flex items-center gap-2">
          <span className="chip chip-green h-7 text-[11.5px]">
            <span className="dot" /> Hush is on
          </span>
          <span className="ml-auto flex items-center gap-1.5 rounded-full bg-ink px-3.5 py-1.5 text-[12px] font-[620] text-white">
            <AppLogo app="meet" size={14} /> Join
          </span>
        </div>
      </div>
    </div>
  );
}

export function Calendar() {
  return (
    <section id="calendar" aria-labelledby="calendar-title" className="relative py-20 sm:py-28">
      <div className="wrap">
        <SectionHeader
          id="calendar-title"
          eyebrow="New · Google Calendar"
          title={
            <>
              Ready before <span className="opal-text">the meeting starts</span>
            </>
          }
          sub="Connect Google Calendar without signing in. Hush shows your next meetings with a Join button, and turns noise cancellation on just before a call."
        />
        <Reveal className="mt-14">
          <Flow />
        </Reveal>
        <div className="mt-6 grid gap-4 md:grid-cols-2">
          <Reveal className="flex">
            <div className="w-full">
              <SyncSettings />
            </div>
          </Reveal>
          <Reveal delay={0.08} className="flex">
            <div className="flex w-full flex-col gap-4">
              <MeetingPopup />
              <div className="flex items-start gap-3 rounded-[20px] px-5 py-4 text-[14.5px] leading-relaxed text-ink-2" style={{ background: "rgba(30,27,24,.04)" }}>
                <LockIcon size={18} className="mt-0.5 shrink-0 text-green" />
                <p>
                  <strong className="font-[640] text-ink">Private by design.</strong> The secret address stays on your computer. Hush reads it straight from Google:
                  there is no Hush server and no Google sign-in.
                </p>
              </div>
            </div>
          </Reveal>
        </div>
      </div>
    </section>
  );
}
