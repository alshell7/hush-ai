// Renders the page into dist/index.html after the client build, so it paints before JS
// runs, and preloads the Latin Geist font file.
import { readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(root, "dist");
const ssrDir = join(root, "dist-ssr");

const { render } = await import(pathToFileURL(join(ssrDir, "entry-server.js")).href);
const html = render();

let page = readFileSync(join(dist, "index.html"), "utf8");
if (!page.includes('<div id="root"></div>')) throw new Error("#root placeholder not found in dist/index.html");
page = page.replace('<div id="root"></div>', `<div id="root">${html}</div>`);

const font = readdirSync(join(dist, "assets")).find((f) => /^geist-latin-wght-normal.*\.woff2$/.test(f));
if (font) {
  page = page.replace(
    "</title>",
    `</title>\n    <link rel="preload" href="/hush-ai/assets/${font}" as="font" type="font/woff2" crossorigin />`,
  );
}

writeFileSync(join(dist, "index.html"), page);
rmSync(ssrDir, { recursive: true, force: true });
console.log(`prerendered ${(html.length / 1024).toFixed(0)} KB of HTML${font ? `, preloading ${font}` : ""}`);
