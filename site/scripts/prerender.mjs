// Renders every page into its own index.html after the client build, so each paints before
// JS runs and GitHub Pages serves it directly: the home page, /docs/, every /docs/<slug>/ and
// 404.html. Each page preloads the Latin Geist font and the code its route needs.
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(root, "dist");
const ssrDir = join(root, "dist-ssr");
const BASE = "/hush-ai/";

const { render } = await import(pathToFileURL(join(ssrDir, "entry-server.js")).href);
const pages = await render();

const template = readFileSync(join(dist, "index.html"), "utf8");
if (!template.includes('<div id="root"></div>')) throw new Error("#root placeholder not found in dist/index.html");

const manifestPath = join(dist, ".vite", "manifest.json");
const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
const entryFor = (src) => manifest[src] ?? Object.values(manifest).find((m) => m.src === src || m.src === `\0${src}` || m.name === src);

/** The chunk of a module plus everything it imports statically, minus what the entry loads. */
function preloads(sources) {
  const entry = Object.values(manifest).find((m) => m.isEntry);
  const loaded = new Set([entry.file, ...(entry.imports ?? []).map((k) => manifest[k].file)]);
  const out = new Set();
  const walk = (m) => {
    if (!m || out.has(m.file) || loaded.has(m.file)) return;
    out.add(m.file);
    for (const k of m.imports ?? []) walk(manifest[k]);
  };
  for (const src of sources) {
    const m = entryFor(src);
    if (!m) throw new Error(`no chunk for ${src} in the manifest`);
    walk(m);
  }
  return [...out];
}

const font = readdirSync(join(dist, "assets")).find((f) => /^geist-latin-wght-normal.*\.woff2$/.test(f));
const escape = (s) => s.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;");

function setMeta(page, attr, key, value) {
  const re = new RegExp(`(<meta\\s+${attr}="${key}"\\s+content=")[^"]*(")`);
  if (!re.test(page)) throw new Error(`meta ${key} not found in the template`);
  return page.replace(re, `$1${escape(value)}$2`);
}

let total = 0;
for (const p of pages) {
  let page = template.replace('<div id="root"></div>', `<div id="root">${p.html}</div>`);
  page = page.replace(/<title>[^<]*<\/title>/, `<title>${escape(p.title)}</title>`);
  page = page.replace(/(<link rel="canonical" href=")[^"]*(")/, `$1${p.canonical}$2`);
  page = setMeta(page, "name", "description", p.description);
  page = setMeta(page, "property", "og:url", p.canonical);
  page = setMeta(page, "property", "og:title", p.title);
  page = setMeta(page, "property", "og:description", p.description);
  page = setMeta(page, "name", "twitter:title", p.title);
  page = setMeta(page, "name", "twitter:description", p.description);
  const links = [
    font ? `<link rel="preload" href="${BASE}assets/${font}" as="font" type="font/woff2" crossorigin />` : "",
    ...preloads(p.chunks).map((f) => `<link rel="modulepreload" crossorigin href="${BASE}${f}" />`),
  ].filter(Boolean);
  page = page.replace("</title>", `</title>\n    ${links.join("\n    ")}`);
  const out = join(dist, p.file);
  mkdirSync(dirname(out), { recursive: true });
  writeFileSync(out, page);
  total += p.html.length;
}

rmSync(ssrDir, { recursive: true, force: true });
if (existsSync(manifestPath)) rmSync(join(dist, ".vite"), { recursive: true, force: true });
console.log(`prerendered ${pages.length} pages (${(total / 1024).toFixed(0)} KB of HTML): ${pages.map((p) => `/${p.file.replace(/index\.html$/, "")}`).join(" ")}`);
