import type { MangaSource, SourceSearchResult } from '@raygun/bindings';
import { useQueries, useQueryClient } from '@tanstack/react-query';
import { ChevronRight } from 'lucide-react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { AnswerCard } from '../answers/AnswerCard';
import { BookCard } from '../audiobooks/BookRow';
import { useBookDirectory } from '../audiobooks/useAudiobooks';
import type { Medium } from '../manga/medium';
import { PodcastResultList } from '../podcasts/PodcastResultList';
import { usePodcastSearch, useTopPodcasts } from '../podcasts/usePodcasts';
import { useMangaSources } from '../manga/useManga';
import { Carousel } from '../shell/Carousel';
import { useDrawnCount } from '../shell/useDrawnCount';
import { OpeningBook } from './AudiobookSearch';
import { useOpenBook } from './useOpenBook';
import { SourceRow } from './CatalogSearch';
import { LibraryMatches } from './LibraryMatches';
import { NoSources } from './NoSources';
import { MusicResults } from './MusicResults';
import { VideoResults } from './VideoResults';
import type { SearchType, SearchView } from './searchTypes';
import { keepSource, selectSourceResult, useSearchFilter } from './searchFilter';
import { sourceSearchOptions } from './useCatalogSearch';
import { NeedsSetupNote } from '../shell/NeedsSetupNote';
import { SourceIssue } from '../shell/SourceIssue';
import { useSourcesSetup } from '../sources/useSourcesSetup';
import { RankedResults } from './CatalogSearch';

/** Every kind All searches; the sources of them waiting for a key are counted in one line. */
const ALL_KINDS = [
  'music.metadata',
  'podcast.directory',
  'audiobook.directory',
  'video.source',
  'manga.source',
  'novel.source',
];

const Section: FC<{
  title: string;
  onMore: () => void;
  children: ReactNode;
}> = ({ title, onMore, children }) => {
  const { t } = useTranslation();
  return (
    <section aria-label={title} className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="font-semibold">{title}</h2>
        <button
          type="button"
          onClick={onMore}
          className="text-primary flex items-center gap-0.5 text-sm"
        >
          {t('search.seeAll')}
          <ChevronRight aria-hidden size={16} />
        </button>
      </div>
      {children}
    </section>
  );
};

/** Rows a section of Search → All shows. */
const PREVIEW_ROWS = 2;
/** Sections whose results are drawn with the page; the others' follow (`useDrawnCount`). */
const FIRST_SECTIONS = 3;
/** Search → All's ranked comics and books: from this many sources, this many titles (M46.35). */
const PREVIEW_SOURCES = 6;
const PREVIEW_ITEMS = 12;

/**
 * The first two sources with results (or popular titles), as short rows. Sources are asked in
 * order until two have something (M46.7), not all at once.
 */
const CatalogPreview: FC<{ medium: Medium; query: string }> = ({ medium, query }) => {
  const { t } = useTranslation();
  const sources = useMangaSources(medium);
  const filter = useSearchFilter();
  // Sources waiting for a key or sign-in aren't asked (the page's line counts them).
  const waiting = new Set(useSourcesSetup().map((source) => source.extensionId));
  if (!sources.data) return <p className="text-muted text-sm">{t('search.searching')}</p>;
  if (sources.data.length === 0) {
    return <NoSources label={t('search.noCatalogSources')} />;
  }
  const asked = sources.data.filter(
    (source) => keepSource(filter, source) && !waiting.has(source.extensionId),
  );
  // Comics and books: one list ranked across the first sources (M46.35), not a few per source.
  if (medium !== 'video') {
    const enabled = asked.filter((source) => source.enabled).slice(0, PREVIEW_SOURCES);
    return (
      <RankedResults
        key={query.trim()}
        sources={enabled}
        medium={medium}
        query={query}
        limit={PREVIEW_ITEMS}
      />
    );
  }
  return <PreviewRows key={query.trim()} medium={medium} query={query} sources={asked} />;
};

const PreviewRows: FC<{ medium: Medium; query: string; sources: MangaSource[] }> = ({
  medium,
  query,
  sources,
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const filter = useSearchFilter();
  // Sources are asked in order, as many as could still fill the rows: answered ones with
  // results (as Search's filters keep them) count, pending ones might (M46.7). Each answer
  // re-renders and may ask the next.
  const kept = selectSourceResult(filter, medium);
  let asked = 0;
  let filled = 0;
  for (const source of sources) {
    if (filled >= PREVIEW_ROWS) break;
    asked += 1;
    const state = client.getQueryState<SourceSearchResult | null>(
      sourceSearchOptions(medium, source.id, query).queryKey,
    );
    const answered = state?.status === 'success' || state?.status === 'error';
    if (!answered || (kept(state.data ?? null)?.items.length ?? 0) > 0) filled += 1;
  }
  const results = useQueries({
    queries: sources
      .slice(0, asked)
      .map((source) => sourceSearchOptions(medium, source.id, query, filter)),
  });
  const settled = results.every((result) => !result.isPending);
  const found = results
    .map((result) => result.data)
    .filter((result) => result !== null && result !== undefined && result.items.length > 0);
  const more = found.length < PREVIEW_ROWS && asked < sources.length;

  if (found.length === 0) {
    return (
      <p className="text-muted text-sm">
        {settled && !more ? t('search.noResults', { query }) : t('search.searching')}
      </p>
    );
  }
  return (
    <div className="flex flex-col gap-4">
      {found.slice(0, PREVIEW_ROWS).map((result) => (
        <SourceRow key={result!.source.id} result={result!} medium={medium} limit={3} />
      ))}
    </div>
  );
};

/** The audiobook sources' first books for the query, or their popular ones before it. */
const AudiobookPreview: FC<{ query: string }> = ({ query }) => {
  const { t } = useTranslation();
  const wanted = query.trim();
  const { openBook, open } = useOpenBook();
  const books = useBookDirectory(wanted ? 'search' : 'popular', wanted);
  const found = books.data?.pages[0]?.books ?? [];
  if (books.error) {
    return (
      <SourceIssue compact onRetry={() => void books.refetch()}>
        {books.error.message}
      </SourceIssue>
    );
  }
  if (books.isSuccess && found.length === 0) {
    return <p className="text-muted text-sm">{t('audiobooks.noResults')}</p>;
  }
  return (
    <>
      <OpeningBook open={open} />
      <Carousel className="-mx-4 gap-1 px-2 lg:-mx-2">
        {found.slice(0, 12).map((book) => (
          <BookCard key={book.feedUrl} book={book} onOpen={openBook} />
        ))}
      </Carousel>
    </>
  );
};

const PodcastPreview: FC<{ query: string }> = ({ query }) => {
  const { t } = useTranslation();
  const found = usePodcastSearch(query);
  const top = useTopPodcasts(query === '');
  const search = query ? found : top;
  if (search.error) {
    return <SourceIssue>{search.error.message}</SourceIssue>;
  }
  if (search.data?.length === 0) {
    return <p className="text-muted text-sm">{t('podcasts.noResults')}</p>;
  }
  return search.data ? <PodcastResultList results={search.data.slice(0, 12)} grid /> : null;
};

/**
 * Search → All: a little of everything, each section leading to its full results. Before
 * anything is typed it shows what's popular: addons' catalogs, top podcasts, sources' popular
 * titles. A question or a calculation gets its answer above them, once it's ready.
 */
export const AllResults: FC<{
  query: string;
  onType: (type: SearchType, view?: SearchView) => void;
  /** Asks the Answers pill something an answer names. */
  onAsk?: (query: string) => void;
}> = ({ query, onType, onAsk }) => {
  const { t } = useTranslation();
  const blank = query.trim() === '';
  const sections = [
    ...(blank
      ? []
      : [
          {
            title: t('search.inYourLibrary'),
            more: () => onType('library'),
            body: <LibraryMatches query={query} limit={4} />,
          },
          {
            title: t('search.type.music'),
            more: () => onType('music'),
            body: <MusicResults query={query} compact />,
          },
        ]),
    {
      title: t('library.media.podcast'),
      more: () => onType('music', 'podcasts'),
      body: <PodcastPreview query={query} />,
    },
    {
      title: t('search.music.audiobooks'),
      more: () => onType('music', 'audiobooks'),
      body: <AudiobookPreview query={query} />,
    },
    {
      title: t('search.type.video'),
      more: () => onType('video'),
      body: (
        <>
          <VideoResults query={query} limit={2} />
          <CatalogPreview medium="video" query={query} />
        </>
      ),
    },
    {
      title: t('library.media.manga'),
      more: () => onType('manga'),
      body: <CatalogPreview medium="manga" query={query} />,
    },
    {
      title: t('library.media.novel'),
      more: () => onType('novel'),
      body: <CatalogPreview medium="novel" query={query} />,
    },
  ];
  // Every section's heading at once; what the first ones found at once too, the others' as each
  // part is on the screen (`useDrawnCount`).
  const drawn = useDrawnCount(sections.length, FIRST_SECTIONS, 2, query.trim());
  return (
    <div data-testid="all-results" className="flex flex-col gap-8">
      {blank && <p className="text-muted text-sm">{t('search.popularHint')}</p>}
      {!blank && (
        <AnswerCard
          query={query}
          onMore={() => onType('answers')}
          onAsk={(next) => (onAsk ? onAsk(next) : onType('answers'))}
        />
      )}
      {sections.map((section, index) => (
        <Section key={section.title} title={section.title} onMore={section.more}>
          {index < drawn && section.body}
        </Section>
      ))}
      <NeedsSetupNote kinds={ALL_KINDS} />
    </div>
  );
};
