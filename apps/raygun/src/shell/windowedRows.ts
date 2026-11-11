import { measureElement, useVirtualizer } from '@tanstack/react-virtual';
import type { Range } from '@tanstack/react-virtual';
import { useCallback, useEffect, useLayoutEffect, useState } from 'react';
import type { RefCallback } from 'react';

import { useRootPx } from '../player/playerLayout';

/**
 * Lists longer than this draw only the rows near the screen; shorter ones draw every row, as a
 * plain list (and stay simple to read in tests).
 */
export const VIRTUAL_FROM = 60;

/** Rows drawn beyond each end of the screen, so a quick scroll doesn't show blanks. */
const OVERSCAN = 8;

/** What a row's outermost element takes, so the list can measure it. Empty for a plain list. */
export type RowProps = {
  ref?: RefCallback<HTMLElement>;
  'data-index'?: number;
};

/**
 * `memo`'s comparison for a row component taking `row`: its other props unchanged (by identity)
 * and the same place in the list.
 */
export const rowsEqual = <Props extends { row: RowProps }>(before: Props, after: Props) => {
  const keys = Object.keys(after) as (keyof Props)[];
  if (keys.length !== Object.keys(before).length) return false;
  for (const key of keys) {
    if (key !== 'row' && !Object.is(before[key], after[key])) return false;
  }
  return before.row.ref === after.row.ref && before.row['data-index'] === after.row['data-index'];
};

type WindowOptions = {
  count: number;
  /** A row's lasting name, so its measured height follows it when the list is filtered. */
  keyOf: (index: number) => string;
  /** A row's height before it's drawn, in rem; drawn rows are measured. */
  rowRem: number;
  /** The space between rows, in rem, as the list's `gap-*` puts it. */
  gapRem?: number;
  /** A row to bring onto the screen (a song the player asked to show). */
  showIndex?: number;
  /** A row kept drawn wherever the list is scrolled (one being dragged). */
  keepIndex?: number | null;
  /** Lists with more rows than this draw only those near the screen ({@link VIRTUAL_FROM}). */
  virtualFrom?: number;
  /** Rows drawn beyond each end of the screen ({@link OVERSCAN}). */
  overscan?: number;
  /**
   * The element the rows scroll in, found from the list's own: the page's `<main>` by default,
   * or a panel that scrolls by itself. Defined once (outside a component): changes aren't followed.
   */
  scrollerOf?: (list: HTMLElement) => HTMLElement | null;
  /** The scroller's height before it's measured, in rem: the window's height by default. */
  viewRem?: number;
};

/** The page's scroller: rows scroll with `<main>`, as every page does. */
const pageScroller = (list: HTMLElement) => list.closest('main');

/** Where `element` starts in `scroller`'s content, whatever it's scrolled to. */
const offsetIn = (element: HTMLElement, scroller: HTMLElement) =>
  Math.round(
    element.getBoundingClientRect().top - scroller.getBoundingClientRect().top + scroller.scrollTop,
  );

/**
 * Which rows of a long list to draw: those near the screen, with the room the others take kept
 * above (`before`) and below (`after`) them, so the page scrolls as if every row were there.
 * Rows stay in the page's flow; each drawn row passes its element to `measure` with its
 * `data-index`. Lists up to {@link VIRTUAL_FROM} rows draw every row. `start` takes the element
 * where the first row would start.
 */
export const useWindowedRows = ({
  count,
  keyOf,
  rowRem,
  gapRem = 0,
  showIndex = -1,
  keepIndex = null,
  virtualFrom = VIRTUAL_FROM,
  overscan = OVERSCAN,
  scrollerOf = pageScroller,
  viewRem,
}: WindowOptions) => {
  const windowed = count > virtualFrom;
  // Where the first row would start: a state's setter is the list's ref, so the page's scroller
  // is known from the drawing after the list's first.
  const [startElement, start] = useState<HTMLElement | null>(null);
  const rootPx = useRootPx();
  const estimate = Math.round(rowRem * rootPx);
  const scrollerFor = (list: HTMLElement | null) => (list ? scrollerOf(list) : null);
  // Where the list starts in the page, looked at again when the page's size changes (what's
  // above the list may change without the list being drawn again) and when the rows do.
  const [margin, setMargin] = useState(0);
  useLayoutEffect(() => {
    const scroller = startElement ? scrollerOf(startElement) : null;
    if (!windowed || !startElement || !scroller) return;
    const update = () => setMargin(offsetIn(startElement, scroller));
    update();
    if (typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(update);
    const page = scroller.firstElementChild;
    if (page) observer.observe(page);
    return () => observer.disconnect();
  }, [windowed, count, startElement, scrollerOf]);
  // One stretch of rows, from the screen's to the kept one: the room around them is padding.
  const rangeExtractor = useCallback(
    (range: Range) => {
      const keep = keepIndex !== null && keepIndex >= 0 && keepIndex < range.count;
      const from = Math.max(0, range.startIndex - range.overscan);
      const to = Math.min(range.count - 1, range.endIndex + range.overscan);
      const first = keep ? Math.min(from, keepIndex) : from;
      const last = keep ? Math.max(to, keepIndex) : to;
      return Array.from({ length: Math.max(0, last - first + 1) }, (_, index) => first + index);
    },
    [keepIndex],
  );

  // The app doesn't use the React Compiler, so the rule's memoization concern doesn't apply.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: windowed ? count : 0,
    getScrollElement: () => scrollerFor(startElement),
    estimateSize: () => estimate,
    getItemKey: keyOf,
    // A row not laid out (a test's page) keeps its estimate rather than counting as nothing.
    measureElement: (element, entry, instance) =>
      measureElement(element, entry, instance) || estimate,
    gap: Math.round(gapRem * rootPx),
    overscan,
    rangeExtractor,
    scrollMargin: margin,
    // The screen's height (or the scroller's, when known) until the scroller is measured, so the
    // first drawing isn't empty.
    initialRect: {
      width: 0,
      height:
        viewRem !== undefined
          ? Math.round(viewRem * rootPx)
          : typeof window === 'undefined'
            ? 0
            : window.innerHeight,
    },
  });

  useEffect(() => {
    if (windowed && showIndex >= 0 && showIndex < count) {
      virtualizer.scrollToIndex(showIndex, { align: 'center' });
    }
  }, [windowed, showIndex, count, virtualizer]);

  const items = windowed ? virtualizer.getVirtualItems() : [];
  const first = items[0];
  const last = items.at(-1);
  return {
    start,
    /** Whether only some rows are drawn: `before` and `after` keep the others' room. */
    windowed,
    before: first ? Math.max(0, first.start - margin) : 0,
    after: last ? Math.max(0, virtualizer.getTotalSize() - (last.end - margin)) : 0,
    /** Takes a drawn row's element (with its `data-index`) to measure it; none for plain lists. */
    measure: windowed ? virtualizer.measureElement : undefined,
    /**
     * Scrolls a row not drawn onto the screen, as little as it takes (a plain list draws every
     * row: focusing one scrolls to it).
     */
    scrollToRow: (index: number) => {
      if (windowed && index >= 0 && index < count) virtualizer.scrollToIndex(index);
    },
    /** The rows of `rows` to draw, each with its index in `rows`. */
    slice: <Item>(rows: readonly Item[]): [Item, number][] =>
      windowed
        ? items.flatMap((item): [Item, number][] => {
            const row = rows[item.index];
            return row === undefined ? [] : [[row, item.index]];
          })
        : rows.map((row, index): [Item, number] => [row, index]),
  };
};
