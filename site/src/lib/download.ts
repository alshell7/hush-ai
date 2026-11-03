// Finds the right installer for the visitor. The only network call the site makes to
// another host: GitHub's "latest release" API. Anything unexpected falls back to the
// releases page, so a download link always works.
import { createContext, useContext, useEffect, useState } from "react";

export const REPO_URL = "https://github.com/alshell7/hush-ai";
export const RELEASES_URL = `${REPO_URL}/releases/latest`;
export const MODELS_URL = "https://huggingface.co/alshell7/hush-ai";
/** The one place the site links to the NVIDIA model's terms. */
export const REUSE_LICENSE_URL = "https://huggingface.co/alshell7/hush-ai/blob/main/LICENSE";
const API_URL = "https://api.github.com/repos/alshell7/hush-ai/releases/latest";
const CACHE_KEY = "hush-release-v1";
const CACHE_MS = 15 * 60 * 1000;

export type OS = "windows" | "macos" | "linux" | "mobile" | "unknown";
export type Desktop = "windows" | "macos" | "linux";
export type Arch = "x64" | "arm64";
export type TargetId = "win-x64" | "win-arm64" | "mac-arm64" | "mac-x64" | "linux-appimage" | "linux-deb" | "linux-rpm";

export type Target = {
  id: TargetId;
  os: Desktop;
  label: string;
  detail: string;
  match: (name: string) => boolean;
};

const ARM = /(arm64|aarch64)/;
const X64 = /(x64|x86_64|amd64|intel)/;
const UNIVERSAL = /universal/;

export const TARGETS: Target[] = [
  { id: "win-x64", os: "windows", label: "x64", detail: "Most Windows PCs", match: (n) => n.endsWith(".exe") && !ARM.test(n) },
  { id: "win-arm64", os: "windows", label: "ARM64", detail: "Snapdragon and other ARM PCs", match: (n) => n.endsWith(".exe") && ARM.test(n) },
  { id: "mac-arm64", os: "macos", label: "Apple Silicon", detail: "M1 and newer", match: (n) => n.endsWith(".dmg") && (ARM.test(n) || UNIVERSAL.test(n)) },
  { id: "mac-x64", os: "macos", label: "Intel", detail: "Older Macs", match: (n) => n.endsWith(".dmg") && (X64.test(n) || UNIVERSAL.test(n)) },
  { id: "linux-appimage", os: "linux", label: "AppImage", detail: "Any distribution", match: (n) => n.endsWith(".appimage") && !ARM.test(n) },
  { id: "linux-deb", os: "linux", label: ".deb", detail: "Debian, Ubuntu", match: (n) => n.endsWith(".deb") && !ARM.test(n) },
  { id: "linux-rpm", os: "linux", label: ".rpm", detail: "Fedora, openSUSE", match: (n) => n.endsWith(".rpm") && !ARM.test(n) },
];

export const OS_NAME: Record<Desktop, string> = { windows: "Windows", macos: "macOS", linux: "Linux" };

export type Asset = { url: string; name: string; size: number };
export type Release = {
  status: "loading" | "ready" | "fallback";
  version: string | null;
  assets: Partial<Record<TargetId, Asset>>;
};

type NavigatorUA = Navigator & {
  userAgentData?: {
    platform?: string;
    mobile?: boolean;
    getHighEntropyValues?: (hints: string[]) => Promise<{ architecture?: string }>;
  };
};

export function detectOS(): OS {
  const nav = navigator as NavigatorUA;
  const ua = navigator.userAgent;
  const platform = (nav.userAgentData?.platform || navigator.platform || "").toLowerCase();
  if (nav.userAgentData?.mobile || /android|iphone|ipad|ipod/i.test(ua)) return "mobile";
  // iPadOS asks for desktop sites and says "Mac"; touch gives it away.
  if (/mac/.test(platform) && navigator.maxTouchPoints > 1) return "mobile";
  if (/win/.test(platform) || /windows nt/i.test(ua)) return "windows";
  if (/mac/.test(platform) || /mac os x/i.test(ua)) return "macos";
  if (/cros/i.test(ua)) return "unknown";
  if (/linux|x11/.test(platform) || /linux|x11/i.test(ua)) return "linux";
  return "unknown";
}

/** ARM or x64. Chromium can tell; elsewhere we assume Apple Silicon on a Mac and x64 otherwise. */
export async function detectArch(os: OS): Promise<Arch> {
  const nav = navigator as NavigatorUA;
  try {
    const hints = await nav.userAgentData?.getHighEntropyValues?.(["architecture", "bitness"]);
    if (hints?.architecture === "arm") return "arm64";
    if (hints?.architecture === "x86") return "x64";
  } catch {
    /* not available */
  }
  if (/aarch64|arm64/i.test(navigator.userAgent)) return "arm64";
  return os === "macos" ? "arm64" : "x64";
}

export function targetFor(os: OS, arch: Arch): TargetId | null {
  if (os === "windows") return arch === "arm64" ? "win-arm64" : "win-x64";
  if (os === "macos") return arch === "x64" ? "mac-x64" : "mac-arm64";
  if (os === "linux") return "linux-appimage";
  return null;
}

type ApiRelease = { tag_name?: string; assets?: { name: string; browser_download_url: string; size: number }[] };

function parse(data: ApiRelease): Release {
  const assets: Release["assets"] = {};
  for (const target of TARGETS) {
    // Prefer setup installers over anything else with the same extension.
    const candidates = (data.assets ?? []).filter((a) => target.match(a.name.toLowerCase()));
    candidates.sort((a, b) => Number(/setup/i.test(b.name)) - Number(/setup/i.test(a.name)));
    const best = candidates[0];
    if (best) assets[target.id] = { url: best.browser_download_url, name: best.name, size: best.size };
  }
  const version = data.tag_name ? data.tag_name.replace(/^v/, "") : null;
  return { status: Object.keys(assets).length ? "ready" : "fallback", version, assets };
}

function readCache(): ApiRelease | null {
  try {
    const raw = sessionStorage.getItem(CACHE_KEY);
    if (!raw) return null;
    const { at, data } = JSON.parse(raw) as { at: number; data: ApiRelease };
    return Date.now() - at < CACHE_MS ? data : null;
  } catch {
    return null;
  }
}

function writeCache(data: ApiRelease) {
  try {
    const slim = { tag_name: data.tag_name, assets: data.assets?.map(({ name, browser_download_url, size }) => ({ name, browser_download_url, size })) };
    sessionStorage.setItem(CACHE_KEY, JSON.stringify({ at: Date.now(), data: slim }));
  } catch {
    /* storage unavailable */
  }
}

let pending: Promise<Release> | null = null;

function fetchRelease(): Promise<Release> {
  if (pending) return pending;
  pending = (async () => {
    const cached = readCache();
    if (cached) return parse(cached);
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 6000);
    try {
      const res = await fetch(API_URL, { headers: { Accept: "application/vnd.github+json" }, signal: controller.signal });
      if (!res.ok) throw new Error(String(res.status));
      const data = (await res.json()) as ApiRelease;
      writeCache(data);
      return parse(data);
    } catch {
      return { status: "fallback", version: null, assets: {} } satisfies Release;
    } finally {
      clearTimeout(timer);
    }
  })();
  return pending;
}

export type Downloads = {
  os: OS;
  arch: Arch;
  release: Release;
  /** The target that matches this computer, if it is a desktop. */
  primary: TargetId | null;
  urlFor: (id: TargetId) => string;
};

/** Visitor's platform plus the latest release. Starts neutral so the prerendered page matches. */
export function useDownloads(): Downloads {
  const [os, setOs] = useState<OS>("unknown");
  const [arch, setArch] = useState<Arch>("x64");
  const [release, setRelease] = useState<Release>({ status: "loading", version: null, assets: {} });

  useEffect(() => {
    let alive = true;
    const found = detectOS();
    setOs(found);
    detectArch(found).then((a) => alive && setArch(a));
    fetchRelease().then((r) => alive && setRelease(r));
    return () => {
      alive = false;
    };
  }, []);

  return {
    os,
    arch,
    release,
    primary: targetFor(os, arch),
    urlFor: (id) => release.assets[id]?.url ?? RELEASES_URL,
  };
}

export function formatSize(bytes: number): string {
  return bytes > 1e6 ? `${(bytes / 1e6).toFixed(0)} MB` : `${Math.max(1, Math.round(bytes / 1e3))} KB`;
}

export const DownloadsContext = createContext<Downloads | null>(null);

export function useDownloadInfo(): Downloads {
  const value = useContext(DownloadsContext);
  if (!value) throw new Error("DownloadsContext is missing");
  return value;
}
