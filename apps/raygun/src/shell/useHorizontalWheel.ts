import { useEffect } from 'react';

/** The nearest element under `start` that scrolls sideways and still can in `direction`. */
const sidewaysScroller = (start: Element | null, direction: number) => {
  for (let node = start; node && node !== document.body; node = node.parentElement) {
    if (!(node instanceof HTMLElement) || node.scrollWidth <= node.clientWidth + 1) continue;
    const { overflowX } = getComputedStyle(node);
    if (overflowX !== 'auto' && overflowX !== 'scroll') continue;
    const room =
      direction > 0
        ? node.scrollLeft < node.scrollWidth - node.clientWidth - 1
        : node.scrollLeft > 0;
    // A row at its end lets the page scroll on.
    return room ? node : null;
  }
  return null;
};

/**
 * Shift with the mouse wheel scrolls sideways rows (posters, chips, covers) when the pointer is
 * over them (M46.8; Shift since M46.14, when rows got their page buttons); at a row's end the
 * page scrolls instead. The wheel alone scrolls the page; Ctrl and sideways wheels keep their own
 * meaning.
 */
export const useHorizontalWheel = () => {
  useEffect(() => {
    const onWheel = (event: WheelEvent) => {
      // Rows page with their buttons (M46.14); the wheel scrolls them only with Shift held, and
      // scrolls the page otherwise. A trackpad's own sideways swipe is left alone.
      if (event.ctrlKey || !event.shiftKey || Math.abs(event.deltaX) > Math.abs(event.deltaY)) {
        return;
      }
      const row = sidewaysScroller(event.target as Element | null, Math.sign(event.deltaY));
      if (!row) return;
      // Lines (Firefox) and pages count as about a card's width.
      const step =
        event.deltaMode === WheelEvent.DOM_DELTA_LINE
          ? event.deltaY * 40
          : event.deltaMode === WheelEvent.DOM_DELTA_PAGE
            ? event.deltaY * row.clientWidth
            : event.deltaY;
      row.scrollBy({ left: step, behavior: 'auto' });
      event.preventDefault();
    };
    window.addEventListener('wheel', onWheel, { passive: false });
    return () => window.removeEventListener('wheel', onWheel);
  }, []);
};
