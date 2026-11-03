// Small illustrations that teach by showing: a meeting app's audio settings with
// "Hush AI Microphone" being chosen, and per-app hints for tooltips.
import { motion, useAnimate } from "motion/react";
import { useEffect } from "react";
import { MicIcon } from "./Icons";
import { Mark } from "./Mochi";

export const APPS = [
  { name: "Zoom", path: "Settings › Audio › Microphone" },
  { name: "Google Meet", path: "More options › Settings › Audio › Microphone" },
  { name: "Microsoft Teams", path: "Settings › Devices › Microphone" },
  { name: "Discord", path: "User Settings › Voice & Video › Input Device" },
  { name: "Slack", path: "Preferences › Audio & video › Microphone" },
  { name: "OBS Studio", path: "Settings › Audio › Mic/Auxiliary Audio" },
];

function Window({ title, children, width = 340 }: { title: string; children: React.ReactNode; width?: number }) {
  return (
    <div className="overflow-hidden rounded-[14px] bg-white text-left" style={{ width, boxShadow: "0 0 0 1px rgba(30,27,24,.08), 0 18px 40px -18px rgba(30,27,24,.35)" }}>
      <div className="flex h-8 items-center gap-1.5 border-b border-line px-3" style={{ background: "#f7f5f1" }}>
        <span className="h-2.5 w-2.5 rounded-full bg-[#ff8a7a]" />
        <span className="h-2.5 w-2.5 rounded-full bg-[#ffd27a]" />
        <span className="h-2.5 w-2.5 rounded-full bg-[#8fdc9f]" />
        <span className="ml-2 text-[11px] font-semibold text-muted">{title}</span>
      </div>
      {children}
    </div>
  );
}

/** A looping demo: open the microphone menu, pick Hush AI Microphone. */
export function ChooseMicDemo({ app = "Your meeting app" }: { app?: string }) {
  const [scope, animate] = useAnimate();

  useEffect(() => {
    let alive = true;
    const run = async () => {
      while (alive) {
        if (document.visibilityState !== "visible") {
          await new Promise((r) => setTimeout(r, 800));
          continue;
        }
        await animate("[data-menu]", { opacity: 0, y: -6, scale: 0.98 }, { duration: 0 });
        await animate("[data-picked]", { opacity: 0 }, { duration: 0 });
        await animate("[data-current]", { opacity: 1 }, { duration: 0 });
        await animate("[data-cursor]", { x: 250, y: 150, opacity: 0 }, { duration: 0 });
        await animate("[data-cursor]", { opacity: 1 }, { duration: 0.3 });
        await animate("[data-cursor]", { x: 230, y: 74 }, { duration: 0.9, ease: [0.4, 0, 0.2, 1] });
        await animate("[data-cursor]", { scale: 0.85 }, { duration: 0.1 });
        animate("[data-cursor]", { scale: 1 }, { duration: 0.15 });
        await animate("[data-menu]", { opacity: 1, y: 0, scale: 1 }, { duration: 0.28, ease: [0.2, 0.9, 0.1, 1] });
        await animate("[data-cursor]", { x: 150, y: 150 }, { duration: 0.8, ease: [0.4, 0, 0.2, 1] });
        await animate("[data-hush]", { backgroundColor: "rgba(255,224,166,.55)" }, { duration: 0.2 });
        await animate("[data-cursor]", { scale: 0.85 }, { duration: 0.1 });
        animate("[data-cursor]", { scale: 1 }, { duration: 0.15 });
        animate("[data-menu]", { opacity: 0, y: -6, scale: 0.98 }, { duration: 0.2 });
        await animate("[data-current]", { opacity: 0 }, { duration: 0.12 });
        await animate("[data-picked]", { opacity: 1 }, { duration: 0.25 });
        await animate("[data-hush]", { backgroundColor: "rgba(255,224,166,0)" }, { duration: 0 });
        await animate("[data-cursor]", { opacity: 0 }, { duration: 0.6, delay: 0.6 });
        await new Promise((r) => setTimeout(r, 1800));
      }
    };
    run();
    return () => {
      alive = false;
    };
  }, [animate]);

  return (
    <div ref={scope} className="relative">
      <Window title={`${app} · Audio settings`}>
        <div className="space-y-3 p-4">
          <div className="text-[11px] font-semibold uppercase tracking-wider text-muted">Microphone</div>
          <div className="relative flex h-9 items-center rounded-lg border border-line bg-white px-3 text-[12.5px]">
            <MicIcon size={14} className="mr-2 text-muted" />
            <span className="relative flex-1">
              <span data-current className="absolute inset-0 truncate text-ink-2">
                Built-in Microphone
              </span>
              <span data-picked className="absolute inset-0 flex items-center gap-1.5 font-semibold text-ink" style={{ opacity: 0 }}>
                <Mark size={14} /> Hush AI Microphone
              </span>
              &nbsp;
            </span>
            <span className="text-faint">▾</span>
          </div>
          <div className="flex items-center gap-2 text-[11px] text-muted">
            <span className="h-1.5 flex-1 rounded-full" style={{ background: "linear-gradient(90deg, #9fe8b8 55%, rgba(30,27,24,.08) 55%)" }} />
            Input level
          </div>
        </div>
      </Window>
      <div
        data-menu
        className="absolute left-4 right-4 top-[96px] overflow-hidden rounded-xl bg-white p-1 text-[12.5px]"
        style={{ opacity: 0, boxShadow: "0 0 0 1px rgba(30,27,24,.08), 0 16px 34px -14px rgba(30,27,24,.4)" }}
      >
        <div className="rounded-lg px-3 py-1.5 text-ink-2">Built-in Microphone</div>
        <div data-hush className="flex items-center gap-1.5 rounded-lg px-3 py-1.5 font-semibold text-ink">
          <Mark size={14} /> Hush AI Microphone
        </div>
        <div className="rounded-lg px-3 py-1.5 text-ink-2">Headset Microphone</div>
      </div>
      <motion.svg data-cursor width="18" height="18" viewBox="0 0 24 24" className="pointer-events-none absolute left-0 top-0" style={{ opacity: 0 }}>
        <path d="M5 3l14 8-6.5 1.5L9 19z" fill="#1e1b18" stroke="#fff" strokeWidth="1.5" strokeLinejoin="round" />
      </motion.svg>
    </div>
  );
}

/** Tooltip body: where to find the microphone setting in an app. */
export function AppHint({ name, path }: { name: string; path: string }) {
  return (
    <div className="w-[240px] space-y-2 py-1">
      <div className="text-[12px] font-semibold">{name}</div>
      <div className="rounded-xl bg-[#f7f5f1] p-2.5 text-[11.5px] leading-relaxed text-ink-2">{path}</div>
      <div className="flex items-center gap-1.5 rounded-lg border border-line bg-white px-2 py-1.5 text-[11.5px] font-semibold">
        <Mark size={13} /> Hush AI Microphone <span className="ml-auto text-green">✓</span>
      </div>
    </div>
  );
}
