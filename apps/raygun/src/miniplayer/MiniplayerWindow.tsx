import { commands } from '@raygun/bindings';
import type { PlaybackState, RepeatMode } from '@raygun/bindings';
import {
  AppWindow,
  GripHorizontal,
  Pause,
  Play,
  Repeat,
  Repeat1,
  Shuffle,
  SkipBack,
  SkipForward,
  Volume2,
  VolumeX,
  X,
} from 'lucide-react';
import { useEffect, useRef } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { Artwork } from '../library/Artwork';
import { FavoriteButton } from '../player/FavoriteButton';
import { formatTime } from '../player/formatTime';
import { useLivePosition } from '../player/useLivePosition';
import { isActive, sendControl, useNowPlaying } from '../player/usePlayback';
import { Marquee } from '../shell/Marquee';

const NEXT_REPEAT: Record<RepeatMode, RepeatMode> = { off: 'all', all: 'one', one: 'off' };

const ControlButton: FC<{
  label: string;
  onClick: () => void;
  pressed?: boolean;
  children: ReactNode;
}> = ({ label, onClick, pressed, children }) => (
  <button
    type="button"
    aria-label={label}
    aria-pressed={pressed}
    onClick={onClick}
    className={`grid size-8 shrink-0 place-items-center rounded-full hover:bg-white/15 ${
      pressed ? 'text-primary' : 'text-white'
    }`}
  >
    {children}
  </button>
);

/** Seeks by where the bar was clicked or dragged. */
const SeekBar: FC<{ state: PlaybackState | undefined; position: number }> = ({
  state,
  position,
}) => {
  const { t } = useTranslation();
  const duration = state?.durationMs ?? 0;
  return (
    <div className="flex flex-col gap-0.5">
      <div className="flex justify-between text-[11px] text-white/85 tabular-nums">
        <span>{formatTime(position)}</span>
        <span>{formatTime(duration)}</span>
      </div>
      <input
        type="range"
        aria-label={t('player.seek')}
        min={0}
        max={Math.max(duration, 1)}
        step={1000}
        value={Math.min(position, duration)}
        disabled={duration <= 0}
        onChange={(event) =>
          void sendControl({ type: 'seek', positionMs: Number(event.target.value) })
        }
        className="accent-primary h-1 w-full cursor-pointer"
      />
    </div>
  );
};

/**
 * Spotify's miniplayer (M46.13): a small window that stays on top with the artwork, the song and
 * whether it's saved. Hovering shows the controls over the artwork (volume, shuffle, previous,
 * play, next, repeat, back to Raygun) with the time; the top edge drags the window and closes it.
 */
export const MiniplayerWindow: FC = () => {
  const { t } = useTranslation();
  const { state, item } = useNowPlaying();
  const position = useLivePosition(state);
  const playing = isActive(state);
  const volume = state?.volume ?? 1;
  const lastVolume = useRef(volume > 0 ? volume : 1);
  // Unmuting goes back to the volume before muting.
  useEffect(() => {
    if (volume > 0) lastVolume.current = volume;
  }, [volume]);
  const repeat = state?.repeat ?? 'off';

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === ' ') {
        event.preventDefault();
        void sendControl({ type: 'toggle' });
      } else if (event.key === 'ArrowRight' && event.ctrlKey) {
        void sendControl({ type: 'next' });
      } else if (event.key === 'ArrowLeft' && event.ctrlKey) {
        void sendControl({ type: 'previous' });
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  return (
    <div
      data-testid="miniplayer"
      className="group bg-background text-foreground relative flex h-screen flex-col overflow-hidden select-none"
    >
      <div
        data-tauri-drag-region
        className="absolute inset-x-0 top-0 z-20 flex h-7 items-center justify-center opacity-0 transition-opacity group-hover:opacity-100"
      >
        <GripHorizontal aria-hidden size={16} className="pointer-events-none text-white/70" />
        <button
          type="button"
          aria-label={t('miniplayer.close')}
          onClick={() => void commands.miniplayerClose(false)}
          className="absolute top-0.5 right-1 grid size-6 place-items-center rounded-full text-white/80 hover:bg-white/15"
        >
          <X aria-hidden size={14} />
        </button>
      </div>

      <div className="relative min-h-0 flex-1 p-3 pt-7">
        <div className="relative h-full w-full overflow-hidden rounded-lg">
          <Artwork
            key={item?.artworkUrl ?? 'none'}
            url={item?.artworkUrl ?? null}
            className="h-full w-full object-cover"
            iconSize={56}
          />
          {item && (
            <div className="absolute inset-0 flex flex-col justify-between bg-black/55 p-2 opacity-0 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100">
              <span />
              <div className="flex items-center justify-center gap-0.5">
                <ControlButton
                  label={volume > 0 ? t('player.mute') : t('player.unmute')}
                  onClick={() =>
                    void sendControl({
                      type: 'setVolume',
                      volume: volume > 0 ? 0 : lastVolume.current,
                    })
                  }
                >
                  {volume > 0 ? (
                    <Volume2 aria-hidden size={16} />
                  ) : (
                    <VolumeX aria-hidden size={16} />
                  )}
                </ControlButton>
                <ControlButton
                  label={t('player.shuffle')}
                  pressed={state?.shuffle}
                  onClick={() => void sendControl({ type: 'setShuffle', enabled: !state?.shuffle })}
                >
                  <Shuffle aria-hidden size={16} />
                </ControlButton>
                <ControlButton
                  label={t('player.previous')}
                  onClick={() => void sendControl({ type: 'previous' })}
                >
                  <SkipBack aria-hidden size={18} fill="currentColor" />
                </ControlButton>
                <button
                  type="button"
                  aria-label={playing ? t('player.pause') : t('player.play')}
                  onClick={() => void sendControl({ type: 'toggle' })}
                  className="mx-1 grid size-11 shrink-0 place-items-center rounded-full bg-white text-black hover:scale-105"
                >
                  {playing ? (
                    <Pause aria-hidden size={20} fill="currentColor" />
                  ) : (
                    <Play aria-hidden size={20} fill="currentColor" className="translate-x-px" />
                  )}
                </button>
                <ControlButton
                  label={t('player.next')}
                  onClick={() => void sendControl({ type: 'next' })}
                >
                  <SkipForward aria-hidden size={18} fill="currentColor" />
                </ControlButton>
                <ControlButton
                  label={t(`player.repeat.${repeat}`)}
                  pressed={repeat !== 'off'}
                  onClick={() => void sendControl({ type: 'setRepeat', mode: NEXT_REPEAT[repeat] })}
                >
                  {repeat === 'one' ? (
                    <Repeat1 aria-hidden size={16} />
                  ) : (
                    <Repeat aria-hidden size={16} />
                  )}
                </ControlButton>
                <ControlButton
                  label={t('miniplayer.back')}
                  onClick={() => void commands.miniplayerClose(true)}
                >
                  <AppWindow aria-hidden size={16} />
                </ControlButton>
              </div>
              <SeekBar state={state} position={position} />
            </div>
          )}
        </div>
      </div>

      <div className="flex items-center gap-2 px-3 pb-3">
        {item ? (
          <>
            <div className="min-w-0 flex-1">
              <Marquee text={item.title} className="text-lg leading-tight font-extrabold" />
              <p className="text-muted truncate text-sm">{item.artist}</p>
            </div>
            <FavoriteButton item={item} compact />
          </>
        ) : (
          <p className="text-muted flex-1 text-sm">{t('miniplayer.nothing')}</p>
        )}
      </div>
    </div>
  );
};
