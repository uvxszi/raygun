import { useCallback, useState } from 'react';

/**
 * Whether an element is on screen or about to be, for pictures that load together once their
 * place is near: a cover made of four songs' covers asks for all four at once instead of leaving
 * each to the browser's lazy loading. One shared IntersectionObserver watches every element;
 * once near, an element stays near. Without IntersectionObserver everything counts as near.
 */

/** How far off screen counts as near: a screen's height above and below, half its width aside. */
const MARGIN = '100% 50%';

const waiting = new Map<Element, () => void>();
let observer: IntersectionObserver | null = null;

const unwatch = (element: Element) => {
  if (!waiting.delete(element)) return;
  observer?.unobserve(element);
  if (waiting.size === 0) {
    observer?.disconnect();
    observer = null;
  }
};

const shared = () => {
  observer ??= new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        const becameNear = waiting.get(entry.target);
        unwatch(entry.target);
        becameNear?.();
      }
    },
    { rootMargin: MARGIN },
  );
  return observer;
};

/** `[near, ref]`: put `ref` on the element; `near` turns true once it's near the screen. */
export const useNearScreen = () => {
  const [near, setNear] = useState(() => typeof IntersectionObserver === 'undefined');
  const ref = useCallback((element: Element | null) => {
    if (!element || typeof IntersectionObserver === 'undefined') return;
    waiting.set(element, () => setNear(true));
    shared().observe(element);
    return () => unwatch(element);
  }, []);
  return [near, ref] as const;
};
