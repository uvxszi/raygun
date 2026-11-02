import type { PlaybackState, QueueItem, RepeatMode } from '@raygun/bindings';
import {
  ListMusic,
  Maximize2,
  MicVocal,
  PanelRight,
  MonitorPlay,
  Pause,
  Play,
  Repeat,
  Repeat1,
  Shuffle,
  SkipBack,
  SkipForward,
  Volume1,
  Volume2,
  VolumeX,
} from 'lucide-react';
import { motion } from 'motion/react';
import type { PanInfo } from 'motion/react';
import { useRef, useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { Artwork } from '../library/Artwork';
import { ArtistLinks } from '../music/ArtistLinks';
import { fromQueueItem } from '../collections/useCollections';
import { useTrackMenu } from '../music/useTrackMenu';
import { Marquee } from '../shell/Marquee';
import { useWideScreen } from '../shell/useWideScreen';
import { FavoriteButton } from './FavoriteButton';
import { formatTime } from './formatTime';
import { PlaybackExtras } from './PlaybackExtras';
import { usePlayerUiStore } from './playerUiStore';
import { onSeekKey } from './seekKeys';
import { useLivePosition } from './useLivePosition';
import { isActive, sendControl, sourceNote, useNowPlaying } from './usePlayback';

const SWIPE_UP_DISTANCE = 40;
const SWIPE_UP_VELOCITY = 300;

const NEXT_REPEAT: Record<RepeatMode, RepeatMode> = { off: 'all', all: 'one', one: 'off' };

/**
 * The player bar above the bottom tabs (docked at the bottom on wide screens), shown while
 * something is queued. On phones: the track, favourite, play and next; tap or swipe up opens the
 * full player. On wide windows (M46.4) it has Spotify's three parts: the track, the transport
 * with a seek bar, and lyrics, the queue, volume and the full player.
 */
export const MiniPlayer: FC = () => {
  const { state, item } = useNowPlaying();
  const wide = useWideScreen();
  // Like every reference player: no bar while nothing is queued.
  if (!item) {
    return null;
  }
  return wide ? <WideBar state={state} item={item} /> : <PhoneBar state={state} item={item} />;
};

type BarProps = { state: PlaybackState | undefined; item: QueueItem };

const PhoneBar: FC<BarProps> = ({ state, item }) => {
  const { t } = useTranslation();
  const openSheet = usePlayerUiStore((store) => store.openSheet);
  const note = sourceNote(state, item);
  const draggedRef = useRef(false);
  const progress =
    state?.durationMs && state.durationMs > 0
      ? Math.min(100, ((state.positionMs ?? 0) / state.durationMs) * 100)
      : 0;

  const handleDragEnd = (_event: PointerEvent, info: PanInfo) => {
    if (info.offset.y < -SWIPE_UP_DISTANCE || info.velocity.y < -SWIPE_UP_VELOCITY) {
      openSheet();
    }
  };

  return (
    <motion.div
      data-testid="mini-player"
      drag="y"
      dragConstraints={{ top: 0, bottom: 0 }}
      dragElastic={{ top: 0.25, bottom: 0 }}
      dragSnapToOrigin
      onPointerDown={() => {
        draggedRef.current = false;
      }}
      onDragStart={() => {
        draggedRef.current = true;
      }}
      onDragEnd={handleDragEnd}
      className="bg-surface relative mx-2 mb-2 overflow-hidden rounded-2xl shadow-lg shadow-black/15"
    >
      <div
        aria-hidden
        className="bg-primary absolute top-0 left-0 h-0.5"
        style={{ width: `${progress}%` }}
      />
      <div className="flex items-center gap-1 py-2 pr-2 pl-4">
        <button
          type="button"
          aria-label={t('player.openPlayer')}
          onClick={() => {
            if (!draggedRef.current) {
              openSheet();
            }
          }}
          className="flex min-w-0 flex-1 items-center gap-3 text-left"
        >
          <Artwork
            key={item.artworkUrl ?? 'none'}
            url={item.artworkUrl}
            className="size-10 shrink-0 rounded-md"
            iconSize={22}
          />
          <TrackText state={state} item={item} note={note?.kind === 'finding'} />
        </button>
        <FavoriteButton item={item} compact />
        <PlayButton state={state} />
        <button
          type="button"
          aria-label={t('player.next')}
          onClick={() => void sendControl({ type: 'next' })}
          className="grid size-10 place-items-center rounded-full"
        >
          <SkipForward aria-hidden size={20} />
        </button>
      </div>
    </motion.div>
  );
};

const WideBar: FC<BarProps> = ({ state, item }) => {
  const { t } = useTranslation();
  const openSheet = usePlayerUiStore((store) => store.openSheet);
  const lyricsShown = usePlayerUiStore((store) => store.sheetOpen && store.sheetView === 'lyrics');
  const panel = usePlayerUiStore((store) => store.panel);
  const togglePanel = usePlayerUiStore((store) => store.togglePanel);
  const note = sourceNote(state, item);
  const repeat = state?.repeat ?? 'off';
  const isVideo = item.source.kind === 'youTube';
  const setVideoShown = usePlayerUiStore((store) => store.setVideoShown);

  return (
    <div
      data-testid="mini-player"
      className="bg-surface relative mx-2 mb-2 grid grid-cols-[minmax(0,1fr)_minmax(0,1.5fr)_minmax(0,1fr)] items-center gap-4 rounded-2xl px-4 py-2 shadow-lg shadow-black/15 lg:mx-4"
    >
      <div className="flex min-w-0 items-center gap-3">
        <button
          type="button"
          aria-label={t('player.openPlayer')}
          onClick={() => openSheet()}
          className="flex min-w-0 items-center gap-3 text-left"
        >
          <Artwork
            key={item.artworkUrl ?? 'none'}
            url={item.artworkUrl}
            className="size-12 shrink-0 rounded-md"
            iconSize={24}
          />
          <TrackText state={state} item={item} note={note?.kind === 'finding'} />
        </button>
        <FavoriteButton item={item} compact />
      </div>

      <div className="flex min-w-0 flex-col items-center gap-0.5">
        <div className="flex items-center gap-2">
          <BarButton
            label={t('player.shuffle')}
            pressed={state?.shuffle ?? false}
            onClick={() => void sendControl({ type: 'setShuffle', enabled: !state?.shuffle })}
          >
            <Shuffle aria-hidden size={18} />
          </BarButton>
          <BarButton
            label={t('player.previous')}
            onClick={() => void sendControl({ type: 'previous' })}
          >
            <SkipBack aria-hidden size={20} fill="currentColor" />
          </BarButton>
          <PlayButton state={state} round />
          <BarButton label={t('player.next')} onClick={() => void sendControl({ type: 'next' })}>
            <SkipForward aria-hidden size={20} fill="currentColor" />
          </BarButton>
          <BarButton
            label={t(`player.repeat.${repeat}`)}
            pressed={repeat !== 'off'}
            onClick={() => void sendControl({ type: 'setRepeat', mode: NEXT_REPEAT[repeat] })}
          >
            {repeat === 'one' ? (
              <Repeat1 aria-hidden size={18} />
            ) : (
              <Repeat aria-hidden size={18} />
            )}
          </BarButton>
        </div>
        <SeekBar state={state} />
      </div>

      <div className="flex min-w-0 items-center justify-end gap-1">
        {isVideo ? (
          <BarButton
            label={t('player.showVideo')}
            onClick={() => {
              setVideoShown(true);
              openSheet();
            }}
          >
            <MonitorPlay aria-hidden size={18} />
          </BarButton>
        ) : (
          <BarButton
            label={t('player.lyrics')}
            pressed={lyricsShown}
            onClick={() => openSheet({ view: 'lyrics' })}
          >
            <MicVocal aria-hidden size={18} />
          </BarButton>
        )}
        <BarButton
          label={t('player.panel.nowPlayingView')}
          pressed={panel === 'nowPlaying'}
          onClick={() => togglePanel('nowPlaying')}
        >
          <PanelRight aria-hidden size={18} />
        </BarButton>
        <BarButton
          label={t('player.queue')}
          pressed={panel === 'queue'}
          onClick={() => togglePanel('queue')}
        >
          <ListMusic aria-hidden size={18} />
        </BarButton>
        <PlaybackExtras state={state} />
        <VolumeControl state={state} />
        <BarButton label={t('player.openPlayer')} onClick={() => openSheet({ view: 'artwork' })}>
          <Maximize2 aria-hidden size={18} />
        </BarButton>
      </div>
    </div>
  );
};

const TrackText: FC<BarProps & { note: boolean }> = ({ item, note }) => {
  const { t } = useTranslation();
  const menu = useTrackMenu();
  return (
    <span className="flex min-w-0 flex-col" onContextMenu={menu(fromQueueItem(item))}>
      <Marquee testId="mini-player-title" text={item.title} className="text-sm font-medium" />
      {note ? (
        <span data-testid="source-note" className="text-muted truncate text-xs">
          {t('player.findingSource')}
        </span>
      ) : (
        item.artist && (
          <ArtistLinks credit={item.artist} nested className="text-muted block truncate text-xs" />
        )
      )}
    </span>
  );
};

const PlayButton: FC<{ state: PlaybackState | undefined; round?: boolean }> = ({
  state,
  round,
}) => {
  const { t } = useTranslation();
  const active = isActive(state);
  return (
    <button
      type="button"
      aria-label={active ? t('player.pause') : t('player.play')}
      onClick={() => void sendControl({ type: 'toggle' })}
      className={
        round
          ? 'bg-foreground text-background grid size-9 place-items-center rounded-full transition-transform hover:scale-105'
          : 'grid size-10 place-items-center rounded-full'
      }
    >
      {active ? (
        <Pause aria-hidden size={round ? 18 : 22} fill={round ? 'currentColor' : 'none'} />
      ) : (
        <Play aria-hidden size={round ? 18 : 22} fill={round ? 'currentColor' : 'none'} />
      )}
    </button>
  );
};

/** Elapsed time, a draggable seek bar, and the length. */
const SeekBar: FC<{ state: PlaybackState | undefined }> = ({ state }) => {
  const { t } = useTranslation();
  const [scrubMs, setScrubMs] = useState<number | null>(null);
  const livePositionMs = useLivePosition(state);
  const durationMs = state?.durationMs ?? 0;
  const positionMs = scrubMs ?? livePositionMs;
  const commit = () => {
    if (scrubMs !== null) {
      void sendControl({ type: 'seek', positionMs: scrubMs });
      setScrubMs(null);
    }
  };
  return (
    <div className="text-muted flex w-full items-center gap-2 text-[11px] tabular-nums">
      <span data-testid="bar-position" className="w-10 text-right">
        {formatTime(positionMs)}
      </span>
      <input
        type="range"
        aria-label={t('player.seek')}
        min={0}
        max={Math.max(durationMs, 1)}
        step={250}
        value={Math.min(positionMs, Math.max(durationMs, 1))}
        disabled={durationMs <= 0}
        onChange={(event) => setScrubMs(Number(event.target.value))}
        onKeyDown={onSeekKey(positionMs, durationMs, setScrubMs)}
        onPointerUp={commit}
        onKeyUp={commit}
        onBlur={commit}
        className="accent-primary h-1 min-w-0 flex-1 cursor-pointer disabled:cursor-default"
      />
      <span data-testid="bar-duration" className="w-10">
        {formatTime(durationMs)}
      </span>
    </div>
  );
};

/** Mute and a volume slider; unmuting returns to the volume before. */
const VolumeControl: FC<{ state: PlaybackState | undefined }> = ({ state }) => {
  const { t } = useTranslation();
  const unmutedVolume = usePlayerUiStore((store) => store.unmutedVolume);
  const setUnmutedVolume = usePlayerUiStore((store) => store.setUnmutedVolume);
  const volume = state?.volume ?? 1;
  const muted = volume <= 0;
  const Icon = muted ? VolumeX : volume < 0.5 ? Volume1 : Volume2;
  return (
    <div className="flex items-center">
      <BarButton
        label={muted ? t('player.unmute') : t('player.mute')}
        onClick={() => {
          if (muted) {
            void sendControl({ type: 'setVolume', volume: unmutedVolume > 0 ? unmutedVolume : 1 });
          } else {
            setUnmutedVolume(volume);
            void sendControl({ type: 'setVolume', volume: 0 });
          }
        }}
      >
        <Icon aria-hidden size={18} />
      </BarButton>
      <input
        type="range"
        aria-label={t('player.volume')}
        min={0}
        max={1}
        step={0.01}
        value={volume}
        onChange={(event) =>
          void sendControl({ type: 'setVolume', volume: Number(event.target.value) })
        }
        className="accent-primary h-1 w-24 cursor-pointer"
      />
    </div>
  );
};

type BarButtonProps = {
  label: string;
  onClick: () => void;
  pressed?: boolean;
  children: ReactNode;
};

const BarButton: FC<BarButtonProps> = ({ label, onClick, pressed, children }) => (
  <button
    type="button"
    aria-label={label}
    title={label}
    aria-pressed={pressed}
    onClick={onClick}
    className={`grid size-9 place-items-center rounded-full transition-colors ${
      pressed === undefined
        ? 'text-foreground/80 hover:text-foreground'
        : pressed
          ? 'text-primary'
          : 'text-muted hover:text-foreground'
    }`}
  >
    {children}
  </button>
);
