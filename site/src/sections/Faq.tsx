import { AnimatePresence, m } from "motion/react";
import { useEffect, useState, type ReactNode } from "react";
import { ChevronDownIcon } from "../components/Icons";
import { Reveal, SectionHeader } from "../components/ui";
import { REPO_URL, REUSE_LICENSE_URL } from "../lib/download";
import { docHref } from "../lib/route";


const FAQ: { id: string; q: string; a: ReactNode }[] = [
  {
    id: "free",
    q: "Is it really free?",
    a: (
      <p>
        Yes. Hush AI is free and open source, with no account, no trial, no subscription and no minute limits. The code is on{" "}
        <a className="link" href={REPO_URL} target="_blank" rel="noopener">
          GitHub
        </a>
        .
      </p>
    ),
  },
  {
    id: "vs",
    q: "Does Hush sound better than Krisp or Adobe Podcast?",
    a: (
      <>
        <p>
          We have not measured Krisp or Adobe Podcast, so we don't claim that. What we can say is how they differ: Hush is free with no limits, open source,
          needs no account, and keeps your audio on your computer. Krisp's noise cancellation also runs on your device, but it needs an account and a paid plan
          after a 7-day trial. Adobe Podcast Enhance Speech works on files you upload to Adobe.
        </p>
        <p>
          The fairest test is your own voice: Hush's setup records a few seconds and plays them back with and without Hush. Our own measurements are in the{" "}
          <a className="link" href={docHref("comparison")}>
            comparison
          </a>
          .
        </p>
      </>
    ),
  },
  {
    id: "privacy",
    q: "Does my audio leave my computer?",
    a: (
      <p>
        No. Hush cleans your microphone and your recordings on your own CPU and sends no usage data anywhere. It goes online only to download a voice model
        once (and check its SHA-256), to check for updates, to read your calendar if you connect one, and to reach Google while you use Live Translate.
      </p>
    ),
  },
  {
    id: "translate-privacy",
    q: "Does Live Translate leave my computer?",
    a: (
      <p>
        Yes, and it is the only Hush feature that does. While Live Translate is on, Hush sends your noise-cancelled voice to Google's Gemini Live Translate model
        and plays the translated voice into Hush AI Microphone. When it is off, nothing goes to Google. Hush itself has no server. Translation needs an internet
        connection and follows about a sentence behind you.
      </p>
    ),
  },
  {
    id: "translate-cost",
    q: "What does Live Translate cost?",
    a: (
      <p>
        Hush is free. Live Translate uses your own Gemini API key, which Hush helps you create in Google AI Studio. Google offers a free tier; paid use is priced
        by Google.
      </p>
    ),
  },
  {
    id: "engines",
    q: "Which engine should I use?",
    a: (
      <>
        <p>
          <strong className="font-[620]">DeepFilterNet 3</strong>, the default, for almost everyone: it uses about 6% of one CPU core and scored highest on
          PESQ, STOI and SI-SDR in our test.
        </p>
        <p>
          <strong className="font-[620]">NVIDIA RE-USE</strong> if you want to try NVIDIA's model. It needs much more CPU. You can switch in Settings at any
          time.
        </p>
      </>
    ),
  },
  {
    id: "license",
    q: "Can I use Hush at work?",
    a: (
      <p>
        Yes: Hush AI and its default engine, DeepFilterNet 3, are open source (Apache-2.0, MIT). The optional NVIDIA engine has its own license terms, which the
        app shows before it downloads:{" "}
        <a className="link" href={REUSE_LICENSE_URL} target="_blank" rel="noopener">
          NVIDIA RE-USE model license
        </a>
        .
      </p>
    ),
  },
  {
    id: "platforms",
    q: "Which computers does it run on?",
    a: (
      <ul className="list-disc space-y-1 pl-5">
        <li>Windows 10 and 11, on x64 and ARM64 PCs.</li>
        <li>macOS 11 or later, on Apple silicon and Intel Macs.</li>
        <li>Linux with PipeWire or PulseAudio, as an AppImage, a .deb or an .rpm package.</li>
      </ul>
    ),
  },
  {
    id: "recordings",
    q: "Which recordings can Hush clean?",
    a: (
      <p>
        WAV, MP3, M4A (AAC), FLAC and OGG files. Drag one into Hush, or record yourself. Hush cleans it on your computer, lets you compare the original and the
        clean version, and saves a clean WAV.
      </p>
    ),
  },
  {
    id: "calendar",
    q: "How does the Google Calendar connection work without signing in?",
    a: (
      <>
        <p>
          Google gives every calendar a private address in iCal format. In Google Calendar's settings, under your calendar, open{" "}
          <em>Integrate calendar</em> and copy <em>Secret address in iCal format</em>. While Hush's connect window is open, it notices the copied address and fills it in.
        </p>
        <p>
          Hush then reads your meetings straight from Google, every 5 to 60 minutes. The address stays on your computer: there is no Hush server and no Google
          sign-in. Anyone with the address can read the calendar, so keep it to yourself; if it leaks, reset it in the same place in Google Calendar.
        </p>
      </>
    ),
  },
  {
    id: "vb-cable",
    q: "Why does Windows need VB-CABLE?",
    a: (
      <>
        <p>A virtual microphone on Windows needs an audio driver that Microsoft has signed. Hush's own driver is built and is waiting for that signature.</p>
        <p>
          Until then, Hush uses VB-CABLE, a free virtual audio cable by VB-Audio. Install it once from VB-Audio's website and Hush renames it to “Hush AI
          Microphone” for you. VB-CABLE adds about 84 ms of delay.
        </p>
      </>
    ),
  },
  {
    id: "unsigned",
    q: "Why does my computer warn me before opening Hush?",
    a: (
      <>
        <p>Our builds aren't code-signed yet, so your system asks once before the first launch.</p>
        <ul className="list-disc space-y-1 pl-5">
          <li>
            <strong className="font-[620]">Windows (SmartScreen):</strong> click <em>More info</em>, then <em>Run anyway</em>.
          </li>
          <li>
            <strong className="font-[620]">macOS (Gatekeeper):</strong> right-click Hush AI in Applications and choose <em>Open</em>. Or go to System Settings ›
            Privacy &amp; Security and click <em>Open Anyway</em>.
          </li>
        </ul>
        <p>You can always read the source code and build Hush yourself.</p>
      </>
    ),
  },
  {
    id: "uninstall",
    q: "How do I uninstall it?",
    a: (
      <>
        <p>
          First open Hush's Settings and, under Hush AI Microphone, press <em>Remove</em> (on Windows: <em>Restore VB-CABLE names</em>). Then remove the app:
        </p>
        <ul className="list-disc space-y-1 pl-5">
          <li>
            <strong className="font-[620]">Windows:</strong> Settings › Apps › Installed apps › Hush AI › Uninstall. VB-CABLE has its own uninstaller.
          </li>
          <li>
            <strong className="font-[620]">macOS:</strong> quit Hush and drag Hush AI from Applications to the Trash.
          </li>
          <li>
            <strong className="font-[620]">Linux:</strong> remove the package, or delete the AppImage.
          </li>
        </ul>
        <p>
          More in the{" "}
          <a className="link" href={docHref("guide")}>
            user guide
          </a>
          .
        </p>
      </>
    ),
  },
];

function Item({ id, q, a, open, onToggle }: { id: string; q: string; a: ReactNode; open: boolean; onToggle: () => void }) {
  return (
    <li id={`faq-${id}`} className="border-t border-line first:border-t-0">
      <h3>
        <button
          type="button"
          aria-expanded={open}
          aria-controls={`faq-${id}-answer`}
          onClick={onToggle}
          className="flex w-full cursor-pointer items-center gap-4 rounded-[18px] px-4 py-5 text-left text-[17px] font-[620] tracking-[-0.015em] transition-colors hover:bg-black/[.03] sm:px-5 sm:text-[18.5px]"
        >
          <span className="flex-1">{q}</span>
          <span
            className="grid h-8 w-8 shrink-0 place-items-center rounded-full bg-black/[.05] text-ink-2 transition-transform duration-300"
            style={{ transform: open ? "rotate(180deg)" : undefined }}
          >
            <ChevronDownIcon size={18} />
          </span>
        </button>
      </h3>
      <AnimatePresence initial={false}>
        {open && (
          <m.div
            id={`faq-${id}-answer`}
            role="region"
            aria-label={q}
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.3, ease: [0.2, 0.9, 0.1, 1] }}
            className="overflow-hidden"
          >
            <div className="space-y-3 px-4 pb-6 pr-12 text-[16px] leading-relaxed text-ink-2 sm:px-5 sm:pr-16">{a}</div>
          </m.div>
        )}
      </AnimatePresence>
    </li>
  );
}

export function Faq() {
  const [open, setOpen] = useState<string | null>("free");

  // Links like #faq-unsigned open their answer.
  useEffect(() => {
    const onHash = () => {
      const id = location.hash.replace(/^#faq-/, "");
      if (FAQ.some((f) => f.id === id)) setOpen(id);
    };
    onHash();
    window.addEventListener("hashchange", onHash);
    return () => window.removeEventListener("hashchange", onHash);
  }, []);

  return (
    <section
      id="faq"
      aria-labelledby="faq-title"
      className="relative py-20 sm:py-28"
    >
      <div className="wrap">
        <SectionHeader id="faq-title" eyebrow="FAQ" title="Questions, answered" />
        <Reveal className="mx-auto mt-12 max-w-[820px]">
          <ul className="card p-2 sm:p-3">
            {FAQ.map((f) => (
              <Item key={f.id} {...f} open={open === f.id} onToggle={() => setOpen(open === f.id ? null : f.id)} />
            ))}
          </ul>
        </Reveal>
      </div>
    </section>
  );
}
