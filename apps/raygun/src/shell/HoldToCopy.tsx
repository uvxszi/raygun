import { useEffect, useRef, useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { showToast } from './toastStore';

/** How long a press is held to copy, ms. */
export const HOLD_TO_COPY_MS = 500;
/** A press that moves this far (px) is a scroll or a drag, never a hold. */
const MOVE_PX = 10;
/** How long the glow stays after copying, ms. */
const COPIED_MS = 600;

/** The glow around text being pressed: an outline and a soft light in the accent colour. */
const GLOW =
  '0 0 0 2px var(--color-primary), 0 0 14px 3px color-mix(in srgb, var(--color-primary) 55%, transparent)';

type HoldToCopyProps = {
  /** What's copied; shown too unless `children` shows something else. */
  text: string;
  children?: ReactNode;
  className?: string;
};

/**
 * Text that copies itself when held, like a menu's title and artists: pressing outlines it with a
 * glow, and holding it for half a second copies it and says so in a toast. With the keyboard,
 * Enter (or Space) on it copies at once. A short tap does nothing.
 */
export const HoldToCopy: FC<HoldToCopyProps> = ({ text, children, className = '' }) => {
  const { t } = useTranslation();
  const [pressed, setPressed] = useState(false);
  const [copied, setCopied] = useState(false);
  const hold = useRef<{ x: number; y: number; timer: ReturnType<typeof setTimeout> } | null>(null);
  const glow = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  useEffect(
    () => () => {
      if (hold.current) clearTimeout(hold.current.timer);
      clearTimeout(glow.current);
    },
    [],
  );

  const release = () => {
    if (hold.current) clearTimeout(hold.current.timer);
    hold.current = null;
    setPressed(false);
  };
  const copy = () => {
    void navigator.clipboard?.writeText(text).catch(() => undefined);
    navigator.vibrate?.(15);
    showToast(t('menu.copied'));
    setCopied(true);
    clearTimeout(glow.current);
    glow.current = setTimeout(() => setCopied(false), COPIED_MS);
  };

  return (
    <button
      type="button"
      title={t('menu.holdToCopy')}
      data-pressed={pressed || copied ? '' : undefined}
      onPointerDown={(event) => {
        if (event.button !== 0) return;
        release();
        const timer = setTimeout(() => {
          hold.current = null;
          setPressed(false);
          copy();
        }, HOLD_TO_COPY_MS);
        hold.current = { x: event.clientX, y: event.clientY, timer };
        setPressed(true);
      }}
      onPointerMove={(event) => {
        const start = hold.current;
        if (start && Math.hypot(event.clientX - start.x, event.clientY - start.y) > MOVE_PX) {
          release();
        }
      }}
      onPointerUp={release}
      onPointerCancel={release}
      onPointerLeave={release}
      // A touch screen's long press is this hold, never the webview's own menu; the system's
      // long press counts as held long enough, whatever comes after it.
      onContextMenu={(event) => {
        event.preventDefault();
        if (!hold.current) return;
        release();
        copy();
      }}
      onKeyDown={(event) => {
        if (event.key !== 'Enter' && event.key !== ' ') return;
        // The menu around it doesn't choose a row with this Enter.
        event.preventDefault();
        event.stopPropagation();
        copy();
      }}
      style={pressed || copied ? { boxShadow: GLOW } : undefined}
      className={`focus-visible:ring-primary -mx-1 max-w-[calc(100%+0.5rem)] truncate rounded-md px-1 text-left transition-shadow duration-150 outline-none focus-visible:ring-2 ${className}`}
    >
      {children ?? text}
    </button>
  );
};
