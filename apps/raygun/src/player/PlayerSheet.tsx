import { ChevronDown, Disc3, Play, SkipBack, SkipForward } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import type { PanInfo } from 'motion/react';
import { useEffect } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { usePlayerUiStore } from './playerUiStore';

const DISMISS_DISTANCE = 120;
const DISMISS_VELOCITY = 500;

/** Full-screen player. Drag down (or press Escape / the close button) to dismiss. */
export const PlayerSheet: FC = () => {
  const { t } = useTranslation();
  const sheetOpen = usePlayerUiStore((state) => state.sheetOpen);
  const closeSheet = usePlayerUiStore((state) => state.closeSheet);

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
          dragConstraints={{ top: 0, bottom: 0 }}
          dragElastic={{ top: 0, bottom: 0.7 }}
          onDragEnd={handleDragEnd}
          className="bg-background fixed inset-0 z-50 flex flex-col px-6 pt-[max(0.75rem,env(safe-area-inset-top))] pb-[max(1.5rem,env(safe-area-inset-bottom))]"
        >
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
            <span className="size-11" />
          </div>

          <div className="flex flex-1 flex-col items-center justify-center gap-8">
            <div className="bg-surface text-muted grid aspect-square w-full max-w-sm place-items-center rounded-2xl">
              <Disc3 aria-hidden size={96} />
            </div>
            <p className="text-muted text-lg">{t('player.nothingPlaying')}</p>
          </div>

          <div className="flex items-center justify-center gap-8">
            <button
              type="button"
              disabled
              aria-label={t('player.previous')}
              className="text-muted disabled:opacity-50"
            >
              <SkipBack aria-hidden size={32} />
            </button>
            <button
              type="button"
              disabled
              aria-label={t('player.play')}
              className="bg-primary text-background grid size-16 place-items-center rounded-full disabled:opacity-50"
            >
              <Play aria-hidden size={30} />
            </button>
            <button
              type="button"
              disabled
              aria-label={t('player.next')}
              className="text-muted disabled:opacity-50"
            >
              <SkipForward aria-hidden size={32} />
            </button>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
};
