import type { PodcastEpisode } from '@raygun/bindings';
import { Check, Clock, Download, LoaderCircle, Play, Trash2 } from 'lucide-react';
import { useNavigate } from '@tanstack/react-router';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { StopDownloadButton } from '../downloads/StopDownload';
import { ConfirmButton } from '../shell/ConfirmButton';
import { MenuButton } from '../shell/MenuButton';
import { formatLength } from './formatLength';
import { episodeMenu } from './podcastMenus';
import { ShowNotes } from './ShowNotes';
import {
  useCancelEpisodeDownload,
  useDownloadEpisode,
  useForgetPosition,
  usePlayEpisode,
  useRemoveEpisodeDownload,
  useSetEarlierPlayed,
  useSetPlayed,
} from './usePodcasts';
import { SourceIssue } from '../shell/SourceIssue';
import { episodeReveal } from '../music/reveal';

const ICON_BUTTON =
  'bg-background grid size-9 shrink-0 place-items-center rounded-full disabled:opacity-60';

/** How far a download is, "34%", when its size is known. */
const share = (received: number, total: number | null) =>
  total && total > 0 ? Math.min(99, Math.floor((received / total) * 100)) : null;

/**
 * One episode: what it is, how far it was listened to, and play, played and download actions. An
 * audiobook's chapter (`chapter`) shows its number and reads finished rather than played; it
 * passes `onPlay` to play the rest of the book after it (M46.22), and `tapToPlay` so tapping the
 * chapter plays it (M46.45; its notes stay in its menu).
 */
export const EpisodeItem: FC<{
  episode: PodcastEpisode;
  showPodcast?: boolean;
  onPlay?: () => void;
  tapToPlay?: boolean;
  chapter?: boolean;
}> = ({ episode, showPodcast = false, onPlay, tapToPlay = false, chapter = false }) => {
  const { t, i18n } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const play = usePlayEpisode();
  const setPlayed = useSetPlayed();
  const setEarlier = useSetEarlierPlayed();
  const forget = useForgetPosition();
  const download = useDownloadEpisode();
  const cancel = useCancelEpisodeDownload();
  const removeDownload = useRemoveEpisodeDownload();
  const navigate = useNavigate();
  const duration = episode.durationMs ?? 0;
  const position = episode.positionMs ?? 0;
  const started = !episode.played && position > 0;
  const downloadState = episode.download?.state;
  const percent = episode.download
    ? share(episode.download.receivedBytes ?? 0, episode.download.totalBytes)
    : null;
  const start = () => (onPlay ? onPlay() : play.mutate(episode.id));

  const status = episode.played
    ? t(chapter ? 'audiobooks.chapterFinished' : 'podcasts.played')
    : started && duration > 0
      ? t('podcasts.left', { time: formatLength(duration - position, t) })
      : duration > 0
        ? formatLength(duration, t)
        : null;
  const date =
    episode.publishedAt && !chapter
      ? new Intl.DateTimeFormat(i18n.language, { dateStyle: 'medium' }).format(episode.publishedAt)
      : null;
  const downloadNote =
    downloadState === 'done'
      ? t('podcasts.downloaded')
      : downloadState === 'queued'
        ? t('podcasts.downloadWaiting')
        : downloadState === 'downloading'
          ? percent !== null
            ? t('podcasts.downloadingShare', { percent })
            : t('podcasts.downloading')
          : null;
  const error =
    play.error ??
    download.error ??
    setPlayed.error ??
    setEarlier.error ??
    forget.error ??
    cancel.error ??
    removeDownload.error;
  const markLabel = chapter
    ? episode.played
      ? 'audiobooks.markUnfinished'
      : 'audiobooks.markFinished'
    : episode.played
      ? 'podcasts.markUnplayed'
      : 'podcasts.markPlayed';

  return (
    <li
      className="flex flex-col gap-2"
      aria-label={episode.title}
      data-reveal={episodeReveal(episode.id)}
    >
      <div className="flex items-start gap-3">
        {chapter && episode.number !== null && (
          <span
            aria-hidden
            className={`w-7 shrink-0 pt-0.5 text-right text-sm tabular-nums ${episode.played ? 'text-muted' : 'text-muted/80'}`}
          >
            {episode.number}
          </span>
        )}
        <button
          type="button"
          onClick={() => (tapToPlay ? start() : setExpanded(!expanded))}
          aria-expanded={tapToPlay ? undefined : expanded}
          className={`flex min-w-0 flex-1 flex-col gap-0.5 text-left ${tapToPlay ? 'hover:text-primary' : ''}`}
        >
          {showPodcast && (
            <span className="text-muted truncate text-xs">{episode.podcastTitle}</span>
          )}
          <span className={`font-medium wrap-anywhere ${episode.played ? 'text-muted' : ''}`}>
            {episode.title}
          </span>
          <span className="text-muted text-xs">
            {[date, status, downloadNote].filter(Boolean).join(' · ')}
          </span>
        </button>
        <button
          type="button"
          aria-label={t(started ? 'podcasts.resumeTitle' : 'podcasts.play', {
            title: episode.title,
          })}
          disabled={play.isPending}
          onClick={start}
          className="bg-primary text-background grid size-9 shrink-0 place-items-center rounded-full disabled:opacity-60"
        >
          <Play aria-hidden size={16} />
        </button>
        <button
          type="button"
          aria-label={t(markLabel)}
          aria-pressed={episode.played}
          disabled={setPlayed.isPending}
          onClick={() => setPlayed.mutate({ id: episode.id, played: !episode.played })}
          className={`${ICON_BUTTON} ${episode.played ? 'text-primary' : ''}`}
        >
          <Check aria-hidden size={16} />
        </button>
        {downloadState === 'done' ? (
          <ConfirmButton
            label={t('podcasts.removeDownload')}
            confirm={t('podcasts.removeDownloadConfirm')}
            icon={Trash2}
            iconSize={16}
            disabled={removeDownload.isPending}
            onConfirm={() => removeDownload.mutate(episode.id)}
            idleClassName={ICON_BUTTON}
          />
        ) : downloadState === 'downloading' || downloadState === 'queued' ? (
          <StopDownloadButton
            label={t('podcasts.cancelDownload')}
            name={episode.title}
            disabled={cancel.isPending}
            onStop={() => cancel.mutate(episode.id)}
            className={`${ICON_BUTTON} text-primary`}
          >
            {downloadState === 'queued' ? (
              <Clock aria-hidden size={16} />
            ) : (
              <LoaderCircle aria-hidden size={16} className="animate-spin" />
            )}
          </StopDownloadButton>
        ) : (
          <button
            type="button"
            aria-label={t(
              downloadState === 'failed' ? 'podcasts.downloadAgain' : 'podcasts.download',
            )}
            disabled={download.isPending}
            onClick={() => download.mutate(episode.id)}
            className={ICON_BUTTON}
          >
            <Download aria-hidden size={16} />
          </button>
        )}
        <MenuButton
          label={t('menu.options', { name: episode.title })}
          items={() =>
            episodeMenu(episode, t, {
              chapter,
              play: start,
              setPlayed: (played) => setPlayed.mutate({ id: episode.id, played }),
              setEarlierPlayed: () => setEarlier.mutate(episode.id),
              startOver: started ? () => forget.mutate(episode.id) : undefined,
              download: () => download.mutate(episode.id),
              cancelDownload: () => cancel.mutate(episode.id),
              removeDownload: () => removeDownload.mutate(episode.id),
              toggleNotes: () => setExpanded(!expanded),
              notesShown: expanded,
              openPodcast: showPodcast
                ? () => void navigate({ to: '/podcast', search: { id: episode.podcastId } })
                : undefined,
            })
          }
          className={ICON_BUTTON}
          iconSize={16}
        />
      </div>
      {started && duration > 0 && (
        <div
          aria-hidden
          className={`bg-background h-1 overflow-hidden rounded-full ${chapter ? 'ml-10' : ''}`}
        >
          <div
            className="bg-primary h-full origin-left"
            style={{ transform: `scaleX(${Math.min(1, position / duration)})` }}
          />
        </div>
      )}
      {downloadState === 'failed' && episode.download?.error && (
        <SourceIssue compact source={episode.podcastTitle}>
          {episode.download.error}
        </SourceIssue>
      )}
      {expanded && episode.description && <ShowNotes text={episode.description} />}
      {error && <SourceIssue compact>{error.message}</SourceIssue>}
    </li>
  );
};
