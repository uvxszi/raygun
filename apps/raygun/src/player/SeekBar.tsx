import type { PlaybackState } from '@raygun/bindings';
import { useState } from 'react';
import type { FC, SyntheticEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { formatTime } from './formatTime';
import { onSeekKey } from './seekKeys';
import { useLiveValue, useLivePosition } from './useLivePosition';
import { sendControl, usePlayerTiming } from './usePlayback';

/** A seek let go: shown until the state that follows it arrives, so the thumb doesn't jump back. */
type Seeking = { positionMs: number; from: PlaybackState | undefined };

type SeekBarProps = {
  /** `sheet`: the times under the slider; `bar`: beside it, in the desktop bar. */
  variant: 'sheet' | 'bar';
  /** Whether the slider is being dragged (the full player doesn't drag meanwhile). */
  onScrub?: (scrubbing: boolean) => void;
  className?: string;
};

/**
 * Elapsed time, a slider and the length. It follows the playhead on its own (the only part of
 * the player that re-renders as a song plays) and seeks once let go, as a drag ends or a key is
 * released.
 */
export const SeekBar: FC<SeekBarProps> = ({ variant, onScrub, className = '' }) => {
  const { t } = useTranslation();
  const state = usePlayerTiming();
  const [scrubMs, setScrubMs] = useState<number | null>(null);
  const [seeking, setSeeking] = useState<Seeking | null>(null);
  const livePositionMs = useLivePosition(state);
  const durationMs = state?.durationMs ?? 0;
  const pending = seeking && seeking.from === state ? seeking.positionMs : null;
  const positionMs = scrubMs ?? pending ?? livePositionMs;
  const scrub = (value: number | null) => {
    setScrubMs(value);
    onScrub?.(value !== null);
  };
  const commit = () => {
    if (scrubMs === null) return;
    void sendControl({ type: 'seek', positionMs: scrubMs });
    setSeeking({ positionMs: scrubMs, from: state });
    scrub(null);
  };
  const slider = (
    <input
      type="range"
      aria-label={t('player.seek')}
      aria-valuetext={`${formatTime(positionMs)} / ${formatTime(durationMs)}`}
      min={0}
      max={Math.max(durationMs, 1)}
      step={250}
      value={Math.min(positionMs, Math.max(durationMs, 1))}
      disabled={!state?.currentItemId || durationMs <= 0}
      onChange={(event) => scrub(Number(event.target.value))}
      onKeyDown={onSeekKey(positionMs, durationMs, scrub)}
      onPointerUp={commit}
      onKeyUp={commit}
      onBlur={commit}
      // The full player and the bar drag; the slider takes its own drags.
      onPointerDownCapture={(event: SyntheticEvent) => event.stopPropagation()}
      className={`accent-primary min-w-0 cursor-pointer disabled:cursor-default ${
        variant === 'bar' ? 'h-1 flex-1' : 'w-full'
      }`}
    />
  );

  if (variant === 'bar') {
    return (
      <div
        className={`text-muted flex w-full min-w-0 flex-1 items-center gap-2 text-[11px] tabular-nums ${className}`}
      >
        <span data-testid="bar-position" className="min-w-10 text-right">
          {formatTime(positionMs)}
        </span>
        {slider}
        <span data-testid="bar-duration" className="min-w-10">
          {formatTime(durationMs)}
        </span>
      </div>
    );
  }
  return (
    <div className={`flex w-full flex-col gap-1 ${className}`}>
      {slider}
      <div className="text-muted flex justify-between text-xs tabular-nums">
        <span data-testid="position">{formatTime(positionMs)}</span>
        <span data-testid="duration">{formatTime(durationMs)}</span>
      </div>
    </div>
  );
};

/**
 * The phone bar's thin progress line, moved with a transform (no layout) and re-rendered only
 * when it moves a visible step.
 */
export const BarProgress: FC = () => {
  const state = usePlayerTiming();
  const durationMs = state?.durationMs ?? 0;
  const share = useLiveValue(state, (position) =>
    durationMs > 0 ? Math.min(1, Math.round((position / durationMs) * 500) / 500) : 0,
  );
  return (
    <div
      aria-hidden
      data-testid="bar-progress"
      className="bg-primary pointer-events-none absolute inset-x-0 top-0 h-0.5 origin-left"
      style={{ transform: `scaleX(${share})` }}
    />
  );
};
