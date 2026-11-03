import { GitHubIcon, StarIcon } from "../components/Icons";
import { Wordmark } from "../components/Mochi";
import { DOCS_URL, MODELS_URL, RELEASES_URL, REPO_URL } from "../lib/download";

const COLUMNS = [
  {
    title: "Product",
    links: [
      { label: "Features", href: "#features" },
      { label: "Demo", href: "#demo" },
      { label: "How it works", href: "#how" },
      { label: "Roadmap", href: "#roadmap" },
      { label: "FAQ", href: "#faq" },
    ],
  },
  {
    title: "Project",
    links: [
      { label: "GitHub", href: REPO_URL },
      { label: "Releases", href: RELEASES_URL },
      { label: "Docs", href: DOCS_URL },
      { label: "Models on Hugging Face", href: MODELS_URL },
      { label: "Report an issue", href: `${REPO_URL}/issues` },
    ],
  },
];

export function Footer() {
  return (
    <footer className="relative border-t border-line bg-white/50">
      <div className="wrap py-14 sm:py-16">
        <div className="grid grid-cols-2 gap-10 md:grid-cols-[1.4fr_1fr_1fr]">
          <div className="col-span-2 md:col-span-1">
            <Wordmark size={30} />
            <p className="mt-4 max-w-[340px] text-[15px] leading-relaxed text-ink-2">Free, open-source noise cancellation that runs on your own computer.</p>
            <a href={REPO_URL} target="_blank" rel="noopener" className="btn btn-soft btn-sm mt-5">
              <GitHubIcon size={16} /> Star on GitHub <StarIcon size={14} className="text-amber" />
            </a>
          </div>
          {COLUMNS.map((col) => (
            <nav key={col.title} aria-label={col.title}>
              <h2 className="text-[13px] font-[600] uppercase tracking-[0.06em] text-muted">{col.title}</h2>
              <ul className="mt-4 space-y-2.5">
                {col.links.map((l) => {
                  const external = l.href.startsWith("http");
                  return (
                    <li key={l.label}>
                      <a
                        href={l.href}
                        className="text-[15px] text-ink-2 transition-colors hover:text-ink"
                        {...(external ? { target: "_blank", rel: "noopener" } : {})}
                      >
                        {l.label}
                      </a>
                    </li>
                  );
                })}
              </ul>
            </nav>
          ))}
        </div>

        <div className="mt-12 space-y-2 border-t border-line pt-6 text-[13px] leading-relaxed text-muted">
          <p>
            The Hush AI app and engine are licensed under Apache-2.0. The default voice model, DeepFilterNet 3, is MIT or Apache-2.0. The optional NVIDIA RE-USE
            weights are NVIDIA's, under the NVIDIA One-Way Noncommercial License, for non-commercial research and educational use only. Demo sample © NVIDIA.
          </p>
          <p>
            Hush AI is an independent open-source project. Zoom, Google Meet, Microsoft Teams, Discord, Slack, OBS Studio and VB-CABLE are trademarks of their
            owners.
          </p>
        </div>
      </div>
    </footer>
  );
}
