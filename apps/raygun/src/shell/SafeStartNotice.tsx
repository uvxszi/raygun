import { commands } from '@raygun/bindings';
import type { SafeStartReason } from '@raygun/bindings';
import { useMutation, useQuery } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from './Sheet';

const REASON_KEYS: Record<SafeStartReason, string> = {
  stopped: 'crash.recoveredStopped',
  renderer: 'crash.recoveredRenderer',
  asked: 'crash.recoveredAsked',
};

/**
 * After a crash, the start that follows is a safe one (`crash_log.rs`): this says so once, names
 * the sources that were at work when Raygun stopped (to turn off), and offers the crash logs and
 * clearing the caches.
 */
export const SafeStartNotice: FC = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [closed, setClosed] = useState(false);
  const [turnedOff, setTurnedOff] = useState<string[]>([]);
  const status = useQuery({
    queryKey: ['safeStart'],
    queryFn: () => commands.safeStartStatus(),
    staleTime: Infinity,
  });
  const turnOff = useMutation({
    mutationFn: async (id: string) => {
      const result = await commands.extensionHoldBack(id, true);
      if (result.status === 'error') throw new Error(result.error);
      return id;
    },
    onSuccess: (id) => setTurnedOff((ids) => [...ids, id]),
  });
  const info = status.data;
  if (closed || !info?.active) return null;
  const held = new Set(info.heldNow.map((source) => source.id));
  const suspects = info.suspects.filter((source) => !held.has(source.id));
  const close = () => setClosed(true);
  const pill = 'rounded-full px-4 py-2 text-sm font-semibold';
  return (
    <Sheet title={t('crash.recovered')} onClose={close}>
      <div data-testid="safe-start-notice" className="flex flex-col gap-3">
        <p>{t(info.reason ? REASON_KEYS[info.reason] : 'crash.recoveredStopped')}</p>
        {info.heldNow.length > 0 && (
          <div className="flex flex-col gap-1">
            <span className="text-muted text-sm font-medium">{t('crash.heldNow')}</span>
            <ul className="flex flex-col gap-1">
              {info.heldNow.map((source) => (
                <li key={source.id}>{source.name}</li>
              ))}
            </ul>
          </div>
        )}
        {suspects.length > 0 && (
          <div className="flex flex-col gap-1">
            <span className="text-muted text-sm font-medium">{t('crash.suspects')}</span>
            <ul className="flex flex-col gap-1">
              {suspects.map((source) => (
                <li key={source.id} className="flex items-center justify-between gap-2">
                  <span className="min-w-0 truncate">{source.name}</span>
                  <button
                    type="button"
                    disabled={turnedOff.includes(source.id)}
                    onClick={() => turnOff.mutate(source.id)}
                    className={`${pill} bg-surface disabled:opacity-50`}
                  >
                    {t('crash.turnOff')}
                  </button>
                </li>
              ))}
            </ul>
          </div>
        )}
        <div className="flex flex-wrap gap-2 pt-1">
          <button
            type="button"
            className={`${pill} bg-surface`}
            onClick={() => {
              close();
              void navigate({ to: '/about/crashes', search: { id: undefined } });
            }}
          >
            {t('crash.logs')}
          </button>
          <button
            type="button"
            className={`${pill} bg-surface`}
            onClick={() => void commands.appRestartSafe(true)}
          >
            {t('crash.clearCaches')}
          </button>
          <button type="button" className={`${pill} bg-primary text-white`} onClick={close}>
            {t('crash.close')}
          </button>
        </div>
      </div>
    </Sheet>
  );
};
