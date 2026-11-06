import { useEffect } from 'react';
import type { RefObject } from 'react';

/** Room left beside a pill scrolled into view, px (the rows' own padding). */
const MARGIN_PX = 16;

/**
 * Keeps the chosen pill of a sideways row in view. One chosen elsewhere (a link that opens
 * Search on Audiobooks, a view kept from before) may sit past the row's edge, where nothing
 * shows which one is on. Only the row scrolls, not the page.
 */
export const usePressedInView = (row: RefObject<HTMLElement | null>, chosen: unknown) => {
  useEffect(() => {
    const element = row.current;
    const pill = element?.querySelector<HTMLElement>(
      '[aria-pressed="true"], [aria-selected="true"]',
    );
    if (!element || !pill) return;
    const box = element.getBoundingClientRect();
    const place = pill.getBoundingClientRect();
    const left =
      place.left < box.left
        ? place.left - box.left - MARGIN_PX
        : place.right > box.right
          ? place.right - box.right + MARGIN_PX
          : 0;
    // jsdom has no scrollBy.
    if (left !== 0) element.scrollBy?.({ left });
  }, [row, chosen]);
};
