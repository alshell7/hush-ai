// Renders the social preview (public/og.svg → og.png, 1200×630) and the Apple touch icon.
// Run with `npm run og` after changing either SVG. Geist TTFs (SIL OFL) live in scripts/fonts.
import { Resvg } from "@resvg/resvg-js";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const pub = join(here, "..", "public");
const font = {
  fontFiles: ["Geist-Medium.ttf", "Geist-SemiBold.ttf", "Geist-Bold.ttf"].map((f) => join(here, "fonts", f)),
  loadSystemFonts: false,
  defaultFontFamily: "Geist",
};

function render(svgPath, outPath, width) {
  const resvg = new Resvg(readFileSync(svgPath), { font, fitTo: { mode: "width", value: width }, background: "rgba(0,0,0,0)" });
  const png = resvg.render().asPng();
  writeFileSync(outPath, png);
  console.log(`${outPath} (${(png.length / 1024).toFixed(0)} KB)`);
}

render(join(pub, "og.svg"), join(pub, "og.png"), 1200);
render(join(here, "icon.svg"), join(pub, "apple-touch-icon.png"), 180);
