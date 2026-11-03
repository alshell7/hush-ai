// Hush AI next to Krisp (live calls) and Adobe Podcast Enhance Speech (recordings). Only facts
// from the vendors' public pages; no claim about how either of them sounds.
import type { ReactNode } from "react";
import { CheckIcon, CloseIcon } from "../components/Icons";
import { Mochi } from "../components/Mochi";
import { Reveal, SectionHeader } from "../components/ui";

type Cell = { text: string; mark?: "yes" | "no" | "none" };
const COLUMNS = [
  { id: "hush", name: "Hush AI", note: "Free and open source" },
  { id: "krisp", name: "Krisp", note: "For live calls" },
  { id: "adobe", name: "Adobe Podcast Enhance Speech", note: "For recordings" },
] as const;

const ROWS: { label: string; cells: [Cell, Cell, Cell] }[] = [
  {
    label: "Price",
    cells: [{ text: "Free, no limits", mark: "yes" }, { text: "7-day free trial, then paid plans (Core $8 per user a month, billed yearly)" }, { text: "Free with daily limits; more with a paid Adobe plan" }],
  },
  { label: "Account", cells: [{ text: "None", mark: "yes" }, { text: "Sign-up required" }, { text: "Adobe account" }] },
  {
    label: "Where your audio is processed",
    cells: [{ text: "Your computer", mark: "yes" }, { text: "Your computer (noise cancellation)", mark: "yes" }, { text: "Adobe's servers: you upload the file" }],
  },
  { label: "Open source", cells: [{ text: "Yes, Apache-2.0", mark: "yes" }, { text: "No", mark: "no" }, { text: "No", mark: "no" }] },
  { label: "Live calls in any app", cells: [{ text: "Yes", mark: "yes" }, { text: "Yes", mark: "yes" }, { text: "No", mark: "no" }] },
  { label: "Clean up recordings", cells: [{ text: "Yes, on your computer", mark: "yes" }, { text: "–", mark: "none" }, { text: "Yes, in the browser", mark: "yes" }] },
];

function Mark({ mark }: { mark?: Cell["mark"] }): ReactNode {
  if (mark === "yes")
    return (
      <span aria-hidden className="mt-[1px] grid h-[18px] w-[18px] shrink-0 place-items-center rounded-full" style={{ background: "rgba(31,157,114,.14)", color: "#157353" }}>
        <CheckIcon size={12} strokeWidth={2.8} />
      </span>
    );
  if (mark === "no")
    return (
      <span aria-hidden className="mt-[1px] grid h-[18px] w-[18px] shrink-0 place-items-center rounded-full bg-black/[.05] text-muted">
        <CloseIcon size={11} strokeWidth={2.6} />
      </span>
    );
  return null;
}

export function Compare() {
  return (
    <section id="compare" aria-labelledby="compare-title" className="relative py-20 sm:py-28">
      <div className="wrap">
        <SectionHeader
          id="compare-title"
          eyebrow="Compare"
          title={
            <>
              The free, open-source <span className="opal-text">alternative</span>
            </>
          }
          sub="Hush AI does the job of Krisp for live calls and of Adobe Podcast Enhance Speech for recordings. Free, with no account, and your audio stays on your computer."
        />

        <Reveal className="mx-auto mt-12 max-w-[1040px]">
          <div className="cmp card p-2 sm:p-3" role="table" aria-labelledby="compare-title">
            <div role="rowgroup">
              <div role="row" className="cmp-row cmp-head">
                <span role="columnheader" className="cmp-label hidden md:block">
                  <span className="sr-only">Feature</span>
                </span>
                {COLUMNS.map((c) => (
                  <span key={c.id} role="columnheader" className={`cmp-cell ${c.id === "hush" ? "cmp-hush cmp-hush-top" : ""}`}>
                    <span className="flex items-center gap-2">
                      {c.id === "hush" && <Mochi size={26} breathe={false} className="hidden shrink-0 sm:block" />}
                      <span className="min-w-0">
                        <span className="block text-[13.5px] font-[680] leading-tight tracking-[-0.01em] text-ink sm:text-[15.5px]">{c.name}</span>
                        <span className="mt-0.5 hidden text-[12px] font-[520] text-muted sm:block">{c.note}</span>
                      </span>
                    </span>
                  </span>
                ))}
              </div>
            </div>
            <div role="rowgroup">
              {ROWS.map((r, i) => (
                <div key={r.label} role="row" className="cmp-row">
                  <span role="rowheader" className="cmp-label">
                    {r.label}
                  </span>
                  {r.cells.map((cell, j) => (
                    <span key={j} role="cell" className={`cmp-cell ${j === 0 ? `cmp-hush ${i === ROWS.length - 1 ? "cmp-hush-bottom" : ""}` : ""}`}>
                      <span className="flex items-start gap-2">
                        <Mark mark={cell.mark} />
                        <span className={cell.mark === "none" ? "text-faint" : ""}>
                          {cell.mark === "none" ? (
                            <>
                              <span aria-hidden>–</span>
                              <span className="sr-only">Not found on its pages</span>
                            </>
                          ) : (
                            cell.text
                          )}
                        </span>
                      </span>
                    </span>
                  ))}
                </div>
              ))}
            </div>
          </div>
          <p className="mt-4 px-1 text-[13px] leading-relaxed text-muted">
            From the vendors' public pages (
            <a className="link text-ink-2" href="https://krisp.ai/pricing/" target="_blank" rel="noopener">
              krisp.ai/pricing
            </a>
            ,{" "}
            <a className="link text-ink-2" href="https://podcast.adobe.com/en/plans" target="_blank" rel="noopener">
              podcast.adobe.com
            </a>
            ), November 2026; – means we did not find it there. We have not measured Krisp's or Adobe's audio quality. Hush's optional Live Translate (Preview) sends your voice to Google
            while it is on. Krisp and Adobe are trademarks of their
            owners, who do not endorse Hush AI.
          </p>
        </Reveal>
      </div>
    </section>
  );
}
