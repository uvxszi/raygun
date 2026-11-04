import type { MangaCard, MangaSource, SourceSearchResult } from '@raygun/bindings';
import { useQueries } from '@tanstack/react-query';
import { ChevronRight, LoaderCircle, Search, SlidersHorizontal } from 'lucide-react';
import { useEffect, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { COVER_GRID_CLASS, coverGridStyle } from '../manga/coverGrid';
import { MangaGrid } from '../manga/MangaGrid';
import type { Medium } from '../manga/medium';
import { useMangaSources } from '../manga/useManga';
import { ROUTES } from '../manga/medium';
import { useInView } from '../shell/useInView';
import { SourceMenu } from './SourceMenu';
import { SourceFiltersPanel } from './SourceFiltersPanel';
import { rankResults, titleScore } from './rankResults';
import type { RankedCard } from './rankResults';
import { videoTitleScore } from '../watch/videoMatch';
import { useSeenTitles } from './seenBefore';
import { failingLast, useSourceHealth } from './sourceHealth';
import {
  sourceSearchOptions,
  useFilteredSearch,
  useSourceFilters,
  useSourceSearch,
} from './useCatalogSearch';
import type { FilterValues } from './useCatalogSearch';
import { LoadMore } from '../shell/LoadMore';
import { useSettings } from '../settings/useSettings';
import { SourceIssue } from '../shell/SourceIssue';

const toGridItems = (items: MangaCard[]) =>
  items.map((item) => ({
    sourceId: item.sourceId,
    id: item.id,
    title: item.title,
    coverUrl: item.coverUrl,
    inLibrary: item.inLibrary,
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
          {t('search.providerFailed', { message: result.error })}
        </SourceIssue>
      ) : result.items.length === 0 ? (
        <p className="text-muted text-xs">{t('search.providerEmpty')}</p>
      ) : (
        <MangaGrid
          medium={medium}
          landscape={Boolean(result.source.searchPill)}
          items={toGridItems(result.items.slice(0, limit))}
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
      style={coverGridStyle(settings?.gridColumns ?? 0, settings?.coverWidth ?? 0)}
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
          <h3 className="text-muted text-sm font-medium">{source.name}</h3>
          {search.error ? (
            <SourceIssue compact source={source.name} onRetry={() => void search.refetch()}>
              {t('search.providerFailed', { message: search.error.message })}
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

/**
 * Every searched source's results in one list (M46.9), the best answers first, each with its
 * source's name. Sources answer a few at a time; the list grows as they do.
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
  const searches = useQueries({
    queries: sources.map((source) => sourceSearchOptions(medium, source.id, query)),
  });
  const answered = searches.filter((search) => search.isSuccess || search.isError).length;
  const results = searches
    .map((search) => search.data)
    .filter((result): result is SourceSearchResult => Boolean(result));
  const answers = rankResults(
    results.filter((result) => !result.error),
    query,
    medium === 'video' ? videoTitleScore : titleScore,
  );
  // Titles already seen (Home, sources' pages, the library) show until every source has
  // answered (M46.47), after the answers that came.
  const seen = useSeenTitles(medium, query);
  const searchedIds = new Set(sources.map((source) => source.id));
  const named = new Map(sources.map((source) => [source.id, source.name]));
  const ranked: RankedCard[] =
    answered < sources.length
      ? [
          ...answers,
          ...seen
            .filter(
              (card) =>
                searchedIds.has(card.sourceId) &&
                !answers.some(
                  (answer) => answer.card.sourceId === card.sourceId && answer.card.id === card.id,
                ),
            )
            .map((card) => ({ card, sourceName: named.get(card.sourceId) ?? '' })),
        ]
      : answers;
  const errors = results.filter((result) => result.error);
  const errorKey = searches
    .map((search, index) => `${sources[index]?.id}:${search.status}:${search.data?.error ?? ''}`)
    .join('|');
  useEffect(() => {
    searches.forEach((search, index) => {
      const id = sources[index]?.id;
      if (!id) return;
      if (search.isError || search.data?.error) failed(id);
      else if (search.isSuccess) worked(id);
    });
    // Reported once per change of the sources' answers.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [errorKey]);
  return (
    <div data-testid="ranked-results" className="flex flex-col gap-3">
      {answered < sources.length && (
        <p role="status" className="text-muted flex items-center gap-2 text-sm">
          <LoaderCircle aria-hidden size={14} className="animate-spin" />
          {t('search.sourcesAnswered', { done: answered, total: sources.length })}
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
          items={ranked.slice(0, limit ?? ranked.length).map(({ card, sourceName }) => ({
            ...toGridItems([card])[0]!,
            subtitle: sourceName,
          }))}
        />
      )}
      {errors.length > 0 && (
        <p className="text-muted text-xs wrap-anywhere">
          {t('search.someFailed', {
            sources: errors.map((result) => `${result.source.name} (${result.error})`).join(', '),
          })}
        </p>
      )}
    </div>
  );
};

const groupKey = (medium: Medium) => `raygun.search.grouped.${medium}`;

const readGrouped = (medium: Medium) => {
  try {
    return localStorage.getItem(groupKey(medium)) === '1';
  } catch {
    return false;
  }
};

const SourceChip: FC<{ label: string; pressed: boolean; onClick: () => void }> = ({
  label,
  pressed,
  onClick,
}) => (
  <button
    type="button"
    aria-pressed={pressed}
    onClick={onClick}
    className={
      pressed
        ? 'bg-primary/20 text-primary shrink-0 rounded-full px-4 py-1.5 text-sm font-semibold'
        : 'bg-surface shrink-0 rounded-full px-4 py-1.5 text-sm'
    }
  >
    {label}
  </button>
);

const SourceSearch: FC<{
  medium: Medium;
  sourceId: string;
  query: string;
  landscape?: boolean;
}> = ({ medium, sourceId, query, landscape = false }) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [values, setValues] = useState<FilterValues>({});
  const filters = useSourceFilters(medium, sourceId);
  const search = useFilteredSearch(medium, sourceId, query, values);
  const items = search.data?.pages.flatMap((page) => page.items) ?? [];
  const hasFilters = (filters.data?.length ?? 0) > 0;
  const active = Object.keys(values).length;

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
      {search.error && <SourceIssue>{search.error.message}</SourceIssue>}
      {search.isFetching && items.length === 0 && (
        <LoaderCircle aria-label={t('search.searching')} className="text-muted animate-spin" />
      )}
      {search.isSuccess && items.length === 0 && (
        <p className="text-muted text-sm">
          {query.trim() ? t('search.noResults', { query }) : t('search.providerEmpty')}
        </p>
      )}
      {items.length > 0 && (
        <MangaGrid medium={medium} landscape={landscape} items={toGridItems(items)} />
      )}
      <LoadMore
        hasNextPage={search.hasNextPage}
        fetching={search.isFetchingNextPage}
        onLoad={() => void search.fetchNextPage()}
      />
    </div>
  );
};

/** Sources' chips before they fold; chosen ones always show. */
const FOLDED_CHIPS = 12;

const chosenKey = (medium: Medium) => `raygun.search.sources.${medium}`;

const readChosen = (medium: Medium): string[] => {
  try {
    const saved: unknown = JSON.parse(localStorage.getItem(chosenKey(medium)) ?? '[]');
    return Array.isArray(saved) ? saved.filter((id): id is string => typeof id === 'string') : [];
  } catch {
    return [];
  }
};

/**
 * Search → Manga, Novels or Videos: sources picked with chips (several at once; none is every
 * source). Several sources search together; one source searches with its filters and tags.
 */
export const CatalogSearch: FC<{
  medium: Medium;
  query: string;
  /** One source only, without chips: a video source's own pill (M46.35). */
  only?: string;
  /** Sources with their own pill (YouTube, PeerTube…) among the chips too (M46.40). */
  withPills?: boolean;
}> = ({ medium, query, only, withPills = false }) => {
  const { t } = useTranslation();
  const listed = useMangaSources(medium);
  // A source with its own pill searches there, not among the rest (M46.35).
  const sources = {
    ...listed,
    data: listed.data?.filter((source) =>
      only ? source.id === only : withPills || !source.searchPill,
    ),
  };
  const [chosen, setChosenState] = useState(() => readChosen(medium));
  const [unfolded, setUnfolded] = useState(false);
  const [finding, setFinding] = useState('');
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
  // Sources removed since they were chosen don't count.
  const picked = only
    ? [only]
    : sources.data
      ? chosen.filter((id) => sources.data?.some((source) => source.id === id))
      : chosen;
  const sourceId = picked.length === 1 ? picked[0]! : '';

  const setChosen = (ids: string[]) => {
    setChosenState(ids);
    try {
      localStorage.setItem(chosenKey(medium), JSON.stringify(ids));
    } catch {
      // A per-device convenience.
    }
  };
  const toggle = (id: string) =>
    setChosen(picked.includes(id) ? picked.filter((other) => other !== id) : [...picked, id]);
  const all = sources.data ?? [];
  // Several sources: a row each, loading as they come into view (M46.7).
  const searched = failingLast(
    picked.length > 0 ? all.filter((source) => picked.includes(source.id)) : all,
    failures,
    failingAtEnd,
  );
  // Finding a source by name shows every match; otherwise the list folds after a dozen.
  const needle = finding.trim().toLowerCase();
  const shown = needle
    ? all.filter(
        (source) => source.name.toLowerCase().includes(needle) || picked.includes(source.id),
      )
    : unfolded || all.length <= FOLDED_CHIPS
      ? all
      : all.filter((source, index) => index < FOLDED_CHIPS || picked.includes(source.id));

  return (
    <div data-testid={`${medium}-search`} className="flex flex-col gap-4">
      {sources.data?.length === 0 && (
        <p className="text-muted text-sm">{t('search.noCatalogSources')}</p>
      )}
      {all.length > 0 && !only && (
        <div role="group" aria-label={t('search.sources')} className="flex flex-wrap gap-2">
          {all.length > FOLDED_CHIPS && (
            <label className="bg-surface flex items-center gap-2 rounded-full px-3 py-1.5">
              <Search aria-hidden size={14} className="text-muted" />
              <input
                type="search"
                value={finding}
                onChange={(event) => setFinding(event.target.value)}
                placeholder={t('search.findSource')}
                aria-label={t('search.findSource')}
                className="placeholder:text-muted w-32 bg-transparent text-sm outline-none"
              />
            </label>
          )}
          {/* Relevant (M46.35): every source's answers ranked as one list; the default. */}
          <SourceChip
            label={t('search.relevant')}
            pressed={picked.length === 0 && !grouped}
            onClick={() => {
              setChosen([]);
              setGrouped(false);
            }}
          />
          <SourceChip
            label={t('search.allSources')}
            pressed={picked.length === 0 && grouped}
            onClick={() => {
              setChosen([]);
              setGrouped(true);
            }}
          />
          {shown.map((source) => (
            <SourceChip
              key={source.id}
              label={source.name}
              pressed={picked.includes(source.id)}
              onClick={() => toggle(source.id)}
            />
          ))}
          {all.length > FOLDED_CHIPS && !needle && (
            <button
              type="button"
              aria-expanded={unfolded}
              onClick={() => setUnfolded(!unfolded)}
              className="text-primary shrink-0 rounded-full px-3 py-1.5 text-sm"
            >
              {unfolded
                ? t('search.fewerSources')
                : t('search.moreSources', { count: all.length - shown.length })}
            </button>
          )}
        </div>
      )}
      {sourceId === '' ? (
        <>
          <div className="flex items-center gap-3">
            {!query.trim() && all.length > 0 && (
              <h2 className="font-semibold">{t('search.popular')}</h2>
            )}
            {picked.length > 1 && (
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
            )}
          </div>
          {grouped || searched.length === 1 ? (
            searched.map((source) => (
              <LazySourceRow
                key={`${source.id}:${query.trim()}`}
                source={source}
                medium={medium}
                query={query}
                onMore={() => setChosen([source.id])}
              />
            ))
          ) : (
            <RankedResults sources={searched} medium={medium} query={query} />
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
        />
      )}
    </div>
  );
};
