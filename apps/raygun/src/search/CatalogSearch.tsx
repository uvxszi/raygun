import type { MangaCard, MangaSource, SourceSearchResult } from '@raygun/bindings';
import { ChevronRight, LoaderCircle, SlidersHorizontal } from 'lucide-react';
import { useState } from 'react';
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
import { useFilteredSearch, useSourceFilters, useSourceSearch } from './useCatalogSearch';
import type { FilterValues } from './useCatalogSearch';
import { LoadMore } from '../shell/LoadMore';
import { useSettings } from '../settings/useSettings';

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
        <p role="alert" className="text-primary text-xs wrap-anywhere">
          {t('search.providerFailed', { message: result.error })}
        </p>
      ) : result.items.length === 0 ? (
        <p className="text-muted text-xs">{t('search.providerEmpty')}</p>
      ) : (
        <MangaGrid medium={medium} items={toGridItems(result.items.slice(0, limit))} />
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
  return (
    <div ref={ref} data-testid="source-row">
      {search.data ? (
        <SourceRow result={search.data} medium={medium} limit={limit} onMore={onMore} />
      ) : (
        <section aria-label={source.name} aria-busy={!search.error} className="flex flex-col gap-2">
          <h3 className="text-muted text-sm font-medium">{source.name}</h3>
          {search.error ? (
            <p role="alert" className="text-primary text-xs wrap-anywhere">
              {t('search.providerFailed', { message: search.error.message })}
            </p>
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

const SourceSearch: FC<{ medium: Medium; sourceId: string; query: string }> = ({
  medium,
  sourceId,
  query,
}) => {
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
      {search.error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {search.error.message}
        </p>
      )}
      {search.isFetching && items.length === 0 && (
        <LoaderCircle aria-label={t('search.searching')} className="text-muted animate-spin" />
      )}
      {search.isSuccess && items.length === 0 && (
        <p className="text-muted text-sm">
          {query.trim() ? t('search.noResults', { query }) : t('search.providerEmpty')}
        </p>
      )}
      {items.length > 0 && <MangaGrid medium={medium} items={toGridItems(items)} />}
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
export const CatalogSearch: FC<{ medium: Medium; query: string }> = ({ medium, query }) => {
  const { t } = useTranslation();
  const sources = useMangaSources(medium);
  const [chosen, setChosenState] = useState(() => readChosen(medium));
  const [unfolded, setUnfolded] = useState(false);
  // Sources removed since they were chosen don't count.
  const picked = sources.data
    ? chosen.filter((id) => sources.data.some((source) => source.id === id))
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
  const searched = picked.length > 0 ? all.filter((source) => picked.includes(source.id)) : all;
  const shown =
    unfolded || all.length <= FOLDED_CHIPS
      ? all
      : all.filter((source, index) => index < FOLDED_CHIPS || picked.includes(source.id));

  return (
    <div data-testid={`${medium}-search`} className="flex flex-col gap-4">
      {sources.data?.length === 0 && (
        <p className="text-muted text-sm">{t('search.noCatalogSources')}</p>
      )}
      {all.length > 0 && (
        <div role="group" aria-label={t('search.sources')} className="flex flex-wrap gap-2">
          <SourceChip
            label={t('search.allSources')}
            pressed={picked.length === 0}
            onClick={() => setChosen([])}
          />
          {shown.map((source) => (
            <SourceChip
              key={source.id}
              label={source.name}
              pressed={picked.includes(source.id)}
              onClick={() => toggle(source.id)}
            />
          ))}
          {all.length > FOLDED_CHIPS && (
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
          {!query.trim() && all.length > 0 && (
            <h2 className="font-semibold">{t('search.popular')}</h2>
          )}
          {searched.map((source) => (
            <LazySourceRow
              key={`${source.id}:${query.trim()}`}
              source={source}
              medium={medium}
              query={query}
              onMore={() => setChosen([source.id])}
            />
          ))}
        </>
      ) : (
        <SourceSearch key={sourceId} medium={medium} sourceId={sourceId} query={query} />
      )}
    </div>
  );
};
