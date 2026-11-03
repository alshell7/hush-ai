// The notch: Hush at the top of the screen, like the iPhone's Dynamic Island.
//
// - Idle: a sliver (or, on a MacBook, exactly the camera notch), so it costs nothing to look at.
// - Live (noise cancellation on in a call, a meeting about to start, or Live Translate on): a
//   slim island with the level on one side and what is happening on the other.
// - Open (hovered, or the tray icon clicked): the controls and both sides' live captions.
// The backend lets clicks through everywhere except the island, and says when the cursor is
// near it; the page reports the island's size as it morphs.
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { AppIcon } from "../components/AppIcons";
import { Spinner } from "../components/Controls";
import { ExternalIcon, MicIcon, PowerIcon, SpeakerIcon, TranslateIcon } from "../components/Icons";
import { useMeters, toDisplay } from "../components/Meters";
import { useSoon } from "../components/MeetingSoon";
import { Mochi } from "../components/Mochi";
import { tail, TALK_MODES, TranslateConfirm, useTranslateSwitch } from "../components/TranslateCard";
import { features, languageName, talking, TRANSLATE_SHORTCUT, useFeatures, type NotchGeometry, type Side } from "../lib/features";
import { api, isTauri, on } from "../lib/ipc";
import { saveSettings, useApp } from "../lib/store";
import { useNow } from "../lib/time";

type Mode = "idle" | "live" | "open";

const SPRING = { type: "spring", stiffness: 420, damping: 36, mass: 0.8 } as const;

export function Notch() {
  const settings = useApp((s) => s.settings);
  const engine = useApp((s) => s.engine);
  const translate = useFeatures((s) => s.translate);
  const listen = useFeatures((s) => s.listen);
  const meetings = useFeatures((s) => s.calendar?.meetings);
  const soon = useSoon();
  const now = useNow(15_000);
  const toggle = useTranslateSwitch();
  const [geometry, setGeometry] = useState<NotchGeometry>({ hasNotch: false, notchWidth: 0, notchHeight: 0, menuBar: 0 });
  const [hovered, setHovered] = useState(false);
  const [pinned, setPinned] = useState(false);
  const leave = useRef(0);
  const island = useRef<HTMLDivElement>(null);

  useEffect(() => {
    document.documentElement.classList.add("notch");
    features.notchGeometry().then(setGeometry).catch(() => undefined);
    const stops = [
      on("notch-geometry", setGeometry),
      on("notch-hover", (inside) => {
        clearTimeout(leave.current);
        if (inside) setHovered(true);
        // A moment's grace, so a cursor passing the edge does not make it flicker.
        else leave.current = window.setTimeout(() => setHovered(false), 420);
      }),
      on("notch-pin", () => setPinned((p) => !p)),
    ];
    const blur = () => setPinned(false);
    window.addEventListener("blur", blur);
    return () => {
      stops.forEach((s) => s.then((fn) => fn()));
      window.removeEventListener("blur", blur);
    };
  }, []);

  const noiseOn = !!settings?.enabled && engine?.status === "on";
  const inMeeting = meetings?.find((m) => !m.allDay && m.start - 2 * 60_000 <= now && now < m.end) ?? null;
  const callActive = !!inMeeting || !!listen?.active || talking(translate);
  const live = (noiseOn && callActive) || talking(translate) || !!soon;
  const mode: Mode = hovered || pinned || toggle.asking ? "open" : live ? "live" : "idle";

  // Report the island's rectangle as it morphs, so only it takes the mouse.
  useLayoutEffect(() => {
    const el = island.current;
    if (!el) return;
    let frame = 0;
    const report = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const r = el.getBoundingClientRect();
        features.notchRegion(r.x, r.y, r.width, r.height).catch(() => undefined);
      });
    };
    report();
    const observer = new ResizeObserver(report);
    observer.observe(el);
    return () => {
      observer.disconnect();
      cancelAnimationFrame(frame);
    };
  }, []);

  if (!settings || !engine) return null;

  const notch = geometry.hasNotch;
  const topHeight = notch ? geometry.notchHeight : 34;
  const width = mode === "open" ? 572 : mode === "live" ? (notch ? geometry.notchWidth + 230 : 330) : notch ? geometry.notchWidth : 132;
  const radius = mode === "open" ? 26 : mode === "live" ? 18 : notch ? 12 : 8;

  return (
    <div className="flex h-full w-full justify-center" onMouseEnter={() => !isTauri && setHovered(true)} onMouseLeave={() => !isTauri && setHovered(false)}>
      <motion.div
        ref={island}
        className="notch-rim relative h-fit"
        data-live={mode !== "idle" && live}
        initial={false}
        animate={{ width, borderBottomLeftRadius: radius + 1.5, borderBottomRightRadius: radius + 1.5 }}
        transition={SPRING}
        style={{ padding: mode === "idle" ? 0 : "0 1.5px 1.5px", boxShadow: mode === "open" ? "0 18px 50px rgba(0,0,0,.35)" : undefined }}
      >
        <motion.div layout transition={SPRING} className="notch-island overflow-hidden" style={{ borderBottomLeftRadius: radius, borderBottomRightRadius: radius }}>
          <AnimatePresence initial={false} mode="popLayout">
            {mode === "idle" && <motion.div key="idle" exit={{ opacity: 0 }} style={{ height: notch ? geometry.notchHeight : 9 }} />}
            {mode === "live" && (
              <Fade key="live">
                <Compact height={topHeight} gap={notch ? geometry.notchWidth : 0} noiseOn={noiseOn} meeting={inMeeting?.title ?? null} />
              </Fade>
            )}
            {mode === "open" && (
              <Fade key="open">
                <Open top={notch ? geometry.notchHeight : 0} toggle={toggle} noiseOn={noiseOn} onClose={() => setPinned(false)} />
              </Fade>
            )}
          </AnimatePresence>
        </motion.div>
      </motion.div>
    </div>
  );
}

function Fade({ children }: { children: ReactNode }) {
  return (
    <motion.div initial={{ opacity: 0, filter: "blur(4px)" }} animate={{ opacity: 1, filter: "blur(0px)" }} exit={{ opacity: 0, filter: "blur(4px)" }} transition={{ duration: 0.18 }}>
      {children}
    </motion.div>
  );
}

/** Five bars that follow the microphone (or their voice when you are quiet). */
function Bars({ count = 5, height = 14 }: { count?: number; height?: number }) {
  const bars = useRef<(HTMLSpanElement | null)[]>([]);
  const level = useRef(0);
  useMeters((m) => {
    level.current = Math.max(toDisplay(m.input), level.current * 0.7);
    bars.current.forEach((bar, i) => {
      if (!bar) return;
      const wobble = 0.55 + 0.45 * Math.abs(Math.sin(Date.now() / 140 + i * 1.7));
      bar.style.transform = `scaleY(${Math.max(0.18, level.current * wobble)})`;
    });
  });
  return (
    <span className="flex items-center gap-[2.5px]" style={{ height }} aria-hidden>
      {Array.from({ length: count }, (_, i) => (
        <span
          key={i}
          ref={(el) => {
            bars.current[i] = el;
          }}
          className="block w-[3px] origin-center rounded-full transition-transform duration-75"
          style={{ height, transform: "scaleY(.18)", background: "linear-gradient(180deg,#c9f2d4,#9be6e0)" }}
        />
      ))}
    </span>
  );
}

/** The live island: level on the left, what is happening on the right (around a camera notch). */
function Compact({ height, gap, noiseOn, meeting }: { height: number; gap: number; noiseOn: boolean; meeting: string | null }) {
  const settings = useApp((s) => s.settings)!;
  const translate = useFeatures((s) => s.translate);
  const soon = useSoon();
  const now = useNow(1000);
  const short = (code: string) => code.split("-")[0].toUpperCase();
  let right: ReactNode;
  if (soon && now < soon.meeting.start) {
    const minutes = Math.max(1, Math.ceil((soon.meeting.start - now) / 60_000));
    right = (
      <span className="flex items-center gap-1.5">
        {soon.meeting.app && <AppIcon app={soon.meeting.app} size={13} />}
        <span className="truncate">in {minutes} min</span>
      </span>
    );
  } else if (talking(translate)) {
    const connecting = translate!.me.status === "connecting" || translate!.them.status === "connecting";
    right = (
      <span className="flex items-center gap-1.5">
        {connecting ? <Spinner size={10} /> : <TranslateIcon size={13} className="text-[#c9f2d4]" />}
        {short(settings.translateFrom ?? "en")} ⇄ {short(settings.translateTo ?? "es")}
      </span>
    );
  } else {
    right = <span className="truncate">{meeting ?? (noiseOn ? "Hush on" : "Hush off")}</span>;
  }
  return (
    <div className="flex items-center justify-between px-3.5 text-[11.5px] font-[620]" style={{ height }}>
      <span className="flex items-center gap-2">
        <Mochi size={17} mood="happy" breathe={false} />
        {noiseOn && <Bars />}
      </span>
      <span style={{ width: gap }} />
      <span className="min-w-0 max-w-[150px] text-right">{right}</span>
    </div>
  );
}

function Chip({ active, onClick, icon, label, sub, busy }: { active: boolean; onClick: () => void; icon: ReactNode; label: string; sub: string; busy?: boolean }) {
  return (
    <button
      onClick={onClick}
      aria-pressed={active}
      className="flex min-w-0 flex-1 items-center gap-2.5 rounded-2xl px-3 py-2.5 text-left transition-colors"
      style={active ? { background: "linear-gradient(120deg,#ffc4b0,#ffe0a6 40%,#c9f2d4 75%,#9be6e0)", color: "#1e1b18" } : { background: "rgba(255,255,255,.07)" }}
    >
      <span className="shrink-0">{busy ? <Spinner size={16} /> : icon}</span>
      <span className="min-w-0">
        <span className="block truncate text-[12.5px] font-[650]">{label}</span>
        <span className={`block truncate text-[11px] ${active ? "text-[#1e1b18]/65" : "text-muted"}`}>{sub}</span>
      </span>
    </button>
  );
}

function Open({ top, toggle, noiseOn, onClose }: { top: number; toggle: ReturnType<typeof useTranslateSwitch>; noiseOn: boolean; onClose: () => void }) {
  const settings = useApp((s) => s.settings)!;
  const engine = useApp((s) => s.engine)!;
  const translate = useFeatures((s) => s.translate);
  const listen = useFeatures((s) => s.listen);
  const meetings = useFeatures((s) => s.calendar?.meetings);
  const soon = useSoon();
  const now = useNow(1000);
  const mode = TALK_MODES.find((m) => m.value === (settings.talkMode ?? "both"))!;
  const next = soon?.meeting ?? meetings?.find((m) => !m.allDay && m.app && m.end > now && m.start - now < 60 * 60_000) ?? null;
  const speaker = listen?.mode === "speaker";
  const cleaning = !!settings.listen && (settings.listenClean ?? "full") !== "off";

  return (
    <div className="space-y-3 px-4 pb-4" style={{ paddingTop: Math.max(top, 10) + 4 }}>
      <div className="flex items-center gap-2.5">
        <Mochi size={24} mood="happy" breathe={false} />
        <div className="min-w-0 flex-1">
          <div className="text-[13px] font-[650]">{noiseOn ? "Hush is on" : "Hush is off"}</div>
          <div className="truncate text-[11px] text-muted">{engine.status === "error" ? engine.message : talking(translate) ? `Live Translate · ${mode.label.toLowerCase()}` : `${TRANSLATE_SHORTCUT} translates in any app`}</div>
        </div>
        <button
          className="grid h-8 w-8 place-items-center rounded-xl text-muted transition-colors hover:bg-white/10 hover:text-white"
          title="Open Hush AI"
          aria-label="Open Hush AI"
          onClick={() => {
            onClose();
            api.openMain().catch(() => undefined);
          }}
        >
          <ExternalIcon size={15} />
        </button>
      </div>

      <div className="flex gap-2">
        <Chip active={noiseOn} onClick={() => api.setEnabled(!settings.enabled)} busy={settings.enabled && engine.status === "starting"} icon={<PowerIcon size={17} />} label="Noise cancellation" sub={noiseOn ? "Your mic is clean" : "Off"} />
        <Chip
          active={toggle.on}
          onClick={() => toggle.request(!toggle.on)}
          busy={translate?.me.status === "connecting" || translate?.them.status === "connecting"}
          icon={<TranslateIcon size={17} />}
          label="Translate"
          sub={toggle.on ? mode.label : `${languageName(settings.translateFrom ?? "en")} ⇄ ${languageName(settings.translateTo ?? "es")}`}
        />
        {speaker && <Chip active={cleaning} onClick={() => saveSettings({ listen: !cleaning, listenClean: settings.listenClean === "off" ? "full" : settings.listenClean })} icon={<SpeakerIcon size={17} />} label="Their voice" sub={cleaning ? (listen?.cleaning ? "Cleaning" : "Clean") : "As it is"} />}
      </div>

      <AnimatePresence initial={false}>{toggle.asking && <TranslateConfirm compact side={toggle.asking} onConfirm={toggle.confirm} onCancel={toggle.cancel} />}</AnimatePresence>

      {talking(translate) ? (
        <div className="space-y-2">
          <Caption side="them" />
          <Caption side="me" />
        </div>
      ) : next ? (
        <div className="flex items-center gap-2.5 rounded-2xl px-3 py-2.5" style={{ background: "rgba(255,255,255,.07)" }}>
          {next.app && <AppIcon app={next.app} size={18} />}
          <div className="min-w-0 flex-1">
            <div className="truncate text-[12.5px] font-[620]">{next.title}</div>
            <div className="text-[11px] text-muted">{next.start <= now ? "Now" : `In ${Math.max(1, Math.ceil((next.start - now) / 60_000))} min`}</div>
          </div>
          <button className="h-8 rounded-full bg-white px-3.5 text-[12px] font-[650] text-[#1e1b18]" onClick={() => features.calendarJoin(next.id).catch(() => undefined)}>
            Join
          </button>
        </div>
      ) : null}

      <div className="flex items-center gap-3 text-[10.5px] text-muted">
        <span className="flex items-center gap-1.5">
          <MicIcon size={12} /> {noiseOn ? <Bars count={7} height={10} /> : "off"}
        </span>
        {listen?.mode !== "none" && listen && (
          <span className="flex items-center gap-1.5">
            <SpeakerIcon size={12} /> {listen.active ? (speaker ? "Their voice is playing" : "Listening along") : speaker ? "Hush AI Speaker ready" : "Listening when needed"}
          </span>
        )}
      </div>
    </div>
  );
}

/** One side's caption, dark: the translation large for them, your words large for you. */
function Caption({ side }: { side: Side }) {
  const lane = useFeatures((s) => s.translate?.[side]);
  if (!lane?.on) return null;
  const main = side === "them" ? lane.translated : lane.original;
  const sub = side === "them" ? lane.original : lane.translated;
  return (
    <div className="rounded-2xl px-3 py-2.5" style={{ background: side === "them" ? "rgba(255,255,255,.09)" : "rgba(255,255,255,.04)" }}>
      <div className="mb-0.5 flex items-center gap-1.5 text-[10.5px] font-semibold text-muted">
        {lane.status === "connecting" ? <Spinner size={10} /> : <span className="h-1.5 w-1.5 rounded-full" style={{ background: lane.status === "error" ? "#ff8a7a" : "#9be6e0" }} />}
        {side === "them" ? "Them" : "You"}
        {lane.status === "error" && <span className="truncate text-[#ff8a7a]">· {lane.message}</span>}
      </div>
      <p className="text-[13.5px] font-[560] leading-snug">{tail(main, 120) || <span className="text-faint">{side === "them" ? "Their words show here in your language." : "Speak; they hear you in their language."}</span>}</p>
      {sub && <p className="mt-0.5 text-[11.5px] leading-snug text-muted">{tail(sub, 120)}</p>}
    </div>
  );
}
