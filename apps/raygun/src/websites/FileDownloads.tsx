import type { WebsiteDownload } from '@raygun/bindings';
import { ExternalLink, FileDown, FolderOpen, RotateCw, Trash2, X } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { formatBytes } from '../lib/formatBytes';
import { EmptyState } from '../shell/EmptyState';
import { SourceIssue } from '../shell/SourceIssue';
import { useFileDownloadActions, useFileDownloads } from './useFileDownloads';

const running = (file: WebsiteDownload) => file.state === 'queued' || file.state === 'downloading';

const action =
  'text-primary flex shrink-0 items-center gap-1 rounded-full px-3 py-1 text-xs font-semibold disabled:opacity-50';

/** What a file's line under its name says: progress, where it went, or its state. */
const useStatus = (file: WebsiteDownload) => {
  const { t } = useTranslation();
  const done = file.doneBytes ?? 0;
  const total = file.totalBytes ?? null;
  switch (file.state) {
    case 'downloading': {
      const amount = total
        ? t('downloads.files.ofSize', { done: formatBytes(done), total: formatBytes(total) })
        : formatBytes(done);
      const speed = file.bytesPerSecond
        ? t('downloads.files.speed', { speed: formatBytes(file.bytesPerSecond) })
        : null;
      return [amount, speed].filter(Boolean).join(' · ');
    }
    case 'done':
      return [total ? formatBytes(total) : null, file.folder].filter(Boolean).join(' · ');
    default:
      return t(`downloads.files.state.${file.state}`);
  }
};

/** One file: its progress and Cancel; then Open, Show in folder and Delete (asked first). */
export const FileDownloadRow: FC<{ file: WebsiteDownload }> = ({ file }) => {
  const { t } = useTranslation();
  const { cancel, retry, remove, open, reveal } = useFileDownloadActions();
  const [confirming, setConfirming] = useState(false);
  const status = useStatus(file);
  const share =
    file.totalBytes && file.totalBytes > 0
      ? Math.min(1, (file.doneBytes ?? 0) / file.totalBytes)
      : null;
  const failure = open.error ?? reveal.error ?? remove.error ?? retry.error;
  return (
    <li
      data-testid="file-download"
      aria-label={file.name}
      className="bg-surface flex flex-col gap-2 rounded-2xl p-3 text-sm"
    >
      <div className="flex items-start gap-3">
        <FileDown aria-hidden size={20} className="text-primary mt-0.5 shrink-0" />
        <div className="flex min-w-0 flex-1 flex-col">
          <span className="truncate font-medium">{file.name}</span>
          <span className="text-muted truncate text-xs">
            {file.site ? `${file.site} · ${status}` : status}
          </span>
        </div>
        {running(file) ? (
          <button
            type="button"
            aria-label={t('downloads.files.cancelOne', { name: file.name })}
            onClick={() => cancel.mutate(file.id)}
            className={action}
          >
            {t('export.cancel')}
          </button>
        ) : (
          <button
            type="button"
            aria-label={t('downloads.files.removeOne', { name: file.name })}
            onClick={() => remove.mutate({ id: file.id, deleteFile: false })}
            className="hover:bg-background grid size-8 shrink-0 place-items-center rounded-full"
          >
            <X aria-hidden size={16} />
          </button>
        )}
      </div>
      {file.state === 'downloading' && (
        <div
          role="progressbar"
          aria-label={status}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={share === null ? undefined : Math.round(share * 100)}
          className="bg-background h-1.5 overflow-hidden rounded-full"
        >
          <div
            className="bg-primary h-full origin-left transition-transform"
            style={{ transform: `scaleX(${share ?? 0})` }}
          />
        </div>
      )}
      {file.state === 'failed' && file.error && (
        <SourceIssue compact title={t('downloads.files.failed', { site: file.site ?? file.name })}>
          {file.error}
        </SourceIssue>
      )}
      {(file.state === 'failed' || file.state === 'cancelled') && (
        <div className="flex flex-wrap gap-1">
          <button
            type="button"
            disabled={retry.isPending}
            onClick={() => retry.mutate(file.id)}
            className={action}
          >
            <RotateCw aria-hidden size={14} />
            {t(
              file.resumable && file.state === 'failed'
                ? 'downloads.files.resume'
                : 'downloads.files.retry',
            )}
          </button>
        </div>
      )}
      {file.state === 'done' &&
        (confirming ? (
          <div
            role="alertdialog"
            aria-label={t('downloads.files.deleteAsk', { name: file.name })}
            className="flex flex-wrap items-center gap-2"
          >
            <span className="flex-1 text-xs">
              {t('downloads.files.deleteAsk', { name: file.name })}
            </span>
            <button
              type="button"
              autoFocus
              onClick={() => setConfirming(false)}
              className="hover:bg-background rounded-full px-3 py-1 text-xs"
            >
              {t('export.cancel')}
            </button>
            <button
              type="button"
              disabled={remove.isPending}
              onClick={() =>
                remove.mutate(
                  { id: file.id, deleteFile: true },
                  { onSettled: () => setConfirming(false) },
                )
              }
              className="bg-primary text-background rounded-full px-3 py-1 text-xs font-semibold disabled:opacity-50"
            >
              {t('downloads.files.delete')}
            </button>
          </div>
        ) : (
          <div className="flex flex-wrap gap-1">
            <button type="button" onClick={() => open.mutate(file.id)} className={action}>
              <ExternalLink aria-hidden size={14} />
              {t('downloads.files.open')}
            </button>
            <button type="button" onClick={() => reveal.mutate(file.id)} className={action}>
              <FolderOpen aria-hidden size={14} />
              {t('downloads.files.showFolder')}
            </button>
            <button type="button" onClick={() => setConfirming(true)} className={action}>
              <Trash2 aria-hidden size={14} />
              {t('downloads.files.delete')}
            </button>
          </div>
        ))}
      {failure && (
        <SourceIssue compact title={t('downloads.files.actionFailed')}>
          {failure.message}
        </SourceIssue>
      )}
    </li>
  );
};

/** Downloads → Files: what was saved from sites in the built-in browser. */
export const FileDownloadsList: FC<{ compact?: boolean }> = ({ compact = false }) => {
  const { t } = useTranslation();
  const files = useFileDownloads();
  // Downloads → All leaves out cancelled ones.
  const list = (files.data ?? []).filter((file) => !compact || file.state !== 'cancelled');
  if (!list.length) {
    return compact || files.isPending ? null : (
      <EmptyState icon={FileDown} title={t('downloads.files.none')} />
    );
  }
  return (
    <ul aria-label={t('downloads.files.tab')} className="flex flex-col gap-2">
      {list.map((file) => (
        <FileDownloadRow key={file.id} file={file} />
      ))}
    </ul>
  );
};
