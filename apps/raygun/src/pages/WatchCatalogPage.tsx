import { useNavigate, useSearch } from '@tanstack/react-router';
import { LoaderCircle } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { StremioCard } from '../watch/StremioRow';
import { useStremioCatalog } from '../watch/useStremio';
import { PageHeader } from './PageHeader';

/** One addon catalog as a grid, by genre, page after page. */
export const WatchCatalogPage: FC = () => {
  const { t } = useTranslation();
  const { addon, type, catalog, genre = '' } = useSearch({ from: '/watch/catalog' });
  const navigate = useNavigate({ from: '/watch/catalog' });
  const pages = useStremioCatalog(addon, type, catalog, genre || null);
  const first = pages.data?.pages[0];
  const items = pages.data?.pages.flatMap((page) => page.items) ?? [];
  // Some addons repeat items across pages.
  const seen = new Set<string>();
  const unique = items.filter((item) => !seen.has(item.id) && seen.add(item.id));
  const landscape = unique[0]?.posterShape === 'landscape';

  const setGenre = (next: string) =>
    void navigate({
      search: { addon, type, catalog, genre: next || undefined },
      replace: true,
    });

  return (
    <section data-testid="watch-catalog-page" className="flex flex-col gap-4">
      <div>
        <PageHeader
          title={first ? `${first.name} · ${t(`watch.kind.${type}`, { defaultValue: type })}` : ''}
        />
        {first && <p className="text-muted text-sm">{first.addonName}</p>}
      </div>
      {first && first.genres.length > 0 && (
        <div
          role="tablist"
          aria-label={t('watch.genres')}
          className="-mx-4 flex gap-2 overflow-x-auto px-4 lg:mx-0 lg:px-0"
        >
          {['', ...first.genres].map((option) => (
            <button
              key={option || 'all'}
              type="button"
              role="tab"
              aria-selected={option === genre}
              onClick={() => setGenre(option)}
              className={`shrink-0 rounded-full px-4 py-1.5 text-sm ${option === genre ? 'bg-primary text-background font-semibold' : 'bg-surface'}`}
            >
              {option || t('watch.allGenres')}
            </button>
          ))}
        </div>
      )}
      {pages.error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {pages.error.message}
        </p>
      )}
      {pages.isPending && (
        <LoaderCircle aria-label={t('watch.loading')} className="text-muted mx-auto animate-spin" />
      )}
      {pages.data && unique.length === 0 && (
        <p className="text-muted text-sm">{t('watch.catalogEmpty')}</p>
      )}
      <ul
        className={`grid gap-3 ${landscape ? 'grid-cols-2 sm:grid-cols-3' : 'grid-cols-3 sm:grid-cols-4 lg:grid-cols-6'}`}
      >
        {unique.map((item) => (
          <li key={item.id}>
            <StremioCard item={item} />
          </li>
        ))}
      </ul>
      {pages.hasNextPage && (
        <button
          type="button"
          disabled={pages.isFetchingNextPage}
          onClick={() => void pages.fetchNextPage()}
          className="bg-surface mx-auto flex items-center gap-2 rounded-full px-5 py-2 text-sm disabled:opacity-60"
        >
          {pages.isFetchingNextPage && (
            <LoaderCircle aria-hidden size={16} className="animate-spin" />
          )}
          {t('search.more')}
        </button>
      )}
    </section>
  );
};
