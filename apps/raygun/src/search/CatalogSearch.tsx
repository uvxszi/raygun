import type { MangaCard, SourceSearchResult } from '@raygun/bindings';
import { ChevronRight, LoaderCircle, SlidersHorizontal } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { MangaGrid } from '../manga/MangaGrid';
import type { Medium } from '../manga/medium';
import { useMangaSources } from '../manga/useManga';
import { SourceFiltersPanel } from './SourceFiltersPanel';
import { useFilteredSearch, useGlobalSearch, useSourceFilters } from './useCatalogSearch';
import type { FilterValues } from './useCatalogSearch';

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
        <h3 className="text-muted text-sm font-medium">{result.source.name}</h3>
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
      {search.hasNextPage && (
        <button
          type="button"
          disabled={search.isFetchingNextPage}
          onClick={() => void search.fetchNextPage()}
          className="bg-surface w-fit rounded-full px-4 py-2 text-sm disabled:opacity-60"
        >
          {t('manga.loadMore')}
        </button>
      )}
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
  const global = useGlobalSearch(medium, query, sourceId === '', picked);

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
          {global.isFetching && !global.data && (
            <LoaderCircle aria-label={t('search.searching')} className="text-muted animate-spin" />
          )}
          {global.error && (
            <p role="alert" className="text-primary text-sm wrap-anywhere">
              {global.error.message}
            </p>
          )}
          {global.data?.map((result) => (
            <SourceRow
              key={result.source.id}
              result={result}
              medium={medium}
              onMore={() => setChosen([result.source.id])}
            />
          ))}
        </>
      ) : (
        <SourceSearch key={sourceId} medium={medium} sourceId={sourceId} query={query} />
      )}
    </div>
  );
};
