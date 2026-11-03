// Enhance a recording: drop a file (or record), Hush cleans it on the computer, compare, save.
// The window below is a looping picture; its timeline is in styles.css (enh-*).
import { CheckIcon, DownloadIcon, MicIcon, PauseIcon, PlayIcon, SparkleIcon } from "../components/Icons";
import { Mark } from "../components/Mochi";
import { Reveal } from "../components/ui";
import { useLoop } from "../lib/hooks";

const FORMATS = ["WAV", "MP3", "M4A / AAC", "FLAC", "OGG"];
const STEPS = [
  { title: "Drop a file, or record yourself", body: "An interview, a lecture, a voice note or a podcast take." },
  { title: "Hush cleans it on your computer", body: "Nothing is uploaded. No account, no daily limit." },
  { title: "Compare before and after", body: "Switch between the original and the clean version while it plays." },
  { title: "Save a clean WAV", body: "Ready for your editor, your podcast or your message." },
];

// Two waveforms of the same recording: noise everywhere, then voice only.
const N = 46;
const NOISY = Array.from({ length: N }, (_, i) => 0.28 + 0.62 * Math.abs(Math.sin(i * 0.55) * Math.cos(i * 0.21)) + ((i * 7) % 5) * 0.03);
const CLEAN = NOISY.map((v, i) => {
  const voice = Math.sin(i * 0.32) > -0.15;
  return voice ? Math.max(0.12, v * 0.92) : 0.04;
});

function Wave({ values, className, color }: { values: number[]; className: string; color: (i: number) => string }) {
  return (
    <div className={`absolute inset-0 flex items-center gap-[3px] ${className}`}>
      {values.map((v, i) => (
        <span key={i} className="h-full flex-1 rounded-full" style={{ transform: `scaleY(${v.toFixed(3)})`, background: color(i) }} />
      ))}
    </div>
  );
}

function EnhanceMock() {
  const { ref, paused, toggle } = useLoop<HTMLDivElement>();
  return (
    <div className="relative">
      <div
        ref={ref}
        aria-hidden
        className="enh relative overflow-hidden rounded-[20px] bg-white text-left"
        style={{ boxShadow: "0 0 0 1px rgba(30,27,24,.08), 0 30px 70px -30px rgba(30,27,24,.45), 0 20px 50px -30px rgba(232,115,90,.35)" }}
      >
        <div className="flex h-10 items-center gap-1.5 border-b border-line px-4" style={{ background: "#f9f7f3" }}>
          <span className="h-2.5 w-2.5 rounded-full bg-[#ff8a7a]" />
          <span className="h-2.5 w-2.5 rounded-full bg-[#ffd27a]" />
          <span className="h-2.5 w-2.5 rounded-full bg-[#8fdc9f]" />
          <span className="ml-3 flex items-center gap-1.5 text-[12px] font-[620] text-muted">
            <Mark size={14} /> Hush AI · Enhance a recording
          </span>
        </div>
        <div className="relative h-[330px] p-5 sm:h-[340px]">
          {/* 1. Drop zone */}
          <div className="enh-zone absolute inset-5 grid place-items-center rounded-[18px] border-[1.5px] border-dashed" style={{ borderColor: "rgba(30,27,24,.18)" }}>
            <div className="enh-hl absolute -inset-[1.5px] rounded-[18px]" style={{ boxShadow: "inset 0 0 0 2px rgba(31,157,114,.6)", background: "rgba(201,242,212,.35)" }} />
            <div className="relative text-center">
              <span className="mx-auto grid h-12 w-12 place-items-center rounded-full" style={{ background: "var(--h-opal)" }}>
                <DownloadIcon size={20} className="rotate-180" />
              </span>
              <div className="mt-3 text-[15px] font-[650]">Drop an audio file</div>
              <div className="mt-1 text-[12px] text-muted">WAV, MP3, M4A, FLAC or OGG</div>
              <div className="mt-4 flex justify-center gap-2 text-[12px] font-[600]">
                <span className="rounded-full bg-ink px-3 py-1.5 text-white">Choose a file</span>
                <span className="flex items-center gap-1.5 rounded-full px-3 py-1.5" style={{ boxShadow: "inset 0 0 0 1px var(--h-line-2)" }}>
                  <span className="h-2 w-2 rounded-full bg-coral" /> Record
                </span>
              </div>
            </div>
          </div>
          <div className="enh-file absolute left-1/2 top-[44%] flex items-center gap-2 rounded-[12px] bg-white px-3 py-2 text-[12.5px] font-[620]" style={{ boxShadow: "0 0 0 1px rgba(30,27,24,.1), 0 16px 30px -14px rgba(30,27,24,.45)" }}>
            <span className="grid h-7 w-7 place-items-center rounded-[8px] bg-[#f4efe8] text-ink-2">
              <MicIcon size={14} />
            </span>
            interview.m4a
            <svg width="16" height="16" viewBox="0 0 24 24" className="absolute -bottom-3 -right-3">
              <path d="M5 3l14 8-6.5 1.5L9 19z" fill="#1e1b18" stroke="#fff" strokeWidth="1.5" strokeLinejoin="round" />
            </svg>
          </div>

          {/* 2–4. Working, compare, save */}
          <div className="enh-work absolute inset-5 flex flex-col">
            <div className="flex items-center gap-3">
              <span className="grid h-9 w-9 place-items-center rounded-[10px] bg-[#f4efe8] text-ink-2">
                <MicIcon size={16} />
              </span>
              <div className="min-w-0 flex-1">
                <div className="truncate text-[14px] font-[650]">interview.m4a</div>
                <div className="relative h-4 text-[12px] text-muted">
                  <span className="enh-st-a absolute inset-0">Cleaning on this computer…</span>
                  <span className="enh-st-b absolute inset-0 flex items-center gap-1 font-[600] text-green">
                    <CheckIcon size={13} strokeWidth={2.6} /> Clean · 4:12
                  </span>
                </div>
              </div>
            </div>
            <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-black/[.06]">
              <div className="enh-prog h-full w-full origin-left rounded-full" style={{ background: "var(--h-opal-strong)" }} />
            </div>
            <div className="relative mt-4 h-[96px] sm:h-[110px]">
              <Wave values={NOISY} className="enh-noisy" color={() => "rgba(74,69,63,.55)"} />
              <Wave values={CLEAN} className="enh-clean" color={(i) => ["#e8735a", "#e3913a", "#d99a1e", "#8cc58a", "#1f9d72", "#178f8a"][Math.floor((i / N) * 6)]} />
            </div>
            <div className="mt-auto flex items-center gap-3">
              <div className="relative flex h-9 w-[190px] rounded-full p-[3px] text-[12px] font-[600]" style={{ background: "rgba(30,27,24,.06)" }}>
                <span className="enh-thumb absolute bottom-[3px] left-[3px] top-[3px] w-[calc(50%-3px)] rounded-full bg-white" style={{ boxShadow: "0 1px 2px rgba(30,27,24,.1), 0 4px 10px -4px rgba(30,27,24,.2)" }} />
                <span className="relative flex flex-1 items-center justify-center">Original</span>
                <span className="relative flex flex-1 items-center justify-center">With Hush</span>
              </div>
              <span className="enh-save ml-auto flex items-center gap-1.5 rounded-full bg-ink px-3.5 py-2 text-[12px] font-[620] text-white">
                <DownloadIcon size={14} /> Save WAV
              </span>
            </div>
            <div className="enh-toast absolute -bottom-1 left-1/2 flex -translate-x-1/2 items-center gap-1.5 whitespace-nowrap rounded-full bg-white px-3 py-1.5 text-[12px] font-[620]" style={{ boxShadow: "0 0 0 1px rgba(31,157,114,.25), 0 12px 26px -12px rgba(31,157,114,.6)" }}>
              <CheckIcon size={13} strokeWidth={2.6} className="text-green" /> Saved interview (Hush).wav
            </div>
          </div>
        </div>
      </div>
      <button
        type="button"
        onClick={toggle}
        className="absolute right-3 top-[7px] grid h-7 w-7 cursor-pointer place-items-center rounded-full text-ink-2 hover:bg-black/5"
        aria-label={paused ? "Play the recording animation" : "Pause the recording animation"}
      >
        {paused ? <PlayIcon size={12} /> : <PauseIcon size={12} />}
      </button>
    </div>
  );
}

export function Enhance() {
  return (
    <section id="enhance" aria-labelledby="enhance-title" className="relative py-20 sm:py-28">
      <div className="wrap grid items-center gap-12 lg:grid-cols-[1fr_1.1fr] lg:gap-16">
        <Reveal>
          <span className="eyebrow">New · Enhance a recording</span>
          <h2 id="enhance-title" className="heading mt-5 text-[34px] sm:text-[44px] lg:text-[52px]">
            Clean up a recording. <span className="opal-text">Keep it here.</span>
          </h2>
          <p className="lede mt-4 text-[17px] sm:text-[19px]">
            Drag an audio file into Hush, or record yourself, and Hush removes the noise on your own computer. The same job as Adobe Podcast Enhance Speech,
            with no upload, no account and no daily limit.
          </p>
          <ol className="mt-8 space-y-3">
            {STEPS.map((s, i) => (
              <li key={s.title} className="flex gap-4">
                <span className="grid h-8 w-8 shrink-0 place-items-center rounded-full bg-ink text-[13.5px] font-[650] text-white">{i + 1}</span>
                <span>
                  <span className="block text-[16px] font-[620] tracking-[-0.01em]">{s.title}</span>
                  <span className="block text-[14.5px] leading-relaxed text-ink-2">{s.body}</span>
                </span>
              </li>
            ))}
          </ol>
          <ul className="mt-7 flex flex-wrap items-center gap-1.5" aria-label="Formats">
            <li className="mr-1 flex items-center gap-1.5 text-[13px] font-[600] text-muted">
              <SparkleIcon size={15} /> Opens
            </li>
            {FORMATS.map((f) => (
              <li key={f} className="chip bg-white" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
                {f}
              </li>
            ))}
          </ul>
        </Reveal>
        <Reveal delay={0.1}>
          <EnhanceMock />
        </Reveal>
      </div>
    </section>
  );
}
