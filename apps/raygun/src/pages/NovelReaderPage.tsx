import type { NovelChapter, NovelReaderPrefs, TextBlock } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate, useRouter, useSearch } from '@tanstack/react-router';
import {
  ArrowLeft,
  ChevronLeft,
  ChevronRight,
  LoaderCircle,
  Minus,
  Plus,
  Type,
} from 'lucide-react';
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import type { FC, KeyboardEvent, MouseEvent, ReactNode, TouchEvent } from 'react';
import { useTranslation } from 'react-i18next';

import {
  DEFAULT_PREFS,
  NOVEL,
  blockAt,
  novelChapterQuery,
  saveNovelProgress,
  textOffsets,
  useNovelChapter,
} from '../novel/useNovel';
import { useSettings, useUpdateSetting } from '../settings/useSettings';

/** Progress is saved this long after reading stops moving. */
const SAVE_DELAY_MS = 800;
/** Past this share of a chapter, the next one is fetched ahead. */
const PREFETCH_AT = 0.6;
const SWIPE_PX = 50;
/** Side margin of a page in paged mode. */
const PAGE_PADDING = 20;

const FONTS: Record<NovelReaderPrefs['font'], string> = {
  serif: 'Georgia, "Noto Serif", "Times New Roman", serif',
  sans: 'system-ui, "Noto Sans", sans-serif',
};

const Block: FC<{
  block: TextBlock;
  index: number;
  register: (index: number, element: HTMLElement | null) => void;
}> = ({ block, index, register }) => {
  const ref = (element: HTMLElement | null) => register(index, element);
  switch (block.kind) {
    case 'heading':
      return (
        <h2 ref={ref} data-block={index} className="mt-[1.2em] mb-[0.8em] text-[1.3em] font-bold">
          {block.text}
        </h2>
      );
    case 'separator':
      return <hr ref={ref} data-block={index} className="border-muted/40 my-[1.5em]" />;
    case 'image':
      return (
        <img
          ref={ref}
          data-block={index}
          src={block.url}
          alt=""
          loading="lazy"
          className="mx-auto my-[1em] max-h-[80vh] max-w-full break-inside-avoid"
        />
      );
    default:
      return (
        <p ref={ref} data-block={index} className="mb-[0.9em]">
          {block.text}
        </p>
      );
  }
};

/** A row of −/value/+ buttons for one preference. */
const Stepper: FC<{
  label: string;
  value: string;
  onLess: () => void;
  onMore: () => void;
  lessDisabled: boolean;
  moreDisabled: boolean;
}> = ({ label, value, onLess, onMore, lessDisabled, moreDisabled }) => {
  const { t } = useTranslation();
  return (
    <div className="flex items-center justify-between gap-3">
      <span className="text-sm">{label}</span>
      <span className="flex items-center gap-2">
        <button
          type="button"
          aria-label={t('novel.less', { label })}
          disabled={lessDisabled}
          onClick={onLess}
          className="bg-background grid size-9 place-items-center rounded-full disabled:opacity-40"
        >
          <Minus aria-hidden size={16} />
        </button>
        <span className="w-12 text-center text-sm tabular-nums">{value}</span>
        <button
          type="button"
          aria-label={t('novel.more', { label })}
          disabled={moreDisabled}
          onClick={onMore}
          className="bg-background grid size-9 place-items-center rounded-full disabled:opacity-40"
        >
          <Plus aria-hidden size={16} />
        </button>
      </span>
    </div>
  );
};

const Choice: FC<{ pressed: boolean; onClick: () => void; children: ReactNode }> = ({
  pressed,
  onClick,
  children,
}) => (
  <button
    type="button"
    aria-pressed={pressed}
    onClick={onClick}
    className={`flex-1 rounded-lg px-3 py-2 text-sm ${pressed ? 'bg-primary text-background font-semibold' : 'bg-background'}`}
  >
    {children}
  </button>
);

const PrefsPanel: FC<{ prefs: NovelReaderPrefs; onChange: (prefs: NovelReaderPrefs) => void }> = ({
  prefs,
  onChange,
}) => {
  const { t } = useTranslation();
  const set = (patch: Partial<NovelReaderPrefs>) => onChange({ ...prefs, ...patch });
  return (
    <div
      role="dialog"
      aria-label={t('novel.textSettings')}
      className="bg-surface fixed inset-x-2 top-16 z-40 mx-auto flex max-w-md flex-col gap-3 rounded-xl p-4 shadow-xl"
    >
      <Stepper
        label={t('novel.fontSize')}
        value={`${prefs.fontSize}`}
        onLess={() => set({ fontSize: prefs.fontSize - 2 })}
        onMore={() => set({ fontSize: prefs.fontSize + 2 })}
        lessDisabled={prefs.fontSize <= 14}
        moreDisabled={prefs.fontSize >= 30}
      />
      <Stepper
        label={t('novel.lineSpacing')}
        value={(prefs.lineSpacing / 10).toFixed(1)}
        onLess={() => set({ lineSpacing: prefs.lineSpacing - 1 })}
        onMore={() => set({ lineSpacing: prefs.lineSpacing + 1 })}
        lessDisabled={prefs.lineSpacing <= 12}
        moreDisabled={prefs.lineSpacing >= 22}
      />
      <div className="flex gap-2">
        <Choice pressed={prefs.font === 'serif'} onClick={() => set({ font: 'serif' })}>
          {t('novel.serif')}
        </Choice>
        <Choice pressed={prefs.font === 'sans'} onClick={() => set({ font: 'sans' })}>
          {t('novel.sans')}
        </Choice>
      </div>
      <div className="flex gap-2">
        <Choice pressed={prefs.width === 'narrow'} onClick={() => set({ width: 'narrow' })}>
          {t('novel.narrow')}
        </Choice>
        <Choice pressed={prefs.width === 'wide'} onClick={() => set({ width: 'wide' })}>
          {t('novel.wide')}
        </Choice>
      </div>
      <div className="flex gap-2">
        <Choice pressed={!prefs.paged} onClick={() => set({ paged: false })}>
          {t('novel.scroll')}
        </Choice>
        <Choice pressed={prefs.paged} onClick={() => set({ paged: true })}>
          {t('novel.paged')}
        </Choice>
      </div>
    </div>
  );
};

/** One chapter: the text in the reader's layout, with progress kept as a character offset. */
const ChapterView: FC<{
  data: NovelChapter;
  prefs: NovelReaderPrefs;
  chrome: boolean;
  onToggleChrome: () => void;
  onNext: () => void;
  onPrevious: () => void;
  onProgress: (fraction: number, label: string) => void;
}> = ({ data, prefs, chrome, onToggleChrome, onNext, onPrevious, onProgress }) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const { starts, total } = useMemo(() => textOffsets(data.blocks), [data.blocks]);
  const refs = useRef<(HTMLElement | null)[]>([]);
  const endRef = useRef<HTMLDivElement>(null);
  const pagesRef = useRef<HTMLDivElement>(null);
  const saveTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const finished = useRef(data.chapter.read);
  const [page, setPage] = useState(0);
  const [pageCount, setPageCount] = useState(1);
  const swipeStart = useRef<number | null>(null);
  const chapterId = data.chapter.id;
  const register = useCallback((index: number, element: HTMLElement | null) => {
    refs.current[index] = element;
  }, []);

  /** The offset not yet saved, written when the timer fires or the chapter closes. */
  const pending = useRef<number | null>(null);

  // Saves progress and keeps the cached chapter in step, so reopening it resumes here.
  const remember = useCallback(
    (offset: number) => {
      pending.current = null;
      void saveNovelProgress(chapterId, offset, total).catch(() => {});
      queryClient.setQueryData<NovelChapter>(
        novelChapterQuery(data.titleId, chapterId).queryKey,
        (cached) =>
          cached && {
            ...cached,
            chapter: {
              ...cached.chapter,
              lastPage: offset,
              totalPages: total,
              read: cached.chapter.read || offset >= total - 1,
            },
          },
      );
      void queryClient.invalidateQueries({ queryKey: [NOVEL, 'title'] });
      void queryClient.invalidateQueries({ queryKey: [NOVEL, 'library'] });
    },
    [chapterId, data.titleId, queryClient, total],
  );

  /** The block reading is at, to keep in view when the text reflows. */
  const currentBlock = useRef(0);

  const report = useCallback(
    (index: number, label?: string) => {
      currentBlock.current = index;
      const offset = starts[index] ?? 0;
      const fraction = offset / total;
      onProgress(fraction, label ?? t('novel.percent', { percent: Math.round(fraction * 100) }));
      if (data.nextChapterId && fraction >= PREFETCH_AT) {
        void queryClient.prefetchQuery(novelChapterQuery(data.titleId, data.nextChapterId));
      }
      if (finished.current) return;
      pending.current = offset;
      clearTimeout(saveTimer.current);
      saveTimer.current = setTimeout(() => remember(offset), SAVE_DELAY_MS);
    },
    [data.nextChapterId, data.titleId, onProgress, queryClient, remember, starts, t, total],
  );

  const finish = useCallback(() => {
    clearTimeout(saveTimer.current);
    if (!finished.current) {
      finished.current = true;
      remember(total);
    }
  }, [remember, total]);

  // Leaving the chapter saves where reading stopped.
  useEffect(
    () => () => {
      clearTimeout(saveTimer.current);
      if (pending.current !== null) remember(pending.current);
    },
    [remember],
  );

  // Where reading stood: the block holding the saved offset.
  const resumeAt = data.chapter.read ? 0 : blockAt(starts, data.chapter.lastPage ?? 0);

  // ---- Scroll mode ----
  useEffect(() => {
    if (prefs.paged) return;
    if (resumeAt > 0) refs.current[resumeAt]?.scrollIntoView?.({ block: 'start' });
    if (typeof IntersectionObserver === 'undefined') return;
    const visible = new Set<number>();
    const observer = new IntersectionObserver((entries) => {
      for (const entry of entries) {
        const target = entry.target as HTMLElement;
        if (target === endRef.current) {
          if (entry.isIntersecting) finish();
          continue;
        }
        const index = Number(target.dataset.block);
        if (entry.isIntersecting) visible.add(index);
        else visible.delete(index);
      }
      if (visible.size > 0) report(Math.min(...visible));
    });
    refs.current.forEach((element) => element && observer.observe(element));
    if (endRef.current) observer.observe(endRef.current);
    return () => observer.disconnect();
    // The resume point applies when the chapter or layout mode opens, not on every save.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [prefs.paged, chapterId]);

  // New typography reflows the text; keep the paragraph being read at the top.
  useLayoutEffect(() => {
    if (prefs.paged || currentBlock.current === 0) return;
    refs.current[currentBlock.current]?.scrollIntoView?.({ block: 'start' });
  }, [prefs.paged, prefs.fontSize, prefs.lineSpacing, prefs.font, prefs.width]);

  // ---- Paged mode ----
  const pageWidth = () => pagesRef.current?.clientWidth ?? 0;

  /** The first block shown on `target`. */
  const firstBlockOn = (target: number) => {
    const width = pageWidth();
    const container = pagesRef.current;
    if (!container || width === 0) return 0;
    const left = container.getBoundingClientRect().left - container.scrollLeft;
    const pageStart = target * width;
    const index = refs.current.findIndex(
      (element) => element && element.getBoundingClientRect().right - left > pageStart + 1,
    );
    return Math.max(index, 0);
  };

  const showPage = (target: number) => {
    const clamped = Math.max(0, Math.min(target, pageCount - 1));
    pagesRef.current?.scrollTo?.({ left: clamped * pageWidth() });
    setPage(clamped);
    const label = t('novel.pageOf', { page: clamped + 1, total: pageCount });
    if (clamped === pageCount - 1) {
      onProgress(1, label);
      finish();
    } else {
      report(firstBlockOn(clamped), label);
    }
  };

  useLayoutEffect(() => {
    if (!prefs.paged) return;
    const container = pagesRef.current;
    if (!container) return;
    const measure = () => {
      const width = container.clientWidth;
      const count = width > 0 ? Math.max(1, Math.round(container.scrollWidth / width)) : 1;
      setPageCount(count);
      return { width, count };
    };
    const { width, count } = measure();
    // Open on the page holding the resume point.
    const element = refs.current[resumeAt];
    const left = container.getBoundingClientRect().left;
    const start =
      element && width > 0
        ? Math.min(count - 1, Math.floor((element.getBoundingClientRect().left - left) / width))
        : 0;
    container.scrollTo?.({ left: start * width });
    setPage(start);
    onProgress(
      start / Math.max(count - 1, 1),
      t('novel.pageOf', { page: start + 1, total: count }),
    );
    window.addEventListener('resize', measure);
    return () => window.removeEventListener('resize', measure);
    // Re-measure when the layout changes, not on every page turn.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [prefs.paged, prefs.fontSize, prefs.lineSpacing, prefs.font, chapterId]);

  const forward = () => {
    if (page < pageCount - 1) {
      showPage(page + 1);
      return;
    }
    finish();
    onNext();
  };
  const back = () => (page <= 0 ? onPrevious() : showPage(page - 1));

  const onPageClick = (event: MouseEvent<HTMLDivElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    const x = (event.clientX - rect.left) / Math.max(rect.width, 1);
    if (x < 1 / 3) back();
    else if (x > 2 / 3) forward();
    else onToggleChrome();
  };

  const onKey = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'ArrowRight') forward();
    if (event.key === 'ArrowLeft') back();
  };

  const onTouchStart = (event: TouchEvent) => {
    swipeStart.current = event.touches[0]?.clientX ?? null;
  };
  const onTouchEnd = (event: TouchEvent) => {
    const start = swipeStart.current;
    const end = event.changedTouches[0]?.clientX;
    swipeStart.current = null;
    if (start === null || end === undefined || Math.abs(end - start) < SWIPE_PX) return;
    if (end < start) forward();
    else back();
  };

  const textStyle = {
    fontSize: `${prefs.fontSize}px`,
    lineHeight: prefs.lineSpacing / 10,
    fontFamily: FONTS[prefs.font],
  };
  const endCard = (
    <div
      ref={endRef}
      data-testid="novel-end"
      className="flex break-inside-avoid flex-col items-center gap-3 py-10 text-center"
    >
      <p className="text-muted text-sm">{t('manga.endOf', { name: data.chapter.name })}</p>
      <button
        type="button"
        onClick={(event) => {
          event.stopPropagation();
          onNext();
        }}
        className="bg-primary text-background rounded-full px-5 py-2.5 font-semibold"
      >
        {data.nextChapterId ? t('manga.nextChapter') : t('novel.backToBook')}
      </button>
    </div>
  );
  const blocks = data.blocks.map((block, index) => (
    <Block key={index} block={block} index={index} register={register} />
  ));

  if (prefs.paged) {
    return (
      <div
        ref={pagesRef}
        data-testid="novel-pages"
        role="presentation"
        tabIndex={0}
        onClick={onPageClick}
        onKeyDown={onKey}
        onTouchStart={onTouchStart}
        onTouchEnd={onTouchEnd}
        className={`h-dvh overflow-hidden outline-none ${chrome ? 'pt-16 pb-16' : 'pt-6 pb-6'}`}
      >
        <div
          style={{
            ...textStyle,
            height: '100%',
            columnWidth: `calc(100vw - ${PAGE_PADDING * 2}px)`,
            columnGap: `${PAGE_PADDING * 2}px`,
            columnFill: 'auto',
            padding: `0 ${PAGE_PADDING}px`,
          }}
        >
          {blocks}
          {endCard}
        </div>
      </div>
    );
  }
  return (
    <div
      data-testid="novel-scroll"
      role="presentation"
      onClick={onToggleChrome}
      className="px-5 pt-20 pb-24"
    >
      <article
        style={textStyle}
        className={`mx-auto ${prefs.width === 'wide' ? 'max-w-3xl' : 'max-w-xl'}`}
      >
        {blocks}
      </article>
      {endCard}
    </div>
  );
};

/** The novel reader: one chapter's text, scrolled or paged, with typography the reader sets. */
export const NovelReaderPage: FC = () => {
  const { t } = useTranslation();
  const { title, chapter } = useSearch({ from: '/novel/read' });
  const navigate = useNavigate({ from: '/novel/read' });
  const router = useRouter();
  const loaded = useNovelChapter(title, chapter);
  const settings = useSettings();
  const updateSetting = useUpdateSetting();
  const prefs = settings.data?.novelReader ?? DEFAULT_PREFS;
  const [chrome, setChrome] = useState(true);
  const [panel, setPanel] = useState(false);
  const [progress, setProgress] = useState({ fraction: 0, label: '' });
  const data = loaded.data;

  const go = (chapterId: string | null) => {
    setPanel(false);
    if (chapterId) void navigate({ search: { title, chapter: chapterId }, replace: true });
    else router.history.back();
  };
  const onProgress = useCallback(
    (fraction: number, label: string) => setProgress({ fraction, label }),
    [],
  );

  return (
    <section
      data-testid="novel-reader"
      className="bg-background text-foreground -mx-4 flex min-h-dvh flex-col lg:-mx-10"
    >
      {chrome && (
        <header className="bg-background/90 fixed inset-x-0 top-0 z-30 flex items-center gap-2 px-2 pt-[env(safe-area-inset-top)] backdrop-blur">
          <button
            type="button"
            aria-label={t('manga.closeReader')}
            onClick={() => router.history.back()}
            className="grid size-11 shrink-0 place-items-center"
          >
            <ArrowLeft aria-hidden size={22} />
          </button>
          <span className="flex min-w-0 flex-1 flex-col py-2">
            <span className="truncate text-sm font-semibold">{data?.title}</span>
            <span className="text-muted truncate text-xs">{data?.chapter.name}</span>
          </span>
          <button
            type="button"
            aria-label={t('novel.textSettings')}
            aria-expanded={panel}
            onClick={() => setPanel((open) => !open)}
            className="grid size-11 shrink-0 place-items-center"
          >
            <Type aria-hidden size={20} />
          </button>
        </header>
      )}
      {chrome && panel && (
        <PrefsPanel
          prefs={prefs}
          onChange={(next) => updateSetting.mutate({ key: 'novelReader', value: next })}
        />
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
      {data && (
        <ChapterView
          key={`${data.chapter.id}:${prefs.paged}`}
          data={data}
          prefs={prefs}
          chrome={chrome}
          onToggleChrome={() => {
            setPanel(false);
            setChrome((shown) => !shown);
          }}
          onNext={() => go(data.nextChapterId)}
          onPrevious={() => data.previousChapterId && go(data.previousChapterId)}
          onProgress={onProgress}
        />
      )}
      {chrome && data && (
        <footer className="bg-background/90 fixed inset-x-0 bottom-0 z-30 flex items-center gap-3 px-3 pt-2 pb-[max(env(safe-area-inset-bottom),0.5rem)] backdrop-blur">
          <button
            type="button"
            aria-label={t('manga.goPreviousChapter')}
            disabled={!data.previousChapterId}
            onClick={() => go(data.previousChapterId)}
            className="grid size-10 place-items-center rounded-full disabled:opacity-30"
          >
            <ChevronLeft aria-hidden size={22} />
          </button>
          <div className="flex flex-1 flex-col gap-1">
            <div className="bg-surface h-1 overflow-hidden rounded-full">
              <div
                className="bg-primary h-full"
                style={{ width: `${Math.round(progress.fraction * 100)}%` }}
              />
            </div>
            <span data-testid="novel-progress" className="text-muted text-center text-xs">
              {progress.label}
            </span>
          </div>
          <button
            type="button"
            aria-label={t('manga.goNextChapter')}
            disabled={!data.nextChapterId}
            onClick={() => go(data.nextChapterId)}
            className="grid size-10 place-items-center rounded-full disabled:opacity-30"
          >
            <ChevronRight aria-hidden size={22} />
          </button>
        </footer>
      )}
    </section>
  );
};
