// Real app logos. Hover, focus or tap one to see where it keeps its microphone setting; the
// tooltip's body is <AppGuide />.
import { AnimatePresence, m } from "motion/react";
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { APPS, AppGuide, appById, type AppId } from "../components/AppGuides";
import { ChooseMicDemo } from "../components/Mockups";
import { Reveal, SectionHeader } from "../components/ui";
import { useActive } from "../lib/hooks";

const useIsoLayoutEffect = typeof window === "undefined" ? useEffect : useLayoutEffect;
const TIP_W = 300;

export function Apps() {
  const gridRef = useRef<HTMLDivElement>(null);
  const tileRefs = useRef<Partial<Record<AppId, HTMLButtonElement | null>>>({});
  const stageRef = useRef<HTMLDivElement>(null);
  const stageActive = useActive(stageRef);
  const [open, setOpen] = useState<AppId | null>(null);
  const [shown, setShown] = useState<AppId>("zoom");
  const [pos, setPos] = useState({ left: 0, top: 0, arrow: TIP_W / 2, width: TIP_W });
  const closeTimer = useRef(0);

  const place = useCallback((id: AppId) => {
    const grid = gridRef.current;
    const tile = tileRefs.current[id];
    if (!grid || !tile) return;
    const g = grid.getBoundingClientRect();
    const t = tile.getBoundingClientRect();
    const width = Math.min(TIP_W, g.width);
    const center = t.left - g.left + t.width / 2;
    const left = Math.max(0, Math.min(g.width - width, center - width / 2));
    setPos({ left, top: t.bottom - g.top + 10, arrow: center - left, width });
  }, []);

  const show = (id: AppId) => {
    clearTimeout(closeTimer.current);
    setOpen(id);
    setShown(id);
  };
  const hide = () => {
    clearTimeout(closeTimer.current);
    closeTimer.current = window.setTimeout(() => setOpen(null), 120);
  };

  useIsoLayoutEffect(() => {
    if (open) place(open);
  }, [open, place]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(null);
    const onResize = () => place(open);
    const onDown = (e: PointerEvent) => {
      if (!gridRef.current?.contains(e.target as Node)) setOpen(null);
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("resize", onResize);
    document.addEventListener("pointerdown", onDown);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("resize", onResize);
      document.removeEventListener("pointerdown", onDown);
    };
  }, [open, place]);

  return (
    <section id="apps" aria-labelledby="apps-title" className="relative py-20 sm:py-28">
      <div className="wrap grid gap-12 lg:grid-cols-[1fr_1.05fr] lg:items-center lg:gap-16">
        <div>
          <SectionHeader
            id="apps-title"
            align="left"
            eyebrow="Works in every app"
            title={
              <>
                One microphone. <span className="opal-text">Every app.</span>
              </>
            }
            sub="Hush adds a microphone called Hush AI Microphone. Choose it once in your meeting, chat or streaming app, and the app hears only your voice. Point at an app to see where."
          />
          <Reveal delay={0.08}>
            <div ref={gridRef} className="relative mt-8" onPointerLeave={(e) => e.pointerType === "mouse" && hide()}>
              <ul className="grid grid-cols-3 gap-2 sm:gap-2.5">
                {APPS.map((a) => {
                  const on = open === a.id;
                  return (
                    <li key={a.id}>
                      <button
                        ref={(el) => {
                          tileRefs.current[a.id] = el;
                        }}
                        type="button"
                        aria-expanded={on}
                        aria-controls={on ? "app-tip" : undefined}
                        aria-label={`${a.name}: where to choose Hush AI Microphone`}
                        onPointerEnter={(e) => e.pointerType === "mouse" && show(a.id)}
                        onClick={() => (on ? setOpen(null) : show(a.id))}
                        onFocus={() => show(a.id)}
                        onBlur={hide}
                        className="app-tile group flex h-[76px] w-full cursor-pointer flex-col items-center justify-center gap-1.5 rounded-[20px] text-[12.5px] font-[600] text-ink-2 transition-[background-color,box-shadow,transform] duration-200 active:scale-[0.97] sm:h-[84px] sm:text-[13.5px]"
                        style={{
                          background: on ? "#fff" : "rgba(255,255,255,.55)",
                          boxShadow: on ? "inset 0 0 0 1px rgba(30,27,24,.1), 0 14px 30px -18px rgba(30,27,24,.5)" : "inset 0 0 0 1px rgba(30,27,24,.07)",
                        }}
                      >
                        <img src={a.logo} alt="" width={30} height={30} loading="lazy" decoding="async" className="h-[30px] w-[30px] object-contain transition-transform duration-300 group-hover:-translate-y-0.5 group-hover:scale-110" />
                        <span className="max-w-full truncate px-1">{a.name}</span>
                      </button>
                    </li>
                  );
                })}
              </ul>
              <AnimatePresence>
                {open && (
                  <m.div
                    id="app-tip"
                    role="tooltip"
                    key="tip"
                    initial={{ opacity: 0, y: -6, scale: 0.97 }}
                    animate={{ opacity: 1, y: 0, scale: 1 }}
                    exit={{ opacity: 0, y: -4, scale: 0.98 }}
                    transition={{ duration: 0.18, ease: [0.2, 0.9, 0.1, 1] }}
                    className="absolute z-20 rounded-[18px] bg-white p-4"
                    style={{ left: pos.left, top: pos.top, width: pos.width, boxShadow: "0 0 0 1px rgba(30,27,24,.08), 0 24px 48px -20px rgba(30,27,24,.45)" }}
                    onPointerEnter={() => clearTimeout(closeTimer.current)}
                  >
                    <span aria-hidden className="absolute -top-[6px] h-3 w-3 rotate-45 bg-white" style={{ left: pos.arrow - 6, boxShadow: "-1px -1px 0 rgba(30,27,24,.08)" }} />
                    <AppGuide app={open} />
                  </m.div>
                )}
              </AnimatePresence>
            </div>
            <p className="mt-5 text-[12.5px] leading-relaxed text-muted">
              App names and logos are trademarks of their owners, who do not endorse Hush AI. They are shown only to say which apps work with Hush AI Microphone.
            </p>
          </Reveal>
        </div>

        <Reveal delay={0.15}>
          <div
            ref={stageRef}
            className="relative grid min-h-[260px] place-items-center overflow-hidden rounded-[28px] px-4 py-10 sm:min-h-[420px] sm:rounded-[32px] sm:px-10 sm:py-14"
            style={{ background: "linear-gradient(140deg, #fff1ea, #fff6e3 40%, #ecfaef 75%, #e3f7f5)", boxShadow: "var(--h-inner), var(--h-shadow-2)" }}
          >
            <div className="pointer-events-none absolute -left-16 -top-20 h-64 w-64 rounded-full opacity-70 blur-3xl" style={{ background: "#ffd3c2" }} />
            <div className="pointer-events-none absolute -bottom-24 -right-10 h-72 w-72 rounded-full opacity-70 blur-3xl" style={{ background: "#bdeee6" }} />
            <div className="relative flex w-full justify-center">
              <ChooseMicDemo app={appById(shown).name} active={stageActive} />
            </div>
          </div>
        </Reveal>
      </div>
    </section>
  );
}
