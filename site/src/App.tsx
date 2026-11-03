import { domAnimation, LazyMotion, MotionConfig } from "motion/react";
import type { ReactNode } from "react";
import { DownloadsContext, useDownloads } from "./lib/download";
import { routeKey, type Route } from "./lib/route";
import { Footer } from "./sections/Footer";
import { Nav } from "./sections/Nav";

/** The frame every page shares: motion settings, release info, navigation and footer. */
export function Root({ route, children }: { route: Route; children: ReactNode }) {
  const downloads = useDownloads();
  return (
    <LazyMotion features={domAnimation} strict>
      <MotionConfig reducedMotion="user">
        <DownloadsContext.Provider value={downloads}>
          <a href="#main" className="skip-link btn btn-primary btn-sm">
            Skip to content
          </a>
          <div className="relative overflow-x-clip" data-route={routeKey(route)}>
            {/* Warm aurora behind the top of the page. Static: painted once. */}
            <div aria-hidden className={`aurora aurora-fade bottom-auto ${route.kind === "home" ? "h-[1150px]" : "h-[620px] opacity-80"}`} />
            <Nav route={route} />
            <main id="main">{children}</main>
            <Footer />
          </div>
        </DownloadsContext.Provider>
      </MotionConfig>
    </LazyMotion>
  );
}
