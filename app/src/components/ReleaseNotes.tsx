// Release notes: "What's new" once after an update, and the notes of a downloaded update before
// restarting. Notes come from CHANGELOG.md (built in) and from the update itself.
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import changelog from "../../../CHANGELOG.md?raw";
import { installUpdate } from "../lib/updater";
import { useApp } from "../lib/store";
import { CloseIcon, SparkleIcon } from "./Icons";
import { Mochi } from "./Mochi";

/** The bullets of one version in CHANGELOG.md. */
export function notesFor(version: string, text = changelog): string[] {
  const section = text.split(/^## /m).find((part) => part.startsWith(`${version}\n`) || part.startsWith(`${version}\r\n`));
  return parseBullets(section ?? "");
}

/** "- **Bold.** text" lines; the download table and anything after a rule are left out. */
export function parseBullets(text: string): string[] {
  const body = text.split(/\n(?:---|\*\*Download\*\*|<!-- downloads -->)/)[0];
  return body
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l.startsWith("- "))
    .map((l) => l.slice(2));
}

/** "**Talk both ways.** They hear…" as a bold lead and the rest. */
function Bullet({ text }: { text: string }) {
  const match = text.match(/^\*\*(.+?)\*\*\s*(.*)$/);
  return (
    <li className="flex gap-2.5">
      <span className="mt-[7px] h-1.5 w-1.5 shrink-0 rounded-full" style={{ background: "var(--h-opal)" }} />
      <span className="text-[13px] leading-snug text-ink-2">
        {match ? (
          <>
            <span className="font-[650] text-ink">{match[1]}</span> {match[2]}
          </>
        ) : (
          text
        )}
      </span>
    </li>
  );
}

function Sheet({ title, eyebrow, notes, onClose, action }: { title: string; eyebrow: string; notes: string[]; onClose: () => void; action?: { label: string; run: () => void } }) {
  return createPortal(
    <motion.div className="fixed inset-0 z-[60] grid place-items-center bg-[rgba(30,27,24,.22)] p-6 backdrop-blur-[3px]" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <motion.div role="dialog" aria-label={title} initial={{ y: 14, scale: 0.97 }} animate={{ y: 0, scale: 1 }} exit={{ y: 10, scale: 0.98 }} transition={{ type: "spring", stiffness: 380, damping: 30 }} className="relative w-full max-w-[440px] overflow-hidden rounded-[28px] bg-white" style={{ boxShadow: "0 0 0 1px var(--h-line), 0 30px 80px rgba(30,27,24,.25)" }}>
        <div className="relative flex items-center gap-3.5 px-6 pb-4 pt-6" style={{ background: "linear-gradient(135deg, rgba(255,196,176,.55), rgba(255,224,166,.5) 45%, rgba(201,242,212,.5) 75%, rgba(155,230,224,.45))" }}>
          <Mochi size={52} mood="happy" />
          <div>
            <div className="flex items-center gap-1 text-[11px] font-semibold uppercase tracking-[0.08em] text-ink-2">
              <SparkleIcon size={12} /> {eyebrow}
            </div>
            <div className="text-[20px] font-[700] tracking-[-0.02em]">{title}</div>
          </div>
          <button className="btn btn-ghost btn-icon absolute right-3 top-3 h-8 w-8" aria-label="Close" onClick={onClose}>
            <CloseIcon size={15} />
          </button>
        </div>
        <ul className="space-y-2.5 px-6 py-5">
          {notes.map((n) => (
            <Bullet key={n} text={n} />
          ))}
        </ul>
        <div className="flex items-center justify-end gap-2 border-t border-[var(--h-line)] px-6 py-4">
          <button className="btn btn-ghost btn-sm" onClick={onClose}>
            {action ? "Later" : "Got it"}
          </button>
          {action && (
            <button className="btn btn-primary btn-sm" onClick={action.run}>
              {action.label}
            </button>
          )}
        </div>
      </motion.div>
    </motion.div>,
    document.body,
  );
}

const SEEN = "hush.notes.seen";
const read = (key: string) => {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
};
const write = (key: string, value: string) => {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Private storage: the notes may show again next time.
  }
};

/** Shows "What's new" once after an update, and an update's notes once when it is ready. */
export function ReleaseNotes({ openUpdate, onUpdateClosed }: { openUpdate: boolean; onUpdateClosed: () => void }) {
  const version = useApp((s) => s.version);
  const update = useApp((s) => s.update);
  const [whatsNew, setWhatsNew] = useState<string[] | null>(null);
  const [autoUpdate, setAutoUpdate] = useState(false);

  // After an update: the notes of the version now running, once. A fresh install sees none.
  useEffect(() => {
    if (!version) return;
    const seen = read(SEEN);
    write(SEEN, version);
    if (seen && seen !== version) {
      const notes = notesFor(version);
      if (notes.length) setWhatsNew(notes);
    }
  }, [version]);

  // A downloaded update: its notes, once per version.
  useEffect(() => {
    if (update?.state !== "ready") return;
    const key = `hush.notes.offered.${update.version}`;
    if (read(key)) return;
    write(key, "1");
    setAutoUpdate(true);
  }, [update?.state, update?.version]);

  const updateNotes = update ? (update.notes ? parseBullets(update.notes) : notesFor(update.version)) : [];
  const showUpdate = !!update && update.state === "ready" && (openUpdate || autoUpdate);

  return (
    <AnimatePresence>
      {whatsNew && !showUpdate && <Sheet key="new" eyebrow="What's new" title={`Hush AI ${version}`} notes={whatsNew} onClose={() => setWhatsNew(null)} />}
      {showUpdate && (
        <Sheet
          key="update"
          eyebrow="Update ready"
          title={`Hush AI ${update.version}`}
          notes={updateNotes.length ? updateNotes : ["Fixes and improvements."]}
          onClose={() => {
            setAutoUpdate(false);
            onUpdateClosed();
          }}
          action={{ label: "Restart to update", run: () => installUpdate() }}
        />
      )}
    </AnimatePresence>
  );
}
