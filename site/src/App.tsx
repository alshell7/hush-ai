import { domAnimation, LazyMotion, MotionConfig } from "motion/react";
import { DownloadsContext, useDownloads } from "./lib/download";
import { AudioDemo } from "./sections/AudioDemo";
import { DownloadSection } from "./sections/Download";
import { Faq } from "./sections/Faq";
import { Features } from "./sections/Features";
import { Footer } from "./sections/Footer";
import { Guides } from "./sections/Guides";
import { Hero } from "./sections/Hero";
import { HowItWorks } from "./sections/HowItWorks";
import { Nav } from "./sections/Nav";
import { Performance } from "./sections/Performance";
import { Roadmap } from "./sections/Roadmap";
import { Showcase } from "./sections/Showcase";

export function App() {
  const downloads = useDownloads();
  return (
    <LazyMotion features={domAnimation} strict>
    <MotionConfig reducedMotion="user">
      <DownloadsContext.Provider value={downloads}>
        <a href="#main" className="skip-link btn btn-primary btn-sm">
          Skip to content
        </a>
        <div className="relative overflow-x-clip">
          {/* Warm aurora behind the top of the page. Static: painted once. */}
          <div aria-hidden className="aurora aurora-fade bottom-auto h-[1150px]" />
          <Nav />
          <main id="main">
            <Hero />
            <AudioDemo />
            <Guides />
            <Showcase />
            <Features />
            <HowItWorks />
            <Performance />
            <Roadmap />
            <Faq />
            <DownloadSection />
          </main>
          <Footer />
        </div>
      </DownloadsContext.Provider>
    </MotionConfig>
    </LazyMotion>
  );
}
