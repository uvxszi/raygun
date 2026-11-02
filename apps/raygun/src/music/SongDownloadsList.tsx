import type { SongDownload } from '@raygun/bindings';
import { CircleAlert, Disc3, FileDown, Mic, RotateCw, Trash2 } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Artwork } from '../library/Artwork';
import { EmptyState } from '../shell/EmptyState';
import { formatBytes } from '../watch/episodeLabel';
import { SaveSongsSheet } from './SaveSongsSheet';
import { useSongDownloadActions, useSongDownloads } from './useSongDownloads';

const ICON =
  'text-muted hover:text-foreground grid size-9 shrink-0 place-items-center rounded-full disabled:opacity-50';

/** "Queued", "45% · 3 MB of 7 MB", "On this device · MP3 · Internet Archive"... */
const useStateLine = () => {
  const { t, i18n } = useTranslation();
  const size = (bytes: number) => formatBytes(bytes, i18n.language);
  return (item: SongDownload) => {
    switch (item.state) {
      case 'queued':
        return t('songs.state.queued');
      case 'downloading': {
        const share = item.totalBytes ? (item.doneBytes ?? 0) / item.totalBytes : null;
        return [
          share === null ? t('songs.state.downloading') : `${Math.round(share * 100)}%`,
          item.totalBytes
            ? t('watch.sizeOf', { done: size(item.doneBytes ?? 0), total: size(item.totalBytes) })
            : (item.doneBytes ?? 0) > 0
              ? size(item.doneBytes ?? 0)
              : null,
        ]
          .filter(Boolean)
          .join(' · ');
      }
      case 'recording':
        return t('songs.state.recording');
      case 'done':
        return [t('songs.state.done'), item.format?.toUpperCase(), item.provider]
          .filter(Boolean)
          .join(' · ');
      case 'needsRecording':
        return t('songs.state.needsRecording');
      case 'failed':
        return t('songs.state.failed');
    }
  };
};

const Row: FC<{ item: SongDownload; onSave: () => void }> = ({ item, onSave }) => {
  const { t } = useTranslation();
  const line = useStateLine();
  const { remove, retry, record } = useSongDownloadActions();
  const share =
    item.state === 'downloading' && item.totalBytes
      ? (item.doneBytes ?? 0) / item.totalBytes
      : null;
  return (
    <li
      data-testid="song-download"
      aria-label={item.title}
      className="flex items-center gap-3 rounded-xl px-2 py-2"
    >
      <Artwork url={item.artworkUrl} className="size-12 shrink-0 rounded-md" iconSize={20} />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="truncate font-medium">{item.title}</span>
        <span className="text-muted truncate text-sm">{item.artist}</span>
        <span
          className={`truncate text-xs ${item.state === 'failed' ? 'text-primary' : 'text-muted'}`}
        >
          {line(item)}
        </span>
        {item.error && item.state !== 'done' && (
          <span className="text-muted flex items-start gap-1 text-xs wrap-anywhere">
            <CircleAlert aria-hidden size={12} className="mt-0.5 shrink-0" />
            {item.error}
          </span>
        )}
        {share !== null && (
          <span className="bg-foreground/10 mt-1 h-1 overflow-hidden rounded-full">
            <span
              className="bg-primary block h-full transition-[width]"
              style={{ width: `${Math.round(share * 100)}%` }}
            />
          </span>
        )}
        {(record.error || retry.error) && (
          <span role="alert" className="text-primary text-xs wrap-anywhere">
            {(record.error ?? retry.error)?.message}
          </span>
        )}
      </div>
      {item.state === 'needsRecording' && item.canRecord && (
        <button
          type="button"
          disabled={record.isPending}
          onClick={() => record.mutate(item.id)}
          className="bg-primary text-background flex shrink-0 items-center gap-1.5 rounded-full px-3 py-1.5 text-sm font-semibold disabled:opacity-60"
        >
          <Mic aria-hidden size={14} />
          {t('songs.record')}
        </button>
      )}
      {item.state === 'failed' && (
        <button
          type="button"
          aria-label={t('songs.retry', { title: item.title })}
          onClick={() => retry.mutate(item.id)}
          className={ICON}
        >
          <RotateCw aria-hidden size={18} />
        </button>
      )}
      {item.state === 'done' && (
        <button
          type="button"
          aria-label={t('songs.saveOne', { title: item.title })}
          data-hint={t('songs.saveOne', { title: item.title })}
          onClick={onSave}
          className={ICON}
        >
          <FileDown aria-hidden size={18} />
        </button>
      )}
      <button
        type="button"
        aria-label={t('songs.remove', { title: item.title })}
        data-hint={t('songs.remove', { title: item.title })}
        disabled={remove.isPending}
        onClick={() => remove.mutate(item.id)}
        className={ICON}
      >
        <Trash2 aria-hidden size={18} />
      </button>
    </li>
  );
};

/**
 * Downloads → Music (M46.10): songs saved into the app, with their progress; finished ones save
 * as files, and ones only YouTube has can be recorded as they play.
 */
export const SongDownloadsList: FC = () => {
  const { t } = useTranslation();
  const downloads = useSongDownloads();
  const [saving, setSaving] = useState<string[] | null>(null);
  const items = downloads.data ?? [];
  const done = items.filter((item) => item.state === 'done');

  if (downloads.isSuccess && items.length === 0) {
    return <EmptyState icon={Disc3} title={t('songs.emptyTitle')} body={t('songs.emptyHint')} />;
  }
  return (
    <div data-testid="song-downloads" className="flex flex-col gap-3">
      {done.length > 1 && (
        <button
          type="button"
          onClick={() => setSaving(done.map((item) => item.id))}
          className="bg-surface flex items-center gap-2 self-start rounded-full px-4 py-2 text-sm"
        >
          <FileDown aria-hidden size={16} />
          {t('songs.saveAll', { count: done.length })}
        </button>
      )}
      <ul aria-label={t('songs.title')} className="flex flex-col">
        {items.map((item) => (
          <Row key={item.id} item={item} onSave={() => setSaving([item.id])} />
        ))}
      </ul>
      {saving && <SaveSongsSheet ids={saving} onClose={() => setSaving(null)} />}
    </div>
  );
};
