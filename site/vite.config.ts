import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { existsSync } from "node:fs";
import { defineConfig } from "vite";
import { docsPlugin } from "./scripts/docs.mjs";

// The A/B demo reads samples/demos.json when it is there; without it, it plays the original
// sample and makes no request that would fail.
const samples = new URL("./public/samples/", import.meta.url);
const hasDemos = existsSync(new URL("demos.json", samples));
const hasCredits = existsSync(new URL("demos/CREDITS.md", samples));

// GitHub project pages serve the site from /hush-ai/.
export default defineConfig({
  base: "/hush-ai/",
  plugins: [react(), tailwindcss(), docsPlugin()],
  define: {
    __HAS_DEMOS__: JSON.stringify(hasDemos),
    __HAS_DEMO_CREDITS__: JSON.stringify(hasCredits),
  },
  build: {
    outDir: "dist",
    target: "es2022",
    cssMinify: "lightningcss",
    manifest: true,
    // Mermaid is large and only loads on the two docs pages that draw diagrams.
    chunkSizeWarningLimit: 900,
  },
});
