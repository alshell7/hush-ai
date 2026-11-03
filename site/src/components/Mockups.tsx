// Adapted from app/src/components/Mockups.tsx: a meeting app's audio settings with
// "Hush AI Microphone" being chosen. The loop only runs while it is on screen.
import { useAnimate, useReducedMotion } from "motion/react";
import { useEffect, useRef } from "react";
import { sleep } from "../lib/hooks";
import { MicIcon } from "./Icons";
import { Mark } from "./Mochi";

export const APPS = [
  { name: "Zoom", path: ["Settings", "Audio", "Microphone"], tint: "#9be6e0" },
  { name: "Google Meet", path: ["More options", "Settings", "Audio", "Microphone"], tint: "#c9f2d4" },
  { name: "Microsoft Teams", path: ["Settings", "Devices", "Microphone"], tint: "#ffc9da" },
  { name: "Discord", path: ["User Settings", "Voice & Video", "Input Device"], tint: "#ffe0a6" },
  { name: "Slack", path: ["Preferences", "Audio & video", "Microphone"], tint: "#ffc4b0" },
  { name: "OBS Studio", path: ["Settings", "Audio", "Mic/Auxiliary Audio"], tint: "#e6e1d8" },
] as const;

export function Window({ title, children, className = "" }: { title: string; children: React.ReactNode; className?: string }) {
  return (
    <div
      className={`overflow-hidden rounded-[14px] bg-white text-left ${className}`}
      style={{ boxShadow: "0 0 0 1px rgba(30,27,24,.08), 0 18px 40px -18px rgba(30,27,24,.35)" }}
    >
      <div className="flex h-8 items-center gap-1.5 border-b border-line px-3" style={{ background: "#f7f5f1" }}>
        <span className="h-2.5 w-2.5 shrink-0 rounded-full bg-[#ff8a7a]" />
        <span className="h-2.5 w-2.5 shrink-0 rounded-full bg-[#ffd27a]" />
        <span className="h-2.5 w-2.5 shrink-0 rounded-full bg-[#8fdc9f]" />
        <span className="ml-2 truncate text-[11px] font-semibold text-muted">{title}</span>
      </div>
      {children}
    </div>
  );
}

/** A looping demo: open the microphone menu, pick Hush AI Microphone. */
export function ChooseMicDemo({ app = "Your meeting app", active }: { app?: string; active: boolean }) {
  const [scope, animate] = useAnimate();
  const reduce = useReducedMotion();
  const activeRef = useRef(active);
  activeRef.current = active;

  useEffect(() => {
    if (reduce) {
      // Still picture: Hush AI Microphone already chosen.
      animate("[data-current]", { opacity: 0 }, { duration: 0 });
      animate("[data-picked]", { opacity: 1 }, { duration: 0 });
      return;
    }
    let alive = true;
    const run = async () => {
      while (alive) {
        if (!activeRef.current) {
          await sleep(500);
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
        await sleep(1800);
      }
    };
    run();
    return () => {
      alive = false;
    };
  }, [animate, reduce]);

  return (
    <div ref={scope} className="relative w-full max-w-[340px]" aria-hidden>
      <Window title={`${app} · Audio settings`}>
        <div className="space-y-3 p-4">
          <div className="text-[11px] font-semibold uppercase tracking-wider text-muted">Microphone</div>
          <div className="relative flex h-9 items-center rounded-lg border border-line bg-white px-3 text-[12.5px]">
            <MicIcon size={14} className="mr-2 shrink-0 text-muted" />
            <span className="relative min-w-0 flex-1">
              <span data-current className="absolute inset-0 truncate text-ink-2">
                Built-in Microphone
              </span>
              <span data-picked className="absolute inset-0 flex items-center gap-1.5 truncate font-semibold text-ink" style={{ opacity: 0 }}>
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
      <svg data-cursor width="18" height="18" viewBox="0 0 24 24" className="pointer-events-none absolute left-0 top-0" style={{ opacity: 0 }}>
        <path d="M5 3l14 8-6.5 1.5L9 19z" fill="#1e1b18" stroke="#fff" strokeWidth="1.5" strokeLinejoin="round" />
      </svg>
    </div>
  );
}
