import { Link, useNavigate } from '@tanstack/react-router';
import { LoaderCircle, Rss, RefreshCw, Search } from 'lucide-react';
import { useEffect, useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { LibraryTop } from '../library/LibraryTop';
import { Sheet } from '../shell/Sheet';
import { TopBarAction } from '../shell/TopBar';

import { EpisodeItem } from './EpisodeItem';
import { PodcastArt } from './PodcastArt';
import { PodcastResultList } from './PodcastResultList';
import {
  useLatestEpisodes,
  useOpenFeed,
  usePodcastEvents,
  usePodcastSearch,
  useRefreshAll,
  useSubscriptions,
  useTopPodcasts,
} from './usePodcasts';

/** Library → Music → Podcasts: subscriptions, new episodes and finding new shows. */
export const PodcastsView: FC = () => {
  const { t } = useTranslation();
  usePodcastEvents();
  const navigate = useNavigate();
  const subscriptions = useSubscriptions();
  const latest = useLatestEpisodes();
  const refreshAll = useRefreshAll();
  const openFeed = useOpenFeed();
  const [searching, setSearching] = useState(false);
  const [draft, setDraft] = useState('');
  const [query, setQuery] = useState('');
  const [addingFeed, setAddingFeed] = useState(false);
  const [feedUrl, setFeedUrl] = useState('');
  // Searches the directory once typing pauses, like the Search tab.
  useEffect(() => {
    const timer = setTimeout(() => setQuery(draft.trim()), 400);
    return () => clearTimeout(timer);
  }, [draft]);
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
  const onAddFeed = (event: FormEvent) => {
    event.preventDefault();
    if (feedUrl.trim()) open(feedUrl.trim());
  };
  const results = query ? search : top;
  const hasSubscriptions = (subscriptions.data?.length ?? 0) > 0;

  return (
    <div data-testid="podcasts-view" className="flex flex-col gap-5">
      <LibraryTop
        media="podcast"
        search={
          searching
            ? {
                value: draft,
                onChange: setDraft,
                placeholder: t('podcasts.searchPlaceholder'),
                onClose: () => {
                  setSearching(false);
                  setDraft('');
                },
              }
            : null
        }
        actions={
          <>
            {!searching && (
              <TopBarAction
                label={t('podcasts.search')}
                icon={Search}
                onClick={() => setSearching(true)}
              />
            )}
            {hasSubscriptions && (
              <TopBarAction
                label={t('podcasts.refresh')}
                icon={RefreshCw}
                disabled={refreshAll.isPending}
                onClick={() => refreshAll.mutate(undefined)}
              />
            )}
          </>
        }
        menu={[{ label: t('podcasts.openFeed'), icon: Rss, onSelect: () => setAddingFeed(true) }]}
      />
      {hasSubscriptions && (
        <section aria-labelledby="podcast-subscriptions" className="flex flex-col gap-2">
          <div className="flex items-center justify-between">
            <h2 id="podcast-subscriptions" className="font-semibold">
              {t('podcasts.subscriptions')}
            </h2>
            {refreshAll.isPending && (
              <RefreshCw
                role="status"
                aria-label={t('podcasts.refresh')}
                size={14}
                className="text-muted animate-spin"
              />
            )}
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
        {results.data && results.data.length > 0 && <PodcastResultList results={results.data} />}
        {openFeed.error && !addingFeed && (
          <p role="alert" className="text-primary text-sm wrap-anywhere">
            {openFeed.error.message}
          </p>
        )}
      </section>
      {addingFeed && (
        <Sheet title={t('podcasts.openFeed')} onClose={() => setAddingFeed(false)}>
          <form onSubmit={onAddFeed} className="flex flex-col gap-3 text-sm">
            <input
              type="url"
              inputMode="url"
              autoCapitalize="off"
              autoCorrect="off"
              spellCheck={false}
              autoFocus
              value={feedUrl}
              onChange={(event) => setFeedUrl(event.target.value)}
              placeholder={t('podcasts.feedPlaceholder')}
              aria-label={t('podcasts.feedUrl')}
              className="bg-background placeholder:text-muted rounded-xl px-4 py-3 outline-none"
            />
            {openFeed.error && (
              <p role="alert" className="text-primary wrap-anywhere">
                {openFeed.error.message}
              </p>
            )}
            <button
              type="submit"
              disabled={!feedUrl.trim() || opening !== null}
              className="bg-primary text-background self-end rounded-full px-5 py-2.5 font-semibold disabled:opacity-50"
            >
              {t('podcasts.openFeed')}
            </button>
          </form>
        </Sheet>
      )}
    </div>
  );
};
