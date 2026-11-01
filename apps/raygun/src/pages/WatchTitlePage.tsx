import type { StremioMeta, StremioVideo } from '@raygun/bindings';
import { useSearch } from '@tanstack/react-router';
import { Check, Eye, EyeOff, Film, LoaderCircle } from 'lucide-react';
import { useMemo, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { ResumeFab, TitleHero } from '../shell/TitleHero';
import { TopBar } from '../shell/TopBar';
import { StreamPicker } from '../watch/StreamPicker';
import type { StreamTarget } from '../watch/StreamPicker';
import { StremioPoster } from '../watch/StremioRow';
import { useSetWatched, useStremioMeta, useStremioPlay } from '../watch/useStremio';

/** "S1 E2 · Pilot", or the title alone when it isn't numbered. */
const episodeLabel = (video: StremioVideo) =>
  video.season !== null && video.episode !== null
    ? `S${video.season} E${video.episode} · ${video.title}`
    : video.title;

const Progress: FC<{ position: number | null; duration: number | null }> = ({
  position,
  duration,
}) =>
  (position ?? 0) > 0 && (duration ?? 0) > 0 ? (
    <span aria-hidden className="block h-1 overflow-hidden rounded-full bg-black/20">
      <span
        className="bg-primary block h-full"
        style={{ width: `${Math.min(100, ((position ?? 0) / (duration ?? 1)) * 100)}%` }}
      />
    </span>
  ) : null;

/** Seasons in order (specials last), and the one to open: where watching left off. */
const useSeasons = (videos: StremioVideo[]) =>
  useMemo(() => {
    const seasons = [...new Set(videos.map((video) => video.season ?? 0))];
    const next = videos.find((video) => !video.watched && (video.season ?? 0) > 0);
    return { seasons, first: next?.season ?? seasons[0] ?? 0 };
  }, [videos]);

const Episodes: FC<{ meta: StremioMeta; onPick: (target: StreamTarget) => void }> = ({
  meta,
  onPick,
}) => {
  const { t } = useTranslation();
  const { seasons, first } = useSeasons(meta.videos);
  const [season, setSeason] = useState<number | null>(null);
  const shown = season ?? first;
  const setWatched = useSetWatched();
  const numbered = seasons.length > 1 || meta.videos.some((video) => video.season !== null);
  const videos = numbered
    ? meta.videos.filter((video) => (video.season ?? 0) === shown)
    : meta.videos;

  return (
    <section aria-labelledby="watch-episodes" className="flex flex-col gap-3">
      <h2 id="watch-episodes" className="font-semibold">
        {t('watch.episodes')}
      </h2>
      {numbered && seasons.length > 1 && (
        <div
          role="tablist"
          aria-label={t('watch.seasons')}
          className="-mx-4 flex gap-2 overflow-x-auto px-4 lg:mx-0 lg:px-0"
        >
          {seasons.map((number) => (
            <button
              key={number}
              type="button"
              role="tab"
              aria-selected={number === shown}
              onClick={() => setSeason(number)}
              className={`shrink-0 rounded-full px-4 py-1.5 text-sm ${number === shown ? 'bg-primary text-background font-semibold' : 'bg-surface'}`}
            >
              {number === 0 ? t('watch.specials') : t('watch.season', { number })}
            </button>
          ))}
        </div>
      )}
      <ul className="flex flex-col gap-3">
        {videos.map((video) => {
          const label = episodeLabel(video);
          const target = {
            kind: meta.kind,
            videoId: video.id,
            title: meta.name,
            subtitle: label,
            poster: video.thumbnail ?? meta.poster,
          };
          return (
            <li key={video.id} aria-label={label} className="flex gap-3">
              <button
                type="button"
                onClick={() => onPick(target)}
                className="flex min-w-0 flex-1 gap-3 text-left"
              >
                <span className="relative w-32 shrink-0 overflow-hidden rounded-lg">
                  <StremioPoster url={video.thumbnail} shape="landscape" />
                  <span className="absolute inset-x-1 bottom-1">
                    <Progress position={video.positionMs} duration={video.durationMs} />
                  </span>
                </span>
                <span className="flex min-w-0 flex-col gap-0.5">
                  <span
                    className={`line-clamp-2 text-sm font-medium ${video.watched ? 'text-muted' : ''}`}
                  >
                    {video.episode !== null ? `${video.episode}. ${video.title}` : video.title}
                  </span>
                  {video.released && (
                    <span className="text-muted text-xs">{video.released.slice(0, 10)}</span>
                  )}
                  {video.overview && (
                    <span className="text-muted line-clamp-2 text-xs">{video.overview}</span>
                  )}
                </span>
              </button>
              <button
                type="button"
                aria-pressed={video.watched}
                aria-label={t(video.watched ? 'watch.markUnwatched' : 'watch.markWatched', {
                  title: label,
                })}
                onClick={() => setWatched.mutate({ ...target, watched: !video.watched })}
                className={`grid size-9 shrink-0 place-items-center self-center rounded-full ${video.watched ? 'bg-primary text-background' : 'bg-surface text-muted'}`}
              >
                <Check aria-hidden size={16} />
              </button>
            </li>
          );
        })}
      </ul>
    </section>
  );
};

/** A movie or series from an addon: details, episodes, and streams to play. */
export const WatchTitlePage: FC = () => {
  const { t } = useTranslation();
  const { type, id } = useSearch({ from: '/watch/title' });
  const meta = useStremioMeta(type, id);
  const setWatched = useSetWatched();
  const trailer = useStremioPlay();
  const [picking, setPicking] = useState<StreamTarget | null>(null);
  const [expanded, setExpanded] = useState(false);
  const view = meta.data;

  const top = <TopBar back fallback="/browse" plainTitle title={view?.addonName ?? ''} />;

  if (meta.error && !view) {
    return (
      <section data-testid="watch-title-page">
        {top}
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {meta.error.message}
        </p>
      </section>
    );
  }
  if (!view) {
    return (
      <section data-testid="watch-title-page">
        {top}
        <LoaderCircle
          aria-label={t('watch.loading')}
          className="text-muted mx-auto my-12 animate-spin"
        />
      </section>
    );
  }

  const ownTarget: StreamTarget = {
    kind: view.kind,
    videoId: view.id,
    title: view.name,
    subtitle: view.releaseInfo,
    poster: view.background ?? view.poster,
  };
  const episodeTarget = (video: StremioVideo): StreamTarget => ({
    kind: view.kind,
    videoId: video.id,
    title: view.name,
    subtitle: episodeLabel(video),
    poster: video.thumbnail ?? view.poster,
  });
  const hasEpisodes = view.videos.length > 0;
  const trailerId = view.trailerYtIds[0];
  // The floating button plays the movie, or the first episode not yet watched.
  const nextVideo = hasEpisodes
    ? (view.videos.find((video) => !video.watched && (video.season ?? 0) > 0) ?? view.videos[0])
    : null;
  const resumable = nextVideo ? (nextVideo.positionMs ?? 0) > 0 : (view.positionMs ?? 0) > 0;

  return (
    <section data-testid="watch-title-page" className="flex min-h-full flex-col gap-5">
      {top}
      <TitleHero
        art={view.poster}
        backdrop={view.background}
        cover={<StremioPoster url={view.poster} className="w-full shadow-lg" />}
        title={view.name}
        facts={[view.releaseInfo, view.runtime, view.rating && `★ ${view.rating}`]}
        genres={view.genres}
        genresLabel={t('watch.genres')}
      >
        <div className="flex flex-wrap items-center gap-2">
          {!hasEpisodes && (
            <button
              type="button"
              aria-pressed={view.watched}
              onClick={() => setWatched.mutate({ ...ownTarget, watched: !view.watched })}
              className="bg-surface/80 flex items-center gap-2 rounded-full px-4 py-2 text-sm"
            >
              {view.watched ? <EyeOff aria-hidden size={16} /> : <Eye aria-hidden size={16} />}
              {t(view.watched ? 'watch.watched' : 'watch.markWatchedShort')}
            </button>
          )}
          {trailerId && (
            <button
              type="button"
              disabled={trailer.isPending}
              onClick={() =>
                trailer.mutate({
                  ...ownTarget,
                  subtitle: t('watch.trailer'),
                  stream: {
                    kind: 'youTube',
                    name: null,
                    description: null,
                    target: trailerId,
                    headers: {},
                    subtitles: [],
                  },
                })
              }
              className="bg-surface/80 flex items-center gap-2 rounded-full px-4 py-2 text-sm disabled:opacity-60"
            >
              <Film aria-hidden size={16} />
              {t('watch.trailer')}
            </button>
          )}
        </div>
        {!hasEpisodes && (
          <div className="mt-3">
            <Progress position={view.positionMs} duration={view.durationMs} />
          </div>
        )}
      </TitleHero>
      {view.description && (
        <button
          type="button"
          aria-expanded={expanded}
          onClick={() => setExpanded(!expanded)}
          className={`text-left text-sm leading-relaxed ${expanded ? '' : 'line-clamp-4'}`}
        >
          {view.description}
        </button>
      )}
      {(view.cast.length > 0 || view.director.length > 0) && (
        <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-sm">
          {view.director.length > 0 && (
            <>
              <dt className="text-muted">{t('watch.director')}</dt>
              <dd>{view.director.join(', ')}</dd>
            </>
          )}
          {view.cast.length > 0 && (
            <>
              <dt className="text-muted">{t('watch.cast')}</dt>
              <dd>{view.cast.slice(0, 8).join(', ')}</dd>
            </>
          )}
        </dl>
      )}
      {(trailer.error ?? setWatched.error) && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {(trailer.error ?? setWatched.error)?.message}
        </p>
      )}
      {hasEpisodes && <Episodes meta={view} onPick={setPicking} />}
      <ResumeFab
        label={t(resumable ? 'watch.resume' : hasEpisodes ? 'watch.start' : 'watch.play')}
        onClick={() => setPicking(nextVideo ? episodeTarget(nextVideo) : ownTarget)}
      />
      {picking && <StreamPicker target={picking} onClose={() => setPicking(null)} />}
    </section>
  );
};
