// Meetings: the calendar's next meetings with Join buttons, and how Hush follows them.
import { AnimatePresence, motion } from "motion/react";
import { useState } from "react";
import { AppIcon, APP_NAMES, GOOGLE_CALENDAR_LOGO } from "../components/AppIcons";
import { CalendarConnect } from "../components/CalendarConnect";
import { Segmented, Spinner, Toggle } from "../components/Controls";
import { BellIcon, CalendarIcon, LockIcon, PowerIcon, RefreshIcon, SparkleIcon, VideoIcon } from "../components/Icons";
import { Mochi } from "../components/Mochi";
import { toast } from "../components/Toast";
import { CALENDAR_COLORS, features, useFeatures, type CalendarInfo, type CalendarView, type Meeting } from "../lib/features";
import { saveSettings, useApp } from "../lib/store";
import { clockTime, dayLabel, relative, timeRange, useNow } from "../lib/time";

export function Meetings() {
  const calendar = useFeatures((s) => s.calendar);
  const [connectOpen, setConnectOpen] = useState(false);

  return (
    <>
      {calendar?.connected ? <Agenda calendar={calendar} onReconnect={() => setConnectOpen(true)} /> : <Welcome onConnect={() => setConnectOpen(true)} />}
      <CalendarConnect open={connectOpen} adding={!!calendar?.connected} onClose={() => setConnectOpen(false)} />
    </>
  );
}

function Welcome({ onConnect }: { onConnect: () => void }) {
  return (
    <section className="glass grid min-h-0 flex-1 grid-cols-[1.05fr_1fr] items-center gap-8 p-8">
      <div className="space-y-5">
        <img src={GOOGLE_CALENDAR_LOGO} alt="" width={44} height={44} />
        <div>
          <div className="eyebrow">Meetings</div>
          <h1 className="mt-1 text-[27px] font-[660] leading-[1.1] tracking-[-0.035em]">
            Hush is ready <span className="opal-text">before your call is</span>
          </h1>
          <p className="mt-2 max-w-[380px] text-[13.5px] leading-relaxed text-ink-2">
            Connect Google Calendar and Hush turns on a minute before each call, pops up with a Join button, and steps back when the call ends.
          </p>
        </div>
        <ul className="space-y-2 text-[13px] text-ink-2">
          {[
            [PowerIcon, "Noise cancellation turns itself on before calls"],
            [BellIcon, "A gentle pop-up with Join, from the tray"],
            [LockIcon, "No Google sign-in; the address stays on this computer"],
          ].map(([Icon, text]) => {
            const I = Icon as typeof PowerIcon;
            return (
              <li key={text as string} className="flex items-center gap-2.5">
                <span className="grid h-7 w-7 place-items-center rounded-xl bg-white/80" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
                  <I size={15} />
                </span>
                {text as string}
              </li>
            );
          })}
        </ul>
        <button className="btn btn-primary h-11 px-6" onClick={onConnect}>
          <CalendarIcon size={17} /> Connect Google Calendar
        </button>
      </div>
      <AgendaPreview />
    </section>
  );
}

/** A sample of what the connected view looks like, gently animated. */
function AgendaPreview() {
  const rows: { app: Meeting["app"]; title: string; time: string }[] = [
    { app: "zoom", title: "Design review", time: "10:30" },
    { app: "meet", title: "Team standup", time: "12:00" },
    { app: "teams", title: "Customer call", time: "15:15" },
  ];
  return (
    <div className="relative" aria-hidden>
      <motion.div className="rounded-[22px] bg-white/85 p-4" style={{ boxShadow: "var(--h-shadow-2)" }} initial={{ rotate: -1.5, y: 8 }} animate={{ rotate: [-1.5, 0.5, -1.5], y: [8, 0, 8] }} transition={{ duration: 7, repeat: Infinity, ease: "easeInOut" }}>
        <div className="eyebrow mb-3">Today</div>
        <div className="space-y-2">
          {rows.map((row, i) => (
            <motion.div key={row.title} className="flex items-center gap-3 rounded-2xl bg-[#f7f5f1] px-3 py-2.5" initial={{ opacity: 0, x: 12 }} animate={{ opacity: 1, x: 0 }} transition={{ delay: 0.15 + i * 0.12 }}>
              <span className="w-10 text-[12px] font-semibold tabular-nums text-muted">{row.time}</span>
              {row.app && <AppIcon app={row.app} size={18} />}
              <span className="flex-1 truncate text-[13px] font-[560]">{row.title}</span>
              <span className="rounded-full bg-ink px-2.5 py-1 text-[11px] font-semibold text-white">Join</span>
            </motion.div>
          ))}
        </div>
      </motion.div>
      <motion.div
        className="absolute -bottom-7 -right-3 flex items-center gap-2.5 rounded-2xl bg-white px-3 py-2.5"
        style={{ boxShadow: "var(--h-shadow-3)" }}
        initial={{ opacity: 0, y: 10, scale: 0.95 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        transition={{ delay: 0.8, type: "spring", stiffness: 300, damping: 22 }}
      >
        <Mochi size={30} mood="happy" breathe={false} />
        <div className="text-[11.5px] leading-tight">
          <div className="font-semibold">Design review in 1 min</div>
          <div className="text-muted">Hush is on</div>
        </div>
      </motion.div>
    </div>
  );
}

function Agenda({ calendar, onReconnect }: { calendar: CalendarInfo; onReconnect: () => void }) {
  const now = useNow(15_000);
  const upcoming = calendar.meetings.filter((m) => m.end > now);
  const timed = upcoming.filter((m) => !m.allDay);
  const next = timed.find((m) => m.start > now - 60_000 || m.end > now);
  const groups = new Map<string, Meeting[]>();
  for (const m of upcoming) {
    if (m === next) continue;
    const key = dayLabel(m.start, now);
    groups.set(key, [...(groups.get(key) ?? []), m]);
  }

  return (
    <main className="grid min-h-0 flex-1 grid-cols-[1.25fr_1fr] gap-4">
      <section className="glass flex min-h-0 flex-col p-5">
        <div className="mb-3 flex items-center justify-between">
          <span className="eyebrow">Up next</span>
          <CalendarDots calendars={calendar.calendars.filter((c) => c.enabled)} />
        </div>
        {next ? <NextMeeting meeting={next} now={now} /> : <Empty />}
        <div className="scroll -mr-2 mt-4 min-h-0 flex-1 space-y-4 pr-2">
          {[...groups.entries()].map(([day, meetings]) => (
            <div key={day}>
              <div className="eyebrow mb-1.5 px-1">{day}</div>
              <ul className="space-y-1">
                {meetings.map((m) => (
                  <Row key={m.id} meeting={m} now={now} />
                ))}
              </ul>
            </div>
          ))}
        </div>
      </section>
      <SyncPanel calendar={calendar} onReconnect={onReconnect} />
    </main>
  );
}

function NextMeeting({ meeting, now }: { meeting: Meeting; now: number }) {
  const live = now >= meeting.start;
  const soon = !live && meeting.start - now < 10 * 60_000;
  return (
    <motion.div layout className="relative overflow-hidden rounded-[22px] bg-white/85 p-4" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
      {(live || soon) && <div className="pointer-events-none absolute -right-10 -top-16 h-40 w-40 rounded-full opacity-70 blur-2xl" style={{ background: "var(--h-opal)" }} />}
      <div className="relative flex items-start gap-3">
        {meeting.app ? <AppIcon app={meeting.app} size={22} tile /> : <span className="grid h-[38px] w-[38px] place-items-center rounded-xl bg-[#f4efe8]"><CalendarIcon size={19} /></span>}
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <div className="truncate text-[17px] font-[650] tracking-[-0.02em]">{meeting.title}</div>
            <Origin meeting={meeting} />
          </div>
          <div className="mt-0.5 text-[12.5px] text-muted">
            {dayLabel(meeting.start, now)} · {timeRange(meeting.start, meeting.end)}
            {meeting.app && ` · ${APP_NAMES[meeting.app]}`}
          </div>
        </div>
      </div>
      <div className="relative mt-3 flex items-center justify-between">
        <AnimatePresence mode="wait">
          <motion.span key={relative(meeting.start, meeting.end, now)} initial={{ opacity: 0, y: 4 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -4 }} className={`chip ${live ? "text-green" : soon ? "text-coral" : ""}`} style={live ? { background: "rgba(31,157,114,.1)" } : undefined}>
            {live && <span className="dot" />}
            {relative(meeting.start, meeting.end, now)}
          </motion.span>
        </AnimatePresence>
        {meeting.app && (
          <button className="btn btn-primary btn-sm" onClick={() => features.calendarJoin(meeting.id).catch((e) => toast(String(e), "error"))}>
            <VideoIcon size={15} /> Join
          </button>
        )}
      </div>
    </motion.div>
  );
}

/** The calendars a meeting is in, as small colour dots. */
function Origin({ meeting }: { meeting: Meeting }) {
  const calendars = useFeatures((s) => s.calendar?.calendars ?? []);
  if (calendars.length < 2) return null;
  const mine = calendars.filter((c) => meeting.calendars.includes(c.id));
  return (
    <span className="flex shrink-0 -space-x-1" title={mine.map((c) => c.name ?? c.source).join(" · ")}>
      {mine.map((c) => (
        <span key={c.id} className="h-2 w-2 rounded-full ring-2 ring-white" style={{ background: CALENDAR_COLORS[c.color % CALENDAR_COLORS.length] }} />
      ))}
    </span>
  );
}

function CalendarDots({ calendars }: { calendars: CalendarView[] }) {
  if (calendars.length === 1) {
    return (
      <span className="flex items-center gap-1.5 text-[12px] text-muted">
        <img src={GOOGLE_CALENDAR_LOGO} alt="" width={14} height={14} /> {calendars[0].name ?? calendars[0].source}
      </span>
    );
  }
  return (
    <span className="flex min-w-0 items-center gap-2.5 text-[11.5px] text-muted">
      {calendars.slice(0, 3).map((c) => (
        <span key={c.id} className="flex min-w-0 items-center gap-1">
          <span className="h-2 w-2 shrink-0 rounded-full" style={{ background: CALENDAR_COLORS[c.color % CALENDAR_COLORS.length] }} />
          <span className="max-w-[96px] truncate">{c.name ?? c.source}</span>
        </span>
      ))}
      {calendars.length > 3 && <span>+{calendars.length - 3}</span>}
    </span>
  );
}

function Row({ meeting, now }: { meeting: Meeting; now: number }) {
  return (
    <li className="group flex items-center gap-3 rounded-2xl px-2 py-2 transition-colors hover:bg-white/70">
      <span className="w-14 shrink-0 text-[12px] font-semibold tabular-nums text-muted">{meeting.allDay ? "All day" : clockTime(meeting.start)}</span>
      <span className="grid h-5 w-5 shrink-0 place-items-center">{meeting.app ? <AppIcon app={meeting.app} size={16} /> : <span className="h-1.5 w-1.5 rounded-full bg-faint" />}</span>
      <span className={`min-w-0 flex-1 truncate text-[13px] ${meeting.allDay ? "text-muted" : "font-[540]"}`}>{meeting.title}</span>
      <Origin meeting={meeting} />
      {meeting.app && (
        <button className="btn btn-soft btn-sm h-7 px-3 text-[12px] opacity-70 transition-opacity group-hover:opacity-100" onClick={() => features.calendarJoin(meeting.id).catch((e) => toast(String(e), "error"))}>
          Join
        </button>
      )}
      {!meeting.app && meeting.start - now < 0 && <span className="text-[11px] text-muted">now</span>}
    </li>
  );
}

function Empty() {
  return (
    <div className="flex items-center gap-4 rounded-[22px] bg-white/70 p-4" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
      <Mochi size={48} mood="sleep" />
      <div>
        <div className="text-[14.5px] font-semibold">Nothing in the next week</div>
        <div className="text-[12.5px] text-muted">Enjoy the quiet. New meetings show up here on their own.</div>
      </div>
    </div>
  );
}

function SyncPanel({ calendar, onReconnect }: { calendar: CalendarInfo; onReconnect: () => void }) {
  const settings = useApp((s) => s.settings)!;
  const now = useNow(30_000);
  const auto = settings.calendarAuto ?? true;
  const syncing = calendar.status === "loading";
  const minutes = calendar.updated ? Math.round((now - calendar.updated) / 60_000) : null;
  const synced = minutes === null ? "Not synced yet" : minutes < 1 ? "Synced just now" : `Synced ${minutes} min ago`;

  return (
    <section className="scroll -mr-2 flex min-h-0 flex-col gap-4 pr-2">
      <div className="glass shrink-0 p-5">
        <div className="mb-3 flex items-center justify-between">
          <span className="text-[15px] font-semibold">Sync</span>
          <button className="btn btn-ghost btn-icon h-8 w-8" aria-label="Sync now" disabled={syncing} onClick={() => features.calendarRefresh()}>
            <motion.span animate={{ rotate: syncing ? 360 : 0 }} transition={syncing ? { repeat: Infinity, duration: 0.9, ease: "linear" } : { duration: 0 }} className="grid place-items-center">
              <RefreshIcon size={16} />
            </motion.span>
          </button>
        </div>
        <div className={`mb-3 flex items-center gap-2 text-[12.5px] ${calendar.status === "error" ? "text-red" : "text-muted"}`}>
          {syncing ? <Spinner size={12} /> : <span className="dot" style={{ color: calendar.status === "error" ? "var(--h-red)" : "var(--h-green)" }} />}
          {syncing ? "Syncing…" : calendar.status === "error" ? calendar.error : synced}
        </div>
        <div className="mb-1.5 text-[12.5px] font-[560] text-ink-2">Check for changes every</div>
        <Segmented
          size="sm"
          label="Sync interval"
          value={String(settings.calendarSync ?? 5)}
          options={["5", "15", "30", "60"].map((v) => ({ value: v, label: v === "60" ? "1 h" : `${v} min` }))}
          onChange={(v) => saveSettings({ calendarSync: Number(v) })}
        />
      </div>

      <div className="glass shrink-0 p-5">
        <div className="mb-1 flex items-center gap-2 text-[15px] font-semibold">
          <SparkleIcon size={16} /> Before each call
        </div>
        <p className="mb-3 text-[12px] leading-snug text-muted">For meetings with a Meet, Zoom, Teams, Webex, Slack or Discord link.</p>
        <Setting title="Turn on noise cancellation" on={auto} onChange={(calendarAuto) => saveSettings({ calendarAuto })} />
        <AnimatePresence initial={false}>
          {auto && (
            <motion.div initial={{ height: 0, opacity: 0 }} animate={{ height: "auto", opacity: 1 }} exit={{ height: 0, opacity: 0 }} className="overflow-hidden">
              <div className="pb-2 pt-1">
                <Segmented
                  size="sm"
                  label="How long before"
                  value={String(settings.calendarLead ?? 1)}
                  options={["1", "2", "5"].map((v) => ({ value: v, label: `${v} min before` }))}
                  onChange={(v) => saveSettings({ calendarLead: Number(v) })}
                />
              </div>
              <Setting title="Turn it off after the call" detail="Only if Hush turned it on." on={settings.calendarAutoOff ?? true} onChange={(calendarAutoOff) => saveSettings({ calendarAutoOff })} />
            </motion.div>
          )}
        </AnimatePresence>
        <Setting title="Pop up with a Join button" detail="From the tray, a minute before." on={settings.calendarPopup ?? true} onChange={(calendarPopup) => saveSettings({ calendarPopup })} />
      </div>

      <Calendars calendar={calendar} onAdd={onReconnect} />
    </section>
  );
}

function Calendars({ calendar, onAdd }: { calendar: CalendarInfo; onAdd: () => void }) {
  const [removing, setRemoving] = useState<string | null>(null);
  return (
    <div className="glass shrink-0 p-5">
      <div className="mb-2 flex items-center justify-between">
        <span className="text-[15px] font-semibold">Calendars</span>
        <button className="btn btn-soft btn-sm h-8" onClick={onAdd}>
          + Add a calendar
        </button>
      </div>
      <ul className="divide-y divide-[var(--h-line)]">
        {calendar.calendars.map((c) => (
          <li key={c.id} className="flex items-center gap-3 py-2.5">
            <span className="h-2.5 w-2.5 shrink-0 rounded-full" style={{ background: CALENDAR_COLORS[c.color % CALENDAR_COLORS.length], opacity: c.enabled ? 1 : 0.35 }} />
            <div className="min-w-0 flex-1">
              <div className={`truncate text-[13.5px] font-[580] ${c.enabled ? "" : "text-muted"}`}>{c.name ?? c.source}</div>
              <div className={`truncate text-[11.5px] ${c.status === "error" ? "text-red" : "text-muted"}`}>
                {c.status === "error" ? c.error : !c.enabled ? "Paused" : c.status === "loading" ? "Syncing…" : `${c.source} · ${c.meetings} upcoming`}
              </div>
            </div>
            {removing === c.id ? (
              <button
                className="btn btn-sm h-7 bg-[#fdecea] px-3 text-[12px] text-red"
                onClick={async () => {
                  await features.calendarDisconnect(c.id);
                  setRemoving(null);
                  toast(`${c.name ?? "Calendar"} removed. Hush forgot its address.`);
                }}
              >
                Remove
              </button>
            ) : (
              <button className="btn btn-ghost btn-icon h-7 w-7 text-faint hover:text-red" aria-label={`Remove ${c.name ?? c.source}`} onClick={() => setRemoving(c.id)} onBlur={() => setTimeout(() => setRemoving(null), 200)}>
                ✕
              </button>
            )}
            <Toggle label={`Follow ${c.name ?? c.source}`} on={c.enabled} onChange={(enabled) => features.calendarSetEnabled(c.id, enabled)} />
          </li>
        ))}
      </ul>
      <p className="mt-2 text-[11.5px] leading-snug text-muted">The same meeting in two calendars shows once, with one pop-up.</p>
    </div>
  );
}

function Setting({ title, detail, on, onChange }: { title: string; detail?: string; on: boolean; onChange: (on: boolean) => void }) {
  return (
    <div className="flex items-center justify-between gap-3 py-2">
      <div>
        <div className="text-[13.5px] font-[560]">{title}</div>
        {detail && <div className="text-[11.5px] text-muted">{detail}</div>}
      </div>
      <Toggle label={title} on={on} onChange={onChange} />
    </div>
  );
}
