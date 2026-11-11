import type { SafeInsets } from './safeArea';

/** What a press or a pointer can mean: buttons, links and the like. */
const CONTROLS = [
  'button',
  'a',
  '[role="button"]',
  '[role="link"]',
  '[role="switch"]',
  '[role="tab"]',
  '[role="checkbox"]',
  '[role="radio"]',
  '[role="menuitem"]',
  '[role="menuitemcheckbox"]',
  '[role="menuitemradio"]',
  '[role="option"]',
].join(', ');

/** Where a title is kept while the label shows instead of the browser's own tip. */
export const KEPT_TITLE = 'data-label-title';

/** The control a press or a pointer on `target` is for. */
export const controlOf = (target: EventTarget | null): HTMLElement | null =>
  target instanceof Element ? target.closest<HTMLElement>(CONTROLS) : null;

const attribute = (control: Element, name: string) => (control.getAttribute(name) ?? '').trim();

/**
 * What the control's label says: for the mouse its title (the tip its page gave it, often more
 * than its name), else its name; for a finger its name (`aria-label`), else its title.
 */
export const nameOf = (control: Element, by: 'pointer' | 'touch') => {
  const title = attribute(control, 'title') || attribute(control, KEPT_TITLE);
  const name = attribute(control, 'aria-label');
  return by === 'pointer' ? title || name : name || title;
};

/** The words the control shows, without text kept for screen readers only. */
export const shownText = (control: Element) => {
  let text = '';
  const walk = (node: Node) => {
    for (const child of node.childNodes) {
      if (child.nodeType === Node.TEXT_NODE) text += child.textContent ?? '';
      else if (child instanceof Element && !child.classList.contains('sr-only')) {
        text += ' ';
        walk(child);
      }
    }
  };
  walk(control);
  return text.replace(/\s+/g, ' ').trim();
};

/** A word of three letters or more: something a control spells out. */
const WORD = /\p{L}{3,}/u;

/**
 * The name a hover (`pointer`) or a held finger (`touch`) on this control shows, or null when it
 * has none or shows it already. A finger's hold also leaves out controls that spell out what they
 * do in words (a song's row, a card), which open as before: only icons, numbers and marks are
 * named.
 */
export const labelFor = (control: Element, by: 'pointer' | 'touch'): string | null => {
  const name = nameOf(control, by);
  if (!name) return null;
  const text = shownText(control);
  if (!text) return name;
  if (text.toLocaleLowerCase().includes(name.toLocaleLowerCase())) return null;
  if (by === 'touch' && WORD.test(text)) return null;
  return name;
};

/** The space between a control and its label, and from the label to the screen's edges, px. */
const GAP = 8;
const EDGE = 8;
/** About how tall a label is (one line), px. */
export const LABEL_HEIGHT = 30;
/** Under a finger: a label below the control starts this far (px) under where it pressed. */
const FINGER = 28;

type Box = { left: number; top: number; right: number; bottom: number };
type Screen = { width: number; height: number; safe: SafeInsets };

/**
 * Where the label shows: a bubble beside the control, else (no room) a toast at the bottom. A
 * finger covers what's under it, so a held control's name goes above it, else below the finger;
 * a pointer's goes below the control, else above it. `x` is the bubble's centre (kept on screen
 * once its width is known), `y` its top or bottom edge.
 */
export type LabelPlace =
  { kind: 'bubble'; x: number; y: number; above: boolean } | { kind: 'toast' };

export const placeLabel = (
  box: Box,
  screen: Screen,
  by: 'pointer' | 'touch',
  pressY?: number,
): LabelPlace => {
  const top = screen.safe.top + EDGE;
  const bottom = screen.height - screen.safe.bottom - EDGE;
  const aboveAt = box.top - GAP;
  const belowAt =
    by === 'touch' ? Math.max(box.bottom, (pressY ?? box.bottom) + FINGER) + GAP : box.bottom + GAP;
  const fitsAbove = aboveAt - LABEL_HEIGHT >= top;
  const fitsBelow = belowAt + LABEL_HEIGHT <= bottom;
  const above = by === 'touch' ? fitsAbove : !fitsBelow && fitsAbove;
  if (!above && !fitsBelow) return { kind: 'toast' };
  const left = screen.safe.left + EDGE;
  const right = screen.width - screen.safe.right - EDGE;
  const centre = (box.left + box.right) / 2;
  return {
    kind: 'bubble',
    x: Math.min(Math.max(centre, left), right),
    y: above ? aboveAt : belowAt,
    above,
  };
};

/** How far (px) a bubble whose box is `left`…`right` moves sideways to stay on screen. */
export const keepOnScreen = (left: number, right: number, screen: Screen) => {
  const min = screen.safe.left + EDGE;
  const max = screen.width - screen.safe.right - EDGE;
  if (right - left > max - min) return min - left;
  if (left < min) return min - left;
  if (right > max) return max - right;
  return 0;
};
