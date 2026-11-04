import { Gauge } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { SourceIssue } from '../shell/SourceIssue';
import { Sheet } from '../shell/Sheet';
import { useSetShowSpeed } from './usePodcasts';

/** Speeds offered (`PLAYBACK_RATES` in raygun-model). */
const RATES = [0.5, 0.75, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2];

const CHIP = 'rounded-full px-3 py-1.5 text-sm tabular-nums';

/**
 * The speed a show's episodes or a book's chapters play at (M46.48), kept for it: they start at
 * it, and a change in the player while one plays sticks with it. Without one they play at the
 * player's speed.
 */
export const ShowSpeedButton: FC<{ id: string; speed: number | null }> = ({ id, speed }) => {
  const { t } = useTranslation();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const setSpeed = useSetShowSpeed();
  const pick = (value: number | null) =>
    setSpeed.mutate({ id, speed: value }, { onSuccess: () => setAnchor(null) });
  return (
    <>
      <button
        type="button"
        aria-haspopup="dialog"
        aria-label={t('podcasts.speed.open')}
        onClick={(event) => setAnchor(anchor ? null : event.currentTarget)}
        className={`bg-surface flex items-center gap-1.5 rounded-full px-4 py-1.5 text-sm ${speed !== null ? 'text-primary font-semibold' : ''}`}
      >
        <Gauge aria-hidden size={16} />
        {speed !== null ? `${speed}×` : t('podcasts.speed.label')}
      </button>
      {anchor && (
        <Sheet
          title={t('podcasts.speed.title')}
          anchor={anchor}
          popoverWidth={300}
          onClose={() => setAnchor(null)}
        >
          <div
            role="radiogroup"
            aria-label={t('podcasts.speed.title')}
            className="flex flex-wrap gap-1.5 px-1 pb-2"
          >
            <button
              type="button"
              role="radio"
              aria-checked={speed === null}
              disabled={setSpeed.isPending}
              onClick={() => pick(null)}
              className={`${CHIP} ${speed === null ? 'bg-primary text-background font-semibold' : 'bg-background'}`}
            >
              {t('podcasts.speed.player')}
            </button>
            {RATES.map((rate) => (
              <button
                key={rate}
                type="button"
                role="radio"
                aria-checked={speed === rate}
                disabled={setSpeed.isPending}
                onClick={() => pick(rate)}
                className={`${CHIP} ${speed === rate ? 'bg-primary text-background font-semibold' : 'bg-background'}`}
              >
                {rate}×
              </button>
            ))}
          </div>
          {setSpeed.error && <SourceIssue compact>{setSpeed.error.message}</SourceIssue>}
        </Sheet>
      )}
    </>
  );
};
