// The tray widget's meeting cards: the pop-up shortly before a call, and the next meeting.
import { AnimatePresence, motion } from "motion/react";
import { features, setFeatures, useFeatures, type Meeting } from "../lib/features";
import { useApp } from "../lib/store";
import { clockTime, relative, useNow } from "../lib/time";
import { AppIcon, APP_NAMES } from "./AppIcons";
import { CloseIcon, VideoIcon } from "./Icons";
import { toast } from "./Toast";

const join = (meeting: Meeting) => features.calendarJoin(meeting.id).catch((e) => toast(String(e), "error"));

/** The announced meeting while its pop-up should show: until 5 minutes after it starts. */
export function useSoon() {
  const soon = useFeatures((s) => s.soon);
  const now = useNow(1000);
  return soon && now < soon.meeting.start + 5 * 60_000 && now < soon.meeting.end ? soon : null;
}

/** "Design review starts in 1 min", with Join. Shown when the calendar announces a call. */
export function MeetingSoon() {
  const soon = useSoon();
  const engine = useApp((s) => s.engine);
  const now = useNow(1000);
  const on = engine?.status === "on" || engine?.status === "starting";

  return (
    <AnimatePresence>
      {soon && (
        <motion.div
          initial={{ opacity: 0, y: -10, scale: 0.96 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={{ opacity: 0, y: -8, scale: 0.97 }}
          transition={{ type: "spring", stiffness: 380, damping: 28 }}
          className="relative mt-3 shrink-0 overflow-hidden rounded-[18px] p-3"
          style={{ background: "linear-gradient(135deg, rgba(255,196,176,.55), rgba(255,224,166,.5) 45%, rgba(201,242,212,.55))", boxShadow: "inset 0 0 0 1px rgba(255,255,255,.7)" }}
        >
          <button className="btn btn-ghost btn-icon absolute right-1.5 top-1.5 h-7 w-7" aria-label="Dismiss" onClick={() => setFeatures({ soon: null })}>
            <CloseIcon size={14} />
          </button>
          <div className="flex items-center gap-2.5 pr-6">
            {soon.meeting.app && <AppIcon app={soon.meeting.app} size={18} tile />}
            <div className="min-w-0">
              <div className="truncate text-[13.5px] font-[650]">{soon.meeting.title}</div>
              <div className="text-[11.5px] font-[560] text-ink-2">{relative(soon.meeting.start, soon.meeting.end, now)}</div>
            </div>
          </div>
          <div className="mt-2.5 flex items-center justify-between gap-2">
            <span className="chip h-6 bg-white/70 text-[11px]" style={{ color: on ? "var(--h-green)" : "var(--h-muted)" }}>
              <span className="dot" />
              {on ? (soon.turnedOn ? "Hush turned on" : "Hush is on") : "Hush is off"}
            </span>
            {soon.meeting.app && (
              <button className="btn btn-primary btn-sm h-8" onClick={() => join(soon.meeting)}>
                <VideoIcon size={14} /> Join {APP_NAMES[soon.meeting.app] === "Video call" ? "" : APP_NAMES[soon.meeting.app]}
              </button>
            )}
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

/** The next call today, as one row. */
export function NextUp() {
  const meetings = useFeatures((s) => s.calendar?.meetings);
  const soon = useFeatures((s) => s.soon);
  const now = useNow(30_000);
  const next = meetings?.find((m) => !m.allDay && m.end > now && m.start - now < 12 * 3600_000);
  if (!next || soon?.meeting.id === next.id) return null;
  return (
    <div className="relative mt-3 flex shrink-0 items-center gap-2.5 rounded-[18px] bg-white/70 px-3 py-2.5" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
      {next.app ? <AppIcon app={next.app} size={16} /> : <span className="h-2 w-2 rounded-full bg-faint" />}
      <div className="min-w-0 flex-1">
        <div className="truncate text-[12.5px] font-[600]">{next.title}</div>
        <div className="text-[11px] text-muted">
          {clockTime(next.start)} · {relative(next.start, next.end, now)}
        </div>
      </div>
      {next.app && (
        <button className="btn btn-soft btn-sm h-7 px-3 text-[12px]" onClick={() => join(next)}>
          Join
        </button>
      )}
    </div>
  );
}
