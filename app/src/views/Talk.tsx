// The Talk tab: Live Translate both ways, and the conversations it keeps. On the left, every
// conversation, grouped by day, with search and filters; on the right, the call's toolbar and
// the conversation as a chat: their words in your language, yours in theirs.
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { AppIcon } from "../components/AppIcons";
import { Spinner, Toggle } from "../components/Controls";
import { GeminiKeyConnect } from "../components/GeminiKeyConnect";
import { ChatIcon, CheckIcon, CopyIcon, DownloadIcon, LockIcon, PencilIcon, SearchIcon, StarIcon, TrashIcon } from "../components/Icons";
import { Mochi } from "../components/Mochi";
import { toast } from "../components/Toast";
import { LanguagePair, LaneCaption, ModePicker, TheirVoiceButton, TranslateConfirm, useTranslateSwitch } from "../components/TranslateCard";
import { dropThread, features, languageName, loadConversations, loadThread, putSummary, talking, threadTitle, TRANSLATE_SHORTCUT, useFeatures, type Line, type ThreadSummary } from "../lib/features";
import { isTauri } from "../lib/ipc";
import { saveSettings, useApp } from "../lib/store";
import { clockTime, useNow } from "../lib/time";

type Filter = "all" | "starred" | "meetings";

const day = new Intl.DateTimeFormat(undefined, { weekday: "long" });
const date = new Intl.DateTimeFormat(undefined, { day: "numeric", month: "long" });

/** "Today", "Yesterday", "Monday", "3 October". */
function pastDay(ms: number, now: number) {
  const start = (t: number) => new Date(t).setHours(0, 0, 0, 0);
  const days = Math.round((start(now) - start(ms)) / 86_400_000);
  if (days <= 0) return "Today";
  if (days === 1) return "Yesterday";
  if (days < 7) return day.format(ms);
  return date.format(ms);
}

const short = (code: string) => code.split("-")[0].toUpperCase();

export function Talk() {
  const conversations = useFeatures((s) => s.conversations);
  const info = useFeatures((s) => s.translate);
  const on = talking(info);
  const [selected, setSelected] = useState<number | null>(null);
  // Follow the newest conversation while talking, until the user opens another one.
  const [follow, setFollow] = useState(true);

  useEffect(() => {
    loadConversations();
  }, []);
  useEffect(() => {
    if (on) setFollow(true);
  }, [on]);

  const newest = conversations?.[0]?.id ?? null;
  const current = follow ? newest : selected ?? newest;

  return (
    <main className="grid min-h-0 flex-1 grid-cols-[minmax(240px,290px)_1fr] gap-4">
      <ConversationList
        selected={current}
        onSelect={(id) => {
          setSelected(id);
          setFollow(id === newest);
        }}
      />
      <section className="glass flex min-h-0 flex-col overflow-hidden">
        <Toolbar />
        <ThreadView id={current} live={on && current === newest} />
      </section>
    </main>
  );
}

/** The call's controls: the switch, which directions, languages and their voice. */
function Toolbar() {
  const info = useFeatures((s) => s.translate);
  const toggle = useTranslateSwitch();
  return (
    <div className="shrink-0 border-b border-[var(--h-line)] px-5 pb-3.5 pt-4">
      <div className="flex flex-wrap items-center gap-x-4 gap-y-3">
        <div className="flex items-center gap-3">
          <Toggle label="Live Translate" size="lg" on={toggle.on} onChange={(next) => toggle.request(next)} />
          <div>
            <div className="flex items-center gap-1.5 text-[15px] font-[650] tracking-[-0.01em]">
              Live Translate <span className="chip h-5 text-[10px]">Preview</span>
            </div>
            <div className="text-[11.5px] text-muted">
              {toggle.on ? <Status /> : info?.hasKey ? <>Off · <kbd className="font-sans font-semibold">{TRANSLATE_SHORTCUT}</kbd> in any app</> : "Needs your Gemini API key"}
            </div>
          </div>
        </div>
        <div className="ml-auto flex items-center gap-2">
          <ModePicker />
          <TheirVoiceButton />
        </div>
      </div>
      <div className="mt-3 max-w-[460px]">
        <LanguagePair inline />
      </div>
      <AnimatePresence initial={false}>{toggle.asking && <TranslateConfirm side={toggle.asking} onConfirm={toggle.confirm} onCancel={toggle.cancel} />}</AnimatePresence>
      {!info?.hasKey && (
        <div className="mt-3 flex items-center gap-2 text-[12px] text-ink-2">
          <LockIcon size={13} />
          Uses your own Gemini API key; voices go to Google only while it is on.
          <button className="btn btn-soft btn-sm ml-auto h-8" onClick={toggle.openSetup}>
            Add your key
          </button>
        </div>
      )}
      <GeminiKeyConnect open={toggle.setupOpen} onClose={toggle.closeSetup} />
    </div>
  );
}

function Status() {
  const info = useFeatures((s) => s.translate)!;
  const lanes = [info.me, info.them].filter((l) => l.on);
  const error = lanes.find((l) => l.status === "error");
  if (error) return <span className="text-red">{error.message}</span>;
  if (lanes.some((l) => l.status === "connecting"))
    return (
      <span className="inline-flex items-center gap-1">
        <Spinner size={10} /> Connecting to Google…
      </span>
    );
  return <span className="text-green">Live · {info.me.on && info.them.on ? "both ways" : info.me.on ? "your voice" : "their voice"}</span>;
}

function ConversationList({ selected, onSelect }: { selected: number | null; onSelect: (id: number) => void }) {
  const conversations = useFeatures((s) => s.conversations);
  const keep = useApp((s) => s.settings?.translateHistory ?? true);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [language, setLanguage] = useState("");
  const now = useNow(60_000);

  const languages = useMemo(() => [...new Set((conversations ?? []).flatMap((c) => c.languages))].sort(), [conversations]);

  const groups = useMemo(() => {
    const q = query.trim().toLowerCase();
    const list = (conversations ?? []).filter(
      (c) =>
        (filter === "all" || (filter === "starred" ? c.starred : !!c.meeting)) &&
        (!language || c.languages.includes(language)) &&
        (!q || threadTitle(c).toLowerCase().includes(q) || c.preview.toLowerCase().includes(q)),
    );
    const map = new Map<string, ThreadSummary[]>();
    for (const c of list) {
      const key = pastDay(c.start, now);
      map.set(key, [...(map.get(key) ?? []), c]);
    }
    return [...map.entries()];
  }, [conversations, query, filter, language, now]);

  const exportAll = () => exportThreads([], "Hush conversations");

  return (
    <section className="glass flex min-h-0 flex-col p-4">
      <div className="mb-2.5 flex items-center justify-between gap-2 px-1">
        <div>
          <div className="text-[15px] font-semibold">Conversations</div>
          <div className="text-[11px] text-muted">{conversations?.length ? `${conversations.length} · on this computer` : "Kept on this computer"}</div>
        </div>
        {!!conversations?.length && (
          <button className="btn btn-ghost btn-icon h-8 w-8" title="Export every conversation" aria-label="Export every conversation" onClick={exportAll}>
            <DownloadIcon size={16} />
          </button>
        )}
      </div>
      <label className="mb-2 flex h-9 items-center gap-2 rounded-xl bg-white/80 px-3 focus-within:bg-white" style={{ boxShadow: "inset 0 0 0 1px var(--h-line-2)" }}>
        <SearchIcon size={14} className="text-faint" />
        <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Search words, people, meetings" className="min-w-0 flex-1 bg-transparent text-[12.5px] outline-none placeholder:text-faint" style={{ userSelect: "text" }} />
      </label>
      <div className="mb-2 flex items-center gap-1.5">
        {(["all", "starred", "meetings"] as Filter[]).map((f) => (
          <button key={f} className={`chip h-7 cursor-pointer px-2.5 text-[11.5px] transition-colors ${filter === f ? "bg-[var(--h-ink)] text-white" : "hover:bg-white"}`} onClick={() => setFilter(f)}>
            {f === "all" ? "All" : f === "starred" ? "Starred" : "Meetings"}
          </button>
        ))}
        {languages.length > 1 && (
          <select aria-label="Language" value={language} onChange={(e) => setLanguage(e.target.value)} className="chip ml-auto h-7 max-w-[92px] cursor-pointer truncate px-2 text-[11.5px] outline-none">
            <option value="">Any language</option>
            {languages.map((l) => (
              <option key={l} value={l}>
                {languageName(l)}
              </option>
            ))}
          </select>
        )}
      </div>
      <div className="scroll -mr-2 min-h-0 flex-1 space-y-3 pr-2">
        {conversations && conversations.length === 0 && (
          <div className="flex flex-col items-center gap-3 px-3 py-10 text-center">
            <Mochi size={52} mood="sleep" />
            <div className="text-[12px] leading-snug text-muted">
              {keep ? "Turn on Live Translate in a call. Each meeting, or each stretch of talk, becomes a conversation here." : "Keeping conversations is off."}
            </div>
            {!keep && (
              <button className="btn btn-soft btn-sm" onClick={() => saveSettings({ translateHistory: true })}>
                Keep conversations
              </button>
            )}
          </div>
        )}
        {groups.map(([label, list]) => (
          <div key={label}>
            <div className="eyebrow mb-1 px-1.5">{label}</div>
            <ul className="space-y-1">
              {list.map((c) => (
                <li key={c.id}>
                  <button
                    onClick={() => onSelect(c.id)}
                    className={`group flex w-full items-start gap-2.5 rounded-2xl px-2.5 py-2 text-left transition-colors ${selected === c.id ? "bg-white" : "hover:bg-white/60"}`}
                    style={selected === c.id ? { boxShadow: "inset 0 0 0 1px var(--h-line), 0 4px 14px rgba(30,27,24,.06)" } : undefined}
                  >
                    <span className="mt-0.5 grid h-7 w-7 shrink-0 place-items-center rounded-xl bg-white" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
                      {c.app ? <AppIcon app={c.app} size={15} /> : <ChatIcon size={14} className="text-ink-2" />}
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="flex items-center gap-1">
                        <span className="truncate text-[12.5px] font-[620]">{threadTitle(c)}</span>
                        {c.starred && <StarIcon filled size={11} className="shrink-0 text-amber" />}
                      </span>
                      <span className="block truncate text-[11.5px] leading-snug text-muted">{c.preview}</span>
                      <span className="mt-0.5 flex items-center gap-1.5 text-[10.5px] text-faint">
                        <span className="tabular-nums">{clockTime(c.start)}</span>·<span>{c.lines} phrases</span>
                        {c.languages.slice(0, 3).map((l) => (
                          <span key={l} className="font-semibold">
                            {short(l)}
                          </span>
                        ))}
                      </span>
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        ))}
        {!!conversations?.length && groups.length === 0 && <div className="py-6 text-center text-[12px] text-muted">Nothing matches.</div>}
      </div>
    </section>
  );
}

async function exportThreads(ids: number[], name: string) {
  try {
    if (!isTauri) {
      toast("Exported (preview).");
      return;
    }
    const { save } = await import("@tauri-apps/plugin-dialog");
    const path = await save({
      defaultPath: `${name.replace(/[\\/:*?"<>|]/g, "-")}.md`,
      filters: [
        { name: "Markdown", extensions: ["md"] },
        { name: "Text", extensions: ["txt"] },
        { name: "JSON", extensions: ["json"] },
      ],
    });
    if (!path) return;
    const extension = path.split(".").pop()?.toLowerCase();
    const format = extension === "txt" ? "txt" : extension === "json" ? "json" : "md";
    await features.conversationsExport(ids, format, path);
    toast("Conversation exported.");
  } catch (e) {
    toast(String(e), "error");
  }
}

function ThreadView({ id, live }: { id: number | null; live: boolean }) {
  const summary = useFeatures((s) => s.conversations?.find((c) => c.id === id) ?? null);
  const lines = useFeatures((s) => (id === null ? null : s.lines[id] ?? null));
  const info = useFeatures((s) => s.translate);
  const scroller = useRef<HTMLDivElement>(null);
  const pinned = useRef(true);

  useEffect(() => {
    if (id !== null) loadThread(id);
    pinned.current = true;
  }, [id]);

  // Stay at the newest line while the user is at the bottom.
  useLayoutEffect(() => {
    const el = scroller.current;
    if (el && pinned.current) el.scrollTop = el.scrollHeight;
  }, [lines, info?.me.pendingOriginal, info?.them.pendingOriginal, live]);

  const showLive = live || (talking(info) && id === null);

  if (id === null && !showLive) {
    return (
      <div className="grid flex-1 place-items-center p-8 text-center">
        <div className="flex max-w-[340px] flex-col items-center gap-3">
          <Mochi size={64} mood="happy" />
          <div className="text-[15px] font-[650]">Talk with anyone, in any language</div>
          <p className="text-[12.5px] leading-snug text-muted">
            They hear you in their language through Hush AI Microphone, and you read them in yours (and can hear them too) through Hush AI Speaker. Every conversation stays here, grouped by meeting.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {summary && <ThreadHeader summary={summary} />}
      <div
        ref={scroller}
        className="scroll min-h-0 flex-1 space-y-2.5 px-5 py-4"
        onScroll={(e) => {
          const el = e.currentTarget;
          pinned.current = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
        }}
      >
        {lines === null && id !== null && (
          <div className="grid place-items-center py-10">
            <Spinner />
          </div>
        )}
        {lines?.map((line, i) => (
          <Bubble key={`${line.at}-${i}`} line={line} />
        ))}
        {showLive && (
          <div className="space-y-2 pt-1">
            <LaneCaption side="them" large pending />
            <LaneCaption side="me" large pending />
          </div>
        )}
      </div>
    </div>
  );
}

function ThreadHeader({ summary }: { summary: ThreadSummary }) {
  const [editing, setEditing] = useState(false);
  const [title, setTitle] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);
  const minutes = Math.max(1, Math.round((summary.end - summary.start) / 60_000));

  const rename = async () => {
    setEditing(false);
    const next = title.trim();
    if (next === threadTitle(summary)) return;
    const updated = await features.conversationRename(summary.id, next || null);
    if (updated) putSummary(updated);
  };
  const star = async () => {
    const updated = await features.conversationStar(summary.id, !summary.starred);
    if (updated) putSummary(updated);
  };
  const remove = async () => {
    await features.conversationDelete(summary.id);
    dropThread(summary.id);
    toast("Conversation deleted.");
  };

  return (
    <div className="flex shrink-0 items-center gap-3 border-b border-[var(--h-line)] px-5 py-2.5">
      <div className="min-w-0 flex-1">
        {editing ? (
          <input
            autoFocus
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            onBlur={rename}
            onKeyDown={(e) => {
              if (e.key === "Enter") rename();
              if (e.key === "Escape") setEditing(false);
            }}
            className="w-full rounded-lg bg-white px-2 py-0.5 text-[14px] font-[650] outline-none"
            style={{ boxShadow: "inset 0 0 0 1px var(--h-line-2)", userSelect: "text" }}
          />
        ) : (
          <button
            className="group flex max-w-full items-center gap-1.5 text-left"
            title="Rename"
            onClick={() => {
              setTitle(threadTitle(summary));
              setEditing(true);
            }}
          >
            <span className="truncate text-[14px] font-[650]">{threadTitle(summary)}</span>
            <PencilIcon size={12} className="shrink-0 text-faint opacity-0 transition-opacity group-hover:opacity-100" />
          </button>
        )}
        <div className="truncate text-[11.5px] text-muted">
          {new Date(summary.start).toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "short" })} · {clockTime(summary.start)} · {minutes} min · {summary.languages.map(languageName).join(", ")}
        </div>
      </div>
      <button className={`btn btn-ghost btn-icon h-8 w-8 ${summary.starred ? "text-amber" : ""}`} aria-label={summary.starred ? "Unstar" : "Star"} title={summary.starred ? "Unstar" : "Star"} onClick={star}>
        <StarIcon size={16} filled={summary.starred} />
      </button>
      <button className="btn btn-ghost btn-icon h-8 w-8" aria-label="Export" title="Export as Markdown, text or JSON" onClick={() => exportThreads([summary.id], threadTitle(summary))}>
        <DownloadIcon size={16} />
      </button>
      {confirmDelete ? (
        <button className="btn btn-sm h-8 bg-[#fdecea] text-red" onClick={remove} onBlur={() => setTimeout(() => setConfirmDelete(false), 200)} autoFocus>
          Delete
        </button>
      ) : (
        <button className="btn btn-ghost btn-icon h-8 w-8 hover:text-red" aria-label="Delete" title="Delete" onClick={() => setConfirmDelete(true)}>
          <TrashIcon size={16} />
        </button>
      )}
    </div>
  );
}

function Bubble({ line }: { line: Line }) {
  const [copied, setCopied] = useState(false);
  const mine = line.side === "me";
  // Their words: the translation you need first. Yours: what you said first.
  const main = mine ? line.original : line.translated || line.original;
  const sub = mine ? line.translated : line.translated ? line.original : "";
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(mine ? line.translated || line.original : main);
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    } catch {
      toast("Could not copy.", "error");
    }
  };
  return (
    <motion.div layout="position" initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} className={`group flex ${mine ? "justify-end" : "justify-start"}`}>
      <div className={`max-w-[78%] rounded-[20px] px-3.5 py-2.5 ${mine ? "rounded-br-md bg-[#1e1b18] text-white" : "rounded-bl-md bg-white"}`} style={mine ? undefined : { boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
        <div className={`mb-0.5 flex items-center gap-1.5 text-[10.5px] font-semibold ${mine ? "text-white/55" : "text-muted"}`}>
          <span>{mine ? "You" : "Them"}</span>·<span className="tabular-nums">{clockTime(line.at)}</span>·
          <span title={`${languageName(line.from)} → ${languageName(line.to)}`}>
            {short(line.from)} → {short(line.to)}
          </span>
          <button className="ml-1 opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100" aria-label="Copy" onClick={copy}>
            {copied ? <CheckIcon size={12} /> : <CopyIcon size={12} />}
          </button>
        </div>
        <p className="text-[14px] font-[540] leading-snug" style={{ userSelect: "text" }}>
          {main}
        </p>
        {sub && (
          <p className={`mt-1 text-[12px] leading-snug ${mine ? "text-white/60" : "text-muted"}`} style={{ userSelect: "text" }}>
            {sub}
          </p>
        )}
      </div>
    </motion.div>
  );
}
