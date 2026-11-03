# Hush AI website

`npm install`, then `npm run dev` for development or `npm run build` for the static site in `dist/`. Preview the build with `npm run preview`. GitHub Pages serves it at `/hush-ai/`.

## Pages

| Route | Source |
| --- | --- |
| `/` | `src/pages/Home.tsx` and the sections in `src/sections/` |
| `/docs/` | `src/pages/Docs.tsx`: an index of every doc |
| `/docs/<slug>/` | one page per Markdown file in the repository (list in `scripts/docs.mjs`) |
| `404.html` | `src/pages/NotFound.tsx` |

The build prerenders every page to its own `index.html` (`scripts/prerender.mjs`), so each one paints before JavaScript runs and GitHub Pages serves it without a 404 trick. The browser then loads only the code of that page and hydrates it.

## Docs

`scripts/docs.mjs` is a Vite plugin. At build time it reads the Markdown files (`docs/*.md`, `README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `crates/hush-core/README.md`, `virtual_mic/README.md`, this file), renders GitHub-flavoured Markdown, highlights code with Shiki, adds heading anchors and a table of contents, and rewrites links: links between docs become site routes, links to source files become GitHub URLs on `main`. New files in `docs/` appear under "More" without any change here. Mermaid diagrams are drawn in the browser, only on pages that have one, when one comes near the screen.

## Data

- **Benchmarks:** every number is in `src/data/benchmarks.ts`.
- **App logos:** `src/assets/apps/` (copies of `assets/apps/`; sources and licenses in `assets/apps/README.md`). Where each app keeps its microphone setting: `src/components/AppGuides.tsx`.
- **A/B demo:** `public/samples/demos.json` and `public/samples/demos/` (scenes, peaks and scores). Without `demos.json` the player plays `noisy.mp3` and `hush.mp3` with `peaks.json`. The build checks which one exists, so restart `npm run dev` after adding it.
- **Live app demo:** expected at `dist/app/` (built by CI from `app/`).

After editing `public/og.svg`, run `npm run og` to re-render `og.png` and the touch icon.

## Rules

Animate only `transform` and `opacity`. Loops pause off-screen and in hidden tabs (`useLoop`, `usePauseOffscreen`), and everything honours `prefers-reduced-motion`. Nothing plays sound before a click.
