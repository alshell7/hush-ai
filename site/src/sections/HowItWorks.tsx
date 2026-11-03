import type { ReactNode } from "react";
import { LaptopGlyph, MicIcon, TerminalGlyph, WindowsGlyph } from "../components/Icons";
import { Mark, Mochi } from "../components/Mochi";
import { APPS } from "../components/Mockups";
import { AppMonogram, Reveal, SectionHeader } from "../components/ui";
import { usePauseOffscreen } from "../lib/hooks";

/** Jagged line: the raw microphone, voice plus noise. */
const NOISY = "M0 20 L4 14 L7 25 L10 9 L13 29 L16 15 L19 23 L22 6 L25 33 L28 13 L31 26 L34 11 L37 28 L40 17 L43 22 L46 8 L49 31 L52 14 L55 25 L58 12 L61 27 L64 16 L67 22 L70 19 L72 20";
/** Smooth line: voice only. */
const CLEAN = "M0 20 C6 20 8 10 14 10 S22 30 28 30 S36 12 42 12 S50 26 56 26 S64 20 72 20";

function Connector({ kind }: { kind: "noisy" | "clean" | "plain" }) {
  return (
    <div aria-hidden className="flex h-14 shrink-0 items-center justify-center self-center lg:h-auto lg:w-[64px]">
      <svg viewBox="0 0 72 40" width="60" height="34" className="rotate-90 overflow-visible lg:rotate-0">
        <path d="M0 20 H72" stroke="rgba(30,27,24,.12)" strokeWidth="1.5" strokeDasharray="3 5" style={{ animation: "h-flow 1.6s linear infinite" }} />
        {kind === "noisy" && <path d={NOISY} fill="none" stroke="#e8735a" strokeWidth="2" strokeLinejoin="round" opacity=".85" />}
        {kind === "clean" && <path d={CLEAN} fill="none" stroke="#1f9d72" strokeWidth="2.4" strokeLinecap="round" />}
        <path d="M64 14 L71 20 L64 26" fill="none" stroke="rgba(30,27,24,.35)" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    </div>
  );
}

function Node({ icon, title, body, highlight, children }: { icon: ReactNode; title: string; body: string; highlight?: boolean; children?: ReactNode }) {
  return (
    <div
      className={`card flex flex-1 flex-col items-center p-6 text-center lg:basis-0 lg:justify-center ${highlight ? "lg:-my-5 lg:grow-[1.15]" : ""}`}
      style={highlight ? { boxShadow: "var(--h-inner), 0 0 0 1.5px rgba(255,174,148,.6), 0 30px 60px -28px rgba(232,115,90,.5)" } : undefined}
    >
      {icon}
      <h3 className="mt-4 text-[18px] font-[650] tracking-[-0.02em]">{title}</h3>
      <p className="mt-1.5 max-w-[260px] text-[14.5px] leading-relaxed text-ink-2">{body}</p>
      {children}
    </div>
  );
}

const PLATFORMS = [
  {
    icon: <LaptopGlyph size={20} />,
    name: "macOS",
    body: "Hush's own Core Audio plug-in. It installs with one password prompt.",
  },
  {
    icon: <WindowsGlyph size={18} />,
    name: "Windows",
    body: "VB-CABLE, a free virtual cable by VB-Audio. You install it once and Hush renames it to Hush AI Microphone. Hush's own driver is built and waiting for Microsoft's signature.",
  },
  {
    icon: <TerminalGlyph size={20} />,
    name: "Linux",
    body: "A PipeWire or PulseAudio virtual source. Hush creates it for you.",
  },
];

const STEPS = ["Download Hush and open it", "Turn noise cancellation on", "Choose Hush AI Microphone in your app"];

export function HowItWorks() {
  const diagramRef = usePauseOffscreen<HTMLDivElement>();
  return (
    <section id="how" aria-labelledby="how-title" className="relative py-20 sm:py-28">
      <div className="wrap">
        <SectionHeader
          id="how-title"
          eyebrow="How it works"
          title={
            <>
              Noise in. <span className="opal-text">Voice out.</span>
            </>
          }
          sub="Hush sits between your real microphone and your apps. It cleans every 20 ms of sound on your CPU and passes on only your voice."
        />

        <Reveal className="mt-14">
          <div ref={diagramRef} className="flex flex-col items-stretch lg:flex-row">
            <Node
              icon={
                <span className="grid h-14 w-14 place-items-center rounded-[18px] bg-[#f4efe8] text-ink-2">
                  <MicIcon size={26} />
                </span>
              }
              title="Your microphone"
              body="Built-in, USB or headset. Voice and noise together."
            />
            <Connector kind="noisy" />
            <Node
              highlight
              icon={
                <span className="grid h-[76px] w-[76px] place-items-center rounded-full" style={{ background: "var(--h-opal)", boxShadow: "inset 0 2px 0 rgba(255,255,255,.8), 0 14px 30px -14px rgba(232,115,90,.6)" }}>
                  <Mochi size={48} breathe={false} />
                </span>
              }
              title="Hush engine, on your CPU"
              body="DeepFilterNet 3 or NVIDIA's Real-time RE-USE, in native Rust. Nothing leaves your computer."
            >
              <div className="mt-4 flex flex-wrap justify-center gap-1.5">
                <span className="chip">Rust</span>
                <span className="chip">No GPU</span>
                <span className="chip">10–20 ms frames</span>
              </div>
            </Node>
            <Connector kind="clean" />
            <Node
              icon={
                <span className="grid h-14 w-14 place-items-center rounded-[18px] bg-white" style={{ boxShadow: "inset 0 0 0 1px var(--h-line-2)" }}>
                  <Mark size={32} />
                </span>
              }
              title="Hush AI Microphone"
              body="A virtual microphone that carries only your voice."
            />
            <Connector kind="plain" />
            <Node
              icon={
                <span className="flex h-14 items-center">
                  {APPS.slice(0, 5).map((a, i) => (
                    <span key={a.name} className="rounded-[12px] ring-2 ring-white" style={{ marginLeft: i ? -10 : 0 }}>
                      <AppMonogram name={a.name} tint={a.tint} size={38} />
                    </span>
                  ))}
                </span>
              }
              title="Your apps"
              body="Zoom, Google Meet, Teams, Discord, Slack, OBS and anything else with a microphone setting."
            />
          </div>
        </Reveal>

        <Reveal className="mt-16">
          <ol className="grid gap-3 sm:grid-cols-3">
            {STEPS.map((step, i) => (
              <li key={step} className="tile flex items-center gap-4 p-4 sm:p-5">
                <span className="grid h-10 w-10 shrink-0 place-items-center rounded-full bg-ink text-[15px] font-[650] text-white">{i + 1}</span>
                <span className="text-[16px] font-[560] leading-snug">{step}</span>
              </li>
            ))}
          </ol>
        </Reveal>

        <div className="mt-14">
          <Reveal>
            <h3 className="text-center text-[20px] font-[640] tracking-[-0.02em]">Where Hush AI Microphone comes from</h3>
          </Reveal>
          <ul className="mt-6 grid gap-4 md:grid-cols-3">
            {PLATFORMS.map((p, i) => (
              <li key={p.name} className="flex">
                <Reveal delay={i * 0.06} className="flex w-full">
                  <div className="card w-full p-6">
                    <div className="flex items-center gap-2.5 text-[16px] font-[640]">
                      <span className="grid h-9 w-9 place-items-center rounded-[12px] bg-[#f4efe8] text-ink-2">{p.icon}</span>
                      {p.name}
                    </div>
                    <p className="mt-3 text-[15px] leading-relaxed text-ink-2">{p.body}</p>
                  </div>
                </Reveal>
              </li>
            ))}
          </ul>
        </div>
      </div>
    </section>
  );
}
