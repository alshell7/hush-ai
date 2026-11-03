// Meeting times in the user's own locale and time zone.
import { useEffect, useState } from "react";

const time = new Intl.DateTimeFormat(undefined, { hour: "numeric", minute: "2-digit" });
const weekday = new Intl.DateTimeFormat(undefined, { weekday: "long", day: "numeric", month: "short" });

export const clockTime = (ms: number) => time.format(ms);

export const timeRange = (start: number, end: number) => `${time.format(start)} – ${time.format(end)}`;

const startOfDay = (ms: number) => {
  const d = new Date(ms);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
};

export function dayLabel(ms: number, now = Date.now()) {
  const days = Math.round((startOfDay(ms) - startOfDay(now)) / 86_400_000);
  if (days === 0) return "Today";
  if (days === 1) return "Tomorrow";
  return weekday.format(ms);
}

/** "in 12 min", "in 2 h 5 min", "now", "ends in 20 min". */
export function relative(start: number, end: number, now = Date.now()) {
  if (now >= end) return "Ended";
  const span = (ms: number) => {
    const minutes = Math.max(1, Math.round(ms / 60_000));
    if (minutes < 60) return `${minutes} min`;
    const hours = Math.floor(minutes / 60);
    const rest = minutes % 60;
    return rest ? `${hours} h ${rest} min` : `${hours} h`;
  };
  if (now >= start) return `Now · ends in ${span(end - now)}`;
  if (start - now < 60_000) return "Starting now";
  return `in ${span(start - now)}`;
}

/** The current time, updated every `every` milliseconds while the page is visible. */
export function useNow(every = 15_000) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const tick = () => document.visibilityState === "visible" && setNow(Date.now());
    const timer = setInterval(tick, every);
    document.addEventListener("visibilitychange", tick);
    return () => {
      clearInterval(timer);
      document.removeEventListener("visibilitychange", tick);
    };
  }, [every]);
  return now;
}
