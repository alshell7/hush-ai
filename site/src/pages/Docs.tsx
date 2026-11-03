// Docs: an index at /docs/ and one page per Markdown file at /docs/<slug>/. The HTML is
// rendered at build time (scripts/docs.mjs); this adds the sidebar, the table of contents,
// copy buttons, prev/next links and, where a page has them, Mermaid diagrams.
import { useEffect, useRef, useState, type ReactNode } from "react";
import { DOCS, type DocContent, type DocMeta } from "virtual:hush-docs";
import {
  ArrowRightIcon,
  BoltIcon,
  BookIcon,
  ChartIcon,
  ChevronDownIcon,
  CodeIcon,
  CpuIcon,
  ExternalIcon,
  GitHubIcon,
  HeartIcon,
  MicIcon,
  ShieldIcon,
  SparkleIcon,
  TargetIcon,
} from "../components/Icons";
import { Mochi } from "../components/Mochi";
import { REPO_URL } from "../lib/download";
import { DOCS_HREF, docHref } from "../lib/route";

const ICONS: Record<string, (p: { size?: number }) => ReactNode> = {
  home: SparkleIcon,
  book: BookIcon,
  flow: ChartIcon,
  cpu: CpuIcon,
  mic: MicIcon,
  chart: ChartIcon,
  gauge: BoltIcon,
  target: TargetIcon,
  code: CodeIcon,
  heart: HeartIcon,
  shield: ShieldIcon,
  globe: ExternalIcon,
};

const TINTS = ["#ffe2d6", "#fff0cc", "#dcf6e3", "#d3f3ef", "#ffe0ea"];

function groups(): [string, DocMeta[]][] {
  const out = new Map<string, DocMeta[]>();
  for (const d of DOCS) out.set(d.group, [...(out.get(d.group) ?? []), d]);
  return [...out];
}

function DocIcon({ doc, size = 18 }: { doc: DocMeta; size?: number }) {
  const Icon = ICONS[doc.icon] ?? BookIcon;
  return <Icon size={size} />;
}

// ------------------------------------------------------------------ Index

export function DocsIndex() {
  let n = 0;
  return (
    <div className="wrap pb-20 pt-12 sm:pb-28 sm:pt-20">
      <header className="mx-auto max-w-[760px] text-center">
        <span className="eyebrow">Documentation</span>
        <h1 className="display mt-6 text-[44px] sm:text-[64px]">
          Everything about <span className="opal-text">Hush</span>
        </h1>
        <p className="lede mx-auto mt-5 max-w-[600px] text-[17px] sm:text-[19px]">
          Plain guides for everyone, and the full engineering story for the curious: how audio flows, how it was measured, and how to build it yourself.
        </p>
        <div className="mt-8 flex flex-col items-center justify-center gap-3 sm:flex-row">
          <a href={docHref("guide")} className="btn btn-primary btn-lg">
            <BookIcon size={18} /> Start with the user guide
          </a>
          <a href={REPO_URL} target="_blank" rel="noopener" className="btn btn-soft btn-lg">
            <GitHubIcon size={18} /> Source on GitHub
          </a>
        </div>
      </header>

      <div className="mx-auto mt-16 max-w-[1080px] space-y-12">
        {groups().map(([group, docs]) => (
          <section key={group} aria-labelledby={`group-${group}`}>
            <h2 id={`group-${group}`} className="text-[13px] font-[650] uppercase tracking-[0.08em] text-muted">
              {group}
            </h2>
            <ul className="mt-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
              {docs.map((d) => (
                <li key={d.slug} className="flex">
                  <a href={docHref(d.slug)} className="doc-card card group flex w-full flex-col p-5 sm:p-6">
                    <span className="grid h-11 w-11 place-items-center rounded-[14px] text-ink" style={{ background: TINTS[n++ % TINTS.length] }}>
                      <DocIcon doc={d} size={20} />
                    </span>
                    <span className="mt-4 text-[18px] font-[650] tracking-[-0.02em]">{d.title}</span>
                    <span className="mt-1.5 flex-1 text-[14.5px] leading-relaxed text-ink-2">{d.summary}</span>
                    <span className="mt-4 flex items-center gap-1.5 text-[13px] font-[580] text-muted">
                      {d.minutes} min read
                      <ArrowRightIcon size={15} className="ml-auto text-ink transition-transform duration-300 group-hover:translate-x-1" />
                    </span>
                  </a>
                </li>
              ))}
            </ul>
          </section>
        ))}
      </div>
    </div>
  );
}

// ------------------------------------------------------------------ One page

function Sidebar({ current }: { current: string }) {
  return (
    <nav aria-label="Docs">
      <a href={DOCS_HREF} className="flex items-center gap-2 rounded-[12px] px-3 py-2 text-[14px] font-[650] text-ink hover:bg-black/[.04]">
        <BookIcon size={16} /> All docs
      </a>
      {groups().map(([group, docs]) => (
        <div key={group} className="mt-5">
          <div className="px-3 text-[11.5px] font-[650] uppercase tracking-[0.08em] text-faint">{group}</div>
          <ul className="mt-1.5 space-y-0.5">
            {docs.map((d) => {
              const on = d.slug === current;
              return (
                <li key={d.slug}>
                  <a
                    href={docHref(d.slug)}
                    aria-current={on ? "page" : undefined}
                    className={`flex items-center gap-2.5 rounded-[12px] px-3 py-[7px] text-[14px] transition-colors ${on ? "bg-white font-[640] text-ink" : "font-[520] text-ink-2 hover:bg-black/[.04] hover:text-ink"}`}
                    style={on ? { boxShadow: "inset 0 0 0 1px var(--h-line), var(--h-shadow-1)" } : undefined}
                  >
                    <span className={on ? "text-coral" : "text-faint"}>
                      <DocIcon doc={d} size={15} />
                    </span>
                    {d.title}
                  </a>
                </li>
              );
            })}
          </ul>
        </div>
      ))}
    </nav>
  );
}

function Toc({ toc, active }: { toc: DocContent["toc"]; active: string | null }) {
  if (!toc.length) return null;
  return (
    <nav aria-label="On this page">
      <div className="text-[11.5px] font-[650] uppercase tracking-[0.08em] text-faint">On this page</div>
      <ul className="mt-3 space-y-1 border-l border-line">
        {toc.map((t) => (
          <li key={t.id}>
            <a
              href={`#${t.id}`}
              className={`-ml-px block border-l-2 py-1 text-[13.5px] leading-snug transition-colors ${t.depth === 3 ? "pl-6" : "pl-3.5"} ${
                active === t.id ? "border-[var(--h-coral)] font-[600] text-ink" : "border-transparent text-muted hover:text-ink"
              }`}
            >
              {t.text}
            </a>
          </li>
        ))}
      </ul>
    </nav>
  );
}

export function DocPage({ doc }: { doc: DocContent }) {
  const meta = DOCS.find((d) => d.slug === doc.slug)!;
  const index = DOCS.indexOf(meta);
  const prev = index > 0 ? DOCS[index - 1] : null;
  const next = index < DOCS.length - 1 ? DOCS[index + 1] : null;
  const bodyRef = useRef<HTMLDivElement>(null);
  const [active, setActive] = useState<string | null>(null);

  // Copy buttons on code blocks.
  useEffect(() => {
    const body = bodyRef.current;
    if (!body) return;
    const onClick = async (e: MouseEvent) => {
      const btn = (e.target as HTMLElement).closest<HTMLButtonElement>("[data-copy]");
      if (!btn) return;
      const code = btn.closest(".code")?.querySelector("pre")?.textContent ?? "";
      try {
        await navigator.clipboard.writeText(code);
        btn.textContent = "Copied";
      } catch {
        btn.textContent = "Press Ctrl+C";
      }
      window.setTimeout(() => (btn.textContent = "Copy"), 1600);
    };
    body.addEventListener("click", onClick);
    return () => body.removeEventListener("click", onClick);
  }, []);

  // The section you are reading, for the table of contents.
  useEffect(() => {
    const body = bodyRef.current;
    if (!body || !doc.toc.length) return;
    const heads = doc.toc.map((t) => document.getElementById(t.id)).filter((x): x is HTMLElement => !!x);
    const io = new IntersectionObserver(
      (entries) => {
        const visible = entries.filter((e) => e.isIntersecting).sort((a, b) => a.boundingClientRect.top - b.boundingClientRect.top);
        if (visible[0]) setActive(visible[0].target.id);
      },
      { rootMargin: "-90px 0px -65% 0px" },
    );
    heads.forEach((h) => io.observe(h));
    return () => io.disconnect();
  }, [doc]);

  // Mermaid, only on pages with diagrams and only when one comes near.
  useEffect(() => {
    const body = bodyRef.current;
    const figures = body ? [...body.querySelectorAll<HTMLElement>("figure[data-mermaid]")] : [];
    if (!figures.length) return;
    let done = false;
    const io = new IntersectionObserver(
      (entries) => {
        if (done || !entries.some((e) => e.isIntersecting)) return;
        done = true;
        io.disconnect();
        import("../lib/mermaid").then((m) => m.renderDiagrams(figures)).catch(() => undefined);
      },
      { rootMargin: "400px" },
    );
    figures.forEach((f) => io.observe(f));
    return () => io.disconnect();
  }, [doc]);

  return (
    <div className="wrap pb-20 pt-8 sm:pb-28 sm:pt-12">
      <div className="docs-grid">
        <aside className="docs-side hidden lg:block">
          <div className="sticky top-[96px] max-h-[calc(100vh-112px)] overflow-y-auto pb-6 pr-2">
            <Sidebar current={doc.slug} />
          </div>
        </aside>

        <div className="min-w-0">
          <details className="docs-mobile-nav card mb-6 lg:hidden">
            <summary className="flex h-12 cursor-pointer list-none items-center gap-2 px-4 text-[14.5px] font-[620]">
              <BookIcon size={16} /> {meta.group} <span className="text-faint">›</span> <span className="truncate">{meta.title}</span>
              <ChevronDownIcon size={18} className="docs-chevron ml-auto shrink-0 text-muted transition-transform" />
            </summary>
            <div className="border-t border-line p-2 pb-4">
              <Sidebar current={doc.slug} />
            </div>
          </details>

          <article aria-labelledby="doc-title">
            <header className="border-b border-line pb-7">
              <div className="flex items-center gap-2 text-[13px] font-[600] text-muted">
                <a href={DOCS_HREF} className="hover:text-ink">
                  Docs
                </a>
                <span className="text-faint">›</span>
                <span>{meta.group}</span>
              </div>
              <div className="mt-4 flex items-start gap-4">
                <span className="mt-1 hidden h-12 w-12 shrink-0 place-items-center rounded-[16px] text-ink sm:grid" style={{ background: "var(--h-opal)" }}>
                  <DocIcon doc={meta} size={22} />
                </span>
                <div className="min-w-0">
                  <h1 id="doc-title" className="heading text-[34px] sm:text-[46px]">
                    {meta.title}
                  </h1>
                  <p className="lede mt-3 text-[16.5px] sm:text-[18px]">{meta.summary}</p>
                </div>
              </div>
              <div className="mt-5 flex flex-wrap items-center gap-x-4 gap-y-2 text-[13px] font-[560] text-muted">
                <span>{meta.minutes} min read</span>
                <a className="flex items-center gap-1.5 hover:text-ink" href={`${REPO_URL}/blob/main/${meta.file}`} target="_blank" rel="noopener">
                  <GitHubIcon size={14} /> {meta.file}
                </a>
                <a className="flex items-center gap-1.5 hover:text-ink" href={`${REPO_URL}/edit/main/${meta.file}`} target="_blank" rel="noopener">
                  Suggest a change <ExternalIcon size={13} />
                </a>
              </div>
            </header>

            <div ref={bodyRef} className="prose mt-8" dangerouslySetInnerHTML={{ __html: doc.html }} />

            <nav aria-label="More docs" className="mt-14 grid gap-3 sm:grid-cols-2">
              {prev ? (
                <a href={docHref(prev.slug)} className="doc-card card group flex flex-col p-5">
                  <span className="text-[12.5px] font-[600] text-muted">← Previous</span>
                  <span className="mt-1 text-[16.5px] font-[650] tracking-[-0.02em]">{prev.title}</span>
                </a>
              ) : (
                <span className="hidden sm:block" />
              )}
              {next && (
                <a href={docHref(next.slug)} className="doc-card card group flex flex-col p-5 text-right">
                  <span className="text-[12.5px] font-[600] text-muted">Next →</span>
                  <span className="mt-1 text-[16.5px] font-[650] tracking-[-0.02em]">{next.title}</span>
                </a>
              )}
            </nav>

            <div className="mt-10 flex items-center gap-3 rounded-[22px] px-5 py-4 text-[14.5px] text-ink-2" style={{ background: "rgba(30,27,24,.04)" }}>
              <Mochi size={36} breathe={false} />
              <p>
                Something unclear or out of date?{" "}
                <a className="link" href={`${REPO_URL}/issues/new/choose`} target="_blank" rel="noopener">
                  Tell us
                </a>{" "}
                or{" "}
                <a className="link" href={`${REPO_URL}/edit/main/${meta.file}`} target="_blank" rel="noopener">
                  edit this page
                </a>
                .
              </p>
            </div>
          </article>
        </div>

        <aside className="docs-toc hidden xl:block">
          <div className="sticky top-[96px] max-h-[calc(100vh-112px)] overflow-y-auto pb-6">
            <Toc toc={doc.toc} active={active} />
          </div>
        </aside>
      </div>
    </div>
  );
}
