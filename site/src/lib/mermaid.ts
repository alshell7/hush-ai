// Draws the docs' Mermaid diagrams in the browser. Loaded only on pages that have one, and
// only when a diagram comes near the screen. Each diagram fills a box whose size is set in
// CSS from the start, so drawing it moves nothing on the page.
import mermaid from "mermaid";

let ready = false;
let count = 0;

function init() {
  if (ready) return;
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: "strict",
    theme: "base",
    fontFamily: '"Geist Variable", ui-sans-serif, system-ui, sans-serif',
    themeVariables: {
      fontSize: "14px",
      primaryColor: "#fff6ef",
      primaryBorderColor: "#e9b9a5",
      primaryTextColor: "#1e1b18",
      secondaryColor: "#effaf2",
      tertiaryColor: "#fbfaf7",
      lineColor: "#a39a8f",
      textColor: "#4a453f",
      clusterBkg: "#fbf8f3",
      clusterBorder: "#e3dcd2",
      edgeLabelBackground: "#ffffff",
    },
    flowchart: { curve: "basis", padding: 14, nodeSpacing: 36, rankSpacing: 44 },
  });
  ready = true;
}

/** Renders every diagram in `figures` that is not drawn yet. */
export async function renderDiagrams(figures: HTMLElement[]): Promise<void> {
  init();
  const narrow = window.matchMedia("(max-width: 639px)").matches;
  for (const fig of figures) {
    if (fig.dataset.ready) continue;
    const stage = fig.querySelector<HTMLElement>(".diagram-stage");
    const source = fig.querySelector(".diagram-src")?.textContent ?? "";
    if (!stage || !source.trim()) continue;
    // Long left-to-right charts, and every one on a narrow screen, are drawn top to bottom;
    // CSS reserves that shape.
    const flip = fig.dataset.mermaid === "tall" || (narrow && fig.dataset.mermaid === "lr");
    const code = flip ? source.replace(/^(\s*(?:flowchart|graph)\s+)(LR|RL)\b/m, "$1TB") : source;
    try {
      const { svg } = await mermaid.render(`hush-diagram-${++count}`, code);
      stage.innerHTML = svg;
      fig.dataset.ready = "true";
    } catch {
      fig.dataset.ready = "failed";
    }
  }
}
