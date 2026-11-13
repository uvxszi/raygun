import { useCallback, useEffect, useEffectEvent, useRef, useState } from 'react';
import { create } from 'zustand';

import type { PreviewTrigger } from './videoCards';

/** A mouse rests this long on a card, or a card stays this long in the middle, before it plays. */
export const PREVIEW_DWELL_MS = 600;

type PreviewState = {
  /** The one card previewing (its key). */
  active: string | null;
  start: (key: string) => void;
  stop: (key: string) => void;
};

/** One preview at a time, whichever card asked last. */
export const usePreviewStore = create<PreviewState>((set) => ({
  active: null,
  start: (key) => set({ active: key }),
  stop: (key) => set((state) => (state.active === key ? { active: null } : state)),
}));

/** The middle band of the screen a card is "centred" in (phones). */
const CENTER_MARGIN = '-40% 0px -40% 0px';

/**
 * Whether the card `key` previews now, and what starts it: a mouse resting on it (`hover`: the
 * card's pointer handlers), or the card staying in the middle of the screen (`center`: its
 * element, given by `setElement`). `trigger` is the list's (`usePreviewTrigger`, asked once per
 * list), `null` for none.
 */
export const useCardPreview = (key: string, trigger: PreviewTrigger | null) => {
  const on = trigger !== null;
  const active = usePreviewStore((state) => state.active === key);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [element, setElement] = useState<HTMLElement | null>(null);

  const onEnter = useCallback(() => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => usePreviewStore.getState().start(key), PREVIEW_DWELL_MS);
  }, [key]);
  const onLeave = useCallback(() => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
    usePreviewStore.getState().stop(key);
  }, [key]);
  const onCenter = useEffectEvent((centred: boolean) => (centred ? onEnter() : onLeave()));

  useEffect(() => {
    if (trigger !== 'center' || !element || typeof IntersectionObserver === 'undefined') return;
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) onCenter(entry.isIntersecting);
      },
      { rootMargin: CENTER_MARGIN },
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, [trigger, element]);

  // Stopped (its timer dropped) when previews turn off or the card goes.
  useEffect(() => {
    if (!on) onLeave();
  }, [on, onLeave]);
  useEffect(() => onLeave, [onLeave]);

  const hover = trigger === 'hover';
  return {
    active: on && active,
    setElement,
    onEnter: hover ? onEnter : undefined,
    onLeave: hover ? onLeave : undefined,
  };
};
