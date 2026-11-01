import type { PodcastSearchResult } from '@raygun/bindings';
import { Link, useNavigate } from '@tanstack/react-router';
import { LoaderCircle, RefreshCw, Search } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { EpisodeItem } from './EpisodeItem';
import { PodcastArt } from './PodcastArt';
import {
  useLatestEpisodes,
  useOpenFeed,
  usePodcastEvents,
  usePodcastSearch,
  useRefreshAll,
  useSubscriptions,
  useTopPodcasts,
} from './usePodcasts';

const ResultList: FC<{
  results: PodcastSearchResult[];
  onOpen: (feedUrl: string) => void;
  opening: string | null;
}> = ({ results, onOpen, opening }) => {
  const { t } = useTranslation();
  return (
    <ul className="bg-surface flex flex-col rounded-xl">
      {results.map((result) => (
        <li key={result.feedUrl}>
          <button
            type="button"
            onClick={() => onOpen(result.feedUrl)}
            disabled={opening !== null}
            className="flex w-full items-center gap-3 px-4 py-3 text-left disabled:opacity-70"
          >
            <PodcastArt url={result.artworkUrl} className="w-12 shrink-0 rounded-md" />
            <span className="flex min-w-0 flex-1 flex-col">
              <span className="truncate font-medium">{result.title}</span>
              <span className="text-muted truncate text-xs">
                {[result.author, result.subscribed ? t('podcasts.subscribed') : null]
                  .filter(Boolean)
                  .join(' · ')}
              </span>
            </span>
            {opening === result.feedUrl && (
              <LoaderCircle aria-label={t('podcasts.opening')} size={18} className="animate-spin" />
            )}
          </button>
        </li>
      ))}
    </ul>
  );
};

/** Library → Music → Podcasts: subscriptions, new episodes and finding new shows. */
export const PodcastsView: FC = () => {
  const { t } = useTranslation();
  usePodcastEvents();
  const navigate = useNavigate();
  const subscriptions = useSubscriptions();
  const latest = useLatestEpisodes();
  const refreshAll = useRefreshAll();
  const openFeed = useOpenFeed();
  const [draft, setDraft] = useState('');
  const [query, setQuery] = useState('');
  const [feedUrl, setFeedUrl] = useState('');
  const search = usePodcastSearch(query);
  const top = useTopPodcasts(query === '');
  const [opening, setOpening] = useState<string | null>(null);

  const open = (url: string) => {
    setOpening(url);
    openFeed.mutate(url, {
      onSuccess: (view) => void navigate({ to: '/podcast', search: { id: view.id } }),
      onSettled: () => setOpening(null),
    });
  };
  const onSearch = (event: FormEvent) => {
    event.preventDefault();
    setQuery(draft.trim());
  };
  const onAddFeed = (event: FormEvent) => {
    event.preventDefault();
    if (feedUrl.trim()) open(feedUrl.trim());
  };
  const results = query ? search : top;
  const hasSubscriptions = (subscriptions.data?.length ?? 0) > 0;

  return (
    <div data-testid="podcasts-view" className="flex flex-col gap-5">
      {hasSubscriptions && (
        <section aria-labelledby="podcast-subscriptions" className="flex flex-col gap-2">
          <div className="flex items-center justify-between">
            <h2 id="podcast-subscriptions" className="font-semibold">
              {t('podcasts.subscriptions')}
            </h2>
            <button
              type="button"
              onClick={() => refreshAll.mutate(undefined)}
              disabled={refreshAll.isPending}
              className="text-muted flex items-center gap-1.5 text-sm disabled:opacity-60"
            >
              <RefreshCw
                aria-hidden
                size={14}
                className={refreshAll.isPending ? 'animate-spin' : ''}
              />
              {t('podcasts.refresh')}
            </button>
          </div>
          <ul className="-mx-4 flex gap-3 overflow-x-auto px-4 lg:mx-0 lg:px-0">
            {subscriptions.data?.map((podcast) => (
              <li key={podcast.id} className="w-28 shrink-0">
                <Link
                  to="/podcast"
                  search={{ id: podcast.id }}
                  aria-label={podcast.title}
                  className="flex flex-col gap-1"
                >
                  <PodcastArt url={podcast.artworkUrl} className="w-28 rounded-lg" />
                  <span className="line-clamp-2 text-sm">{podcast.title}</span>
                  {podcast.unplayed > 0 && (
                    <span className="text-muted text-xs">
                      {t('podcasts.unplayed', { count: podcast.unplayed })}
                    </span>
                  )}
                </Link>
              </li>
            ))}
          </ul>
        </section>
      )}
      {hasSubscriptions && (latest.data?.length ?? 0) > 0 && (
        <section aria-labelledby="podcast-latest" className="flex flex-col gap-2">
          <h2 id="podcast-latest" className="font-semibold">
            {t('podcasts.newEpisodes')}
          </h2>
          <ul className="bg-surface flex flex-col gap-4 rounded-xl px-4 py-3">
            {latest.data?.map((episode) => (
              <EpisodeItem key={episode.id} episode={episode} showPodcast />
            ))}
          </ul>
        </section>
      )}
      <section aria-labelledby="podcast-discover" className="flex flex-col gap-2">
        <h2 id="podcast-discover" className="font-semibold">
          {t('podcasts.discover')}
        </h2>
        {!hasSubscriptions && subscriptions.data && (
          <p className="text-muted text-sm">{t('podcasts.empty')}</p>
        )}
        <form
          onSubmit={onSearch}
          className="bg-surface flex items-center gap-2 rounded-lg px-3 py-2"
        >
          <Search aria-hidden size={16} className="text-muted" />
          <input
            type="search"
            value={draft}
            onChange={(event) => {
              setDraft(event.target.value);
              if (!event.target.value) setQuery('');
            }}
            placeholder={t('podcasts.searchPlaceholder')}
            aria-label={t('podcasts.search')}
            className="placeholder:text-muted min-w-0 flex-1 bg-transparent text-sm outline-none"
          />
        </form>
        <h3 className="text-muted text-sm">
          {query ? t('podcasts.resultsFor', { query }) : t('podcasts.topShows')}
        </h3>
        {results.isFetching && !results.data && (
          <LoaderCircle aria-label={t('podcasts.loading')} className="text-muted animate-spin" />
        )}
        {results.error && (
          <p role="alert" className="text-primary text-sm wrap-anywhere">
            {results.error.message}
          </p>
        )}
        {results.data?.length === 0 && query && (
          <p className="text-muted text-sm">{t('podcasts.noResults')}</p>
        )}
        {results.data && results.data.length > 0 && (
          <ResultList results={results.data} onOpen={open} opening={opening} />
        )}
        <form onSubmit={onAddFeed} className="flex gap-2">
          <input
            type="url"
            inputMode="url"
            autoCapitalize="off"
            autoCorrect="off"
            spellCheck={false}
            value={feedUrl}
            onChange={(event) => setFeedUrl(event.target.value)}
            placeholder={t('podcasts.feedPlaceholder')}
            aria-label={t('podcasts.feedUrl')}
            className="bg-surface placeholder:text-muted min-w-0 flex-1 rounded-lg px-3 py-2 text-sm outline-none"
          />
          <button
            type="submit"
            disabled={!feedUrl.trim() || opening !== null}
            className="bg-surface rounded-lg px-4 py-2 text-sm disabled:opacity-40"
          >
            {t('podcasts.openFeed')}
          </button>
        </form>
        {openFeed.error && (
          <p role="alert" className="text-primary text-sm wrap-anywhere">
            {openFeed.error.message}
          </p>
        )}
      </section>
    </div>
  );
};
