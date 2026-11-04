import { useEffect, useLayoutEffect, useRef, useState } from 'react';

import { clampZoom } from '../readerDisplay';

/** A point on screen, in client pixels. */
export type ScreenPoint = { x: number; y: number };

/** Where a zoom was centred: the content point under it, in unzoomed pixels. */
type Anchor = { contentX: number; contentY: number; clientX: number; clientY: number };

/** A pinch only counts once the fingers moved this much apart or together. */
const PINCH_SLOP = 0.02;
/** Taps and swipes right after a pinch are the fingers lifting, not a page turn. */
const AFTER_PINCH_MS = 350;

const distance = (touches: TouchList) => {
  const [first, second] = [touches[0], touches[1]];
  if (!first || !second) return 0;
  return Math.hypot(first.clientX - second.clientX, first.clientY - second.clientY);
};

const middle = (touches: TouchList): ScreenPoint => {
  const [first, second] = [touches[0], touches[1]];
  if (!first || !second) return { x: 0, y: 0 };
  return { x: (first.clientX + second.clientX) / 2, y: (first.clientY + second.clientY) / 2 };
};

/** The element a zoomed page or strip scrolls in. */
const scrollerOf = (content: HTMLElement) =>
  content.closest<HTMLElement>('[data-reader-scroll]') ?? content.closest<HTMLElement>('main');

/**
 * The reader's zoom: buttons, keys, Ctrl+wheel, double taps and two-finger pinches. A zoom keeps
 * the point it was made at under the finger (or the pointer, or the screen's middle). A pinch is
 * drawn with a transform while the fingers move, and laid out once they lift, so the page reflows
 * once rather than on every frame.
 */
export const useReaderZoom = () => {
  const [zoom, setZoomState] = useState(1);
  /** The zoomed element: the strip's column or the page's image. */
  const contentRef = useRef<HTMLElement | null>(null);
  const anchor = useRef<Anchor | null>(null);
  const zoomRef = useRef(zoom);
  const pinchedAt = useRef(0);
  /** A pinch under way: the fingers' first distance and middle, and the scale reached. */
  const pinch = useRef<{ start: number; center: ScreenPoint; scale: number } | null>(null);

  useEffect(() => {
    zoomRef.current = zoom;
  }, [zoom]);

  const zoomAt = (next: number, point?: ScreenPoint) => {
    const from = zoomRef.current;
    const to = clampZoom(next);
    if (to === from) return;
    const content = contentRef.current;
    const at = point ?? { x: window.innerWidth / 2, y: window.innerHeight / 2 };
    if (content) {
      const box = content.getBoundingClientRect();
      anchor.current = {
        contentX: (at.x - box.left) / from,
        contentY: (at.y - box.top) / from,
        clientX: at.x,
        clientY: at.y,
      };
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
    scroller.scrollLeft += box.left + pending.contentX * zoom - pending.clientX;
    scroller.scrollTop += box.top + pending.contentY * zoom - pending.clientY;
  }, [zoom]);

  /** Scales around where the pinch began (set once, before any transform moves the box). */
  const startPreview = (center: ScreenPoint) => {
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
