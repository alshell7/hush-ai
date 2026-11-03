import { AnimatePresence, m } from "motion/react";
import { useRef, useState, type KeyboardEvent } from "react";
import { CheckIcon } from "../components/Icons";
import { Mark } from "../components/Mochi";
import { APPS, ChooseMicDemo } from "../components/Mockups";
import { AppMonogram, Reveal, SectionHeader } from "../components/ui";
import { useActive } from "../lib/hooks";

export function Guides() {
  const [index, setIndex] = useState(0);
  const tabs = useRef<(HTMLButtonElement | null)[]>([]);
  const stageRef = useRef<HTMLDivElement>(null);
  const stageActive = useActive(stageRef);
  const app = APPS[index];

  const onKey = (e: KeyboardEvent, i: number) => {
    const step = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const next = (i + step + APPS.length) % APPS.length;
    setIndex(next);
    tabs.current[next]?.focus();
  };

  return (
    <section id="guides" aria-labelledby="guides-title" className="relative py-20 sm:py-28">
      <div className="wrap grid gap-12 lg:grid-cols-[1fr_1.05fr] lg:items-center lg:gap-16">
        <div>
          <SectionHeader
            id="guides-title"
            align="left"
            eyebrow="Works in every app"
            title={
              <>
                One microphone. <span className="opal-text">Every app.</span>
              </>
            }
            sub="Hush adds a microphone called Hush AI Microphone. Choose it once in your meeting or streaming app, and the app hears only your voice."
          />
          <Reveal delay={0.1}>
            <div role="tablist" aria-label="Where to choose the microphone" className="mt-8 grid grid-cols-2 gap-2 sm:grid-cols-3">
              {APPS.map((a, i) => {
                const selected = i === index;
                return (
                  <button
                    key={a.name}
                    ref={(el) => {
                      tabs.current[i] = el;
                    }}
                    type="button"
                    role="tab"
                    id={`guide-tab-${i}`}
                    aria-selected={selected}
                    aria-controls="guide-panel"
                    tabIndex={selected ? 0 : -1}
                    onClick={() => setIndex(i)}
                    onPointerEnter={(e) => e.pointerType === "mouse" && setIndex(i)}
                    onKeyDown={(e) => onKey(e, i)}
                    className="relative flex h-14 cursor-pointer items-center gap-2.5 rounded-[18px] px-2.5 text-left text-[14.5px] font-[580] transition-[background-color,box-shadow,transform] duration-200 active:scale-[0.98]"
                    style={{
                      background: selected ? "#fff" : "rgba(255,255,255,.5)",
                      boxShadow: selected
                        ? "inset 0 0 0 1px rgba(30,27,24,.1), 0 12px 28px -16px rgba(30,27,24,.45)"
                        : "inset 0 0 0 1px rgba(30,27,24,.07)",
                    }}
                  >
                    <AppMonogram name={a.name} tint={a.tint} size={34} />
                    <span className="min-w-0 leading-tight">{a.name}</span>
                  </button>
                );
              })}
            </div>

            <div id="guide-panel" role="tabpanel" aria-labelledby={`guide-tab-${index}`} className="card mt-3 overflow-hidden p-5 sm:p-6" style={{ borderRadius: 22 }}>
              <AnimatePresence mode="wait" initial={false}>
                <m.div
                  key={app.name}
                  initial={{ opacity: 0, y: 6 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -6 }}
                  transition={{ duration: 0.18 }}
                >
                  <div className="text-[13px] font-[600] uppercase tracking-[0.06em] text-muted">In {app.name}, open</div>
                  <ol className="mt-3 flex flex-wrap items-center gap-1.5 text-[14.5px] font-[560]">
                    {app.path.map((step, i) => (
                      <li key={step} className="flex items-center gap-1.5">
                        <span className="rounded-full bg-[#f4efe8] px-3 py-1.5 text-ink">{step}</span>
                        {i < app.path.length - 1 && (
                          <span aria-hidden className="text-faint">
                            ›
                          </span>
                        )}
                      </li>
                    ))}
                  </ol>
                  <div className="mt-4 flex flex-wrap items-center gap-2 text-[14.5px]">
                    <span className="text-ink-2">then choose</span>
                    <span
                      className="inline-flex items-center gap-2 rounded-full bg-white py-1.5 pl-2 pr-3 font-[620]"
                      style={{ boxShadow: "inset 0 0 0 1px rgba(31,157,114,.25), 0 6px 16px -10px rgba(31,157,114,.6)" }}
                    >
                      <Mark size={18} /> Hush AI Microphone
                      <CheckIcon size={16} strokeWidth={2.4} className="text-green" />
                    </span>
                  </div>
                </m.div>
              </AnimatePresence>
            </div>
          </Reveal>
        </div>

        <Reveal delay={0.15}>
          <div
            ref={stageRef}
            className="relative grid min-h-[250px] place-items-center overflow-hidden rounded-[28px] px-4 py-10 sm:min-h-[420px] sm:rounded-[32px] sm:px-10 sm:py-14"
            style={{ background: "linear-gradient(140deg, #fff1ea, #fff6e3 40%, #ecfaef 75%, #e3f7f5)", boxShadow: "var(--h-inner), var(--h-shadow-2)" }}
          >
            <div className="pointer-events-none absolute -left-16 -top-20 h-64 w-64 rounded-full opacity-70 blur-3xl" style={{ background: "#ffd3c2" }} />
            <div className="pointer-events-none absolute -bottom-24 -right-10 h-72 w-72 rounded-full opacity-70 blur-3xl" style={{ background: "#bdeee6" }} />
            <div className="relative flex w-full justify-center">
              <ChooseMicDemo app={app.name} active={stageActive} />
            </div>
          </div>
        </Reveal>
      </div>
    </section>
  );
}
