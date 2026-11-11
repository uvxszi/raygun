import { useEffect, useState } from 'react';

/** Loads and decodes a picture off screen; settles either way. */
const decoded = (src: string): Promise<unknown> => {
  const next = new Image();
  next.decoding = 'async';
  if (typeof next.decode !== 'function') {
    return new Promise((resolve) => {
      next.onload = resolve;
      next.onerror = resolve;
      next.src = src;
    });
  }
  next.src = src;
  return next.decode();
};

/**
 * The picture a reader page shows: `wanted`, except that while another picture of the same page
 * is on screen (`onScreen`: it loaded), that one stays until `wanted` has loaded and decoded. A
 * reader AI layer's picture, or the page as it came once the layers are off, then swaps in on the
 * same element without the page going blank or changing size. Another page (`page`), or a page
 * that hasn't shown yet, takes `wanted` at once; a picture that fails to decode swaps in anyway,
 * so the page's own failure shows.
 */
export const useSteadyPicture = (page: string, wanted: string, onScreen: boolean): string => {
  // What was asked for when this last drew, and the picture kept while `wanted` decodes.
  const [asked, setAsked] = useState({ page, src: wanted });
  const [held, setHeld] = useState<string | null>(null);
  if (asked.page !== page || asked.src !== wanted) {
    setAsked({ page, src: wanted });
    const keep = asked.page === page && onScreen ? (held ?? asked.src) : null;
    setHeld(keep === wanted ? null : keep);
  }
  useEffect(() => {
    if (held === null) return;
    let stopped = false;
    const swap = () => {
      if (!stopped) setHeld(null);
    };
    decoded(wanted).then(swap, swap);
    return () => {
      stopped = true;
    };
  }, [held, wanted]);
  return held ?? wanted;
};
