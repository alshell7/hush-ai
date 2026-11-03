import { BoltIcon, CodeIcon, CpuIcon, LockIcon, RefreshIcon, TargetIcon, TrayIcon } from "../components/Icons";
import { Mark } from "../components/Mochi";
import { Reveal, SectionHeader } from "../components/ui";

const FEATURES = [
  {
    icon: <LockIcon size={22} />,
    tint: "#ffe2d6",
    title: "On-device and private",
    body: "Your voice is processed on your own CPU. No cloud, no account, nothing uploaded.",
  },
  {
    icon: <BoltIcon size={22} />,
    tint: "#fff0cc",
    title: "Real time",
    body: "10–20 ms frames and about 60–80 ms of delay, plus your device's own buffers.",
  },
  {
    icon: <CpuIcon size={22} />,
    tint: "#dcf6e3",
    title: "Light on your CPU",
    body: "Uses the fewest threads that keep up. On Windows it runs at “Pro Audio” priority.",
  },
  {
    icon: <Mark size={24} />,
    tint: "#d3f3ef",
    title: "Hush AI Microphone everywhere",
    body: "One clean virtual microphone for Zoom, Meet, Teams, Discord, Slack and OBS.",
  },
  {
    icon: <TrayIcon size={22} />,
    tint: "#ffe0ea",
    title: "Tray and menu-bar widget",
    body: "Switch Hush on or off and check your levels without opening the window.",
  },
  {
    icon: <RefreshIcon size={22} />,
    tint: "#fff0cc",
    title: "Automatic updates",
    body: "New versions download in the background. Restart Hush when it suits you.",
  },
  {
    icon: <CodeIcon size={22} />,
    tint: "#dcf6e3",
    title: "Open source",
    body: "The app and the engine are Apache-2.0. Read the code, build it yourself, send a fix.",
  },
  {
    icon: <TargetIcon size={22} />,
    tint: "#ffe2d6",
    title: "Proven accuracy",
    body: "DeepFilterNet 3 output is identical to its official Rust inference. RE-USE matches NVIDIA's GPU reference at 113–118 dB waveform SNR.",
  },
];

export function Features() {
  return (
    <section id="features" aria-labelledby="features-title" className="relative py-20 sm:py-28">
      <div className="wrap">
        <SectionHeader
          id="features-title"
          eyebrow="Features"
          title={
            <>
              Quiet, fast and <span className="opal-text">yours</span>
            </>
          }
          sub="Hush runs DeepFilterNet 3 or NVIDIA's Real-time RE-USE speech model in native Rust, right on your CPU. No Python, no GPU, no cloud."
        />
        <ul className="mt-14 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
          {FEATURES.map((f, i) => (
            <li key={f.title} className="flex">
              <Reveal delay={(i % 4) * 0.06} className="flex w-full">
                <div className="card flex w-full items-start gap-4 p-5 transition-transform duration-300 hover:-translate-y-1 sm:flex-col sm:gap-0 sm:p-6">
                  <span
                    className="grid h-11 w-11 shrink-0 place-items-center rounded-[15px] text-ink sm:h-12 sm:w-12 sm:rounded-[16px]"
                    style={{ background: f.tint, boxShadow: "inset 0 1px 0 rgba(255,255,255,.8), inset 0 0 0 1px rgba(30,27,24,.05)" }}
                  >
                    {f.icon}
                  </span>
                  <div>
                    <h3 className="text-[17px] font-[640] leading-snug tracking-[-0.02em] sm:mt-5 sm:text-[18px]">{f.title}</h3>
                    <p className="mt-1 text-[14.5px] leading-relaxed text-ink-2 sm:mt-2 sm:text-[15px]">{f.body}</p>
                  </div>
                </div>
              </Reveal>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}
