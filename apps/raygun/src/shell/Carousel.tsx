import { ChevronLeft, ChevronRight } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

/** A page is most of the row, so the last card of one page shows again at the start of the next. */
const PAGE = 0.85;

type CarouselProps = {
  children: ReactNode;
  /** The list's own classes: its gap and padding. */
  className?: string;
  testId?: string;
};

/**
 * A row of cards that pages sideways (M46.14), as Spotify's rows do: buttons at its ends glide it a
 * page at a time and show on hover, Shift with the wheel scrolls it, a finger swipes it, and no
 * scrollbar shows under it.
 */
export const Carousel: FC<CarouselProps> = ({ children, className = '', testId }) => {
  const { t } = useTranslation();
  const list = useRef<HTMLUListElement>(null);
  const [edges, setEdges] = useState({ start: true, end: true });

  const update = useCallback(() => {
    const row = list.current;
    if (!row) return;
    const start = row.scrollLeft <= 2;
    const end = row.scrollLeft + row.clientWidth >= row.scrollWidth - 2;
    setEdges((previous) =>
      previous.start === start && previous.end === end ? previous : { start, end },
    );
  }, []);

  useEffect(() => {
    update();
    const row = list.current;
    if (!row || typeof ResizeObserver === 'undefined') return;
    // The row's width changes with the window, its content as cards load.
    const observer = new ResizeObserver(update);
    observer.observe(row);
    for (const child of Array.from(row.children)) observer.observe(child);
    return () => observer.disconnect();
  }, [update, children]);

  const page = (direction: 1 | -1) => {
    const row = list.current;
    row?.scrollBy({ left: direction * row.clientWidth * PAGE, behavior: 'smooth' });
  };

  const button = (direction: 1 | -1) => (
    <button
      type="button"
      aria-label={direction < 0 ? t('carousel.back') : t('carousel.forward')}
      onClick={() => page(direction)}
      className={`bg-surface/95 text-foreground absolute top-[38%] z-10 grid size-9 -translate-y-1/2 place-items-center rounded-full shadow-lg shadow-black/30 transition-opacity group-hover/carousel:opacity-100 hover:scale-105 focus-visible:opacity-100 [@media(hover:hover)]:opacity-0 [@media(hover:none)]:hidden ${
        direction < 0 ? 'left-1' : 'right-1'
      }`}
    >
      {direction < 0 ? (
        <ChevronLeft aria-hidden size={20} />
      ) : (
        <ChevronRight aria-hidden size={20} />
      )}
    </button>
  );

  return (
    <div className="group/carousel relative">
      <ul
        ref={list}
        data-testid={testId}
        data-carousel=""
        onScroll={update}
        className={`no-scrollbar flex overflow-x-auto overscroll-x-contain ${className}`}
      >
        {children}
      </ul>
      {!edges.start && button(-1)}
      {!edges.end && button(1)}
    </div>
  );
};
