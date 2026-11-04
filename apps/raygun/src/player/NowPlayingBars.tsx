import { useRef } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { usePauseOffscreen } from '../shell/usePauseOffscreen';

/**
 * Spotify's playing mark (M46.16): bars that bounce while the song plays and rest when it's
 * paused, shown beside the song wherever it's listed. They stand still while scrolled away.
 */
export const NowPlayingBars: FC<{ playing: boolean; className?: string }> = ({
  playing,
  className = '',
}) => {
  const { t } = useTranslation();
  const box = useRef<HTMLSpanElement>(null);
  usePauseOffscreen(box, playing);
  return (
    <span
      ref={box}
      role="img"
      aria-label={t(playing ? 'player.nowPlayingMark' : 'player.pausedMark')}
      data-testid="now-playing-bars"
      data-playing={playing ? 'true' : 'false'}
      className={`text-primary inline-flex h-3.5 shrink-0 items-end gap-[2px] ${className}`}
    >
      {[0, 1, 2].map((bar) => (
        <span
          key={bar}
          className={`bg-current w-[3px] rounded-sm ${playing ? 'raygun-eq-bar' : 'h-1'}`}
          style={playing ? { animationDelay: `${bar * -0.3}s` } : undefined}
        />
      ))}
    </span>
  );
};
