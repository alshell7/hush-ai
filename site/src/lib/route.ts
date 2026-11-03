// The site's pages. Each one is prerendered to its own index.html, so GitHub Pages serves it
// directly; the browser picks the same page from its address before hydrating.
import { DOCS } from "virtual:hush-docs";

export const BASE = import.meta.env.BASE_URL;

export type Route = { kind: "home" } | { kind: "docs" } | { kind: "doc"; slug: string } | { kind: "404" };

export function parseRoute(pathname: string): Route {
  let path = pathname.startsWith(BASE) ? pathname.slice(BASE.length) : pathname.replace(/^\//, "");
  path = path.replace(/index\.html$/, "").replace(/\/+$/, "");
  if (path === "") return { kind: "home" };
  if (path === "docs") return { kind: "docs" };
  const m = path.match(/^docs\/([a-z0-9-]+)$/);
  if (m && DOCS.some((d) => d.slug === m[1])) return { kind: "doc", slug: m[1] };
  return { kind: "404" };
}

/** Written into the prerendered page, so the browser knows which page it is hydrating. */
export const routeKey = (r: Route) => (r.kind === "doc" ? `doc:${r.slug}` : r.kind);

export const docHref = (slug: string) => `${BASE}docs/${slug}/`;
export const DOCS_HREF = `${BASE}docs/`;
/** A link to a section of the home page that works from every page. */
export const homeHref = (hash = "") => `${BASE}${hash}`;
