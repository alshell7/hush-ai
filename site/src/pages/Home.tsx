import { Apps } from "../sections/Apps";
import { AudioDemo } from "../sections/AudioDemo";
import { Benchmarks } from "../sections/Benchmarks";
import { Built } from "../sections/Built";
import { Calendar } from "../sections/Calendar";
import { Compare } from "../sections/Compare";
import { DownloadSection } from "../sections/Download";
import { Enhance } from "../sections/Enhance";
import { Faq } from "../sections/Faq";
import { Features } from "../sections/Features";
import { Hero } from "../sections/Hero";
import { Roadmap } from "../sections/Roadmap";
import { Showcase } from "../sections/Showcase";
import { Translate } from "../sections/Translate";

export function HomePage() {
  return (
    <>
      <Hero />
      <AudioDemo />
      <Built />
      <Compare />
      <Features />
      <Apps />
      <Enhance />
      <Translate />
      <Calendar />
      <Showcase />
      <Benchmarks />
      <Roadmap />
      <Faq />
      <DownloadSection />
    </>
  );
}
