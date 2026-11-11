/**
 * Scrollbars shown while scrolling (Settings → Appearance → Scrollbars): an element scrolled by
 * hand, or whose bar the mouse is on, gets `data-scrollbar="shown"`; a second after the last
 * scroll (or once the mouse leaves the bar) it's `fading`, then nothing (styles.css draws each).
 *
 * Only scrolls that people make show it: the wheel, a finger, the keys, the bar dragged. The
 * app's own (the lyrics gliding to their line, a revealed row brought into view, a list scrolling
 * under something dragged) come without any of those and show nothing. A scroll counts when it
 * follows such input closely on an element that holds where the input went, or carries on a
 * scroll by hand (smooth scrolling, a fling after the finger left).
 *
 * Cheap on purpose: a few passive listeners on the window, installed only while the option is on,
 * and an attribute set on the element; no React, nothing per frame.
 */

/** How long a bar stays after the last scroll by hand, ms. */
export const SHOWN_MS = 1000;
/** How long it takes to fade out (styles.css), ms. */
export const FADE_MS = 400;
/** A scroll this soon after the wheel, a finger or a key is theirs, ms. */
const INPUT_MS = 150;
/** A scroll this soon after an element's last scroll by hand carries it on, ms. */
const CARRY_MS = 150;
/** Keys that scroll what has the focus (or the page). */
const SCROLL_KEYS = new Set([
  'ArrowUp',
  'ArrowDown',
  'ArrowLeft',
  'ArrowRight',
  'PageUp',
  'PageDown',
  'Home',
  'End',
  ' ',
]);

const ATTRIBUTE = 'data-scrollbar';

type Input = { target: EventTarget | null; at: number; keys: boolean };

/**
 * Whether a finger put down on `target` scrolls what's under it: not on something that takes the
 * finger's moves itself (`touch-action: none`, as drag handles have), where only the app moves
 * things, a list scrolling under a dragged row included.
 */
export const fingerScrolls = (target: EventTarget | null) => {
  for (
    let element = target instanceof Element ? target : null;
    element;
    element = element.parentElement
  ) {
    if (getComputedStyle(element).touchAction === 'none') return false;
  }
  return true;
};

/** Whether the pointer is on `element`'s own scrollbar (`clientX`/`clientY` on the page). */
export const onScrollbar = (element: Element, clientX: number, clientY: number) => {
  const { clientWidth, clientHeight, scrollWidth, scrollHeight } = element;
  const vertical = scrollHeight > clientHeight;
  const horizontal = scrollWidth > clientWidth;
  if (!vertical && !horizontal) return false;
  const box = element.getBoundingClientRect();
  // From the start of the padding box: a bar on the right (or left, right to left) is past its
  // width, one at the bottom past its height.
  const x = clientX - box.left - element.clientLeft;
  const y = clientY - box.top - element.clientTop;
  return (vertical && (x >= clientWidth || x < 0)) || (horizontal && y >= clientHeight);
};

/** Starts showing bars as they're scrolled; returns what stops it. */
export const startScrollbarReveal = (view: Window = window) => {
  const page = view.document;
  const timers = new Map<Element, ReturnType<typeof setTimeout>>();
  const lastByHand = new WeakMap<Element, number>();
  let input: Input | null = null;
  /** The element whose bar is being dragged (a button held down on it). */
  let dragging: Element | null = null;
  /** The element whose bar the mouse is on. */
  let pointed: Element | null = null;

  const clear = (element: Element) => {
    const timer = timers.get(element);
    if (timer !== undefined) clearTimeout(timer);
    timers.delete(element);
  };

  const fade = (element: Element) => {
    clear(element);
    if (element === dragging || element === pointed) return;
    element.setAttribute(ATTRIBUTE, 'fading');
    timers.set(
      element,
      setTimeout(() => {
        timers.delete(element);
        element.removeAttribute(ATTRIBUTE);
      }, FADE_MS),
    );
  };

  /** Shows `element`'s bar; it fades a moment later unless it's held (dragged, pointed at). */
  const show = (element: Element) => {
    clear(element);
    if (element.getAttribute(ATTRIBUTE) !== 'shown') element.setAttribute(ATTRIBUTE, 'shown');
    if (element !== dragging && element !== pointed) {
      timers.set(
        element,
        setTimeout(() => fade(element), SHOWN_MS),
      );
    }
  };

  const byHand = (element: Element, now: number) => {
    if (element === dragging) return true;
    if (now - (lastByHand.get(element) ?? Number.NEGATIVE_INFINITY) < CARRY_MS) return true;
    if (!input || now - input.at >= INPUT_MS) return false;
    if (input.keys) {
      // Keys scroll what holds the focus, or the page.
      const focused = page.activeElement;
      return !focused || focused === page.body || element.contains(focused);
    }
    return input.target instanceof Node && element.contains(input.target);
  };

  const onScroll = (event: Event) => {
    const target = event.target;
    const element =
      target instanceof Element ? target : (page.scrollingElement ?? page.documentElement);
    const now = performance.now();
    if (!byHand(element, now)) return;
    lastByHand.set(element, now);
    show(element);
  };

  const onWheel = (event: WheelEvent) => {
    input = { target: event.target, at: performance.now(), keys: false };
  };

  /** Where the finger went down, and whether it scrolls there ({@link fingerScrolls}, once a touch). */
  let finger: { target: EventTarget | null; scrolls: boolean } | null = null;

  // After the page's own handlers: a finger that drags something (its moves prevented, or put down
  // on a drag handle) scrolls nothing by hand, whatever scrolls under it.
  const onTouchMove = (event: TouchEvent) => {
    if (event.defaultPrevented) return;
    if (finger?.target !== event.target) {
      finger = { target: event.target, scrolls: fingerScrolls(event.target) };
    }
    if (finger.scrolls) input = { target: event.target, at: performance.now(), keys: false };
  };

  // After the page's own handlers too: keys something takes for itself (a dragged row moved with
  // the arrows, a menu) scroll nothing by hand.
  const onKey = (event: KeyboardEvent) => {
    if (SCROLL_KEYS.has(event.key) && !event.defaultPrevented) {
      input = { target: null, at: performance.now(), keys: true };
    }
  };

  const onPointerDown = (event: PointerEvent) => {
    const target = event.target;
    if (target instanceof Element && onScrollbar(target, event.clientX, event.clientY)) {
      dragging = target;
      show(target);
    }
  };

  const onPointerUp = () => {
    const released = dragging;
    dragging = null;
    if (released) show(released);
  };

  const point = (element: Element | null) => {
    if (element === pointed) return;
    const left = pointed;
    pointed = element;
    if (left) show(left);
    if (element) show(element);
  };

  const onPointerMove = (event: PointerEvent) => {
    if (event.pointerType !== 'mouse') return;
    const target = event.target;
    point(
      target instanceof Element && onScrollbar(target, event.clientX, event.clientY)
        ? target
        : null,
    );
  };

  // The mouse left the window from a bar.
  const onPointerOut = (event: PointerEvent) => {
    if (event.relatedTarget === null) point(null);
  };

  const passive = { passive: true } as const;
  const capture = { passive: true, capture: true } as const;
  page.addEventListener('scroll', onScroll, capture);
  view.addEventListener('wheel', onWheel, capture);
  view.addEventListener('touchmove', onTouchMove, passive);
  view.addEventListener('keydown', onKey, passive);
  view.addEventListener('pointerdown', onPointerDown, capture);
  view.addEventListener('pointerup', onPointerUp, capture);
  view.addEventListener('pointercancel', onPointerUp, capture);
  view.addEventListener('pointermove', onPointerMove, passive);
  view.addEventListener('pointerout', onPointerOut, passive);

  return () => {
    // Listeners are told apart by whether they capture.
    page.removeEventListener('scroll', onScroll, true);
    view.removeEventListener('wheel', onWheel, true);
    view.removeEventListener('touchmove', onTouchMove);
    view.removeEventListener('keydown', onKey);
    view.removeEventListener('pointerdown', onPointerDown, true);
    view.removeEventListener('pointerup', onPointerUp, true);
    view.removeEventListener('pointercancel', onPointerUp, true);
    view.removeEventListener('pointermove', onPointerMove);
    view.removeEventListener('pointerout', onPointerOut);
    for (const [element, timer] of timers) {
      clearTimeout(timer);
      element.removeAttribute(ATTRIBUTE);
    }
    timers.clear();
    for (const element of page.querySelectorAll(`[${ATTRIBUTE}]`)) {
      element.removeAttribute(ATTRIBUTE);
    }
  };
};
