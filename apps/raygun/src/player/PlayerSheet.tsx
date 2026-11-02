import type { RepeatMode } from '@raygun/bindings';
import {
  ChevronDown,
  ImageIcon,
  ListPlus,
  MicVocal,
  Pause,
  Play,
  Repeat,
  Repeat1,
  Shuffle,
  SkipBack,
  SkipForward,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import type { PanInfo } from 'motion/react';
import { useEffect, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useTrackActionsStore } from '../collections/trackActionsStore';
import { fromQueueItem } from '../collections/useCollections';
import { Artwork } from '../library/Artwork';
import { FavoriteButton } from './FavoriteButton';
import { formatTime } from './formatTime';
import { LyricsView } from './LyricsView';
import { usePlayerUiStore } from './playerUiStore';
import { useCloseOnBack } from './useCloseOnBack';
import { useVideoSurface } from './useVideoSurface';
import { isActive, sendControl, sourceNote, useNowPlaying } from './usePlayback';

const DISMISS_DISTANCE = 120;
const DISMISS_VELOCITY = 500;

const NEXT_REPEAT: Record<RepeatMode, RepeatMode> = { off: 'all', all: 'one', one: 'off' };

/** Full-screen player. Drag down (or press Escape / the close button) to dismiss. */
export const PlayerSheet: FC = () => {
  const { t } = useTranslation();
  const sheetOpen = usePlayerUiStore((state) => state.sheetOpen);
  const closeSheet = usePlayerUiStore((state) => state.closeSheet);
  const { state, item, upNext } = useNowPlaying();
  const note = sourceNote(state, item);
  const [scrubMs, setScrubMs] = useState<number | null>(null);
  const showLyrics = usePlayerUiStore((state) => state.lyrics);
  const setShowLyrics = usePlayerUiStore((state) => state.setLyrics);
  const isVideo = item?.source.kind === 'youTube';
  const videoSurfaceRef = useVideoSurface(sheetOpen && isVideo);
  const openTrackActions = useTrackActionsStore((store) => store.open);

  useCloseOnBack(sheetOpen, closeSheet);

  useEffect(() => {
    if (!sheetOpen) {
      return;
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        closeSheet();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [sheetOpen, closeSheet]);

  const handleDragEnd = (_event: PointerEvent, info: PanInfo) => {
    if (info.offset.y > DISMISS_DISTANCE || info.velocity.y > DISMISS_VELOCITY) {
      closeSheet();
    }
  };

  const commitScrub = () => {
    if (scrubMs !== null) {
      void sendControl({ type: 'seek', positionMs: scrubMs });
      setScrubMs(null);
    }
  };

  const durationMs = state?.durationMs ?? 0;
  const positionMs = scrubMs ?? state?.positionMs ?? 0;
  const repeat = state?.repeat ?? 'off';

  return (
    <AnimatePresence>
      {sheetOpen && (
        <motion.div
          key="player-sheet"
          role="dialog"
          aria-modal="true"
          aria-label={t('player.nowPlaying')}
          data-testid="player-sheet"
          initial={{ y: '100%' }}
          animate={{ y: 0 }}
          exit={{ y: '100%' }}
          transition={{ type: 'spring', damping: 32, stiffness: 320 }}
          drag="y"
          dragListener={scrubMs === null}
          dragConstraints={{ top: 0, bottom: 0 }}
          dragElastic={{ top: 0, bottom: 0.7 }}
          onDragEnd={handleDragEnd}
          className="bg-background fixed inset-0 isolate z-50 flex flex-col gap-6 overflow-hidden px-6 pt-[max(0.75rem,env(safe-area-inset-top))] pb-[max(1.5rem,env(safe-area-inset-bottom))]"
        >
          {item?.artworkUrl && !isVideo && (
            <div aria-hidden className="pointer-events-none absolute inset-0 -z-10">
              <img
                src={item.artworkUrl}
                alt=""
                className="size-full scale-125 object-cover opacity-45 blur-3xl"
              />
              <div className="from-background/30 via-background/60 to-background absolute inset-0 bg-gradient-to-b" />
            </div>
          )}
          <div className="flex items-center justify-between">
            <button
              type="button"
              aria-label={t('player.closePlayer')}
              onClick={closeSheet}
              className="text-muted grid size-11 place-items-center rounded-full"
            >
              <ChevronDown aria-hidden size={28} />
            </button>
            <span aria-hidden className="bg-muted/40 h-1.5 w-10 rounded-full" />
            {item && !isVideo ? (
              <button
                type="button"
                aria-label={t(showLyrics ? 'player.showArtwork' : 'player.lyrics')}
                aria-pressed={showLyrics}
                onClick={() => setShowLyrics(!showLyrics)}
                className={`grid size-11 place-items-center rounded-full ${showLyrics ? 'text-primary' : 'text-muted'}`}
              >
                {showLyrics ? (
                  <ImageIcon aria-hidden size={22} />
                ) : (
                  <MicVocal aria-hidden size={22} />
                )}
              </button>
            ) : (
              <span className="size-11" />
            )}
          </div>

          <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-6">
            {isVideo ? (
              <div
                ref={videoSurfaceRef}
                data-testid="video-surface"
                className="aspect-video w-full max-w-md rounded-2xl bg-black md:max-w-2xl lg:max-w-4xl"
              />
            ) : showLyrics && item ? (
              <LyricsView item={item} state={state} />
            ) : (
              <Artwork
                key={item?.artworkUrl ?? 'none'}
                url={item?.artworkUrl}
                className="w-full max-w-xs rounded-2xl shadow-xl"
                iconSize={96}
              />
            )}
            <div className="flex w-full max-w-md items-center gap-1">
              <div className="flex min-w-0 flex-1 flex-col gap-1">
                <p data-testid="sheet-title" className="w-full truncate text-2xl font-bold">
                  {item?.title ?? t('player.nothingPlaying')}
                </p>
                {item?.artist && (
                  <p className="text-muted w-full truncate text-lg">{item.artist}</p>
                )}
                {note && (
                  <p data-testid="sheet-source-note" className="text-muted text-xs">
                    {note.kind === 'finding'
                      ? t('player.findingSource')
                      : t('player.via', { source: note.source })}
                  </p>
                )}
                {state?.status === 'error' && state.error && (
                  <p role="alert" className="text-primary text-sm wrap-anywhere">
                    {state.error}
                  </p>
                )}
              </div>
              {item && <FavoriteButton item={item} />}
              {item ? (
                <button
                  type="button"
                  aria-label={t('player.addToPlaylist')}
                  onClick={() => openTrackActions(fromQueueItem(item))}
                  className="text-muted grid size-11 shrink-0 place-items-center rounded-full"
                >
                  <ListPlus aria-hidden size={22} />
                </button>
              ) : (
                <span className="size-11 shrink-0" />
              )}
            </div>
          </div>

          <div className="mx-auto flex w-full max-w-md flex-col gap-1">
            <input
              type="range"
              aria-label={t('player.seek')}
              min={0}
              max={Math.max(durationMs, 1)}
              step={1000}
              value={Math.min(positionMs, Math.max(durationMs, 1))}
              disabled={!item || durationMs <= 0}
              onChange={(event) => setScrubMs(Number(event.target.value))}
              onPointerUp={commitScrub}
              onKeyUp={commitScrub}
              onPointerDownCapture={(event) => event.stopPropagation()}
              className="accent-primary w-full"
            />
            <div className="text-muted flex justify-between text-xs tabular-nums">
              <span data-testid="position">{formatTime(positionMs)}</span>
              <span data-testid="duration">{formatTime(durationMs)}</span>
            </div>
          </div>

          <div className="mx-auto flex w-full max-w-md items-center justify-between">
            <button
              type="button"
              aria-label={t('player.shuffle')}
              aria-pressed={state?.shuffle ?? false}
              onClick={() => void sendControl({ type: 'setShuffle', enabled: !state?.shuffle })}
              className={state?.shuffle ? 'text-primary' : 'text-muted'}
            >
              <Shuffle aria-hidden size={22} />
            </button>
            <button
              type="button"
              disabled={!item}
              aria-label={t('player.previous')}
              onClick={() => void sendControl({ type: 'previous' })}
              className="disabled:opacity-40"
            >
              <SkipBack aria-hidden size={32} />
            </button>
            <button
              type="button"
              disabled={!item}
              aria-label={isActive(state) ? t('player.pause') : t('player.play')}
              onClick={() => void sendControl({ type: 'toggle' })}
              className="bg-primary text-background grid size-18 place-items-center rounded-3xl shadow-lg shadow-black/20 disabled:opacity-40"
            >
              {isActive(state) ? (
                <Pause aria-hidden size={32} fill="currentColor" />
              ) : (
                <Play aria-hidden size={32} fill="currentColor" />
              )}
            </button>
            <button
              type="button"
              disabled={!item}
              aria-label={t('player.next')}
              onClick={() => void sendControl({ type: 'next' })}
              className="disabled:opacity-40"
            >
              <SkipForward aria-hidden size={32} />
            </button>
            <button
              type="button"
              aria-label={t(`player.repeat.${repeat}`)}
              onClick={() => void sendControl({ type: 'setRepeat', mode: NEXT_REPEAT[repeat] })}
              className={repeat === 'off' ? 'text-muted' : 'text-primary'}
            >
              {repeat === 'one' ? (
                <Repeat1 aria-hidden size={22} />
              ) : (
                <Repeat aria-hidden size={22} />
              )}
            </button>
          </div>

          {upNext.length > 0 && (
            <section className="mx-auto flex max-h-44 w-full max-w-md flex-col gap-1 overflow-y-auto">
              <h2 className="text-muted text-xs font-semibold tracking-wide uppercase">
                {t('player.upNext')}
              </h2>
              <ul data-testid="up-next" className="flex flex-col">
                {upNext.map((next) => (
                  <li key={next.id}>
                    <button
                      type="button"
                      aria-label={next.title}
                      onClick={() => void sendControl({ type: 'playItem', itemId: next.id })}
                      className="hover:bg-surface/60 flex w-full items-center gap-3 rounded-lg py-1.5 text-left"
                    >
                      <Artwork
                        url={next.artworkUrl}
                        className="size-10 shrink-0 rounded-md"
                        iconSize={18}
                      />
                      <span className="flex min-w-0 flex-col">
                        <span className="truncate text-sm font-medium">{next.title}</span>
                        {next.artist && (
                          <span className="text-muted truncate text-xs">{next.artist}</span>
                        )}
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            </section>
          )}
        </motion.div>
      )}
    </AnimatePresence>
  );
};
