import { LoaderCircle, RotateCw, X } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useInstallQueue } from './installQueue';

/**
 * The extension install queue (Rust's, so it carries on in the background): while it runs,
 * "Installing extensions · 3 of 12", the one installing now and a bar, with Cancel for the rest;
 * after a batch with failures, its counts with Retry.
 */
export const InstallProgress: FC = () => {
  const { t } = useTranslation();
  const { queue, cancelAll, retry, dismiss } = useInstallQueue();
  const failures = queue.items.filter((item) => item.state === 'failed').length;
  const waiting = queue.items.some((item) => item.state === 'queued');

  if (queue.running && queue.total > 0) {
    const ended = queue.done + queue.failed;
    const position = Math.min(ended + 1, queue.total);
    const step =
      queue.phase === 'loading'
        ? t('extensions.installs.phase.loading')
        : [queue.current, queue.phase && t(`extensions.installs.phase.${queue.phase}`)]
            .filter(Boolean)
            .join(' · ');
    return (
      <div
        role="status"
        aria-label={t('extensions.installs.label')}
        data-testid="install-progress"
        className="bg-surface flex flex-col gap-2 rounded-2xl px-4 py-3"
      >
        <div className="flex items-center gap-3">
          <LoaderCircle aria-hidden size={18} className="text-primary shrink-0 animate-spin" />
          <span className="flex min-w-0 flex-1 flex-col">
            <span className="text-sm font-medium">
              {t('extensions.installs.title', { position, total: queue.total })}
            </span>
            {step && <span className="text-muted truncate text-xs">{step}</span>}
          </span>
          {waiting && (
            <button
              type="button"
              onClick={cancelAll}
              className="hover:bg-background rounded-full px-3 py-1.5 text-sm font-semibold"
            >
              {t('extensions.installs.cancel')}
            </button>
          )}
        </div>
        <div
          role="progressbar"
          aria-label={t('extensions.installs.label')}
          aria-valuemin={0}
          aria-valuemax={queue.total}
          aria-valuenow={ended}
          className="bg-background h-1 overflow-hidden rounded-full"
        >
          <div
            className="bg-primary h-full origin-left transition-transform"
            style={{ transform: `scaleX(${queue.total ? ended / queue.total : 0})` }}
          />
        </div>
      </div>
    );
  }

  if (failures === 0) return null;
  const counts = [
    queue.done > 0 && t('extensions.installs.installed', { count: queue.done }),
    t('extensions.installs.failed', { count: failures }),
  ]
    .filter(Boolean)
    .join(' · ');
  return (
    <div
      role="status"
      aria-label={t('extensions.installs.label')}
      data-testid="install-progress"
      className="bg-surface flex items-center gap-2 rounded-2xl px-4 py-2.5"
    >
      <span className="min-w-0 flex-1 text-sm">{counts}</span>
      <button
        type="button"
        onClick={retry}
        className="hover:bg-background flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm font-semibold"
      >
        <RotateCw aria-hidden size={14} />
        {t('extensions.installs.retry')}
      </button>
      <button
        type="button"
        aria-label={t('extensions.installs.dismiss')}
        onClick={dismiss}
        className="text-muted hover:text-foreground grid size-8 place-items-center rounded-full"
      >
        <X aria-hidden size={16} />
      </button>
    </div>
  );
};
