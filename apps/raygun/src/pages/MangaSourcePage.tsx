import type { MangaBrowseMode } from '@raygun/bindings';
import { useNavigate, useSearch } from '@tanstack/react-router';
import { LoaderCircle, Search } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { MangaGrid } from '../manga/MangaGrid';
import { useMangaBrowse, useMangaSources } from '../manga/useManga';
import { PageHeader } from './PageHeader';

const LISTS: MangaBrowseMode[] = ['popular', 'latest'];

/** One source's catalogue: popular, latest or search results. */
export const MangaSourcePage: FC = () => {
  const { t } = useTranslation();
  const { id, mode = 'popular', q = '' } = useSearch({ from: '/manga/source' });
  const navigate = useNavigate({ from: '/manga/source' });
  const sources = useMangaSources();
  const source = sources.data?.find((candidate) => candidate.id === id);
  const browse = useMangaBrowse(id, mode, q);
  const [input, setInput] = useState(q);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const query = input.trim();
    void navigate({
      search: query ? { id, mode: 'search', q: query } : { id, mode: 'popular' },
      replace: true,
    });
  };

  const items = browse.data?.pages.flatMap((page) => page.items) ?? [];

  return (
    <section data-testid="manga-source-page" className="flex flex-col gap-4">
      <PageHeader title={source?.name ?? t('manga.source')} />
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
              void navigate({ search: { id, mode: list }, replace: true });
            }}
            className={
              mode === list
                ? 'bg-primary text-background rounded-full px-4 py-1.5 text-sm font-semibold'
                : 'bg-surface rounded-full px-4 py-1.5 text-sm'
            }
          >
            {t(`manga.list.${list}`)}
          </button>
        ))}
      </nav>
      {browse.error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {browse.error.message}
        </p>
      )}
      {browse.isPending && browse.fetchStatus === 'fetching' && (
        <LoaderCircle
          aria-label={t('manga.loading')}
          className="text-muted animate-spin self-center"
        />
      )}
      {browse.isSuccess && items.length === 0 && (
        <p className="text-muted text-sm">{t('manga.nothingFound')}</p>
      )}
      <MangaGrid items={items} />
      {browse.hasNextPage && (
        <button
          type="button"
          disabled={browse.isFetchingNextPage}
          onClick={() => void browse.fetchNextPage()}
          className="bg-surface flex items-center gap-2 self-center rounded-full px-5 py-2 text-sm disabled:opacity-60"
        >
          {browse.isFetchingNextPage && (
            <LoaderCircle aria-hidden size={16} className="animate-spin" />
          )}
          {t('manga.loadMore')}
        </button>
      )}
    </section>
  );
};
