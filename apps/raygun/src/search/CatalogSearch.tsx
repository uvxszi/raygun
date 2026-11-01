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
      {!query.trim() && active === 0 && (
        <p className="text-muted text-sm">
          {t(hasFilters ? 'search.filters.typeOrFilter' : 'search.typeToSearch')}
        </p>
      )}
      {search.error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {search.error.message}
        </p>
      )}
      {search.isFetching && items.length === 0 && (
        <LoaderCircle aria-label={t('search.searching')} className="text-muted animate-spin" />
      )}
      {search.isSuccess && items.length === 0 && (
        <p className="text-muted text-sm">{t('search.noResults', { query })}</p>
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

/** Search → Manga or Novels: every source at once, or one source with its filters and tags. */
export const CatalogSearch: FC<{ medium: Medium; query: string }> = ({ medium, query }) => {
  const { t } = useTranslation();
  const sources = useMangaSources(medium);
  const [sourceId, setSourceId] = useState('');
  const global = useGlobalSearch(medium, query, sourceId === '');

  return (
    <div data-testid={`${medium}-search`} className="flex flex-col gap-4">
      {sources.data?.length === 0 && (
        <p className="text-muted text-sm">{t('search.noCatalogSources')}</p>
      )}
      {(sources.data?.length ?? 0) > 0 && (
        <label className="flex items-center gap-2 text-sm">
          <span className="text-muted">{t('search.source')}</span>
          <select
            value={sourceId}
            onChange={(event) => setSourceId(event.target.value)}
            className="bg-surface min-w-0 flex-1 rounded-lg px-3 py-2 outline-none"
          >
            <option value="">{t('search.allSources')}</option>
            {sources.data?.map((source) => (
              <option key={source.id} value={source.id}>
                {source.name}
              </option>
            ))}
          </select>
        </label>
      )}
      {sourceId === '' ? (
        <>
          {!query.trim() && <p className="text-muted text-sm">{t('search.typeToSearch')}</p>}
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
              onMore={() => setSourceId(result.source.id)}
            />
          ))}
        </>
      ) : (
        <SourceSearch key={sourceId} medium={medium} sourceId={sourceId} query={query} />
      )}
    </div>
  );
};
