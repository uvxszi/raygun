import { useSearch } from '@tanstack/react-router';
import {
  Check,
  CheckCheck,
  Copy,
  Globe,
  LoaderCircle,
  Plus,
  RefreshCw,
  Trash2,
} from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { TopBar } from '../shell/TopBar';
import { PinButton } from '../shell/PinButton';
import { OverflowMenu } from '../shell/OverflowMenu';

import { EpisodeItem } from '../podcasts/EpisodeItem';
import { EpisodeList } from '../podcasts/EpisodeList';
import { EpisodeOrderButton } from '../podcasts/EpisodeOrderButton';
import { PodcastArt } from '../podcasts/PodcastArt';
import { ShowNotes } from '../podcasts/ShowNotes';
import { ShowSpeedButton } from '../podcasts/ShowSpeedButton';
import {
  useOpenPodcastPage,
  usePodcast,
  usePodcastEvents,
  useRefreshPodcast,
  useRemoveDownloads,
  useSetAllPlayed,
  useSetEpisodeOrder,
  useSetSubscribed,
} from '../podcasts/usePodcasts';
import { SourceIssue } from '../shell/SourceIssue';
import { useSearchMode } from '../shell/useSearchMode';
import { matchesSearch } from '../shell/useListSearch';
import { usePlayContext } from '../music/playContext';

const copy = (text: string) => void navigator.clipboard?.writeText(text).catch(() => undefined);

/**
 * A podcast: subscribe, read about it, set its speed, and play or download its episodes, sorted
 * and filtered its own way (M46.48).
 */
export const PodcastPage: FC = () => {
  const { t } = useTranslation();
  const find = useSearchMode(t('podcasts.findEpisode'));
  usePodcastEvents();
  const { id } = useSearch({ from: '/podcast' });
  const podcast = usePodcast(id);
  const refresh = useRefreshPodcast();
  const setSubscribed = useSetSubscribed();
  const setOrder = useSetEpisodeOrder();
  const setAll = useSetAllPlayed();
  const removeDownloads = useRemoveDownloads();
  const openPage = useOpenPodcastPage();
  const view = podcast.data;
  usePlayContext(
    view
      ? {
          kind: 'podcast',
          name: view.title,
          subtitle: view.author,
          artworkUrl: view.artworkUrl,
          to: '/podcast',
          search: { id },
        }
      : null,
  );

  if (podcast.error && !view) {
    return (
      <section data-testid="podcast-page">
        <SourceIssue onRetry={() => void podcast.refetch()}>{podcast.error.message}</SourceIssue>
      </section>
    );
  }
  if (!view) {
    return (
      <section data-testid="podcast-page" className="grid place-items-center py-12">
        <LoaderCircle aria-label={t('podcasts.loading')} className="text-muted animate-spin" />
      </section>
    );
  }

  const shown = view.episodes.filter((episode) =>
    matchesSearch(find.query, [episode.title, episode.description]),
  );
  const filtered = view.order.filters.length > 0 || find.query.trim() !== '';
  const error =
    refresh.error ??
    setSubscribed.error ??
    setOrder.error ??
    setAll.error ??
    removeDownloads.error ??
    openPage.error;

  return (
    <section data-testid="podcast-page" className="flex flex-col gap-5">
      <TopBar
        back
        fallback="/library"
        plainTitle
        title={t('library.media.podcast')}
        search={find.search}
        actions={
          <>
            {find.action}
            <EpisodeOrderButton
              order={view.order}
              custom={view.customOrder}
              book={false}
              onChange={(order) => setOrder.mutate({ id: view.id, order })}
            />
            <PinButton
              target={{
                kind: 'podcast',
                to: '/podcast',
                search: { id: view.id },
                name: view.title,
                artworkUrl: view.artworkUrl ?? null,
              }}
            />
            <OverflowMenu
              items={[
                {
                  label: t('podcasts.menu.refresh'),
                  icon: RefreshCw,
                  onSelect: () => refresh.mutate(view.id),
                },
                {
                  label: t('podcasts.markAllPlayed'),
                  icon: CheckCheck,
                  onSelect: () => setAll.mutate({ id: view.id, played: true }),
                },
                {
                  label: t('podcasts.markAllUnplayed'),
                  icon: Check,
                  onSelect: () => setAll.mutate({ id: view.id, played: false }),
                },
                {
                  label: t('podcasts.removeDownloads'),
                  icon: Trash2,
                  onSelect: () => removeDownloads.mutate(view.id),
                },
                ...(view.link
                  ? [
                      {
                        label: t('podcasts.openWebsite'),
                        icon: Globe,
                        onSelect: () => view.link && openPage.mutate(view.link),
                      },
                    ]
                  : []),
                {
                  label: t('podcasts.menu.copyFeed'),
                  icon: Copy,
                  onSelect: () => copy(view.feedUrl),
                },
              ]}
            />
          </>
        }
      />
      <header className="flex gap-4">
        <PodcastArt url={view.artworkUrl} className="w-28 shrink-0 rounded-lg shadow-lg sm:w-40" />
        <div className="flex min-w-0 flex-col gap-2">
          <h1 className="text-xl font-bold tracking-tight wrap-anywhere">{view.title}</h1>
          {view.author && <p className="text-muted text-sm">{view.author}</p>}
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              aria-pressed={view.subscribed}
              disabled={setSubscribed.isPending}
              onClick={() => setSubscribed.mutate({ id: view.id, subscribed: !view.subscribed })}
              className={`flex items-center gap-1.5 rounded-full px-4 py-1.5 text-sm font-semibold disabled:opacity-60 ${view.subscribed ? 'bg-surface' : 'bg-primary text-background'}`}
            >
              {view.subscribed ? <Check aria-hidden size={16} /> : <Plus aria-hidden size={16} />}
              {t(view.subscribed ? 'podcasts.subscribed' : 'podcasts.subscribe')}
            </button>
            <ShowSpeedButton id={view.id} speed={view.speed} />
            <button
              type="button"
              aria-label={t('podcasts.refresh')}
              disabled={refresh.isPending}
              onClick={() => refresh.mutate(view.id)}
              className="bg-surface grid size-8 place-items-center rounded-full disabled:opacity-60"
            >
              <RefreshCw
                aria-hidden
                size={14}
                className={refresh.isPending ? 'animate-spin' : ''}
              />
            </button>
          </div>
        </div>
      </header>
      {view.description && <ShowNotes text={view.description} clamp />}
      {error && <SourceIssue>{error.message}</SourceIssue>}
      <section aria-labelledby="podcast-episodes" className="flex flex-col gap-2">
        <h2 id="podcast-episodes" className="font-semibold">
          {filtered && shown.length !== view.episodeCount
            ? t('podcasts.episodesShown', { shown: shown.length, count: view.episodeCount })
            : t('podcasts.episodes', { count: view.episodeCount })}
        </h2>
        {view.episodeCount === 0 ? (
          <p className="text-muted text-sm">{t('podcasts.noEpisodes')}</p>
        ) : shown.length === 0 ? (
          <div className="flex flex-col items-start gap-2">
            <p className="text-muted text-sm">{t('podcasts.noneShown')}</p>
            {view.order.filters.length > 0 && (
              <button
                type="button"
                onClick={() =>
                  setOrder.mutate({ id: view.id, order: { ...view.order, filters: [] } })
                }
                className="bg-surface rounded-full px-4 py-1.5 text-sm"
              >
                {t('podcasts.showAll')}
              </button>
            )}
          </div>
        ) : (
          <EpisodeList episodes={shown}>
            {(episode) => <EpisodeItem key={episode.id} episode={episode} />}
          </EpisodeList>
        )}
      </section>
    </section>
  );
};
