// Every benchmark number on the site comes from this file. Source: docs/COMPARISON.md and
// benchmarks/compare/results.json (VoiceBank-DEMAND test set, scored at 16 kHz). Update it
// here when the in-app Rust DeepFilterNet path is measured again.

import { REUSE_LICENSE_URL } from "../lib/download";

export type MetricId = "pesq" | "stoi" | "sisdr" | "dnsmos" | "cpu";
export type SystemId = "noisy" | "rnnoise" | "reuseLight" | "reuse8" | "deepfilter";

export type System = {
  id: SystemId;
  name: string;
  /** The name without "Hush ·", for narrow screens and the CPU race. */
  short: string;
  detail: string;
  /** "hush" for Hush's engines; others are references. */
  kind: "hush" | "reference" | "input";
  pesq: number;
  stoi: number;
  sisdr: number;
  dnsmos: number;
  /** CPU time per second of audio on one thread (1 = a whole core); null when not run. */
  cpu: number | null;
};

export const SYSTEMS: System[] = [
  { id: "noisy", name: "No suppression", short: "No suppression", detail: "The noisy input", kind: "input", pesq: 1.97, stoi: 0.921, sisdr: 8.4, dnsmos: 2.7, cpu: null },
  { id: "rnnoise", name: "RNNoise", short: "RNNoise", detail: "xiph/rnnoise", kind: "reference", pesq: 2.11, stoi: 0.89, sisdr: 12.3, dnsmos: 2.94, cpu: 0.05 },
  { id: "reuseLight", name: "Hush · NVIDIA RE-USE Light", short: "NVIDIA RE-USE Light", detail: "16 kHz, 3 layers", kind: "hush", pesq: 2.61, stoi: 0.934, sisdr: 18.0, dnsmos: 3.16, cpu: null },
  { id: "reuse8", name: "Hush · NVIDIA RE-USE", short: "NVIDIA RE-USE", detail: "16 kHz, 8 layers", kind: "hush", pesq: 2.77, stoi: 0.938, sisdr: 18.5, dnsmos: 3.23, cpu: null },
  { id: "deepfilter", name: "Hush · DeepFilterNet 3", short: "DeepFilterNet 3", detail: "48 kHz, the default", kind: "hush", pesq: 3.1, stoi: 0.945, sisdr: 19.0, dnsmos: 3.18, cpu: 0.07 },
];

export type Metric = {
  id: MetricId;
  label: string;
  name: string;
  /** What it means, in plain words. */
  plain: string;
  /** Axis shown for the bars. The start is labelled on the chart. */
  scale: [number, number];
  higherIsBetter: boolean;
  format: (v: number) => string;
};

export const METRICS: Metric[] = [
  {
    id: "pesq",
    label: "PESQ",
    name: "Perceived quality",
    plain: "How close the result sounds to the clean studio recording, on a scale of 1 to 4.5.",
    scale: [1, 4.5],
    higherIsBetter: true,
    format: (v) => v.toFixed(2),
  },
  {
    id: "stoi",
    label: "STOI",
    name: "Intelligibility",
    plain: "How easy the words are to understand, from 0 to 1.",
    scale: [0.85, 1],
    higherIsBetter: true,
    format: (v) => v.toFixed(3),
  },
  {
    id: "sisdr",
    label: "SI-SDR",
    name: "Noise removed",
    plain: "How much stronger the voice is than the noise and distortion that remain, in decibels.",
    scale: [0, 20],
    higherIsBetter: true,
    format: (v) => `${v.toFixed(1)} dB`,
  },
  {
    id: "dnsmos",
    label: "DNSMOS",
    name: "Natural sound",
    plain: "Microsoft's neural estimate of the score listeners would give (overall), from 1 to 5. The clean recordings score 3.22.",
    scale: [2.5, 3.5],
    higherIsBetter: true,
    format: (v) => v.toFixed(2),
  },
  {
    id: "cpu",
    label: "CPU",
    name: "CPU use",
    plain: "Share of one CPU core needed for each second of audio, on one thread. Lower is better.",
    scale: [0, 1.5],
    higherIsBetter: false,
    format: (v) => `${Math.round(v * 100)}%`,
  },
];

export const DATASET = {
  name: "VoiceBank-DEMAND",
  files: 824,
  minutes: 34.5,
  scoredAt: "16 kHz",
  doc: "comparison",
};

/** Small print under the charts. */
export const NOTES = [
  "DeepFilterNet 3 numbers come from the official deep-filter 0.5.6 binary. Hush runs the same libDF code and model.",
  "One dataset, objective metrics only, no listening test. The test speech and noise are close to DeepFilterNet 3's training data.",
  "CPU is approximate: one process at a time on an 8-core x64 laptop that was also doing other work.",
  "Live load in the app (debug build, Windows laptop, one thread): DeepFilterNet 3 about 2.3% of real time.",
];

export type EngineId = "deepfilter" | "reuse";

export const ENGINES: Record<
  EngineId,
  { name: string; short: string; role: string; license: string; licenseUrl: string | null; points: string[] }
> = {
  deepfilter: {
    name: "DeepFilterNet 3",
    short: "DeepFilterNet 3",
    role: "The default",
    license: "MIT or Apache-2.0. Free for any use.",
    licenseUrl: null,
    points: ["About 7% of one CPU core", "Highest PESQ, STOI and SI-SDR here", "Full band, 48 kHz, on one thread", "Free for any use, also at work"],
  },
  reuse: {
    name: "NVIDIA Real-time RE-USE",
    short: "NVIDIA RE-USE",
    role: "The most natural sound",
    license: "NVIDIA RE-USE model license",
    licenseUrl: REUSE_LICENSE_URL,
    points: ["Best DNSMOS (natural sound) with 8 layers", "Light, Balanced and Studio quality in the app", "Wants a faster CPU", "Optional: download it in Settings"],
  },
};

/** Best value of a metric among all systems that were measured for it. */
export function best(metric: MetricId): SystemId {
  const m = METRICS.find((x) => x.id === metric)!;
  const rows = SYSTEMS.filter((s) => (metric === "cpu" ? s.cpu !== null : s.kind !== "input"));
  rows.sort((a, b) => ((m.higherIsBetter ? -1 : 1) * ((a[metric] ?? 0) - (b[metric] ?? 0))));
  return rows[0].id;
}
