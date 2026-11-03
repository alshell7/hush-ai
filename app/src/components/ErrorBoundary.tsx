// A crash in one view must not blank the window. Show a calm message and a way back.
import { Component, type ReactNode } from "react";
import { Mochi } from "./Mochi";

export class ErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  render() {
    if (!this.state.error) return this.props.children;
    return (
      <div className="grid h-full place-items-center p-8 text-center">
        <div className="max-w-[380px] space-y-3">
          <div className="flex justify-center">
            <Mochi size={88} mood="oops" />
          </div>
          <h1 className="text-[20px] font-[640] tracking-[-0.02em]">Something went wrong here</h1>
          <p className="text-[13px] text-muted">Noise cancellation keeps working. Reload this window to continue.</p>
          <pre className="max-h-24 overflow-auto rounded-xl bg-white/70 p-2 text-left text-[11px] text-muted">{String(this.state.error.message)}</pre>
          <button className="btn btn-primary" onClick={() => location.reload()}>
            Reload
          </button>
        </div>
      </div>
    );
  }
}
