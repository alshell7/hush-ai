// The main switch: the mascot inside a ring. Asleep when off, happy when on; it squishes
// a little with your voice. The opal ring turns slowly only while Hush is on.
import { motion, AnimatePresence } from "motion/react";
import { useRef } from "react";
import type { EngineStatus } from "../lib/ipc";
import { Mochi } from "./Mochi";
import { toDisplay, useMeters } from "./Meters";

export function PowerHero({
  status,
  onToggle,
  size = 168,
}: {
  status: EngineStatus;
  onToggle: () => void;
  size?: number;
}) {
  const on = status === "on";
  const starting = status === "starting";
  const error = status === "error";
  const squish = useRef<HTMLDivElement>(null);

  useMeters((m) => {
    if (!squish.current) return;
    const v = on ? toDisplay(m.output) : 0;
    squish.current.style.transform = `scale(${1 + v * 0.05}, ${1 - v * 0.035})`;
  });

  const mood = error ? "oops" : on ? "happy" : starting ? "awake" : "sleep";
  const inner = size - 22;

  return (
    <button
      onClick={onToggle}
      aria-label={on ? "Turn noise cancellation off" : "Turn noise cancellation on"}
      aria-pressed={on}
      className="group relative grid cursor-pointer place-items-center rounded-full outline-offset-4 transition-transform duration-200 active:scale-[0.97]"
      style={{ width: size, height: size }}
    >
      {/* Ring */}
      <span
        className="absolute inset-0 rounded-full transition-opacity duration-500"
        style={{
          background: "var(--h-opal-conic)",
          opacity: on || starting ? 1 : 0,
          animation: on ? "h-spin 9s linear infinite" : starting ? "h-spin 1.4s linear infinite" : undefined,
          WebkitMask: "radial-gradient(farthest-side, transparent calc(100% - 4px), #000 calc(100% - 3.5px))",
          mask: "radial-gradient(farthest-side, transparent calc(100% - 4px), #000 calc(100% - 3.5px))",
        }}
      />
      <span
        className="absolute inset-0 rounded-full border-2 border-dashed transition-opacity duration-500"
        style={{ borderColor: error ? "rgba(214,69,75,.45)" : "rgba(30,27,24,.16)", opacity: on || starting ? 0 : 1 }}
      />
      {/* Glow */}
      <span
        className="absolute rounded-full blur-2xl transition-opacity duration-700"
        style={{ inset: 14, background: "var(--h-opal-strong)", opacity: on ? 0.55 : 0 }}
      />
      {/* Disc */}
      <span
        className="relative grid place-items-center rounded-full transition-[background,box-shadow] duration-500"
        style={{
          width: inner,
          height: inner,
          background: on ? "var(--h-opal)" : "linear-gradient(180deg,#ffffff,#f4f1ec)",
          boxShadow: on
            ? "inset 0 2px 0 rgba(255,255,255,.8), inset 0 -10px 24px rgba(232,115,90,.12), 0 18px 40px -16px rgba(232,115,90,.55)"
            : "inset 0 2px 0 #fff, 0 14px 34px -18px rgba(30,27,24,.4)",
        }}
      >
        <div ref={squish} style={{ transition: "transform 90ms linear", transformOrigin: "50% 85%" }}>
          <Mochi size={inner * 0.62} mood={mood} tone="pearl" />
        </div>
      </span>
      {/* Ripple when turning on */}
      <AnimatePresence>
        {on && (
          <motion.span
            key="ripple"
            className="pointer-events-none absolute inset-0 rounded-full border-2"
            style={{ borderColor: "rgba(255,174,148,.7)" }}
            initial={{ scale: 1, opacity: 0.7 }}
            animate={{ scale: 1.35, opacity: 0 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.9, ease: "easeOut" }}
          />
        )}
      </AnimatePresence>
    </button>
  );
}
