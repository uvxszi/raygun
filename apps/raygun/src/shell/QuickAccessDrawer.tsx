import { useRouterState } from '@tanstack/react-router';
import { ChevronsLeft, ChevronsRight } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import type { PanInfo } from 'motion/react';
import { useCallback, useEffect, useRef } from 'react';
import type { FC } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';

import { useCloseOnBack } from '../player/useCloseOnBack';
import { usePlayerUiStore } from '../player/playerUiStore';
import { useQuickAccessShown, useQuickAccessStore } from './quickAccessStore';
import { RailPins } from './RailPins';
import { useRail } from './useRail';

/** How far in from the right edge a drag may start to pull the drawer out, in px. */
const EDGE_PX = 24;
/** How far it must travel, mostly sideways, to open it. */
const PULL_PX = 32;

/** Opens the drawer when a finger drags in from the right edge. */
const useEdgePull = (enabled: boolean, open: () => void) => {
  useEffect(() => {
    if (!enabled) return;
    let start: { x: number; y: number } | null = null;
    const onStart = (event: TouchEvent) => {
      const touch = event.touches[0];
      start =
        event.touches.length === 1 && touch && touch.clientX >= window.innerWidth - EDGE_PX
          ? { x: touch.clientX, y: touch.clientY }
          : null;
    };
    const onMove = (event: TouchEvent) => {
      const touch = event.touches[0];
      if (!start || !touch) return;
      const across = start.x - touch.clientX;
      const down = Math.abs(touch.clientY - start.y);
      if (down > PULL_PX) start = null;
      else if (across > PULL_PX && across > down) {
        start = null;
        open();
      }
    };
    const onEnd = () => {
      start = null;
    };
    window.addEventListener('touchstart', onStart, { passive: true });
    window.addEventListener('touchmove', onMove, { passive: true });
    window.addEventListener('touchend', onEnd, { passive: true });
    window.addEventListener('touchcancel', onEnd, { passive: true });
    return () => {
      window.removeEventListener('touchstart', onStart);
      window.removeEventListener('touchmove', onMove);
      window.removeEventListener('touchend', onEnd);
      window.removeEventListener('touchcancel', onEnd);
    };
  }, [enabled, open]);
};

/**
 * Quick access on phones: the sidebar's pins (M46.29) in a drawer from the right, opened by
 * Raygun's icon in the top bar or a drag in from the right edge. Compact shows covers only,
 * expanded their names too (`Settings::rail.drawerExpanded`). Tapping outside, swiping it back,
 * Back and opening a pin close it.
 */
export const QuickAccessDrawer: FC<{ blocked: boolean }> = ({ blocked }) => {
  const { t } = useTranslation();
  const shown = useQuickAccessShown();
  const open = useQuickAccessStore((state) => state.open);
  const setOpen = useQuickAccessStore((state) => state.setOpen);
  const playerOpen = usePlayerUiStore((state) => state.sheetOpen);
  const rail = useRail();
  const expanded = rail.prefs.drawerExpanded ?? true;
  const visible = open && shown && !blocked;
  const close = useCallback(() => setOpen(false), [setOpen]);
  const openDrawer = useCallback(() => setOpen(true), [setOpen]);
  useEdgePull(shown && !blocked && !playerOpen && !open, openDrawer);
  useCloseOnBack(visible, close);

  // Opening a pin is a new page: the drawer goes.
  const href = useRouterState({ select: (state) => state.location.href });
  const shownHref = useRef(href);
  useEffect(() => {
    if (href !== shownHref.current) {
      shownHref.current = href;
      close();
    }
  }, [href, close]);
  useEffect(() => {
    if (!shown || blocked) close();
  }, [shown, blocked, close]);
  useEffect(() => {
    if (!visible) return;
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') close();
    };
    document.addEventListener('keydown', escape);
    return () => document.removeEventListener('keydown', escape);
  }, [visible, close]);

  const onDragEnd = (_event: unknown, info: PanInfo) => {
    if (info.offset.x > 80 || info.velocity.x > 400) close();
  };

  return createPortal(
    <AnimatePresence>
      {visible && (
        <motion.div
          key="quick-access"
          className="fixed inset-0 z-[60]"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.18 }}
        >
          <div
            data-testid="quick-access-backdrop"
            aria-hidden
            onClick={close}
            className="absolute inset-0 bg-black/60"
          />
          <motion.div
            role="dialog"
            aria-modal="true"
            aria-label={t('quickAccess.title')}
            data-testid="quick-access"
            data-expanded={expanded ? '' : undefined}
            drag="x"
            dragDirectionLock
            dragConstraints={{ left: 0, right: 0 }}
            dragElastic={{ left: 0, right: 1 }}
            dragSnapToOrigin
            onDragEnd={onDragEnd}
            initial={{ x: '100%' }}
            animate={{ x: 0 }}
            exit={{ x: '100%' }}
            transition={{ type: 'tween', ease: 'easeOut', duration: 0.22 }}
            style={{ width: expanded ? 'min(18rem, 82vw)' : '5.5rem' }}
            className="bg-surface absolute inset-y-0 right-0 flex flex-col overflow-hidden rounded-l-3xl pt-[max(0.75rem,env(safe-area-inset-top))] pr-[env(safe-area-inset-right)] pb-[env(safe-area-inset-bottom)] shadow-2xl shadow-black/40"
          >
            <div
              className={`flex shrink-0 items-center pb-2 ${expanded ? 'justify-between pr-2 pl-4' : 'justify-center'}`}
            >
              {expanded && (
                <span className="text-primary flex min-w-0 items-center gap-2 font-semibold">
                  <img src="/icon.svg" alt="" className="size-7 shrink-0" />
                  <span className="truncate">{t('quickAccess.title')}</span>
                </span>
              )}
              <button
                type="button"
                aria-label={t(expanded ? 'quickAccess.compact' : 'quickAccess.expand')}
                aria-pressed={expanded}
                onClick={() => rail.setDrawerExpanded(!expanded)}
                className="text-muted hover:text-foreground grid size-10 shrink-0 place-items-center rounded-full"
              >
                {expanded ? (
                  <ChevronsRight aria-hidden size={20} />
                ) : (
                  <ChevronsLeft aria-hidden size={20} />
                )}
              </button>
            </div>
            <nav
              aria-label={t('rail.library')}
              // A pin's page replaces the drawer's history entry; then the drawer goes.
              onClick={(event) => {
                if ((event.target as Element).closest('a')) setTimeout(close, 0);
              }}
              className={`flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto overscroll-contain pb-3 ${expanded ? 'px-2' : 'px-1'}`}
            >
              <RailPins narrow={!expanded} shown drawer />
            </nav>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>,
    document.body,
  );
};
