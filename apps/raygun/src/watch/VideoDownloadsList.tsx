import { commands } from '@raygun/bindings';
import type { VideoDownloadItem } from '@raygun/bindings';
import {
  CircleAlert,
  Clapperboard,
  Pause,
  Play,
  RotateCw,
  Sparkles,
  Trash2,
  X,
} from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useCaptions } from '../player/spoken';
import { openContextMenu } from '../shell/contextMenuStore';
import { EmptyState } from '../shell/EmptyState';
import { formatBytes } from './episodeLabel';
import {
  downloadShare,
  usePlayVideoDownload,
  useRemoveVideoDownload,
  useRetryVideoDownload,
  useSetVideoDownloadsPaused,
  useVideoDownloads,
} from './useVideoLibrary';
import { SourceIssue } from '../shell/SourceIssue';

const ICON = 'grid size-9 shrink-0 place-items-center rounded-full disabled:opacity-60';

/** How far a download got: "45% · 120 MB of 800 MB", or what it has so far. */
const useAmount = () => {
  const { t, i18n } = useTranslation();
  const size = (bytes: number) => formatBytes(bytes, i18n.language);
  return (item: VideoDownloadItem) => {
    const doneBytes = item.doneBytes ?? 0;
    const share = downloadShare(item);
    const percent = share === null ? null : `${Math.round(share * 100)}%`;
    const amount = item.totalBytes
      ? t('watch.sizeOf', { done: size(doneBytes), total: size(item.totalBytes) })
      : doneBytes > 0
        ? size(doneBytes)
        : null;
    return [percent, amount].filter(Boolean).join(' · ');
  };
};

/** "Queued", "Paused · 12% · 44 MB of 383 MB", "On this device · 800 MB"... */
const useStateLine = (paused: boolean) => {
  const { t, i18n } = useTranslation();
  const amount = useAmount();
  return (item: VideoDownloadItem) => {
    const doneBytes = item.doneBytes ?? 0;
    switch (item.state) {
      case 'queued':
        return paused
          ? [t('watch.paused'), amount(item)].filter(Boolean).join(' · ')
          : t('manga.downloadState.queued');
      case 'failed':
        return t('manga.downloadState.failed');
      case 'done':
        return doneBytes > 0
          ? t('watch.onDevice', { size: formatBytes(doneBytes, i18n.language) })
          : t('manga.downloadState.done');
      case 'downloading':
        return amount(item) || t('manga.downloadState.downloading');
    }
  };
};

const Row: FC<{ item: VideoDownloadItem; paused: boolean }> = ({ item, paused }) => {
  const { t } = useTranslation();
  const stateLine = useStateLine(paused);
  const play = usePlayVideoDownload();
  const retry = useRetryVideoDownload();
  const remove = useRemoveVideoDownload();
  const name = [item.title, item.subtitle].filter(Boolean).join(' · ');
  const share = downloadShare(item);
  const running = item.state === 'queued' || item.state === 'downloading';
  // AI subtitles for a saved video, made offline from its menu (M46.33).
  const captions = useCaptions(item.key);
  const transcribing = captions.data?.running ?? false;
  const menu = (event: React.MouseEvent) => {
    if (item.state !== 'done') return;
    openContextMenu(event, [
      { kind: 'heading', label: name },
      {
        label: t('downloads.makeSubtitles'),
        icon: Sparkles,
        onSelect: () => void commands.captionsVideoMake(item.key),
      },
    ]);
  };
  return (
    <li
      data-testid="video-download-row"
      aria-label={name}
      onContextMenu={menu}
      className="bg-surface flex flex-col gap-2 rounded-xl p-3"
    >
      <div className="flex items-center gap-3">
        <button
          type="button"
          aria-label={t('watch.playDownload', { name })}
          disabled={play.isPending}
          onClick={() => play.mutate(item.id)}
          className="relative w-24 shrink-0 overflow-hidden rounded-lg"
        >
          {item.poster ? (
            <img
              src={item.poster}
              alt=""
              loading="lazy"
              className="aspect-video w-full object-cover"
            />
          ) : (
            <span className="bg-background text-muted grid aspect-video w-full place-items-center">
              {item.state !== 'done' && <Clapperboard aria-hidden size={18} />}
            </span>
          )}
          {item.state === 'done' && (
            <span className="absolute inset-0 grid place-items-center bg-black/30 text-white">
              <Play aria-hidden size={20} fill="currentColor" />
            </span>
          )}
        </button>
        <span className="flex min-w-0 flex-1 flex-col">
          <span className="truncate text-sm font-medium">{item.title}</span>
          {item.subtitle && <span className="text-muted truncate text-xs">{item.subtitle}</span>}
          <span className="text-muted truncate text-xs">{stateLine(item)}</span>
          {transcribing && (
            <span role="status" className="text-muted truncate text-xs">
              {t('downloads.makingSubtitles')}
            </span>
          )}
        </span>
        {item.state === 'failed' && (
          <button
            type="button"
            aria-label={t('watch.retryDownload', { name })}
            disabled={retry.isPending}
            onClick={() => retry.mutate(item.id)}
            className={`${ICON} text-primary`}
          >
            <RotateCw aria-hidden size={18} />
          </button>
        )}
        <button
          type="button"
          aria-label={t(running ? 'watch.cancelDownload' : 'watch.deleteDownload', { name })}
          disabled={remove.isPending}
          onClick={() => remove.mutate(item.id)}
          className={`${ICON} text-muted`}
        >
          {running ? <X aria-hidden size={18} /> : <Trash2 aria-hidden size={18} />}
        </button>
      </div>
      {item.state === 'downloading' && (
        <div
          role="progressbar"
          aria-label={name}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={share === null ? undefined : Math.round(share * 100)}
          className="bg-background h-1 overflow-hidden rounded-full"
        >
          <div
            className={`bg-primary h-full transition-[width] ${share === null ? 'w-1/3 animate-pulse' : ''}`}
            style={share === null ? undefined : { width: `${share * 100}%` }}
          />
        </div>
      )}
      {item.state === 'failed' && item.error && (
        <p className="text-primary flex items-start gap-1.5 text-xs wrap-anywhere">
          <CircleAlert aria-hidden size={14} className="mt-px shrink-0" />
          {item.error}
        </p>
      )}
      {play.error && <SourceIssue compact>{play.error.message}</SourceIssue>}
    </li>
  );
};

/** Downloads → Videos: films and episodes saved or saving, newest first (Aniyomi's queue). */
export const VideoDownloadsList: FC = () => {
  const { t, i18n } = useTranslation();
  const downloads = useVideoDownloads();
  const setPaused = useSetVideoDownloadsPaused();
  const list = downloads.data;
  const active = list?.items.filter((item) => item.state !== 'done') ?? [];
  const saved = list?.items.filter((item) => item.state === 'done') ?? [];
  const savedBytes = saved.reduce((sum, item) => sum + (item.doneBytes ?? 0), 0);

  return (
    <div data-testid="video-downloads" className="flex flex-col gap-4">
      {downloads.error && <SourceIssue>{downloads.error.message}</SourceIssue>}
      {list && list.items.length === 0 && (
        <EmptyState
          icon={Clapperboard}
          title={t('watch.noDownloads')}
          body={t('watch.noDownloadsHint')}
        />
      )}
      {list && list.items.length > 0 && (
        <div className="flex flex-wrap items-center gap-3">
          {active.length > 0 && (
            <button
              type="button"
              disabled={setPaused.isPending}
              onClick={() => setPaused.mutate(!list.paused)}
              className="bg-surface flex items-center gap-2 rounded-full px-4 py-2 text-sm"
            >
              {list.paused ? (
                <Play aria-hidden size={16} className="text-primary" />
              ) : (
                <Pause aria-hidden size={16} className="text-primary" />
              )}
              {t(list.paused ? 'manga.resumeDownloads' : 'manga.pauseDownloads')}
            </button>
          )}
          {saved.length > 0 && (
            <p className="text-muted text-xs">
              {t('watch.savedVideos', {
                count: saved.length,
                size: formatBytes(savedBytes, i18n.language),
              })}
            </p>
          )}
        </div>
      )}
      {active.length > 0 && (
        <ul aria-label={t('watch.downloading')} className="flex flex-col gap-2">
          {active.map((item) => (
            <Row key={item.id} item={item} paused={list?.paused ?? false} />
          ))}
        </ul>
      )}
      {saved.length > 0 && (
        <section aria-labelledby="saved-videos" className="flex flex-col gap-2">
          <h2 id="saved-videos" className="text-muted text-sm font-medium">
            {t('watch.onThisDevice')}
          </h2>
          <ul className="flex flex-col gap-2">
            {saved.map((item) => (
              <Row key={item.id} item={item} paused={false} />
            ))}
          </ul>
        </section>
      )}
    </div>
  );
};
