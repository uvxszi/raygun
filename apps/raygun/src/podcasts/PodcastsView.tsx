import { Link, useNavigate } from '@tanstack/react-router';
import { Podcast, RefreshCw, Rss, Search } from 'lucide-react';
import { useMemo, useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { AudioViewChips } from '../library/AudioViewChips';
import { LibraryTop } from '../library/LibraryTop';
import { EmptyState } from '../shell/EmptyState';
import { Sheet } from '../shell/Sheet';
import { TopBarAction } from '../shell/TopBar';

import { EpisodeItem } from './EpisodeItem';
import { PodcastArt } from './PodcastArt';
import {
  useLatestEpisodes,
  useOpenFeed,
  usePodcastEvents,
  useRefreshAll,
  useSubscriptions,
} from './usePodcasts';
import { Carousel } from '../shell/Carousel';

const matches = (text: string, query: string) =>
  text.toLocaleLowerCase().includes(query.toLocaleLowerCase());

/**
 * Library → Audio → Podcasts: the shows you follow and their new episodes. Finding shows happens in
 * Search; the library's search only narrows what you follow.
 */
export const PodcastsView: FC = () => {
  const { t } = useTranslation();
  usePodcastEvents();
  const navigate = useNavigate();
  const subscriptions = useSubscriptions();
  const latest = useLatestEpisodes();
  const refreshAll = useRefreshAll();
  const openFeed = useOpenFeed();
  const [searching, setSearching] = useState(false);
  const [query, setQuery] = useState('');
  const [addingFeed, setAddingFeed] = useState(false);
  const [feedUrl, setFeedUrl] = useState('');

  const filter = query.trim();
  const shows = useMemo(
    () =>
      (subscriptions.data ?? []).filter(
        (podcast) =>
          !filter || matches(podcast.title, filter) || matches(podcast.author ?? '', filter),
      ),
    [subscriptions.data, filter],
  );
  const episodes = useMemo(
    () =>
      (latest.data ?? []).filter(
        (episode) =>
          !filter || matches(episode.title, filter) || matches(episode.podcastTitle, filter),
      ),
    [latest.data, filter],
  );
  const onAddFeed = (event: FormEvent) => {
    event.preventDefault();
    if (!feedUrl.trim()) return;
    openFeed.mutate(feedUrl.trim(), {
      onSuccess: (view) => void navigate({ to: '/podcast', search: { id: view.id } }),
    });
  };
  const hasSubscriptions = (subscriptions.data?.length ?? 0) > 0;

  return (
    <div data-testid="podcasts-view" className="flex flex-col gap-4">
      <LibraryTop
        media="music"
        search={
          searching
            ? {
                value: query,
                onChange: setQuery,
                placeholder: t('podcasts.filterPlaceholder'),
                onClose: () => {
                  setSearching(false);
                  setQuery('');
                },
              }
            : null
        }
        actions={
          hasSubscriptions && (
            <>
              {!searching && (
                <TopBarAction
                  label={t('library.filter.search')}
                  icon={Search}
                  onClick={() => setSearching(true)}
                />
              )}
              <TopBarAction
                label={t('podcasts.refresh')}
                icon={RefreshCw}
                disabled={refreshAll.isPending}
                onClick={() => refreshAll.mutate(undefined)}
              />
            </>
          )
        }
        menu={[{ label: t('podcasts.openFeed'), icon: Rss, onSelect: () => setAddingFeed(true) }]}
      />
      <AudioViewChips view="podcasts" />
      {subscriptions.data?.length === 0 && (
        <EmptyState
          icon={Podcast}
          title={t('podcasts.emptyTitle')}
          body={t('podcasts.empty')}
          action={{
            label: t('library.findInSearch'),
            icon: Search,
            to: '/browse',
            search: { tab: 'search', type: 'podcast' },
          }}
        />
      )}
      {hasSubscriptions && filter && shows.length === 0 && episodes.length === 0 && (
        <p className="text-muted text-sm">{t('library.filter.noMatches')}</p>
      )}
      {shows.length > 0 && (
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
          <Carousel className="-mx-4 gap-3 px-4 lg:mx-0 lg:px-0">
            {shows.map((podcast) => (
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
          </Carousel>
        </section>
      )}
      {hasSubscriptions && episodes.length > 0 && (
        <section aria-labelledby="podcast-latest" className="flex flex-col gap-2">
          <h2 id="podcast-latest" className="font-semibold">
            {t('podcasts.newEpisodes')}
          </h2>
          <ul className="bg-surface flex flex-col gap-4 rounded-xl px-4 py-3">
            {episodes.map((episode) => (
              <EpisodeItem key={episode.id} episode={episode} showPodcast />
            ))}
          </ul>
        </section>
      )}
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
              disabled={!feedUrl.trim() || openFeed.isPending}
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
