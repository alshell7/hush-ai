import { m } from "motion/react";
import { useRef, type KeyboardEvent, type ReactNode } from "react";

const ease = [0.2, 0.9, 0.1, 1] as const;

/** Fades content up once, the first time it scrolls into view. */
export function Reveal({ children, delay = 0, className }: { children: ReactNode; delay?: number; className?: string }) {
  return (
    <m.div
      className={className}
      initial={{ opacity: 0, y: 18 }}
      whileInView={{ opacity: 1, y: 0 }}
      viewport={{ once: true, margin: "0px 0px -8% 0px" }}
      transition={{ duration: 0.7, ease, delay }}
    >
      {children}
    </m.div>
  );
}

export function SectionHeader({
  id,
  eyebrow,
  title,
  sub,
  align = "center",
}: {
  id?: string;
  eyebrow: string;
  title: ReactNode;
  sub?: ReactNode;
  align?: "center" | "left";
}) {
  const center = align === "center";
  return (
    <Reveal className={center ? "mx-auto max-w-[760px] text-center" : "max-w-[620px]"}>
      <span className="eyebrow">{eyebrow}</span>
      <h2 id={id} className="heading mt-5 text-[34px] sm:text-[44px] lg:text-[54px]">
        {title}
      </h2>
      {sub && <p className={`lede mt-4 text-[17px] sm:text-[19px] ${center ? "mx-auto max-w-[620px]" : ""}`}>{sub}</p>}
    </Reveal>
  );
}

/** Pill radio group with a sliding white thumb, as in the app. */
export function Segmented<T extends string>({
  value,
  options,
  onChange,
  label,
  size = "md",
}: {
  value: T;
  options: { value: T; label: ReactNode }[];
  onChange: (value: T) => void;
  label: string;
  size?: "md" | "lg";
}) {
  const refs = useRef<(HTMLButtonElement | null)[]>([]);
  const height = size === "lg" ? 46 : 38;

  const onKey = (e: KeyboardEvent, index: number) => {
    const step = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const next = (index + step + options.length) % options.length;
    onChange(options[next].value);
    refs.current[next]?.focus();
  };

  return (
    <div role="radiogroup" aria-label={label} className="relative isolate flex rounded-full p-[4px]" style={{ background: "rgba(30,27,24,0.06)" }}>
      {/* The white thumb slides with a CSS transform; all options share the width. */}
      <span
        aria-hidden
        className="absolute bottom-[4px] left-[4px] top-[4px] -z-10 rounded-full bg-white"
        style={{
          width: `calc((100% - 8px) / ${options.length})`,
          transform: `translateX(${Math.max(0, options.findIndex((o) => o.value === value)) * 100}%)`,
          transition: "transform .38s var(--h-ease)",
          boxShadow: "0 1px 2px rgba(30,27,24,.08), 0 4px 12px -4px rgba(30,27,24,.16)",
        }}
      />
      {options.map((option, index) => {
        const active = option.value === value;
        return (
          <button
            key={option.value}
            ref={(el) => {
              refs.current[index] = el;
            }}
            type="button"
            role="radio"
            aria-checked={active}
            tabIndex={active ? 0 : -1}
            onClick={() => onChange(option.value)}
            onKeyDown={(e) => onKey(e, index)}
            className="relative flex flex-1 basis-0 cursor-pointer items-center justify-center gap-2 whitespace-nowrap rounded-full px-4 font-[580] transition-colors duration-200"
            style={{ height, fontSize: size === "lg" ? 15 : 14, color: active ? "var(--h-ink)" : "var(--h-muted)" }}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}

/** Small rounded square with an app's initial, tinted from the opal palette. */
export function AppMonogram({ name, tint, size = 28 }: { name: string; tint: string; size?: number }) {
  const letters = name === "OBS Studio" ? "OBS" : name === "Microsoft Teams" ? "T" : name === "Google Meet" ? "M" : name[0];
  return (
    <span
      aria-hidden
      className="grid shrink-0 place-items-center font-[680] text-ink"
      style={{
        width: size,
        height: size,
        borderRadius: size * 0.32,
        background: tint,
        fontSize: letters.length > 1 ? size * 0.3 : size * 0.46,
        letterSpacing: "-0.02em",
        boxShadow: "inset 0 1px 0 rgba(255,255,255,.7), inset 0 0 0 1px rgba(30,27,24,.06)",
      }}
    >
      {letters}
    </span>
  );
}
