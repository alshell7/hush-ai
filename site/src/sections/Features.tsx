import type { ReactNode } from "react";
import { BoltIcon, CpuIcon, LockIcon, PauseIcon, PlayIcon, RefreshIcon, TrayIcon } from "../components/Icons";
import { AppsArt, CpuArt, PlatformsArt, PrivateArt, RealtimeArt, TrayArt, UpdateArt } from "../components/LineArt";
import { Mark } from "../components/Mochi";
import { Reveal, SectionHeader } from "../components/ui";
import { useLoop } from "../lib/hooks";
import { docHref } from "../lib/route";

type Feature = {
  id: string;
  icon: ReactNode;
  title: string;
  body: ReactNode;
  art: ReactNode;
  chips?: string[];
  span: string;
  tall?: boolean;
};

const FEATURES: Feature[] = [
  {
    id: "private",
    icon: <LockIcon size={18} />,
    title: "On your computer, and private",
    body: (
      <>
        Your voice is cleaned on your own CPU. There is no account and no telemetry. Hush goes online only for the one-time model download, update checks, your
        calendar if you connect one, and Google while you use Live Translate, the one feature that is not on-device.
      </>
    ),
    art: <PrivateArt />,
    chips: ["No account", "No telemetry", "Noise cancellation works offline"],
    span: "md:col-span-2",
    tall: true,
  },
  {
    id: "realtime",
    icon: <BoltIcon size={18} />,
    title: "Real time",
    body: <>About 60 ms from your microphone to your call. Each model needs about 40 ms on its own; Hush works in 10–20 ms frames.</>,
    art: <RealtimeArt />,
    span: "",
    tall: true,
  },
  {
    id: "cpu",
    icon: <CpuIcon size={18} />,
    title: "Light on the CPU",
    body: <>DeepFilterNet 3, the default engine, uses about 6% of one CPU core. Your fans stay quiet and your battery lasts.</>,
    art: <CpuArt />,
    span: "",
  },
  {
    id: "apps",
    icon: <Mark size={20} />,
    title: "Works with every app",
    body: <>Hush adds a microphone called Hush AI Microphone. Choose it once in Zoom, Meet, Teams, Slack, Discord, Webex or OBS.</>,
    art: <AppsArt />,
    span: "",
  },
  {
    id: "tray",
    icon: <TrayIcon size={18} />,
    title: "Tray and menu-bar widget",
    body: <>Turn Hush on or off, switch microphones and glance at your levels, without opening the window.</>,
    art: <TrayArt />,
    span: "",
  },
  {
    id: "updates",
    icon: <RefreshIcon size={18} />,
    title: "Automatic updates",
    body: <>New versions download quietly. Each one is signed, and Hush checks the signature before it installs.</>,
    art: <UpdateArt />,
    span: "md:col-span-2 lg:col-span-1",
  },
  {
    id: "platforms",
    icon: <LaptopMini />,
    title: "Windows, macOS and Linux",
    body: <>Windows on x64 and ARM64. macOS 11 or later on Apple silicon and Intel. Linux as an AppImage, .deb or .rpm.</>,
    art: <PlatformsArt />,
    chips: ["Windows x64 · ARM64", "macOS 11+ · Apple silicon · Intel", "Linux · AppImage · deb · rpm"],
    span: "md:col-span-2",
  },
];

function LaptopMini() {
  return (
    <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <rect x="4" y="5" width="16" height="11" rx="2" />
      <path d="M2.5 19h19" />
    </svg>
  );
}

function FeatureCard({ f, index }: { f: Feature; index: number }) {
  const { ref, paused, toggle } = useLoop<HTMLDivElement>();
  return (
    <li className={`flex ${f.span}`}>
      <Reveal delay={(index % 3) * 0.06} className="flex w-full">
        <article className="feature card group flex w-full flex-col overflow-hidden" aria-labelledby={`feature-${f.id}`}>
          <div ref={ref} className={`la relative ${f.tall ? "h-[230px] sm:h-[250px]" : "h-[200px] sm:h-[210px]"}`}>
            <div className="la-grid absolute inset-0" aria-hidden />
            <div className="absolute inset-0 px-3 pt-3">{f.art}</div>
            <button
              type="button"
              onClick={toggle}
              className="absolute right-3 top-3 grid h-8 w-8 cursor-pointer place-items-center rounded-full bg-white/80 text-ink-2 opacity-0 transition-opacity duration-200 focus-visible:opacity-100 group-hover:opacity-100 [@media(hover:none)]:opacity-100"
              style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}
              aria-label={paused ? `Play the “${f.title}” animation` : `Pause the “${f.title}” animation`}
            >
              {paused ? <PlayIcon size={13} /> : <PauseIcon size={13} />}
            </button>
          </div>
          <div className="flex flex-1 flex-col p-5 pt-4 sm:p-6 sm:pt-5">
            <h3 id={`feature-${f.id}`} className="flex items-center gap-2.5 text-[17.5px] font-[650] tracking-[-0.02em] sm:text-[18.5px]">
              <span className="feature-icon grid h-8 w-8 shrink-0 place-items-center rounded-[11px] text-ink" style={{ background: "var(--h-opal)" }}>
                {f.icon}
              </span>
              {f.title}
            </h3>
            <p className="mt-2.5 text-[15px] leading-relaxed text-ink-2">{f.body}</p>
            {f.chips && (
              <ul className="mt-4 flex flex-wrap gap-1.5">
                {f.chips.map((c) => (
                  <li key={c} className="chip bg-white" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
                    {c}
                  </li>
                ))}
              </ul>
            )}
          </div>
        </article>
      </Reveal>
    </li>
  );
}

export function Features() {
  return (
    <section id="features" aria-labelledby="features-title" className="relative py-20 sm:py-28">
      <div className="wrap">
        <SectionHeader
          id="features-title"
          eyebrow="Features"
          title={
            <>
              Quiet, fast and <span className="opal-text">yours</span>
            </>
          }
          sub={
            <>
              Two proven speech models in native code, right on your CPU. No GPU, no cloud, no subscription.{" "}
              <a className="link" href={docHref("architecture")}>
                How it works
              </a>
            </>
          }
        />
        <ul className="mt-14 grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {FEATURES.map((f, i) => (
            <FeatureCard key={f.id} f={f} index={i} />
          ))}
        </ul>
      </div>
    </section>
  );
}
