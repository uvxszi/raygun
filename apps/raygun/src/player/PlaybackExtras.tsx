import type { PlaybackState, SleepTimer } from '@raygun/bindings';
import { Gauge } from 'lucide-react';
import { useEffect, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';
import { sendControl } from './usePlayback';

/** Speeds offered (`PLAYBACK_RATES` in raygun-model). */
const RATES = [0.5, 0.75, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2];
const SLEEP_MINUTES = [5, 10, 15, 30, 45, 60, 90];

const CHIP = 'rounded-full px-3 py-1.5 text-sm tabular-nums';

/** Minutes left on the sleep timer, refreshed while it runs. */
const useMinutesLeft = (sleepAt: number | null | undefined) => {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!sleepAt) return;
    const timer = window.setInterval(() => setNow(Date.now()), 15_000);
    return () => window.clearInterval(timer);
  }, [sleepAt]);
  return sleepAt ? Math.max(1, Math.ceil((sleepAt - now) / 60_000)) : null;
};

/**
 * Speed and the sleep timer (M46.8), behind one button in the player bar and the full player.
 * The button shows a running timer's minutes, or a speed other than normal.
 */
export const PlaybackExtras: FC<{ state: PlaybackState | undefined; className?: string }> = ({
  state,
  className = '',
}) => {
  const { t } = useTranslation();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const rate = state?.rate ?? 1;
  const minutesLeft = useMinutesLeft(state?.sleepAt);
  const sleeping = minutesLeft !== null || state?.stopAfterCurrent;
  const setTimer = (timer: SleepTimer) => void sendControl({ type: 'setSleepTimer', timer });
  const badge =
    minutesLeft !== null
      ? `${minutesLeft}′`
      : state?.stopAfterCurrent
        ? '⏾'
        : rate !== 1
          ? `${rate}×`
          : null;

  return (
    <>
      <button
        type="button"
        aria-label={t('player.extras.open')}
        aria-haspopup="dialog"
        aria-expanded={anchor !== null}
        onClick={(event) => setAnchor(anchor ? null : event.currentTarget)}
        className={`flex h-9 min-w-9 items-center justify-center gap-1 rounded-full px-1.5 text-xs font-semibold ${
          sleeping || rate !== 1 ? 'text-primary' : 'text-muted hover:text-foreground'
        } ${className}`}
      >
        <Gauge aria-hidden size={18} />
        {badge && <span className="tabular-nums">{badge}</span>}
      </button>
      {anchor && (
        <Sheet
          title={t('player.extras.title')}
          anchor={anchor}
          popoverWidth={300}
          onClose={() => setAnchor(null)}
        >
          <div className="flex flex-col gap-4 px-1">
            <section aria-label={t('player.extras.speed')} className="flex flex-col gap-2">
              <h3 className="text-sm font-semibold">{t('player.extras.speed')}</h3>
              <div
                role="radiogroup"
                aria-label={t('player.extras.speed')}
                className="flex flex-wrap gap-1.5"
              >
                {RATES.map((value) => (
                  <button
                    key={value}
                    type="button"
                    role="radio"
                    aria-checked={rate === value}
                    onClick={() => void sendControl({ type: 'setRate', rate: value })}
                    className={`${CHIP} ${rate === value ? 'bg-primary text-background font-semibold' : 'bg-background'}`}
                  >
                    {value}×
                  </button>
                ))}
              </div>
            </section>
            <section aria-label={t('player.extras.sleep')} className="flex flex-col gap-2">
              <h3 className="text-sm font-semibold">
                {t('player.extras.sleep')}
                {minutesLeft !== null && (
                  <span className="text-primary font-normal">
                    {' · '}
                    {t('player.extras.minutesLeft', { count: minutesLeft })}
                  </span>
                )}
              </h3>
              <div
                role="radiogroup"
                aria-label={t('player.extras.sleep')}
                className="flex flex-wrap gap-1.5"
              >
                <button
                  type="button"
                  role="radio"
                  aria-checked={!sleeping}
                  onClick={() => setTimer({ kind: 'off' })}
                  className={`${CHIP} ${!sleeping ? 'bg-primary text-background font-semibold' : 'bg-background'}`}
                >
                  {t('player.extras.off')}
                </button>
                {SLEEP_MINUTES.map((minutes) => (
                  <button
                    key={minutes}
                    type="button"
                    role="radio"
                    aria-checked={false}
                    onClick={() => setTimer({ kind: 'minutes', minutes })}
                    className={`${CHIP} bg-background`}
                  >
                    {t('player.extras.minutes', { count: minutes })}
                  </button>
                ))}
                <button
                  type="button"
                  role="radio"
                  aria-checked={!!state?.stopAfterCurrent}
                  onClick={() => setTimer({ kind: 'endOfTrack' })}
                  className={`${CHIP} ${state?.stopAfterCurrent ? 'bg-primary text-background font-semibold' : 'bg-background'}`}
                >
                  {t('player.extras.endOfSong')}
                </button>
              </div>
            </section>
          </div>
        </Sheet>
      )}
    </>
  );
};
