// Renders the brand SVGs to the PNGs the app needs. Run: node scripts/render-icons.mjs
import { Resvg } from "@resvg/resvg-js";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";

const brand = new URL("../../assets/brand/", import.meta.url);
const out = new URL("../src-tauri/icons/", import.meta.url);
mkdirSync(out, { recursive: true });

function render(name, width, target) {
  const svg = readFileSync(new URL(name, brand), "utf8");
  const png = new Resvg(svg, { fitTo: { mode: "width", value: width } }).render().asPng();
  writeFileSync(target, png);
}

render("icon.svg", 1024, new URL("../../assets/brand/icon.png", import.meta.url));
for (const [svg, png] of [
  ["tray-on.svg", "tray-on-template.png"],
  ["tray-off.svg", "tray-off-template.png"],
  // Windows and Linux: one pair for light taskbars, one for dark.
  ["tray-on-light.svg", "tray-on-light.png"],
  ["tray-off-light.svg", "tray-off-light.png"],
  ["tray-on-dark.svg", "tray-on-dark.png"],
  ["tray-off-dark.svg", "tray-off-dark.png"],
]) {
  render(svg, 64, new URL(png, out));
}
console.log("icons rendered");
