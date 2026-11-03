// Shared controls: segmented picker, switch, device picker and tooltip.
import { Label, ListBox, Select, Tooltip } from "@heroui/react";
import { AnimatePresence, motion } from "motion/react";
import { useId, type ReactNode } from "react";

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  size = "md",
  label,
}: {
  value: T;
  options: { value: T; label: ReactNode; hint?: ReactNode }[];
  onChange: (value: T) => void;
  size?: "sm" | "md";
  label?: string;
}) {
  const id = useId();
  const height = size === "sm" ? 30 : 38;
  const hint = options.find((o) => o.value === value)?.hint;
  return (
    <div>
      <div role="radiogroup" aria-label={label} className="relative flex rounded-full p-[3px]" style={{ background: "rgba(30,27,24,0.06)" }}>
        {options.map((option) => {
          const active = option.value === value;
          return (
            <button
              key={option.value}
              role="radio"
              aria-checked={active}
              onClick={() => onChange(option.value)}
              className="relative z-0 flex-1 cursor-pointer whitespace-nowrap rounded-full px-3 font-[560] transition-colors duration-200"
              style={{ height, fontSize: size === "sm" ? 12 : 13.5, color: active ? "var(--h-ink)" : "var(--h-muted)" }}
            >
              {active && (
                <motion.span
                  layoutId={`seg-${id}`}
                  className="absolute inset-0 -z-10 rounded-full bg-white"
                  style={{ boxShadow: "0 1px 2px rgba(30,27,24,.08), 0 4px 12px -4px rgba(30,27,24,.16)" }}
                  transition={{ type: "spring", stiffness: 420, damping: 34 }}
                />
              )}
              {option.label}
            </button>
          );
        })}
      </div>
      {hint && (
        <AnimatePresence mode="wait" initial={false}>
          <motion.p
            key={String(value)}
            initial={{ opacity: 0, y: 3 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -3 }}
            transition={{ duration: 0.18 }}
            className="mt-2 px-1 text-[12.5px] leading-snug text-muted"
          >
            {hint}
          </motion.p>
        </AnimatePresence>
      )}
    </div>
  );
}

export function Toggle({
  on,
  onChange,
  label,
  size = "md",
  disabled,
}: {
  on: boolean;
  onChange: (on: boolean) => void;
  label: string;
  size?: "md" | "lg";
  disabled?: boolean;
}) {
  const w = size === "lg" ? 58 : 44;
  const h = size === "lg" ? 34 : 26;
  const knob = h - 6;
  return (
    <button
      role="switch"
      aria-checked={on}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!on)}
      className="relative shrink-0 cursor-pointer rounded-full transition-[background,box-shadow] duration-300 disabled:cursor-not-allowed disabled:opacity-50"
      style={{
        width: w,
        height: h,
        background: on ? "var(--h-opal-strong)" : "rgba(30,27,24,0.14)",
        boxShadow: on ? "inset 0 1px 0 rgba(255,255,255,.6), 0 6px 16px -8px rgba(232,115,90,.8)" : "inset 0 1px 2px rgba(30,27,24,.12)",
      }}
    >
      <motion.span
        className="absolute top-[3px] rounded-full bg-white"
        style={{ width: knob, height: knob, boxShadow: "0 2px 6px rgba(30,27,24,.25)" }}
        initial={false}
        animate={{ left: on ? w - knob - 3 : 3 }}
        whileTap={{ scaleX: 1.15 }}
        transition={{ type: "spring", stiffness: 500, damping: 32 }}
      />
    </button>
  );
}

export function DevicePicker({
  label,
  devices,
  value,
  onChange,
  icon,
}: {
  label: string;
  devices: { name: string | null; label: string }[];
  value: string | null;
  onChange: (name: string | null) => void;
  icon?: ReactNode;
}) {
  const key = (name: string | null) => name ?? "__default";
  const known = devices.some((d) => d.name === value);
  const items = known || value === null ? devices : [...devices, { name: value, label: `${value} (not connected)` }];
  return (
    <Select
      className="w-full"
      aria-label={label}
      value={key(value)}
      onChange={(next) => onChange(next === "__default" ? null : String(next))}
    >
      <Label className="sr-only">{label}</Label>
      <Select.Trigger className="h-11 rounded-2xl bg-white/80 pl-3">
        {icon && <span className="mr-2 text-muted">{icon}</span>}
        <Select.Value className="truncate text-[13.5px] font-[520]" />
        <Select.Indicator />
      </Select.Trigger>
      <Select.Popover className="rounded-2xl">
        <ListBox>
          {items.map((device) => (
            <ListBox.Item key={key(device.name)} id={key(device.name)} textValue={device.label}>
              {device.label}
              <ListBox.ItemIndicator />
            </ListBox.Item>
          ))}
        </ListBox>
      </Select.Popover>
    </Select>
  );
}

export function Tip({ content, children, placement = "top" }: { content: ReactNode; children: ReactNode; placement?: "top" | "bottom" | "left" | "right" }) {
  return (
    <Tooltip delay={350} closeDelay={80}>
      <Tooltip.Trigger>{children}</Tooltip.Trigger>
      <Tooltip.Content placement={placement} showArrow className="max-w-[300px] rounded-2xl px-3 py-2 text-[12.5px] leading-snug">
        <Tooltip.Arrow />
        {content}
      </Tooltip.Content>
    </Tooltip>
  );
}

export function Spinner({ size = 18 }: { size?: number }) {
  return (
    <span
      className="inline-block rounded-full"
      style={{
        width: size,
        height: size,
        border: "2px solid rgba(30,27,24,.15)",
        borderTopColor: "var(--h-ink)",
        animation: "h-spin .8s linear infinite",
      }}
    />
  );
}
