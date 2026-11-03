import { AnimatePresence, m } from "motion/react";
import { useEffect, useState, type ReactNode } from "react";
import { ChevronDownIcon } from "../components/Icons";
import { Reveal, SectionHeader } from "../components/ui";
import { MODELS_URL, REPO_URL } from "../lib/download";

const NVIDIA_LICENSE = "https://huggingface.co/nvidia/Real-time_RE-USE";
const DFN_LICENSE = "https://github.com/Rikorose/DeepFilterNet#license";

const FAQ: { id: string; q: string; a: ReactNode }[] = [
  {
    id: "free",
    q: "Is it really free?",
    a: (
      <p>
        Yes. Hush AI is free and open source. There is no account, no trial and no subscription. The code is on{" "}
        <a className="link" href={REPO_URL} target="_blank" rel="noopener">
          GitHub
        </a>
        .
      </p>
    ),
  },
  {
    id: "privacy",
    q: "Does my audio leave my computer?",
    a: (
      <p>
        No. Hush processes your microphone on your own CPU and does not send audio anywhere. It goes online for two things only: downloading a voice model
        once (it checks the file's SHA-256), and checking for app updates.
      </p>
    ),
  },
  {
    id: "platforms",
    q: "Which platforms does it run on?",
    a: (
      <ul className="list-disc space-y-1 pl-5">
        <li>Windows, on x64 and ARM64 PCs.</li>
        <li>macOS 11 or later, on Apple Silicon and Intel Macs.</li>
        <li>Linux, as an AppImage or a .deb package.</li>
      </ul>
    ),
  },
  {
    id: "vb-cable",
    q: "Why does Windows need VB-CABLE?",
    a: (
      <>
        <p>
          A virtual microphone on Windows needs an audio driver that Microsoft has signed. Hush's own driver is built and is waiting for that signature.
        </p>
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
        <p>You can always check the source code and build Hush yourself.</p>
      </>
    ),
  },
  {
    id: "license",
    q: "What is the model license? Can I use Hush at work?",
    a: (
      <>
        <p>
          The Hush app and engine code are Apache-2.0, so you can use, change and share the code freely, including at work.
        </p>
        <p>
          Hush has two voice models. The default, <b>DeepFilterNet 3</b>, is open source under the{" "}
          <a className="link" href={DFN_LICENSE} target="_blank" rel="noopener">
            MIT or Apache-2.0 license
          </a>
          , so you can use it at work too.
        </p>
        <p>
          The other one, <b>NVIDIA RE-USE</b>, comes with NVIDIA's weights under the{" "}
          <a className="link" href={NVIDIA_LICENSE} target="_blank" rel="noopener">
            NVIDIA One-Way Noncommercial License
          </a>
          : non-commercial research and educational use only. Hush asks you to accept it before the download. The weights are mirrored on{" "}
          <a className="link" href={MODELS_URL} target="_blank" rel="noopener">
            Hugging Face
          </a>
          .
        </p>
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
            <strong className="font-[620]">Linux:</strong> remove the .deb package, or delete the AppImage.
          </li>
        </ul>
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
