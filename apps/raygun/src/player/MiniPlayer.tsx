import { commands } from '@raygun/bindings';
import type { QueueItem, QueueSnapshot } from '@raygun/bindings';
import {
  BookOpen,
  ListMusic,
  Maximize2,
  MicVocal,
  MonitorPlay,
  PanelRight,
  PictureInPicture2,
  Podcast,
  Subtitles,
  Volume1,
  Volume2,
  VolumeX,
  X,
} from 'lucide-react';
import { useNavigate } from '@tanstack/react-router';
import { useQueryClient } from '@tanstack/react-query';
import { motion } from 'motion/react';
import type { PanInfo } from 'motion/react';
import { useRef } from 'react';
import type { FC, MouseEvent, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { fromQueueItem } from '../collections/useCollections';
import { ExplicitMark } from '../music/ExplicitMark';
import { PlayingOn } from '../connect/PlayingOn';
import { ConnectButton } from '../devices/ConnectButton';
import { Artwork } from '../library/Artwork';
import { songCover } from '../library/coverTargets';
import { IncognitoBadge } from '../privacy/IncognitoBadge';
import { useTrackMenu } from '../music/useTrackMenu';
import { appBackground } from '../shell/appWindow';
import { openContextMenu } from '../shell/contextMenuStore';
import { Marquee } from '../shell/Marquee';
import { useIsDesktop } from '../shell/useIsDesktop';
import { SoundButton } from '../sound/SoundButton';
import { FavoriteButton } from './FavoriteButton';
import { PlaybackExtras } from './PlaybackExtras';
import { usePlayerWide } from './playerLayout';
import { usePlayerUiStore } from './playerUiStore';
import { PlayingFrom } from './PlayingFrom';
import { BarProgress, SeekBar } from './SeekBar';
import { spokenName, useSpokenItem } from './spoken';
import { swipeAction } from './swipe';
import {
  NextButton,
  PlayPauseButton,
  PreviousButton,
  RepeatButton,
  ShuffleButton,
} from './transport';
import {
  sendControl,
  shownQueue,
  useCurrentItem,
  usePlayerSelect,
  useSourceNote,
} from './usePlayback';
import { SkipNotice } from './SkipNotice';

/** The song before the current one in play order (the last one with repeat all), to swipe back
 * to: unlike Previous, it never just restarts the song. */
const previousItem = (
  queue: QueueSnapshot | undefined,
  currentId: string | null | undefined,
  repeatAll: boolean,
) => {
  if (!queue || !currentId) return null;
  const current = queue.items.findIndex((each) => each.id === currentId);
  const at = queue.order.indexOf(current);
  if (at > 0) return queue.items[queue.order[at - 1] ?? -1] ?? null;
  if (at === 0 && repeatAll && queue.order.length > 1) {
    return queue.items[queue.order.at(-1) ?? -1] ?? null;
  }
  return null;
};

/**
 * The player bar above the bottom tabs (docked at the bottom on wide screens), shown while
 * something is queued. On phones: the track, favourite, play and next; tap or swipe up opens the
 * full player. On wide windows (M46.4) it has Spotify's three parts: the track, the transport
 * with a seek bar, and lyrics, the queue, volume and the full player.
 *
 * It follows only the song and what its buttons show: position reports re-render the seek bar
 * and the progress line, nothing else.
 */
export const MiniPlayer: FC = () => {
  const item = useCurrentItem();
  const wide = usePlayerWide();
  // Like every reference player: no bar while nothing is queued.
  if (!item) {
    return null;
  }
  return wide ? (
    <>
      <WideBar item={item} />
      <PlayingOn className="-mt-1 mx-2 mb-2 rounded-xl lg:mx-4" />
      <SkipNotice />
    </>
  ) : (
    <PhoneBar item={item} />
  );
};

type BarProps = { item: QueueItem };

const PhoneBar: FC<BarProps> = ({ item }) => {
  const { t } = useTranslation();
  const openSheet = usePlayerUiStore((store) => store.openSheet);
  const client = useQueryClient();
  // Chapters and episodes aren't songs to save (M46.32).
  const spoken = useSpokenItem(item);
  const draggedRef = useRef(false);
  const currentId = item.id;
  const repeatAll = usePlayerSelect((state) => state?.repeat === 'all');

  const handleDragEnd = (_event: PointerEvent, info: PanInfo) => {
    const action = swipeAction(info.offset, info.velocity);
    if (action === 'open') openSheet();
    // Down closes the player: playback stops and the queue empties (M46.24).
    if (action === 'close') void sendControl({ type: 'clear' });
    if (action === 'next') void sendControl({ type: 'next' });
    if (action === 'previous') {
      const before = previousItem(shownQueue(client), currentId, repeatAll);
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
      // Sizes follow the bar's own width (a container), so a large interface size or a narrow
      // window never pushes the song's name out.
      className="bg-surface @container relative mr-[max(0.5rem,env(safe-area-inset-right))] mb-2 ml-[max(0.5rem,env(safe-area-inset-left))] overflow-hidden rounded-2xl shadow-lg shadow-black/15"
    >
      <BarProgress />
      <div className="flex items-center gap-1 py-[min(0.5rem,2cqw)] pr-[min(0.5rem,2cqw)] pl-[min(1rem,3.5cqw)]">
        <button
          type="button"
          aria-label={t('player.openPlayer')}
          onClick={() => {
            if (!draggedRef.current) {
              openSheet();
            }
          }}
          className="flex min-w-0 flex-1 items-center gap-[min(0.75rem,3cqw)] text-left"
        >
          <Artwork
            key={item.artworkUrl ?? 'none'}
            url={item.artworkUrl}
            fallback={songCover(item)}
            className="size-[min(2.5rem,12cqw)] shrink-0 rounded-md"
            iconSize={22}
          />
          <TrackText item={item} />
        </button>
        <IncognitoBadge />
        <span className="flex shrink-0 items-center [&_button]:size-[min(2.5rem,11cqw)]">
          <ConnectButton />
          {!spoken && <FavoriteButton item={item} compact />}
          <PlayPauseButton className="rounded-full" iconSize={22} filled={false} />
        </span>
      </div>
      <PlayingOn />
      <SkipNotice />
    </motion.div>
  );
};

const WideBar: FC<BarProps> = ({ item }) => {
  const { t } = useTranslation();
  const openSheet = usePlayerUiStore((store) => store.openSheet);
  const lyricsShown = usePlayerUiStore((store) => store.sheetOpen && store.sheetView === 'lyrics');
  const spoken = useSpokenItem(item);
  const panel = usePlayerUiStore((store) => store.panel);
  const togglePanel = usePlayerUiStore((store) => store.togglePanel);
  const isVideo = item.source.kind === 'youTube';
  const setVideoShown = usePlayerUiStore((store) => store.setVideoShown);
  const desktop = useIsDesktop();

  return (
    <div
      data-testid="mini-player"
      // The right side keeps what fits (a container): the volume slider, the sound and speed
      // buttons and the miniplayer come in as the bar widens.
      className="bg-surface @container relative mx-2 mb-2 rounded-2xl px-4 py-2 shadow-lg shadow-black/15 lg:mx-4"
    >
      <div className="grid grid-cols-[minmax(0,1fr)_minmax(0,1.3fr)_minmax(0,1.2fr)] items-center gap-4">
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
            <TrackText item={item} />
          </button>
          {!spoken && <FavoriteButton item={item} compact download />}
          <PlayingFrom variant="icon" />
        </div>

        <div className="flex min-w-0 flex-col items-center gap-0.5">
          <div className="flex items-center gap-2">
            <ShuffleButton className="size-9" iconSize={18} />
            <PreviousButton className="size-9" iconSize={20} filled />
            <PlayPauseButton
              className="bg-foreground text-background size-9 rounded-full transition-transform hover:scale-105"
              iconSize={18}
            />
            <NextButton className="size-9" iconSize={20} filled />
            <RepeatButton className="size-9" iconSize={18} />
          </div>
          <div className="flex w-full items-center gap-1">
            <SeekBar variant="bar" />
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

        <div className="flex min-w-0 items-center justify-end gap-0.5">
          <IncognitoBadge className="mr-1" />
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
          <span className="hidden @min-[54rem]:contents">
            <BarButton
              label={t('player.panel.nowPlayingView')}
              pressed={panel === 'nowPlaying'}
              onClick={() => togglePanel('nowPlaying')}
            >
              <PanelRight aria-hidden size={18} />
            </BarButton>
          </span>
          <BarButton
            label={t('player.queue')}
            pressed={panel === 'queue'}
            onClick={() => togglePanel('queue')}
          >
            <ListMusic aria-hidden size={18} />
          </BarButton>
          <span className="hidden @min-[70rem]:contents">
            <PlaybackExtras spoken={spoken} />
            <SoundButton />
          </span>
          <span className="hidden @min-[62rem]:contents">
            <ConnectButton className="size-9" />
          </span>
          <VolumeControl />
          {desktop && (
            <span className="hidden @min-[80rem]:contents">
              <BarButton
                label={t('miniplayer.open')}
                onClick={() => void commands.miniplayerOpen(appBackground())}
              >
                <PictureInPicture2 aria-hidden size={18} />
              </BarButton>
            </span>
          )}
          <BarButton label={t('player.openPlayer')} onClick={() => openSheet({ view: 'artwork' })}>
            <Maximize2 aria-hidden size={18} />
          </BarButton>
        </div>
      </div>
    </div>
  );
};

/**
 * The song's title and who it's by, as plain text: the bar is tapped to open the player, and a
 * name that led elsewhere was tapped by accident. Its menu (right click, long press) has Go to
 * the artist, or to the book or show.
 */
const TrackText: FC<BarProps> = ({ item }) => {
  const { t } = useTranslation();
  const menu = useTrackMenu();
  const navigate = useNavigate();
  const note = useSourceNote(item);
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
  const byline = spoken ? spokenName(spoken) : item.artist;
  return (
    <span
      className="title-gap flex min-w-0 flex-col"
      onContextMenu={spoken ? spokenMenu : menu(fromQueueItem(item))}
    >
      <span className="flex min-w-0 items-center gap-1.5">
        <Marquee
          testId="mini-player-title"
          text={item.title}
          className="min-w-0 text-sm leading-tight font-medium"
        />
        {item.explicit && <ExplicitMark />}
      </span>
      {note?.kind === 'finding' ? (
        <span data-testid="source-note" className="text-muted truncate text-xs leading-tight">
          {t('player.findingSource')}
        </span>
      ) : (
        byline && (
          <span
            data-testid={spoken ? 'spoken-byline' : 'bar-artist'}
            className="text-muted block truncate text-xs leading-tight"
          >
            {byline}
          </span>
        )
      )}
    </span>
  );
};

/** Mute and a volume slider (on wide enough bars); unmuting returns to the volume before. */
const VolumeControl: FC = () => {
  const { t } = useTranslation();
  const unmutedVolume = usePlayerUiStore((store) => store.unmutedVolume);
  const setUnmutedVolume = usePlayerUiStore((store) => store.setUnmutedVolume);
  const volume = usePlayerSelect((state) => state?.volume ?? 1);
  const muted = volume <= 0;
  const Icon = muted ? VolumeX : volume < 0.5 ? Volume1 : Volume2;
  return (
    <div className="flex shrink-0 items-center">
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
        aria-valuetext={`${Math.round(volume * 100)} %`}
        min={0}
        max={1}
        step={0.01}
        value={volume}
        onChange={(event) =>
          void sendControl({ type: 'setVolume', volume: Number(event.target.value) })
        }
        className="accent-primary hidden h-1 w-20 cursor-pointer @min-[80rem]:block"
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
    className={`grid size-9 shrink-0 place-items-center rounded-full transition-colors ${
      pressed === undefined
        ? 'text-foreground/80 hover:text-foreground'
        : pressed
          ? 'text-primary relative after:absolute after:bottom-0.5 after:left-1/2 after:size-1 after:-translate-x-1/2 after:rounded-full after:bg-current'
          : 'text-muted hover:text-foreground'
    }`}
  >
    {children}
  </button>
);
