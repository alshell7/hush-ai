import { AnimatePresence, motion } from "motion/react";
import { useEffect } from "react";
import { Toasts, toast } from "./components/Toast";
import { startFeatures } from "./lib/features";
import { api, invoke, isTauri, on } from "./lib/ipc";
import { selftest } from "./lib/selftest";
import { startStore, useApp } from "./lib/store";
import { startUpdateChecks } from "./lib/updater";
import { Main } from "./views/Main";
import { Onboarding } from "./views/Onboarding";
import { Notch } from "./views/Notch";
import { Widget } from "./views/Widget";

const isWidget = location.hash === "#widget";
const isNotch = location.hash === "#notch";
/** A small window that is not the main one: no update checks, no self-test. */
const isOverlay = isWidget || isNotch;

export function App() {
  const ready = useApp((s) => s.ready);
  const onboarded = useApp((s) => s.settings?.onboarded);

  useEffect(() => {
    startStore()
      .then(() => {
        startFeatures();
        // Show the window only after the first frame, so it never flashes white.
        requestAnimationFrame(() => requestAnimationFrame(() => !isOverlay && api.ready()));
        if (!isOverlay) startUpdateChecks();
        if (!isOverlay && isTauri) invoke<boolean>("selftest_mode").then((yes) => {
          if (yes) selftest();
        });
      })
      .catch((e) => toast(`Hush AI could not start: ${e}`, "error", 10000));
    if (!isOverlay) {
      const stop = on("engine", (e) => {
        if (e.status === "error" && e.message) toast(e.message, "error");
      });
      return () => {
        stop.then((fn) => fn());
      };
    }
  }, []);

  if (isWidget) return ready ? <Widget /> : null;
  if (isNotch) return ready ? <Notch /> : null;

  return (
    <>
      <div className="aurora" />
      <AnimatePresence mode="wait">
        {ready && (
          <motion.div key={onboarded ? "home" : "onboarding"} className="relative h-full" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0, scale: 0.99 }} transition={{ duration: 0.35 }}>
            {onboarded ? <Main /> : <Onboarding />}
          </motion.div>
        )}
      </AnimatePresence>
      <Toasts />
    </>
  );
}
