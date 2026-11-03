import { Reveal, SectionHeader } from "../components/ui";

const STATS = [
  { value: "7%", label: "CPU, default model", note: "DeepFilterNet 3 on one laptop core, offline. 11–13% in a live run." },
  { value: "80–250 MB", label: "Memory", note: "With RE-USE. It does not grow over time." },
  { value: "60 min", label: "Headset test", note: "RE-USE through a physical headset, with zero lost audio." },
  { value: "113–118 dB", label: "Waveform SNR", note: "CPU output against NVIDIA's GPU reference, on short speech." },
];

const PROFILES = {
  Light: { color: "#1f9d72" },
  Balanced: { color: "#d99a1e" },
  Studio: { color: "#e8735a" },
} as const;

const ROWS: { profile: keyof typeof PROFILES; audio: string; machine: string; threads?: string; ms: number }[] = [
  { profile: "Light", audio: "16 kHz · 3 layers", machine: "Apple M1", threads: "2 threads", ms: 11.7 },
  { profile: "Light", audio: "16 kHz · 3 layers", machine: "Intel x64 (Modal)", threads: "4 threads", ms: 17.1 },
  { profile: "Balanced", audio: "24 kHz · 4 layers", machine: "Apple M1", ms: 21.3 },
  { profile: "Studio", audio: "48 kHz · 3 layers", machine: "Apple M1", threads: "2 threads", ms: 36.8 },
  { profile: "Studio", audio: "48 kHz · 3 layers", machine: "Intel x64", threads: "8 threads", ms: 23.7 },
];
const MAX = 40;

export function Performance() {
  return (
    <section id="performance" aria-labelledby="perf-title" className="relative py-20 sm:py-28">
      <div className="wrap">
        <SectionHeader
          id="perf-title"
          eyebrow="Performance"
          title={
            <>
              Measured, <span className="opal-text">not promised</span>
            </>
          }
          sub="Measured on an Apple M1 and on Intel x64. On your computer, Hush picks the fewest CPU threads that keep up."
        />

        <ul className="mt-12 grid grid-cols-2 gap-3 sm:mt-14 sm:gap-4 lg:grid-cols-4">
          {STATS.map((s, i) => (
            <li key={s.label} className="flex">
              <Reveal delay={i * 0.06} className="flex w-full">
                <div className="card w-full p-4 sm:p-6">
                  <div className="text-[11.5px] font-[600] uppercase tracking-[0.06em] text-muted sm:text-[13px]">{s.label}</div>
                  <div className="heading mt-2 whitespace-nowrap text-[23px] tabular-nums sm:text-[38px]">{s.value}</div>
                  <p className="mt-2 text-[13px] leading-snug text-ink-2 sm:text-[14.5px] sm:leading-relaxed">{s.note}</p>
                </div>
              </Reveal>
            </li>
          ))}
        </ul>

        <Reveal className="mt-6">
          <div className="card overflow-hidden p-2 sm:p-3">
            <table className="w-full border-collapse text-left">
              <caption className="px-4 pb-1 pt-4 text-left sm:px-5">
                <span className="block text-[18px] font-[640] tracking-[-0.02em]">NVIDIA RE-USE: time to process one 20 ms frame (p99)</span>
                <span className="mt-1 block text-[14px] font-normal text-muted">99 of 100 frames finish within this time. Lower is better.</span>
              </caption>
              <thead>
                <tr className="text-[12.5px] font-[600] uppercase tracking-[0.06em] text-muted">
                  <th scope="col" className="px-4 py-3 font-[600] sm:px-5">
                    Quality
                  </th>
                  <th scope="col" className="px-2 py-3 font-[600] sm:px-5">
                    Machine
                  </th>
                  <th scope="col" className="px-4 py-3 text-right font-[600] sm:px-5 md:text-left">
                    p99
                  </th>
                </tr>
              </thead>
              <tbody>
                {ROWS.map((r) => (
                  <tr key={`${r.profile}-${r.machine}`} className="border-t border-line">
                    <th scope="row" className="px-4 py-4 align-middle font-normal sm:px-5">
                      <span className="flex items-center gap-2 text-[15.5px] font-[620]">
                        <span className="h-2.5 w-2.5 shrink-0 rounded-full" style={{ background: PROFILES[r.profile].color }} />
                        {r.profile}
                      </span>
                      <span className="mt-0.5 block pl-[18px] text-[13px] text-muted">{r.audio}</span>
                    </th>
                    <td className="px-2 py-4 align-middle sm:px-5">
                      <span className="block text-[15px] font-[540]">{r.machine}</span>
                      {r.threads && <span className="mt-0.5 block text-[13px] text-muted">{r.threads}</span>}
                    </td>
                    <td className="px-4 py-4 align-middle sm:px-5 md:w-[46%]">
                      <div className="flex items-center justify-end gap-3 md:justify-start">
                        <span aria-hidden className="hidden h-2.5 flex-1 overflow-hidden rounded-full bg-black/[.05] md:block">
                          <span className="block h-full rounded-full" style={{ width: `${(r.ms / MAX) * 100}%`, background: PROFILES[r.profile].color }} />
                        </span>
                        <span className="w-[72px] shrink-0 text-right text-[15.5px] font-[640] tabular-nums">{r.ms.toFixed(1)} ms</span>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p className="mt-4 px-1 text-[13.5px] leading-relaxed text-muted">
            On Windows, VB-CABLE adds about 84 ms. Light uses 16 kHz audio with 3 layers, Balanced 24 kHz with 4 layers, and Studio full-band 48 kHz with 3 layers.
          </p>
        </Reveal>
      </div>
    </section>
  );
}
