// Used only at build time: renders every page to HTML so it paints before any JS runs.
import { StrictMode } from "react";
import { renderToString } from "react-dom/server";
import { DOCS, loaders } from "virtual:hush-docs";
import { Root } from "./App";
import type { Route } from "./lib/route";
import { DocPage, DocsIndex } from "./pages/Docs";
import { HomePage } from "./pages/Home";
import { NotFound } from "./pages/NotFound";

const SITE = "https://alshell7.github.io/hush-ai/";

export type Page = {
  /** Output file, relative to dist/. */
  file: string;
  /** Source module of the page's client chunk, for modulepreload links. */
  chunks: string[];
  title: string;
  description: string;
  canonical: string;
  html: string;
};

const HOME_DESCRIPTION =
  "Hush AI removes background noise from your microphone in real time and cleans up recordings, on your own computer. A free, open-source alternative to Krisp and Adobe Podcast Enhance for Windows, macOS and Linux.";

function html(route: Route, node: React.ReactNode) {
  return renderToString(
    <StrictMode>
      <Root route={route}>{node}</Root>
    </StrictMode>,
  );
}

export async function render(): Promise<Page[]> {
  const pages: Page[] = [
    {
      file: "index.html",
      chunks: ["src/pages/Home.tsx"],
      title: "Hush AI · Free, open-source noise cancellation",
      description: HOME_DESCRIPTION,
      canonical: SITE,
      html: html({ kind: "home" }, <HomePage />),
    },
    {
      file: "docs/index.html",
      chunks: ["src/pages/Docs.tsx"],
      title: "Docs · Hush AI",
      description: "Hush AI documentation: user guide, architecture, measurements, building from source and contributing.",
      canonical: `${SITE}docs/`,
      html: html({ kind: "docs" }, <DocsIndex />),
    },
    {
      file: "404.html",
      chunks: ["src/pages/NotFound.tsx"],
      title: "Page not found · Hush AI",
      description: HOME_DESCRIPTION,
      canonical: SITE,
      html: html({ kind: "404" }, <NotFound />),
    },
  ];
  for (const meta of DOCS) {
    const doc = (await loaders[meta.slug]()).default;
    pages.push({
      file: `docs/${meta.slug}/index.html`,
      chunks: ["src/pages/Docs.tsx", `virtual:hush-doc/${meta.slug}`],
      title: `${meta.title} · Hush AI docs`,
      description: meta.summary,
      canonical: `${SITE}docs/${meta.slug}/`,
      html: html({ kind: "doc", slug: meta.slug }, <DocPage doc={doc} />),
    });
  }
  return pages;
}
