import { useEffect, useLayoutEffect, useRef, useState } from 'react';

import { clampZoom } from '../readerDisplay';
import { anchoredScroll, contentPoint } from './zoomMath';
import type { Point } from './zoomMath';

/** Where a zoom was made: the content point under it (unzoomed pixels) and where it was. */
type Anchor = { point: Point; at: Point };

/** A pinch only counts once the fingers moved this much apart or together. */
const PINCH_SLOP = 0.02;
/** Taps and swipes right after a pinch are the fingers lifting, not a page turn. */
const AFTER_PINCH_MS = 350;

const distance = (touches: TouchList) => {
  const [first, second] = [touches[0], touches[1]];
  if (!first || !second) return 0;
  return Math.hypot(first.clientX - second.clientX, first.clientY - second.clientY);
};

const middle = (touches: TouchList): Point => {
  const [first, second] = [touches[0], touches[1]];
  if (!first || !second) return { x: 0, y: 0 };
  return { x: (first.clientX + second.clientX) / 2, y: (first.clientY + second.clientY) / 2 };
};

/** The reader's own screen a zoomed page or strip scrolls in. */
const scrollerOf = (content: HTMLElement) =>
  content.closest<HTMLElement>('[data-reader-scroll]') ?? content.closest<HTMLElement>('main');

/** The middle of the reader's screen (of the window, before it's laid out). */
const screenMiddle = (content: HTMLElement | null): Point => {
  const box = content ? scrollerOf(content)?.getBoundingClientRect() : undefined;
  if (box && box.width > 0 && box.height > 0) {
    return { x: box.left + box.width / 2, y: box.top + box.height / 2 };
  }
  return { x: window.innerWidth / 2, y: window.innerHeight / 2 };
};

/**
 * The reader's zoom: buttons, keys, Ctrl+wheel, double taps and two-finger pinches. Only the page
 * (or the strip) is zoomed, inside the reader's own fixed screen, which scrolls it. A zoom keeps
 * the point it was made at under the finger (or the pointer, or the screen's middle), as far as
 * the page's edges allow (`zoomMath`). A pinch is drawn with a transform while the fingers move,
 * and laid out once they lift, so the page reflows once rather than on every frame.
 */
export const useReaderZoom = () => {
  const [zoom, setZoomState] = useState(1);
  /** The zoomed element: the strip's column or the page's image. */
  const contentRef = useRef<HTMLElement | null>(null);
  const anchor = useRef<Anchor | null>(null);
  const zoomRef = useRef(zoom);
  const pinchedAt = useRef(0);
  /** A pinch under way: the fingers' first distance and middle, and the scale reached. */
  const pinch = useRef<{ start: number; center: Point; scale: number } | null>(null);

  useEffect(() => {
    zoomRef.current = zoom;
  }, [zoom]);

  const zoomAt = (next: number, point?: Point) => {
    const from = zoomRef.current;
    const to = clampZoom(next);
    if (to === from) return;
    const content = contentRef.current;
    const at = point ?? screenMiddle(content);
    if (content) {
      const box = content.getBoundingClientRect();
      anchor.current = { point: contentPoint({ x: box.left, y: box.top }, at, from), at };
    }
    zoomRef.current = to;
    setZoomState(to);
  };

  // Once laid out at the new zoom, the anchored point goes back under the finger.
  useLayoutEffect(() => {
    const pending = anchor.current;
    anchor.current = null;
    const content = contentRef.current;
    if (!pending || !content) return;
    const scroller = scrollerOf(content);
    if (!scroller) return;
    const box = content.getBoundingClientRect();
    const next = anchoredScroll({
      scroll: { x: scroller.scrollLeft, y: scroller.scrollTop },
      box: { x: box.left, y: box.top },
      point: pending.point,
      zoom,
      at: pending.at,
      max: {
        x: scroller.scrollWidth - scroller.clientWidth,
        y: scroller.scrollHeight - scroller.clientHeight,
      },
    });
    scroller.scrollLeft = next.x;
    scroller.scrollTop = next.y;
  }, [zoom]);

  /** Scales around where the pinch began (set once, before any transform moves the box). */
  const startPreview = (center: Point) => {
    const content = contentRef.current;
    if (!content) return;
    const box = content.getBoundingClientRect();
    content.style.transformOrigin = `${center.x - box.left}px ${center.y - box.top}px`;
  };
  const preview = (scale: number) => {
    const content = contentRef.current;
    if (content) content.style.transform = `scale(${scale})`;
  };
  const clearPreview = () => {
    const content = contentRef.current;
    if (!content) return;
    content.style.transform = '';
    content.style.transformOrigin = '';
  };

  /** Two-finger pinches on the reader (`touchmove` is listened to without `passive`). */
  const bindPinch = (element: HTMLElement | null) => {
    if (!element) return () => undefined;
    const onStart = (event: TouchEvent) => {
      if (event.touches.length !== 2) return;
      const center = middle(event.touches);
      pinch.current = { start: distance(event.touches), center, scale: 1 };
      startPreview(center);
    };
    const onMove = (event: TouchEvent) => {
      const gesture = pinch.current;
      if (!gesture || event.touches.length !== 2 || gesture.start === 0) return;
      // The browser would scroll the strip with the two fingers.
      event.preventDefault();
      const raw = distance(event.touches) / gesture.start;
      if (Math.abs(raw - 1) < PINCH_SLOP && gesture.scale === 1) return;
      const from = zoomRef.current;
      gesture.scale = clampZoom(from * raw) / from;
      pinchedAt.current = Date.now();
      preview(gesture.scale);
    };
    const onEnd = (event: TouchEvent) => {
      const gesture = pinch.current;
      if (!gesture || event.touches.length >= 2) return;
      pinch.current = null;
      clearPreview();
      if (gesture.scale !== 1) {
        pinchedAt.current = Date.now();
        // Laid out at the new zoom with the pinch's first middle where it was: as previewed.
        zoomAt(zoomRef.current * gesture.scale, gesture.center);
      }
    };
    element.addEventListener('touchstart', onStart, { passive: true });
    element.addEventListener('touchmove', onMove, { passive: false });
    element.addEventListener('touchend', onEnd);
    element.addEventListener('touchcancel', onEnd);
    return () => {
      element.removeEventListener('touchstart', onStart);
      element.removeEventListener('touchmove', onMove);
      element.removeEventListener('touchend', onEnd);
      element.removeEventListener('touchcancel', onEnd);
    };
  };

  /** Whether fingers are pinching, or just stopped. */
  const pinching = () => pinch.current !== null || Date.now() - pinchedAt.current < AFTER_PINCH_MS;

  return { zoom, zoomAt, contentRef, bindPinch, pinching };
};
