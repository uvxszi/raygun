import type { SongDownload, SongDownloadProblem } from '@raygun/bindings';
import { useTranslation } from 'react-i18next';

import { formatTime } from '../player/formatTime';
import { formatBytes } from '../watch/episodeLabel';

/**
 * Spins and pulses only where things may move (`data-motion` lively or calm: Settings →
 * Appearance → Animations, lowered by the system's animation settings, battery saver and reduced
 * motion); elsewhere they stand still. They animate transform and opacity only.
 */
export const SPIN = '[:root:is([data-motion=lively],[data-motion=calm])_&]:animate-spin';
export const PULSE = '[:root:is([data-motion=lively],[data-motion=calm])_&]:animate-pulse';
/** A progress bar's fill glides between reports (a transform, so nothing is laid out again). */
export const GLIDE =
  '[:root:is([data-motion=lively],[data-motion=calm])_&]:transition-transform [:root:is([data-motion=lively],[data-motion=calm])_&]:duration-300 ease-linear';

/** On its way: queued or downloading (a capture included). */
export const isBusy = (item: SongDownload) =>
  item.state === 'queued' || item.state === 'downloading';

/** Stopped without a file. */
export const isFailed = (item: SongDownload) => item.state === 'failed';

/** Saved, with something a check found wrong (M46.49). */
export const needsAttention = (item: SongDownload) =>
  item.state === 'done' && item.problem !== null;

/** How far it is, 0–1; `null` while that isn't known. */
export const shareOf = (item: SongDownload) => {
  if (item.state !== 'downloading') return null;
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
    /** Downloading or being captured now. */
    running: list.filter((item) => item.state === 'downloading'),
    /** Waiting, in the order they'll download. */
    queued: list
      .filter((item) => item.state === 'queued')
      .sort((a, b) => placeOf(a) - placeOf(b) || a.id.localeCompare(b.id)),
    failed: list.filter(isFailed).sort(newestFirst),
    /** Saved songs a check found something wrong with (M46.49). */
    attention: list.filter(needsAttention).sort(newestFirst),
    done: list.filter((item) => item.state === 'done' && !needsAttention(item)).sort(newestFirst),
  };
};

/**
 * One line saying where a download stands: "Queued", "Audius · 45% · 3.1 MB of 7 MB · 0:12
 * left", "Capturing from Radio · 40%", "On this device · MP3 · 7.2 MB · Internet Archive"...
 * `held` when battery saver holds the queue.
 */
export const useStateLine = () => {
  const { t, i18n } = useTranslation();
  const size = (bytes: number) => formatBytes(bytes, i18n.language);
  return (item: SongDownload, paused = false, held = false) => {
    const again = item.replacing ? t('songs.state.again') : null;
    switch (item.state) {
      case 'queued':
        return [
          paused
            ? t('songs.state.paused')
            : held
              ? t('songs.state.batterySaver')
              : t('songs.state.queued'),
          again,
          item.provider,
        ]
          .filter(Boolean)
          .join(' · ');
      case 'downloading': {
        const share = shareOf(item);
        const done = item.doneBytes ?? 0;
        if (item.kind === 'capture') {
          return [
            again,
            item.provider
              ? t('songs.state.capturingFrom', { source: item.provider })
              : t('songs.state.capturing'),
            share === null ? null : `${Math.round(share * 100)}%`,
            done > 0 ? size(done) : null,
          ]
            .filter(Boolean)
            .join(' · ');
        }
        const moving = done > 0 || share !== null;
        return [
          again,
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
      case 'done':
        return [
          t('songs.state.done'),
          item.format?.toUpperCase(),
          item.totalBytes ? size(item.totalBytes) : null,
          item.kind === 'capture' ? t('songs.state.captured') : null,
          item.provider,
        ]
          .filter(Boolean)
          .join(' · ');
      case 'failed':
        return t('songs.state.failed');
    }
  };
};

/**
 * What a check found wrong with a saved song, in a line (M46.49): "The file is gone", "Its source
 * changed: Audius → archive.org", "Its details changed: Swim – BTS → Swim Good – Frank Ocean".
 */
export const useProblemLine = () => {
  const { t } = useTranslation();
  return (problem: SongDownloadProblem) => {
    const change =
      problem.was && problem.now ? `${problem.was} → ${problem.now}` : (problem.now ?? '');
    switch (problem.kind) {
      case 'missing':
        return t('songs.problem.missing');
      case 'damaged':
        return t('songs.problem.damaged');
      case 'sourceChanged':
        return t('songs.problem.sourceChanged', { change });
      case 'detailsChanged':
        return t('songs.problem.detailsChanged', { change });
    }
  };
};
