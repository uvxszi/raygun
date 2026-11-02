import { useEffect, useRef } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PodcastResultList } from '../podcasts/PodcastResultList';
import { usePodcastSearch, useTopPodcasts } from '../podcasts/usePodcasts';
import { AllResults } from './AllResults';
import { CatalogSearch } from './CatalogSearch';
import { LibraryMatches } from './LibraryMatches';
import { MusicResults } from './MusicResults';
import { SEARCH_TYPES } from './searchTypes';
import type { SearchType } from './searchTypes';
import { VideoResults } from './VideoResults';

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

/**
 * Browse → Search: everything at once, or one kind of media with its sources' filters and tags.
 * Manga, novel and video sources browse with filters alone, and podcasts show top shows, before
 * anything is typed. The query comes from Browse's search bar.
 */
export const SearchPanel: FC<{
  query: string;
  type: SearchType;
  onType: (type: SearchType) => void;
}> = ({ query, type, onType }) => {
  const { t } = useTranslation();
  // Music and the library need something typed; the rest show what's popular until then.
  const needsQuery = type === 'music' || type === 'library';
  const selected = useRef<HTMLButtonElement>(null);

  // A link can open a kind whose chip is off-screen (Library → Manga's "Find in Browse").
  useEffect(() => {
    // jsdom has no scrollIntoView.
    selected.current?.scrollIntoView?.({ block: 'nearest', inline: 'nearest' });
  }, [type]);

  return (
    <div data-testid="search-page" className="flex flex-col gap-4">
      <div
        role="tablist"
        aria-label={t('search.typeLabel')}
        className="-mx-4 flex gap-2 no-scrollbar overflow-x-auto px-4 lg:mx-0 lg:px-0"
      >
        {SEARCH_TYPES.map((option) => (
          <button
            key={option}
            ref={type === option ? selected : undefined}
            type="button"
            role="tab"
            aria-selected={type === option}
            onClick={() => onType(option)}
            className={`shrink-0 rounded-full px-4 py-1.5 text-sm ${type === option ? 'bg-primary/20 text-primary font-semibold' : 'bg-surface'}`}
          >
            {t(`search.type.${option}`)}
          </button>
        ))}
      </div>
      {!query && needsQuery && <p className="text-muted text-sm">{t(`search.hintFor.${type}`)}</p>}
      {type === 'all' && <AllResults query={query} onType={onType} />}
      {query && type === 'music' && <MusicResults query={query} />}
      {type === 'podcast' && <PodcastSearch query={query} />}
      {type === 'video' && (
        <>
          <VideoResults query={query} />
          <section aria-label={t('search.fromSources')} className="flex flex-col gap-3">
            <h2 className="font-semibold">{t('search.fromSources')}</h2>
            <CatalogSearch key="video" medium="video" query={query} />
          </section>
        </>
      )}
      {query && type === 'library' && <LibraryMatches query={query} />}
      {(type === 'manga' || type === 'novel') && (
        <CatalogSearch key={type} medium={type} query={query} />
      )}
    </div>
  );
};
