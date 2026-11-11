import { useCallback, useEffect, useEffectEvent, useRef, useState } from 'react';

import { createGlide } from '../../player/lyricsGlide';
import type { Glide } from '../../player/lyricsGlide';
import { onScrollbar } from '../../shell/scrollbarReveal';

/** After the reader scrolls or turns pages by hand, reading isn't followed for this long. */
export const HOLD_MS = 6000;
/** Where the sentence being read is brought to: this far down the text's box. */
const READING_LINE = 0.38;
/** While the sentence is between these (of the box's height), the text stays where it is. */
const BAND_TOP = 0.18;
const BAND_BOTTOM = 0.68;
/** A finger that moves farther than this scrolls; less is a tap. */
const SCROLL_SLOP_PX = 12;
/** Keys that scroll a page. */
const SCROLL_KEYS = new Set(['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Home', 'End', ' ']);

/** The sentence being read in a block (its mark), else the block itself. */
const spoken = (block: HTMLElement) => block.querySelector<HTMLElement>('[data-spoken]') ?? block;

type Follow = {
  /** Pages instead of a scrolling column. */
  paged: boolean;
  /** The block being read aloud, and where its sentence starts. */
  speaking: number | null;
  sentenceStart: number | undefined;
  /** The element a block is drawn in. */
  blockElement: (index: number) => HTMLElement | null;
  /** The box the column scrolls in (scroll mode). */
  scroller: () => HTMLElement | null;
  /** Turns to the page holding `element` (paged mode). */
  showPageOf: (element: HTMLElement) => void;
};

/**
 * Keeps the sentence being read aloud in view: the column glides (as `data-motion` allows) so it
 * sits a little above the middle once it leaves the middle of the screen, or the page holding it
 * is turned to. Scrolling or turning pages by hand holds that for a few seconds (`held`; Back to
 * reading goes back at once); the app's own scrolls don't.
 */
export const useFollowReading = ({
  paged,
  speaking,
  sentenceStart,
  blockElement,
  scroller,
  showPageOf,
}: Follow) => {
  const [held, setHeld] = useState(false);
  /** Asks to go back to the sentence now (Back to reading). */
  const [back, setBack] = useState(0);
  const heldUntil = useRef(0);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const glide = useRef<{ box: HTMLElement; glide: Glide } | null>(null);
  const reading = speaking !== null;
  const readingRef = useRef(reading);
  useEffect(() => {
    readingRef.current = reading;
  });

  const release = useCallback(() => {
    heldUntil.current = 0;
    clearTimeout(timer.current);
    setHeld(false);
  }, []);

  /** The reader moved the text by hand: following waits. */
  const hold = useCallback(() => {
    if (!readingRef.current) return;
    heldUntil.current = Date.now() + HOLD_MS;
    glide.current?.glide.stop();
    setHeld(true);
    clearTimeout(timer.current);
    timer.current = setTimeout(release, HOLD_MS);
  }, [release]);

  const follow = useEffectEvent((force: boolean) => {
    if (speaking === null) return;
    if (!force && Date.now() < heldUntil.current) return;
    const block = blockElement(speaking);
    if (!block) return;
    const element = spoken(block);
    if (paged) {
      showPageOf(element);
      return;
    }
    const box = scroller();
    if (!box) return;
    const view = box.getBoundingClientRect();
    const height = view.height > 0 ? view.height : window.innerHeight;
    const rect = element.getBoundingClientRect();
    const top = (rect.top - view.top) / height;
    const bottom = (rect.bottom - view.top) / height;
    if (!force && top >= BAND_TOP && bottom <= BAND_BOTTOM) return;
    // A sentence too tall for the space under the line starts higher.
    const line = bottom - top > BAND_BOTTOM - READING_LINE ? BAND_TOP : READING_LINE;
    const destination = box.scrollTop + (rect.top - view.top) - height * line;
    if (glide.current?.box !== box) glide.current = { box, glide: createGlide(box) };
    glide.current.glide.to(destination);
  });

  // Each new sentence (and a new layout) is followed.
  useEffect(() => {
    follow(false);
  }, [speaking, sentenceStart, paged]);

  useEffect(() => {
    if (back > 0) follow(true);
  }, [back]);

  // Scrolling the column by hand holds following (pages call `hold` as they turn).
  useEffect(() => {
    if (!reading || paged) return;
    const box = scroller();
    if (!box) return;
    let startY: number | null = null;
    const onWheel = () => hold();
    const onTouchStart = (event: TouchEvent) => {
      startY = event.touches[0]?.clientY ?? null;
    };
    const onTouchMove = (event: TouchEvent) => {
      const y = event.touches[0]?.clientY;
      if (startY !== null && y !== undefined && Math.abs(y - startY) > SCROLL_SLOP_PX) hold();
    };
    const onPointerDown = (event: PointerEvent) => {
      if (onScrollbar(box, event.clientX, event.clientY)) hold();
    };
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.closest('input, select, textarea, [role="dialog"]')) return;
      if (SCROLL_KEYS.has(event.key)) hold();
    };
    box.addEventListener('wheel', onWheel, { passive: true });
    box.addEventListener('touchstart', onTouchStart, { passive: true });
    box.addEventListener('touchmove', onTouchMove, { passive: true });
    box.addEventListener('pointerdown', onPointerDown);
    window.addEventListener('keydown', onKey);
    return () => {
      box.removeEventListener('wheel', onWheel);
      box.removeEventListener('touchstart', onTouchStart);
      box.removeEventListener('touchmove', onTouchMove);
      box.removeEventListener('pointerdown', onPointerDown);
      window.removeEventListener('keydown', onKey);
    };
  }, [reading, paged, hold, scroller]);

  useEffect(
    () => () => {
      clearTimeout(timer.current);
      glide.current?.glide.stop();
    },
    [],
  );

  /** Back to the sentence being read, following again. */
  const backToReading = useCallback(() => {
    release();
    setBack((count) => count + 1);
  }, [release]);

  return { held: held && reading, hold, backToReading };
};
