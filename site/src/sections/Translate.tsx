// Live Translate (Preview): you speak your language and the call hears you in theirs, through
// Google's Gemini Live Translate model. The one Hush feature that is not on-device, and the
// section says so plainly. The call picture is a CSS loop (tr-* in styles.css).
import { CheckIcon, PauseIcon, PlayIcon, TranslateIcon } from "../components/Icons";
import { Mark, Mochi } from "../components/Mochi";
import { Reveal, SectionHeader } from "../components/ui";
import { useLoop } from "../lib/hooks";

const SETUP = [
  { title: "Bring your own Gemini API key", body: "Hush opens Google AI Studio's key page, notices the key you copy, checks it with Google and keeps it in Hush's settings folder. Google has a free tier." },
  { title: "Pick the languages", body: "“I speak” and “They hear”, from more than 70 languages, with a swap button." },
  { title: "Turn it on in your call", body: "Your noise-cancelled voice goes to Gemini, and the translated voice comes out of Hush AI Microphone. Nothing changes in Zoom, Meet or Teams." },
];

const SWITCHES = ["A toggle in the app", "A row in the tray or menu-bar widget, with a swap", "“Translate my voice” in the tray menu"];

function Bars({ className, n = 18 }: { className: string; n?: number }) {
  return (
    <span className={`flex h-5 items-center gap-[2px] ${className}`}>
      {Array.from({ length: n }, (_, i) => (
        <span key={i} className="h-full w-[3px] rounded-full" style={{ transform: `scaleY(${(0.25 + (((i * 37) % 11) / 11) * 0.75).toFixed(2)})`, background: "currentColor" }} />
      ))}
    </span>
  );
}

function CallMock() {
  const { ref, paused, toggle } = useLoop<HTMLDivElement>();
  return (
    <div className="relative">
      <div
        ref={ref}
        aria-hidden
        className="tr relative overflow-hidden rounded-[24px] p-4 sm:p-5"
        style={{ background: "linear-gradient(150deg,#fff3ec,#fff8ea 40%,#eefaf1 75%,#e6f7f5)", boxShadow: "var(--h-inner), var(--h-shadow-2)" }}
      >
        {/* Languages, with the swap */}
        <div className="flex items-center justify-center gap-2 text-[12.5px] font-[620]">
          <span className="relative flex h-9 w-[118px] items-center justify-center rounded-full bg-white" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
            <span className="tr-lang-a absolute inset-0 flex items-center justify-center">I speak · EN</span>
            <span className="tr-lang-b absolute inset-0 flex items-center justify-center">I speak · ES</span>
          </span>
          <span className="tr-swap grid h-9 w-9 place-items-center rounded-full bg-ink text-white">
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M7 4 3 8l4 4M3 8h14M17 20l4-4-4-4M21 16H7" />
            </svg>
          </span>
          <span className="relative flex h-9 w-[118px] items-center justify-center rounded-full bg-white" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
            <span className="tr-lang-a absolute inset-0 flex items-center justify-center">They hear · ES</span>
            <span className="tr-lang-b absolute inset-0 flex items-center justify-center">They hear · EN</span>
          </span>
        </div>

        <div className="mt-4 grid gap-3 sm:grid-cols-[1.15fr_1fr]">
          {/* You, with live captions */}
          <div className="rounded-[18px] bg-white p-3.5" style={{ boxShadow: "0 0 0 1px rgba(30,27,24,.07), 0 16px 34px -22px rgba(30,27,24,.45)" }}>
            <div className="flex items-center gap-2.5">
              <Mochi size={30} mood="happy" breathe={false} />
              <span className="text-[13px] font-[650]">You</span>
              <Bars className="tr-voice ml-auto text-coral" />
            </div>
            <div className="mt-3 space-y-2">
              <div className="relative overflow-hidden rounded-[12px] bg-[#faf7f3] px-3 py-2">
                <div className="text-[10.5px] font-[650] uppercase tracking-[0.06em] text-muted">You said</div>
                <div className="relative mt-0.5 text-[13.5px] font-[560] text-ink">
                  Hi everyone, thanks for joining.
                  <span className="tr-cover-1 absolute inset-0 bg-[#faf7f3]" />
                </div>
              </div>
              <div className="relative overflow-hidden rounded-[12px] px-3 py-2" style={{ background: "linear-gradient(100deg, rgba(201,242,212,.6), rgba(155,230,224,.35))" }}>
                <div className="text-[10.5px] font-[650] uppercase tracking-[0.06em] text-green">They hear · Spanish</div>
                <div className="relative mt-0.5 text-[13.5px] font-[600] text-ink">
                  Hola a todos, gracias por venir.
                  <span className="tr-cover-2 absolute inset-0" style={{ background: "linear-gradient(100deg, #def6e4, #d2f2ec)" }} />
                </div>
              </div>
            </div>
          </div>

          {/* The other side of the call */}
          <div className="relative grid min-h-[170px] place-items-center rounded-[18px] bg-[#2a2622] p-3 pt-11 text-white sm:pt-3">
            <div className="absolute left-3 top-3 flex items-center gap-1.5 rounded-full bg-white/10 px-2 py-1 text-[10.5px] font-[600]">
              <Mark size={12} /> Hush AI Microphone
            </div>
            <div className="text-center">
              <span className="relative mx-auto grid h-14 w-14 place-items-center rounded-full bg-[#4a453f] text-[18px] font-[650]">
                <span className="tr-ring absolute -inset-1 rounded-full" style={{ boxShadow: "0 0 0 2px #9fe8b8" }} />
                Ana
              </span>
              <div className="tr-hears mt-3 flex items-center justify-center gap-1.5 text-[12.5px] font-[600]">
                <CheckIcon size={13} strokeWidth={2.6} className="text-[#9fe8b8]" /> hears you in Spanish
              </div>
              <Bars className="tr-voice-out mx-auto mt-2 text-[#9fe8b8]" n={14} />
            </div>
          </div>
        </div>
      </div>
      <button
        type="button"
        onClick={toggle}
        className="absolute bottom-3 right-3 grid h-8 w-8 cursor-pointer place-items-center rounded-full bg-white/80 text-ink-2"
        style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}
        aria-label={paused ? "Play the translation animation" : "Pause the translation animation"}
      >
        {paused ? <PlayIcon size={13} /> : <PauseIcon size={13} />}
      </button>
    </div>
  );
}

function Key({ children }: { children: string }) {
  return (
    <kbd className="rounded-[7px] bg-white px-1.5 py-0.5 font-sans text-[12.5px] font-[650] text-ink" style={{ boxShadow: "inset 0 0 0 1px var(--h-line-2), 0 1px 0 rgba(30,27,24,.12)" }}>
      {children}
    </kbd>
  );
}

export function Translate() {
  return (
    <section id="translate" aria-labelledby="translate-title" className="relative py-20 sm:py-28">
      <div className="wrap">
        <SectionHeader
          id="translate-title"
          eyebrow="New · Live Translate (Preview)"
          title={
            <>
              Speak your language. <span className="opal-text">They hear theirs.</span>
            </>
          }
          sub="Turn on Live Translate and the call hears you in another language, in a natural translated voice. It works in every app through Hush AI Microphone, with Google's Gemini Live Translate model."
        />

        <div className="mt-12 grid items-start gap-10 lg:grid-cols-[1.1fr_1fr] lg:gap-14">
          <Reveal>
            <CallMock />
            <div className="mt-4 rounded-[20px] p-5 text-[14.5px] leading-relaxed text-ink-2" style={{ background: "rgba(255,174,148,.14)", boxShadow: "inset 0 0 0 1px rgba(232,115,90,.22)" }}>
              <p className="font-[650] text-ink">The one Hush feature that is not on your computer</p>
              <ul className="mt-2 list-disc space-y-1 pl-5">
                <li>While Live Translate is on, your voice goes to Google to be translated. Hush itself has no server.</li>
                <li>Noise cancellation, recordings and everything else stay on your computer.</li>
                <li>The translation follows about a sentence behind you: Gemini translates phrase by phrase.</li>
                <li>It needs an internet connection.</li>
              </ul>
            </div>
          </Reveal>

          <Reveal delay={0.08}>
            <ol className="space-y-4">
              {SETUP.map((s, i) => (
                <li key={s.title} className="flex gap-4">
                  <span className="grid h-8 w-8 shrink-0 place-items-center rounded-full bg-ink text-[13.5px] font-[650] text-white">{i + 1}</span>
                  <span>
                    <span className="block text-[16.5px] font-[630] tracking-[-0.01em]">{s.title}</span>
                    <span className="mt-0.5 block text-[15px] leading-relaxed text-ink-2">{s.body}</span>
                  </span>
                </li>
              ))}
            </ol>
            <div className="card mt-7 p-5">
              <h3 className="flex items-center gap-2 text-[16px] font-[650]">
                <TranslateIcon size={18} /> Switch it during a call
              </h3>
              <ul className="mt-3 space-y-2 text-[14.5px] text-ink-2">
                {SWITCHES.map((s) => (
                  <li key={s} className="flex items-start gap-2.5">
                    <CheckIcon size={15} strokeWidth={2.4} className="mt-1 shrink-0 text-green" /> {s}
                  </li>
                ))}
                <li className="flex flex-wrap items-center gap-x-2 gap-y-1.5">
                  <CheckIcon size={15} strokeWidth={2.4} className="shrink-0 text-green" /> A shortcut:
                  <span className="flex items-center gap-1">
                    <Key>Ctrl</Key>+<Key>Alt</Key>+<Key>T</Key>
                  </span>
                  <span className="text-muted">on Windows and Linux,</span>
                  <span className="flex items-center gap-1">
                    <Key>⌘</Key>
                    <Key>⌥</Key>
                    <Key>T</Key>
                  </span>
                  <span className="text-muted">on macOS</span>
                </li>
              </ul>
              <p className="mt-4 text-[13px] text-muted">Live captions show what you said and what they hear. Languages use BCP-47 codes: en, es, fr, de, hi, ar, zh-Hans, ja, pt-BR and more.</p>
            </div>
          </Reveal>
        </div>
      </div>
    </section>
  );
}
