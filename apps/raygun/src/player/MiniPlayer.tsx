import { commands } from '@raygun/bindings';
import type { PlaybackState, QueueItem, QueueSnapshot, RepeatMode } from '@raygun/bindings';
import {
  BookOpen,
  ListMusic,
  Maximize2,
  MicVocal,
  PanelRight,
  MonitorPlay,
  PictureInPicture2,
  Pause,
  Play,
  Podcast,
  Repeat,
  Repeat1,
  Shuffle,
  SkipBack,
  SkipForward,
  Subtitles,
  Volume1,
  Volume2,
  VolumeX,
  X,
} from 'lucide-react';
import { useNavigate } from '@tanstack/react-router';
import { motion } from 'motion/react';
import type { PanInfo } from 'motion/react';
import { useRef, useState } from 'react';
import type { FC, MouseEvent, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { Artwork } from '../library/Artwork';
import { ArtistLinks } from '../music/ArtistLinks';
import { PlayingOn } from '../connect/PlayingOn';
import { ConnectButton } from '../devices/ConnectButton';
import { appBackground } from '../shell/appWindow';
import { useIsDesktop } from '../shell/useIsDesktop';
import { fromQueueItem } from '../collections/useCollections';
import { useTrackMenu } from '../music/useTrackMenu';
import { openContextMenu } from '../shell/contextMenuStore';
import { Marquee } from '../shell/Marquee';
import { useWideScreen } from '../shell/useWideScreen';
import { FavoriteButton } from './FavoriteButton';
import { formatTime } from './formatTime';
import { SoundButton } from '../sound/SoundButton';
import { PlaybackExtras } from './PlaybackExtras';
import { usePlayerUiStore } from './playerUiStore';
import { onSeekKey } from './seekKeys';
import { swipeAction } from './swipe';
import { useSpokenItem } from './spoken';
import { SkipNotice } from './SkipNotice';
import { SpokenByline } from './SpokenByline';
import { useLivePosition } from './useLivePosition';
import { isActive, sendControl, sourceNote, useNowPlaying, useQueue } from './usePlayback';
import { songCover } from '../library/coverTargets';

/** The song before the current one in play order (the last one with repeat all), to swipe back
 * to: unlike Previous, it never just restarts the song. */
const previousItem = (queue: QueueSnapshot | undefined, state: PlaybackState | undefined) => {
  if (!queue || !state?.currentItemId) return null;
  const current = queue.items.findIndex((each) => each.id === state.currentItemId);
  const at = queue.order.indexOf(current);
  if (at > 0) return queue.items[queue.order[at - 1] ?? -1] ?? null;
  if (at === 0 && state.repeat === 'all' && queue.order.length > 1) {
    return queue.items[queue.order.at(-1) ?? -1] ?? null;
  }
  return null;
};

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
  return wide ? (
    <>
      <WideBar state={state} item={item} />
      <PlayingOn className="-mt-1 mx-2 mb-2 rounded-xl lg:mx-4" />
      <SkipNotice />
    </>
  ) : (
    <PhoneBar state={state} item={item} />
  );
};

type BarProps = { state: PlaybackState | undefined; item: QueueItem };

const PhoneBar: FC<BarProps> = ({ state, item }) => {
  const { t } = useTranslation();
  const openSheet = usePlayerUiStore((store) => store.openSheet);
  const note = sourceNote(state, item);
  // Chapters and episodes aren't songs to save (M46.32).
  const spoken = useSpokenItem(item);
  const draggedRef = useRef(false);
  const progress =
    state?.durationMs && state.durationMs > 0
      ? Math.min(100, ((state.positionMs ?? 0) / state.durationMs) * 100)
      : 0;

  const queue = useQueue().data;

  const handleDragEnd = (_event: PointerEvent, info: PanInfo) => {
    const action = swipeAction(info.offset, info.velocity);
    if (action === 'open') openSheet();
    // Down closes the player: playback stops and the queue empties (M46.24).
    if (action === 'close') void sendControl({ type: 'clear' });
    if (action === 'next') void sendControl({ type: 'next' });
    if (action === 'previous') {
      const before = previousItem(queue, state);
      void sendControl(
        before ? { type: 'playItem', itemId: before.id } : { type: 'seek', positionMs: 0 },
      );
    }
  };

  return (
    <motion.div
      data-testid="mini-player"
      // Up opens the full player, down closes it (M46.24); sideways changes the song (M46.13),
      // the bar following the finger.
      drag
      dragDirectionLock
      dragConstraints={{ top: 0, bottom: 0, left: 0, right: 0 }}
      dragElastic={{ top: 0.25, bottom: 0.5, left: 0.4, right: 0.4 }}
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
            fallback={songCover(item)}
            className="size-10 shrink-0 rounded-md"
            iconSize={22}
          />
          <TrackText state={state} item={item} note={note?.kind === 'finding'} />
        </button>
        <ConnectButton />
        {!spoken && <FavoriteButton item={item} compact />}
        <PlayButton state={state} />
      </div>
      <PlayingOn />
      <SkipNotice />
    </motion.div>
  );
};

const WideBar: FC<BarProps> = ({ state, item }) => {
  const { t } = useTranslation();
  const openSheet = usePlayerUiStore((store) => store.openSheet);
  const lyricsShown = usePlayerUiStore((store) => store.sheetOpen && store.sheetView === 'lyrics');
  const spoken = useSpokenItem(item);
  const panel = usePlayerUiStore((store) => store.panel);
  const togglePanel = usePlayerUiStore((store) => store.togglePanel);
  const note = sourceNote(state, item);
  const repeat = state?.repeat ?? 'off';
  const isVideo = item.source.kind === 'youTube';
  const setVideoShown = usePlayerUiStore((store) => store.setVideoShown);
  const desktop = useIsDesktop();

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
            fallback={songCover(item)}
            className="size-12 shrink-0 rounded-md"
            iconSize={24}
          />
          <TrackText state={state} item={item} note={note?.kind === 'finding'} />
        </button>
        {!spoken && <FavoriteButton item={item} compact download />}
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
        <div className="flex w-full items-center gap-1">
          <SeekBar state={state} />
          {/* Closes the player: playback stops and the queue empties (M46.24). */}
          <button
            type="button"
            aria-label={t('player.close')}
            title={t('player.close')}
            onClick={() => void sendControl({ type: 'clear' })}
            className="text-muted hover:text-foreground grid size-6 shrink-0 place-items-center rounded-full"
          >
            <X aria-hidden size={14} />
          </button>
        </div>
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
            label={spoken ? t('captions.title') : t('player.lyrics')}
            pressed={lyricsShown}
            onClick={() => openSheet({ view: 'lyrics' })}
          >
            {spoken ? <Subtitles aria-hidden size={18} /> : <MicVocal aria-hidden size={18} />}
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
        <SoundButton />
        <ConnectButton className="size-9" />
        <VolumeControl state={state} />
        {desktop && (
          <BarButton
            label={t('miniplayer.open')}
            onClick={() => void commands.miniplayerOpen(appBackground())}
          >
            <PictureInPicture2 aria-hidden size={18} />
          </BarButton>
        )}
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
  const navigate = useNavigate();
  const spoken = useSpokenItem(item);
  // A chapter or an episode isn't a song: its menu leads to the book or the show (M46.32).
  const spokenMenu = (event: MouseEvent) => {
    if (!spoken) return;
    const book = spoken.kind === 'audiobook';
    openContextMenu(event, [
      {
        label: t(book ? 'captions.goToBook' : 'captions.goToShow'),
        icon: book ? BookOpen : Podcast,
        onSelect: () =>
          void navigate({ to: book ? '/audiobook' : '/podcast', search: { id: spoken.podcastId } }),
      },
    ]);
  };
  return (
    <span
      className="flex min-w-0 flex-col"
      onContextMenu={spoken ? spokenMenu : menu(fromQueueItem(item))}
    >
      <Marquee testId="mini-player-title" text={item.title} className="text-sm font-medium" />
      {note ? (
        <span data-testid="source-note" className="text-muted truncate text-xs">
          {t('player.findingSource')}
        </span>
      ) : (
        (spoken && (
          <SpokenByline spoken={spoken} nested className="text-muted block truncate text-xs" />
        )) ||
        (item.artist && (
          <ArtistLinks credit={item.artist} nested className="text-muted block truncate text-xs" />
        ))
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
    <div className="text-muted flex w-full min-w-0 flex-1 items-center gap-2 text-[11px] tabular-nums">
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
