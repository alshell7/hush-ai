// Line icons, 24×24, 1.8 stroke. Most are shared with the app (app/src/components/Icons.tsx).
import type { SVGProps } from "react";

type P = SVGProps<SVGSVGElement> & { size?: number };

function Svg({ size = 20, children, ...rest }: P) {
  return (
    <svg
      viewBox="0 0 24 24"
      width={size}
      height={size}
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
      focusable="false"
      {...rest}
    >
      {children}
    </svg>
  );
}

export const MicIcon = (p: P) => (
  <Svg {...p}>
    <rect x="9" y="3" width="6" height="11" rx="3" />
    <path d="M5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21M8.5 21h7" />
  </Svg>
);

export const DownloadIcon = (p: P) => (
  <Svg {...p}>
    <path d="M12 4v11" className="transition-transform duration-300 group-hover:translate-y-0.5" />
    <path d="M7.5 10.5 12 15l4.5-4.5" className="transition-transform duration-300 group-hover:translate-y-0.5" />
    <path d="M5 19.5h14" />
  </Svg>
);

export const ArrowRightIcon = (p: P) => (
  <Svg {...p}>
    <path d="M5 12h14M13 6l6 6-6 6" className="transition-transform duration-300 group-hover:translate-x-0.5" />
  </Svg>
);

export const ExternalIcon = (p: P) => (
  <Svg {...p}>
    <path d="M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5" />
  </Svg>
);

export const PlayIcon = (p: P) => (
  <Svg {...p}>
    <path d="M7 5.5v13a1 1 0 0 0 1.5.9l10.4-6.5a1 1 0 0 0 0-1.8L8.5 4.6A1 1 0 0 0 7 5.5Z" fill="currentColor" stroke="none" />
  </Svg>
);

export const PauseIcon = (p: P) => (
  <Svg {...p}>
    <rect x="6.5" y="5" width="4" height="14" rx="1.4" fill="currentColor" stroke="none" />
    <rect x="13.5" y="5" width="4" height="14" rx="1.4" fill="currentColor" stroke="none" />
  </Svg>
);

export const SparkleIcon = (p: P) => (
  <Svg {...p}>
    <path d="M12 3.5c.6 4.2 2.3 5.9 6.5 6.5-4.2.6-5.9 2.3-6.5 6.5-.6-4.2-2.3-5.9-6.5-6.5 4.2-.6 5.9-2.3 6.5-6.5Z" />
    <path d="M18.5 15.5c.3 1.6.9 2.2 2.5 2.5-1.6.3-2.2.9-2.5 2.5-.3-1.6-.9-2.2-2.5-2.5 1.6-.3 2.2-.9 2.5-2.5Z" />
  </Svg>
);

export const BoltIcon = (p: P) => (
  <Svg {...p}>
    <path d="M13 3 5 13.5h6L10 21l8-10.5h-6L13 3Z" />
  </Svg>
);

export const ShieldIcon = (p: P) => (
  <Svg {...p}>
    <path d="M12 3 5 6v5.5c0 4.4 2.9 8 7 9.5 4.1-1.5 7-5.1 7-9.5V6l-7-3Z" />
    <path d="m9 12 2 2 4-4" />
  </Svg>
);

export const TranslateIcon = (p: P) => (
  <Svg {...p}>
    <path d="M4 5.5h9M8.5 3.5v2M6 5.5c.6 3.6 3 6.4 6 7.5M11 5.5c-.8 3.7-3.4 6.6-7 8" />
    <path d="M13 20.5 16.5 12l3.5 8.5M14.2 17.5h4.6" />
  </Svg>
);

export const AgentIcon = (p: P) => (
  <Svg {...p}>
    <path d="M4.5 13v-1.5a7.5 7.5 0 0 1 15 0V13" />
    <rect x="3.5" y="12.5" width="4" height="6" rx="1.8" />
    <rect x="16.5" y="12.5" width="4" height="6" rx="1.8" />
    <path d="M18.5 18.5c0 1.5-1.5 2.5-4 2.5h-1.5" />
  </Svg>
);

export const ChartIcon = (p: P) => (
  <Svg {...p}>
    <path d="M4 20V4M4 20h16" />
    <path d="m7.5 15 3.5-4 3 2.5L19 8" />
  </Svg>
);

export const LockIcon = (p: P) => (
  <Svg {...p}>
    <rect x="5" y="10.5" width="14" height="10" rx="2.5" />
    <path d="M8 10.5V8a4 4 0 0 1 8 0v2.5M12 14.5v2" />
  </Svg>
);

export const CpuIcon = (p: P) => (
  <Svg {...p}>
    <rect x="6" y="6" width="12" height="12" rx="2.5" />
    <rect x="9.5" y="9.5" width="5" height="5" rx="1" />
    <path d="M9.5 3v3M14.5 3v3M9.5 18v3M14.5 18v3M3 9.5h3M3 14.5h3M18 9.5h3M18 14.5h3" />
  </Svg>
);

export const TrayIcon = (p: P) => (
  <Svg {...p}>
    <rect x="3" y="4" width="18" height="16" rx="3" />
    <path d="M3 8h18" />
    <rect x="12.5" y="10.5" width="6" height="6.5" rx="1.6" />
  </Svg>
);

export const RefreshIcon = (p: P) => (
  <Svg {...p}>
    <path d="M19.5 12a7.5 7.5 0 0 1-13 5.1M4.5 12a7.5 7.5 0 0 1 13-5.1" />
    <path d="M17.5 3.5v3.4h-3.4M6.5 20.5v-3.4h3.4" />
  </Svg>
);

export const CodeIcon = (p: P) => (
  <Svg {...p}>
    <path d="m8.5 7.5-4.5 4.5 4.5 4.5M15.5 7.5l4.5 4.5-4.5 4.5M13.5 5l-3 14" />
  </Svg>
);

export const TargetIcon = (p: P) => (
  <Svg {...p}>
    <circle cx="12" cy="12" r="8.5" />
    <circle cx="12" cy="12" r="4.5" />
    <circle cx="12" cy="12" r="0.8" fill="currentColor" />
  </Svg>
);

export const CheckIcon = (p: P) => (
  <Svg {...p}>
    <path d="M5 12.5l4.5 4.5L19 7.5" />
  </Svg>
);

export const ChevronDownIcon = (p: P) => (
  <Svg {...p}>
    <path d="m6 9 6 6 6-6" />
  </Svg>
);

export const MenuIcon = (p: P) => (
  <Svg {...p}>
    <path d="M4 8h16M4 16h16" />
  </Svg>
);

export const CloseIcon = (p: P) => (
  <Svg {...p}>
    <path d="M6.5 6.5l11 11M17.5 6.5l-11 11" />
  </Svg>
);

export const StarIcon = (p: P) => (
  <Svg {...p}>
    <path d="m12 3.6 2.6 5.3 5.8.8-4.2 4.1 1 5.8L12 16.9l-5.2 2.7 1-5.8-4.2-4.1 5.8-.8L12 3.6Z" />
  </Svg>
);

export const HeartIcon = (p: P) => (
  <Svg {...p}>
    <path d="M12 19.5s-7.5-4.3-7.5-9.6A4.1 4.1 0 0 1 12 7.6a4.1 4.1 0 0 1 7.5 2.3c0 5.3-7.5 9.6-7.5 9.6Z" />
  </Svg>
);

export const BookIcon = (p: P) => (
  <Svg {...p}>
    <path d="M5 4.5h10.5A3.5 3.5 0 0 1 19 8v11.5H8.5A3.5 3.5 0 0 1 5 16V4.5Z" />
    <path d="M5 16a3.5 3.5 0 0 1 3.5-3.5H19" />
  </Svg>
);

/* Noise sources, for the hero. */
export const KeyboardIcon = (p: P) => (
  <Svg {...p}>
    <rect x="2.5" y="6" width="19" height="12" rx="2.5" />
    <path d="M6.5 10h.01M10 10h.01M13.5 10h.01M17 10h.01M8 14h8" />
  </Svg>
);

export const FanIcon = (p: P) => (
  <Svg {...p}>
    <circle cx="12" cy="12" r="1.6" />
    <path d="M12 10.4c-1-2.6-.6-5.9 1.6-6.4 2.4-.5 3 3 .4 5M13.4 12.8c2.7.4 5.3 2.4 4.6 4.5-.8 2.3-4 1-4.4-1.6M10.6 12.8c-1.7 2.2-4.8 3.4-6.2 1.7-1.6-1.9 1.1-4.2 3.6-3.2" />
  </Svg>
);

export const PawIcon = (p: P) => (
  <Svg {...p}>
    <path d="M12 12.5c-2.6 0-5 2.6-5 4.8 0 1.6 1.4 2.2 2.6 2.2 1 0 1.6-.5 2.4-.5s1.4.5 2.4.5c1.2 0 2.6-.6 2.6-2.2 0-2.2-2.4-4.8-5-4.8Z" />
    <ellipse cx="7" cy="9.5" rx="1.6" ry="2" />
    <ellipse cx="17" cy="9.5" rx="1.6" ry="2" />
    <ellipse cx="10" cy="5.8" rx="1.6" ry="2" />
    <ellipse cx="14" cy="5.8" rx="1.6" ry="2" />
  </Svg>
);

export const CarIcon = (p: P) => (
  <Svg {...p}>
    <path d="M4 15.5v-3l2-4.5a2 2 0 0 1 1.8-1.2h8.4A2 2 0 0 1 18 8l2 4.5v3a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1Z" />
    <path d="M4 12.5h16" />
    <path d="M7 16.5v1.5M17 16.5v1.5" />
  </Svg>
);

export const CupIcon = (p: P) => (
  <Svg {...p}>
    <path d="M5 9h11v5a5 5 0 0 1-5 5h-1a5 5 0 0 1-5-5V9Z" />
    <path d="M16 10.5h1.5a2.5 2.5 0 0 1 0 5H16M8.5 3.5c-.6 1 .6 1.5 0 2.5M12 3.5c-.6 1 .6 1.5 0 2.5" />
  </Svg>
);

/* Platforms. Simple glyphs, not vendor logos. */
export const WindowsGlyph = (p: P) => (
  <svg viewBox="0 0 24 24" width={p.size ?? 18} height={p.size ?? 18} fill="currentColor" aria-hidden focusable="false" className={p.className}>
    <rect x="3.5" y="3.5" width="7.6" height="7.6" rx="1.2" />
    <rect x="12.9" y="3.5" width="7.6" height="7.6" rx="1.2" />
    <rect x="3.5" y="12.9" width="7.6" height="7.6" rx="1.2" />
    <rect x="12.9" y="12.9" width="7.6" height="7.6" rx="1.2" />
  </svg>
);

export const LaptopGlyph = (p: P) => (
  <Svg {...p}>
    <rect x="4.5" y="5" width="15" height="10.5" rx="2" />
    <path d="M2.5 19h19" />
  </Svg>
);

export const TerminalGlyph = (p: P) => (
  <Svg {...p}>
    <rect x="3" y="4.5" width="18" height="15" rx="3" />
    <path d="m7 9.5 3 2.5-3 2.5M12.5 15h4.5" />
  </Svg>
);

export const GitHubIcon = ({ size = 18, className }: { size?: number; className?: string }) => (
  <svg viewBox="0 0 24 24" width={size} height={size} fill="currentColor" aria-hidden focusable="false" className={className}>
    <path d="M12 2a10 10 0 0 0-3.2 19.5c.5.1.7-.2.7-.5v-1.7c-2.8.6-3.4-1.3-3.4-1.3-.4-1.1-1.1-1.4-1.1-1.4-.9-.6.1-.6.1-.6 1 .1 1.5 1 1.5 1 .9 1.5 2.4 1.1 2.9.8.1-.6.4-1.1.6-1.3-2.2-.3-4.6-1.1-4.6-5a3.9 3.9 0 0 1 1-2.7 3.6 3.6 0 0 1 .1-2.7s.8-.3 2.7 1a9.4 9.4 0 0 1 5 0c1.9-1.3 2.7-1 2.7-1 .5 1.4.2 2.4.1 2.7a3.9 3.9 0 0 1 1 2.7c0 3.9-2.4 4.7-4.6 5 .4.3.7.9.7 1.9V21c0 .3.2.6.7.5A10 10 0 0 0 12 2Z" />
  </svg>
);

export const HuggingFaceGlyph = (p: P) => (
  <Svg {...p}>
    <circle cx="12" cy="11.5" r="8" />
    <path d="M9 10.5h.01M15 10.5h.01" strokeWidth={2.6} />
    <path d="M9.2 13.8c1.5 1.4 4.1 1.4 5.6 0" />
  </Svg>
);
