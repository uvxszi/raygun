import { commands } from '@raygun/bindings';
import type { MangaChapterPages, ReaderMode } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate, useRouter, useSearch } from '@tanstack/react-router';
import {
  ArrowLeft,
  BookOpen,
  Check,
  ChevronLeft,
  ChevronRight,
  ImageOff,
  LoaderCircle,
  Maximize,
  Minimize,
  SkipBack,
  SkipForward,
  SlidersHorizontal,
  ZoomIn,
  ZoomOut,
} from 'lucide-react';
import { useCallback, useEffect, useEffectEvent, useRef, useState } from 'react';
import type { CSSProperties, FC, MouseEvent as ReactMouseEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { PageActionsSheet } from '../export/PageActionsSheet';
import { unwrapResult } from '../lib/commandResult';
import { PagedView } from '../manga/reader/PagedView';
import { tapAction } from '../manga/reader/tapZones';
import type { TapAction } from '../manga/reader/tapZones';
import { useReaderZoom } from '../manga/reader/useReaderZoom';
import { useWakeLock } from '../manga/reader/useWakeLock';
import { WebtoonView } from '../manga/reader/WebtoonView';
import { READER_MODES, sizedFrom, useReaderDisplay, ZOOM_STEP } from '../manga/readerDisplay';
import { ReaderDisplaySettings } from '../manga/ReaderDisplaySettings';
import { MANGA, saveProgress, useMangaChapter } from '../manga/useManga';
import { Sheet } from '../shell/Sheet';
import { SourceIssue } from '../shell/SourceIssue';
import { useWindowFullscreen } from '../shell/useWindowFullscreen';

/** Progress is saved this long after reading stops, and right away on the last page. */
const SAVE_DELAY_MS = 800;
/** How close to the end the next chapter is fetched. */
const NEXT_CHAPTER_LEAD = 3;
/** A second tap within this long is a double tap (zoom), not a page turn. */
const DOUBLE_TAP_MS = 250;
/** A tap zone scrolls the strip by this share of the screen. */
const SCROLL_SHARE = 0.85;

const preload = (urls: string[]) => {
  for (const url of urls) {
    const image = new Image();
    image.src = url;
  }
};

type EndCardProps = {
  data: MangaChapterPages;
  onGo: (chapterId: string | null) => void;
  onClose: () => void;
};

const EndCard: FC<EndCardProps> = ({ data, onGo, onClose }) => {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col items-center gap-3 px-4 py-10 text-white">
      <p className="text-sm text-white/70">{t('manga.endOf', { name: data.chapter.name })}</p>
      {!data.nextChapterId && <p className="text-sm text-white/70">{t('manga.lastChapter')}</p>}
      <div className="flex flex-wrap justify-center gap-3">
        {data.previousChapterId && (
          <button
            type="button"
            onClick={() => onGo(data.previousChapterId)}
            className="flex items-center gap-1 rounded-full bg-white/15 px-4 py-2 text-sm"
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
          <button
            type="button"
            onClick={onClose}
            className="rounded-full bg-white/15 px-4 py-2 text-sm"
          >
            {t('manga.closeReader')}
          </button>
        )}
      </div>
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
  /** A mode picked in this reader (`own: null` follows the default again). */
  const [modeChoice, setModeChoice] = useState<{ own: ReaderMode | null } | null>(null);
  const [jump, setJump] = useState<{ page: number; nonce: number } | null>(null);
  const [pageMenu, setPageMenu] = useState<number | null>(null);
  const [showingDisplay, setShowingDisplay] = useState(false);
  const [choosingMode, setChoosingMode] = useState(false);
  const { display, ready } = useReaderDisplay();
  const fullscreen = useWindowFullscreen();
  const zoom = useReaderZoom();
  const sectionRef = useRef<HTMLElement>(null);
  const tapTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const data = loaded.data;
  const total = data?.pages.length ?? 0;
  const own = modeChoice ? modeChoice.own : (data?.ownReaderMode ?? null);
  const mode: ReaderMode =
    own ?? (ready ? display.defaultMode : (data?.readerMode ?? display.defaultMode));
  const page = Math.min(current, Math.max(total - 1, 0));
  useWakeLock(display.keepScreenOn && data !== undefined);

  const jumpTo = (target: number) => {
    setCurrent(target);
    setJump((last) => ({ page: target, nonce: (last?.nonce ?? 0) + 1 }));
  };

  // Resume where this chapter was left.
  const resumed = useRef<string | null>(null);
  useEffect(() => {
    if (!data || resumed.current === data.chapter.id) return;
    resumed.current = data.chapter.id;
    const resume = Math.min(data.chapter.lastPage ?? 0, Math.max(data.pages.length - 1, 0));
    setCurrent(resume);
    setJump(resume > 0 ? { page: resume, nonce: Date.now() } : null);
  }, [data]);

  useEffect(() => {
    if (!data || total === 0) return;
    const save = () => void saveProgress(data.chapter.id, page, total).catch(() => undefined);
    if (page >= total - 1) {
      save();
      return;
    }
    const timer = setTimeout(save, SAVE_DELAY_MS);
    return () => clearTimeout(timer);
  }, [page, data, total]);

  // Near the end, the next chapter's page list and first pages load ahead.
  useEffect(() => {
    const next = data?.nextChapterId;
    if (!next || page < total - NEXT_CHAPTER_LEAD) return;
    void queryClient
      .fetchQuery({
        queryKey: [MANGA, 'chapter', title, next],
        queryFn: () => unwrapResult(commands.mangaChapter(title, next)),
        staleTime: 60_000,
      })
      .then((upcoming) => preload(upcoming.pages.slice(0, display.preloadPages)))
      .catch(() => undefined);
  }, [data, page, total, title, queryClient, display.preloadPages]);

  // Zoom (M46.7): Ctrl+scroll at the pointer, Ctrl+plus/minus, Ctrl+0 back to the fit.
  const onZoomKey = useEffectEvent((event: KeyboardEvent) => {
    if (!event.ctrlKey && !event.metaKey) return;
    if (event.key === '=' || event.key === '+') zoom.zoomAt(zoom.zoom + ZOOM_STEP);
    else if (event.key === '-') zoom.zoomAt(zoom.zoom - ZOOM_STEP);
    else if (event.key === '0') zoom.zoomAt(1);
    else return;
    event.preventDefault();
  });
  const onWheel = useEffectEvent((event: WheelEvent) => {
    if (!event.ctrlKey) return;
    // Otherwise the webview zooms the whole app.
    event.preventDefault();
    zoom.zoomAt(zoom.zoom + (event.deltaY < 0 ? ZOOM_STEP : -ZOOM_STEP), {
      x: event.clientX,
      y: event.clientY,
    });
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

  // Two fingers zoom the page or the strip.
  const bindPinch = useEffectEvent((element: HTMLElement | null) => zoom.bindPinch(element));
  useEffect(() => bindPinch(sectionRef.current), []);

  // The title page and library show read state; refresh them when leaving the reader.
  useEffect(
    () => () => {
      void queryClient.invalidateQueries({ queryKey: [MANGA, 'title'] });
      void queryClient.invalidateQueries({ queryKey: [MANGA, 'library'] });
      void queryClient.invalidateQueries({ queryKey: [MANGA, 'updates'] });
    },
    [queryClient],
  );
  useEffect(
    () => () => {
      if (tapTimer.current) clearTimeout(tapTimer.current);
    },
    [],
  );

  // Chapters replace each other: Back leaves the reader.
  const goTo = (chapterId: string | null) => {
    if (!chapterId) return;
    setCurrent(0);
    setJump(null);
    void navigate({ search: { title, chapter: chapterId }, replace: true });
  };
  const close = () => router.history.back();
  const hideChrome = useCallback(() => setChrome(false), []);

  const act = (action: TapAction) => {
    if (action === 'menu') {
      setChrome((shown) => !shown);
      return;
    }
    const delta = action === 'forward' ? 1 : -1;
    if (mode === 'webtoon') {
      const scroller = sectionRef.current?.closest('main');
      scroller?.scrollBy?.({ top: delta * scroller.clientHeight * SCROLL_SHARE });
    } else {
      setCurrent((at) => Math.min(Math.max(at + delta, 0), total));
    }
  };

  const onTap = (event: ReactMouseEvent<HTMLElement>) => {
    const target = event.target as HTMLElement;
    if (target.closest('button, a, input')) return;
    // The strip is taller than the screen: its zones are the screen's.
    const area = (
      (mode === 'webtoon' ? target.closest('main') : null) ?? event.currentTarget
    ).getBoundingClientRect();
    const x = area.width > 0 ? (event.clientX - area.left) / area.width : 0.5;
    const y = area.height > 0 ? (event.clientY - area.top) / area.height : 0.5;
    // A zoomed page pans; taps only show the controls.
    const action =
      mode !== 'webtoon' && zoom.zoom > 1
        ? 'menu'
        : tapAction(display.tapZones, display.invertTaps, mode, x, y);
    if (tapTimer.current) clearTimeout(tapTimer.current);
    if (!display.doubleTapZoom) {
      act(action);
      return;
    }
    tapTimer.current = setTimeout(() => {
      tapTimer.current = null;
      act(action);
    }, DOUBLE_TAP_MS);
  };

  const onDoubleTap = (event: ReactMouseEvent<HTMLElement>) => {
    if (!display.doubleTapZoom || (event.target as HTMLElement).closest('button, a')) return;
    if (tapTimer.current) clearTimeout(tapTimer.current);
    tapTimer.current = null;
    zoom.zoomAt(zoom.zoom > 1 ? 1 : 2, { x: event.clientX, y: event.clientY });
  };

  const changeMode = (next: ReaderMode | null) => {
    setModeChoice({ own: next });
    setChoosingMode(false);
    // The strip opens on the page being read, not at its top.
    if (page > 0) jumpTo(page);
    if (data) void commands.mangaSetReaderMode(data.titleId, next);
  };

  const endCard = data && <EndCard data={data} onGo={goTo} onClose={close} />;
  const modeLabel = (value: ReaderMode) => t(`manga.mode.${value}`);

  return (
    <section
      ref={sectionRef}
      data-testid="manga-reader"
      // Edge to edge: the page area's side and bottom padding is the reader's too.
      className="-mx-4 -mb-4 flex min-h-full flex-col bg-black lg:-mx-10"
    >
      {chrome && (
        <header className="bg-surface/95 fixed inset-x-0 top-[var(--app-bar,0px)] z-30 flex min-h-16 items-center gap-1 px-1 pt-[env(safe-area-inset-top)] shadow-md backdrop-blur">
          <button
            type="button"
            aria-label={t('manga.closeReader')}
            onClick={close}
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
        <div className="px-4 pt-24">
          <SourceIssue onRetry={() => void loaded.refetch()}>{loaded.error.message}</SourceIssue>
        </div>
      )}
      {!data && !loaded.error && (
        <LoaderCircle
          aria-label={t('manga.loading')}
          className="text-muted m-auto mt-32 animate-spin"
        />
      )}
      {data && total === 0 && (
        <div className="flex min-h-[100dvh] flex-col items-center justify-center gap-3 px-6 text-center text-white">
          <ImageOff aria-hidden size={32} className="text-white/60" />
          <p className="text-sm">{t('manga.noPages')}</p>
          <button
            type="button"
            onClick={() => void loaded.refetch()}
            className="rounded-full bg-white/15 px-4 py-2 text-sm"
          >
            {t('issue.retry')}
          </button>
          {endCard}
        </div>
      )}
      {data && total > 0 && mode === 'webtoon' && (
        <WebtoonView
          key={data.chapter.id}
          pages={data.pages}
          current={current}
          jump={jump}
          zoom={zoom.zoom}
          size={display.webtoonSize}
          preload={display.preloadPages}
          chrome={chrome}
          contentRef={zoom.contentRef}
          onCurrent={setCurrent}
          onTap={onTap}
          onDoubleTap={onDoubleTap}
          onHideChrome={hideChrome}
          onPageMenu={setPageMenu}
          endCard={endCard}
        />
      )}
      {data && total > 0 && mode !== 'webtoon' && (
        <PagedView
          pages={data.pages}
          current={current}
          rtl={mode === 'pagedRtl'}
          zoom={zoom.zoom}
          scale={display.pagedScale}
          preload={display.preloadPages}
          contentRef={zoom.contentRef}
          onCurrent={setCurrent}
          onTap={onTap}
          onDoubleTap={onDoubleTap}
          onPageMenu={setPageMenu}
          pinching={zoom.pinching}
          endCard={endCard}
        />
      )}
      {data && !chrome && display.pageNumber && total > 0 && current < total && (
        <span
          data-testid="page-number"
          aria-hidden
          className="pointer-events-none fixed inset-x-0 bottom-[calc(0.5rem+env(safe-area-inset-bottom))] z-20 mx-auto w-fit rounded-full bg-black/60 px-2.5 py-0.5 text-xs font-medium text-white/90 tabular-nums"
        >
          {page + 1} / {total}
        </span>
      )}
      {data && chrome && total > 0 && (
        <footer className="fixed inset-x-0 bottom-0 z-30 flex justify-center px-3 pt-2 pb-[calc(0.75rem+env(safe-area-inset-bottom))]">
          <div
            data-testid="reader-bar"
            // The bar: the screen's width on phones and 70 % on wide windows, made bigger or
            // smaller by the page bar size (M46.48).
            className="flex w-(--bar-width) items-center gap-2 [--bar-base:100%] lg:[--bar-base:70%]"
            style={
              { '--bar-width': sizedFrom('var(--bar-base)', display.barSize) } as CSSProperties
            }
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
                onChange={(event) => jumpTo(Number(event.target.value))}
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
                  onClick={() => zoom.zoomAt(zoom.zoom - ZOOM_STEP)}
                  className="hover:bg-background grid size-12 place-items-center rounded-full"
                >
                  <ZoomOut aria-hidden size={20} />
                </button>
                <button
                  type="button"
                  aria-label={t('reader.display.zoomReset')}
                  onClick={() => zoom.zoomAt(1)}
                  className="min-w-12 px-1 text-sm font-medium tabular-nums"
                >
                  {Math.round(zoom.zoom * 100)}%
                </button>
                <button
                  type="button"
                  aria-label={t('reader.display.zoomIn')}
                  onClick={() => zoom.zoomAt(zoom.zoom + ZOOM_STEP)}
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
            {[null, ...READER_MODES].map((option) => (
              <button
                key={option ?? 'default'}
                type="button"
                role="radio"
                aria-checked={own === option}
                onClick={() => changeMode(option)}
                className="hover:bg-background flex items-center gap-4 rounded-xl px-3 py-3 text-left"
              >
                <span className="flex-1">
                  {option
                    ? modeLabel(option)
                    : t('manga.mode.default', { mode: modeLabel(display.defaultMode) })}
                </span>
                {own === option && <Check aria-hidden size={20} className="text-primary" />}
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
                  onClick={() => zoom.zoomAt(zoom.zoom - ZOOM_STEP)}
                  className="bg-background grid size-9 place-items-center rounded-full"
                >
                  <ZoomOut aria-hidden size={18} />
                </button>
                <button
                  type="button"
                  aria-label={t('reader.display.zoomReset')}
                  onClick={() => zoom.zoomAt(1)}
                  className="min-w-14 text-sm font-medium tabular-nums"
                >
                  {Math.round(zoom.zoom * 100)}%
                </button>
                <button
                  type="button"
                  aria-label={t('reader.display.zoomIn')}
                  onClick={() => zoom.zoomAt(zoom.zoom + ZOOM_STEP)}
                  className="bg-background grid size-9 place-items-center rounded-full"
                >
                  <ZoomIn aria-hidden size={18} />
                </button>
              </div>
            </div>
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
