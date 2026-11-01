import type { VideoEpisodeView, VideoTitleView } from '@raygun/bindings';
import { useSearch } from '@tanstack/react-router';
import { Check, ExternalLink, Eye, EyeOff, LoaderCircle, RefreshCw } from 'lucide-react';
import { useMemo, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { ResumeFab, TitleHero } from '../shell/TitleHero';
import { TopBar, TopBarAction } from '../shell/TopBar';
import { ServerPicker } from '../watch/ServerPicker';
import type { ServerTarget } from '../watch/ServerPicker';
import { StremioPoster } from '../watch/StremioRow';
import {
  useRefreshVideoTitle,
  useSetEpisodeWatched,
  useVideoTitle,
} from '../watch/useVideoSources';

const number = (value: number) => (Number.isInteger(value) ? String(value) : value.toFixed(1));

/** "S1 E2 · Name", "E2 · Name" or the name alone. */
const episodeLabel = (episode: VideoEpisodeView) =>
  episode.season !== null && episode.number !== null
    ? `S${number(episode.season)} E${number(episode.number)} · ${episode.name}`
    : episode.number !== null && !episode.name.endsWith(` ${number(episode.number)}`)
      ? `E${number(episode.number)} · ${episode.name}`
      : episode.name;

const Progress: FC<{ episode: VideoEpisodeView }> = ({ episode }) => {
  const position = episode.positionMs ?? 0;
  const duration = episode.durationMs ?? 0;
  return position > 0 && duration > 0 ? (
    <span aria-hidden className="block h-1 overflow-hidden rounded-full bg-black/20">
      <span
        className="bg-primary block h-full"
        style={{ width: `${Math.min(100, (position / duration) * 100)}%` }}
      />
    </span>
  ) : null;
};

/** A movie: the title has one episode and no seasons. */
const isMovie = (view: VideoTitleView) =>
  view.episodes.length === 1 && view.episodes[0]!.season === null;

const Episodes: FC<{ view: VideoTitleView; onPick: (episode: VideoEpisodeView) => void }> = ({
  view,
  onPick,
}) => {
  const { t } = useTranslation();
  const setWatched = useSetEpisodeWatched();
  const seasons = useMemo(
    () => [...new Set(view.episodes.map((episode) => episode.season))],
    [view.episodes],
  );
  const firstUnwatched = view.episodes.find((episode) => !episode.watched);
  const [season, setSeason] = useState<number | null | undefined>(undefined);
  const shown = season === undefined ? (firstUnwatched?.season ?? seasons[0] ?? null) : season;
  const episodes =
    seasons.length > 1
      ? view.episodes.filter((episode) => episode.season === shown)
      : view.episodes;

  return (
    <section aria-labelledby="video-episodes" className="flex flex-col gap-3">
      <h2 id="video-episodes" className="font-semibold">
        {t('watch.episodes')}
      </h2>
      {seasons.length > 1 && (
        <div
          role="tablist"
          aria-label={t('watch.seasons')}
          className="-mx-4 flex gap-2 overflow-x-auto px-4 lg:mx-0 lg:px-0"
        >
          {seasons.map((value) => (
            <button
              key={value ?? 'none'}
              type="button"
              role="tab"
              aria-selected={value === shown}
              onClick={() => setSeason(value)}
              className={`shrink-0 rounded-full px-4 py-1.5 text-sm ${value === shown ? 'bg-primary/20 text-primary font-semibold' : 'bg-surface'}`}
            >
              {value === null || value === 0
                ? t('watch.specials')
                : t('watch.season', { number: number(value) })}
            </button>
          ))}
        </div>
      )}
      <ul className="flex flex-col gap-2">
        {episodes.map((episode) => {
          const label = episodeLabel(episode);
          return (
            <li key={episode.id} aria-label={label} className="flex items-center gap-3">
              <button
                type="button"
                onClick={() => onPick(episode)}
                className="bg-surface flex min-w-0 flex-1 flex-col gap-1.5 rounded-xl px-4 py-3 text-left"
              >
                <span
                  className={`line-clamp-2 text-sm font-medium ${episode.watched ? 'text-muted' : ''}`}
                >
                  {label}
                </span>
                {episode.date !== null && (
                  <span className="text-muted text-xs">
                    {new Date(episode.date).toISOString().slice(0, 10)}
                  </span>
                )}
                <Progress episode={episode} />
              </button>
              <button
                type="button"
                aria-pressed={episode.watched}
                aria-label={t(episode.watched ? 'watch.markUnwatched' : 'watch.markWatched', {
                  title: label,
                })}
                onClick={() =>
                  setWatched.mutate({
                    titleId: view.id,
                    episodeId: episode.id,
                    watched: !episode.watched,
                  })
                }
                className={`grid size-9 shrink-0 place-items-center rounded-full ${episode.watched ? 'bg-primary text-background' : 'bg-surface text-muted'}`}
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

/** A title from a video source: details, episodes, and servers to play from. */
export const VideoTitlePage: FC = () => {
  const { t } = useTranslation();
  const { source, id } = useSearch({ from: '/watch/video' });
  const title = useVideoTitle(source, id);
  const refresh = useRefreshVideoTitle();
  const setWatched = useSetEpisodeWatched();
  const [picking, setPicking] = useState<ServerTarget | null>(null);
  const [expanded, setExpanded] = useState(false);
  const view = title.data;

  const top = (
    <TopBar
      back
      fallback="/browse"
      plainTitle
      title={view?.sourceName ?? ''}
      actions={
        view && (
          <>
            <TopBarAction
              label={t('watch.refresh')}
              icon={RefreshCw}
              disabled={refresh.isPending}
              onClick={() => refresh.mutate({ sourceId: view.sourceId, remoteId: view.remoteId })}
            />
            {view.url && (
              <TopBarAction
                label={t('watch.openInBrowser')}
                icon={ExternalLink}
                onClick={() => window.open(view.url ?? '', '_blank', 'noopener')}
              />
            )}
          </>
        )
      }
    />
  );

  if (title.error && !view) {
    return (
      <section data-testid="video-title-page">
        {top}
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {title.error.message}
        </p>
      </section>
    );
  }
  if (!view) {
    return (
      <section data-testid="video-title-page">
        {top}
        <LoaderCircle
          aria-label={t('watch.loading')}
          className="text-muted mx-auto my-12 animate-spin"
        />
      </section>
    );
  }
  if (view.hiddenNsfw) {
    return (
      <section data-testid="video-title-page">
        {top}
        <p className="text-muted text-sm">{t('watch.hiddenNsfw')}</p>
      </section>
    );
  }

  const pick = (episode: VideoEpisodeView) =>
    setPicking({
      sourceId: view.sourceId,
      titleId: view.id,
      episodeId: episode.id,
      remoteId: episode.remoteId,
      heading: isMovie(view) ? view.title : `${view.title} · ${episodeLabel(episode)}`,
    });
  const movie = isMovie(view) ? view.episodes[0]! : null;
  // The floating button plays the movie, or the first episode not yet watched.
  const next = movie ?? view.episodes.find((episode) => !episode.watched) ?? view.episodes[0];

  return (
    <section data-testid="video-title-page" className="flex min-h-full flex-col gap-5">
      {top}
      <TitleHero
        art={view.coverUrl}
        cover={<StremioPoster url={view.coverUrl} className="w-full shadow-lg" />}
        title={view.title}
        facts={[
          view.mediaType && t(`watch.mediaType.${view.mediaType}`),
          view.year,
          view.sourceName,
        ]}
        genres={view.genres}
        genresLabel={t('watch.genres')}
      >
        {movie && (
          <div className="flex flex-col gap-3">
            <button
              type="button"
              aria-pressed={movie.watched}
              onClick={() =>
                setWatched.mutate({
                  titleId: view.id,
                  episodeId: movie.id,
                  watched: !movie.watched,
                })
              }
              className="bg-surface/80 flex w-fit items-center gap-2 rounded-full px-4 py-2 text-sm"
            >
              {movie.watched ? <EyeOff aria-hidden size={16} /> : <Eye aria-hidden size={16} />}
              {t(movie.watched ? 'watch.watched' : 'watch.markWatchedShort')}
            </button>
            <Progress episode={movie} />
          </div>
        )}
      </TitleHero>
      {view.description && (
        <button
          type="button"
          aria-expanded={expanded}
          onClick={() => setExpanded(!expanded)}
          className={`text-left text-sm leading-relaxed whitespace-pre-line ${expanded ? '' : 'line-clamp-4'}`}
        >
          {view.description}
        </button>
      )}
      {(view.cast.length > 0 || view.directors.length > 0) && (
        <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-sm">
          {view.directors.length > 0 && (
            <>
              <dt className="text-muted">{t('watch.director')}</dt>
              <dd>{view.directors.join(', ')}</dd>
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
      {(refresh.error ?? setWatched.error) && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {(refresh.error ?? setWatched.error)?.message}
        </p>
      )}
      {!movie && view.episodes.length > 0 && <Episodes view={view} onPick={pick} />}
      {view.episodes.length === 0 && <p className="text-muted text-sm">{t('watch.noEpisodes')}</p>}
      {next && (
        <ResumeFab
          label={t(
            (next.positionMs ?? 0) > 0 ? 'watch.resume' : movie ? 'watch.play' : 'watch.start',
          )}
          onClick={() => pick(next)}
        />
      )}
      {picking && <ServerPicker target={picking} onClose={() => setPicking(null)} />}
    </section>
  );
};
