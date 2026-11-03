import { StrictMode, type ReactNode } from "react";
import { createRoot, hydrateRoot } from "react-dom/client";
import { loaders } from "virtual:hush-docs";
import { Root } from "./App";
import { parseRoute, routeKey, type Route } from "./lib/route";
import "./styles.css";

/** Loads only the code (and, for a doc, the content) of the page in the address bar. */
async function page(route: Route): Promise<ReactNode> {
  switch (route.kind) {
    case "home": {
      const { HomePage } = await import("./pages/Home");
      return <HomePage />;
    }
    case "docs": {
      const { DocsIndex } = await import("./pages/Docs");
      return <DocsIndex />;
    }
    case "doc": {
      const [{ DocPage }, doc] = await Promise.all([import("./pages/Docs"), loaders[route.slug]()]);
      return <DocPage doc={doc.default} />;
    }
    default: {
      const { NotFound } = await import("./pages/NotFound");
      return <NotFound />;
    }
  }
}

const route = parseRoute(location.pathname);
const content = await page(route);
const root = document.getElementById("root")!;
const app = (
  <StrictMode>
    <Root route={route}>{content}</Root>
  </StrictMode>
);

// The build prerenders every page into #root; hydrate it. In development it starts empty,
// and a server that answers an unknown address with another page gets a fresh render.
const prerendered = root.querySelector("[data-route]")?.getAttribute("data-route");
if (prerendered === routeKey(route)) hydrateRoot(root, app);
else {
  root.textContent = "";
  createRoot(root).render(app);
}
