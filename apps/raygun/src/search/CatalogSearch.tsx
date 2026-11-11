import type { MangaCard, MangaSource, SourceSearchResult } from '@raygun/bindings';
import { useQueries } from '@tanstack/react-query';
import type { UseQueryResult } from '@tanstack/react-query';
import { ChevronRight, LoaderCircle, SlidersHorizontal } from 'lucide-react';
import { memo, useCallback, useDeferredValue, useEffect, useId, useMemo, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { COVER_GRID_CLASS, coverGridStyle } from '../manga/coverGrid';
import { MangaGrid } from '../manga/MangaGrid';
import type { Medium } from '../manga/medium';
import { useMangaLibrary, useMangaSources } from '../manga/useManga';
import { ROUTES } from '../manga/medium';
import { AdultMark } from '../shell/AdultMark';
import { matchesSearch } from '../shell/useListSearch';
import { useInView } from '../shell/useInView';
import { CustomSourcesPill, SourceSwitchPanel } from './CustomSources';
import { useCustomSources } from './useCustomSources';
import { FailedSources } from './FailedSources';
import { FromListingsNote } from './FromListingsNote';
import { NoSources } from './NoSources';
import { SourceMenu } from './SourceMenu';
import { SourceFiltersPanel } from './SourceFiltersPanel';
import { rankResults, titleScore } from './rankResults';
import type { RankedCard } from './rankResults';
import { videoTitleScore } from '../watch/videoMatch';
import { hidesSaved, keepCard, keepCards, keepSource, useSearchFilter } from './searchFilter';
import { useSeenTitles } from './seenBefore';
import { answeredFirst, failingLast, useSourceHealth } from './sourceHealth';
import {
  sourceSearchOptions,
  useFilteredSearch,
  useSourceAnswers,
  useSourceFilters,
  useSourceSearch,
} from './useCatalogSearch';
import type { FilterValues } from './useCatalogSearch';
import { LoadMore } from '../shell/LoadMore';
import { useSettings } from '../settings/useSettings';
import { SourceIssue } from '../shell/SourceIssue';

/** Cards for a grid; `fromListing` labels titles found in a source's listings, not its search. */
const toGridItems = (items: MangaCard[], fromListing?: string) =>
  items.map((item) => ({
    sourceId: item.sourceId,
    id: item.id,
    title: item.title,
    coverUrl: item.coverUrl,
    inLibrary: item.inLibrary,
    ...(item.adult ? { adult: true } : {}),
    ...(item.fromListing && fromListing ? { subtitle: fromListing } : {}),
  }));

/** One source's first results in a global search, as a short row. */
export const SourceRow: FC<{
  result: SourceSearchResult;
  medium: Medium;
  limit?: number;
  onMore?: () => void;
}> = ({ result, medium, limit = 6, onMore }) => {
  const { t } = useTranslation();
  return (
    <section aria-label={result.source.name} className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <h3 className="text-muted flex min-w-0 items-center gap-1 text-sm font-medium">
          <span className="truncate">{result.source.name}</span>
          {result.source.adult && <AdultMark />}
          {result.fromListings && <FromListingsNote />}
          <SourceMenu
            sourceId={result.source.id}
            extensionId={result.source.extensionId}
            name={result.source.name}
            page={{ to: ROUTES[medium].source, search: { id: result.source.id } }}
            onOnly={onMore}
          />
        </h3>
        {onMore && result.items.length > 0 && (
          <button
            type="button"
            onClick={onMore}
            className="text-primary flex items-center gap-0.5 text-sm"
          >
            {t('search.more')}
            <ChevronRight aria-hidden size={16} />
          </button>
        )}
      </div>
      {result.error ? (
        <SourceIssue compact source={result.source.name}>
          {result.error}
        </SourceIssue>
      ) : result.items.length === 0 ? (
        <p className="text-muted text-xs">{t('search.providerEmpty')}</p>
      ) : (
        <MangaGrid
          medium={medium}
          landscape={Boolean(result.source.searchPill)}
          // A row from the source's listings says so once, beside its name.
          items={toGridItems(
            result.items.slice(0, limit),
            result.fromListings ? undefined : t('search.fromListing'),
          )}
        />
      )}
    </section>
  );
};

/**
 * Cover-shaped placeholders while a source's row loads, in the same grid as the covers that
 * replace them (the cover size setting), so the page doesn't jump or show giant boxes.
 */
const RowPlaceholder: FC<{ count?: number }> = ({ count = 6 }) => {
  const settings = useSettings().data;
  return (
    <div
      aria-hidden
      data-testid="row-placeholder"
      className={COVER_GRID_CLASS}
      style={coverGridStyle(settings?.coverSize ?? 0)}
    >
      {Array.from({ length: count }, (_, index) => (
        <span key={index} className="bg-surface aspect-[2/3] animate-pulse rounded-lg" />
      ))}
    </div>
  );
};

/**
 * A source's row that loads only when it comes near the window (M46.7), queued with the others,
 * so a search over many sources never runs them all at once.
 */
export const LazySourceRow: FC<{
  source: MangaSource;
  medium: Medium;
  query: string;
  limit?: number;
  onMore?: () => void;
}> = ({ source, medium, query, limit, onMore }) => {
  const { t } = useTranslation();
  const [ref, seen] = useInView<HTMLDivElement>();
  const search = useSourceSearch(medium, source.id, query, seen);
  useHealthReport(source.id, search.error, search.data);
  return (
    <div ref={ref} data-testid="source-row">
      {search.data ? (
        <SourceRow result={search.data} medium={medium} limit={limit} onMore={onMore} />
      ) : (
        <section aria-label={source.name} aria-busy={!search.error} className="flex flex-col gap-2">
          <h3 className="text-muted flex min-w-0 items-center gap-1 text-sm font-medium">
            <span className="truncate">{source.name}</span>
            {source.adult && <AdultMark />}
          </h3>
          {search.error ? (
            <SourceIssue compact source={source.name} onRetry={() => void search.refetch()}>
              {search.error.message}
            </SourceIssue>
          ) : search.isSuccess ? (
            <p className="text-muted text-xs">{t('search.providerEmpty')}</p>
          ) : (
            <RowPlaceholder count={Math.min(limit ?? 6, 6)} />
          )}
        </section>
      )}
    </div>
  );
};

/** Remembers which sources failed lately, so lists can put them last (M46.9). */
const useHealthReport = (
  id: string,
  error: unknown,
  data: SourceSearchResult | null | undefined,
) => {
  const { failed, worked } = useSourceHealth();
  useEffect(() => {
    if (error || data?.error) failed(id);
    else if (data) worked(id);
  }, [id, error, data, failed, worked]);
};

/** Titles a ranked list draws at first, and how many more each "Show more" adds. */
const RANKED_PAGE = 48;

/**
 * Every searched source's results in one list (M46.9), the best answers first, each with its
 * source's name. Sources answer a few at a time; the list grows as they do, and draws a page of
 * covers at a time however many sources answer. Sources that couldn't search are named once,
 * quietly, at the end.
 */
export const RankedResults: FC<{
  sources: MangaSource[];
  medium: Medium;
  query: string;
  /** Show at most this many (Search → All's preview, M46.35). */
  limit?: number;
}> = ({ sources, medium, query, limit }) => {
  const { t } = useTranslation();
  const { failed, worked } = useSourceHealth();
  const [shownCount, setShownCount] = useState(RANKED_PAGE);
  const ids = sources.map((source) => source.id).join('\n');
  // Only what the list needs from each search, worked out again only when an answer comes.
  const combine = useCallback(
    (answers: UseQueryResult<SourceSearchResult | null>[]) => {
      const order = ids.split('\n');
      return {
        results: answers
          .map((search) => search.data)
          .filter((result): result is SourceSearchResult => Boolean(result)),
        answered: answers.filter((search) => search.isSuccess || search.isError).length,
        errorKey: answers
          .map((search, index) => `${order[index]}:${search.status}:${search.data?.error ?? ''}`)
          .join('|'),
        failedIds: answers
          .map((search, index) =>
            search.isError || search.data?.error ? (order[index] ?? null) : null,
          )
          .filter((id): id is string => id !== null),
      };
    },
    [ids],
  );
  const filter = useSearchFilter();
  const searches = useQueries({
    queries: sources.map((source) => sourceSearchOptions(medium, source.id, query, filter)),
    combine,
  });
  const { results, answered, errorKey } = searches;
  const answers = useMemo(
    () =>
      rankResults(
        results.filter((result) => !result.error),
        query,
        medium === 'video' ? videoTitleScore : titleScore,
      ),
    [results, query, medium],
  );
  // Titles already seen (Home, sources' pages, the library) show until every source has
  // answered (M46.47), after the answers that came, as the filters keep them.
  const seen = useSeenTitles(medium, query);
  const searchedIds = new Set(sources.map((source) => source.id));
  const named = new Map(sources.map((source) => [source.id, source.name]));
  const adultSources = new Set(sources.filter((source) => source.adult).map((source) => source.id));
  const ranked: RankedCard[] =
    answered < sources.length
      ? [
          ...answers,
          ...seen
            .filter(
              (card) =>
                searchedIds.has(card.sourceId) &&
                keepCard(filter, card, medium, adultSources.has(card.sourceId)) &&
                !answers.some(
                  (answer) => answer.card.sourceId === card.sourceId && answer.card.id === card.id,
                ),
            )
            .map((card) => ({ card, sourceName: named.get(card.sourceId) ?? '' })),
        ]
      : answers;
  const failures = results
    .filter((result) => result.error)
    .map((result) => ({ name: result.source.name, error: result.error ?? '' }));
  useEffect(() => {
    for (const id of searches.failedIds) failed(id);
    for (const result of results) {
      if (!result.error) worked(result.source.id);
    }
    // Reported once per change of the sources' answers.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [errorKey]);
  const cap = limit ?? shownCount;
  return (
    <div data-testid="ranked-results" className="flex flex-col gap-3">
      {answered < sources.length && (
        <p role="status" className="text-muted flex items-center gap-2 text-sm">
          <LoaderCircle aria-hidden size={14} className="animate-spin" />
          {t('search.sourcesAnswered', { done: answered, count: sources.length })}
        </p>
      )}
      {answered === sources.length && ranked.length === 0 && (
        <p className="text-muted text-sm">
          {query.trim() ? t('search.noResults', { query }) : t('search.providerEmpty')}
        </p>
      )}
      {ranked.length > 0 && (
        <MangaGrid
          medium={medium}
          items={ranked.slice(0, cap).map(({ card, sourceName }) => ({
            ...toGridItems([card])[0]!,
            source: sourceName,
            ...(card.fromListing ? { subtitle: t('search.fromListing') } : {}),
            ...(card.adult || adultSources.has(card.sourceId) ? { adult: true } : {}),
          }))}
        />
      )}
      {limit === undefined && ranked.length > cap && (
        <button
          type="button"
          onClick={() => setShownCount(cap + RANKED_PAGE)}
          className="bg-surface self-center rounded-full px-5 py-2 text-sm"
        >
          {t('search.showMore', { count: Math.min(RANKED_PAGE, ranked.length - cap) })}
        </button>
      )}
      <FailedSources failed={failures} />
    </div>
  );
};

/** Titles in the library that match, before the sources' answers (Relevant). */
const InYourLibrary: FC<{ medium: Medium; query: string; onMore: () => void }> = ({
  medium,
  query,
  onMore,
}) => {
  const { t } = useTranslation();
  const library = useMangaLibrary(medium);
  const wanted = query.trim();
  const matches = wanted
    ? (library.data ?? []).filter((entry) => matchesSearch(wanted, [entry.title]))
    : [];
  if (matches.length === 0) return null;
  return (
    <section aria-label={t('search.inYourLibrary')} className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <h3 className="text-muted text-sm font-medium">{t('search.inYourLibrary')}</h3>
        {matches.length > 6 && (
          <button
            type="button"
            onClick={onMore}
            className="text-primary flex items-center gap-0.5 text-sm"
          >
            {t('search.seeAll')}
            <ChevronRight aria-hidden size={16} />
          </button>
        )}
      </div>
      <MangaGrid
        medium={medium}
        items={matches.slice(0, 6).map((entry) => ({
          sourceId: entry.sourceId,
          id: entry.remoteId,
          title: entry.title,
          coverUrl: entry.coverUrl,
          unread: entry.unread,
          author: entry.authors.join(', ') || null,
        }))}
      />
    </section>
  );
};

const NO_IDS: string[] = [];
const NO_SOURCES: MangaSource[] = [];

/** The ranked list, drawn again only when what it searches changes (not as a switch shows). */
const SearchedResults = memo(RankedResults);

const groupKey = (medium: Medium) => `raygun.search.grouped.${medium}`;

const readGrouped = (medium: Medium) => {
  try {
    return localStorage.getItem(groupKey(medium)) === '1';
  } catch {
    return false;
  }
};

const SourceSearch: FC<{
  medium: Medium;
  sourceId: string;
  query: string;
  landscape?: boolean;
  /** The source is rated for adults: under "18+ only" all its titles stay. */
  adult?: boolean;
}> = ({ medium, sourceId, query, landscape = false, adult = false }) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [values, setValues] = useState<FilterValues>({});
  const filters = useSourceFilters(medium, sourceId);
  const search = useFilteredSearch(medium, sourceId, query, values);
  const filter = useSearchFilter();
  // One source's own search isn't filtered in Rust (sources' pages share it): Search's filters
  // apply here.
  const items = keepCards(
    filter,
    search.data?.pages.flatMap((page) => page.items) ?? [],
    medium,
    adult,
  );
  const hasFilters = (filters.data?.length ?? 0) > 0;
  const active = Object.keys(values).length;
  // Its search failed or found nothing: every title shown came from its listings.
  const listed = items.length > 0 && items.every((item) => item.fromListing);

  return (
    <div className="flex flex-col gap-3">
      {hasFilters && (
        <button
          type="button"
          aria-expanded={open}
          onClick={() => setOpen(!open)}
          className={`flex w-fit items-center gap-2 rounded-full px-4 py-2 text-sm ${open || active ? 'bg-primary/20 text-primary font-semibold' : 'bg-surface'}`}
        >
          <SlidersHorizontal aria-hidden size={16} />
          {active > 0 ? t('search.filters.active', { count: active }) : t('search.filters.title')}
        </button>
      )}
      {open && filters.data && (
        <SourceFiltersPanel filters={filters.data} values={values} onChange={setValues} />
      )}
      {!query.trim() && active === 0 && <h2 className="font-semibold">{t('search.popular')}</h2>}
      {search.error && (
        <SourceIssue onRetry={() => void search.refetch()}>{search.error.message}</SourceIssue>
      )}
      {search.isFetching && items.length === 0 && (
        <LoaderCircle aria-label={t('search.searching')} className="text-muted animate-spin" />
      )}
      {search.isSuccess && items.length === 0 && (
        <p className="text-muted text-sm">
          {query.trim() ? t('search.noResults', { query }) : t('search.providerEmpty')}
        </p>
      )}
      {listed && (
        <p data-testid="from-listings" className="text-muted text-xs">
          {t('search.fromListingsHint')}
        </p>
      )}
      {items.length > 0 && (
        <MangaGrid
          medium={medium}
          landscape={landscape}
          items={toGridItems(items, listed ? undefined : t('search.fromListing'))}
        />
      )}
      <LoadMore
        hasNextPage={search.hasNextPage}
        fetching={search.isFetchingNextPage}
        onLoad={() => void search.fetchNextPage()}
      />
    </div>
  );
};

/**
 * Search → Comics, Books or Videos: every source, or with Custom sources on, the ones switched on
 * in its panel. Several sources search together, their answers ranked as one list (Best
 * matches, which also leads with what's already in the library) or a row each (By source); one
 * source alone searches with its filters and tags.
 */
export const CatalogSearch: FC<{
  medium: Medium;
  query: string;
  /** One source only, without a choice of sources: a video source's own pill (M46.35). */
  only?: string;
  /** Sources with their own pill (YouTube, PeerTube…) among the others too (M46.40). */
  withPills?: boolean;
  /** Where Best matches' "In your library" leads. */
  onLibrary?: () => void;
}> = ({ medium, query, only, withPills = false, onLibrary }) => {
  const { t } = useTranslation();
  const listed = useMangaSources(medium);
  const filter = useSearchFilter();
  // A source with its own pill searches there, not among the rest (M46.35); "No 18+" leaves
  // adult sources out. The same list while nothing changes (Custom sources' tiles follow it).
  const kept = useMemo(
    () =>
      listed.data?.filter(
        (source) =>
          (only ? source.id === only : withPills || !source.searchPill) &&
          keepSource(filter, source),
      ),
    [listed.data, only, withPills, filter],
  );
  const sources = { ...listed, data: kept };
  const choice = useCustomSources(medium, sources.data);
  const panelId = useId();
  const [grouped, setGroupedState] = useState(() => readGrouped(medium));
  const failures = useSourceHealth((state) => state.failures);
  const failingAtEnd = useSettings().data?.failingSourcesLast ?? true;
  const setGrouped = (next: boolean) => {
    setGroupedState(next);
    try {
      localStorage.setItem(groupKey(medium), next ? '1' : '0');
    } catch {
      // A per-device convenience.
    }
  };
  // Custom sources off: every source; on: the ones switched on (one alone, with its filters).
  const custom = !only && choice.custom;
  // The pill and the panel answer a tap at once; the search follows without holding it up.
  const choosing = useMemo(() => ({ custom, picked: choice.picked }), [custom, choice.picked]);
  const asked = useDeferredValue(choosing);
  const picked = useMemo(
    () => (only ? [only] : asked.custom ? asked.picked : NO_IDS),
    [only, asked],
  );
  const sourceId = picked.length === 1 ? picked[0]! : '';
  const all = sources.data ?? NO_SOURCES;
  // Several sources: a row each, loading as they come into view (M46.7).
  const searched = useMemo(() => {
    const on = new Set(picked);
    return failingLast(
      picked.length > 0 ? all.filter((source) => on.has(source.id)) : asked.custom ? [] : all,
      failures,
      failingAtEnd,
    );
  }, [picked, all, asked.custom, failures, failingAtEnd]);
  // By source, the rows that answered with titles lead, then those with none, then failures;
  // a row still asking keeps its place.
  const answers = useSourceAnswers(
    medium,
    searched.map((source) => source.id),
    query,
  );
  const rows = answeredFirst(searched, (source) => answers(source.id));
  const relevant = !only && !asked.custom && !grouped;
  // Best matches or By source, while more than one source searches.
  const arrange = sourceId === '' && searched.length > 1 && (
    <div
      role="group"
      aria-label={t('search.arrange')}
      className="bg-surface ml-auto flex rounded-full p-0.5 text-sm"
    >
      {([false, true] as const).map((byGroup) => (
        <button
          key={String(byGroup)}
          type="button"
          aria-pressed={grouped === byGroup}
          onClick={() => setGrouped(byGroup)}
          className={`rounded-full px-3 py-1 ${grouped === byGroup ? 'bg-primary/20 text-primary font-semibold' : 'text-muted'}`}
        >
          {t(byGroup ? 'search.bySource' : 'search.byResult')}
        </button>
      ))}
    </div>
  );

  return (
    <div data-testid={`${medium}-search`} className="flex flex-col gap-4">
      {sources.data?.length === 0 && <NoSources label={t('search.noCatalogSources')} />}
      {all.length > 0 && !only && (
        <div className="flex flex-col gap-2">
          {/* One pill: off searches every source; on unfolds a switch for each. */}
          <div className="flex flex-wrap items-center gap-2">
            <CustomSourcesPill
              on={custom}
              count={choice.picked.length}
              total={all.length}
              panelId={panelId}
              onChange={choice.setCustom}
            />
            {arrange}
          </div>
          <SourceSwitchPanel
            open={custom}
            id={panelId}
            medium={medium}
            sources={all}
            picked={choice.picked}
            onChange={choice.set}
            onChoose={choice.choose}
          />
        </div>
      )}
      {relevant && medium !== 'video' && onLibrary && !hidesSaved(filter, medium) && (
        <InYourLibrary medium={medium} query={query} onMore={onLibrary} />
      )}
      {asked.custom && picked.length === 0 ? (
        <p className="text-muted text-sm">{t('search.customSources.noneOn')}</p>
      ) : sourceId === '' ? (
        <>
          {!query.trim() && all.length > 0 && (
            <h2 className="font-semibold">{t('search.popular')}</h2>
          )}
          {grouped || searched.length === 1 ? (
            rows.map((source) => (
              <LazySourceRow
                key={`${source.id}:${query.trim()}`}
                source={source}
                medium={medium}
                query={query}
                onMore={() => choice.only(source.id)}
              />
            ))
          ) : (
            <SearchedResults key={query.trim()} sources={searched} medium={medium} query={query} />
          )}
        </>
      ) : (
        <SourceSearch
          key={sourceId}
          medium={medium}
          sourceId={sourceId}
          query={query}
          // Video sites' thumbnails are 16:9 (M46.45).
          landscape={Boolean(all.find((source) => source.id === sourceId)?.searchPill)}
          adult={Boolean(all.find((source) => source.id === sourceId)?.adult)}
        />
      )}
    </div>
  );
};
