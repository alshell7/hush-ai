// Small notices at the bottom of the window.
import { AnimatePresence, motion } from "motion/react";
import { useSyncExternalStore } from "react";

interface Notice {
  id: number;
  text: string;
  tone: "info" | "error";
}

let notices: Notice[] = [];
let next = 1;
const subs = new Set<() => void>();
const publish = () => subs.forEach((fn) => fn());

export function toast(text: string, tone: Notice["tone"] = "info", ms = 4200) {
  const id = next++;
  notices = [...notices.slice(-2), { id, text, tone }];
  publish();
  setTimeout(() => {
    notices = notices.filter((n) => n.id !== id);
    publish();
  }, ms);
}

export function Toasts() {
  const list = useSyncExternalStore(
    (fn) => {
      subs.add(fn);
      return () => subs.delete(fn);
    },
    () => notices,
  );
  return (
    <div className="pointer-events-none fixed inset-x-0 bottom-5 z-50 flex flex-col items-center gap-2">
      <AnimatePresence>
        {list.map((n) => (
          <motion.div
            key={n.id}
            layout
            initial={{ opacity: 0, y: 16, scale: 0.96 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 8, scale: 0.98 }}
            transition={{ type: "spring", stiffness: 380, damping: 30 }}
            className="pointer-events-auto max-w-[460px] rounded-2xl px-4 py-2.5 text-[13px] font-[520] text-white"
            style={{ background: n.tone === "error" ? "#b8383e" : "rgba(30,27,24,.92)", boxShadow: "0 16px 36px -14px rgba(30,27,24,.5)" }}
          >
            {n.text}
          </motion.div>
        ))}
      </AnimatePresence>
    </div>
  );
}
