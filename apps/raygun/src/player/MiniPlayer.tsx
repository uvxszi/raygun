import { Disc3, Play } from 'lucide-react';
import { motion } from 'motion/react';
import type { PanInfo } from 'motion/react';
import { useRef } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { usePlayerUiStore } from './playerUiStore';

const SWIPE_UP_DISTANCE = 40;
const SWIPE_UP_VELOCITY = 300;

/**
 * Compact player bar above the bottom tabs (docked at the bottom on wide screens).
 * Tap or swipe up opens the full player sheet.
 */
export const MiniPlayer: FC = () => {
  const { t } = useTranslation();
  const openSheet = usePlayerUiStore((state) => state.openSheet);
  const draggedRef = useRef(false);

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
      className="bg-surface border-background flex items-center gap-3 border-t px-4 py-2 lg:px-6 lg:py-3"
    >
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
        <span className="bg-background text-muted grid size-10 shrink-0 place-items-center rounded-md">
          <Disc3 aria-hidden size={22} />
        </span>
        <span className="text-muted truncate text-sm">{t('player.nothingPlaying')}</span>
      </button>
      <button
        type="button"
        disabled
        aria-label={t('player.play')}
        className="text-muted grid size-10 place-items-center rounded-full disabled:opacity-50"
      >
        <Play aria-hidden size={22} />
      </button>
    </motion.div>
  );
};
