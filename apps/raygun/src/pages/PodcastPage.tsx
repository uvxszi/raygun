import { useSearch } from '@tanstack/react-router';
import { Check, LoaderCircle, Plus, RefreshCw } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { EpisodeItem } from '../podcasts/EpisodeItem';
import { PodcastArt } from '../podcasts/PodcastArt';
import {
  usePodcast,
  usePodcastEvents,
  useRefreshPodcast,
  useSetSubscribed,
} from '../podcasts/usePodcasts';

/** A podcast: subscribe, read about it, and play or download its episodes. */
export const PodcastPage: FC = () => {
  const { t } = useTranslation();
  usePodcastEvents();
  const { id } = useSearch({ from: '/podcast' });
  const podcast = usePodcast(id);
  const refresh = useRefreshPodcast();
  const setSubscribed = useSetSubscribed();
  const [expanded, setExpanded] = useState(false);
  const view = podcast.data;

  if (podcast.error && !view) {
    return (
      <section data-testid="podcast-page">
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {podcast.error.message}
        </p>
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

  return (
    <section data-testid="podcast-page" className="flex flex-col gap-5">
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
      {view.description && (
        <button
          type="button"
          onClick={() => setExpanded(!expanded)}
          aria-expanded={expanded}
          className={`text-muted text-left text-sm whitespace-pre-line wrap-anywhere ${expanded ? '' : 'line-clamp-3'}`}
        >
          {view.description}
        </button>
      )}
      {(refresh.error ?? setSubscribed.error) && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {(refresh.error ?? setSubscribed.error)?.message}
        </p>
      )}
      <section aria-labelledby="podcast-episodes" className="flex flex-col gap-2">
        <h2 id="podcast-episodes" className="font-semibold">
          {t('podcasts.episodes', { count: view.episodes.length })}
        </h2>
        {view.episodes.length === 0 ? (
          <p className="text-muted text-sm">{t('podcasts.noEpisodes')}</p>
        ) : (
          <ul className="bg-surface flex flex-col gap-4 rounded-xl px-4 py-3">
            {view.episodes.map((episode) => (
              <EpisodeItem key={episode.id} episode={episode} />
            ))}
          </ul>
        )}
      </section>
    </section>
  );
};
