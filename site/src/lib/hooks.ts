import { useEffect, useRef, useState, useSyncExternalStore, type RefObject } from "react";

/** True while the element is on (or near) the screen and the tab is visible. */
export function useActive<T extends Element>(ref: RefObject<T | null>, rootMargin = "0px"): boolean {
  const [active, setActive] = useState(false);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    let inView = false;
    const update = () => setActive(inView && document.visibilityState === "visible");
    const io = new IntersectionObserver(
      ([entry]) => {
        inView = entry.isIntersecting;
        update();
      },
      { rootMargin },
    );
    io.observe(el);
    document.addEventListener("visibilitychange", update);
    return () => {
      io.disconnect();
      document.removeEventListener("visibilitychange", update);
    };
  }, [ref, rootMargin]);
  return active;
}

/**
 * Pauses every CSS animation inside the element while it is off-screen or the tab is
 * hidden. It writes a data attribute directly, so nothing re-renders.
 */
export function usePauseOffscreen<T extends HTMLElement>(): RefObject<T | null> {
  const ref = useRef<T>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    let inView = false;
    const update = () => {
      el.dataset.paused = String(!(inView && document.visibilityState === "visible"));
    };
    const io = new IntersectionObserver(([entry]) => {
      inView = entry.isIntersecting;
      update();
    });
    io.observe(el);
    document.addEventListener("visibilitychange", update);
    return () => {
      io.disconnect();
      document.removeEventListener("visibilitychange", update);
    };
  }, []);
  return ref;
}

/** A media query, false during prerender. */
export function useMediaQuery(query: string): boolean {
  return useSyncExternalStore(
    (onChange) => {
      const mql = window.matchMedia(query);
      mql.addEventListener("change", onChange);
      return () => mql.removeEventListener("change", onChange);
    },
    () => window.matchMedia(query).matches,
    () => false,
  );
}

/** True after hydration. Use it to render browser-only details without a mismatch. */
export function useMounted(): boolean {
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);
  return mounted;
}

export const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));
