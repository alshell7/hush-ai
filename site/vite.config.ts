import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// GitHub project pages serve the site from /hush-ai/.
export default defineConfig({
  base: "/hush-ai/",
  plugins: [react(), tailwindcss()],
  build: {
    outDir: "dist",
    target: "es2022",
    cssMinify: "lightningcss",
  },
});
