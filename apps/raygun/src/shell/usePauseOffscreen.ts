import { useEffect } from 'react';
import type { RefObject } from 'react';

/**
 * Pauses an element's CSS animations while it's off screen (scrolled away in a long list): one
 * shared IntersectionObserver marks it `data-offscreen`, which styles.css pauses. Nothing
 * re-renders; the attribute is set on the element itself.
 */
let observer: IntersectionObserver | null = null;
let watched = 0;

const shared = () => {
  observer ??= new IntersectionObserver((entries) => {
    for (const entry of entries) {
      const element = entry.target as HTMLElement;
      if (entry.isIntersecting) delete element.dataset.offscreen;
      else element.dataset.offscreen = '';
    }
  });
  return observer;
};

/** How many elements are watched now (tests). */
export const offscreenWatched = () => watched;

export const usePauseOffscreen = (ref: RefObject<HTMLElement | null>, enabled = true) => {
  useEffect(() => {
    const element = ref.current;
    if (!enabled || !element || typeof IntersectionObserver === 'undefined') return;
    const watcher = shared();
    watcher.observe(element);
    watched += 1;
    return () => {
      watcher.unobserve(element);
      delete element.dataset.offscreen;
      watched -= 1;
      if (watched === 0) {
        watcher.disconnect();
        observer = null;
      }
    };
  }, [ref, enabled]);
};
