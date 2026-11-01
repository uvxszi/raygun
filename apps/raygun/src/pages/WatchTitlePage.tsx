import type { StremioMeta, StremioVideo } from '@raygun/bindings';
import { useSearch } from '@tanstack/react-router';
import { Check, Eye, EyeOff, Film, LoaderCircle, Play } from 'lucide-react';
import { useMemo, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

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

  if (meta.error && !view) {
    return (
      <section data-testid="watch-title-page">
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {meta.error.message}
        </p>
      </section>
    );
  }
  if (!view) {
    return (
      <section data-testid="watch-title-page" className="grid place-items-center py-12">
        <LoaderCircle aria-label={t('watch.loading')} className="text-muted animate-spin" />
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
  const hasEpisodes = view.videos.length > 0;
  const facts = [view.releaseInfo, view.runtime, view.rating && `★ ${view.rating}`].filter(Boolean);
  const trailerId = view.trailerYtIds[0];

  return (
    <section data-testid="watch-title-page" className="flex flex-col gap-5">
      {view.background && (
        <img
          src={view.background}
          alt=""
          className="-mx-4 -mt-4 aspect-video w-[calc(100%+2rem)] max-w-none object-cover opacity-80 lg:mx-0 lg:mt-0 lg:w-full lg:rounded-xl"
        />
      )}
      <header className="flex gap-4">
        <StremioPoster url={view.poster} className="w-28 shrink-0 shadow-lg sm:w-36" />
        <div className="flex min-w-0 flex-col gap-2">
          <h1 className="text-xl font-bold tracking-tight wrap-anywhere">{view.name}</h1>
          {facts.length > 0 && <p className="text-muted text-sm">{facts.join(' · ')}</p>}
          {view.genres.length > 0 && (
            <ul aria-label={t('watch.genres')} className="flex flex-wrap gap-1.5">
              {view.genres.slice(0, 6).map((genre) => (
                <li key={genre} className="bg-surface rounded-full px-2.5 py-0.5 text-xs">
                  {genre}
                </li>
              ))}
            </ul>
          )}
          <p className="text-muted text-xs">{t('watch.from', { addon: view.addonName })}</p>
        </div>
      </header>
      <div className="flex flex-wrap items-center gap-2">
        {!hasEpisodes && (
          <>
            <button
              type="button"
              onClick={() => setPicking(ownTarget)}
              className="bg-primary text-background flex items-center gap-2 rounded-full px-5 py-2 text-sm font-semibold"
            >
              <Play aria-hidden size={16} />
              {t((view.positionMs ?? 0) > 0 ? 'watch.resume' : 'watch.play')}
            </button>
            <button
              type="button"
              aria-pressed={view.watched}
              onClick={() => setWatched.mutate({ ...ownTarget, watched: !view.watched })}
              className="bg-surface flex items-center gap-2 rounded-full px-4 py-2 text-sm"
            >
              {view.watched ? <EyeOff aria-hidden size={16} /> : <Eye aria-hidden size={16} />}
              {t(view.watched ? 'watch.watched' : 'watch.markWatchedShort')}
            </button>
          </>
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
            className="bg-surface flex items-center gap-2 rounded-full px-4 py-2 text-sm disabled:opacity-60"
          >
            <Film aria-hidden size={16} />
            {t('watch.trailer')}
          </button>
        )}
      </div>
      {!hasEpisodes && <Progress position={view.positionMs} duration={view.durationMs} />}
      {view.description && (
        <button
          type="button"
          aria-expanded={expanded}
          onClick={() => setExpanded(!expanded)}
          className={`text-left text-sm ${expanded ? '' : 'line-clamp-4'}`}
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
      {picking && <StreamPicker target={picking} onClose={() => setPicking(null)} />}
    </section>
  );
};
