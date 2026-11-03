# Hush AI website

`npm install`, then `npm run dev` for development or `npm run build` for the static site in `dist/` (prerendered; served at `/hush-ai/` on GitHub Pages). Preview the build with `npm run preview`.
The live app demo is expected at `dist/app/` (built by CI); `public/samples/` holds the A/B audio (`noisy.mp3`, `hush.mp3`, `peaks.json`). After editing `public/og.svg`, run `npm run og` to re-render `og.png` and the touch icon.
