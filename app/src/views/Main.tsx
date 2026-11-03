// The main window: Live noise cancellation, Studio for recordings, and Meetings, as tabs under
// one title bar.
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { CalendarIcon, ChatIcon, GearIcon, PowerIcon, WaveIcon } from "../components/Icons";
import { Wordmark } from "../components/Mochi";
import { TitleBar } from "../components/TitleBar";
import { useFeatures, talking } from "../lib/features";
import { useApp } from "../lib/store";
import { useNow } from "../lib/time";
import { ReleaseNotes } from "../components/ReleaseNotes";
import { Live } from "./Home";
import { Meetings } from "./Meetings";
import { SettingsPanel } from "./Settings";
import { Studio } from "./Studio";
import { Talk } from "./Talk";

const TABS = [
  { id: "live", label: "Live", icon: PowerIcon },
  { id: "translate", label: "Talk", icon: ChatIcon },
  { id: "studio", label: "Studio", icon: WaveIcon },
  { id: "meetings", label: "Meetings", icon: CalendarIcon },
] as const;
type Tab = (typeof TABS)[number]["id"];

export function Main() {
  const [tab, setTab] = useState<Tab>("live");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [notesOpen, setNotesOpen] = useState(false);
  const update = useApp((s) => s.update);
  const studio = useFeatures((s) => s.studio);
  const meetings = useFeatures((s) => s.calendar?.meetings);
  const now = useNow(30_000);
  const translateSetup = useFeatures((s) => s.translateSetup);
  const translating = useFeatures((s) => talking(s.translate));
  // The tray menu or the shortcut asked for the Gemini key setup.
  useEffect(() => {
    if (translateSetup) setTab("translate");
  }, [translateSetup]);
  const soon = meetings?.some((m) => !m.allDay && m.app && m.start - now < 10 * 60_000 && m.end > now);
  const working = studio?.phase === "processing" || studio?.phase === "decoding" || studio?.phase === "recording";

  // Ctrl/Cmd + 1, 2, 3 switch tabs.
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey)) return;
      const index = Number(e.key) - 1;
      if (TABS[index]) {
        e.preventDefault();
        setTab(TABS[index].id);
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, []);

  return (
    <div className="relative flex h-full flex-col">
      <TitleBar>
        <Wordmark size={26} />
        <div data-tauri-drag-region className="flex-1" />
        <nav role="tablist" aria-label="Sections" className="no-drag absolute left-1/2 flex -translate-x-1/2 rounded-full p-[3px]" style={{ background: "rgba(30,27,24,.06)" }}>
          {TABS.map(({ id, label, icon: Icon }) => {
            const active = tab === id;
            const dot = (id === "meetings" && soon) || (id === "studio" && working && !active) || (id === "translate" && translating && !active);
            return (
              <button
                key={id}
                role="tab"
                aria-selected={active}
                onClick={() => setTab(id)}
                className="relative z-0 flex h-[34px] items-center gap-1.5 rounded-full px-3.5 text-[13px] font-[580] transition-colors duration-200"
                style={{ color: active ? "var(--h-ink)" : "var(--h-muted)" }}
              >
                {active && (
                  <motion.span
                    layoutId="main-tab"
                    className="absolute inset-0 -z-10 rounded-full bg-white"
                    style={{ boxShadow: "0 1px 2px rgba(30,27,24,.08), 0 6px 16px -6px rgba(30,27,24,.2)" }}
                    transition={{ type: "spring", stiffness: 420, damping: 34 }}
                  />
                )}
                <Icon size={15} />
                {label}
                {dot && <span className="absolute right-2 top-1.5 h-1.5 w-1.5 rounded-full" style={{ background: id === "meetings" ? "var(--h-coral)" : "var(--h-green)" }} />}
              </button>
            );
          })}
        </nav>
        <AnimatePresence>
          {update && (
            <motion.button
              initial={{ opacity: 0, scale: 0.9 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0 }}
              className="btn btn-opal btn-sm no-drag"
              disabled={update.state !== "ready"}
              onClick={() => setNotesOpen(true)}
            >
              {update.state === "ready" ? `Update to ${update.version}` : `Downloading ${update.version}…`}
            </motion.button>
          )}
        </AnimatePresence>
        <button className="btn btn-ghost btn-icon group no-drag" aria-label="Settings" onClick={() => setSettingsOpen(true)}>
          <GearIcon size={19} />
        </button>
      </TitleBar>

      <div className="relative flex min-h-0 flex-1 flex-col px-5 pb-5">
        <AnimatePresence mode="wait" initial={false}>
          <motion.div
            key={tab}
            className="flex min-h-0 flex-1 flex-col"
            initial={{ opacity: 0, y: 8, scale: 0.995 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -6, scale: 0.995 }}
            transition={{ duration: 0.24, ease: [0.2, 0.9, 0.1, 1] }}
          >
            {tab === "live" && <Live onOpenMeetings={() => setTab("meetings")} onOpenTranslate={() => setTab("translate")} />}
            {tab === "translate" && <Talk />}
            {tab === "studio" && <Studio />}
            {tab === "meetings" && <Meetings />}
          </motion.div>
        </AnimatePresence>
      </div>

      <SettingsPanel open={settingsOpen} onClose={() => setSettingsOpen(false)} />
      <ReleaseNotes openUpdate={notesOpen} onUpdateClosed={() => setNotesOpen(false)} />
    </div>
  );
}
