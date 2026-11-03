import { AgentIcon, ArrowRightIcon, ChartIcon, CheckIcon, GitHubIcon, LockIcon, SparkleIcon, TranslateIcon } from "../components/Icons";
import { Reveal, SectionHeader } from "../components/ui";
import { REPO_URL } from "../lib/download";

const ROADMAP = [
  { icon: SparkleIcon, title: "Noise cancellation", done: true, preview: false },
  { icon: TranslateIcon, title: "Voice translation", done: true, preview: true },
  { icon: AgentIcon, title: "Agent assist", done: false, preview: false },
  { icon: ChartIcon, title: "Speech analytics", done: false, preview: false },
  { icon: LockIcon, title: "Voice security", done: false, preview: false },
];

export function Roadmap() {
  return (
    <section id="roadmap" aria-labelledby="roadmap-title" className="relative py-20 sm:py-28">
      <div className="wrap">
        <SectionHeader
          id="roadmap-title"
          eyebrow="Roadmap"
          title={
            <>
              Noise cancellation is <span className="opal-text">just the start</span>
            </>
          }
          sub="Noise cancellation is here, and voice translation is in preview. The next ones are open for ideas and for help."
        />

        <Reveal className="mx-auto mt-12 max-w-[760px]">
          <div className="glass p-3 sm:p-4">
            <ul aria-label="Roadmap">
              {ROADMAP.map(({ icon: Icon, title, done, preview }, i) => (
                <li
                  key={title}
                  className={`flex items-center gap-4 rounded-[20px] px-3 py-3.5 sm:px-4 sm:py-4 ${i ? "border-t border-line" : ""}`}
                  style={done ? { background: "linear-gradient(100deg, rgba(201,242,212,.55), rgba(155,230,224,.3))", borderTopColor: "transparent" } : undefined}
                >
                  <span
                    aria-hidden
                    className="grid h-7 w-7 shrink-0 place-items-center rounded-full"
                    style={done ? { background: "var(--h-green)", color: "#fff" } : { boxShadow: "inset 0 0 0 1.8px rgba(30,27,24,.22)" }}
                  >
                    {done && <CheckIcon size={16} strokeWidth={2.6} />}
                  </span>
                  <span className={`hidden h-11 w-11 shrink-0 place-items-center rounded-[14px] sm:grid ${done ? "bg-white text-green" : "bg-[#f4efe8] text-muted"}`}>
                    <Icon size={22} />
                  </span>
                  <span className={`min-w-0 flex-1 text-[17px] sm:text-[19px] ${done ? "font-[650] tracking-[-0.02em]" : "font-[560] text-ink-2"}`}>{title}</span>
                  <span className={`chip shrink-0 ${done ? "chip-green" : ""}`}>
                    {done ? (
                      <>
                        <span className="dot" /> {preview ? "Preview" : "Available"}
                      </>
                    ) : (
                      "Coming soon"
                    )}
                  </span>
                </li>
              ))}
            </ul>
          </div>
        </Reveal>

        <Reveal className="mx-auto mt-8 max-w-[760px]">
          <div className="flex flex-col items-start gap-4 rounded-[24px] px-6 py-5 sm:flex-row sm:items-center" style={{ background: "rgba(30,27,24,.04)" }}>
            <p className="flex-1 text-[16px] leading-relaxed text-ink-2">
              <strong className="font-[640] text-ink">Want to help build the next one?</strong> Ideas, issues and pull requests are all welcome.
            </p>
            <a href={`${REPO_URL}/issues`} target="_blank" rel="noopener" className="btn btn-primary group shrink-0">
              <GitHubIcon size={18} /> Contribute <ArrowRightIcon size={17} />
            </a>
          </div>
        </Reveal>
      </div>
    </section>
  );
}
