import { useQueryClient } from '@tanstack/react-query';
import { Pause, Play, Repeat, Repeat1, Shuffle, SkipBack, SkipForward } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import {
  isActive,
  isWaiting,
  NEXT_REPEAT,
  pressPlayPause,
  sendControl,
  useCurrentItemId,
  usePlayerSelect,
} from './usePlayback';

/** A dot under a toggle that's on (Spotify's), so its state doesn't rest on colour alone. */
const ON_DOT =
  'relative after:absolute after:bottom-0.5 after:left-1/2 after:size-1 after:-translate-x-1/2 after:rounded-full after:bg-current';

type ButtonProps = {
  /** Sizes and spacing: each place sets its own. */
  className?: string;
  iconSize?: number;
};

const toggleClass = (on: boolean) =>
  on ? `text-primary ${ON_DOT}` : 'text-muted hover:text-foreground';

/** Shuffle, lit with a dot while on. */
export const ShuffleButton: FC<ButtonProps> = ({ className = '', iconSize = 20 }) => {
  const { t } = useTranslation();
  const shuffle = usePlayerSelect((state) => state?.shuffle ?? false);
  return (
    <button
      type="button"
      aria-label={t('player.shuffle')}
      title={t('player.shuffle')}
      aria-pressed={shuffle}
      onClick={() => void sendControl({ type: 'setShuffle', enabled: !shuffle })}
      className={`grid shrink-0 place-items-center rounded-full transition-colors ${toggleClass(shuffle)} ${className}`}
    >
      <Shuffle aria-hidden size={iconSize} />
    </button>
  );
};

/** Repeat: off, all, then one (a 1 on the icon), lit with a dot while on. */
export const RepeatButton: FC<ButtonProps> = ({ className = '', iconSize = 20 }) => {
  const { t } = useTranslation();
  const repeat = usePlayerSelect((state) => state?.repeat ?? 'off');
  const label = t(`player.repeat.${repeat}`);
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      aria-pressed={repeat !== 'off'}
      onClick={() => void sendControl({ type: 'setRepeat', mode: NEXT_REPEAT[repeat] })}
      className={`grid shrink-0 place-items-center rounded-full transition-colors ${toggleClass(repeat !== 'off')} ${className}`}
    >
      {repeat === 'one' ? (
        <Repeat1 aria-hidden size={iconSize} />
      ) : (
        <Repeat aria-hidden size={iconSize} />
      )}
    </button>
  );
};

export const PreviousButton: FC<ButtonProps & { filled?: boolean }> = ({
  className = '',
  iconSize = 24,
  filled = false,
}) => {
  const { t } = useTranslation();
  const playing = useCurrentItemId() !== null;
  return (
    <button
      type="button"
      disabled={!playing}
      aria-label={t('player.previous')}
      title={t('player.previous')}
      onClick={() => void sendControl({ type: 'previous' })}
      className={`grid shrink-0 place-items-center rounded-full disabled:opacity-40 ${className}`}
    >
      <SkipBack aria-hidden size={iconSize} fill={filled ? 'currentColor' : 'none'} />
    </button>
  );
};

export const NextButton: FC<ButtonProps & { filled?: boolean }> = ({
  className = '',
  iconSize = 24,
  filled = false,
}) => {
  const { t } = useTranslation();
  const playing = useCurrentItemId() !== null;
  return (
    <button
      type="button"
      disabled={!playing}
      aria-label={t('player.next')}
      title={t('player.next')}
      onClick={() => void sendControl({ type: 'next' })}
      className={`grid shrink-0 place-items-center rounded-full disabled:opacity-40 ${className}`}
    >
      <SkipForward aria-hidden size={iconSize} fill={filled ? 'currentColor' : 'none'} />
    </button>
  );
};

/**
 * Play or Pause, by whether something plays (or is about to). It flips on the tap, before Rust
 * answers ({@link pressPlayPause}). While the player really waits for its song (a stream being
 * found or buffering, see {@link isWaiting}) a ring turns around it, so a Pause that isn't playing
 * yet shows it; a downloaded song or a local file starts at once and shows none.
 */
export const PlayPauseButton: FC<ButtonProps & { filled?: boolean }> = ({
  className = '',
  iconSize = 24,
  filled = true,
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const active = usePlayerSelect(isActive);
  const waiting = usePlayerSelect(isWaiting);
  const playing = useCurrentItemId() !== null;
  const Icon = active ? Pause : Play;
  return (
    <button
      type="button"
      disabled={!playing}
      aria-label={active ? t('player.pause') : t('player.play')}
      aria-busy={waiting || undefined}
      onClick={() => void pressPlayPause(client, { type: 'toggle' })}
      className={`relative grid shrink-0 place-items-center disabled:opacity-40 ${className}`}
    >
      {waiting && (
        <span
          aria-hidden
          data-testid="play-waiting"
          className="absolute size-[78%] animate-spin rounded-full border-2 border-current/25 border-t-current"
        />
      )}
      <Icon aria-hidden size={iconSize} fill={filled ? 'currentColor' : 'none'} />
    </button>
  );
};
