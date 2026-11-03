// Window title bar. macOS draws its own traffic lights over the content; Windows and Linux
// get minimal controls. The whole bar drags the window.
import { isTauri } from "../lib/ipc";
import { useApp } from "../lib/store";
import { CloseIcon, MinimizeIcon } from "./Icons";

async function windowAction(action: "minimize" | "close") {
  if (!isTauri) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const win = getCurrentWindow();
  if (action === "minimize") await win.minimize();
  else await win.close();
}

export function TitleBar({ children }: { children?: React.ReactNode }) {
  const platform = useApp((s) => s.platform);
  const mac = platform === "macos";
  return (
    <header data-tauri-drag-region className="relative z-20 flex h-14 shrink-0 items-center px-5" style={{ paddingLeft: mac ? 86 : 20 }}>
      <div data-tauri-drag-region className="flex flex-1 items-center gap-3">
        {children}
      </div>
      {!mac && (
        <div className="ml-2 flex items-center gap-1">
          <button aria-label="Minimize" className="btn btn-ghost btn-icon h-8 w-8" onClick={() => windowAction("minimize")}>
            <MinimizeIcon size={16} />
          </button>
          <button aria-label="Close to tray" className="btn btn-ghost btn-icon h-8 w-8 hover:!bg-[#f3d6d2]" onClick={() => windowAction("close")}>
            <CloseIcon size={16} />
          </button>
        </div>
      )}
    </header>
  );
}
