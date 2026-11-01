import { Pause, Play } from 'lucide-react';
import { motion } from 'motion/react';
import type { PanInfo } from 'motion/react';
import { useRef } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Artwork } from '../library/Artwork';
import { usePlayerUiStore } from './playerUiStore';
import { isActive, sendControl, sourceNote, useNowPlaying } from './usePlayback';

const SWIPE_UP_DISTANCE = 40;
const SWIPE_UP_VELOCITY = 300;

/**
 * Compact player bar above the bottom tabs (docked at the bottom on wide screens), shown while
 * something is queued. Tap or swipe up opens the full player sheet.
 */
export const MiniPlayer: FC = () => {
  const { t } = useTranslation();
  const openSheet = usePlayerUiStore((state) => state.openSheet);
  const { state, item } = useNowPlaying();
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

  // Like every reference player: no bar while nothing is queued.
  if (!item) {
    return null;
  }

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
      className="bg-surface relative mx-2 mb-2 overflow-hidden rounded-2xl shadow-lg shadow-black/15 lg:mx-4"
    >
      <div
        aria-hidden
        className="bg-primary absolute top-0 left-0 h-0.5"
        style={{ width: `${progress}%` }}
      />
      <div className="flex items-center gap-3 px-4 py-2 lg:px-6 lg:py-3">
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
            key={item?.artworkUrl ?? 'none'}
            url={item?.artworkUrl}
            className="size-10 shrink-0 rounded-md"
            iconSize={22}
          />
          {item ? (
            <span className="flex min-w-0 flex-col">
              <span data-testid="mini-player-title" className="truncate text-sm font-medium">
                {item.title}
              </span>
              {note?.kind === 'finding' ? (
                <span data-testid="source-note" className="text-muted truncate text-xs">
                  {t('player.findingSource')}
                </span>
              ) : (
                item.artist && <span className="text-muted truncate text-xs">{item.artist}</span>
              )}
            </span>
          ) : (
            <span className="text-muted truncate text-sm">{t('player.nothingPlaying')}</span>
          )}
        </button>
        <button
          type="button"
          disabled={!item}
          aria-label={isActive(state) ? t('player.pause') : t('player.play')}
          onClick={() => void sendControl({ type: 'toggle' })}
          className="grid size-10 place-items-center rounded-full disabled:opacity-40"
        >
          {isActive(state) ? <Pause aria-hidden size={22} /> : <Play aria-hidden size={22} />}
        </button>
      </div>
    </motion.div>
  );
};
