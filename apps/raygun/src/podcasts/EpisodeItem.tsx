import type { PodcastEpisode } from '@raygun/bindings';
import { Check, Download, LoaderCircle, Play, Trash2 } from 'lucide-react';
import { useNavigate } from '@tanstack/react-router';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { openContextMenu } from '../shell/contextMenuStore';
import { formatLength } from './formatLength';
import { episodeMenu } from './podcastMenus';
import {
  useDownloadEpisode,
  usePlayEpisode,
  useRemoveEpisodeDownload,
  useSetPlayed,
} from './usePodcasts';
import { SourceIssue } from '../shell/SourceIssue';

const ICON_BUTTON =
  'bg-background grid size-9 shrink-0 place-items-center rounded-full disabled:opacity-60';

/**
 * One episode: what it is, how far it was listened to, and play, played and download actions. An
 * audiobook's chapter passes `onPlay` to play the rest of the book after it (M46.22).
 */
export const EpisodeItem: FC<{
  episode: PodcastEpisode;
  showPodcast?: boolean;
  onPlay?: () => void;
}> = ({ episode, showPodcast = false, onPlay }) => {
  const { t, i18n } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const play = usePlayEpisode();
  const setPlayed = useSetPlayed();
  const download = useDownloadEpisode();
  const removeDownload = useRemoveEpisodeDownload();
  const navigate = useNavigate();
  const duration = episode.durationMs ?? 0;
  const position = episode.positionMs ?? 0;
  const started = !episode.played && position > 0;
  const downloadState = episode.download?.state;

  const status = episode.played
    ? t('podcasts.played')
    : started && duration > 0
      ? t('podcasts.left', { time: formatLength(duration - position, t) })
      : duration > 0
        ? formatLength(duration, t)
        : null;
  const date = episode.publishedAt
    ? new Intl.DateTimeFormat(i18n.language, { dateStyle: 'medium' }).format(episode.publishedAt)
    : null;
  const error = play.error ?? download.error ?? setPlayed.error;

  return (
    <li
      className="flex flex-col gap-2"
      aria-label={episode.title}
      onContextMenu={(event) =>
        openContextMenu(
          event,
          episodeMenu(episode, t, {
            play: () => (onPlay ? onPlay() : play.mutate(episode.id)),
            setPlayed: (played) => setPlayed.mutate({ id: episode.id, played }),
            download: () => download.mutate(episode.id),
            removeDownload: () => removeDownload.mutate(episode.id),
            toggleNotes: () => setExpanded(!expanded),
            notesShown: expanded,
            openPodcast: showPodcast
              ? () => void navigate({ to: '/podcast', search: { id: episode.podcastId } })
              : undefined,
          }),
        )
      }
    >
      <div className="flex items-start gap-3">
        <button
          type="button"
          onClick={() => setExpanded(!expanded)}
          aria-expanded={expanded}
          className="flex min-w-0 flex-1 flex-col gap-0.5 text-left"
        >
          {showPodcast && (
            <span className="text-muted truncate text-xs">{episode.podcastTitle}</span>
          )}
          <span className={`font-medium ${episode.played ? 'text-muted' : ''}`}>
            {episode.title}
          </span>
          <span className="text-muted text-xs">
            {[date, status].filter(Boolean).join(' · ')}
            {downloadState === 'done' && ` · ${t('podcasts.downloaded')}`}
          </span>
        </button>
        <button
          type="button"
          aria-label={t('podcasts.play', { title: episode.title })}
          disabled={play.isPending}
          onClick={() => (onPlay ? onPlay() : play.mutate(episode.id))}
          className="bg-primary text-background grid size-9 shrink-0 place-items-center rounded-full disabled:opacity-60"
        >
          <Play aria-hidden size={16} />
        </button>
        <button
          type="button"
          aria-label={t(episode.played ? 'podcasts.markUnplayed' : 'podcasts.markPlayed')}
          aria-pressed={episode.played}
          disabled={setPlayed.isPending}
          onClick={() => setPlayed.mutate({ id: episode.id, played: !episode.played })}
          className={`${ICON_BUTTON} ${episode.played ? 'text-primary' : ''}`}
        >
          <Check aria-hidden size={16} />
        </button>
        {downloadState === 'done' ? (
          <button
            type="button"
            aria-label={t('podcasts.removeDownload')}
            disabled={removeDownload.isPending}
            onClick={() => removeDownload.mutate(episode.id)}
            className={ICON_BUTTON}
          >
            <Trash2 aria-hidden size={16} />
          </button>
        ) : (
          <button
            type="button"
            aria-label={t('podcasts.download')}
            disabled={downloadState === 'downloading' || download.isPending}
            onClick={() => download.mutate(episode.id)}
            className={ICON_BUTTON}
          >
            {downloadState === 'downloading' ? (
              <LoaderCircle aria-hidden size={16} className="animate-spin" />
            ) : (
              <Download aria-hidden size={16} />
            )}
          </button>
        )}
      </div>
      {started && duration > 0 && (
        <div aria-hidden className="bg-background h-1 overflow-hidden rounded-full">
          <div
            className="bg-primary h-full"
            style={{ width: `${Math.min(100, (position / duration) * 100)}%` }}
          />
        </div>
      )}
      {downloadState === 'failed' && episode.download?.error && (
        <SourceIssue compact>
          {t('podcasts.downloadFailed', { message: episode.download.error })}
        </SourceIssue>
      )}
      {expanded && episode.description && (
        <p className="text-muted text-sm whitespace-pre-line wrap-anywhere">
          {episode.description}
        </p>
      )}
      {error && <SourceIssue compact>{error.message}</SourceIssue>}
    </li>
  );
};
