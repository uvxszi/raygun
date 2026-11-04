import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { FC, MouseEvent as ReactMouseEvent, ReactNode, RefObject } from 'react';

import { stripWidth } from '../readerDisplay';

import { ReaderImage } from './ReaderImage';

/** Pages kept loaded above the current one, so scrolling back a little shows them at once. */
const BEHIND = 2;
/** Pages loaded beyond the ones the preload setting asks for. */
const AHEAD = 1;
/** A page's width over its height before any page of the chapter has loaded: a printed page. */
const FIRST_GUESS = 0.7;
/** The current page is the one crossing a line a third of the way down the screen. */
const READING_LINE = '-33% 0px -66% 0px';
/** A scroll this far with the controls up puts them away (Mihon's "hide on scroll"). */
const HIDE_AFTER_PX = 160;

/** The ratio most loaded pages have; unloaded pages take their place at it. */
const typicalRatio = (ratios: Record<number, number>) => {
  const known = Object.values(ratios).sort((a, b) => a - b);
  return known.length === 0 ? FIRST_GUESS : (known[Math.floor(known.length / 2)] ?? FIRST_GUESS);
};

type SlotProps = {
  url: string;
  index: number;
  /** Only pages near the one being read hold an image; the rest keep their place. */
  loaded: boolean;
  ratio: number;
  register: (index: number, element: HTMLDivElement | null) => void;
  onRatio: (index: number, ratio: number) => void;
  onMenu: (index: number) => void;
};

const SlotView: FC<SlotProps> = ({ url, index, loaded, ratio, register, onRatio, onMenu }) => (
  <div
    ref={(element) => register(index, element)}
    data-index={index}
    data-testid="webtoon-page"
    className="relative w-full bg-neutral-900"
    style={{ aspectRatio: ratio }}
  >
    {loaded && (
      <ReaderImage
        url={url}
        index={index}
        onRatio={onRatio}
        onMenu={onMenu}
        className="block h-auto w-full"
      />
    )}
  </div>
);
const Slot = memo(SlotView);

export type WebtoonViewProps = {
  pages: string[];
  current: number;
  /** A page to scroll to, again whenever `nonce` changes. */
  jump: { page: number; nonce: number } | null;
  zoom: number;
  /** The column's size: percent more or less than the screen's own (M46.48). */
  size: number;
  preload: number;
  chrome: boolean;
  contentRef: RefObject<HTMLElement | null>;
  onCurrent: (page: number) => void;
  onTap: (event: ReactMouseEvent<HTMLElement>) => void;
  onHideChrome: () => void;
  onPageMenu: (page: number) => void;
  endCard: ReactNode;
};

/**
 * The vertical strip: every page keeps its place (at its own shape once known), but only the pages
 * around the one being read hold images, so a chapter of hundreds of pages scrolls in bounded
 * memory. The browser's scroll anchoring keeps the view still as pages above load.
 */
export const WebtoonView: FC<WebtoonViewProps> = ({
  pages,
  current,
  jump,
  zoom,
  size,
  preload,
  chrome,
  contentRef,
  onCurrent,
  onTap,
  onHideChrome,
  onPageMenu,
  endCard,
}) => {
  const slots = useRef<(HTMLDivElement | null)[]>([]);
  const endRef = useRef<HTMLDivElement>(null);
  const jumpedAt = useRef(0);
  const [ratios, setRatios] = useState<Record<number, number>>({});
  const total = pages.length;
  const guess = useMemo(() => typicalRatio(ratios), [ratios]);
  const first = Math.max(0, current - BEHIND);
  const last = Math.min(total - 1, current + preload + AHEAD);

  const register = useCallback((index: number, element: HTMLDivElement | null) => {
    slots.current[index] = element;
  }, []);
  const onRatio = useCallback(
    (index: number, ratio: number) =>
      setRatios((known) => (known[index] === ratio ? known : { ...known, [index]: ratio })),
    [],
  );

  // The page crossing the reading line is the current one; the end card counts as the last page.
  useEffect(() => {
    if (typeof IntersectionObserver === 'undefined') return;
    const crossing = new Set<number>();
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          const index = Number((entry.target as HTMLElement).dataset.index);
          if (Number.isNaN(index)) continue;
          if (entry.isIntersecting) crossing.add(index);
          else crossing.delete(index);
        }
        if (crossing.size > 0) onCurrent(Math.min(...crossing));
      },
      { rootMargin: READING_LINE },
    );
    for (const slot of slots.current.slice(0, total)) {
      if (slot) observer.observe(slot);
    }
    if (endRef.current) observer.observe(endRef.current);
    return () => observer.disconnect();
  }, [pages, total, onCurrent]);

  useEffect(() => {
    if (!jump) return;
    jumpedAt.current = Date.now();
    slots.current[jump.page]?.scrollIntoView?.({ block: 'start' });
  }, [jump]);

  // Scrolling the strip by hand puts the controls away.
  useEffect(() => {
    const scroller = endRef.current?.closest('main');
    if (!chrome || !scroller) return;
    let start = scroller.scrollTop;
    const onScroll = () => {
      // A jump (the slider, resuming) isn't the reader scrolling.
      if (Date.now() - jumpedAt.current < 500) {
        start = scroller.scrollTop;
        return;
      }
      if (Math.abs(scroller.scrollTop - start) > HIDE_AFTER_PX) onHideChrome();
    };
    scroller.addEventListener('scroll', onScroll, { passive: true });
    return () => scroller.removeEventListener('scroll', onScroll);
  }, [chrome, onHideChrome]);

  // Keys scroll the strip: arrows a little, Space and Page Down a screen.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.closest('input, select, textarea, [role="dialog"]')) return;
      if (event.ctrlKey || event.metaKey || event.altKey) return;
      const scroller = endRef.current?.closest('main');
      if (!scroller) return;
      const screen = scroller.clientHeight * 0.85;
      const steps: Record<string, number> = {
        ArrowDown: 80,
        ArrowUp: -80,
        PageDown: screen,
        PageUp: -screen,
        ' ': event.shiftKey ? -screen : screen,
      };
      if (event.key === 'Home') scroller.scrollTo?.({ top: 0 });
      else if (event.key === 'End') scroller.scrollTo?.({ top: scroller.scrollHeight });
      else if (event.key in steps) scroller.scrollBy?.({ top: steps[event.key] });
      else return;
      event.preventDefault();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  return (
    <div
      role="presentation"
      onClick={onTap}
      className="flex min-h-full flex-col select-none [-webkit-touch-callout:none]"
    >
      <div
        ref={(element) => {
          contentRef.current = element;
        }}
        data-testid="webtoon-strip"
        // The column: the screen's width up to 768 px (M46.7), made bigger or smaller by the
        // webtoon size (M46.48), then zoomed.
        className="mx-auto flex max-w-none shrink-0 flex-col"
        style={{ width: stripWidth(size, zoom) }}
      >
        {pages.map((url, index) => (
          <Slot
            // By place: a page list asked for again may carry new addresses for the same pages.
            key={index}
            url={url}
            index={index}
            loaded={index >= first && index <= last}
            ratio={ratios[index] ?? guess}
            register={register}
            onRatio={onRatio}
            onMenu={onPageMenu}
          />
        ))}
      </div>
      <div
        ref={endRef}
        data-index={Math.max(total - 1, 0)}
        className="flex min-h-[70dvh] flex-col justify-center"
      >
        {endCard}
      </div>
    </div>
  );
};
