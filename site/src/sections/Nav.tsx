import { AnimatePresence, m } from "motion/react";
import { useEffect, useState } from "react";
import { BookIcon, CloseIcon, GitHubIcon, MenuIcon, StarIcon } from "../components/Icons";
import { Wordmark } from "../components/Mochi";
import { REPO_URL } from "../lib/download";
import { DOCS_HREF, homeHref, type Route } from "../lib/route";

const LINKS = [
  { href: homeHref("#demo"), label: "Listen" },
  { href: homeHref("#built"), label: "How it's built" },
  { href: homeHref("#features"), label: "Features" },
  { href: homeHref("#compare"), label: "Compare" },
  { href: homeHref("#benchmarks"), label: "Benchmarks" },
  { href: homeHref("#faq"), label: "FAQ" },
];

export function Nav({ route }: { route: Route }) {
  const [open, setOpen] = useState(false);
  const onDocs = route.kind === "docs" || route.kind === "doc";

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open]);

  return (
    <header className="sticky top-0 z-50 pt-3">
      <div className="wrap">
        <nav aria-label="Main" className="glass flex h-[60px] items-center gap-2 rounded-full pl-4 pr-2 sm:pl-5">
          <a href={homeHref()} className="rounded-full" aria-label="Hush AI home">
            <Wordmark size={28} />
          </a>
          <ul className="mx-auto hidden items-center gap-0.5 lg:flex">
            {LINKS.map((link) => (
              <li key={link.href}>
                <a href={link.href} className="btn btn-ghost btn-sm px-3">
                  {link.label}
                </a>
              </li>
            ))}
            <li>
              <a href={DOCS_HREF} className={`btn btn-sm px-3 ${onDocs ? "btn-soft" : "btn-ghost"}`} aria-current={onDocs ? "page" : undefined}>
                Docs
              </a>
            </li>
          </ul>
          <div className="ml-auto flex items-center gap-1.5 lg:ml-0">
            <a href={REPO_URL} className="btn btn-ghost btn-sm hidden px-3 sm:inline-flex" rel="noopener" target="_blank" aria-label="Hush AI on GitHub">
              <GitHubIcon size={17} />
              <span className="hidden xl:inline">GitHub</span>
              <StarIcon size={15} className="text-amber" />
            </a>
            <a href={homeHref("#download")} className="btn btn-primary btn-sm">
              Download
            </a>
            <button
              type="button"
              className="btn btn-ghost btn-icon lg:hidden"
              aria-expanded={open}
              aria-controls="mobile-menu"
              aria-label={open ? "Close menu" : "Open menu"}
              onClick={() => setOpen((v) => !v)}
            >
              {open ? <CloseIcon size={20} /> : <MenuIcon size={20} />}
            </button>
          </div>
        </nav>
        <AnimatePresence>
          {open && (
            <m.div
              id="mobile-menu"
              initial={{ opacity: 0, y: -8, scale: 0.98 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: -8, scale: 0.98 }}
              transition={{ duration: 0.22, ease: [0.2, 0.9, 0.1, 1] }}
              className="glass mt-2 origin-top bg-white/90 p-2 lg:hidden"
            >
              <ul className="grid gap-0.5">
                {LINKS.map((link) => (
                  <li key={link.href}>
                    <a href={link.href} onClick={() => setOpen(false)} className="flex h-12 items-center rounded-2xl px-4 text-[16px] font-[560] hover:bg-black/5">
                      {link.label}
                    </a>
                  </li>
                ))}
                <li>
                  <a href={DOCS_HREF} onClick={() => setOpen(false)} className="flex h-12 items-center gap-2 rounded-2xl px-4 text-[16px] font-[560] hover:bg-black/5">
                    <BookIcon size={18} /> Docs
                  </a>
                </li>
                <li>
                  <a href={REPO_URL} target="_blank" rel="noopener" className="flex h-12 items-center gap-2 rounded-2xl px-4 text-[16px] font-[560] hover:bg-black/5">
                    <GitHubIcon size={18} /> Star on GitHub
                  </a>
                </li>
              </ul>
            </m.div>
          )}
        </AnimatePresence>
      </div>
    </header>
  );
}
