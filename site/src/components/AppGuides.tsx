// Where each app keeps its microphone setting. The body of every app tooltip on the site is
// <AppGuide app="…" />, so richer per-app pictures can replace it in one place.
import discord from "../assets/apps/discord.svg";
import meet from "../assets/apps/meet.svg";
import obs from "../assets/apps/obs.svg";
import slack from "../assets/apps/slack.svg";
import teams from "../assets/apps/teams.svg";
import telegram from "../assets/apps/telegram.svg";
import webex from "../assets/apps/webex.svg";
import whatsapp from "../assets/apps/whatsapp.svg";
import zoom from "../assets/apps/zoom.svg";
import { CheckIcon } from "./Icons";
import { Mark } from "./Mochi";

export type AppId = "zoom" | "meet" | "teams" | "slack" | "discord" | "webex" | "obs" | "whatsapp" | "telegram";

export type AppInfo = {
  id: AppId;
  name: string;
  logo: string;
  /** Menu path to the microphone setting, or null when the app has no single place for it. */
  path: string[] | null;
};

export const APPS: AppInfo[] = [
  { id: "zoom", name: "Zoom", logo: zoom, path: ["Settings", "Audio", "Microphone"] },
  { id: "meet", name: "Google Meet", logo: meet, path: ["More options", "Settings", "Audio", "Microphone"] },
  { id: "teams", name: "Microsoft Teams", logo: teams, path: ["Settings", "Devices", "Microphone"] },
  { id: "slack", name: "Slack", logo: slack, path: ["Preferences", "Audio & video", "Microphone"] },
  { id: "discord", name: "Discord", logo: discord, path: ["User Settings", "Voice & Video", "Input Device"] },
  { id: "webex", name: "Webex", logo: webex, path: ["Settings", "Audio", "Microphone"] },
  { id: "obs", name: "OBS Studio", logo: obs, path: ["Settings", "Audio", "Mic/Auxiliary Audio"] },
  { id: "whatsapp", name: "WhatsApp", logo: whatsapp, path: null },
  { id: "telegram", name: "Telegram", logo: telegram, path: null },
];

export const appById = (id: AppId) => APPS.find((a) => a.id === id)!;

export function AppLogo({ app, size = 28, className = "" }: { app: AppId; size?: number; className?: string }) {
  const info = appById(app);
  return <img src={info.logo} alt="" width={size} height={size} loading="lazy" decoding="async" className={`shrink-0 object-contain ${className}`} style={{ width: size, height: size }} />;
}

/** Where to choose Hush AI Microphone in one app. */
export function AppGuide({ app }: { app: AppId }) {
  const info = appById(app);
  return (
    <div className="text-left">
      <div className="flex items-center gap-2.5">
        <AppLogo app={app} size={22} />
        <span className="text-[14.5px] font-[640] tracking-[-0.01em] text-ink">{info.name}</span>
      </div>
      {info.path ? (
        <>
          <div className="mt-3 text-[11.5px] font-[600] uppercase tracking-[0.07em] text-muted">Open</div>
          <ol className="mt-1.5 flex flex-wrap items-center gap-1 text-[13px] font-[560]" aria-label={`Menu path in ${info.name}`}>
            {info.path.map((step, i) => (
              <li key={step} className="flex items-center gap-1">
                <span className="rounded-full bg-[#f4efe8] px-2.5 py-1 text-ink">{step}</span>
                {i < info.path!.length - 1 && (
                  <span aria-hidden className="text-faint">
                    ›
                  </span>
                )}
              </li>
            ))}
          </ol>
        </>
      ) : (
        <p className="mt-2.5 text-[13px] leading-snug text-ink-2">Open the app's audio or call settings, where it lists microphones.</p>
      )}
      <div className="mt-3 flex flex-wrap items-center gap-2 text-[13px] text-ink-2">
        then choose
        <span
          className="inline-flex items-center gap-1.5 rounded-full bg-white py-1 pl-1.5 pr-2.5 font-[620] text-ink"
          style={{ boxShadow: "inset 0 0 0 1px rgba(31,157,114,.28), 0 6px 16px -10px rgba(31,157,114,.6)" }}
        >
          <Mark size={16} /> Hush AI Microphone
          <CheckIcon size={14} strokeWidth={2.4} className="text-green" />
        </span>
      </div>
    </div>
  );
}
