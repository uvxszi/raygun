import { commands, events } from '@raygun/bindings';
import type {
  NovelChapter,
  NovelReaderPrefs,
  NovelTheme,
  PdfContentsEntry,
  TextBlock,
} from '@raygun/bindings';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate, useRouter, useSearch } from '@tanstack/react-router';
import {
  ArrowLeft,
  ChevronLeft,
  ChevronRight,
  FileImage,
  ListOrdered,
  ListTree,
  LoaderCircle,
  LocateFixed,
  Pause,
  Play,
  RotateCcw,
  SkipBack,
  SkipForward,
  SlidersHorizontal,
  Square,
  Type,
  Volume2,
} from 'lucide-react';
import {
  useCallback,
  useEffect,
  useEffectEvent,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import type { CSSProperties, FC, MouseEvent, PointerEvent, ReactNode, TouchEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { BookSearchSheet } from '../dictionary/BookSearchSheet';
import type { BookPlace } from '../dictionary/BookSearchSheet';
import { useDictionaryPrefs } from '../dictionary/prefs';
import { clearSelection, hasSelectionIn, useWordSelection } from '../dictionary/useWordSelection';
import type { WordSelection } from '../dictionary/useWordSelection';
import { WordLookup } from '../dictionary/WordLookup';
import { ReadingFontChoices } from '../fonts/ReadingFont';
import { READER_STACKS, quoteFamily, useInstalledFamily } from '../fonts/systemFonts';
import { unwrapResult } from '../lib/commandResult';
import { ChapterListSheet } from '../manga/reader/ChapterListSheet';
import { useSystemBars } from '../manga/reader/useSystemBars';
import { useWakeLock } from '../manga/reader/useWakeLock';
import { useReaderDisplay } from '../manga/readerDisplay';
import { ReaderSettingsLink } from '../manga/ReaderSettingsLink';
import { inSideScroller } from '../novel/code';
import { CodeBlock } from '../novel/CodeBlock';
import { MathFormula } from '../novel/MathFormula';
import { outlineBlock } from '../novel/pdfContents';
import { PdfContentsSheet } from '../novel/PdfContentsSheet';
import { PdfPagesView } from '../novel/PdfPagesView';
import { ReadAloudOptions } from '../novel/readAloud/ReadAloudOptions';
import { textOffsetAt } from '../novel/readAloud/textOffset';
import { useFollowReading } from '../novel/readAloud/useFollowReading';
import { TableBlock } from '../novel/TableBlock';
import { SpokenText, TextParts } from '../novel/TextParts';
import type { SpokenRange } from '../novel/TextParts';
import { ReaderAiButton } from '../readerAi/ReaderAi';
import { useNovelAi } from '../readerAi/useReaderAi';
import {
  DEFAULT_PREFS,
  INDENT,
  LINE_SPACING,
  NOVEL,
  TEXT_SIZE,
  blockAt,
  indentStyle,
  novelChapterQuery,
  saveNovelProgress,
  speechParagraphs,
  textOffsets,
  useNovelChapter,
  usePdfText,
  usePdfView,
} from '../novel/useNovel';
import { SizeField } from '../settings/prefs';
import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { useBackLayer } from '../shell/backLayers';
import { Sheet } from '../shell/Sheet';
import { SourceIssue } from '../shell/SourceIssue';

/** Progress is saved this long after reading stops moving. */
const SAVE_DELAY_MS = 800;
/** Past this share of a chapter, the next one is fetched ahead. */
const PREFETCH_AT = 0.6;
const SWIPE_PX = 50;
/** Holding a paragraph this long reads aloud from it. */
const LONG_PRESS_MS = 500;
/** A press that moves farther than this is a scroll, not a long press. */
const PRESS_SLOP_PX = 10;
/** Reading-aloud speeds the toolbar steps through, in tenths. */
const SPEEDS = [8, 10, 12, 15, 20] as const;
/** Side margin of a page in paged mode. */
const PAGE_PADDING = 20;
/** A page's widest, by the Width setting (M46.38): the scrolled column's width and a little more. */
const PAGE_WIDTH = { narrow: '46rem', wide: '64rem' } as const;

/** The page colours of each theme; the app's own follows the app's theme. */
const THEMES: Record<NovelTheme, { background: string; text: string } | null> = {
  app: null,
  light: { background: '#fbfaf6', text: '#1f1f1f' },
  sepia: { background: '#f4ecd8', text: '#5b4636' },
  dark: { background: '#1c1c1e', text: '#d6d6d6' },
  black: { background: '#000000', text: '#bdbdbd' },
};
const THEME_ORDER: NovelTheme[] = ['app', 'light', 'sepia', 'dark', 'black'];

/** Keys typed into fields or dialogs aren't the reader's. */
const ownKeys = (event: globalThis.KeyboardEvent) => {
  const target = event.target as HTMLElement | null;
  return !(
    target?.closest('input, select, textarea, [role="dialog"]') ||
    event.ctrlKey ||
    event.metaKey ||
    event.altKey
  );
};

/** A chapter not loaded yet has no blocks to translate. */
const NO_BLOCKS: TextBlock[] = [];

/** A language's name in the app's language (`de` → German), or nothing. */
const languageName = (tag: string | null, locale: string) => {
  if (!tag) return '';
  try {
    return new Intl.DisplayNames([locale], { type: 'language' }).of(tag) ?? tag;
  } catch {
    return tag;
  }
};

const Block: FC<{
  block: TextBlock;
  index: number;
  active: boolean;
  /** The sentence of the active paragraph being read aloud. */
  sentence?: SpokenRange;
  /** How far paragraphs start their first line in (M46.10), in tenths of an em; 0 = none. */
  indent: number;
  /** Lines run to both edges, words hyphenated. */
  justify: boolean;
  register: (index: number, element: HTMLElement | null) => void;
  /** The block's text as the AI layers translated it. */
  translated?: string;
}> = ({ block, index, active, sentence, indent, justify, register, translated }) => {
  const ref = (element: HTMLElement | null) => register(index, element);
  // The paragraph being read aloud.
  const highlight = active ? 'bg-primary/15 -mx-1 rounded px-1' : '';
  switch (block.kind) {
    case 'heading':
      return (
        <h2
          ref={ref}
          data-block={index}
          aria-current={active || undefined}
          className={`mt-[1.2em] mb-[0.8em] text-[1.3em] font-bold ${highlight}`}
        >
          {translated ?? <SpokenText text={block.text} sentence={active ? sentence : undefined} />}
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
    case 'math':
      return (
        <div
          ref={ref}
          data-block={index}
          aria-current={active || undefined}
          className={`my-[0.8em] break-inside-avoid ${highlight}`}
        >
          <MathFormula mathml={block.mathml} text={block.text} speech={block.speech} display />
        </div>
      );
    case 'code':
      return (
        <CodeBlock text={block.text} language={block.language} index={index} register={register} />
      );
    case 'table':
      return <TableBlock block={block} index={index} active={active} register={register} />;
    default: {
      const spoken = active ? sentence : undefined;
      // A list item's mark is drawn before it (bookText.css), never written into its text.
      const list = block.list;
      return (
        <p
          ref={ref}
          data-block={index}
          data-marker={list?.marker || undefined}
          aria-current={active || undefined}
          style={{
            ...(list
              ? ({ '--item-depth': Math.max(1, list.depth) } as CSSProperties)
              : indentStyle(indent)),
            ...(justify ? { textAlign: 'justify', hyphens: 'auto' } : {}),
          }}
          className={`mb-[0.9em] ${list ? 'book-item' : ''} ${highlight}`}
        >
          {translated ??
            (block.parts ? (
              <TextParts parts={block.parts} sentence={spoken} />
            ) : (
              <SpokenText text={block.text} sentence={spoken} />
            ))}
        </p>
      );
    }
  }
};

const choiceClass = (pressed: boolean) =>
  `flex-1 rounded-lg px-3 py-2 text-sm ${pressed ? 'bg-primary/20 text-primary font-semibold' : 'bg-background'}`;

const Choice: FC<{ pressed: boolean; onClick: () => void; children: ReactNode }> = ({
  pressed,
  onClick,
  children,
}) => (
  <button type="button" aria-pressed={pressed} onClick={onClick} className={choiceClass(pressed)}>
    {children}
  </button>
);

/** The reader's options: the text's, or reading aloud's. */
type PanelTab = 'text' | 'readAloud';

const PrefsPanel: FC<{
  prefs: NovelReaderPrefs;
  onChange: (prefs: NovelReaderPrefs) => void;
  /** Reading aloud is there: its options get a tab. */
  speech: boolean;
  tab: PanelTab;
  onTab: (tab: PanelTab) => void;
  titleId: string;
  /** The language being read aloud now. */
  readingLanguage: string | null;
}> = ({ prefs, onChange, speech, tab, onTab, titleId, readingLanguage }) => {
  const { t } = useTranslation();
  const set = (patch: Partial<NovelReaderPrefs>) => onChange({ ...prefs, ...patch });
  const shown = speech ? tab : 'text';
  return (
    <div
      role="dialog"
      aria-label={t('novel.textSettings')}
      className="bg-surface fixed top-[calc(var(--app-bar,0px)+4rem+var(--safe-top))] right-[calc(var(--main-right,0px)+max(0.5rem,var(--safe-right)))] left-[calc(var(--main-left,0px)+max(0.5rem,var(--safe-left)))] z-40 mx-auto flex max-h-[calc(100dvh-var(--app-bar,0px)-5rem-var(--safe-top)-var(--safe-bottom))] max-w-md flex-col gap-3 overflow-y-auto overscroll-contain rounded-xl p-4 shadow-xl"
    >
      {speech && (
        <div role="tablist" aria-label={t('novel.textSettings')} className="flex gap-2">
          {(['text', 'readAloud'] as const).map((value) => (
            <button
              key={value}
              type="button"
              role="tab"
              aria-selected={shown === value}
              onClick={() => onTab(value)}
              className={choiceClass(shown === value)}
            >
              {t(`novel.panelTabs.${value}`)}
            </button>
          ))}
        </div>
      )}
      {shown === 'readAloud' ? (
        <ReadAloudOptions titleId={titleId} readingLanguage={readingLanguage} />
      ) : (
        <TextPrefs prefs={prefs} set={set} />
      )}
    </div>
  );
};

/** The text's options: size, spacing, typeface, width, indent, alignment, colours, layout. */
const TextPrefs: FC<{
  prefs: NovelReaderPrefs;
  set: (patch: Partial<NovelReaderPrefs>) => void;
}> = ({ prefs, set }) => {
  const { t } = useTranslation();
  return (
    <>
      <SizeField
        title={t('novel.fontSize')}
        unit="px"
        {...TEXT_SIZE}
        value={prefs.textSize ?? 0}
        onChange={(textSize) => set({ textSize })}
      />
      <SizeField
        title={t('novel.lineSpacing')}
        unit=""
        {...LINE_SPACING}
        zero={DEFAULT_PREFS.lineSpacing}
        factor={0.1}
        value={prefs.lineSpacing}
        onChange={(lineSpacing) => set({ lineSpacing })}
      />
      <ReadingFontChoices buttonClass={choiceClass} />
      <div className="flex gap-2">
        <Choice pressed={prefs.width === 'narrow'} onClick={() => set({ width: 'narrow' })}>
          {t('novel.narrow')}
        </Choice>
        <Choice pressed={prefs.width === 'wide'} onClick={() => set({ width: 'wide' })}>
          {t('novel.wide')}
        </Choice>
      </div>
      <SizeField
        title={t('novel.indent')}
        unit="em"
        {...INDENT}
        factor={0.1}
        value={prefs.indent ?? 0}
        onChange={(indent) => set({ indent })}
      />
      <div className="flex gap-2">
        <Choice pressed={!prefs.justify} onClick={() => set({ justify: false })}>
          {t('novel.alignStart')}
        </Choice>
        <Choice pressed={prefs.justify ?? false} onClick={() => set({ justify: true })}>
          {t('novel.justify')}
        </Choice>
      </div>
      <div role="radiogroup" aria-label={t('novel.theme')} className="flex gap-2">
        {THEME_ORDER.map((theme) => {
          const colors = THEMES[theme];
          const chosen = (prefs.theme ?? 'app') === theme;
          return (
            <button
              key={theme}
              type="button"
              role="radio"
              aria-checked={chosen}
              aria-label={t(`novel.themes.${theme}`)}
              title={t(`novel.themes.${theme}`)}
              onClick={() => set({ theme })}
              style={
                colors ? { backgroundColor: colors.background, color: colors.text } : undefined
              }
              className={`grid h-10 flex-1 place-items-center rounded-lg border text-sm font-semibold ${colors ? '' : 'bg-background text-foreground'} ${chosen ? 'border-primary ring-primary ring-2' : 'border-muted/40'}`}
            >
              Aa
            </button>
          );
        })}
      </div>
      <div className="flex gap-2">
        <Choice pressed={!prefs.paged} onClick={() => set({ paged: false })}>
          {t('novel.scroll')}
        </Choice>
        <Choice pressed={prefs.paged} onClick={() => set({ paged: true })}>
          {t('novel.paged')}
        </Choice>
      </div>
      <ReaderSettingsLink />
    </>
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
  /** The block reading is at (where reading aloud starts). */
  onBlock: (index: number) => void;
  /** The block being read aloud. */
  speaking: number | null;
  /** The sentence of that block being read aloud. */
  sentence?: SpokenRange;
  /**
   * Reads aloud from a block (a long press, a tap while reading, Read aloud from here), from the
   * sentence at `offset` in its text when the place is known.
   */
  onSpeakFrom?: (index: number, offset?: number | null) => void;
  /** Blocks' texts as the AI layers translated them, by index. */
  translated: Map<number, string>;
  /**
   * A PDF's text: the block each page starts at. Progress is then the page reached, as in its
   * page view.
   */
  pageStarts?: number[];
  /**
   * A block to bring into view (a bookmark picked in the contents), where the chapter opens when
   * given as it opens; again whenever `nonce` changes.
   */
  goTo?: { block: number; nonce: number } | null;
  /** Called once `goTo` is shown. */
  onGoToShown?: () => void;
}> = ({
  data,
  prefs,
  chrome,
  onToggleChrome,
  onNext,
  onPrevious,
  onProgress,
  onBlock,
  speaking,
  sentence,
  onSpeakFrom,
  translated,
  pageStarts,
  goTo,
  onGoToShown,
}) => {
  const { t, i18n } = useTranslation();
  const queryClient = useQueryClient();
  // Word lookups (Settings → Reader → Dictionary): the text is selectable, and a selection is
  // looked up; a long press then selects a word instead of reading aloud from the paragraph.
  const lookups = useDictionaryPrefs().enabled;
  const { block: askedBlock, page: askedPage } = useSearch({ from: '/novel/read' });
  const navigate = useNavigate({ from: '/novel/read' });
  const scrollRef = useRef<HTMLDivElement>(null);
  const [lookup, setLookup] = useState<WordSelection | null>(null);
  const [searchFor, setSearchFor] = useState<string | null>(null);
  // Where each block is as progress counts it: a character offset, or a PDF's page.
  const { starts, total } = useMemo(() => {
    if (!pageStarts) return textOffsets(data.blocks);
    const pageOf = data.blocks.map((_, index) => blockAt(pageStarts, index));
    return { starts: pageOf, total: Math.max(pageStarts.length, 1) };
  }, [data.blocks, pageStarts]);
  const refs = useRef<(HTMLElement | null)[]>([]);
  const endRef = useRef<HTMLDivElement>(null);
  const pagesRef = useRef<HTMLDivElement>(null);
  const saveTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const finished = useRef(data.chapter.read);
  const [page, setPage] = useState(0);
  const [pageCount, setPageCount] = useState(1);
  const swipeStart = useRef<number | null>(null);
  const chapterId = data.chapter.id;
  // A font of the device's in front of the reader's own (Settings → Reader → Typeface).
  const deviceFont = useInstalledFamily(useSettings().data?.fonts?.reading);
  const register = useCallback((index: number, element: HTMLElement | null) => {
    refs.current[index] = element;
  }, []);

  /** The offset not yet saved, written when the timer fires or the chapter closes. */
  const pending = useRef<number | null>(null);

  // Saves progress and keeps the cached chapter in step, so reopening it resumes here.
  const remember = useCallback(
    (offset: number) => {
      pending.current = null;
      // The title page and Continue reading ask again once it's saved, not before.
      void saveNovelProgress(chapterId, offset, total)
        .then(() =>
          Promise.all([
            queryClient.invalidateQueries({ queryKey: [NOVEL, 'title'] }),
            queryClient.invalidateQueries({ queryKey: [NOVEL, 'library'] }),
          ]),
        )
        .catch(() => {});
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
    },
    [chapterId, data.titleId, queryClient, total],
  );

  /** The block reading is at, to keep in view when the text reflows. */
  const currentBlock = useRef(0);

  const report = useCallback(
    (index: number, label?: string) => {
      currentBlock.current = index;
      onBlock(index);
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
    [
      data.nextChapterId,
      data.titleId,
      onBlock,
      onProgress,
      queryClient,
      remember,
      starts,
      t,
      total,
    ],
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

  // Where reading stood: the block holding the saved offset (a PDF's: its page's first block);
  // a search in the book opens a chapter at the paragraph (or the PDF page) it found.
  const saved = data.chapter.lastPage ?? 0;
  const searched =
    askedBlock !== undefined && askedBlock < data.blocks.length
      ? askedBlock
      : pageStarts && askedPage !== undefined
        ? pageStarts[Math.min(askedPage, pageStarts.length - 1)]
        : undefined;
  const resumeAt =
    goTo?.block ??
    searched ??
    (data.chapter.read
      ? 0
      : pageStarts
        ? (pageStarts[Math.min(saved, pageStarts.length - 1)] ?? 0)
        : blockAt(starts, saved));

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
  }, [
    prefs.paged,
    prefs.textSize,
    prefs.lineSpacing,
    prefs.font,
    deviceFont,
    prefs.width,
    prefs.indent,
    prefs.justify,
  ]);

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
    /** Counts the pages and shows the one holding `block`. */
    const place = (block: number) => {
      const width = container.clientWidth;
      const count = width > 0 ? Math.max(1, Math.round(container.scrollWidth / width)) : 1;
      setPageCount(count);
      const element = refs.current[block];
      // Where the text starts, scrolled or not.
      const left = container.getBoundingClientRect().left - container.scrollLeft;
      const start =
        element && width > 0
          ? Math.min(
              count - 1,
              Math.max(0, Math.floor((element.getBoundingClientRect().left - left) / width)),
            )
          : 0;
      container.scrollTo?.({ left: start * width });
      setPage(start);
      return { start, count };
    };
    // Open on the page holding the resume point; new typography keeps the paragraph being read.
    const { start, count } = place(currentBlock.current > 0 ? currentBlock.current : resumeAt);
    onProgress(
      start / Math.max(count - 1, 1),
      t('novel.pageOf', { page: start + 1, total: count }),
    );
    // A turned phone or a resized window lays the pages out again around the same paragraph, and
    // so does a device font arriving after the text was laid out in its fallback.
    const onResize = () => place(currentBlock.current);
    window.addEventListener('resize', onResize);
    document.fonts?.addEventListener?.('loadingdone', onResize);
    return () => {
      window.removeEventListener('resize', onResize);
      document.fonts?.removeEventListener?.('loadingdone', onResize);
    };
    // Re-measure when the layout changes, not on every page turn.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    prefs.paged,
    prefs.textSize,
    prefs.lineSpacing,
    prefs.font,
    deviceFont,
    prefs.width,
    prefs.indent,
    prefs.justify,
    chapterId,
  ]);

  /** The page an element is on (paged mode). */
  const pageOf = (element: HTMLElement) => {
    const container = pagesRef.current;
    const width = pageWidth();
    if (!container || width === 0) return null;
    const left = container.getBoundingClientRect().left - container.scrollLeft;
    return Math.floor((element.getBoundingClientRect().left - left) / width);
  };

  // Keeps the sentence being read aloud in view: the column glides to it (Android's WebView drops
  // smooth `scrollIntoView` in the app's scrolling container), or its page is turned to; the
  // reader's own scrolling or page turns hold that a while.
  const scroller = useCallback(() => scrollRef.current?.closest('main') ?? null, []);
  const following = useFollowReading({
    paged: prefs.paged,
    speaking,
    sentenceStart: sentence?.start,
    blockElement: (index) => refs.current[index] ?? null,
    scroller,
    showPageOf: (element) => {
      const target = pageOf(element);
      if (target !== null && target !== page) showPage(target);
    },
  });

  const forward = () => {
    following.hold();
    if (page < pageCount - 1) {
      showPage(page + 1);
      return;
    }
    finish();
    onNext();
  };
  const back = () => {
    following.hold();
    if (page <= 0) onPrevious();
    else showPage(page - 1);
  };

  // ---- Choosing where reading aloud starts ----
  const press = useRef<{ x: number; y: number; timer: ReturnType<typeof setTimeout> } | null>(null);
  /** Set after a long press, so the click that ends it does nothing else. */
  const longPressed = useRef(false);

  const blockOf = (target: EventTarget | null) => {
    const element = (target as HTMLElement | null)?.closest?.('[data-block]');
    const index = element ? Number((element as HTMLElement).dataset.block) : NaN;
    const block = data.blocks[index];
    return block &&
      (block.kind === 'paragraph' ||
        block.kind === 'heading' ||
        block.kind === 'math' ||
        block.kind === 'table')
      ? index
      : null;
  };

  const cancelPress = () => {
    if (press.current) clearTimeout(press.current.timer);
    press.current = null;
  };

  /**
   * Where in block `index`'s text a point on screen is, while it shows its own text (a table is
   * read row by row, in words of its own: from its start).
   */
  const offsetAt = (index: number, x: number, y: number) => {
    const element = refs.current[index];
    return element && !translated.has(index) && data.blocks[index]?.kind !== 'table'
      ? textOffsetAt(element, x, y)
      : null;
  };

  /** Whether an event happened on the text, not in a lookup drawn over it. */
  const fromText = (event: { currentTarget: EventTarget; target: EventTarget }) =>
    (event.currentTarget as Node).contains(event.target as Node);

  const pressHandlers = onSpeakFrom
    ? {
        onPointerDown: (event: PointerEvent<HTMLElement>) => {
          cancelPress();
          const index = blockOf(event.target);
          if (index === null) return;
          const { clientX: x, clientY: y } = event;
          press.current = {
            x,
            y,
            timer: setTimeout(() => {
              press.current = null;
              longPressed.current = true;
              // From the sentence held down.
              onSpeakFrom(index, offsetAt(index, x, y));
            }, LONG_PRESS_MS),
          };
        },
        onPointerMove: (event: PointerEvent<HTMLElement>) => {
          const start = press.current;
          if (
            start &&
            Math.hypot(event.clientX - start.x, event.clientY - start.y) > PRESS_SLOP_PX
          ) {
            cancelPress();
          }
        },
        onPointerUp: cancelPress,
        onPointerCancel: cancelPress,
        // The system menu a long press opens on text would cover the reader.
        onContextMenu: (event: MouseEvent<HTMLElement>) => event.preventDefault(),
      }
    : {};
  // With lookups on, a long press is the system's: it selects a word (Read aloud from here is in
  // the lookup), and the menu it would open over the text is the lookup's own.
  const textHandlers = lookups
    ? {
        onContextMenu: (event: MouseEvent<HTMLElement>) => {
          if (fromText(event)) event.preventDefault();
        },
      }
    : pressHandlers;

  useEffect(() => () => cancelPress(), []);

  /** The element the reader's text is selected in. */
  const selectionRoot = () => (prefs.paged ? pagesRef.current : scrollRef.current);
  const forgetSelection = useWordSelection(prefs.paged ? pagesRef : scrollRef, lookups, setLookup);
  const closeLookup = useCallback(() => {
    setLookup(null);
    clearSelection();
    forgetSelection();
  }, [forgetSelection]);

  /** Swallows the click ending a long press, closing a lookup or ending a selection. */
  const handledTap = (event: MouseEvent<HTMLElement>) => {
    // The lookup and its sheets are in the text's tree but drawn elsewhere: not the reader's taps.
    if (!fromText(event)) return true;
    if (lookups) {
      // A tap closes a lookup, or ends a selection (a drag of the mouse ends in a click too),
      // and does nothing else; a double click selects a word.
      if (lookup) {
        closeLookup();
        return true;
      }
      if (hasSelectionIn(selectionRoot()) || event.detail > 1) return true;
    }
    if (longPressed.current) {
      longPressed.current = false;
      return true;
    }
    return false;
  };

  /**
   * While reading aloud, a tap on the text reads from the sentence tapped; one on the sentence
   * being read shows or hides the bars, as a tap does otherwise. Whether it read from there.
   */
  const jumpTo = (event: MouseEvent<HTMLElement>) => {
    if (speaking === null || !onSpeakFrom) return false;
    const index = blockOf(event.target);
    if (index === null) return false;
    const offset = offsetAt(index, event.clientX, event.clientY);
    const onSentence =
      index === speaking &&
      sentence !== undefined &&
      offset !== null &&
      offset >= sentence.start &&
      offset < sentence.end;
    if (onSentence) return false;
    onSpeakFrom(index, offset);
    return true;
  };

  // Pages turn from the sides while reading aloud too; the middle reads from the sentence tapped.
  const onPageClick = (event: MouseEvent<HTMLDivElement>) => {
    if (handledTap(event)) return;
    const rect = event.currentTarget.getBoundingClientRect();
    const x = (event.clientX - rect.left) / Math.max(rect.width, 1);
    if (x < 1 / 3) back();
    else if (x > 2 / 3) forward();
    else if (!jumpTo(event)) onToggleChrome();
  };

  // Keys: arrows, Space and Page Up/Down turn pages, or scroll the column a screen at a time.
  const onKey = useEffectEvent((event: globalThis.KeyboardEvent) => {
    if (!ownKeys(event)) return;
    const ahead = event.key === 'PageDown' || (event.key === ' ' && !event.shiftKey);
    const behind = event.key === 'PageUp' || (event.key === ' ' && event.shiftKey);
    if (prefs.paged) {
      if (event.key === 'ArrowRight' || ahead) forward();
      else if (event.key === 'ArrowLeft' || behind) back();
      else return;
    } else {
      const scroller = endRef.current?.closest('main');
      if (!scroller || !(ahead || behind)) return;
      scroller.scrollBy?.({ top: (ahead ? 1 : -1) * scroller.clientHeight * 0.85 });
    }
    event.preventDefault();
  });
  useEffect(() => {
    const listener = (event: globalThis.KeyboardEvent) => onKey(event);
    window.addEventListener('keydown', listener);
    return () => window.removeEventListener('keydown', listener);
  }, []);

  const onTouchStart = (event: TouchEvent) => {
    // Code and wide formulas scroll sideways under the finger instead.
    swipeStart.current =
      fromText(event) && !inSideScroller(event.target) ? (event.touches[0]?.clientX ?? null) : null;
  };
  const onTouchEnd = (event: TouchEvent) => {
    const start = swipeStart.current;
    const end = event.changedTouches[0]?.clientX;
    swipeStart.current = null;
    if (start === null || end === undefined || Math.abs(end - start) < SWIPE_PX) return;
    // A selection's handles being dragged aren't a page turn.
    if (lookups && hasSelectionIn(selectionRoot())) return;
    if (end < start) forward();
    else back();
  };

  const textStyle = {
    // The screen's own size (rem follows the screen, M46.48) and the reader's text size on it;
    // wide windows sit further away than a phone, so text is a fifth larger there (M46.6). The
    // app's text size (`--text-scale`) comes on top.
    fontSize: `calc((1.125rem + ${prefs.textSize ?? 0}px) * var(--reader-scale, 1) * var(--text-scale, 1))`,
    lineHeight: prefs.lineSpacing / 10,
    fontFamily: deviceFont
      ? `${quoteFamily(deviceFont)}, ${READER_STACKS[prefs.font]}`
      : READER_STACKS[prefs.font],
  };
  const endCard = (
    <div
      ref={endRef}
      data-testid="novel-end"
      className="book-text flex break-inside-avoid flex-col items-center gap-3 py-10 text-center"
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
  /** Brings a block into view: scrolled to, or its page turned to. */
  const showBlock = (index: number) => {
    const element = refs.current[index];
    if (!element) return;
    if (!prefs.paged) {
      element.scrollIntoView?.({ block: 'center' });
      return;
    }
    const container = pagesRef.current;
    const width = pageWidth();
    if (!container || width === 0) return;
    const left = container.getBoundingClientRect().left - container.scrollLeft;
    showPage(Math.floor((element.getBoundingClientRect().left - left) / width));
  };

  // A bookmark picked in the contents, once per pick (one the chapter opened with is where it
  // opened, `resumeAt`); then it's forgotten.
  const shownGoTo = useRef(goTo?.nonce ?? null);
  const onGoTo = useEffectEvent((block: number | null) => {
    if (block !== null) showBlock(block);
    onGoToShown?.();
  });
  useEffect(() => {
    if (!goTo) return;
    const fresh = shownGoTo.current !== goTo.nonce;
    shownGoTo.current = goTo.nonce;
    onGoTo(fresh ? goTo.block : null);
  }, [goTo]);

  /** A place a search in the book found: in this chapter, or another opened there. */
  const openPlace = (place: BookPlace) => {
    setSearchFor(null);
    if (place.chapterId === data.chapter.id) {
      showBlock('block' in place ? place.block : (pageStarts?.[place.page] ?? 0));
      return;
    }
    void navigate({
      search: {
        title: data.titleId,
        chapter: place.chapterId,
        ...('block' in place ? { block: place.block } : { page: place.page }),
      },
      replace: true,
    });
  };

  // Drawn over the page (portals); its taps and swipes aren't the text's (`fromText`).
  const lookupLayer = (
    <>
      {lookup && (
        <WordLookup
          key={`${lookup.block}:${lookup.text}`}
          selection={lookup}
          titleId={data.titleId}
          appLanguage={i18n.resolvedLanguage ?? i18n.language ?? 'en'}
          onClose={closeLookup}
          onSearchBook={(text) => {
            closeLookup();
            setSearchFor(text);
          }}
          onReadAloud={
            onSpeakFrom && lookup.block !== null
              ? () => {
                  const { block, offset } = lookup;
                  closeLookup();
                  // From the sentence the selection starts in (a table from its start).
                  if (block !== null)
                    onSpeakFrom(
                      block,
                      translated.has(block) || data.blocks[block]?.kind === 'table'
                        ? null
                        : (offset ?? null),
                    );
                }
              : undefined
          }
        />
      )}
      {searchFor !== null && (
        <BookSearchSheet
          titleId={data.titleId}
          chapterId={data.chapter.id}
          initial={searchFor}
          pdf={pageStarts !== undefined}
          onPick={openPlace}
          onClose={() => setSearchFor(null)}
        />
      )}
    </>
  );
  const selectable = lookups ? 'selectable' : 'select-none';
  // Shown while the reader moved away from what's being read.
  const backToReading = following.held && (
    <div className="pointer-events-none fixed top-[calc(var(--app-bar,0px)+4.75rem+var(--safe-top))] right-[var(--main-right,0px)] left-[var(--main-left,0px)] z-30 flex justify-center">
      <button
        type="button"
        // Drawn over the text, in its tree: not a tap on the text.
        onClick={(event) => {
          event.stopPropagation();
          following.backToReading();
        }}
        onPointerDown={(event) => event.stopPropagation()}
        onTouchStart={(event) => event.stopPropagation()}
        onTouchEnd={(event) => event.stopPropagation()}
        className="bg-primary text-background pointer-events-auto flex items-center gap-2 rounded-full px-4 py-2 text-sm font-semibold shadow-lg"
      >
        <LocateFixed aria-hidden size={16} />
        {t('novel.backToReading')}
      </button>
    </div>
  );

  const blocks = data.blocks.map((block, index) => (
    <Block
      key={index}
      block={block}
      index={index}
      active={index === speaking}
      sentence={index === speaking ? sentence : undefined}
      indent={prefs.indent ?? 0}
      justify={prefs.justify ?? false}
      register={register}
      translated={translated.get(index)}
    />
  ));

  if (prefs.paged) {
    return (
      <div
        ref={pagesRef}
        data-testid="novel-pages"
        role="presentation"
        onClick={onPageClick}
        onTouchStart={onTouchStart}
        onTouchEnd={onTouchEnd}
        {...textHandlers}
        // Pages as wide as the column the scrolled text reads in (M46.38), not the whole window.
        style={{ maxWidth: PAGE_WIDTH[prefs.width] }}
        className={`mx-auto h-[calc(100dvh-var(--app-bar,0px))] overflow-hidden outline-none ${selectable} [-webkit-touch-callout:none] lg:[--reader-scale:1.2] ${chrome ? 'pt-16 pb-16' : 'pt-6 pb-6'}`}
      >
        <div
          className="book-text book-pages"
          style={{
            ...textStyle,
            height: '100%',
            columnWidth: `calc(min(var(--main-width, 100vw), ${PAGE_WIDTH[prefs.width]}) - ${PAGE_PADDING * 2}px)`,
            columnGap: `${PAGE_PADDING * 2}px`,
            columnFill: 'auto',
            padding: `0 ${PAGE_PADDING}px`,
          }}
        >
          {blocks}
          {endCard}
        </div>
        {lookupLayer}
        {backToReading}
      </div>
    );
  }
  return (
    <div
      ref={scrollRef}
      data-testid="novel-scroll"
      role="presentation"
      onClick={(event) => {
        if (!handledTap(event) && !jumpTo(event)) onToggleChrome();
      }}
      {...textHandlers}
      // Never wider than the page: what can't wrap scrolls in its own box (bookText.css).
      className={`overflow-x-clip px-5 pt-20 pb-24 ${selectable} [-webkit-touch-callout:none] lg:[--reader-scale:1.2]`}
    >
      <article
        style={textStyle}
        className={`book-text mx-auto ${prefs.width === 'wide' ? 'max-w-3xl lg:max-w-4xl' : 'max-w-xl lg:max-w-2xl'}`}
      >
        {blocks}
      </article>
      {endCard}
      {lookupLayer}
      {backToReading}
    </div>
  );
};

/** The novel reader: one chapter's text, scrolled or paged, with typography the reader sets. */
export const NovelReaderPage: FC = () => {
  const { t, i18n } = useTranslation();
  const { title, chapter, page: askedPage } = useSearch({ from: '/novel/read' });
  const navigate = useNavigate({ from: '/novel/read' });
  const router = useRouter();
  const loaded = useNovelChapter(title, chapter);
  // A PDF chapter reads as its pages, or as the text read from them.
  const pdf = loaded.data?.pdf ?? null;
  const [pdfView, setPdfView] = usePdfView(title);
  const pdfText = usePdfText(title, chapter, pdf !== null && pdfView === 'text');
  const textChapter = useMemo(
    () =>
      loaded.data && pdfText.data ? { ...loaded.data, blocks: pdfText.data.blocks } : undefined,
    [loaded.data, pdfText.data],
  );
  const settings = useSettings();
  const updateSetting = useUpdateSetting();
  const prefs = settings.data?.novelReader ?? DEFAULT_PREFS;
  // Ctrl+scroll and Ctrl+plus/minus make the text bigger or smaller, as zooming would (M46.7).
  const resize = (delta: number) => {
    const current = prefs.textSize ?? 0;
    const textSize = Math.min(TEXT_SIZE.max, Math.max(TEXT_SIZE.min, current + delta));
    if (textSize !== current)
      updateSetting.mutate({ key: 'novelReader', value: { ...prefs, textSize } });
  };
  const onZoomWheel = useEffectEvent((event: WheelEvent) => {
    if (!event.ctrlKey) return;
    event.preventDefault();
    resize(event.deltaY < 0 ? 2 : -2);
  });
  const onZoomKey = useEffectEvent((event: globalThis.KeyboardEvent) => {
    if (!event.ctrlKey && !event.metaKey) return;
    if (event.key === '=' || event.key === '+') resize(2);
    else if (event.key === '-') resize(-2);
    else if (event.key === '0') resize(-(prefs.textSize ?? 0));
    else return;
    event.preventDefault();
  });
  useEffect(() => {
    const wheel = (event: WheelEvent) => onZoomWheel(event);
    const key = (event: globalThis.KeyboardEvent) => onZoomKey(event);
    window.addEventListener('wheel', wheel, { passive: false });
    window.addEventListener('keydown', key);
    return () => {
      window.removeEventListener('wheel', wheel);
      window.removeEventListener('keydown', key);
    };
  }, []);
  const [chrome, setChrome] = useState(true);
  const [panel, setPanel] = useState(false);
  const [panelTab, setPanelTab] = useState<PanelTab>('text');
  // Back closes the text settings before it leaves the reader.
  useBackLayer(chrome && panel, () => setPanel(false));
  /** The reading-aloud bar's options button while its options are open. */
  const [speechOptions, setSpeechOptions] = useState<HTMLElement | null>(null);
  /** The chapter list's button while the list is open. */
  const [chapterList, setChapterList] = useState<HTMLElement | null>(null);
  /** A PDF book's contents: their button while they're open, and the page then being read. */
  const [contents, setContents] = useState<{ anchor: HTMLElement; pageNumber: number } | null>(
    null,
  );
  /** A bookmark picked in a PDF book's contents, until its chapter's text shows it. */
  const [bookmark, setBookmark] = useState<{
    chapterId: string;
    page: number;
    title: string;
    nonce: number;
  } | null>(null);
  const forgetBookmark = useCallback(() => setBookmark(null), []);
  const [progress, setProgress] = useState({ fraction: 0, label: '' });
  const [speechNotice, setSpeechNotice] = useState<string | null>(null);
  const data = pdf ? textChapter : loaded.data;
  // Where the bookmark leads in the text, once its chapter's text is here.
  const bookmarkBlock = useMemo(() => {
    const pageStarts = pdfText.data?.pageStarts;
    if (!bookmark || !data || !pageStarts || bookmark.chapterId !== data.chapter.id) return null;
    return {
      block: outlineBlock(data.blocks, pageStarts, bookmark.page, bookmark.title),
      nonce: bookmark.nonce,
    };
  }, [bookmark, data, pdfText.data]);
  const error = loaded.error ?? (pdf ? pdfText.error : null);
  const theme = THEMES[prefs.theme ?? 'app'];
  useWakeLock((prefs.keepScreenOn ?? true) && data !== undefined);
  // The comics reader's Full screen setting holds for books too.
  const fullScreen = useReaderDisplay().display.fullScreen;
  useSystemBars(fullScreen && !chrome && data !== undefined);
  useEffect(() => {
    if (!speechNotice) return;
    const timer = setTimeout(() => setSpeechNotice(null), 4000);
    return () => clearTimeout(timer);
  }, [speechNotice]);

  // ---- Reading aloud ----
  const speechAvailable = useQuery({
    queryKey: ['tts', 'available'],
    queryFn: () => unwrapResult(commands.ttsAvailable()),
    staleTime: Infinity,
  });
  const [reading, setReading] = useState(false);
  /** Reading aloud paused on a sentence; Resume goes on from it. */
  const [paused, setPaused] = useState(false);
  const [speaking, setSpeaking] = useState<number | null>(null);
  const [sentence, setSentence] = useState<SpokenRange | undefined>(undefined);
  const position = useRef(0);
  // The AI layers' translation, from where the chapter was left.
  const resumeBlock = useMemo(
    () =>
      data && !data.chapter.read
        ? blockAt(textOffsets(data.blocks).starts, data.chapter.lastPage ?? 0)
        : 0,
    [data],
  );
  const novelAi = useNovelAi(data?.chapter.id ?? '', data?.blocks ?? NO_BLOCKS, resumeBlock);
  const readingRef = useRef(false);
  const dataRef = useRef(data);
  // Speed, pitch, volume and voice are settings Rust applies to the reading as they change.
  const rate = prefs.ttsRate ?? 10;
  /** The language being read, as reading aloud reports it. */
  const [readingLanguage, setReadingLanguage] = useState<string | null>(null);
  /** Set when a chapter ended aloud: the next one starts reading once it loads. */
  const continueAloud = useRef(false);
  /** Why another voice reads than the one set: each reason is said once per start. */
  const noticeShown = useRef<string | null>(null);
  useEffect(() => {
    dataRef.current = data;
  });

  /**
   * Reads `chapter` aloud from block `first` (from the sentence at `offset` in its text, when a
   * place was picked). With `resume`, where the chapter was left aloud goes first when it's
   * around there (Rust keeps it, to the sentence and word).
   */
  const speakFrom = useCallback(
    (chapter: NovelChapter, first: number, resume: boolean, offset?: number | null) => {
      const paragraphs = speechParagraphs(chapter.blocks, 0);
      if (!paragraphs.some((paragraph) => paragraph.id >= first)) return;
      noticeShown.current = null;
      setPaused(false);
      void unwrapResult(
        commands.ttsSpeak({
          chapterId: chapter.chapter.id,
          titleId: chapter.titleId,
          title: chapter.title,
          chapter: chapter.chapter.name,
          paragraphs,
          from: first,
          ...(offset === null || offset === undefined ? {} : { offset }),
          resume,
        }),
      ).catch((error: unknown) => {
        readingRef.current = false;
        setReading(false);
        setSpeaking(null);
        setSentence(undefined);
        setSpeechNotice(error instanceof Error ? error.message : String(error));
      });
    },
    [],
  );

  /** Reading aloud's state on screen goes; Rust's session is left as it is. */
  const readingEnded = useCallback(() => {
    readingRef.current = false;
    continueAloud.current = false;
    setReading(false);
    setPaused(false);
    setSpeaking(null);
    setSentence(undefined);
  }, []);

  const stopReading = useCallback(() => {
    readingEnded();
    void commands.ttsStop();
  }, [readingEnded]);

  const toggleReading = () => {
    if (!data) return;
    if (readingRef.current) {
      stopReading();
      return;
    }
    readingRef.current = true;
    setReading(true);
    speakFrom(data, position.current, true);
  };

  /** Pauses on the sentence being read, or goes on from it. */
  const togglePause = () => {
    setPaused(!paused);
    void (paused ? commands.ttsResume() : commands.ttsPause());
  };

  /** Reads aloud from a chosen block, from the sentence at `offset` in it when it's known. */
  const readFrom = useCallback(
    (index: number, offset?: number | null) => {
      const chapter = dataRef.current;
      if (!chapter) return;
      readingRef.current = true;
      continueAloud.current = false;
      setReading(true);
      setSpeaking(index);
      setSentence(undefined);
      position.current = index;
      speakFrom(chapter, index, false, offset);
    },
    [speakFrom],
  );

  /** Reads from the paragraph `step` places from the one being read (0: its start again). */
  const stepTo = (step: number) => {
    setPaused(false);
    void commands.ttsStep(step);
  };

  useEffect(() => {
    const unlisten = events.ttsEvent.listen(({ payload }) => {
      // Late reports of the chapter left while the next one loads are about nothing on screen.
      if (!readingRef.current || continueAloud.current) return;
      if (payload.chapterId !== dataRef.current?.chapter.id) return;
      if (payload.language) setReadingLanguage(payload.language);
      const notice = payload.notice;
      const key = notice ? JSON.stringify(notice) : null;
      if (notice && key !== noticeShown.current && notice.kind !== 'engineFailed') {
        noticeShown.current = key;
        const language = 'language' in notice && notice.language;
        setSpeechNotice(
          notice.kind === 'voiceFailed'
            ? t('novel.voiceFailed')
            : notice.kind === 'otherVoice'
              ? language
                ? t('novel.otherVoice', { language: languageName(language, i18n.language) })
                : t('novel.otherVoiceUnknown')
              : language
                ? t('novel.noDownloadedVoice', {
                    language: languageName(language, i18n.language),
                  })
                : t('novel.noVoiceDownloaded'),
        );
      }
      switch (payload.state) {
        case 'speaking':
        case 'paused':
          setPaused(payload.state === 'paused');
          setSpeaking(payload.paragraph);
          setSentence({ start: payload.start, end: payload.end });
          position.current = payload.paragraph;
          break;
        case 'ended': {
          const next = dataRef.current?.nextChapterId;
          if (next) {
            continueAloud.current = true;
            setSpeaking(null);
            setSentence(undefined);
            void navigate({ search: { title, chapter: next }, replace: true });
          } else {
            readingEnded();
          }
          break;
        }
        case 'stopped':
          // The notification or a headset stopped it.
          readingEnded();
          break;
        default:
          // The voice engine gave up (no voice for the language, or it was taken away).
          readingEnded();
          setSpeechNotice(t('novel.readAloudStopped'));
      }
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [navigate, readingEnded, title, t, i18n.language]);

  // The next chapter keeps reading aloud from its start.
  useEffect(() => {
    if (data && continueAloud.current && readingRef.current) {
      continueAloud.current = false;
      position.current = 0;
      speakFrom(data, 0, false);
    }
  }, [data, speakFrom]);

  // Leaving the reader stops speech (the place is kept for next time).
  useEffect(
    () => () => {
      if (readingRef.current) void commands.ttsStop();
    },
    [],
  );

  const onBlock = useCallback((index: number) => {
    if (!readingRef.current) position.current = index;
  }, []);

  const go = (chapterId: string | null) => {
    setPanel(false);
    if (!chapterId) {
      router.history.back();
      return;
    }
    // Reading aloud moves with the reader: the chapter left stops at once, and the one opened
    // reads from its start when it loads.
    if (readingRef.current && chapterId !== dataRef.current?.chapter.id) {
      continueAloud.current = true;
      setSpeaking(null);
      setSentence(undefined);
      setPaused(false);
      void commands.ttsStop();
    }
    void navigate({ search: { title, chapter: chapterId }, replace: true });
  };

  /**
   * A bookmark picked in a PDF book's contents: the heading it names in the text (else its page's
   * start), in this chapter (reading aloud goes on from there), or in the chapter opened for it.
   */
  const openBookmark = (entry: PdfContentsEntry) => {
    setContents(null);
    setBookmark((last) => ({
      chapterId: entry.chapterId,
      page: entry.page,
      title: entry.title,
      nonce: (last?.nonce ?? 0) + 1,
    }));
    const pageStarts = pdfText.data?.pageStarts;
    if (data && entry.chapterId === data.chapter.id) {
      if (readingRef.current && pageStarts) {
        readFrom(outlineBlock(data.blocks, pageStarts, entry.page, entry.title));
      }
      return;
    }
    stopReading();
    setPanel(false);
    void navigate({ search: { title, chapter: entry.chapterId }, replace: true });
  };
  const onProgress = useCallback(
    (fraction: number, label: string) => setProgress({ fraction, label }),
    [],
  );

  if (pdf && pdfView === 'pages' && loaded.data) {
    return (
      <PdfPagesView
        key={loaded.data.chapter.id}
        data={loaded.data}
        pdf={pdf}
        onRetry={() => void loaded.refetch()}
        onGo={(chapterId, page) =>
          void navigate({
            search: { title, chapter: chapterId, ...(page === undefined ? {} : { page }) },
            replace: true,
          })
        }
        onTextView={() => setPdfView('text')}
        goTo={askedPage === undefined ? null : { page: askedPage, nonce: 1 }}
      />
    );
  }

  return (
    <section
      data-testid="novel-reader"
      data-theme-page={prefs.theme ?? 'app'}
      // Edge to edge: the page area's side and bottom padding is the reader's too. In landscape
      // the text stays clear of a cutout and a navigation bar at the sides.
      className="bg-background text-foreground -mx-4 -mb-4 flex min-h-[calc(100dvh-var(--app-bar,0px))] flex-col pr-[var(--safe-right)] pl-[var(--safe-left)] lg:-mx-10"
      style={theme ? { backgroundColor: theme.background, color: theme.text } : undefined}
    >
      {chrome && (
        <header className="bg-surface/95 fixed left-[var(--main-left,0px)] right-[var(--main-right,0px)] top-[var(--app-bar,0px)] z-30 flex min-h-16 items-center gap-1 pt-[var(--safe-top)] pr-[calc(0.25rem+var(--safe-right))] pl-[calc(0.25rem+var(--safe-left))] shadow-md backdrop-blur">
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
          {pdf && (
            <button
              type="button"
              aria-label={t('pdf.pageView')}
              onClick={() => {
                stopReading();
                setPdfView('pages');
              }}
              className="hover:bg-background grid size-11 shrink-0 place-items-center rounded-full"
            >
              <FileImage aria-hidden size={21} />
            </button>
          )}
          {pdf && pdf.contents.length > 0 && (
            <button
              type="button"
              aria-label={t('pdf.contents')}
              aria-haspopup="dialog"
              aria-expanded={contents !== null}
              onClick={(event) => {
                setPanel(false);
                const at = pdfText.data ? blockAt(pdfText.data.pageStarts, position.current) : 0;
                setContents(
                  contents ? null : { anchor: event.currentTarget, pageNumber: pdf.firstPage + at },
                );
              }}
              className="hover:bg-background grid size-11 shrink-0 place-items-center rounded-full"
            >
              <ListTree aria-hidden size={21} />
            </button>
          )}
          {data && (
            <button
              type="button"
              aria-label={t('reader.chapters.open')}
              aria-haspopup="dialog"
              aria-expanded={chapterList !== null}
              onClick={(event) => {
                setPanel(false);
                setChapterList(chapterList ? null : event.currentTarget);
              }}
              className="hover:bg-background grid size-11 shrink-0 place-items-center rounded-full"
            >
              <ListOrdered aria-hidden size={22} />
            </button>
          )}
          <button
            type="button"
            aria-label={t('novel.textSettings')}
            aria-expanded={panel}
            onClick={() => setPanel((open) => !open)}
            className={`grid size-11 shrink-0 place-items-center rounded-full ${panel ? 'bg-primary/20 text-primary' : 'hover:bg-background'}`}
          >
            <Type aria-hidden size={20} />
          </button>
        </header>
      )}
      {chrome && panel && (
        <PrefsPanel
          prefs={prefs}
          onChange={(next) => updateSetting.mutate({ key: 'novelReader', value: next })}
          speech={Boolean(speechAvailable.data)}
          tab={panelTab}
          onTab={setPanelTab}
          titleId={data?.titleId ?? title}
          readingLanguage={readingLanguage}
        />
      )}
      {error && (
        <div className="px-4 pt-24">
          <SourceIssue onRetry={() => void (loaded.error ? loaded.refetch() : pdfText.refetch())}>
            {error.message}
          </SourceIssue>
        </div>
      )}
      {!data && !error && (
        <LoaderCircle
          aria-label={t('manga.loading')}
          className="text-muted m-auto mt-32 animate-spin"
        />
      )}
      {data && pdf && data.blocks.length === 0 && (
        <div className="flex flex-col items-center gap-4 px-6 pt-32 text-center">
          <p className="text-muted text-sm">{t('pdf.noText')}</p>
          <button
            type="button"
            onClick={() => setPdfView('pages')}
            className="bg-primary text-background rounded-full px-5 py-2.5 font-semibold"
          >
            {t('pdf.pageView')}
          </button>
        </div>
      )}
      {data && !(pdf && data.blocks.length === 0) && (
        <ChapterView
          pageStarts={pdf ? pdfText.data?.pageStarts : undefined}
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
          onBlock={onBlock}
          speaking={speaking}
          sentence={sentence}
          onSpeakFrom={speechAvailable.data ? readFrom : undefined}
          translated={novelAi.texts}
          goTo={bookmarkBlock}
          onGoToShown={forgetBookmark}
        />
      )}
      {data && (chrome || reading || speechNotice) && (
        // The reader's bottom controls, one above another so none covers another: the AI
        // button (bottom right) beside reading aloud's controls while they fit side by side
        // (above them only when they don't), then the chapter bar.
        <div className="pointer-events-none fixed left-[var(--main-left,0px)] right-[var(--main-right,0px)] bottom-0 z-30 flex flex-col items-center gap-2 pt-2 pr-[calc(0.75rem+var(--safe-right))] pb-[calc(0.75rem+var(--safe-bottom))] pl-[calc(0.75rem+var(--safe-left))]">
          {(chrome || reading) && (
            // Right to left: the AI button keeps its place at the right end; when the bar
            // doesn't fit beside it, the row wraps and the button goes above.
            <div
              data-testid="reader-bottom-row"
              className="@container flex w-full flex-row-reverse flex-wrap items-end gap-2"
            >
              {chrome && (
                <div className="pointer-events-auto shrink-0">
                  <ReaderAiButton medium="novel" busy={novelAi.busy} error={novelAi.error} />
                </div>
              )}
              {reading && (
                <div className="flex min-w-0 grow justify-center">
                  <div
                    role="toolbar"
                    aria-label={t('novel.speechControls')}
                    className="bg-surface/95 pointer-events-auto flex max-w-full shrink-0 items-center justify-center gap-0.5 rounded-full px-1.5 py-1 shadow-lg backdrop-blur"
                  >
                    <button
                      type="button"
                      aria-label={t('novel.previousParagraph')}
                      onClick={() => stepTo(-1)}
                      className="grid size-10 place-items-center rounded-full"
                    >
                      <SkipBack aria-hidden size={18} />
                    </button>
                    <button
                      type="button"
                      aria-label={t(paused ? 'novel.resumeReading' : 'novel.pauseReading')}
                      aria-pressed={paused}
                      onClick={togglePause}
                      className="bg-primary text-background grid size-11 place-items-center rounded-full"
                    >
                      {paused ? <Play aria-hidden size={18} /> : <Pause aria-hidden size={18} />}
                    </button>
                    <button
                      type="button"
                      aria-label={t('novel.nextParagraph')}
                      onClick={() => stepTo(1)}
                      className="grid size-10 place-items-center rounded-full"
                    >
                      <SkipForward aria-hidden size={18} />
                    </button>
                    <button
                      type="button"
                      aria-label={t('novel.replayParagraph')}
                      onClick={() => stepTo(0)}
                      className="grid size-10 place-items-center rounded-full"
                    >
                      <RotateCcw aria-hidden size={18} />
                    </button>
                    <button
                      type="button"
                      aria-label={`${t('novel.speed')}: ${(rate / 10).toFixed(1)}×`}
                      onClick={() =>
                        updateSetting.mutate({
                          key: 'novelReader',
                          value: {
                            ...prefs,
                            ttsRate: SPEEDS.find((speed) => speed > rate) ?? SPEEDS[0],
                          },
                        })
                      }
                      className="min-w-11 rounded-full px-1.5 text-sm font-semibold tabular-nums"
                    >
                      {(rate / 10).toFixed(1)}×
                    </button>
                    <button
                      type="button"
                      aria-label={t('novel.readAloudOptions.open')}
                      aria-haspopup="dialog"
                      aria-expanded={speechOptions !== null}
                      onClick={(event) =>
                        setSpeechOptions(speechOptions ? null : event.currentTarget)
                      }
                      className="grid size-10 place-items-center rounded-full"
                    >
                      <SlidersHorizontal aria-hidden size={18} />
                    </button>
                  </div>
                </div>
              )}
              {chrome && reading && (
                // On wide screens, as wide as the AI button on the other side: the bar stays in
                // the middle.
                <span aria-hidden className="hidden size-12 shrink-0 @min-[26rem]:block" />
              )}
            </div>
          )}
          {speechNotice && (
            <p
              role="status"
              className="bg-surface/95 text-foreground pointer-events-auto w-fit max-w-[90vw] rounded-full px-4 py-2 text-sm shadow-lg"
            >
              {speechNotice}
            </p>
          )}
          {chrome && (
            <footer className="pointer-events-auto flex w-full items-center gap-2">
              <button
                type="button"
                aria-label={t('manga.goPreviousChapter')}
                disabled={!data.previousChapterId}
                onClick={() => go(data.previousChapterId)}
                className="bg-surface/95 grid size-12 shrink-0 place-items-center rounded-full shadow-md backdrop-blur disabled:opacity-40"
              >
                <ChevronLeft aria-hidden size={22} />
              </button>
              <div className="bg-surface/95 flex min-w-0 flex-1 items-center gap-2 rounded-full px-4 py-2.5 shadow-md backdrop-blur">
                <div className="flex min-w-0 flex-1 flex-col gap-1">
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
                {speechAvailable.data && (
                  <button
                    type="button"
                    aria-label={t(reading ? 'novel.stopReading' : 'novel.readAloud')}
                    aria-pressed={reading}
                    onClick={toggleReading}
                    className={`grid size-10 shrink-0 place-items-center rounded-full ${reading ? 'bg-primary text-background' : 'hover:bg-background'}`}
                  >
                    {reading ? <Square aria-hidden size={18} /> : <Volume2 aria-hidden size={20} />}
                  </button>
                )}
              </div>
              <button
                type="button"
                aria-label={t('manga.goNextChapter')}
                disabled={!data.nextChapterId}
                onClick={() => go(data.nextChapterId)}
                className="bg-surface/95 grid size-12 shrink-0 place-items-center rounded-full shadow-md backdrop-blur disabled:opacity-40"
              >
                <ChevronRight aria-hidden size={22} />
              </button>
            </footer>
          )}
        </div>
      )}
      {data && reading && speechOptions && (
        <Sheet
          title={t('novel.readAloudOptions.title')}
          anchor={speechOptions}
          popoverWidth={360}
          onClose={() => setSpeechOptions(null)}
        >
          <ReadAloudOptions titleId={data.titleId} readingLanguage={readingLanguage} />
        </Sheet>
      )}
      {data && chapterList && (
        <ChapterListSheet
          medium="novel"
          titleId={data.titleId}
          currentId={data.chapter.id}
          anchor={chapterList}
          onPick={(chapterId) => {
            setChapterList(null);
            go(chapterId);
          }}
          onClose={() => setChapterList(null)}
        />
      )}
      {pdf && contents && (
        <PdfContentsSheet
          contents={pdf.contents}
          pageNumber={contents.pageNumber}
          anchor={contents.anchor}
          onPick={openBookmark}
          onClose={() => setContents(null)}
        />
      )}
    </section>
  );
};
