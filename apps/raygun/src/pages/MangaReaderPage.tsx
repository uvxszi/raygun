import { commands } from '@raygun/bindings';
import type { MangaChapterPages, PagedScale, ReaderMode } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { Link, useNavigate, useRouter, useSearch } from '@tanstack/react-router';
import {
  ArrowLeft,
  BookOpen,
  Check,
  ChevronLeft,
  ChevronRight,
  LoaderCircle,
  Maximize,
  Minimize,
  SkipBack,
  SkipForward,
  SlidersHorizontal,
  ZoomIn,
  ZoomOut,
} from 'lucide-react';
import { useEffect, useEffectEvent, useRef, useState } from 'react';
import type {
  CSSProperties,
  FC,
  MouseEvent as ReactMouseEvent,
  PointerEvent as ReactPointerEvent,
} from 'react';
import { useTranslation } from 'react-i18next';

import { PageActionsSheet } from '../export/PageActionsSheet';
import { unwrapResult } from '../lib/commandResult';
import { Sheet } from '../shell/Sheet';
import { useWindowFullscreen } from '../shell/useWindowFullscreen';
import { clampZoom, useReaderDisplay, ZOOM_STEP } from '../manga/readerDisplay';
import { ReaderDisplaySettings } from '../manga/ReaderDisplaySettings';
import { MANGA, saveProgress, useMangaChapter } from '../manga/useManga';

/** Progress is saved this long after reading stops, and right away on the last page. */
const SAVE_DELAY_MS = 800;
/** How close to the end the next chapter is fetched. */
const NEXT_CHAPTER_LEAD = 3;
/** A second tap within this long is a double tap (zoom), not a page turn. */
const DOUBLE_TAP_MS = 250;
/** Horizontal movement that counts as a swipe. */
const SWIPE_PX = 50;
const MODES: ReaderMode[] = ['webtoon', 'pagedRtl', 'pagedLtr'];

const preload = (urls: string[]) => {
  for (const url of urls) {
    const image = new Image();
    image.src = url;
  }
};

type EndCardProps = { data: MangaChapterPages; onGo: (chapterId: string | null) => void };

const EndCard: FC<EndCardProps> = ({ data, onGo }) => {
  const { t } = useTranslation();
  return (
    <div className="text-foreground flex flex-col items-center gap-3 px-4 py-10">
      <p className="text-muted text-sm">{t('manga.endOf', { name: data.chapter.name })}</p>
      <div className="flex gap-3">
        {data.previousChapterId && (
          <button
            type="button"
            onClick={() => onGo(data.previousChapterId)}
            className="bg-surface flex items-center gap-1 rounded-full px-4 py-2 text-sm"
          >
            <ChevronLeft aria-hidden size={16} />
            {t('manga.previousChapter')}
          </button>
        )}
        {data.nextChapterId ? (
          <button
            type="button"
            onClick={() => onGo(data.nextChapterId)}
            className="bg-primary text-background flex items-center gap-1 rounded-full px-5 py-2 text-sm font-semibold"
          >
            {t('manga.nextChapter')}
            <ChevronRight aria-hidden size={16} />
          </button>
        ) : (
          <Link
            to="/library"
            search={{ media: 'manga' }}
            className="bg-surface rounded-full px-4 py-2 text-sm"
          >
            {t('manga.backToLibrary')}
          </Link>
        )}
      </div>
    </div>
  );
};

type ViewProps = {
  data: MangaChapterPages;
  current: number;
  onCurrent: (page: number) => void;
  onToggleChrome: () => void;
  onGo: (chapterId: string | null) => void;
  /** A long press on a page (M46.1). */
  onPageMenu: (page: number) => void;
  /** The reader's zoom, 1 being the page's fit (M46.7). */
  zoom: number;
};

/** One continuous strip; the page most in view is the current one. */
const WebtoonView: FC<ViewProps & { jumpTo: number | null; width: number }> = ({
  data,
  zoom,
  width,
  onCurrent,
  onToggleChrome,
  onGo,
  onPageMenu,
  jumpTo,
}) => {
  const { t } = useTranslation();
  const pages = useRef<(HTMLImageElement | null)[]>([]);

  useEffect(() => {
    if (typeof IntersectionObserver === 'undefined') return;
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting)
            onCurrent(Number((entry.target as HTMLElement).dataset.index ?? 0));
        }
      },
      { threshold: 0.5 },
    );
    for (const page of pages.current) {
      if (page) observer.observe(page);
    }
    return () => observer.disconnect();
  }, [data, onCurrent]);

  useEffect(() => {
    if (jumpTo !== null) pages.current[jumpTo]?.scrollIntoView?.();
  }, [jumpTo]);

  return (
    <>
      <button
        type="button"
        aria-label={t('manga.toggleControls')}
        onClick={onToggleChrome}
        // The column: a share of the window, or the screen's width up to 768 px (M46.7).
        className="mx-auto flex max-w-none flex-col"
        style={{
          width: width > 0 ? `${Math.round(width * zoom)}%` : `calc(min(100%, 48rem) * ${zoom})`,
        }}
      >
        {data.pages.map((url, index) => (
          <img
            key={url}
            ref={(element) => {
              pages.current[index] = element;
            }}
            data-index={index}
            data-testid="manga-page"
            src={url}
            alt={t('manga.pageAlt', { page: index + 1 })}
            loading={index < 3 ? 'eager' : 'lazy'}
            decoding="async"
            className="min-h-[60vh] w-full bg-neutral-900 object-contain"
            onLoad={(event) => event.currentTarget.classList.remove('min-h-[60vh]')}
            onContextMenu={(event) => {
              event.preventDefault();
              onPageMenu(index);
            }}
          />
        ))}
      </button>
      <EndCard data={data} onGo={onGo} />
    </>
  );
};

/** How a page sits in the paged modes: the chosen fit, made `zoom` times bigger (M46.7). */
const pageLayout = (
  scale: PagedScale,
  zoom: number,
): { className: string; style: CSSProperties } => {
  switch (scale) {
    case 'fitWidth':
      return { className: 'h-auto max-w-none', style: { width: `${Math.round(100 * zoom)}%` } };
    case 'fitHeight':
      return { className: 'w-auto max-w-none', style: { height: `${Math.round(100 * zoom)}dvh` } };
    case 'original':
      return { className: 'max-w-none', style: { zoom } };
    default:
      return zoom === 1
        ? { className: 'max-h-full max-w-full object-contain', style: {} }
        : {
            className: 'max-w-none object-contain',
            style: { width: `${Math.round(100 * zoom)}%`, height: `${Math.round(100 * zoom)}dvh` },
          };
  }
};

/** One page at a time with tap zones, swipes, arrow keys and double-tap zoom. */
const PagedView: FC<
  ViewProps & { rtl: boolean; scale: PagedScale; onZoom: (zoom: number) => void }
> = ({ data, current, onCurrent, onToggleChrome, onGo, onPageMenu, rtl, zoom, scale, onZoom }) => {
  const { t } = useTranslation();
  const total = data.pages.length;
  const zoomed = zoom > 1;
  const fitted = zoom === 1 && scale === 'fitScreen';
  const box = useRef<HTMLDivElement>(null);
  const tapTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const swipeStart = useRef<number | null>(null);
  const swiped = useRef(false);

  // Index `total` is the end card after the last page.
  const step = (delta: number) => {
    onCurrent(Math.min(Math.max(current + delta, 0), total));
  };

  // A new page starts at its top left.
  useEffect(() => {
    box.current?.scrollTo?.(0, 0);
  }, [current]);
  const forward = () => step(1);
  const back = () => step(-1);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'ArrowRight') (rtl ? back : forward)();
      if (event.key === 'ArrowLeft') (rtl ? forward : back)();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  useEffect(
    () => () => {
      if (tapTimer.current) clearTimeout(tapTimer.current);
    },
    [],
  );

  const onTap = (event: ReactMouseEvent<HTMLDivElement>) => {
    if (swiped.current) {
      swiped.current = false;
      return;
    }
    const box = event.currentTarget.getBoundingClientRect();
    const fraction = box.width > 0 ? (event.clientX - box.left) / box.width : 0.5;
    if (tapTimer.current) clearTimeout(tapTimer.current);
    tapTimer.current = setTimeout(() => {
      tapTimer.current = null;
      if (zoomed || (fraction > 1 / 3 && fraction < 2 / 3)) onToggleChrome();
      else if (fraction <= 1 / 3) (rtl ? forward : back)();
      else (rtl ? back : forward)();
    }, DOUBLE_TAP_MS);
  };

  const onDoubleTap = () => {
    if (tapTimer.current) clearTimeout(tapTimer.current);
    tapTimer.current = null;
    onZoom(zoomed ? 1 : 2);
  };

  const onPointerDown = (event: ReactPointerEvent) => {
    swipeStart.current = zoomed ? null : event.clientX;
  };
  const onPointerUp = (event: ReactPointerEvent) => {
    if (swipeStart.current === null) return;
    const moved = event.clientX - swipeStart.current;
    swipeStart.current = null;
    if (Math.abs(moved) < SWIPE_PX) return;
    swiped.current = true;
    // Swiping toward the reading direction's start turns forward.
    if (moved < 0 !== rtl) forward();
    else back();
  };

  if (current >= total) {
    return (
      <div className="flex min-h-[80vh] flex-col justify-center">
        <EndCard data={data} onGo={onGo} />
        <button type="button" onClick={back} className="text-muted self-center py-4 text-sm">
          {t('manga.backToLastPage')}
        </button>
      </div>
    );
  }

  return (
    <div
      ref={box}
      data-testid="paged-reader"
      data-scale={scale}
      onClick={onTap}
      onDoubleClick={onDoubleTap}
      onPointerDown={onPointerDown}
      onPointerUp={onPointerUp}
      className={`h-[100dvh] w-full select-none ${fitted ? 'flex items-center justify-center overflow-hidden' : 'grid overflow-auto [place-items:safe_center]'}`}
    >
      <img
        key={data.pages[current]}
        data-testid="manga-page"
        src={data.pages[current]}
        alt={t('manga.pageAlt', { page: current + 1 })}
        draggable={false}
        onContextMenu={(event) => {
          event.preventDefault();
          onPageMenu(current);
        }}
        className={pageLayout(scale, zoom).className}
        style={pageLayout(scale, zoom).style}
      />
    </div>
  );
};

/** The manga reader: webtoon strip or paged, left or right to left (M27). */
export const MangaReaderPage: FC = () => {
  const { t } = useTranslation();
  const { title, chapter } = useSearch({ from: '/manga/read' });
  const navigate = useNavigate({ from: '/manga/read' });
  const router = useRouter();
  const queryClient = useQueryClient();
  const loaded = useMangaChapter(title, chapter);
  const [current, setCurrent] = useState(0);
  const [chrome, setChrome] = useState(true);
  const [modeOverride, setModeOverride] = useState<ReaderMode | null>(null);
  const [jumpTo, setJumpTo] = useState<number | null>(null);
  const [pageMenu, setPageMenu] = useState<number | null>(null);
  const [zoom, setZoomState] = useState(1);
  const [showingDisplay, setShowingDisplay] = useState(false);
  const { display } = useReaderDisplay();
  const fullscreen = useWindowFullscreen();
  const setZoom = (next: number) => setZoomState(clampZoom(next));
  const data = loaded.data;
  const total = data?.pages.length ?? 0;
  const mode = modeOverride ?? data?.readerMode ?? 'webtoon';
  const page = Math.min(current, Math.max(total - 1, 0));

  // Resume where this chapter was left.
  const resumed = useRef<string | null>(null);
  useEffect(() => {
    if (!data || resumed.current === data.chapter.id) return;
    resumed.current = data.chapter.id;
    const resume = data.chapter.lastPage ?? 0;
    setCurrent(resume);
    setJumpTo(resume || null);
  }, [data]);

  useEffect(() => {
    if (!data || total === 0) return;
    const save = () => void saveProgress(data.chapter.id, page, total);
    if (page >= total - 1) {
      save();
      return;
    }
    const timer = setTimeout(save, SAVE_DELAY_MS);
    return () => clearTimeout(timer);
  }, [page, data, total]);

  // Load ahead: the next few pages, and near the end the next chapter.
  useEffect(() => {
    if (!data) return;
    preload(data.pages.slice(page + 1, page + 1 + display.preloadPages));
    const next = data.nextChapterId;
    if (next && page >= total - NEXT_CHAPTER_LEAD) {
      void queryClient
        .fetchQuery({
          queryKey: [MANGA, 'chapter', title, next],
          queryFn: () => unwrapResult(commands.mangaChapter(title, next)),
          staleTime: 60_000,
        })
        .then((upcoming) => preload(upcoming.pages.slice(0, display.preloadPages)))
        .catch(() => undefined);
    }
  }, [data, page, total, title, queryClient, display.preloadPages]);

  // Zoom (M46.7): Ctrl+scroll, Ctrl+plus/minus, Ctrl+0 back to the fit. Plain scrolling scrolls.
  const onZoomKey = useEffectEvent((event: KeyboardEvent) => {
    if (!event.ctrlKey && !event.metaKey) return;
    if (event.key === '=' || event.key === '+') setZoom(zoom + ZOOM_STEP);
    else if (event.key === '-') setZoom(zoom - ZOOM_STEP);
    else if (event.key === '0') setZoom(1);
    else return;
    event.preventDefault();
  });
  const onWheel = useEffectEvent((event: WheelEvent) => {
    if (!event.ctrlKey) return;
    // Otherwise the webview zooms the whole app.
    event.preventDefault();
    setZoom(zoom + (event.deltaY < 0 ? ZOOM_STEP : -ZOOM_STEP));
  });
  useEffect(() => {
    const key = (event: KeyboardEvent) => onZoomKey(event);
    const wheel = (event: WheelEvent) => onWheel(event);
    window.addEventListener('keydown', key);
    window.addEventListener('wheel', wheel, { passive: false });
    return () => {
      window.removeEventListener('keydown', key);
      window.removeEventListener('wheel', wheel);
    };
  }, []);

  // The title page and library show read state; refresh them when leaving the reader.
  useEffect(
    () => () => {
      void queryClient.invalidateQueries({ queryKey: [MANGA, 'title'] });
      void queryClient.invalidateQueries({ queryKey: [MANGA, 'library'] });
    },
    [queryClient],
  );

  const goTo = (chapterId: string | null) => {
    if (!chapterId) return;
    setCurrent(0);
    setJumpTo(null);
    void navigate({ search: { title, chapter: chapterId } });
  };

  const [choosingMode, setChoosingMode] = useState(false);
  const changeMode = (next: ReaderMode) => {
    setModeOverride(next);
    setChoosingMode(false);
    if (data) void commands.mangaSetReaderMode(data.titleId, next);
  };

  return (
    <section data-testid="manga-reader" className="-mx-4 flex flex-col bg-black lg:-mx-10">
      {chrome && (
        <header className="bg-surface/95 fixed inset-x-0 top-[var(--app-bar,0px)] z-30 flex min-h-16 items-center gap-1 px-1 pt-[env(safe-area-inset-top)] shadow-md backdrop-blur">
          <button
            type="button"
            aria-label={t('manga.closeReader')}
            onClick={() => router.history.back()}
            className="hover:bg-background grid size-11 shrink-0 place-items-center rounded-full"
          >
            <ArrowLeft aria-hidden size={22} />
          </button>
          <span className="flex min-w-0 flex-1 flex-col px-1 py-2">
            <span className="truncate text-base font-semibold">{data?.title}</span>
            <span className="text-muted truncate text-sm">{data?.chapter.name}</span>
          </span>
          <button
            type="button"
            aria-label={t('reader.display.title')}
            aria-haspopup="dialog"
            onClick={() => setShowingDisplay(true)}
            className="hover:bg-background grid size-11 shrink-0 place-items-center rounded-full"
          >
            <SlidersHorizontal aria-hidden size={22} />
          </button>
          <button
            type="button"
            aria-label={t('manga.readingMode')}
            aria-haspopup="dialog"
            onClick={() => setChoosingMode(true)}
            className="hover:bg-background grid size-11 shrink-0 place-items-center rounded-full"
          >
            <BookOpen aria-hidden size={22} />
          </button>
        </header>
      )}
      {loaded.error && (
        <p role="alert" className="text-primary p-6 pt-20 text-sm wrap-anywhere">
          {loaded.error.message}
        </p>
      )}
      {!data && !loaded.error && (
        <LoaderCircle
          aria-label={t('manga.loading')}
          className="text-muted m-auto mt-32 animate-spin"
        />
      )}
      {data && mode === 'webtoon' && (
        <WebtoonView
          data={data}
          current={current}
          onCurrent={setCurrent}
          onToggleChrome={() => setChrome((shown) => !shown)}
          onGo={goTo}
          onPageMenu={setPageMenu}
          jumpTo={jumpTo}
          zoom={zoom}
          width={display.webtoonWidth}
        />
      )}
      {data && mode !== 'webtoon' && (
        <PagedView
          data={data}
          current={current}
          onCurrent={setCurrent}
          onToggleChrome={() => setChrome((shown) => !shown)}
          onGo={goTo}
          onPageMenu={setPageMenu}
          rtl={mode === 'pagedRtl'}
          zoom={zoom}
          scale={display.pagedScale}
          onZoom={setZoom}
        />
      )}
      {data && chrome && total > 0 && (
        <footer className="fixed inset-x-0 bottom-0 z-30 flex justify-center px-3 pt-2 pb-[calc(0.75rem+env(safe-area-inset-bottom))]">
          <div
            data-testid="reader-bar"
            // The bar's width: the user's share, or the screen's on phones and 70 % on wide windows.
            className={`flex items-center gap-2 ${display.barWidth > 0 ? '' : 'w-full lg:w-[70%]'}`}
            style={display.barWidth > 0 ? { width: `${display.barWidth}%` } : undefined}
          >
            {display.barChapters && (
              <button
                type="button"
                aria-label={t('manga.goPreviousChapter')}
                disabled={!data.previousChapterId}
                onClick={() => goTo(data.previousChapterId)}
                className="bg-surface/95 grid size-12 shrink-0 place-items-center rounded-full shadow-md backdrop-blur disabled:opacity-40"
              >
                <SkipBack aria-hidden size={20} />
              </button>
            )}
            <div className="bg-surface/95 flex min-w-0 flex-1 items-center gap-3 rounded-full px-4 py-3 shadow-md backdrop-blur">
              <input
                type="range"
                aria-label={t('manga.jumpToPage')}
                min={0}
                max={total - 1}
                value={page}
                dir={mode === 'pagedRtl' ? 'rtl' : 'ltr'}
                onChange={(event) => {
                  const target = Number(event.target.value);
                  setCurrent(target);
                  setJumpTo(target);
                }}
                className="accent-primary min-w-0 flex-1"
              />
              <span
                data-testid="page-indicator"
                className="shrink-0 text-sm font-medium tabular-nums"
              >
                {t('manga.pageOf', { page: page + 1, total })}
              </span>
            </div>
            {display.barZoom && (
              <div className="bg-surface/95 flex shrink-0 items-center rounded-full shadow-md backdrop-blur">
                <button
                  type="button"
                  aria-label={t('reader.display.zoomOut')}
                  onClick={() => setZoom(zoom - ZOOM_STEP)}
                  className="hover:bg-background grid size-12 place-items-center rounded-full"
                >
                  <ZoomOut aria-hidden size={20} />
                </button>
                <button
                  type="button"
                  aria-label={t('reader.display.zoomReset')}
                  onClick={() => setZoom(1)}
                  className="min-w-12 px-1 text-sm font-medium tabular-nums"
                >
                  {Math.round(zoom * 100)}%
                </button>
                <button
                  type="button"
                  aria-label={t('reader.display.zoomIn')}
                  onClick={() => setZoom(zoom + ZOOM_STEP)}
                  className="hover:bg-background grid size-12 place-items-center rounded-full"
                >
                  <ZoomIn aria-hidden size={20} />
                </button>
              </div>
            )}
            {display.barMode && (
              <button
                type="button"
                aria-label={t('manga.readingMode')}
                onClick={() => setChoosingMode(true)}
                className="bg-surface/95 grid size-12 shrink-0 place-items-center rounded-full shadow-md backdrop-blur"
              >
                <BookOpen aria-hidden size={20} />
              </button>
            )}
            {display.barFullscreen && fullscreen.available && (
              <button
                type="button"
                aria-label={
                  fullscreen.on ? t('desktopPlayer.exitFullscreen') : t('desktopPlayer.fullscreen')
                }
                onClick={() => fullscreen.toggle()}
                className="bg-surface/95 grid size-12 shrink-0 place-items-center rounded-full shadow-md backdrop-blur"
              >
                {fullscreen.on ? (
                  <Minimize aria-hidden size={20} />
                ) : (
                  <Maximize aria-hidden size={20} />
                )}
              </button>
            )}
            {display.barChapters && (
              <button
                type="button"
                aria-label={t('manga.goNextChapter')}
                disabled={!data.nextChapterId}
                onClick={() => goTo(data.nextChapterId)}
                className="bg-surface/95 grid size-12 shrink-0 place-items-center rounded-full shadow-md backdrop-blur disabled:opacity-40"
              >
                <SkipForward aria-hidden size={20} />
              </button>
            )}
          </div>
        </footer>
      )}
      {choosingMode && (
        <Sheet title={t('manga.readingMode')} onClose={() => setChoosingMode(false)}>
          <div
            role="radiogroup"
            aria-label={t('manga.readingMode')}
            className="-mx-2 flex flex-col"
          >
            {MODES.map((option) => (
              <button
                key={option}
                type="button"
                role="radio"
                aria-checked={mode === option}
                onClick={() => changeMode(option)}
                className="hover:bg-background flex items-center gap-4 rounded-xl px-3 py-3 text-left"
              >
                <span className="flex-1">{t(`manga.mode.${option}`)}</span>
                {mode === option && <Check aria-hidden size={20} className="text-primary" />}
              </button>
            ))}
          </div>
        </Sheet>
      )}
      {showingDisplay && (
        <Sheet title={t('reader.display.title')} onClose={() => setShowingDisplay(false)}>
          <div className="-mx-2 flex flex-col gap-2">
            <div className="flex items-center justify-between gap-3 px-1">
              <span className="text-sm">{t('reader.display.zoom')}</span>
              <div className="flex items-center gap-1">
                <button
                  type="button"
                  aria-label={t('reader.display.zoomOut')}
                  onClick={() => setZoom(zoom - ZOOM_STEP)}
                  className="bg-background grid size-9 place-items-center rounded-full"
                >
                  <ZoomOut aria-hidden size={18} />
                </button>
                <button
                  type="button"
                  aria-label={t('reader.display.zoomReset')}
                  onClick={() => setZoom(1)}
                  className="min-w-14 text-sm font-medium tabular-nums"
                >
                  {Math.round(zoom * 100)}%
                </button>
                <button
                  type="button"
                  aria-label={t('reader.display.zoomIn')}
                  onClick={() => setZoom(zoom + ZOOM_STEP)}
                  className="bg-background grid size-9 place-items-center rounded-full"
                >
                  <ZoomIn aria-hidden size={18} />
                </button>
              </div>
            </div>
            <p className="text-muted px-1 text-xs">{t('reader.display.zoomHint')}</p>
            <ReaderDisplaySettings />
          </div>
        </Sheet>
      )}
      {data && pageMenu !== null && data.pages[pageMenu] && (
        <PageActionsSheet
          page={{
            titleId: data.titleId,
            chapterId: data.chapter.id,
            index: pageMenu,
            url: data.pages[pageMenu],
          }}
          onClose={() => setPageMenu(null)}
        />
      )}
    </section>
  );
};
