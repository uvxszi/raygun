import type { PagedScale } from '@raygun/bindings';
import { useEffect, useRef } from 'react';
import type {
  CSSProperties,
  FC,
  MouseEvent as ReactMouseEvent,
  PointerEvent as ReactPointerEvent,
  ReactNode,
  RefObject,
} from 'react';
import { useTranslation } from 'react-i18next';

import { pageIdentity } from './drawnPages';
import { ReaderImage } from './ReaderImage';

/** Horizontal movement that counts as a swipe. */
const SWIPE_PX = 50;

/** How a page sits in the paged modes: the chosen fit, made `zoom` times bigger (M46.7). */
const pageLayout = (
  scale: PagedScale,
  zoom: number,
): { className: string; style: CSSProperties } => {
  switch (scale) {
    case 'fitWidth':
      return { className: 'h-auto max-w-none', style: { width: `${Math.round(100 * zoom)}%` } };
    case 'fitHeight':
      return {
        className: 'w-auto max-w-none',
        style: { height: `calc(${Math.round(100 * zoom)} * (100dvh - var(--app-bar, 0px)) / 100)` },
      };
    case 'original':
      return { className: 'max-w-none', style: { zoom } };
    default:
      return zoom === 1
        ? { className: 'max-h-full max-w-full object-contain', style: {} }
        : {
            className: 'max-w-none object-contain',
            style: {
              width: `${Math.round(100 * zoom)}%`,
              height: `calc(${Math.round(100 * zoom)} * (100dvh - var(--app-bar, 0px)) / 100)`,
            },
          };
  }
};

/** Pages loaded and decoded ahead (and the one before), kept while they may be turned to. */
const usePreload = (pages: string[], current: number, ahead: number) => {
  useEffect(() => {
    const kept: HTMLImageElement[] = [];
    const wanted = [...pages.slice(current + 1, current + 1 + ahead), pages[current - 1]];
    for (const url of wanted) {
      if (!url) continue;
      const image = new Image();
      image.decoding = 'async';
      image.src = url;
      // Decoded now, off the main thread, so turning to it shows it at once.
      const decoded = image.decode?.();
      void decoded?.catch(() => undefined);
      kept.push(image);
    }
    return () => {
      kept.length = 0;
    };
  }, [pages, current, ahead]);
};

export type PagedViewProps = {
  pages: string[];
  current: number;
  rtl: boolean;
  zoom: number;
  scale: PagedScale;
  preload: number;
  contentRef: RefObject<HTMLElement | null>;
  /** Index `pages.length` is the end card after the last page. */
  onCurrent: (page: number) => void;
  onTap: (event: ReactMouseEvent<HTMLElement>) => void;
  onPageMenu: (page: number) => void;
  /** Fingers are pinching (or just were): their lifting isn't a swipe. */
  pinching: () => boolean;
  endCard: ReactNode;
};

/** One page at a time with tap zones, swipes, keys and pinch zoom (double taps come as taps). */
export const PagedView: FC<PagedViewProps> = ({
  pages,
  current,
  rtl,
  zoom,
  scale,
  preload,
  contentRef,
  onCurrent,
  onTap,
  onPageMenu,
  pinching,
  endCard,
}) => {
  const { t } = useTranslation();
  const total = pages.length;
  const zoomed = zoom > 1;
  const fitted = zoom === 1 && scale === 'fitScreen';
  const box = useRef<HTMLDivElement>(null);
  const swipe = useRef<{ x: number; pointers: number } | null>(null);
  const swiped = useRef(false);
  usePreload(pages, current, preload);

  const step = (delta: number) => onCurrent(Math.min(Math.max(current + delta, 0), total));
  const forward = () => step(1);
  const back = () => step(-1);

  // A new page starts at its top left.
  useEffect(() => {
    box.current?.scrollTo?.(0, 0);
  }, [current]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      // The page slider and dialogs handle their own keys.
      if (target?.closest('input, select, textarea, [role="dialog"]')) return;
      if (event.ctrlKey || event.metaKey || event.altKey) return;
      if (event.key === 'ArrowRight') (rtl ? back : forward)();
      else if (event.key === 'ArrowLeft') (rtl ? forward : back)();
      else if (event.key === 'PageDown' || (event.key === ' ' && !event.shiftKey)) forward();
      else if (event.key === 'PageUp' || (event.key === ' ' && event.shiftKey)) back();
      else if (event.key === 'Home') onCurrent(0);
      else if (event.key === 'End') onCurrent(Math.max(total - 1, 0));
      else return;
      event.preventDefault();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  const onPointerDown = (event: ReactPointerEvent) => {
    if (swipe.current) {
      // A second finger: a pinch, not a swipe.
      swipe.current.pointers += 1;
      return;
    }
    swipe.current = zoomed ? null : { x: event.clientX, pointers: 1 };
  };
  const onPointerUp = (event: ReactPointerEvent) => {
    const start = swipe.current;
    swipe.current = null;
    if (!start || start.pointers > 1 || pinching()) return;
    const moved = event.clientX - start.x;
    if (Math.abs(moved) < SWIPE_PX) return;
    swiped.current = true;
    // Swiping toward the reading direction's start turns forward.
    if (moved < 0 !== rtl) forward();
    else back();
  };
  const onClick = (event: ReactMouseEvent<HTMLElement>) => {
    if (swiped.current) {
      swiped.current = false;
      return;
    }
    if (pinching()) return;
    onTap(event);
  };

  if (current >= total) {
    return (
      <div className="flex min-h-[80vh] flex-col justify-center">
        {endCard}
        <button type="button" onClick={back} className="text-muted self-center py-4 text-sm">
          {t('manga.backToLastPage')}
        </button>
      </div>
    );
  }

  const layout = pageLayout(scale, zoom);
  return (
    <div
      ref={box}
      data-testid="paged-reader"
      data-reader-scroll
      data-scale={scale}
      onClick={onClick}
      onPointerDown={onPointerDown}
      onPointerUp={onPointerUp}
      onPointerCancel={() => {
        swipe.current = null;
      }}
      // Sideways finger moves are the reader's swipes: a browser pan would cancel the pointer
      // before it lifts. A zoomed or full-size page pans both ways (and doesn't swipe).
      style={{ touchAction: zoomed || scale === 'original' ? 'pan-x pan-y' : 'pan-y' }}
      className={`relative h-[calc(100dvh-var(--app-bar,0px))] w-full select-none [-webkit-touch-callout:none] ${fitted ? 'flex items-center justify-center overflow-hidden' : 'grid overflow-auto [place-items:safe_center]'}`}
    >
      <ReaderImage
        key={`${current}:${pageIdentity(pages[current] ?? '')}`}
        url={pages[current] ?? ''}
        index={current}
        imageRef={(element) => {
          contentRef.current = element;
        }}
        onMenu={onPageMenu}
        className={layout.className}
        style={layout.style}
      />
    </div>
  );
};
