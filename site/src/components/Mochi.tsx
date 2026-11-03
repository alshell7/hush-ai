// Copied from app/src/components/Mochi.tsx so the site builds on its own.
// The Hush mascot. Every feature is a quadratic path, so moods morph smoothly:
// a dot is a very short path with a thick round stroke.
import { m } from "motion/react";
import { useId } from "react";

export type Mood = "happy" | "sleep" | "awake" | "oops";

const EYES: Record<Mood, { left: string; right: string; width: number }> = {
  happy: { left: "M19 38.5Q23 33.5 27 38.5", right: "M37 38.5Q41 33.5 45 38.5", width: 3.4 },
  sleep: { left: "M19.5 38Q23 39.2 26.5 38", right: "M37.5 38Q41 39.2 44.5 38", width: 3 },
  awake: { left: "M22.6 37Q23 36.6 23.4 37", right: "M40.6 37Q41 36.6 41.4 37", width: 5.2 },
  oops: { left: "M22.6 36.4Q23 36 23.4 36.4", right: "M40.6 36.4Q41 36 41.4 36.4", width: 5.2 },
};

const MOUTH: Record<Mood, { d: string; width: number }> = {
  happy: { d: "M29.4 43.4Q32 46.4 34.6 43.4", width: 2.6 },
  sleep: { d: "M30.4 44.2Q32 44.8 33.6 44.2", width: 2.2 },
  awake: { d: "M29.8 43.4Q32 45.8 34.2 43.4", width: 2.4 },
  oops: { d: "M31.4 44.4Q32 43.6 32.6 44.4", width: 3.6 },
};

const spring = { type: "spring", stiffness: 260, damping: 22 } as const;

export function Mochi({
  size = 96,
  mood = "happy",
  tone = "pearl",
  breathe = true,
  className,
}: {
  size?: number;
  mood?: Mood;
  tone?: "pearl" | "ink";
  breathe?: boolean;
  className?: string;
}) {
  const id = useId().replace(/:/g, "");
  const ink = tone === "ink";
  const feature = ink ? "#ffffff" : "#231f1c";
  const eyes = EYES[mood];
  const mouth = MOUTH[mood];
  return (
    <svg
      viewBox="0 0 64 64"
      width={size}
      height={size}
      className={className}
      style={{ overflow: "visible" }}
      aria-hidden
    >
      <defs>
        <linearGradient id={`${id}-body`} x1="0.3" y1="0" x2="0.7" y2="1">
          <stop offset="0" stopColor={ink ? "#2b2622" : "#ffffff"} />
          <stop offset="1" stopColor={ink ? "#171412" : "#fff3ea"} />
        </linearGradient>
        <radialGradient id={`${id}-shadow`} cx="0.5" cy="0.5" r="0.5">
          <stop offset="0" stopColor="#5a3a22" stopOpacity="0.28" />
          <stop offset="1" stopColor="#5a3a22" stopOpacity="0" />
        </radialGradient>
      </defs>
      <ellipse cx="32" cy="57.5" rx="22" ry="3.2" fill={`url(#${id}-shadow)`} />
      <g
        style={{
          transformBox: "fill-box",
          transformOrigin: "50% 100%",
          animation: breathe ? "h-breathe 4.2s var(--h-ease-soft) infinite" : undefined,
        }}
      >
        <path
          d="M4 40C4 21 16.5 8 32 8C47.5 8 60 21 60 40C60 50.5 52 56 32 56C12 56 4 50.5 4 40Z"
          fill={`url(#${id}-body)`}
        />
        {!ink && (
          <path
            d="M10.5 26C13.5 19.5 19.5 15.2 26 14.6"
            fill="none"
            stroke="#fff"
            strokeWidth="2.2"
            strokeLinecap="round"
            opacity="0.95"
          />
        )}
        <ellipse cx="15.5" cy="43" rx="3.8" ry="2.2" fill="#ff9c8f" opacity={ink ? 0.85 : 0.5} />
        <ellipse cx="48.5" cy="43" rx="3.8" ry="2.2" fill="#ff9c8f" opacity={ink ? 0.85 : 0.5} />
        <g
          style={{
            transformBox: "fill-box",
            transformOrigin: "50% 50%",
            animation: mood === "awake" ? "h-blink 5.2s linear infinite" : undefined,
          }}
        >
          <m.path
            initial={false}
            animate={{ d: eyes.left, strokeWidth: eyes.width }}
            transition={spring}
            fill="none"
            stroke={feature}
            strokeLinecap="round"
          />
          <m.path
            initial={false}
            animate={{ d: eyes.right, strokeWidth: eyes.width }}
            transition={spring}
            fill="none"
            stroke={feature}
            strokeLinecap="round"
          />
        </g>
        <m.path
          initial={false}
          animate={{ d: mouth.d, strokeWidth: mouth.width }}
          transition={spring}
          fill="none"
          stroke={feature}
          strokeLinecap="round"
        />
      </g>
      {mood === "sleep" && (
        <g fill={ink ? "#1e1b18" : "#7b746b"} fontFamily="var(--font-sans)" fontWeight="700">
          <text x="50" y="14" fontSize="7" style={{ animation: "h-float-z 2.8s ease-out infinite" }}>
            z
          </text>
          <text x="55" y="9" fontSize="5" style={{ animation: "h-float-z 2.8s ease-out 1.4s infinite" }}>
            z
          </text>
        </g>
      )}
    </svg>
  );
}

/** The brand mark: ink mochi, happy. */
export function Mark({ size = 28 }: { size?: number }) {
  return <Mochi size={size} tone="ink" mood="happy" breathe={false} />;
}

export function Wordmark({ size = 28 }: { size?: number }) {
  return (
    <div className="flex items-center gap-2 select-none">
      <Mark size={size} />
      <span className="font-[650] tracking-[-0.035em] text-ink" style={{ fontSize: size * 0.72 }}>
        hush
      </span>
      <span
        className="rounded-full px-[7px] py-[1px] font-bold text-ink"
        style={{ background: "var(--h-opal)", fontSize: size * 0.36, letterSpacing: "0.02em" }}
      >
        AI
      </span>
    </div>
  );
}
