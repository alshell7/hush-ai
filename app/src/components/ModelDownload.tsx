// The license of a voice model and its download button, for the main screen and Settings.
import { useState } from "react";
import { ENGINES, useModelDownload } from "../lib/engines";
import { formatBytes, openUrl, type Engine } from "../lib/ipc";
import { useApp } from "../lib/store";
import { Spinner } from "./Controls";
import { DownloadIcon } from "./Icons";

/** RE-USE needs the NVIDIA license accepted; DeepFilterNet 3 only says what its license allows. */
export function LicenseNote({ engine, agreed, onAgree, disabled }: { engine: Engine; agreed: boolean; onAgree: (agreed: boolean) => void; disabled?: boolean }) {
  const info = ENGINES[engine];
  const link = (
    <a className="link" onClick={(e) => (e.preventDefault(), openUrl(info.licenseUrl))}>
      {info.license}
    </a>
  );
  if (engine === "deepFilter") {
    return (
      <p className="text-[13px] leading-snug text-ink-2">
        {info.name} by Hendrik Schröter is open source under the {link} license: {info.terms}.
      </p>
    );
  }
  return (
    <label className="flex cursor-pointer items-start gap-3 text-[13px] leading-snug text-ink-2">
      <input type="checkbox" className="mt-0.5 h-4 w-4 accent-[#1e1b18]" checked={agreed} disabled={disabled} onChange={(e) => onAgree(e.target.checked)} />
      <span>
        I accept the {link} of the model weights: {info.terms}.
      </span>
    </label>
  );
}

/** Shown while the chosen model is not downloaded. */
export function ModelDownload({ engine }: { engine: Engine }) {
  const accepted = useApp((s) => s.settings?.licenseAccepted ?? false);
  const [agreed, setAgreed] = useState(accepted);
  const model = useModelDownload(engine);
  if (model.installed) return null;
  const downloading = model.state === "downloading";
  return (
    <div className="mt-3 space-y-2.5 rounded-2xl bg-white/70 p-3" style={{ boxShadow: "inset 0 0 0 1px var(--h-line)" }}>
      <LicenseNote engine={engine} agreed={agreed} onAgree={setAgreed} disabled={downloading} />
      <div className="flex items-center gap-3">
        {downloading ? (
          <button className="btn btn-soft btn-sm" onClick={model.cancel}>
            <Spinner size={14} /> Cancel
          </button>
        ) : (
          <button className="btn btn-primary btn-sm" disabled={engine === "reuse" && !agreed} onClick={model.download}>
            <DownloadIcon size={15} /> {model.state === "error" ? "Try again" : `Download ${formatBytes(model.size)}`}
          </button>
        )}
        <span className="text-[12px] tabular-nums text-muted">
          {downloading && model.progress && `${formatBytes(model.progress.received)} of ${formatBytes(model.progress.total)}`}
        </span>
      </div>
      {model.state === "error" && <div className="rounded-xl bg-[#fdecea] px-3 py-2 text-[12.5px] text-red">{model.error}</div>}
    </div>
  );
}
