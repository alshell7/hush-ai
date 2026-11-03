// Animated guides: a miniature of each meeting app where a cursor opens the audio settings
// and picks "Hush AI Microphone". They loop while visible and show the end state when the
// system asks for reduced motion.
import { Tooltip } from "@heroui/react";
import { motion, useAnimate, useReducedMotion } from "motion/react";
import { useEffect, useState, type ReactNode } from "react";
import type { AppId } from "../lib/features";
import { AppIcon, APP_NAMES } from "./AppIcons";
import { Mark } from "./Mochi";

type Opener = "gear" | "dots" | "more";

interface Theme {
  /** Window and call background. */
  bg: string;
  panel: string;
  line: string;
  text: string;
  muted: string;
  /** The app's own accent for selected items. */
  accent: string;
  tile: string;
}

interface Guide {
  app: Exclude<AppId, "other">;
  path: string[];
  opener: Opener;
  /** Where the first click is: a call toolbar, the window's header or a bottom-left user bar. */
  place: "toolbar" | "header" | "userbar";
  /** A menu between the opener and the settings, with the item to choose. */
  menu?: { items: string[]; pick: string };
  nav: string[];
  section: string;
  field: string;
  /** The call view and the settings use different themes in some apps (Meet). */
  call: Theme;
  settings: Theme;
}

const LIGHT: Omit<Theme, "accent"> = { bg: "#ffffff", panel: "#f6f5f3", line: "rgba(30,27,24,.1)", text: "#1e1b18", muted: "#8a837a", tile: "#eef0f3" };
const DARK: Omit<Theme, "accent"> = { bg: "#2b2d31", panel: "#232428", line: "rgba(255,255,255,.08)", text: "#f2f3f5", muted: "#949ba4", tile: "#383a40" };
const CALL_DARK: Omit<Theme, "accent"> = { bg: "#202124", panel: "#2d2e31", line: "rgba(255,255,255,.08)", text: "#e8eaed", muted: "#9aa0a6", tile: "#3c4043" };

export const GUIDES: Guide[] = [
  {
    app: "zoom",
    path: ["Settings", "Audio", "Microphone"],
    opener: "gear",
    place: "header",
    nav: ["General", "Video", "Audio", "Share screen", "Background"],
    section: "Audio",
    field: "Microphone",
    call: { ...LIGHT, accent: "#0b5cff" },
    settings: { ...LIGHT, accent: "#0b5cff" },
  },
  {
    app: "meet",
    path: ["More options", "Settings", "Audio", "Microphone"],
    opener: "dots",
    place: "toolbar",
    menu: { items: ["Change layout", "Full screen", "Settings"], pick: "Settings" },
    nav: ["Audio", "Video", "General", "Captions"],
    section: "Audio",
    field: "Microphone",
    call: { ...CALL_DARK, accent: "#8ab4f8" },
    settings: { ...LIGHT, accent: "#1a73e8" },
  },
  {
    app: "teams",
    path: ["Settings", "Devices", "Microphone"],
    opener: "more",
    place: "header",
    menu: { items: ["Settings", "Zoom", "Keyboard shortcuts"], pick: "Settings" },
    nav: ["General", "Accounts", "Privacy", "Notifications", "Devices"],
    section: "Devices",
    field: "Microphone",
    call: { ...LIGHT, panel: "#f5f5f5", accent: "#5b5fc7" },
    settings: { ...LIGHT, panel: "#f5f5f5", accent: "#5b5fc7" },
  },
  {
    app: "discord",
    path: ["User Settings", "Voice & Video", "Input Device"],
    opener: "gear",
    place: "userbar",
    nav: ["My Account", "Profiles", "Privacy & Safety", "Voice & Video", "Notifications"],
    section: "Voice & Video",
    field: "Input Device",
    call: { ...DARK, accent: "#5865f2" },
    settings: { ...DARK, bg: "#313338", accent: "#5865f2" },
  },
  {
    app: "slack",
    path: ["Preferences", "Audio & video", "Microphone"],
    opener: "gear",
    place: "header",
    nav: ["Notifications", "Navigation", "Appearance", "Audio & video", "Accessibility"],
    section: "Audio & video",
    field: "Microphone",
    call: { ...LIGHT, accent: "#1264a3" },
    settings: { ...LIGHT, accent: "#1264a3" },
  },
  {
    app: "webex",
    path: ["Settings", "Audio", "Microphone"],
    opener: "gear",
    place: "header",
    nav: ["General", "Audio", "Video", "Notifications"],
    section: "Audio",
    field: "Microphone",
    call: { ...CALL_DARK, bg: "#1b1b1b", accent: "#07c1e4" },
    settings: { ...LIGHT, accent: "#0a78cc" },
  },
  {
    app: "obs",
    path: ["Settings", "Audio", "Mic/Auxiliary Audio"],
    opener: "gear",
    place: "userbar",
    nav: ["General", "Stream", "Output", "Audio", "Video", "Hotkeys"],
    section: "Audio",
    field: "Mic/Auxiliary Audio",
    call: { ...DARK, bg: "#272a33", panel: "#1f2128", tile: "#3a3f4b", accent: "#476bd7" },
    settings: { ...DARK, bg: "#272a33", panel: "#1f2128", accent: "#476bd7" },
  },
];

export const guideFor = (app: AppId) => GUIDES.find((g) => g.app === app);

const W = 300;
const H = 178;
const BAR = 26;

function OpenerGlyph({ kind, color }: { kind: Opener; color: string }) {
  if (kind === "dots")
    return (
      <svg width="12" height="12" viewBox="0 0 24 24" fill={color}>
        <circle cx="12" cy="5" r="2.2" />
        <circle cx="12" cy="12" r="2.2" />
        <circle cx="12" cy="19" r="2.2" />
      </svg>
    );
  if (kind === "more")
    return (
      <svg width="12" height="12" viewBox="0 0 24 24" fill={color}>
        <circle cx="5" cy="12" r="2.2" />
        <circle cx="12" cy="12" r="2.2" />
        <circle cx="19" cy="12" r="2.2" />
      </svg>
    );
  return (
    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 13.5a7.7 7.7 0 0 0 0-3l2-1.5-2-3.4-2.3.9a7.6 7.6 0 0 0-2.6-1.5L14 2.5h-4l-.5 2.5A7.6 7.6 0 0 0 6.9 6.5l-2.3-.9-2 3.4 2 1.5a7.7 7.7 0 0 0 0 3l-2 1.5 2 3.4 2.3-.9a7.6 7.6 0 0 0 2.6 1.5l.5 2.5h4l.5-2.5a7.6 7.6 0 0 0 2.6-1.5l2.3.9 2-3.4-2-1.5Z" />
    </svg>
  );
}

/** Position of `el` inside `root`, ignoring transforms (tooltips scale while they open). */
function offsetIn(el: HTMLElement, root: HTMLElement) {
  let x = el.offsetWidth / 2;
  let y = el.offsetHeight / 2;
  let node: HTMLElement | null = el;
  while (node && node !== root) {
    x += node.offsetLeft;
    y += node.offsetTop;
    node = node.offsetParent as HTMLElement | null;
  }
  return { x, y };
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** A looping miniature of `app` where the cursor picks Hush AI Microphone. */
export function AppGuide({ app, scale = 1 }: { app: AppId; scale?: number }) {
  const guide = guideFor(app) ?? GUIDES[0];
  const reduce = useReducedMotion();
  const [scope, animate] = useAnimate<HTMLDivElement>();
  const { call, settings } = guide;

  useEffect(() => {
    const root = scope.current;
    if (!root || reduce) return;
    let alive = true;
    const q = (name: string) => root.querySelector<HTMLElement>(`[data-g="${name}"]`);
    const point = (name: string) => {
      const el = q(name);
      return el ? offsetIn(el, root) : { x: W / 2, y: H / 2 };
    };
    const move = (name: string, duration = 0.7) => {
      const { x, y } = point(name);
      return animate("[data-g=cursor]", { x: x - 3, y: y - 2 }, { duration, ease: [0.45, 0, 0.2, 1] });
    };
    const click = async () => {
      animate("[data-g=ring]", { scale: [0.3, 1.6], opacity: [0.55, 0] }, { duration: 0.45 });
      await animate("[data-g=cursor]", { scale: 0.82 }, { duration: 0.08 });
      await animate("[data-g=cursor]", { scale: 1 }, { duration: 0.14 });
    };
    const show = (name: string, on: boolean, duration = 0.28) =>
      animate(`[data-g=${name}]`, on ? { opacity: 1, scale: 1, y: 0 } : { opacity: 0, scale: 0.96, y: 4 }, { duration, ease: [0.2, 0.9, 0.1, 1] });

    const run = async () => {
      while (alive) {
        if (document.visibilityState !== "visible") {
          await sleep(700);
          continue;
        }
        // Reset to the app's main view.
        await Promise.all([
          show("menu", false, 0),
          show("settings", false, 0),
          show("dropdown", false, 0),
          animate("[data-g=picked]", { opacity: 0 }, { duration: 0 }),
          animate("[data-g=current]", { opacity: 1 }, { duration: 0 }),
          animate("[data-g=done]", { opacity: 0, scale: 0.6 }, { duration: 0 }),
          animate("[data-g=level]", { scaleX: 0.35 }, { duration: 0 }),
          animate("[data-g=navpill]", { y: 0, opacity: 0 }, { duration: 0 }),
          animate("[data-g=cursor]", { x: W - 40, y: H - 26, opacity: 0, scale: 1 }, { duration: 0 }),
        ]);
        await sleep(350);
        if (!alive) return;
        await animate("[data-g=cursor]", { opacity: 1 }, { duration: 0.25 });
        await move("opener", 0.85);
        await click();
        if (guide.menu) {
          await show("menu", true, 0.22);
          await move("menu-pick", 0.55);
          await animate("[data-g=menu-pick]", { backgroundColor: "rgba(127,127,127,.18)" }, { duration: 0.12 });
          await click();
          await show("menu", false, 0.16);
          animate("[data-g=menu-pick]", { backgroundColor: "rgba(127,127,127,0)" }, { duration: 0 });
        }
        await show("settings", true, 0.34);
        await sleep(200);
        await move("nav-pick", 0.6);
        await click();
        const navTop = q("nav-pick")?.offsetTop ?? 0;
        await animate("[data-g=navpill]", { y: navTop, opacity: 1 }, { type: "spring", stiffness: 420, damping: 32 });
        await sleep(160);
        await move("select", 0.6);
        await click();
        await show("dropdown", true, 0.22);
        await move("hush", 0.5);
        await animate("[data-g=hush]", { backgroundColor: "rgba(255,210,122,.4)" }, { duration: 0.15 });
        await click();
        await show("dropdown", false, 0.18);
        animate("[data-g=hush]", { backgroundColor: "rgba(255,210,122,0)" }, { duration: 0 });
        await animate("[data-g=current]", { opacity: 0 }, { duration: 0.1 });
        await animate("[data-g=picked]", { opacity: 1 }, { duration: 0.22 });
        animate("[data-g=done]", { opacity: 1, scale: 1 }, { type: "spring", stiffness: 500, damping: 22 });
        animate("[data-g=level]", { scaleX: [0.35, 0.8, 0.5, 0.92, 0.6, 0.85] }, { duration: 1.8, ease: "easeInOut" });
        await animate("[data-g=cursor]", { opacity: 0 }, { duration: 0.5, delay: 0.5 });
        await sleep(2200);
      }
    };
    run();
    return () => {
      alive = false;
    };
  }, [animate, scope, reduce, guide]);

  const navIndex = guide.nav.indexOf(guide.section);
  // With reduced motion the guide shows its end state.
  const end = !!reduce;
  const ink = settings.text;

  return (
    <div style={{ width: W * scale, height: H * scale }}>
      <div ref={scope} className="relative overflow-hidden text-left" style={{ width: W, height: H, transform: `scale(${scale})`, transformOrigin: "0 0", borderRadius: 12, background: call.bg, boxShadow: `0 0 0 1px ${call.line}` }}>
        {/* Title bar */}
        <div className="flex items-center gap-1.5 px-2.5" style={{ height: BAR, background: call.panel, borderBottom: `1px solid ${call.line}` }}>
          <AppIcon app={guide.app} size={13} />
          <span className="text-[10.5px] font-semibold" style={{ color: call.text }}>
            {APP_NAMES[guide.app]}
          </span>
          <span className="flex-1" />
          {guide.place === "header" && (
            <span data-g="opener" className="grid h-[18px] w-[18px] place-items-center rounded-md" style={{ background: "rgba(127,127,127,.12)" }}>
              <OpenerGlyph kind={guide.opener} color={call.muted} />
            </span>
          )}
        </div>

        {/* Main view: a call with two people */}
        <div className="absolute inset-x-0 bottom-0 flex flex-col" style={{ top: BAR }}>
          <div className="grid flex-1 grid-cols-2 gap-1.5 p-2">
            {["#ffd5c2", "#c9f2d4"].map((color, i) => (
              <div key={color} className="relative grid place-items-center rounded-lg" style={{ background: call.tile }}>
                <span className="h-7 w-7 rounded-full" style={{ background: color }} />
                <span className="absolute bottom-1 left-1.5 h-1.5 rounded-full" style={{ width: i ? 26 : 34, background: call.muted, opacity: 0.5 }} />
              </div>
            ))}
          </div>
          <div className="flex h-8 items-center justify-center gap-1.5 pb-1.5">
            {guide.place === "userbar" ? (
              <div className="flex w-full items-center gap-1.5 px-2">
                <span className="h-4 w-4 rounded-full" style={{ background: "#ffd27a" }} />
                <span className="h-1.5 w-12 rounded-full" style={{ background: call.muted, opacity: 0.6 }} />
                <span className="flex-1" />
                <span data-g="opener" className="grid h-[18px] w-[18px] place-items-center rounded-md" style={{ background: "rgba(127,127,127,.14)" }}>
                  <OpenerGlyph kind={guide.opener} color={call.muted} />
                </span>
              </div>
            ) : (
              <>
                {[0, 1].map((i) => (
                  <span key={i} className="h-5 w-5 rounded-full" style={{ background: "rgba(127,127,127,.2)" }} />
                ))}
                {guide.place === "toolbar" && (
                  <span data-g="opener" className="grid h-5 w-5 place-items-center rounded-full" style={{ background: "rgba(127,127,127,.2)" }}>
                    <OpenerGlyph kind={guide.opener} color={call.text} />
                  </span>
                )}
                <span className="h-5 w-8 rounded-full" style={{ background: "#e5484d" }} />
              </>
            )}
          </div>
        </div>

        {/* Menu after the opener */}
        {guide.menu && (
          <div
            data-g="menu"
            className="absolute z-10 rounded-lg p-1 text-[10px]"
            style={{
              opacity: 0,
              right: guide.place === "header" ? 8 : undefined,
              top: guide.place === "header" ? BAR + 2 : undefined,
              left: guide.place === "toolbar" ? W / 2 - 40 : undefined,
              bottom: guide.place === "toolbar" ? 36 : undefined,
              width: 112,
              background: settings.bg,
              color: settings.text,
              boxShadow: `0 0 0 1px ${settings.line}, 0 10px 24px -10px rgba(0,0,0,.45)`,
            }}
          >
            {guide.menu.items.map((item) => (
              <div key={item} data-g={item === guide.menu!.pick ? "menu-pick" : undefined} className="rounded-md px-2 py-1">
                {item}
              </div>
            ))}
          </div>
        )}

        {/* Settings */}
        <div
          data-g="settings"
          className="absolute inset-2 z-20 flex overflow-hidden rounded-[10px]"
          style={{ top: BAR + 6, opacity: end ? 1 : 0, background: settings.bg, boxShadow: `0 0 0 1px ${settings.line}, 0 14px 30px -14px rgba(0,0,0,.5)` }}
        >
          <div className="w-[92px] shrink-0 p-1.5" style={{ background: settings.panel, borderRight: `1px solid ${settings.line}` }}>
            <div data-g="navlist" className="relative">
              <motion.span data-g="navpill" className="absolute inset-x-0 top-0 h-[19px] rounded-md" style={{ background: `${settings.accent}22`, opacity: end ? 1 : 0, y: end ? navIndex * 19 : 0 }} />
              {guide.nav.map((item, i) => (
                <div
                  key={item}
                  data-g={i === navIndex ? "nav-pick" : i === 0 ? "nav-0" : undefined}
                  className="relative flex h-[19px] items-center truncate rounded-md px-1.5 text-[9.5px]"
                  style={{ color: i === navIndex ? settings.text : settings.muted, fontWeight: i === navIndex ? 600 : 450 }}
                >
                  {item}
                </div>
              ))}
            </div>
          </div>
          <div className="relative flex-1 p-2.5">
            <div className="mb-2 text-[11px] font-semibold" style={{ color: ink }}>
              {guide.section}
            </div>
            <div className="mb-1 text-[9px] font-semibold uppercase tracking-wider" style={{ color: settings.muted }}>
              {guide.field}
            </div>
            <div data-g="select" className="relative flex h-[24px] items-center rounded-md px-2 text-[10px]" style={{ background: settings.panel, boxShadow: `inset 0 0 0 1px ${settings.line}`, color: ink }}>
              <span className="relative h-full flex-1">
                <span data-g="current" className="absolute inset-0 flex items-center truncate" style={{ opacity: end ? 0 : 1, color: settings.muted }}>
                  Built-in Microphone
                </span>
                <span data-g="picked" className="absolute inset-0 flex items-center gap-1 font-semibold" style={{ opacity: end ? 1 : 0 }}>
                  <Mark size={11} /> Hush AI Microphone
                </span>
              </span>
              <span style={{ color: settings.muted }}>▾</span>
              <span data-g="done" className="absolute -right-1.5 -top-1.5 grid h-3.5 w-3.5 place-items-center rounded-full text-[8px] font-bold text-white" style={{ background: "#1f9d72", opacity: end ? 1 : 0 }}>
                ✓
              </span>
            </div>
            <div className="mt-2.5 flex items-center gap-1.5">
              <span className="text-[8.5px]" style={{ color: settings.muted }}>
                Input level
              </span>
              <span className="h-1 flex-1 overflow-hidden rounded-full" style={{ background: settings.line }}>
                <span data-g="level" className="block h-full origin-left rounded-full" style={{ background: "linear-gradient(90deg,#9fe8b8,#6fd9d1)", transform: `scaleX(${end ? 0.8 : 0.35})` }} />
              </span>
            </div>
            <div
              data-g="dropdown"
              className="absolute left-2.5 right-2.5 z-10 rounded-md p-0.5 text-[10px]"
              style={{ top: 70, opacity: 0, background: settings.bg, color: ink, boxShadow: `0 0 0 1px ${settings.line}, 0 10px 22px -10px rgba(0,0,0,.45)` }}
            >
              <div className="rounded px-1.5 py-[3px]" style={{ color: settings.muted }}>
                Built-in Microphone
              </div>
              <div data-g="hush" className="flex items-center gap-1 rounded px-1.5 py-[3px] font-semibold">
                <Mark size={11} /> Hush AI Microphone
              </div>
              <div className="rounded px-1.5 py-[3px]" style={{ color: settings.muted }}>
                AirPods Pro
              </div>
            </div>
          </div>
        </div>

        {/* Cursor */}
        <motion.div data-g="cursor" className="pointer-events-none absolute left-0 top-0 z-30" style={{ opacity: 0 }}>
          <span data-g="ring" className="absolute -left-2 -top-2 h-5 w-5 rounded-full" style={{ background: "rgba(255,174,148,.7)", opacity: 0 }} />
          <svg width="15" height="15" viewBox="0 0 24 24" className="relative">
            <path d="M5 3l14 8-6.5 1.5L9 19z" fill="#1e1b18" stroke="#fff" strokeWidth="1.6" strokeLinejoin="round" />
          </svg>
        </motion.div>
      </div>
    </div>
  );
}

/** Where the setting is, as a breadcrumb. */
export function GuidePath({ app }: { app: AppId }) {
  const guide = guideFor(app);
  if (!guide) return null;
  return (
    <div className="flex flex-wrap items-center gap-1 text-[11.5px] text-ink-2">
      {guide.path.map((step, i) => (
        <span key={step} className="flex items-center gap-1">
          {i > 0 && <span className="text-faint">›</span>}
          <span className={i === guide.path.length - 1 ? "font-semibold text-ink" : undefined}>{step}</span>
        </span>
      ))}
    </div>
  );
}

/** Tooltip body: the app's name, the animated guide and the path. */
export function AppGuideCard({ app }: { app: AppId }) {
  return (
    <div className="w-[300px] space-y-2.5">
      <div className="flex items-center gap-2">
        <AppIcon app={app} size={16} />
        <span className="text-[12.5px] font-semibold">Choose Hush AI Microphone in {APP_NAMES[app]}</span>
      </div>
      <AppGuide app={app} />
      <GuidePath app={app} />
    </div>
  );
}

/** An app's logo that shows its guide on hover or focus. */
export function AppGuideTip({ app, children }: { app: AppId; children?: ReactNode }) {
  return (
    <Tooltip delay={200} closeDelay={120}>
      {/* The trigger is itself focusable (role=button), so its content is a plain span. */}
      <Tooltip.Trigger aria-label={`Where to choose Hush AI Microphone in ${APP_NAMES[app]}`} className="rounded-[12px] transition-transform duration-200 hover:-translate-y-0.5 focus-visible:-translate-y-0.5">
        <span className="block">{children ?? <AppIcon app={app} size={20} tile />}</span>
      </Tooltip.Trigger>
      <Tooltip.Content placement="bottom" showArrow className="rounded-[18px] bg-white p-3 text-ink" style={{ boxShadow: "var(--h-shadow-3)" }}>
        <Tooltip.Arrow />
        <AppGuideCard app={app} />
      </Tooltip.Content>
    </Tooltip>
  );
}

/** App tabs with one large guide, for onboarding. */
export function AppGuidePicker({ scale = 1.12 }: { scale?: number }) {
  const [app, setApp] = useState<AppId>("zoom");
  return (
    <div className="space-y-3">
      <div role="tablist" aria-label="Apps" className="flex flex-wrap gap-1.5">
        {GUIDES.map((g) => (
          <button
            key={g.app}
            role="tab"
            aria-selected={g.app === app}
            aria-label={APP_NAMES[g.app]}
            onClick={() => setApp(g.app)}
            className="relative grid h-10 w-10 place-items-center rounded-[13px] transition-colors"
            style={{ background: g.app === app ? "#fff" : "rgba(255,255,255,.5)" }}
          >
            {g.app === app && <motion.span layoutId="guide-tab" className="absolute inset-0 rounded-[13px]" style={{ boxShadow: "0 0 0 1.5px var(--h-ink), 0 6px 14px -8px rgba(30,27,24,.45)" }} transition={{ type: "spring", stiffness: 420, damping: 32 }} />}
            <AppIcon app={g.app} size={20} />
          </button>
        ))}
      </div>
      <motion.div key={app} initial={{ opacity: 0, y: 6, filter: "blur(4px)" }} animate={{ opacity: 1, y: 0, filter: "blur(0px)" }} transition={{ duration: 0.3 }}>
        <AppGuide app={app} scale={scale} />
        <div className="mt-2">
          <GuidePath app={app} />
        </div>
      </motion.div>
    </div>
  );
}
