// The real app UI, built into ./app/ by CI, runs here on a simulated engine. Until it
// loads (or when it is missing, as in local development) a static picture stands in.
import { useEffect, useRef, useState } from "react";
import { AppHeroCard, AppScreen, WidgetMock } from "../components/AppMockup";
import { CheckIcon } from "../components/Icons";
import { Reveal, SectionHeader } from "../components/ui";
import { sleep, useActive, useMediaQuery } from "../lib/hooks";

const APP_W = 940;
const APP_H = 640;

function LiveWindow() {
  const frameRef = useRef<HTMLDivElement>(null);
  const [scale, setScale] = useState(1);
  const [status, setStatus] = useState<"waiting" | "ready" | "failed">("waiting");
  const [canEmbed, setCanEmbed] = useState(false);
  const desktop = useMediaQuery("(min-width: 1024px)");
  const visible = useActive(frameRef, "300px");

  useEffect(() => {
    // Never nest the site inside itself (a dev server answers unknown paths with this page).
    setCanEmbed(window.self === window.top);
    const el = frameRef.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => setScale(entry.contentRect.width / APP_W));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Show the frame only once the app has actually drawn something; anything else (a 404
  // page, this site served as a fallback, a broken build) keeps the static picture.
  const onLoad = async (e: React.SyntheticEvent<HTMLIFrameElement>) => {
    try {
      const doc = e.currentTarget.contentDocument;
      const root = doc?.getElementById("root");
      if (!doc || !root || doc.querySelector('meta[name="hush-site"]')) return setStatus("failed");
      for (let i = 0; i < 40 && !root.childElementCount; i++) await sleep(150);
      setStatus(root.childElementCount ? "ready" : "failed");
    } catch {
      setStatus("failed");
    }
  };

  const showFrame = desktop && canEmbed && status !== "failed";
  const ready = status === "ready";

  return (
    <>
    <div
      ref={frameRef}
      className="relative w-full overflow-hidden rounded-[18px] bg-paper"
      style={{
        aspectRatio: `${APP_W} / ${APP_H}`,
        boxShadow: "0 0 0 1px rgba(30,27,24,.1), 0 2px 4px rgba(30,27,24,.05), 0 50px 100px -40px rgba(30,27,24,.45), 0 30px 60px -30px rgba(232,115,90,.25)",
      }}
    >
      <div className="absolute left-0 top-0 origin-top-left" style={{ width: APP_W, height: APP_H, transform: `scale(${scale})` }}>
        <div aria-hidden={ready} className="transition-opacity duration-500" style={{ opacity: ready ? 0 : 1 }}>
          <AppScreen />
        </div>
        {showFrame && (
          <iframe
            src="./app/index.html?platform=macos"
            title="Hush AI app demo"
            loading="lazy"
            width={APP_W}
            height={APP_H}
            onLoad={onLoad}
            className="absolute inset-0 border-0 transition-opacity duration-500"
            // Off-screen, the hidden frame stops rendering, so the demo costs nothing.
            style={{ opacity: ready ? 1 : 0, display: ready && !visible ? "none" : "block", colorScheme: "light" }}
          />
        )}
        <div aria-hidden className="pointer-events-none absolute left-[20px] top-[21px] flex gap-2">
          <span className="h-3 w-3 rounded-full bg-[#ff6b5f]" style={{ boxShadow: "inset 0 0 0 .5px rgba(0,0,0,.15)" }} />
          <span className="h-3 w-3 rounded-full bg-[#febc2e]" style={{ boxShadow: "inset 0 0 0 .5px rgba(0,0,0,.15)" }} />
          <span className="h-3 w-3 rounded-full bg-[#28c840]" style={{ boxShadow: "inset 0 0 0 .5px rgba(0,0,0,.15)" }} />
        </div>
      </div>
    </div>
    <p className="mt-5 flex h-7 items-center justify-center text-[13.5px] text-muted">
      {ready && (
        <span className="flex items-center gap-3">
          <span className="chip chip-green">
            <span className="dot" /> Live demo · simulated engine
          </span>
          <a className="link text-ink-2" href={`${import.meta.env.BASE_URL}app/`} target="_blank" rel="noopener">
            Open it full size
          </a>
        </span>
      )}
    </p>
    </>
  );
}

const WIDGET_POINTS = ["Turn noise cancellation on or off", "Switch microphones", "Glance at your levels"];

export function Showcase() {
  return (
    <section id="app" aria-labelledby="app-title" className="relative py-20 sm:py-28">
      <div className="wrap">
        <SectionHeader
          id="app-title"
          eyebrow="The app"
          title={
            <>
              One switch. <span className="opal-text">That's it.</span>
            </>
          }
          sub={
            <>
              <span className="hidden lg:inline">This is the real Hush AI app, running in your browser with a simulated engine. Go ahead, click around.</span>
              <span className="lg:hidden">Turn Hush on, pick your microphone, and get back to your call.</span>
            </>
          }
        />

        <Reveal className="mx-auto mt-12 hidden max-w-[980px] lg:block">
          <LiveWindow />
        </Reveal>
        <Reveal className="mx-auto mt-10 max-w-[420px] lg:hidden">
          <AppHeroCard />
        </Reveal>

        <div className="mx-auto mt-16 grid max-w-[980px] items-center gap-10 sm:mt-20 md:grid-cols-2 md:gap-14">
          <Reveal className="order-2 flex justify-center md:order-1">
            <WidgetMock />
          </Reveal>
          <Reveal className="order-1 md:order-2" delay={0.1}>
            <h3 className="heading text-[28px] sm:text-[36px]">Always one click away</h3>
            <p className="lede mt-3 text-[17px]">
              Hush lives in your system tray on Windows and Linux, and in the menu bar on macOS. The small widget does the everyday things without opening the window.
            </p>
            <ul className="mt-6 space-y-2.5">
              {WIDGET_POINTS.map((p) => (
                <li key={p} className="flex items-center gap-3 text-[16px] font-[540]">
                  <span className="grid h-6 w-6 place-items-center rounded-full" style={{ background: "rgba(31,157,114,.12)", color: "#157353" }}>
                    <CheckIcon size={14} strokeWidth={2.4} />
                  </span>
                  {p}
                </li>
              ))}
            </ul>
          </Reveal>
        </div>
      </div>
    </section>
  );
}
