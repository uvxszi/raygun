import { commands } from '@raygun/bindings';
import type { MangaBrowseMode } from '@raygun/bindings';
import { useMutation } from '@tanstack/react-query';
import { useNavigate, useSearch } from '@tanstack/react-router';
import { Globe, LoaderCircle, Search, SlidersHorizontal } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { MangaGrid } from '../manga/MangaGrid';
import { SelectionScope } from '../select/Selectable';
import { ROUTES } from '../manga/medium';
import type { Medium } from '../manga/medium';
import { useMangaBrowse, useMangaSources } from '../manga/useManga';
import { SourceFiltersPanel } from '../search/SourceFiltersPanel';
import { useFilteredSearch, useSourceFilters } from '../search/useCatalogSearch';
import type { FilterValues } from '../search/useCatalogSearch';
import { LoadMore } from '../shell/LoadMore';
import { TextField } from '../shell/TextField';
import { SourceIssue } from '../shell/SourceIssue';
import { TopBarAction } from '../shell/TopBar';
import { AdultMark } from '../shell/AdultMark';
import { PageHeader } from './PageHeader';

const LISTS: MangaBrowseMode[] = ['popular', 'latest'];

type SourceSearch = { id?: string; mode?: MangaBrowseMode; q?: string };

const filtersKey = (medium: Medium, sourceId: string) =>
  `raygun.sourceFilters.${medium}.${sourceId}`;

/** The filters last used on a source this session (coming back from a title keeps them). */
const readFilters = (medium: Medium, sourceId: string): FilterValues => {
  try {
    const saved: unknown = JSON.parse(sessionStorage.getItem(filtersKey(medium, sourceId)) ?? '{}');
    return saved && typeof saved === 'object' ? (saved as FilterValues) : {};
  } catch {
    return {};
  }
};

/**
 * One source's catalogue (Mihon's browse screen): popular, latest, search, and the source's own
 * filters when it has them.
 */
export const MangaSourcePage: FC<{ medium?: Medium }> = ({ medium = 'manga' }) => {
  const { t } = useTranslation();
  const search: SourceSearch = useSearch({ strict: false });
  const { id = '', mode = 'popular', q = '' } = search;
  const navigate = useNavigate();
  const sources = useMangaSources(medium);
  const source = sources.data?.find((candidate) => candidate.id === id);
  const [input, setInput] = useState(q);
  const [values, setValuesState] = useState<FilterValues>(() => readFilters(medium, id));
  const [showingFilters, setShowingFilters] = useState(false);
  const filters = useSourceFilters(medium, id);
  const hasFilters = (filters.data?.length ?? 0) > 0;
  const filtered = hasFilters && Object.keys(values).length > 0;
  // Filters search the source (with the words typed, if any); otherwise the chosen list.
  const browse = useMangaBrowse(filtered ? '' : id, mode, q, medium);
  const filteredSearch = useFilteredSearch(medium, filtered ? id : '', q, values);
  const list = filtered ? filteredSearch : browse;
  const website = useMutation({
    mutationFn: () => unwrapResult(commands.catalogOpenPage(id, null)),
  });
  const show = (next: SourceSearch) =>
    void navigate({ to: ROUTES[medium].source, search: { id, ...next }, replace: true });
  const setValues = (next: FilterValues) => {
    setValuesState(next);
    try {
      sessionStorage.setItem(filtersKey(medium, id), JSON.stringify(next));
    } catch {
      // Kept for this visit only.
    }
  };

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const query = input.trim();
    show(query ? { mode: 'search', q: query } : { mode: 'popular' });
  };

  const items = list.data?.pages.flatMap((page) => page.items) ?? [];
  const chip = (pressed: boolean) =>
    pressed
      ? 'bg-primary/20 text-primary flex items-center gap-1.5 rounded-full px-4 py-1.5 text-sm font-semibold'
      : 'bg-surface flex items-center gap-1.5 rounded-full px-4 py-1.5 text-sm';

  return (
    <section data-testid="manga-source-page" className="flex flex-col gap-4">
      <PageHeader
        back
        fallback="/browse"
        title={source?.name ?? t('manga.source')}
        titleMark={source?.adult && <AdultMark />}
        actions={
          id && (
            <TopBarAction
              label={t('manga.openInBrowser')}
              icon={Globe}
              disabled={website.isPending}
              onClick={() => website.mutate()}
            />
          )
        }
      />
      <form onSubmit={submit}>
        <TextField
          type="search"
          value={input}
          onChange={setInput}
          icon={Search}
          iconSize={20}
          autoComplete="off"
          placeholder={t('manga.searchPlaceholder')}
          aria-label={t('manga.search')}
          className="bg-surface gap-3 rounded-xl py-1.5 pr-2 pl-4"
          inputClassName="py-1.5"
        />
      </form>
      <nav className="flex flex-wrap gap-2" aria-label={t('manga.lists')}>
        {LISTS.map((item) => (
          <button
            key={item}
            type="button"
            aria-pressed={!filtered && mode === item}
            onClick={() => {
              setInput('');
              setValues({});
              show({ mode: item });
            }}
            className={chip(!filtered && mode === item)}
          >
            {t(`manga.list.${item}`)}
          </button>
        ))}
        {hasFilters && (
          <button
            type="button"
            aria-expanded={showingFilters}
            onClick={() => setShowingFilters(!showingFilters)}
            className={chip(filtered || showingFilters)}
          >
            <SlidersHorizontal aria-hidden size={14} />
            {filtered
              ? t('search.filters.active', { count: Object.keys(values).length })
              : t('search.filters.title')}
          </button>
        )}
      </nav>
      {showingFilters && filters.data && (
        <SourceFiltersPanel filters={filters.data} values={values} onChange={setValues} />
      )}
      {website.error && (
        <SourceIssue compact source={source?.name}>
          {website.error.message}
        </SourceIssue>
      )}
      {list.error && (
        <SourceIssue source={source?.name} onRetry={() => void list.refetch()}>
          {list.error.message}
        </SourceIssue>
      )}
      {list.isPending && list.fetchStatus === 'fetching' && (
        <LoaderCircle
          aria-label={t('manga.loading')}
          className="text-muted animate-spin self-center"
        />
      )}
      {list.isSuccess && items.length === 0 && (
        <p className="text-muted text-sm">{t('manga.nothingFound')}</p>
      )}
      <SelectionScope>
        <MangaGrid items={items} medium={medium} landscape={Boolean(source?.searchPill)} />
        <LoadMore
          hasNextPage={list.hasNextPage}
          fetching={list.isFetchingNextPage}
          onLoad={() => void list.fetchNextPage()}
        />
      </SelectionScope>
    </section>
  );
};

export const NovelSourcePage: FC = () => <MangaSourcePage medium="novel" />;

export const VideoSourcePage: FC = () => <MangaSourcePage medium="video" />;
