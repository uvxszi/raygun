import { useNavigate, useSearch } from '@tanstack/react-router';
import { LoaderCircle, Search } from 'lucide-react';
import { useEffect, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';
import { useIsFetching } from '@tanstack/react-query';

import { PodcastResultList } from '../podcasts/PodcastResultList';
import { usePodcastSearch, useTopPodcasts } from '../podcasts/usePodcasts';
import { AllResults } from '../search/AllResults';
import { CatalogSearch } from '../search/CatalogSearch';
import { LibraryMatches } from '../search/LibraryMatches';
import { MusicResults } from '../search/MusicResults';
import { SEARCH_TYPES } from '../search/searchTypes';
import { VideoResults } from '../search/VideoResults';
import type { SearchType } from '../search/searchTypes';

/** Wait for a pause in typing before searching, so each keystroke doesn't hit the network. */
export const SEARCH_DEBOUNCE_MS = 400;

/** Podcasts: the directory's results, or its top shows before anything is typed. */
const PodcastSearch: FC<{ query: string }> = ({ query }) => {
  const { t } = useTranslation();
  const found = usePodcastSearch(query);
  const top = useTopPodcasts(query === '');
  const search = query ? found : top;
  return (
    <div data-testid="podcast-search" className="flex flex-col gap-3">
      {!query && <h2 className="font-semibold">{t('podcasts.topShows')}</h2>}
      {search.error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {search.error.message}
        </p>
      )}
      {search.data?.length === 0 && query && (
        <p className="text-muted text-sm">{t('podcasts.noResults')}</p>
      )}
      {search.data && search.data.length > 0 && <PodcastResultList results={search.data} />}
    </div>
  );
};

/** Search: everything at once, or one kind of media (with its sources' filters and tags). */
export const SearchPage: FC = () => {
  const { t } = useTranslation();
  const { q = '', type = 'all' } = useSearch({ from: '/search' });
  const navigate = useNavigate({ from: '/search' });
  const [input, setInput] = useState(q);
  const fetching = useIsFetching() > 0;

  // The query lives in the URL, so Back to this page shows the same results.
  useEffect(() => {
    if (input.trim() === q) {
      return;
    }
    const timer = setTimeout(() => {
      const next = input.trim();
      void navigate({
        search: { q: next || undefined, type: type === 'all' ? undefined : type },
        replace: true,
      });
    }, SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [input, q, type, navigate]);

  const setType = (next: SearchType) =>
    void navigate({
      search: { q: q || undefined, type: next === 'all' ? undefined : next },
      replace: true,
    });
  // Manga, novel and video sources browse with filters alone, and podcasts show top shows.
  const needsQuery = type !== 'manga' && type !== 'novel' && type !== 'video' && type !== 'podcast';

  return (
    <section data-testid="search-page" className="flex flex-col gap-4">
      <header className="bg-background sticky top-0 z-30 -mx-4 flex min-h-16 items-center px-3 py-2 lg:-mx-10 lg:px-8">
        <h1 className="sr-only">{t('search.title')}</h1>
        <label className="bg-surface flex min-w-0 flex-1 items-center gap-3 rounded-full px-5 py-3">
          <Search aria-hidden size={20} className="text-muted shrink-0" />
          <input
            type="search"
            enterKeyHint="search"
            value={input}
            onChange={(event) => setInput(event.target.value)}
            placeholder={t(`search.placeholderFor.${type}`)}
            aria-label={t('search.title')}
            className="placeholder:text-muted min-w-0 flex-1 bg-transparent outline-none"
          />
          {fetching && q && (
            <LoaderCircle
              aria-label={t('search.searching')}
              role="status"
              size={20}
              className="text-muted shrink-0 animate-spin"
            />
          )}
        </label>
      </header>
      <div
        role="tablist"
        aria-label={t('search.typeLabel')}
        className="-mx-4 flex gap-2 overflow-x-auto px-4 lg:mx-0 lg:px-0"
      >
        {SEARCH_TYPES.map((option) => (
          <button
            key={option}
            type="button"
            role="tab"
            aria-selected={type === option}
            onClick={() => setType(option)}
            className={`shrink-0 rounded-full px-4 py-1.5 text-sm ${type === option ? 'bg-primary/20 text-primary font-semibold' : 'bg-surface'}`}
          >
            {t(`search.type.${option}`)}
          </button>
        ))}
      </div>
      {!q && needsQuery && <p className="text-muted text-sm">{t(`search.hintFor.${type}`)}</p>}
      {q && type === 'all' && <AllResults query={q} onType={setType} />}
      {q && type === 'music' && <MusicResults query={q} />}
      {type === 'podcast' && <PodcastSearch query={q} />}
      {type === 'video' && (
        <>
          {q && <VideoResults query={q} />}
          <section aria-label={t('search.fromSources')} className="flex flex-col gap-3">
            <h2 className="font-semibold">{t('search.fromSources')}</h2>
            <CatalogSearch key="video" medium="video" query={q} />
          </section>
        </>
      )}
      {q && type === 'library' && <LibraryMatches query={q} />}
      {(type === 'manga' || type === 'novel') && (
        <CatalogSearch key={type} medium={type} query={q} />
      )}
    </section>
  );
};
