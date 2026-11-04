import type { SleepTimer } from '@raygun/bindings';
import { Gauge } from 'lucide-react';
import { useEffect, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';
import { sendControl, usePlayerSelect } from './usePlayback';

/** Speeds offered (`PLAYBACK_RATES` in raygun-model). */
const RATES = [0.5, 0.75, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2];
const SLEEP_MINUTES = [5, 10, 15, 30, 45, 60, 90];

const CHIP = 'rounded-full px-3 py-1.5 text-sm tabular-nums';
const CHIP_ON = 'bg-primary text-background font-semibold';
const CHIP_OFF = 'bg-background hover:bg-background/70';

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

/** The speed and the sleep timer, re-rendering only when one of them changes. */
const useExtras = () => {
  const rate = usePlayerSelect((state) => state?.rate ?? 1);
  const sleepAt = usePlayerSelect((state) => state?.sleepAt ?? null);
  const afterSong = usePlayerSelect((state) => state?.stopAfterCurrent ?? false);
  return { rate, sleepAt, afterSong };
};

/**
 * Speed and the sleep timer (M46.8), behind one button in the player bar and the full player.
 * The button shows a running timer's minutes, or a speed other than normal.
 */
export const PlaybackExtras: FC<{ className?: string }> = ({ className = '' }) => {
  const { t } = useTranslation();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const { rate, sleepAt, afterSong } = useExtras();
  const minutesLeft = useMinutesLeft(sleepAt);
  const sleeping = minutesLeft !== null || afterSong;
  const badge =
    minutesLeft !== null ? `${minutesLeft}′` : afterSong ? '⏾' : rate !== 1 ? `${rate}×` : null;
  const label = [
    t('player.extras.open'),
    minutesLeft !== null
      ? t('player.extras.pausesIn', { count: minutesLeft })
      : afterSong
        ? t('player.extras.pausesAfterSong')
        : null,
    rate !== 1 ? `${rate}×` : null,
  ]
    .filter(Boolean)
    .join(' · ');

  return (
    <>
      <button
        type="button"
        aria-label={t('player.extras.open')}
        title={label}
        aria-haspopup="dialog"
        aria-expanded={anchor !== null}
        onClick={(event) => setAnchor(anchor ? null : event.currentTarget)}
        className={`flex h-9 min-w-9 shrink-0 items-center justify-center gap-1 rounded-full px-1.5 text-xs font-semibold ${
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
          <ExtrasPanel />
        </Sheet>
      )}
    </>
  );
};

const ExtrasPanel: FC = () => {
  const { t } = useTranslation();
  const { rate, sleepAt, afterSong } = useExtras();
  const minutesLeft = useMinutesLeft(sleepAt);
  const setTimer = (timer: SleepTimer) => void sendControl({ type: 'setSleepTimer', timer });
  const status =
    minutesLeft !== null
      ? t('player.extras.pausesIn', { count: minutesLeft })
      : afterSong
        ? t('player.extras.pausesAfterSong')
        : null;

  return (
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
              className={`${CHIP} ${rate === value ? CHIP_ON : CHIP_OFF}`}
            >
              {value}×
            </button>
          ))}
        </div>
      </section>
      <section aria-label={t('player.extras.sleep')} className="flex flex-col gap-2">
        <h3 className="text-sm font-semibold">{t('player.extras.sleep')}</h3>
        {status && (
          <div className="bg-background flex items-center gap-2 rounded-xl px-3 py-2">
            <p role="status" className="text-primary min-w-0 flex-1 text-sm font-medium">
              {status}
            </p>
            <button
              type="button"
              onClick={() => setTimer({ kind: 'off' })}
              className="hover:text-foreground text-muted shrink-0 rounded-full px-2 py-0.5 text-sm font-semibold"
            >
              {t('player.extras.turnOff')}
            </button>
          </div>
        )}
        <div className="flex flex-wrap gap-1.5">
          {SLEEP_MINUTES.map((minutes) => (
            <button
              key={minutes}
              type="button"
              onClick={() => setTimer({ kind: 'minutes', minutes })}
              className={`${CHIP} ${CHIP_OFF}`}
            >
              {t('player.extras.minutes', { count: minutes })}
            </button>
          ))}
          <button
            type="button"
            aria-pressed={afterSong}
            onClick={() => setTimer(afterSong ? { kind: 'off' } : { kind: 'endOfTrack' })}
            className={`${CHIP} ${afterSong ? CHIP_ON : CHIP_OFF}`}
          >
            {t('player.extras.endOfSong')}
          </button>
        </div>
      </section>
    </div>
  );
};
