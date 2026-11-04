import type { StremioMeta, StremioVideo, VideoDownloadItem } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { useSearch } from '@tanstack/react-router';
import { Check, Eye, EyeOff, Film, ListVideo, LoaderCircle, Play } from 'lucide-react';
import { useMemo, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { CategoryPicker } from '../manga/CategoryPicker';
import { ResumeFab, TitleHero } from '../shell/TitleHero';
import { TopBar, TopBarAction } from '../shell/TopBar';
import { useWideScreen } from '../shell/useWideScreen';
import { TrailerWindow } from '../video/TrailerWindow';
import { LibraryPill } from '../watch/LibraryPill';
import { ExportButton } from '../export/ExportButton';
import { TitleListActions } from '../manga/TitleListActions';
import { useDuplicateCheck } from '../manga/useDuplicateCheck';
import type { PickIntent } from '../watch/ServerPicker';
import { StreamPicker } from '../watch/StreamPicker';
import { TitleRatings } from '../watch/TitleRatings';
import type { StreamTarget } from '../watch/StreamPicker';
import { StremioPoster } from '../watch/StremioRow';
import { VideoDownloadControl } from '../watch/VideoDownloadControl';
import { useSetWatched, useStremioMeta, useStremioPlay } from '../watch/useStremio';
import {
  stremioKey,
  useDownloadsByKey,
  useSetStremioInLibrary,
  useWatchStremio,
} from '../watch/useVideoLibrary';
import { findInCache } from '../shell/queryMemory';
import { SourceIssue } from '../shell/SourceIssue';
import { episodeLabel, isGenericName, showsSeasons } from '../watch/episodeLabel';
import { LoadMore } from '../shell/LoadMore';

/** "S1 E2 · Pilot" without the show's name, or the title alone when it isn't numbered; `showSeason` from `showsSeasons`. */
const videoLabel = (video: StremioVideo, showSeason: boolean, show: string) =>
  episodeLabel(
    { season: video.season, number: video.episode, name: video.title },
    showSeason,
    show,
  );

/** Where watching continues, as Rust decides it (`StremioMeta.continueVideoId`). */
const continueAt = (videos: StremioVideo[], id: string | null) =>
  id === null ? undefined : videos.find((video) => video.id === id);

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
const useSeasons = (videos: StremioVideo[], continueId: string | null) =>
  useMemo(() => {
    const seasons = [...new Set(videos.map((video) => video.season ?? 0))];
    return { seasons, first: continueAt(videos, continueId)?.season ?? seasons[0] ?? 0 };
  }, [videos, continueId]);

/** Episodes drawn at a time: a long season (a thousand anime episodes) is drawn as it's scrolled. */
const EPISODE_CHUNK = 60;

const Episodes: FC<{
  meta: StremioMeta;
  downloads: Map<string, VideoDownloadItem>;
  onPick: (target: StreamTarget, intent: PickIntent | 'watch') => void;
}> = ({ meta, downloads, onPick }) => {
  const { t } = useTranslation();
  const { seasons, first } = useSeasons(meta.videos, meta.continueVideoId);
  const [season, setSeason] = useState<number | null>(null);
  const shown = season ?? first;
  const [drawn, setDrawn] = useState({ season: shown, count: EPISODE_CHUNK });
  const setWatched = useSetWatched();
  const numbered = seasons.length > 1 || meta.videos.some((video) => video.season !== null);
  const videos = numbered
    ? meta.videos.filter((video) => (video.season ?? 0) === shown)
    : meta.videos;
  const showSeason = showsSeasons(meta.name, meta.videos);
  // A newly picked season is drawn from its top again.
  const count = drawn.season === shown ? drawn.count : EPISODE_CHUNK;

  return (
    <section aria-labelledby="watch-episodes" className="flex flex-col gap-3">
      <h2 id="watch-episodes" className="font-semibold">
        {t('watch.episodes')}
      </h2>
      {numbered && seasons.length > 1 && (
        <div
          role="tablist"
          aria-label={t('watch.seasons')}
          className="-mx-4 flex gap-2 no-scrollbar overflow-x-auto px-4 lg:mx-0 lg:px-0"
        >
          {seasons.map((number) => (
            <button
              key={number}
              type="button"
              role="tab"
              aria-selected={number === shown}
              onClick={() => setSeason(number)}
              className={`shrink-0 rounded-full px-4 py-1.5 text-sm ${number === shown ? 'bg-primary/20 text-primary font-semibold' : 'bg-surface'}`}
            >
              {number === 0 ? t('watch.specials') : t('watch.season', { number })}
            </button>
          ))}
        </div>
      )}
      <ul className="flex flex-col gap-3">
        {videos.slice(0, count).map((video) => {
          const label = videoLabel(video, showSeason, meta.name);
          const target = {
            kind: meta.kind,
            metaId: meta.id,
            videoId: video.id,
            title: meta.name,
            subtitle: label,
            poster: video.thumbnail ?? meta.poster,
            titleId: meta.titleId,
          };
          return (
            <li key={video.id} aria-label={label} className="flex gap-3">
              <button
                type="button"
                onClick={() => onPick(target, 'watch')}
                onContextMenu={(event) => {
                  // A long press chooses the stream.
                  event.preventDefault();
                  onPick(target, 'play');
                }}
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
                    {video.episode !== null && !isGenericName(video.title, null, video.episode)
                      ? `${video.episode}. ${video.title}`
                      : video.title}
                  </span>
                  {video.released && (
                    <span className="text-muted text-xs">{video.released.slice(0, 10)}</span>
                  )}
                  {video.overview && (
                    <span className="text-muted line-clamp-2 text-xs">{video.overview}</span>
                  )}
                </span>
              </button>
              <span className="self-center">
                <VideoDownloadControl
                  item={downloads.get(stremioKey(meta.kind, video.id))}
                  downloaded={video.downloaded}
                  name={label}
                  onDownload={() => onPick(target, 'download')}
                />
              </span>
              <button
                type="button"
                aria-pressed={video.watched}
                aria-label={t(video.watched ? 'watch.markUnwatched' : 'watch.markWatched', {
                  title: label,
                })}
                onClick={() =>
                  setWatched.mutate({
                    kind: target.kind,
                    videoId: target.videoId,
                    title: target.title,
                    subtitle: target.subtitle,
                    poster: target.poster,
                    watched: !video.watched,
                  })
                }
                className={`grid size-9 shrink-0 place-items-center self-center rounded-full ${video.watched ? 'bg-primary text-background' : 'bg-surface text-muted'}`}
              >
                <Check aria-hidden size={16} />
              </button>
            </li>
          );
        })}
      </ul>
      <LoadMore
        hasNextPage={count < videos.length}
        fetching={false}
        onLoad={() => setDrawn({ season: shown, count: count + EPISODE_CHUNK * 2 })}
        label={t('search.more')}
      />
    </section>
  );
};

/** A movie or series from an addon: details, episodes, and streams to play. */
export const WatchTitlePage: FC = () => {
  const { t } = useTranslation();
  const wide = useWideScreen();
  const { type, id } = useSearch({ from: '/watch/title' });
  const meta = useStremioMeta(type, id);
  const setWatched = useSetWatched();
  const trailer = useStremioPlay();
  const setInLibrary = useSetStremioInLibrary();
  const duplicates = useDuplicateCheck('video');
  const downloads = useDownloadsByKey();
  const watch = useWatchStremio();
  const [picking, setPicking] = useState<{ target: StreamTarget; intent: PickIntent } | null>(null);
  // Watching opens the player at once with the best stream (M44); a long press picks one.
  const pick = (target: StreamTarget, intent: PickIntent | 'watch' = 'watch') => {
    if (intent !== 'watch') {
      setPicking({ target, intent });
    } else if (meta.data) {
      const movie = meta.data.videos.length === 0;
      watch.mutate({
        kind: meta.data.kind,
        id: meta.data.id,
        videoId: movie ? null : target.videoId,
      });
    }
  };
  const [expanded, setExpanded] = useState(false);
  const view = meta.data;
  const client = useQueryClient();
  // The catalogue's card for it (M46.40): its name and poster at once, and when addons fail.
  const known = useMemo(
    () =>
      findInCache(client, [], (record) =>
        record.id === id && typeof record.name === 'string' && 'poster' in record
          ? {
              name: record.name,
              poster: typeof record.poster === 'string' ? record.poster : null,
            }
          : null,
      ),
    [client, id],
  );
  const knownHero = known && (
    <TitleHero
      art={known.poster}
      cover={<StremioPoster url={known.poster} className="w-full rounded-xl shadow-lg" />}
      title={known.name}
    />
  );

  // A saved title opened from storage (no addon answered) has no addon to name.
  const top = (
    <TopBar
      back
      fallback="/browse"
      plainTitle
      title={view ? view.addonName || t('watch.savedCopy') : ''}
    />
  );

  if (meta.error && !view) {
    return (
      <section data-testid="watch-title-page" className="flex flex-col gap-5">
        {top}
        {knownHero}
        <SourceIssue>{meta.error.message}</SourceIssue>
      </section>
    );
  }
  if (!view) {
    return (
      <section data-testid="watch-title-page" className="flex flex-col gap-5">
        {top}
        {knownHero}
        <LoaderCircle
          aria-label={t('watch.loading')}
          className="text-muted mx-auto my-12 animate-spin"
        />
      </section>
    );
  }

  const ownTarget: StreamTarget = {
    kind: view.kind,
    metaId: view.id,
    videoId: view.videoId,
    title: view.name,
    subtitle: view.releaseInfo,
    poster: view.background ?? view.poster,
    titleId: view.titleId,
  };
  const showSeason = showsSeasons(view.name, view.videos);
  const episodeTarget = (video: StremioVideo): StreamTarget => ({
    kind: view.kind,
    metaId: view.id,
    videoId: video.id,
    title: view.name,
    subtitle: videoLabel(video, showSeason, view.name),
    poster: video.thumbnail ?? view.poster,
    titleId: view.titleId,
  });
  const hasEpisodes = view.videos.length > 0;
  const trailerId = view.trailerYtIds[0];
  // The floating button plays the movie, or where watching continues.
  const nextVideo = hasEpisodes
    ? (continueAt(view.videos, view.continueVideoId) ?? view.videos[0])
    : null;
  const resumable = nextVideo ? (nextVideo.positionMs ?? 0) > 0 : (view.positionMs ?? 0) > 0;
  const playLabel = t(resumable ? 'watch.resume' : hasEpisodes ? 'watch.start' : 'watch.play');
  const playNow = () => pick(nextVideo ? episodeTarget(nextVideo) : ownTarget);
  const playTrailer = (videoId: string) =>
    trailer.mutate({
      kind: ownTarget.kind,
      videoId: ownTarget.videoId,
      title: ownTarget.title,
      poster: ownTarget.poster,
      subtitle: t('watch.trailer'),
      stream: {
        kind: 'youTube',
        name: null,
        description: null,
        target: videoId,
        headers: {},
        subtitles: [],
        bingeGroup: null,
        filename: null,
        size: null,
        videoHash: null,
        notWebReady: false,
      },
    });

  return (
    <section data-testid="watch-title-page" className="flex min-h-full flex-col gap-5">
      <TopBar
        back
        fallback="/browse"
        plainTitle
        title={view.addonName || t('watch.savedCopy')}
        actions={
          <>
            <TopBarAction
              label={t('watch.chooseSource')}
              icon={ListVideo}
              onClick={() => pick(nextVideo ? episodeTarget(nextVideo) : ownTarget, 'play')}
            />
            {view.titleId && <ExportButton medium="video" titleIds={[view.titleId]} />}
          </>
        }
      />
      <TitleHero
        art={view.poster}
        backdrop={view.background}
        cover={<StremioPoster url={view.poster} className="w-full shadow-lg" />}
        title={view.name}
        facts={[
          view.releaseInfo,
          view.runtime,
          view.ratings.length === 0 && view.rating && `★ ${view.rating}`,
        ]}
        factsExtra={
          view.ratings.length > 0 && (
            <TitleRatings kind={view.kind} id={view.id} ratings={view.ratings} />
          )
        }
        genres={view.genres}
        genresLabel={t('watch.genres')}
        aside={
          trailerId ? (
            <TrailerWindow
              key={trailerId}
              videoId={trailerId}
              title={view.name}
              onFull={() => playTrailer(trailerId)}
            />
          ) : undefined
        }
      >
        <div className="flex flex-wrap items-center gap-2">
          {/* Play is on the first screen (M46.34); phones keep the floating button. */}
          {wide && (
            <button
              type="button"
              onClick={playNow}
              className="bg-primary text-background flex items-center gap-2 rounded-full px-5 py-2 text-sm font-semibold"
            >
              <Play aria-hidden size={16} fill="currentColor" />
              {playLabel}
            </button>
          )}
          <LibraryPill
            inLibrary={view.inLibrary}
            busy={setInLibrary.isPending || duplicates.checking}
            onToggle={() => {
              const toggle = (inLibrary: boolean) =>
                setInLibrary.mutate({ kind: view.kind, id: view.id, inLibrary });
              if (view.inLibrary) toggle(false);
              else void duplicates.addChecked(view.titleId ?? '', view.name, () => toggle(true));
            }}
          />
          {view.inLibrary && view.titleId && (
            <TitleListActions medium="video" titleId={view.titleId} title={view.name} pill />
          )}
          {/* A live channel is neither watched to the end nor saved. */}
          {!hasEpisodes && !view.live && (
            <button
              type="button"
              aria-pressed={view.watched}
              onClick={() =>
                setWatched.mutate({
                  kind: view.kind,
                  videoId: view.videoId,
                  title: view.name,
                  subtitle: view.releaseInfo,
                  poster: ownTarget.poster,
                  watched: !view.watched,
                })
              }
              className="bg-surface/80 flex items-center gap-2 rounded-full px-4 py-2 text-sm"
            >
              {view.watched ? <EyeOff aria-hidden size={16} /> : <Eye aria-hidden size={16} />}
              {t(view.watched ? 'watch.watched' : 'watch.markWatchedShort')}
            </button>
          )}
          {!hasEpisodes && !view.live && (
            <span className="bg-surface/80 rounded-full">
              <VideoDownloadControl
                item={downloads.get(stremioKey(view.kind, view.videoId))}
                downloaded={view.downloaded}
                name={view.name}
                onDownload={() => pick(ownTarget, 'download')}
              />
            </span>
          )}
          {trailerId && (
            <button
              type="button"
              disabled={trailer.isPending}
              onClick={() => playTrailer(trailerId)}
              className="bg-surface/80 flex items-center gap-2 rounded-full px-4 py-2 text-sm disabled:opacity-60"
            >
              <Film aria-hidden size={16} />
              {t('watch.trailer')}
            </button>
          )}
        </div>
        {!hasEpisodes && !view.live && (
          <div className="mt-3">
            <Progress position={view.positionMs} duration={view.durationMs} />
          </div>
        )}
      </TitleHero>
      {view.inLibrary && view.titleId && <CategoryPicker medium="video" titleId={view.titleId} />}
      {duplicates.dialog}
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
      {(trailer.error ?? setWatched.error ?? setInLibrary.error ?? watch.error) && (
        <SourceIssue>
          {(trailer.error ?? setWatched.error ?? setInLibrary.error ?? watch.error)?.message}
        </SourceIssue>
      )}
      {hasEpisodes && <Episodes meta={view} downloads={downloads} onPick={pick} />}
      {!wide && <ResumeFab label={playLabel} onClick={playNow} />}
      {picking && (
        <StreamPicker
          target={picking.target}
          intent={picking.intent}
          onClose={() => setPicking(null)}
        />
      )}
    </section>
  );
};
