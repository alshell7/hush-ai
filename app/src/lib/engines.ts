// The two voice models: names, licenses and a download hook shared by every screen.
import { useEffect, useState } from "react";
import { api, on, type Engine, type ModelProgress } from "./ipc";
import { saveSettings, setStore, useApp } from "./store";

export const ENGINES: Record<Engine, { label: string; name: string; hint: string; license: string; licenseUrl: string; terms: string }> = {
  deepFilter: {
    label: "Efficient",
    name: "DeepFilterNet 3",
    hint: "DeepFilterNet 3. Full band (48 kHz) on one CPU thread. Open source.",
    license: "MIT or Apache-2.0",
    licenseUrl: "https://github.com/Rikorose/DeepFilterNet#license",
    terms: "open source",
  },
  reuse: {
    label: "NVIDIA RE-USE",
    name: "NVIDIA Real-time RE-USE",
    hint: "NVIDIA's Mamba model, with Light, Balanced and Studio. Uses much more CPU.",
    license: "license terms",
    licenseUrl: "https://huggingface.co/alshell7/hush-ai/blob/main/LICENSE",
    terms: "",
  },
};

export const ENGINE_ORDER: Engine[] = ["deepFilter", "reuse"];

/** Download one model and follow its progress. RE-USE records that its license was accepted. */
export function useModelDownload(engine: Engine) {
  const installed = useApp((s) => !!s.models?.[engine].installed);
  const size = useApp((s) => s.models?.[engine].size ?? 0);
  const [progress, setProgress] = useState<ModelProgress | null>(null);
  const [state, setState] = useState<"idle" | "downloading" | "error">("idle");
  const [error, setError] = useState("");

  useEffect(() => {
    setProgress(null);
    setState("idle");
    setError("");
    const stop = on("model-progress", (p) => p.engine === engine && setProgress(p));
    return () => {
      stop.then((fn) => fn());
    };
  }, [engine]);

  const download = async () => {
    setState("downloading");
    setError("");
    setProgress(null);
    if (engine === "reuse") await saveSettings({ licenseAccepted: true });
    try {
      setStore({ models: await api.downloadModel(engine) });
      setState("idle");
      return true;
    } catch (e) {
      setError(String(e).split("\n")[0]);
      setState("error");
      return false;
    }
  };

  return { installed, size, progress, state, error, download, cancel: () => api.cancelModel() };
}
