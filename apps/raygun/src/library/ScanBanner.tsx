import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useScanStore } from './useLibrary';

/** Progress of a running scan; shows the outcome of a failed one. */
export const ScanBanner: FC = () => {
  const { t } = useTranslation();
  const progress = useScanStore((state) => state.progress);
  if (!progress || progress.phase === 'done') {
    return null;
  }
  if (progress.phase === 'failed') {
    return (
      <p role="alert" className="text-primary text-sm">
        {t('library.scanFailed', { message: progress.message ?? '' })}
      </p>
    );
  }
  const percent =
    progress.phase === 'reading' && progress.toRead > 0
      ? Math.round((progress.read / progress.toRead) * 100)
      : null;
  return (
    <div data-testid="scan-banner" role="status" className="flex flex-col gap-1.5 text-sm">
      <span className="text-muted">
        {progress.phase === 'discovering'
          ? t('library.scanDiscovering')
          : t('library.scanReading', { read: progress.read, total: progress.toRead })}
      </span>
      <div className="bg-surface h-1 overflow-hidden rounded-full">
        <div
          className="bg-primary h-full transition-[width]"
          style={{ width: percent === null ? '15%' : `${percent}%` }}
        />
      </div>
    </div>
  );
};
