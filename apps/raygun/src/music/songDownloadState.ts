import type { SongDownload } from '@raygun/bindings';
import { useTranslation } from 'react-i18next';

import { formatTime } from '../player/formatTime';
import { formatBytes } from '../watch/episodeLabel';

/**
 * Spins and pulses only where motion is on (Settings → Appearance → Animations, and the system's
 * reduced motion); they animate transform and opacity only.
 */
export const SPIN = 'motion-safe:[:root:not([data-motion=off])_&]:animate-spin';
export const PULSE = 'motion-safe:[:root:not([data-motion=off])_&]:animate-pulse';
/** A progress bar's fill glides between reports (a transform, so nothing is laid out again). */
export const GLIDE =
  'motion-safe:[:root:not([data-motion=off])_&]:transition-transform motion-safe:[:root:not([data-motion=off])_&]:duration-300 ease-linear';

/** On its way: queued, downloading or being recorded. */
export const isBusy = (item: SongDownload) =>
  item.state === 'queued' || item.state === 'downloading' || item.state === 'recording';

/** Stopped without a file: failed, or waiting to be recorded. */
export const isFailed = (item: SongDownload) =>
  item.state === 'failed' || item.state === 'needsRecording';

/** How far it is, 0–1; `null` while that isn't known. */
export const shareOf = (item: SongDownload) => {
  if (item.state !== 'downloading' && item.state !== 'recording') return null;
  if (item.fraction !== null) return Math.min(1, Math.max(0, item.fraction));
  return item.totalBytes ? Math.min(1, (item.doneBytes ?? 0) / item.totalBytes) : null;
};

/** Its place in the queue (earlier downloads first). */
export const placeOf = (item: SongDownload) => item.queuedAt ?? 0;

/** The downloads in the groups the list shows, each in its own order. */
export const sectionsOf = (list: readonly SongDownload[]) => {
  const newestFirst = (a: SongDownload, b: SongDownload) =>
    (b.finishedAt ?? placeOf(b)) - (a.finishedAt ?? placeOf(a));
  return {
    /** Downloading or being recorded now. */
    running: list.filter((item) => item.state === 'downloading' || item.state === 'recording'),
    /** Waiting, in the order they'll download. */
    queued: list
      .filter((item) => item.state === 'queued')
      .sort((a, b) => placeOf(a) - placeOf(b) || a.id.localeCompare(b.id)),
    failed: list.filter(isFailed).sort(newestFirst),
    done: list.filter((item) => item.state === 'done').sort(newestFirst),
  };
};

/**
 * One line saying where a download stands: "Queued", "Audius · 45% · 3.1 MB of 7 MB · 0:12
 * left", "On this device · MP3 · 7.2 MB · Internet Archive"...
 */
export const useStateLine = () => {
  const { t, i18n } = useTranslation();
  const size = (bytes: number) => formatBytes(bytes, i18n.language);
  return (item: SongDownload, paused = false) => {
    switch (item.state) {
      case 'queued':
        return [paused ? t('songs.state.paused') : t('songs.state.queued'), item.provider]
          .filter(Boolean)
          .join(' · ');
      case 'downloading': {
        const share = shareOf(item);
        const done = item.doneBytes ?? 0;
        const moving = done > 0 || share !== null;
        return [
          item.provider
            ? moving
              ? item.provider
              : t('songs.state.from', { source: item.provider })
            : t('songs.state.finding'),
          share === null ? null : `${Math.round(share * 100)}%`,
          item.totalBytes
            ? t('watch.sizeOf', { done: size(done), total: size(item.totalBytes) })
            : done > 0
              ? size(done)
              : null,
          item.etaMs !== null && item.etaMs > 0
            ? t('songs.left', { time: formatTime(item.etaMs) })
            : item.bytesPerSecond
              ? t('songs.perSecond', { size: size(item.bytesPerSecond) })
              : null,
        ]
          .filter(Boolean)
          .join(' · ');
      }
      case 'recording':
        return t('songs.state.recording');
      case 'done':
        return [
          t('songs.state.done'),
          item.format?.toUpperCase(),
          item.totalBytes ? size(item.totalBytes) : null,
          item.provider,
        ]
          .filter(Boolean)
          .join(' · ');
      case 'needsRecording':
        return t('songs.state.needsRecording');
      case 'failed':
        return t('songs.state.failed');
    }
  };
};
