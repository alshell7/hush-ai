// Real logos of the apps people use Hush with. The files live in /assets/apps (see its README
// for sources and licenses); names and logos belong to their owners.
import discord from "../../../assets/apps/discord.svg";
import calendar from "../../../assets/apps/google-calendar.svg";
import meet from "../../../assets/apps/meet.svg";
import obs from "../../../assets/apps/obs.svg";
import slack from "../../../assets/apps/slack.svg";
import teams from "../../../assets/apps/teams.svg";
import webex from "../../../assets/apps/webex.svg";
import zoom from "../../../assets/apps/zoom.svg";
import type { AppId } from "../lib/features";
import { VideoIcon } from "./Icons";

export const APP_NAMES: Record<AppId, string> = {
  zoom: "Zoom",
  meet: "Google Meet",
  teams: "Microsoft Teams",
  slack: "Slack",
  discord: "Discord",
  webex: "Webex",
  obs: "OBS Studio",
  other: "Video call",
};

const LOGOS: Partial<Record<AppId, string>> = { zoom, meet, teams, slack, discord, webex, obs };

export const GOOGLE_CALENDAR_LOGO = calendar;

/** An app's logo; `tile` puts it on a small white rounded square. */
export function AppIcon({ app, size = 20, tile = false }: { app: AppId; size?: number; tile?: boolean }) {
  const logo = LOGOS[app];
  const mark = logo ? (
    <img src={logo} alt="" width={size} height={size} draggable={false} style={{ width: size, height: size, objectFit: "contain" }} />
  ) : (
    <VideoIcon size={size} className="text-muted" />
  );
  if (!tile) return mark;
  const box = Math.round(size * 1.75);
  return (
    <span
      className="grid shrink-0 place-items-center bg-white"
      style={{ width: box, height: box, borderRadius: Math.round(box * 0.3), boxShadow: "0 0 0 1px rgba(30,27,24,.07), 0 4px 10px -6px rgba(30,27,24,.3)" }}
      aria-label={APP_NAMES[app]}
      role="img"
    >
      {mark}
    </span>
  );
}
