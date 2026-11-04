import type { ExportJob } from '@raygun/bindings';
import { FileDown, X } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { EmptyState } from '../shell/EmptyState';
import { useCancelExport, useDismissExport, useExportJobs } from './useExports';

const running = (job: ExportJob) => job.state === 'downloading' || job.state === 'writing';

const JobCard: FC<{ job: ExportJob }> = ({ job }) => {
  const { t } = useTranslation();
  const cancel = useCancelExport();
  const dismiss = useDismissExport();
  const [done, total] =
    job.state === 'downloading' ? [job.downloaded, job.toDownload] : [job.written, job.toWrite];
  const fraction = total > 0 ? Math.min(1, done / total) : 0;
  const status =
    job.state === 'downloading'
      ? t('export.state.downloading', { done, total })
      : job.state === 'writing'
        ? t('export.state.writing', { done, total })
        : t(`export.state.${job.state}`);

  return (
    <li
      data-testid="export-job"
      aria-label={job.label}
      className="bg-surface flex flex-col gap-2 rounded-2xl p-3 text-sm"
    >
      <div className="flex items-start gap-3">
        <FileDown aria-hidden size={20} className="text-primary mt-0.5 shrink-0" />
        <div className="flex min-w-0 flex-1 flex-col">
          <span className="truncate font-medium">{job.label}</span>
          <span className="text-muted text-xs">
            {t(`export.formats.${job.format}`)} · {status}
          </span>
        </div>
        {running(job) ? (
          <button
            type="button"
            onClick={() => cancel.mutate(job.id)}
            className="text-primary shrink-0 rounded-full px-3 py-1 text-xs font-semibold"
          >
            {t('export.cancel')}
          </button>
        ) : (
          <button
            type="button"
            aria-label={t('export.dismiss', { name: job.label })}
            onClick={() => dismiss.mutate(job.id)}
            className="hover:bg-background grid size-8 shrink-0 place-items-center rounded-full"
          >
            <X aria-hidden size={16} />
          </button>
        )}
      </div>
      {running(job) && (
        <div
          role="progressbar"
          aria-label={status}
          aria-valuemin={0}
          aria-valuemax={total}
          aria-valuenow={done}
          className="bg-background h-1.5 overflow-hidden rounded-full"
        >
          <div className="bg-primary h-full" style={{ width: `${fraction * 100}%` }} />
        </div>
      )}
      {job.state === 'failed' ? (
        <div role="alert" className="text-muted flex flex-col gap-1 text-xs">
          <p>{t('export.didntFinish')}</p>
          {job.message && (
            // What went wrong exactly (a folder that can't be written, a full disk…).
            <details>
              <summary className="hover:text-foreground cursor-pointer font-semibold">
                {t('issue.more')}
              </summary>
              <p className="selectable bg-background mt-1 rounded-lg px-3 py-2 font-mono wrap-anywhere">
                {job.message}
              </p>
            </details>
          )}
        </div>
      ) : (
        job.message && <p className="text-muted text-xs">{job.message}</p>
      )}
      {job.failedChapters > 0 && (
        <p className="text-primary text-xs">
          {t('export.failedChapters', { count: job.failedChapters })}
        </p>
      )}
      {job.skippedPages > 0 && (
        <p className="text-muted text-xs">
          {t('export.skippedPages', { count: job.skippedPages })}
        </p>
      )}
      {job.files.length > 0 && (
        <ul className="text-muted flex flex-col gap-0.5 text-xs wrap-anywhere">
          {job.files.slice(0, 5).map((file) => (
            <li key={file}>{file}</li>
          ))}
          {job.files.length > 5 && (
            <li>{t('export.moreFiles', { count: job.files.length - 5 })}</li>
          )}
        </ul>
      )}
    </li>
  );
};

/** Downloads → Exports: this session's exports and saves that failed. */
export const ExportsList: FC = () => {
  const { t } = useTranslation();
  const jobs = useExportJobs();
  if (!jobs.data) return null;
  if (jobs.data.length === 0) {
    return <EmptyState icon={FileDown} title={t('export.noneTitle')} body={t('export.none')} />;
  }
  return (
    <ul data-testid="exports-list" className="flex flex-col gap-2">
      {jobs.data.map((job) => (
        <JobCard key={job.id} job={job} />
      ))}
    </ul>
  );
};
