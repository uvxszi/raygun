import { WifiOff } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useOfflineMode } from './useNetworkMode';

type OfflineNoteProps = {
  /** One line, for rows and small spaces. */
  compact?: boolean;
  testId?: string;
  className?: string;
};

/**
 * What something online-only shows while Offline Mode is on: calm, saying why, with a way to
 * turn it off. Nothing failed, so there's no error and nothing to try again.
 */
export const OfflineNote: FC<OfflineNoteProps> = ({ compact = false, testId, className = '' }) => {
  const { t } = useTranslation();
  const offline = useOfflineMode();
  return (
    <div
      role="status"
      data-testid={testId ?? 'offline-note'}
      className={`bg-surface/70 text-muted flex flex-col gap-1 rounded-xl text-sm ${
        compact ? 'px-3 py-2' : 'px-4 py-3'
      } ${className}`}
    >
      <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
        <WifiOff aria-hidden size={16} className="shrink-0" />
        <span className="text-foreground min-w-0 font-medium">{t('offline.note')}</span>
        {offline.on && (
          <button
            type="button"
            disabled={offline.busy}
            onClick={() => offline.set(false)}
            className="hover:text-foreground hover:bg-background ml-auto rounded-full px-2.5 py-1 text-xs font-semibold disabled:opacity-60"
          >
            {t('offline.turnOff')}
          </button>
        )}
      </div>
      {!compact && <p>{t('offline.noteHint')}</p>}
    </div>
  );
};
