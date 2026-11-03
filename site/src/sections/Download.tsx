import type { ReactNode } from "react";
import { DownloadIcon, ExternalIcon, LaptopGlyph, TerminalGlyph, WindowsGlyph } from "../components/Icons";
import { Mochi } from "../components/Mochi";
import { Reveal } from "../components/ui";
import { formatSize, OS_NAME, RELEASES_URL, TARGETS, useDownloadInfo, type Desktop } from "../lib/download";
import { usePauseOffscreen } from "../lib/hooks";
import { homeHref } from "../lib/route";
import { DownloadButton } from "./Hero";

const GLYPH: Record<Desktop, ReactNode> = {
  windows: <WindowsGlyph size={20} />,
  macos: <LaptopGlyph size={22} />,
  linux: <TerminalGlyph size={22} />,
};

function PlatformCard({ os }: { os: Desktop }) {
  const info = useDownloadInfo();
  const mine = info.os === os;
  const targets = TARGETS.filter((t) => t.os === os);
  return (
    <div
      className="card flex flex-col p-5 sm:p-6"
      style={mine ? { boxShadow: "var(--h-inner), 0 0 0 2px rgba(255,174,148,.75), 0 30px 60px -28px rgba(232,115,90,.55)" } : undefined}
    >
      <div className="flex items-center gap-3">
        <span className="grid h-11 w-11 place-items-center rounded-[14px] bg-[#f4efe8] text-ink">{GLYPH[os]}</span>
        <h3 className="text-[19px] font-[650] tracking-[-0.02em]">{OS_NAME[os]}</h3>
        {mine && <span className="chip chip-green ml-auto">Your system</span>}
      </div>
      <ul className="mt-5 space-y-2">
        {targets.map((t) => {
          const asset = info.release.assets[t.id];
          const recommended = info.primary === t.id;
          return (
            <li key={t.id}>
              <a
                href={info.urlFor(t.id)}
                rel="noopener"
                className={`group flex items-center gap-3 rounded-[16px] px-4 py-3 transition-[background-color,transform] duration-200 active:scale-[0.98] ${
                  recommended ? "bg-ink text-white hover:bg-[#36312c]" : "bg-black/[.04] hover:bg-black/[.07]"
                }`}
              >
                <span className="min-w-0 flex-1">
                  <span className="block text-[15.5px] font-[620]">{t.label}</span>
                  <span className={`block text-[13px] ${recommended ? "text-white/75" : "text-muted"}`}>
                    {t.detail}
                    {asset ? ` · ${formatSize(asset.size)}` : ""}
                  </span>
                </span>
                {asset ? <DownloadIcon size={19} /> : <ExternalIcon size={17} />}
              </a>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

export function DownloadSection() {
  const info = useDownloadInfo();
  const mascotRef = usePauseOffscreen<HTMLDivElement>();
  return (
    <section id="download" aria-labelledby="download-title" className="relative py-20 sm:py-28">
      <div className="wrap">
        <Reveal>
          <div
            className="relative overflow-hidden rounded-[36px] px-4 py-14 sm:rounded-[44px] sm:px-10 sm:py-20"
            style={{ background: "linear-gradient(160deg, #fff4ee, #fffaf0 45%, #f1fbf3)", boxShadow: "var(--h-inner), var(--h-shadow-3)" }}
          >
            <div className="aurora" />
            <div className="relative text-center">
              <div ref={mascotRef} className="mx-auto w-fit" style={{ animation: "h-breathe 4.2s var(--h-ease-soft) infinite", transformOrigin: "50% 90%" }}>
                <Mochi size={92} breathe={false} />
              </div>
              <h2 id="download-title" className="heading mx-auto mt-6 max-w-[760px] text-[38px] sm:text-[56px] lg:text-[64px]">
                Give your calls <span className="opal-text">some quiet</span>
              </h2>
              <p className="lede mx-auto mt-4 max-w-[540px] text-[17px] sm:text-[19px]">
                Free for Windows, macOS and Linux. No account, no limits.
                {info.release.version && <span className="text-muted"> Version {info.release.version}.</span>}
              </p>
              {info.os === "mobile" ? (
                <p className="mx-auto mt-8 max-w-[420px] rounded-[20px] bg-white/80 px-5 py-4 text-[15.5px] text-ink-2" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
                  Hush AI is a desktop app. Open this page on your computer to download it, or pick a build below.
                </p>
              ) : (
                <div className="mt-8 flex flex-col items-center justify-center gap-3 sm:flex-row">
                  <DownloadButton />
                  <a href={RELEASES_URL} target="_blank" rel="noopener" className="btn btn-soft btn-lg group">
                    All releases <ExternalIcon size={17} />
                  </a>
                </div>
              )}
            </div>

            <div className="relative mx-auto mt-12 grid max-w-[1040px] gap-4 md:grid-cols-3">
              {(["windows", "macos", "linux"] as const).map((os) => (
                <PlatformCard key={os} os={os} />
              ))}
            </div>

            <p className="relative mx-auto mt-8 max-w-[640px] text-center text-[14.5px] leading-relaxed text-ink-2">
              Builds aren't code-signed yet, so your computer asks once before opening Hush.{" "}
              <a href={homeHref("#faq-unsigned")} className="link">
                Here's what to click
              </a>
              . On Windows, also install the free{" "}
              <a href={homeHref("#faq-vb-cable")} className="link">
                VB-CABLE
              </a>{" "}
              for now.
            </p>
          </div>
        </Reveal>
      </div>
    </section>
  );
}
