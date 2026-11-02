import { commands } from '@raygun/bindings';
import type { MangaChapterPages, ReaderMode } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { Link, useNavigate, useRouter, useSearch } from '@tanstack/react-router';
import {
  ArrowLeft,
  BookOpen,
  Check,
  ChevronLeft,
  ChevronRight,
  LoaderCircle,
  SkipBack,
  SkipForward,
} from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import type { FC, MouseEvent as ReactMouseEvent, PointerEvent as ReactPointerEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { PageActionsSheet } from '../export/PageActionsSheet';
import { unwrapResult } from '../lib/commandResult';
import { Sheet } from '../shell/Sheet';
import { MANGA, saveProgress, useMangaChapter } from '../manga/useManga';

/** Progress is saved this long after reading stops, and right away on the last page. */
const SAVE_DELAY_MS = 800;
/** Pages after the current one that start loading ahead. */
const PRELOAD_AHEAD = 3;
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
};

/** One continuous strip; the page most in view is the current one. */
const WebtoonView: FC<ViewProps & { jumpTo: number | null }> = ({
  data,
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
        className="flex flex-col"
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

/** One page at a time with tap zones, swipes, arrow keys and double-tap zoom. */
const PagedView: FC<ViewProps & { rtl: boolean }> = ({
  data,
  current,
  onCurrent,
  onToggleChrome,
  onGo,
  onPageMenu,
  rtl,
}) => {
  const { t } = useTranslation();
  const total = data.pages.length;
  const [zoomed, setZoomed] = useState(false);
  const tapTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const swipeStart = useRef<number | null>(null);
  const swiped = useRef(false);

  // Index `total` is the end card after the last page.
  const step = (delta: number) => {
    setZoomed(false);
    onCurrent(Math.min(Math.max(current + delta, 0), total));
  };
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
    setZoomed((on) => !on);
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
      data-testid="paged-reader"
      onClick={onTap}
      onDoubleClick={onDoubleTap}
      onPointerDown={onPointerDown}
      onPointerUp={onPointerUp}
      className={`h-[100dvh] w-full select-none ${zoomed ? 'block overflow-auto' : 'flex items-center justify-center overflow-hidden'}`}
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
        // Zoomed: twice the screen width at its own aspect ratio, panned by scrolling.
        className={zoomed ? 'h-auto max-w-none' : 'max-h-full max-w-full object-contain'}
        style={zoomed ? { width: '200%' } : undefined}
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
    preload(data.pages.slice(page + 1, page + 1 + PRELOAD_AHEAD));
    const next = data.nextChapterId;
    if (next && page >= total - NEXT_CHAPTER_LEAD) {
      void queryClient
        .fetchQuery({
          queryKey: [MANGA, 'chapter', title, next],
          queryFn: () => unwrapResult(commands.mangaChapter(title, next)),
          staleTime: 60_000,
        })
        .then((upcoming) => preload(upcoming.pages.slice(0, PRELOAD_AHEAD)))
        .catch(() => undefined);
    }
  }, [data, page, total, title, queryClient]);

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
        <header className="bg-surface/95 fixed inset-x-0 top-0 z-30 flex min-h-16 items-center gap-1 px-1 pt-[env(safe-area-inset-top)] shadow-md backdrop-blur">
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
        />
      )}
      {data && chrome && total > 0 && (
        <footer className="fixed inset-x-0 bottom-0 z-30 flex items-center gap-2 px-3 pt-2 pb-[calc(0.75rem+env(safe-area-inset-bottom))]">
          <button
            type="button"
            aria-label={t('manga.goPreviousChapter')}
            disabled={!data.previousChapterId}
            onClick={() => goTo(data.previousChapterId)}
            className="bg-surface/95 grid size-12 shrink-0 place-items-center rounded-full shadow-md backdrop-blur disabled:opacity-40"
          >
            <SkipBack aria-hidden size={20} />
          </button>
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
          <button
            type="button"
            aria-label={t('manga.goNextChapter')}
            disabled={!data.nextChapterId}
            onClick={() => goTo(data.nextChapterId)}
            className="bg-surface/95 grid size-12 shrink-0 place-items-center rounded-full shadow-md backdrop-blur disabled:opacity-40"
          >
            <SkipForward aria-hidden size={20} />
          </button>
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
