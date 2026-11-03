// "Noise in, voice out": a looping illustration of what Hush does to the microphone. Two
// waveforms scroll through the mascot; only transforms animate, so it costs almost nothing.
import { useMemo } from "react";
import { Mark } from "./Mochi";

const WIDTH = 240;
const MID = 26;

/** One period of speech: syllable bumps with pauses. Noise adds dense jitter everywhere. */
function wave(noisy: boolean) {
  let d = "";
  for (let x = 0; x <= WIDTH * 2; x += 2) {
    const t = (x % WIDTH) / WIDTH;
    const syllables = Math.max(0, Math.sin(t * Math.PI * 7)) * Math.max(0, Math.sin(t * Math.PI * 2 + 0.6));
    const speech = syllables ** 1.3 * 20;
    const jitter = noisy ? 5 + 4 * Math.abs(Math.sin(x * 12.9898) * 43758.5453 % 1) + (Math.sin(x * 0.9) > 0.92 ? 9 : 0) : 0.6;
    const h = Math.min(MID - 2, speech + jitter);
    d += `M${x} ${MID - h}V${MID + h}`;
  }
  return d;
}

const NOISES = ["Dog", "Keys", "Traffic", "Fan", "Café"];

export function NoiseMorph() {
  const [noisy, clean] = useMemo(() => [wave(true), wave(false)], []);
  return (
    <div className="relative flex h-[84px] items-center overflow-hidden rounded-[18px] bg-white/70" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }} aria-hidden>
      {/* Noisy side */}
      <div className="relative h-full flex-1 overflow-hidden" style={{ maskImage: "linear-gradient(90deg, transparent, #000 18%, #000)" }}>
        <svg className="nm-scroll absolute left-0 top-1/2 -translate-y-1/2" width={WIDTH * 2} height={MID * 2} viewBox={`0 0 ${WIDTH * 2} ${MID * 2}`}>
          <path d={noisy} stroke="#b9b2a7" strokeWidth="1.3" strokeLinecap="round" />
        </svg>
        {NOISES.map((noise, i) => (
          <span
            key={noise}
            className="nm-float absolute rounded-full bg-white px-1.5 text-[9.5px] font-semibold text-muted"
            style={{ top: `${14 + ((i * 37) % 60)}%`, left: `${8 + i * 17}%`, animationDelay: `${i * -1.1}s`, boxShadow: "0 0 0 1px var(--h-line)" }}
          >
            {noise}
          </span>
        ))}
      </div>
      {/* Hush */}
      <div className="relative z-10 mx-1 grid h-14 w-14 shrink-0 place-items-center rounded-full bg-white" style={{ boxShadow: "0 0 0 1px var(--h-line), 0 8px 18px -10px rgba(30,27,24,.5)" }}>
        <span className="nm-halo absolute inset-0 rounded-full" style={{ background: "var(--h-opal-conic)", opacity: 0.35 }} />
        <span className="relative">
          <Mark size={30} />
        </span>
      </div>
      {/* Clean side */}
      <div className="relative h-full flex-1 overflow-hidden" style={{ maskImage: "linear-gradient(90deg, #000, #000 82%, transparent)" }}>
        <svg className="nm-scroll absolute left-0 top-1/2 -translate-y-1/2" width={WIDTH * 2} height={MID * 2} viewBox={`0 0 ${WIDTH * 2} ${MID * 2}`}>
          <defs>
            <linearGradient id="nm-clean" x1="0" x2="1">
              <stop offset="0" stopColor="#1f9d72" />
              <stop offset="1" stopColor="#178f8a" />
            </linearGradient>
          </defs>
          <path d={clean} stroke="url(#nm-clean)" strokeWidth="1.6" strokeLinecap="round" />
        </svg>
      </div>
    </div>
  );
}
