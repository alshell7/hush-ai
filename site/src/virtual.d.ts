// Build-time modules from scripts/docs.mjs, and flags from vite.config.ts.

declare const __HAS_DEMOS__: boolean;
declare const __HAS_DEMO_CREDITS__: boolean;

declare module "virtual:hush-docs" {
  export type DocMeta = {
    slug: string;
    file: string;
    title: string;
    group: string;
    icon: string;
    summary: string;
    minutes: number;
    mermaid: boolean;
  };
  export type DocContent = {
    slug: string;
    html: string;
    toc: { id: string; text: string; depth: number }[];
  };
  export const DOCS: DocMeta[];
  export const loaders: Record<string, () => Promise<{ default: DocContent }>>;
}

declare module "virtual:hush-doc/*" {
  import type { DocContent } from "virtual:hush-docs";
  const doc: DocContent;
  export default doc;
}
