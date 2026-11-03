// Renders the repository's Markdown docs at build time (GitHub-flavoured Markdown, Shiki
// highlighting, heading anchors, a table of contents) and serves them to the site as virtual
// modules:
//   virtual:hush-docs        the index: titles, groups, summaries and a loader per page
//   virtual:hush-doc/<slug>  one page: its HTML and table of contents
// Links between docs become site routes, links to source files become GitHub URLs on main.
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, posix, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import GithubSlugger from "github-slugger";
import { toString as hastToString } from "hast-util-to-string";
import rehypeRaw from "rehype-raw";
import rehypeStringify from "rehype-stringify";
import remarkGfm from "remark-gfm";
import remarkParse from "remark-parse";
import remarkRehype from "remark-rehype";
import { createHighlighter } from "shiki";
import { unified } from "unified";
import { SKIP, visit } from "unist-util-visit";

const SITE = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const REPO = resolve(SITE, "..");
const GITHUB = "https://github.com/alshell7/hush-ai";
const RAW = "https://raw.githubusercontent.com/alshell7/hush-ai/main";

/** Every page, in reading order. Docs added to docs/ later appear under "More" on their own. */
const PAGES = [
  { slug: "overview", file: "README.md", title: "Overview", group: "Start here", icon: "home", summary: "What Hush AI is, how to install it and how it works, on one page." },
  { slug: "guide", file: "docs/GUIDE.md", group: "Start here", icon: "book", summary: "Install, set up, choose Hush AI Microphone in your apps, and fix problems." },
  { slug: "architecture", file: "docs/ARCHITECTURE.md", group: "How it works", icon: "flow", summary: "How audio flows through Hush: threads, queues, clock drift and the delay budget." },
  { slug: "engine", file: "crates/hush-core/README.md", title: "Engine (hush-core)", group: "How it works", icon: "cpu", summary: "The Rust engine on its own: stream a signal, live devices and examples." },
  { slug: "virtual-microphone", file: "virtual_mic/README.md", group: "How it works", icon: "mic", summary: "The Hush AI Microphone drivers for macOS and Windows, and how they were tested." },
  { slug: "comparison", file: "docs/COMPARISON.md", group: "Measurements", icon: "chart", summary: "DeepFilterNet 3, NVIDIA RE-USE and RNNoise on VoiceBank-DEMAND: the method, every metric and the caveats." },
  { slug: "performance", file: "docs/PERFORMANCE.md", group: "Measurements", icon: "gauge", summary: "CPU time per frame, threads, priority under load, long runs and delay." },
  { slug: "accuracy", file: "docs/ACCURACY.md", group: "Measurements", icon: "target", summary: "Hush's CPU output against NVIDIA's GPU reference, and the regression gates in CI." },
  { slug: "build", file: "docs/BUILD.md", group: "Project", icon: "code", summary: "Develop, test and release the app, the engine and the drivers." },
  { slug: "contributing", file: "CONTRIBUTING.md", group: "Project", icon: "heart", summary: "Ways to help, and what a pull request needs." },
  { slug: "security", file: "SECURITY.md", group: "Project", icon: "shield", summary: "How to report a security problem, and what Hush does with your system." },
  { slug: "website", file: "site/README.md", title: "Website", group: "Project", icon: "globe", summary: "How this website is built and previewed." },
];

function discover() {
  const known = new Set(PAGES.map((p) => p.file));
  const extra = existsSync(join(REPO, "docs"))
    ? readdirSync(join(REPO, "docs"))
        .filter((f) => f.endsWith(".md") && !known.has(`docs/${f}`))
        .sort()
        .map((f) => ({ slug: f.replace(/\.md$/, "").toLowerCase().replace(/[^a-z0-9]+/g, "-"), file: `docs/${f}`, group: "More", icon: "book" }))
    : [];
  return [...PAGES, ...extra].filter((p) => existsSync(join(REPO, p.file)));
}

// ------------------------------------------------------------------ Highlighting

/** Shiki theme in the site's warm palette. */
const THEME = {
  name: "hush",
  type: "light",
  colors: { "editor.background": "#00000000", "editor.foreground": "#2f2a25" },
  tokenColors: [
    { scope: ["comment", "punctuation.definition.comment"], settings: { foreground: "#9a9186", fontStyle: "italic" } },
    { scope: ["keyword", "storage", "storage.type", "keyword.control", "keyword.operator.new"], settings: { foreground: "#c2553d" } },
    { scope: ["string", "string.quoted", "markup.inline.raw"], settings: { foreground: "#1d7a58" } },
    { scope: ["constant.numeric", "constant.language", "constant.character"], settings: { foreground: "#a8650a" } },
    { scope: ["entity.name.function", "support.function", "meta.function-call"], settings: { foreground: "#127a75" } },
    { scope: ["entity.name.type", "support.type", "entity.name.class", "entity.name.namespace"], settings: { foreground: "#8a5212" } },
    { scope: ["variable.parameter", "variable.other.member"], settings: { foreground: "#4a453f" } },
    { scope: ["entity.name.tag", "support.variable", "variable.other.constant"], settings: { foreground: "#b5482f" } },
    { scope: ["keyword.operator", "punctuation"], settings: { foreground: "#7a7268" } },
    { scope: ["variable.other.readwrite", "variable"], settings: { foreground: "#2f2a25" } },
  ],
};
const LANGS = ["sh", "bash", "powershell", "rust", "json", "toml", "ts", "tsx", "c", "cpp", "yaml"];
const ALIAS = { shell: "sh", console: "sh", zsh: "sh", ps1: "powershell", rs: "rust", js: "ts" };
let highlighter;
async function getHighlighter() {
  highlighter ??= createHighlighter({ themes: [THEME], langs: LANGS });
  return highlighter;
}

// ------------------------------------------------------------------ Helpers

const h = (tagName, properties = {}, children = []) => ({ type: "element", tagName, properties, children });
const text = (value) => ({ type: "text", value });

/** Width and height of a PNG, read from its header, so images reserve their space. */
function pngSize(file) {
  try {
    const b = readFileSync(file);
    if (b.readUInt32BE(12) === 0x49484452) return { width: b.readUInt32BE(16), height: b.readUInt32BE(20) };
  } catch {
    /* not a PNG */
  }
  return null;
}

/** Number at the start of a table cell ("11.7 ms", "**90%**", "8,180 …"); null if none. */
function cellNumber(value) {
  const m = value.trim().match(/^[~≈]?\s*([+−-]?)(\d[\d,]*(?:\.\d+)?)/);
  if (!m) return null;
  const n = Number(m[2].replace(/,/g, ""));
  return m[1] ? NaN : n; // signed values (differences) get no bar
}

// ------------------------------------------------------------------ Rendering

function renderer({ base, pages }) {
  const byFile = new Map(pages.map((p) => [p.file, p]));
  const route = (slug) => `${base}docs/${slug}/`;

  /** Where a link in `from` (a repository path) should go on the site. */
  function href(url, from) {
    if (/^(https?:|mailto:|#)/i.test(url)) return { url, external: /^https?:/i.test(url) };
    const [path, hash = ""] = url.split("#");
    const target = posix.normalize(posix.join(posix.dirname(from), decodeURIComponent(path))).replace(/^\.\//, "").replace(/\/$/, "");
    const anchor = hash ? `#${hash}` : "";
    const page = byFile.get(target);
    if (page) return { url: `${route(page.slug)}${anchor}`, external: false };
    if (target.startsWith("site/public/")) return { url: `${base}${target.slice("site/public/".length)}`, external: false };
    const abs = join(REPO, target);
    const kind = existsSync(abs) && statSync(abs).isDirectory() ? "tree" : "blob";
    return { url: `${GITHUB}/${kind}/main/${target}${anchor}`, external: true };
  }

  return async function render(page) {
    const source = readFileSync(join(REPO, page.file), "utf8");
    const shiki = await getHighlighter();
    const slugger = new GithubSlugger();
    const toc = [];
    let title = page.title;
    let mermaid = 0;
    let summary = page.summary;

    // Markdown tree: drop the HTML banner at the top of a README and the "Home · Guide · …"
    // breadcrumb line that GitHub needs but the site's sidebar replaces.
    const stripChrome = () => (tree) => {
      let i = 0;
      while (i < tree.children.length && tree.children[i].type === "html") i++;
      tree.children.splice(0, i);
      const h1 = tree.children.findIndex((n) => n.type === "heading" && n.depth === 1);
      if (h1 >= 0) {
        const raw = tree.children[h1].children.map((c) => c.value ?? c.children?.map((d) => d.value).join("") ?? "").join("");
        title ??= raw.trim();
        tree.children.splice(h1, 1);
      }
      const first = tree.children[0];
      if (first?.type === "paragraph" && first.children.every((c) => c.type === "link" || (c.type === "text" && /^[\s·|]*$/.test(c.value)))) {
        tree.children.shift();
      }
      if (!summary) {
        const p = tree.children.find((n) => n.type === "paragraph");
        const plain = (n) => (n.value ?? "") + (n.children ?? []).map(plain).join("");
        if (p) summary = plain(p).replace(/\s+/g, " ").trim().slice(0, 180);
      }
    };

    const transform = () => (tree) => {
      // Raw HTML parsing moves the whitespace of tables out of them; keep one newline.
      tree.children = tree.children.filter((n, i, all) => !(n.type === "text" && !n.value.trim() && all[i - 1]?.type === "text"));
      for (const n of tree.children) if (n.type === "text" && !n.value.trim()) n.value = String.fromCharCode(10);
      visit(tree, "element", (node, index, parent) => {
        const p = node.properties ?? (node.properties = {});

        // Headings: GitHub-style ids, an anchor link, and the table of contents.
        if (/^h[2-4]$/.test(node.tagName)) {
          const label = hastToString(node).trim();
          const id = slugger.slug(label);
          p.id = id;
          if (node.tagName !== "h4") toc.push({ id, text: label, depth: Number(node.tagName[1]) });
          node.children = [
            ...node.children,
            h("a", { className: ["anchor"], href: `#${id}`, ariaLabel: `Link to “${label}”` }, [text("#")]),
          ];
          return;
        }

        if (node.tagName === "a" && typeof p.href === "string") {
          const { url, external } = href(p.href, page.file);
          p.href = url;
          if (external) {
            p.target = "_blank";
            p.rel = ["noopener"];
            p.className = [...(p.className ?? []), "ext"];
          }
          return;
        }

        if (node.tagName === "img" && typeof p.src === "string") {
          if (!/^https?:/i.test(p.src)) {
            const target = posix.normalize(posix.join(posix.dirname(page.file), p.src));
            const size = pngSize(join(REPO, target));
            p.src = `${RAW}/${target}`;
            if (size) {
              const width = Number(p.width) || size.width;
              p.width = width;
              p.height = Math.round((width * size.height) / size.width);
            }
          }
          p.loading = "lazy";
          p.decoding = "async";
          delete p.align;
          return;
        }

        if (node.tagName === "p" && p.align) {
          delete p.align;
          p.className = ["center"];
          return;
        }

        // GitHub alerts: > [!NOTE] …
        if (node.tagName === "blockquote") {
          const firstP = node.children.find((c) => c.type === "element" && c.tagName === "p");
          const firstText = firstP?.children[0];
          const m = firstText?.type === "text" && firstText.value.match(/^\s*\[!(NOTE|TIP|IMPORTANT|WARNING|CAUTION)\]\s*/);
          if (m) {
            firstText.value = firstText.value.slice(m[0].length);
            const kind = m[1].toLowerCase();
            node.tagName = "aside";
            node.properties = { className: ["callout", `callout-${kind}`] };
            node.children = [h("p", { className: ["callout-title"] }, [text(kind[0].toUpperCase() + kind.slice(1))]), ...node.children];
          }
          return;
        }

        // Code blocks: Shiki at build time; Mermaid diagrams are drawn in the browser.
        if (node.tagName === "pre" && node.children[0]?.tagName === "code" && parent) {
          const code = node.children[0];
          const lang = (code.properties.className ?? []).map(String).find((c) => c.startsWith("language-"))?.slice(9) ?? "text";
          const value = hastToString(code).replace(/\n$/, "");
          if (lang === "mermaid") {
            mermaid++;
            // Short left-to-right charts stay wide; long ones are drawn top to bottom so their
            // text stays readable. The browser reserves the box for each shape in CSS.
            const wide = /^\s*(flowchart|graph)\s+(LR|RL)/m.test(value);
            const edges = (value.match(/-->/g) ?? []).length;
            const dir = wide ? (edges <= 4 ? "lr" : "tall") : "tb";
            parent.children[index] = h("figure", { className: ["diagram"], dataMermaid: dir }, [
              h("div", { className: ["diagram-stage"], ariaHidden: "true" }),
              h("pre", { className: ["diagram-src"] }, [h("code", {}, [text(value)])]),
            ]);
            return SKIP;
          }
          const id = ALIAS[lang] ?? lang;
          const known = LANGS.includes(id);
          const root = shiki.codeToHast(value, { lang: known ? id : "text", theme: "hush" });
          const pre = root.children.find((c) => c.tagName === "pre");
          pre.properties = { className: ["shiki"], tabIndex: 0 };
          const label = { sh: "Shell", bash: "Shell", powershell: "PowerShell", rust: "Rust", json: "JSON", toml: "TOML", ts: "TypeScript", tsx: "TSX", c: "C", cpp: "C++", yaml: "YAML" }[id] ?? "Text";
          parent.children[index] = h("div", { className: ["code"] }, [
            h("div", { className: ["code-bar"] }, [
              h("span", {}, [text(label)]),
              h("button", { type: "button", className: ["code-copy"], dataCopy: "" }, [text("Copy")]),
            ]),
            pre,
          ]);
          return SKIP;
        }

        // Tables: scroll on their own on small screens; numeric columns get small bars.
        if (node.tagName === "table" && parent) {
          const rows = [];
          visit(node, "element", (r) => {
            if (r.tagName === "tr") rows.push(r.children.filter((c) => c.type === "element"));
          });
          const body = rows.slice(1);
          const cols = rows[0]?.length ?? 0;
          for (let c = 0; c < cols; c++) {
            const right = rows.every((r) => !r[c] || r[c].properties?.align === "right");
            for (const r of rows) {
              if (!r[c]) continue;
              const cp = r[c].properties;
              delete cp.align;
              if (right) cp.className = [...(cp.className ?? []), "num"];
            }
            if (!right || /thread|version|rev/i.test(hastToString(rows[0][c] ?? text("")))) continue;
            const values = body.map((r) => (r[c] ? cellNumber(hastToString(r[c])) : null));
            const nums = values.filter((v) => v !== null && Number.isFinite(v));
            if (values.some((v) => Number.isNaN(v)) || nums.length < 3) continue;
            const max = Math.max(...nums);
            if (!(max > 0) || Math.min(...nums) === max) continue;
            body.forEach((r, i) => {
              const v = values[i];
              if (v === null || !Number.isFinite(v) || !r[c]) return;
              r[c].properties.className = [...r[c].properties.className, "bar"];
              r[c].properties.style = `--v:${(v / max).toFixed(3)}`;
            });
          }
          // Keep walking into the table (its links need rewriting too).
          parent.children[index] = h("div", { className: ["table-wrap"], tabIndex: 0, role: "region", ariaLabel: "Table" }, [node]);
          return;
        }

        if (node.tagName === "details") {
          p.className = [...(p.className ?? []), "disclosure"];
        }
      });
    };

    const file = await unified()
      .use(remarkParse)
      .use(remarkGfm)
      .use(stripChrome)
      .use(remarkRehype, { allowDangerousHtml: true })
      .use(rehypeRaw)
      .use(transform)
      .use(rehypeStringify)
      .process(source);

    const html = String(file);
    const words = html.replace(/<[^>]+>/g, " ").split(/\s+/).filter(Boolean).length;
    return { title: title ?? page.slug, summary: summary ?? "", html, toc, mermaid: mermaid > 0, minutes: Math.max(1, Math.round(words / 230)) };
  };
}

// ------------------------------------------------------------------ Vite plugin

const INDEX = "virtual:hush-docs";
const PAGE = "virtual:hush-doc/";

export function docsPlugin() {
  let base = "/";
  let cache = null;
  const build = () => {
    cache ??= (async () => {
      const pages = discover();
      const render = renderer({ base, pages });
      const out = [];
      for (const page of pages) out.push({ ...page, ...(await render(page)) });
      return out;
    })();
    return cache;
  };

  return {
    name: "hush-docs",
    configResolved(config) {
      base = config.base;
    },
    resolveId(id) {
      if (id === INDEX || id.startsWith(PAGE)) return `\0${id}`;
    },
    async load(id) {
      if (!id.startsWith("\0virtual:hush-doc")) return;
      const docs = await build();
      for (const d of docs) this.addWatchFile(join(REPO, d.file));
      if (id === `\0${INDEX}`) {
        const meta = docs.map(({ slug, file, title, group, icon, summary, minutes, mermaid }) => ({ slug, file, title, group, icon, summary, minutes, mermaid }));
        const loaders = docs.map((d) => `  ${JSON.stringify(d.slug)}: () => import(${JSON.stringify(PAGE + d.slug)}),`).join("\n");
        return `export const DOCS = ${JSON.stringify(meta)};\nexport const loaders = {\n${loaders}\n};\n`;
      }
      const slug = id.slice(`\0${PAGE}`.length);
      const doc = docs.find((d) => d.slug === slug);
      if (!doc) throw new Error(`unknown doc ${slug}`);
      return `export default ${JSON.stringify({ slug: doc.slug, html: doc.html, toc: doc.toc })};\n`;
    },
    configureServer(server) {
      const files = discover().map((p) => join(REPO, p.file));
      server.watcher.add(files);
      server.watcher.on("change", (f) => {
        if (!files.some((x) => resolve(x) === resolve(f))) return;
        cache = null;
        for (const mod of server.moduleGraph.getModulesByFile?.(f) ?? []) server.moduleGraph.invalidateModule(mod);
        for (const [id, mod] of server.moduleGraph.idToModuleMap) if (id.startsWith("\0virtual:hush-doc")) server.moduleGraph.invalidateModule(mod);
        server.ws.send({ type: "full-reload" });
      });
    },
  };
}

