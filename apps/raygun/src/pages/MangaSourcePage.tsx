import type { MangaBrowseMode } from '@raygun/bindings';
import { useNavigate, useSearch } from '@tanstack/react-router';
import { LoaderCircle, Search } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { MangaGrid } from '../manga/MangaGrid';
import { ROUTES } from '../manga/medium';
import type { Medium } from '../manga/medium';
import { useMangaBrowse, useMangaSources } from '../manga/useManga';
import { PageHeader } from './PageHeader';
import { LoadMore } from '../shell/LoadMore';
import { SourceIssue } from '../shell/SourceIssue';

const LISTS: MangaBrowseMode[] = ['popular', 'latest'];

type SourceSearch = { id?: string; mode?: MangaBrowseMode; q?: string };

/** One source's catalogue: popular, latest or search results. */
export const MangaSourcePage: FC<{ medium?: Medium }> = ({ medium = 'manga' }) => {
  const { t } = useTranslation();
  const search: SourceSearch = useSearch({ strict: false });
  const { id = '', mode = 'popular', q = '' } = search;
  const navigate = useNavigate();
  const sources = useMangaSources(medium);
  const source = sources.data?.find((candidate) => candidate.id === id);
  const browse = useMangaBrowse(id, mode, q, medium);
  const [input, setInput] = useState(q);
  const show = (next: SourceSearch) =>
    void navigate({ to: ROUTES[medium].source, search: { id, ...next }, replace: true });

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const query = input.trim();
    show(query ? { mode: 'search', q: query } : { mode: 'popular' });
  };

  const items = browse.data?.pages.flatMap((page) => page.items) ?? [];

  return (
    <section data-testid="manga-source-page" className="flex flex-col gap-4">
      <PageHeader back fallback="/browse" title={source?.name ?? t('manga.source')} />
      <form onSubmit={submit} className="bg-surface flex items-center gap-3 rounded-xl px-4 py-3">
        <Search aria-hidden size={20} className="text-muted" />
        <input
          type="search"
          enterKeyHint="search"
          value={input}
          onChange={(event) => setInput(event.target.value)}
          placeholder={t('manga.searchPlaceholder')}
          aria-label={t('manga.search')}
          className="placeholder:text-muted w-full bg-transparent outline-none"
        />
      </form>
      <nav className="flex gap-2" aria-label={t('manga.lists')}>
        {LISTS.map((list) => (
          <button
            key={list}
            type="button"
            aria-pressed={mode === list}
            onClick={() => {
              setInput('');
              show({ mode: list });
            }}
            className={
              mode === list
                ? 'bg-primary/20 text-primary rounded-full px-4 py-1.5 text-sm font-semibold'
                : 'bg-surface rounded-full px-4 py-1.5 text-sm'
            }
          >
            {t(`manga.list.${list}`)}
          </button>
        ))}
      </nav>
      {browse.error && <SourceIssue>{browse.error.message}</SourceIssue>}
      {browse.isPending && browse.fetchStatus === 'fetching' && (
        <LoaderCircle
          aria-label={t('manga.loading')}
          className="text-muted animate-spin self-center"
        />
      )}
      {browse.isSuccess && items.length === 0 && (
        <p className="text-muted text-sm">{t('manga.nothingFound')}</p>
      )}
      <MangaGrid items={items} medium={medium} />
      <LoadMore
        hasNextPage={browse.hasNextPage}
        fetching={browse.isFetchingNextPage}
        onLoad={() => void browse.fetchNextPage()}
      />
    </section>
  );
};

export const NovelSourcePage: FC = () => <MangaSourcePage medium="novel" />;

export const VideoSourcePage: FC = () => <MangaSourcePage medium="video" />;
