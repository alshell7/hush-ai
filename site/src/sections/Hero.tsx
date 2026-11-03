import { m } from "motion/react";
import { useState } from "react";
import { CarIcon, CheckIcon, CupIcon, DownloadIcon, FanIcon, GitHubIcon, KeyboardIcon, PawIcon } from "../components/Icons";
import { Mochi } from "../components/Mochi";
import { OS_NAME, REPO_URL, TARGETS, useDownloadInfo, type Desktop } from "../lib/download";
import { usePauseOffscreen } from "../lib/hooks";

const NOISES = [
  { label: "Keyboard", Icon: KeyboardIcon, pos: "left-0 top-[6%] sm:-left-[6%] sm:top-[12%]", delay: 0, wide: false },
  { label: "Fan hum", Icon: FanIcon, pos: "right-0 top-[2%] sm:-right-[4%] sm:top-[4%]", delay: 1.4, wide: false },
  { label: "Barking dog", Icon: PawIcon, pos: "-left-[10%] top-[54%]", delay: 0.7, wide: true },
  { label: "Traffic", Icon: CarIcon, pos: "-right-[2%] top-[60%] sm:-right-[10%] sm:top-[50%]", delay: 2.1, wide: false },
  { label: "Café chatter", Icon: CupIcon, pos: "right-[2%] bottom-[4%]", delay: 2.8, wide: true },
];

function HeroMascot() {
  const ref = usePauseOffscreen<HTMLDivElement>();
  const [awake, setAwake] = useState(false);
  return (
    <div
      ref={ref}
      className="relative mx-auto aspect-square w-full max-w-[300px] sm:max-w-[420px] lg:max-w-[460px]"
      onPointerEnter={() => setAwake(true)}
      onPointerLeave={() => setAwake(false)}
    >
      {/* Soft glow, painted once. */}
      <div className="absolute inset-[12%] rounded-full opacity-70 blur-3xl" style={{ background: "var(--h-opal-strong)" }} />
      <div className="absolute -inset-[3%] rounded-full" style={{ boxShadow: "inset 0 0 0 1px rgba(30,27,24,.06)" }} />
      <div className="absolute -inset-[9%] rounded-full opacity-70" style={{ boxShadow: "inset 0 0 0 1px rgba(30,27,24,.04)" }} />
      {/* A gentle sound ripple. */}
      <div
        className="absolute inset-[4%] rounded-full"
        style={{ boxShadow: "inset 0 0 0 2px rgba(255,174,148,.5)", animation: "h-pulse-ring 4.5s ease-out infinite", willChange: "transform, opacity" }}
      />
      {/* The opal ring turns slowly, like the app's switch when Hush is on. */}
      <div
        className="absolute inset-[4%] rounded-full"
        style={{
          background: "var(--h-opal-conic)",
          animation: "h-spin 18s linear infinite",
          willChange: "transform",
          WebkitMask: "radial-gradient(farthest-side, transparent calc(100% - 7px), #000 calc(100% - 6px))",
          mask: "radial-gradient(farthest-side, transparent calc(100% - 7px), #000 calc(100% - 6px))",
        }}
      />
      <div
        className="absolute inset-[9%] grid place-items-center rounded-full"
        style={{
          background: "var(--h-opal)",
          boxShadow: "inset 0 3px 0 rgba(255,255,255,.85), inset 0 -18px 40px rgba(232,115,90,.12), 0 30px 60px -24px rgba(232,115,90,.55)",
        }}
      >
        <div className="absolute inset-[3%] rounded-full" style={{ background: "radial-gradient(60% 50% at 35% 25%, rgba(255,255,255,.7), transparent 70%)" }} />
        <div className="relative w-[60%]" style={{ animation: "h-breathe 4.2s var(--h-ease-soft) infinite", transformOrigin: "50% 90%", willChange: "transform" }}>
          <Mochi size={240} mood={awake ? "awake" : "happy"} breathe={false} className="h-auto w-full" />
        </div>
      </div>

      {NOISES.map(({ label, Icon, pos, delay, wide }, i) => (
        <m.div
          key={label}
          className={`absolute ${pos} ${wide ? "hidden sm:block" : ""}`}
          initial={{ opacity: 0, scale: 0.85 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ delay: 0.3 + i * 0.12, duration: 0.6, ease: [0.2, 0.9, 0.1, 1] }}
        >
          <div style={{ animation: `h-float 6s ease-in-out ${delay}s infinite` }}>
            <span
              className="flex h-9 items-center gap-2 rounded-full bg-white/85 pl-1.5 pr-3 text-[13px] font-[560] text-muted sm:h-10 sm:text-[13.5px]"
              style={{ boxShadow: "inset 0 1px 0 #fff, 0 0 0 1px rgba(30,27,24,.06), 0 10px 24px -14px rgba(30,27,24,.35)" }}
            >
              <span className="grid h-6 w-6 place-items-center rounded-full bg-[#f4efe8] text-ink-2 sm:h-7 sm:w-7">
                <Icon size={15} />
              </span>
              <span className="line-through decoration-coral/70 decoration-[1.5px]">{label}</span>
            </span>
          </div>
        </m.div>
      ))}
      <m.div
        className="absolute bottom-[2%] left-0 sm:bottom-[6%] sm:left-[-2%]"
        initial={{ opacity: 0, scale: 0.85 }}
        animate={{ opacity: 1, scale: 1 }}
        transition={{ delay: 1, duration: 0.6, ease: [0.2, 0.9, 0.1, 1] }}
      >
        <span
          className="flex h-10 items-center gap-2 rounded-full bg-white pl-1.5 pr-3.5 text-[13.5px] font-[620] text-ink sm:h-11 sm:text-[14.5px]"
          style={{ boxShadow: "inset 0 1px 0 #fff, 0 0 0 1px rgba(31,157,114,.18), 0 14px 30px -14px rgba(31,157,114,.5)" }}
        >
          <span className="grid h-7 w-7 place-items-center rounded-full text-white sm:h-8 sm:w-8" style={{ background: "var(--h-green)" }}>
            <CheckIcon size={16} strokeWidth={2.4} />
          </span>
          Your voice
        </span>
      </m.div>
    </div>
  );
}

export function DownloadButton({ size = "lg" }: { size?: "lg" | "md" }) {
  const { os, primary, urlFor } = useDownloadInfo();
  const name = primary ? OS_NAME[os as Desktop] : null;
  return (
    <a
      href={primary ? urlFor(primary) : "#download"}
      className={`btn btn-primary group ${size === "lg" ? "btn-lg min-w-[236px]" : ""}`}
      rel={primary ? "noopener" : undefined}
    >
      <DownloadIcon size={19} />
      {name ? `Download for ${name}` : "Download Hush AI"}
    </a>
  );
}

function DownloadMeta() {
  const { os, primary, release } = useDownloadInfo();
  if (!primary) return null;
  const target = TARGETS.find((t) => t.id === primary)!;
  return (
    <p className="mt-3 text-[13.5px] text-muted">
      {OS_NAME[os as Desktop]} · {target.label}
      {release.version ? ` · v${release.version}` : ""} ·{" "}
      <a href="#download" className="link text-ink-2">
        Other downloads
      </a>
    </p>
  );
}

export function Hero() {
  return (
    <section id="top" aria-labelledby="hero-title" className="relative">
      <div className="wrap grid items-center gap-12 pb-16 pt-14 sm:pt-20 lg:grid-cols-[1.2fr_1fr] lg:gap-8 lg:pb-24 lg:pt-24">
        <div className="text-center lg:text-left">
          <span className="eyebrow">On-device noise cancellation</span>
          <h1 id="hero-title" className="display mt-6 text-[clamp(38px,11.4vw,46px)] sm:text-[66px] lg:text-[64px] xl:text-[76px]">
            Silence the noise.
            <br />
            <span className="opal-text">Keep your voice.</span>
          </h1>
          <p className="lede mx-auto mt-6 max-w-[560px] text-[17.5px] sm:text-[20px] lg:mx-0">
            Hush AI removes keyboards, fans, barking dogs, traffic and café chatter from your microphone in real time, on your own computer. Every app gets a clean
            microphone called <strong className="font-[620] text-ink">Hush AI Microphone</strong>.
          </p>
          <div className="mt-9 flex flex-col items-center gap-3 sm:flex-row sm:justify-center lg:justify-start">
            <DownloadButton />
            <a href={REPO_URL} className="btn btn-soft btn-lg group" target="_blank" rel="noopener">
              <GitHubIcon size={19} /> View on GitHub
            </a>
          </div>
          <DownloadMeta />
          <p className="mt-5 flex flex-wrap items-center justify-center gap-x-2.5 gap-y-1 text-[14px] font-[560] text-ink-2 lg:justify-start">
            <span>Free</span>
            <span aria-hidden className="text-faint">·</span>
            <span>Open source</span>
            <span aria-hidden className="text-faint">·</span>
            <span>Windows, macOS, Linux</span>
          </p>
        </div>
        <HeroMascot />
      </div>
    </section>
  );
}
