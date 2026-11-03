import { DOCS } from "virtual:hush-docs";
import { GitHubIcon, StarIcon } from "../components/Icons";
import { Wordmark } from "../components/Mochi";
import { MODELS_URL, RELEASES_URL, REPO_URL, REUSE_LICENSE_URL } from "../lib/download";
import { DOCS_HREF, docHref, homeHref } from "../lib/route";

const COLUMNS = [
  {
    title: "Product",
    links: [
      { label: "Hear the difference", href: homeHref("#demo") },
      { label: "Features", href: homeHref("#features") },
      { label: "Compare", href: homeHref("#compare") },
      { label: "Enhance a recording", href: homeHref("#enhance") },
      { label: "Live Translate", href: homeHref("#translate") },
      { label: "Google Calendar", href: homeHref("#calendar") },
      { label: "Benchmarks", href: homeHref("#benchmarks") },
      { label: "How it's built", href: homeHref("#built") },
      { label: "Download", href: homeHref("#download") },
    ],
  },
  {
    title: "Docs",
    links: [{ label: "All docs", href: DOCS_HREF }, ...DOCS.slice(0, 8).map((d) => ({ label: d.title, href: docHref(d.slug) }))],
  },
  {
    title: "Project",
    links: [
      { label: "GitHub", href: REPO_URL },
      { label: "Releases", href: RELEASES_URL },
      { label: "Models on Hugging Face", href: MODELS_URL },
      { label: "Report an issue", href: `${REPO_URL}/issues` },
      { label: "Contributing", href: docHref("contributing") },
      { label: "Security", href: docHref("security") },
      { label: "Roadmap", href: homeHref("#roadmap") },
      { label: "FAQ", href: homeHref("#faq") },
    ],
  },
];

export function Footer() {
  return (
    <footer className="relative border-t border-line bg-white/50">
      <div className="wrap py-14 sm:py-16">
        <div className="grid grid-cols-2 gap-10 md:grid-cols-[1.3fr_1fr_1fr_1fr]">
          <div className="col-span-2 md:col-span-1">
            <Wordmark size={30} />
            <p className="mt-4 max-w-[340px] text-[15px] leading-relaxed text-ink-2">
              Free, open-source noise cancellation for live calls and recordings. It runs on your own computer.
            </p>
            <a href={REPO_URL} target="_blank" rel="noopener" className="btn btn-soft btn-sm mt-5">
              <GitHubIcon size={16} /> Star on GitHub <StarIcon size={14} className="text-amber" />
            </a>
          </div>
          {COLUMNS.map((col) => (
            <nav key={col.title} aria-label={col.title} className={col.title === "Project" ? "col-span-2 sm:col-span-1" : ""}>
              <h2 className="text-[13px] font-[600] uppercase tracking-[0.06em] text-muted">{col.title}</h2>
              <ul className="mt-4 space-y-2.5">
                {col.links.map((l) => {
                  const external = l.href.startsWith("http");
                  return (
                    <li key={l.label}>
                      <a href={l.href} className="text-[15px] text-ink-2 transition-colors hover:text-ink" {...(external ? { target: "_blank", rel: "noopener" } : {})}>
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
            The Hush AI app and engine are licensed under Apache-2.0. DeepFilterNet 3 is MIT or Apache-2.0.{" "}
            <a className="link text-ink-2" href={REUSE_LICENSE_URL} target="_blank" rel="noopener">
              NVIDIA RE-USE model license
            </a>
            . Demo samples are credited next to the player.
          </p>
          <p>
            Hush AI is an independent open-source project. Zoom, Google Meet, Google Calendar, Microsoft Teams, Slack, Discord, Webex, OBS Studio, WhatsApp,
            Telegram, Krisp, Adobe Podcast and VB-CABLE are trademarks of their owners, who do not endorse Hush AI. Their names and logos are used only to show
            compatibility or to compare.
          </p>
        </div>
      </div>
    </footer>
  );
}
