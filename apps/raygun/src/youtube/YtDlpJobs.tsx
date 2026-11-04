import type { YtDlpJob } from '@raygun/bindings';
import { FolderOpen, Music, RotateCw, Video, X } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { SourceIssue } from '../shell/SourceIssue';
import { useYtDlpActions, useYtDlpJobs } from './useYtDlp';

const running = (job: YtDlpJob) => job.state === 'queued' || job.state === 'downloading';

/** One yt-dlp download (M46.47): its state and progress, Cancel, then Show in folder. */
export const YtDlpJobCard: FC<{ job: YtDlpJob }> = ({ job }) => {
  const { t } = useTranslation();
  const { cancel, reveal, download } = useYtDlpActions();
  const Icon = job.kind === 'audio' ? Music : Video;
  const share = job.progress ?? 0;
  const status =
    job.state === 'downloading' && job.progress !== null
      ? `${Math.round(share * 100)}%`
      : t(`youtube.state.${job.state}`);
  return (
    <li
      data-testid="ytdlp-job"
      aria-label={job.title}
      className="bg-surface flex flex-col gap-2 rounded-2xl p-3 text-sm"
    >
      <div className="flex items-start gap-3">
        <Icon aria-hidden size={20} className="text-primary mt-0.5 shrink-0" />
        <div className="flex min-w-0 flex-1 flex-col">
          <span className="truncate font-medium">{job.title}</span>
          <span className="text-muted text-xs">
            {t(`youtube.kind.${job.kind}`)} · {status}
          </span>
        </div>
        {job.state === 'done' && (
          <button
            type="button"
            onClick={() => reveal.mutate(job.id)}
            className="text-primary flex shrink-0 items-center gap-1 rounded-full px-3 py-1 text-xs font-semibold"
          >
            <FolderOpen aria-hidden size={14} />
            {t('youtube.showFile')}
          </button>
        )}
        {(job.state === 'failed' || job.state === 'cancelled') && (
          <button
            type="button"
            disabled={download.isPending}
            onClick={() =>
              // A new job takes its place in the list.
              download.mutate(
                { url: job.url, title: job.title, kind: job.kind },
                { onSuccess: () => cancel.mutate(job.id) },
              )
            }
            className="text-primary flex shrink-0 items-center gap-1 rounded-full px-3 py-1 text-xs font-semibold disabled:opacity-50"
          >
            <RotateCw aria-hidden size={14} />
            {t('youtube.tryAgain')}
          </button>
        )}
        {running(job) ? (
          <button
            type="button"
            aria-label={t('export.cancelOne', { name: job.title })}
            onClick={() => cancel.mutate(job.id)}
            className="text-primary shrink-0 rounded-full px-3 py-1 text-xs font-semibold"
          >
            {t('export.cancel')}
          </button>
        ) : (
          <button
            type="button"
            aria-label={t('export.dismiss', { name: job.title })}
            onClick={() => cancel.mutate(job.id)}
            className="hover:bg-background grid size-8 shrink-0 place-items-center rounded-full"
          >
            <X aria-hidden size={16} />
          </button>
        )}
      </div>
      {job.state === 'downloading' && (
        <div
          role="progressbar"
          aria-label={status}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.round(share * 100)}
          className="bg-background h-1.5 overflow-hidden rounded-full"
        >
          <div
            className="bg-primary h-full origin-left transition-transform"
            style={{ transform: `scaleX(${Math.min(1, share)})` }}
          />
        </div>
      )}
      {job.file && <p className="text-muted text-xs wrap-anywhere">{job.file}</p>}
      {job.message &&
        (job.state === 'failed' ? (
          <SourceIssue compact title={t('youtube.failed')}>
            {job.message}
          </SourceIssue>
        ) : (
          <p className="text-muted text-xs">{job.message}</p>
        ))}
      {(reveal.error ?? download.error) && (
        <SourceIssue
          compact
          title={t(reveal.error ? 'youtube.revealFailed' : 'youtube.downloadFailed')}
        >
          {(reveal.error ?? download.error)?.message}
        </SourceIssue>
      )}
    </li>
  );
};

/** Downloads → Exports: what yt-dlp saved from YouTube addresses this session (M46.47). */
export const YtDlpJobs: FC = () => {
  const { t } = useTranslation();
  const jobs = useYtDlpJobs();
  if (!jobs.data?.length) return null;
  return (
    <section aria-label={t('youtube.downloads')} className="flex flex-col gap-2">
      <h2 className="text-muted text-sm font-semibold">{t('youtube.downloads')}</h2>
      <ul className="flex flex-col gap-2">
        {jobs.data.map((job) => (
          <YtDlpJobCard key={job.id} job={job} />
        ))}
      </ul>
    </section>
  );
};
