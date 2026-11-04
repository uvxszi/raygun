import { commands } from '@raygun/bindings';
import type { VideoDownloadItem, VideoEpisodeView, VideoTitleView } from '@raygun/bindings';
import { useSearch } from '@tanstack/react-router';
import {
  Check,
  ExternalLink,
  Eye,
  EyeOff,
  ListVideo,
  LoaderCircle,
  Play,
  RefreshCw,
  SearchCheck,
} from 'lucide-react';
import { useMemo, useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { CategoryPicker } from '../manga/CategoryPicker';
import { MangaCover } from '../manga/MangaCover';
import { OtherSources } from '../manga/OtherSources';
import { useKnownCard } from '../manga/knownCard';
import { ResumeFab, TitleHero } from '../shell/TitleHero';
import { TopBar, TopBarAction } from '../shell/TopBar';
import { useWideScreen } from '../shell/useWideScreen';
import { PinButton } from '../shell/PinButton';
import { LibraryPill } from '../watch/LibraryPill';
import { ExportButton } from '../export/ExportButton';
import { TitleListActions } from '../manga/TitleListActions';
import { useDuplicateCheck } from '../manga/useDuplicateCheck';
import { ServerPicker } from '../watch/ServerPicker';
import type { PickIntent, ServerTarget } from '../watch/ServerPicker';
import { StremioPoster } from '../watch/StremioRow';
import { VideoDownloadControl } from '../watch/VideoDownloadControl';

/** Videos from Local Files folders (M46.27): already on this device. */
const LOCAL_VIDEO = 'local.video';
import { episodeLabel } from '../watch/episodeLabel';
import {
  sourceKey,
  useDownloadsByKey,
  useSetVideoInLibrary,
  useWatchSourceTitle,
} from '../watch/useVideoLibrary';
import {
  useRefreshVideoTitle,
  useSetEpisodeWatched,
  useVideoSources,
  useVideoTitle,
} from '../watch/useVideoSources';
import { VideoComments } from '../watch/VideoComments';
import { SourceIssue } from '../shell/SourceIssue';

const number = (value: number) => (Number.isInteger(value) ? String(value) : value.toFixed(1));

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

/** Where watching continues, as Rust decides it (`VideoTitleView.continueEpisodeId`). */
const continueAt = (view: VideoTitleView) =>
  view.episodes.find((episode) => episode.id === view.continueEpisodeId);

const Episodes: FC<{
  view: VideoTitleView;
  downloads: Map<string, VideoDownloadItem>;
  onPick: (episode: VideoEpisodeView, intent: PickIntent | 'watch') => void;
}> = ({ view, downloads, onPick }) => {
  const { t } = useTranslation();
  const setWatched = useSetEpisodeWatched();
  const seasons = useMemo(
    () => [...new Set(view.episodes.map((episode) => episode.season))],
    [view.episodes],
  );
  const next = continueAt(view);
  const [season, setSeason] = useState<number | null | undefined>(undefined);
  const shown = season === undefined ? (next?.season ?? seasons[0] ?? null) : season;
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
          className="-mx-4 flex gap-2 no-scrollbar overflow-x-auto px-4 lg:mx-0 lg:px-0"
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
          const label = episodeLabel(episode, true, view.title);
          return (
            <li key={episode.id} aria-label={label} className="flex items-center gap-3">
              <button
                type="button"
                onClick={() => onPick(episode, 'watch')}
                onContextMenu={(event) => {
                  // A long press chooses the link (Aniyomi's episode options).
                  event.preventDefault();
                  onPick(episode, 'play');
                }}
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
              {view.sourceId !== LOCAL_VIDEO && (
                <VideoDownloadControl
                  item={downloads.get(sourceKey(view.sourceId, episode.remoteId))}
                  downloaded={episode.downloaded}
                  name={label}
                  onDownload={() => onPick(episode, 'download')}
                />
              )}
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
  const wide = useWideScreen();
  const { source, id } = useSearch({ from: '/watch/video' });
  const title = useVideoTitle(source, id);
  const refresh = useRefreshVideoTitle();
  const setWatched = useSetEpisodeWatched();
  const setInLibrary = useSetVideoInLibrary();
  const duplicates = useDuplicateCheck('video');
  const downloads = useDownloadsByKey();
  const watch = useWatchSourceTitle();
  const [picking, setPicking] = useState<{ target: ServerTarget; intent: PickIntent } | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [finding, setFinding] = useState(false);
  const view = title.data;
  // A video site's (its own Search pill: YouTube, PeerTube) thumbnails are 16:9 (M46.45).
  const siteSource = Boolean(
    useVideoSources().data?.find((each) => each.id === source)?.searchPill,
  );
  // What the list already showed (M46.38), while the page loads or when the source fails.
  const known = useKnownCard('video', source, id);
  const knownHero = known && (
    <TitleHero
      art={known.coverUrl}
      wideCover={siteSource}
      cover={
        <MangaCover
          url={known.coverUrl}
          video
          landscape={siteSource}
          className="w-full rounded-xl shadow-lg"
        />
      }
      title={known.title}
    />
  );

  const header = (choose?: ReactNode) => (
    <TopBar
      back
      fallback="/browse"
      plainTitle
      title={view?.sourceName ?? ''}
      actions={
        view && (
          <>
            <PinButton
              target={{
                kind: 'video',
                to: '/watch/video',
                search: { source, id },
                name: view.title,
                artworkUrl: view.coverUrl ?? null,
              }}
            />
            {choose}
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
                // Raygun's browser (M46.38): a new window does nothing in the app.
                onClick={() => void commands.catalogOpenPage(view.sourceId, view.url)}
              />
            )}
            <ExportButton medium="video" titleIds={[view.id]} />
          </>
        )
      }
    />
  );
  const top = header();

  if (title.error && !view) {
    return (
      <section data-testid="video-title-page" className="flex flex-col gap-5">
        {top}
        {knownHero}
        <SourceIssue>{title.error.message}</SourceIssue>
      </section>
    );
  }
  if (!view) {
    return (
      <section data-testid="video-title-page" className="flex flex-col gap-5">
        {top}
        {knownHero}
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

  // Watching opens the player at once and plays the best link (M44); choosing a link is the
  // picker's job.
  const pick = (episode: VideoEpisodeView, intent: PickIntent | 'watch' = 'watch') => {
    if (intent === 'watch') {
      watch.mutate({ titleId: view.id, episodeId: episode.remoteId });
      return;
    }
    setPicking({
      target: {
        sourceId: view.sourceId,
        titleId: view.id,
        episodeId: episode.id,
        remoteId: episode.remoteId,
        heading: isMovie(view)
          ? view.title
          : `${view.title} · ${episodeLabel(episode, true, view.title)}`,
      },
      intent,
    });
  };
  const movie = isMovie(view) ? view.episodes[0]! : null;
  // The floating button plays the movie, or where watching continues.
  const next = movie ?? continueAt(view) ?? view.episodes[0];

  return (
    <section data-testid="video-title-page" className="flex min-h-full flex-col gap-5">
      {header(
        next && (
          <TopBarAction
            label={t('watch.chooseSource')}
            icon={ListVideo}
            onClick={() => pick(next, 'play')}
          />
        ),
      )}
      <TitleHero
        art={view.coverUrl}
        wideCover={siteSource}
        cover={
          siteSource ? (
            <MangaCover
              url={view.coverUrl}
              video
              landscape
              className="w-full rounded-xl shadow-lg"
            />
          ) : (
            <StremioPoster url={view.coverUrl} className="w-full shadow-lg" />
          )
        }
        title={view.title}
        adult={view.adult}
        facts={[
          view.mediaType && t(`watch.mediaType.${view.mediaType}`),
          view.year,
          view.sourceName,
        ]}
        genres={view.genres}
        genresLabel={t('watch.genres')}
      >
        <div className="flex flex-col gap-3">
          <div className="flex flex-wrap items-center gap-2">
            {next && wide && (
              // Play is on the first screen (M46.34); phones keep the floating button.
              <button
                type="button"
                onClick={() => pick(next)}
                className="bg-primary text-background flex items-center gap-2 rounded-full px-5 py-2 text-sm font-semibold"
              >
                <Play aria-hidden size={16} fill="currentColor" />
                {t(
                  (next.positionMs ?? 0) > 0
                    ? 'watch.resume'
                    : movie
                      ? 'watch.play'
                      : 'watch.start',
                )}
              </button>
            )}
            <LibraryPill
              inLibrary={view.inLibrary}
              busy={setInLibrary.isPending || duplicates.checking}
              onToggle={() => {
                const add = () => setInLibrary.mutate({ titleId: view.id, inLibrary: true });
                if (view.inLibrary) setInLibrary.mutate({ titleId: view.id, inLibrary: false });
                else void duplicates.addChecked(view.id, view.title, add);
              }}
            />
            <TitleListActions medium="video" titleId={view.id} title={view.title} pill />
            {movie && (
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
            )}
            {movie && view.sourceId !== LOCAL_VIDEO && (
              <span className="bg-surface/80 rounded-full">
                <VideoDownloadControl
                  item={downloads.get(sourceKey(view.sourceId, movie.remoteId))}
                  downloaded={movie.downloaded}
                  name={view.title}
                  onDownload={() => pick(movie, 'download')}
                />
              </span>
            )}
          </div>
          {movie && <Progress episode={movie} />}
        </div>
      </TitleHero>
      {view.inLibrary && <CategoryPicker medium="video" titleId={view.id} />}
      {duplicates.dialog}
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
      {(refresh.error ?? setWatched.error ?? setInLibrary.error ?? watch.error) && (
        <SourceIssue>
          {(refresh.error ?? setWatched.error ?? setInLibrary.error ?? watch.error)?.message}
        </SourceIssue>
      )}
      {/* The same title (and season) elsewhere, when this source is slow or gone. A video
          site's videos (YouTube, PeerTube) aren't a catalogue's titles. */}
      {view.sourceId !== LOCAL_VIDEO && !siteSource && (
        <button
          type="button"
          aria-expanded={finding}
          onClick={() => setFinding(!finding)}
          className="bg-surface hover:bg-primary/15 flex items-center gap-2 self-start rounded-full px-4 py-2 text-sm font-medium"
        >
          <SearchCheck aria-hidden size={16} />
          {t('otherSources.find')}
        </button>
      )}
      {finding && <OtherSources medium="video" title={view.title} sourceId={view.sourceId} />}
      {!movie && view.episodes.length > 0 && (
        <Episodes view={view} downloads={downloads} onPick={pick} />
      )}
      {view.episodes.length === 0 && <p className="text-muted text-sm">{t('watch.noEpisodes')}</p>}
      {/* A single video's comments, when its source has them (M46.45: YouTube). */}
      {view.episodes.length === 1 && (
        <VideoComments sourceId={view.sourceId} videoId={view.remoteId} />
      )}
      {next && !wide && (
        <ResumeFab
          label={t(
            (next.positionMs ?? 0) > 0 ? 'watch.resume' : movie ? 'watch.play' : 'watch.start',
          )}
          onClick={() => pick(next)}
        />
      )}
      {picking && (
        <ServerPicker
          target={picking.target}
          intent={picking.intent}
          onClose={() => setPicking(null)}
        />
      )}
    </section>
  );
};
