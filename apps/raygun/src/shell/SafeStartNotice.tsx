import { commands } from '@raygun/bindings';
import type { SafeStartInfo, SafeStartReason } from '@raygun/bindings';
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

/** Kinds of crash record the notice can name (`crash_log.rs`, `CrashLog.kt`). */
const KNOWN_KINDS = new Set([
  'panic',
  'java',
  'native',
  'anr',
  'memory',
  'exit',
  'start',
  'renderer',
  'page',
  'ui',
]);

/**
 * What the notice says happened: the last run's own crash record when there is one (an
 * exception, a native crash, the system ending it…), else why this start is a safe one.
 */
const recoveredKey = (info: SafeStartInfo): string[] => {
  if (info.reason === 'asked') return [REASON_KEYS.asked];
  const kind = info.lastCrash?.kind;
  if (kind && KNOWN_KINDS.has(kind))
    return [`crash.recoveredKinds.${kind}`, 'crash.recoveredSkipped'];
  return [info.reason ? REASON_KEYS[info.reason] : REASON_KEYS.stopped];
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
        <p>
          {recoveredKey(info)
            .map((key) => t(key))
            .join(' ')}
        </p>
        {info.lastCrash?.summary && (
          <p className="text-muted text-sm break-words">{info.lastCrash.summary}</p>
        )}
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
