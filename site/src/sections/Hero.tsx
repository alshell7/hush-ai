import { m } from "motion/react";
import { useState } from "react";
import { APPS, AppLogo } from "../components/AppGuides";
import { CarIcon, CheckIcon, CupIcon, DownloadIcon, FanIcon, GitHubIcon, KeyboardIcon, PawIcon } from "../components/Icons";
import { Mochi } from "../components/Mochi";
import { OS_NAME, REPO_URL, TARGETS, useDownloadInfo, type Desktop } from "../lib/download";
import { usePauseOffscreen } from "../lib/hooks";
import { homeHref } from "../lib/route";

const NOISES = [
  { label: "Keyboard", Icon: KeyboardIcon, pos: "left-0 top-[6%] sm:-left-[6%] sm:top-[12%]", delay: 0, wide: false },
  { label: "Fan hum", Icon: FanIcon, pos: "right-0 top-[2%] sm:-right-[4%] sm:top-[4%]", delay: 1.4, wide: false },
  { label: "Barking dog", Icon: PawIcon, pos: "-left-[10%] top-[54%]", delay: 0.7, wide: true },
  { label: "Traffic", Icon: CarIcon, pos: "-right-[2%] top-[60%] sm:-right-[10%] sm:top-[50%]", delay: 2.1, wide: false },
  { label: "Café chatter", Icon: CupIcon, pos: "right-[2%] bottom-[4%]", delay: 2.8, wide: true },
];

/** The mascot in its opal ring. The switch below it turns "Hush" on and off, for fun. */
function HeroMascot() {
  const ref = usePauseOffscreen<HTMLDivElement>();
  const [on, setOn] = useState(true);
  const [awake, setAwake] = useState(false);
  return (
    <div className="relative mx-auto w-full max-w-[300px] sm:max-w-[420px] lg:max-w-[460px]">
      <div
        ref={ref}
        className="relative aspect-square w-full"
        onPointerEnter={(e) => e.pointerType === "mouse" && setAwake(true)}
        onPointerLeave={() => setAwake(false)}
      >
        {/* Soft glow, painted once. */}
        <div className="absolute inset-[12%] rounded-full blur-3xl transition-opacity duration-700" style={{ background: "var(--h-opal-strong)", opacity: on ? 0.7 : 0.25 }} />
        <div className="absolute -inset-[3%] rounded-full" style={{ boxShadow: "inset 0 0 0 1px rgba(30,27,24,.06)" }} />
        <div className="absolute -inset-[9%] rounded-full opacity-70" style={{ boxShadow: "inset 0 0 0 1px rgba(30,27,24,.04)" }} />
        <div
          className="absolute inset-[4%] rounded-full"
          style={{ boxShadow: "inset 0 0 0 2px rgba(255,174,148,.5)", animation: "h-pulse-ring 4.5s ease-out infinite", willChange: "transform, opacity" }}
        />
        {/* The opal ring turns slowly, like the app's switch when Hush is on. */}
        <div
          className="absolute inset-[4%] rounded-full transition-opacity duration-500"
          style={{
            background: "var(--h-opal-conic)",
            opacity: on ? 1 : 0,
            animation: "h-spin 18s linear infinite",
            willChange: "transform",
            WebkitMask: "radial-gradient(farthest-side, transparent calc(100% - 7px), #000 calc(100% - 6px))",
            mask: "radial-gradient(farthest-side, transparent calc(100% - 7px), #000 calc(100% - 6px))",
          }}
        />
        <div
          className="absolute inset-[4%] rounded-full border-2 border-dashed transition-opacity duration-500"
          style={{ borderColor: "rgba(30,27,24,.14)", opacity: on ? 0 : 1 }}
        />
        <div
          className="absolute inset-[9%] grid place-items-center overflow-hidden rounded-full"
          style={{ background: "linear-gradient(180deg,#fff,#f4f1ec)", boxShadow: "inset 0 3px 0 rgba(255,255,255,.85), 0 30px 60px -24px rgba(30,27,24,.35)" }}
        >
          <div className="absolute inset-0 transition-opacity duration-500" style={{ background: "var(--h-opal)", opacity: on ? 1 : 0 }} />
          <div className="absolute inset-[3%] rounded-full" style={{ background: "radial-gradient(60% 50% at 35% 25%, rgba(255,255,255,.7), transparent 70%)" }} />
          <div className="relative w-[60%]" style={{ animation: "h-breathe 4.2s var(--h-ease-soft) infinite", transformOrigin: "50% 90%", willChange: "transform" }}>
            <Mochi size={240} mood={!on ? "oops" : awake ? "awake" : "happy"} breathe={false} className="h-auto w-full" />
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
            <div style={{ animation: `${on ? "h-float 6s ease-in-out" : "h-jitter .5s steps(2)"} ${delay}s infinite` }}>
              <span
                className="flex h-9 items-center gap-2 rounded-full bg-white/85 pl-1.5 pr-3 text-[13px] font-[560] transition-colors duration-300 sm:h-10 sm:text-[13.5px]"
                style={{
                  color: on ? "var(--h-muted)" : "var(--h-ink)",
                  boxShadow: on
                    ? "inset 0 1px 0 #fff, 0 0 0 1px rgba(30,27,24,.06), 0 10px 24px -14px rgba(30,27,24,.35)"
                    : "inset 0 1px 0 #fff, 0 0 0 1.5px rgba(232,115,90,.45), 0 10px 24px -12px rgba(232,115,90,.5)",
                }}
              >
                <span className="grid h-6 w-6 place-items-center rounded-full bg-[#f4efe8] text-ink-2 sm:h-7 sm:w-7">
                  <Icon size={15} />
                </span>
                <span className={`decoration-coral/70 decoration-[1.5px] ${on ? "line-through" : ""}`}>{label}</span>
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

      <div className="mt-6 flex justify-center">
        <button
          type="button"
          role="switch"
          aria-checked={on}
          onClick={() => setOn((v) => !v)}
          className="group flex h-12 cursor-pointer items-center gap-3 rounded-full bg-white/80 pl-4 pr-1.5 text-[14.5px] font-[600] text-ink-2 transition-transform active:scale-[0.97]"
          style={{ boxShadow: "inset 0 0 0 1px var(--h-line-2), var(--h-shadow-1)" }}
        >
          <span>Noise cancellation</span>
          <span className="relative block h-9 w-[60px] rounded-full transition-[background] duration-300" style={{ background: on ? "var(--h-opal-strong)" : "rgba(30,27,24,.14)" }}>
            <span
              className="absolute left-[3px] top-[3px] h-[30px] w-[30px] rounded-full bg-white transition-transform duration-300"
              style={{ transform: on ? "translateX(24px)" : "none", boxShadow: "0 2px 6px rgba(30,27,24,.25)", transitionTimingFunction: "var(--h-ease)" }}
            />
          </span>
          <span className="sr-only">{on ? "On" : "Off"}</span>
        </button>
      </div>
    </div>
  );
}

export function DownloadButton({ size = "lg" }: { size?: "lg" | "md" }) {
  const { os, primary, urlFor } = useDownloadInfo();
  const name = primary ? OS_NAME[os as Desktop] : null;
  return (
    <a
      href={primary ? urlFor(primary) : homeHref("#download")}
      className={`btn btn-primary group ${size === "lg" ? "btn-lg min-w-[236px]" : ""}`}
      rel={primary ? "noopener" : undefined}
    >
      <DownloadIcon size={19} className="transition-transform duration-300 group-hover:translate-y-[2px]" />
      {name ? `Download for ${name}` : "Download Hush AI"}
    </a>
  );
}

function DownloadMeta() {
  const { os, primary, release } = useDownloadInfo();
  // Same height before and after detection, so nothing below moves.
  const target = primary ? TARGETS.find((t) => t.id === primary)! : null;
  return (
    <p className="mt-3 h-5 text-[13.5px] text-muted" aria-live="polite">
      {target && (
        <>
          {OS_NAME[os as Desktop]} · {target.label}
          {release.version ? ` · v${release.version}` : ""} ·{" "}
          <a href={homeHref("#download")} className="link text-ink-2">
            Other downloads
          </a>
        </>
      )}
    </p>
  );
}

export function Hero() {
  return (
    <section id="top" aria-labelledby="hero-title" className="relative">
      <div className="wrap grid items-center gap-12 pb-12 pt-12 sm:pt-20 lg:grid-cols-[1.2fr_1fr] lg:gap-8 lg:pb-16 lg:pt-20">
        <div className="text-center lg:text-left">
          <a href={homeHref("#compare")} className="eyebrow group transition-colors hover:bg-white">
            <span className="sm:hidden">Free alternative to Krisp &amp; Adobe Podcast</span>
            <span className="hidden sm:inline">The free, open-source alternative to Krisp and Adobe Podcast</span>
            <span aria-hidden className="text-faint transition-transform duration-300 group-hover:translate-x-0.5">
              →
            </span>
          </a>
          <h1 id="hero-title" className="display mt-6 text-[clamp(38px,11.4vw,46px)] sm:text-[66px] lg:text-[64px] xl:text-[76px]">
            Silence the noise.
            <br />
            <span className="opal-text">Keep your voice.</span>
          </h1>
          <p className="lede mx-auto mt-6 max-w-[580px] text-[17.5px] sm:text-[20px] lg:mx-0">
            Hush AI removes keyboards, fans, barking dogs, traffic and café chatter from your microphone while you talk, and cleans up recordings, on your own
            computer. Every app gets a clean microphone called <strong className="font-[620] text-ink">Hush AI Microphone</strong>, and with Live Translate it can
            even speak for you in another language.
          </p>
          <div className="mt-9 flex flex-col items-center gap-3 sm:flex-row sm:justify-center lg:justify-start">
            <DownloadButton />
            <a href={REPO_URL} className="btn btn-soft btn-lg group" target="_blank" rel="noopener">
              <GitHubIcon size={19} /> View on GitHub
            </a>
          </div>
          <DownloadMeta />
          <ul className="mt-4 flex flex-wrap items-center justify-center gap-x-2.5 gap-y-1 text-[14px] font-[560] text-ink-2 lg:justify-start">
            {["Free, no limits", "Open source", "No account", "Windows, macOS, Linux"].map((t, i) => (
              <li key={t} className="flex items-center gap-2.5">
                {i > 0 && (
                  <span aria-hidden className="text-faint">
                    ·
                  </span>
                )}
                {t}
              </li>
            ))}
          </ul>
        </div>
        <HeroMascot />
      </div>

      <div className="wrap pb-6">
        <div className="flex flex-col items-center gap-4 sm:flex-row sm:justify-center sm:gap-6">
          <span className="text-[12.5px] font-[600] uppercase tracking-[0.08em] text-muted">Works with</span>
          <ul className="flex flex-wrap items-center justify-center gap-x-5 gap-y-3" aria-label="Apps that work with Hush AI Microphone">
            {APPS.slice(0, 7).map((a) => (
              <li key={a.id} className="flex items-center gap-2 text-[13.5px] font-[580] text-ink-2">
                <AppLogo app={a.id} size={20} />
                <span className="hidden md:inline">{a.name}</span>
                <span className="sr-only md:hidden">{a.name}</span>
              </li>
            ))}
            <li className="text-[13.5px] font-[580] text-muted">and more</li>
          </ul>
        </div>
      </div>
    </section>
  );
}
