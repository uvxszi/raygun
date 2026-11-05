import { useRouterState } from '@tanstack/react-router';
import { useEffect } from 'react';
import type { FC } from 'react';

import { REVEAL_ATTRIBUTE, matchesReveal, revealWait, useRevealStore } from '../music/reveal';

/** How long the found row stays marked. */
const FLASH_MS = 2_000;

const smooth = () => {
  const motion = document.documentElement.dataset.motion;
  return motion !== 'reduced' && motion !== 'off';
};

/**
 * Scrolls to the song the player asked for (`music/reveal.ts`) once its page is on screen and the
 * row is drawn, and marks the row for a moment (`[data-revealed]` in styles.css). Rows drawn later
 * (a list still loading, a list drawn a part at a time) are found as they arrive.
 */
export const RevealHost: FC = () => {
  const target = useRevealStore((store) => store.target);
  const location = useRouterState({ select: (state) => state.location });
  const arrived =
    target !== null &&
    location.pathname === target.to &&
    Object.entries(target.search).every(
      ([key, value]) => String((location.search as Record<string, unknown>)[key] ?? '') === value,
    );

  useEffect(() => {
    if (!target || !arrived) return;
    let frame = 0;
    let giveUp: ReturnType<typeof setTimeout> | undefined;
    const find = () => {
      frame = 0;
      for (const row of document.querySelectorAll<HTMLElement>(`[${REVEAL_ATTRIBUTE}]`)) {
        if (!matchesReveal(row.getAttribute(REVEAL_ATTRIBUTE) ?? '', target.keys)) continue;
        row.scrollIntoView?.({
          block: 'center',
          inline: 'center',
          behavior: smooth() ? 'smooth' : 'auto',
        });
        row.setAttribute('data-revealed', '');
        setTimeout(() => row.removeAttribute('data-revealed'), FLASH_MS);
        finish();
        return true;
      }
      return false;
    };
    const observer = new MutationObserver(() => {
      if (!frame) frame = requestAnimationFrame(find);
    });
    const finish = () => {
      observer.disconnect();
      clearTimeout(giveUp);
      if (frame) cancelAnimationFrame(frame);
      useRevealStore.getState().done();
    };
    if (!find()) {
      observer.observe(document.body, { childList: true, subtree: true });
      giveUp = setTimeout(finish, revealWait(target));
    }
    return () => {
      observer.disconnect();
      clearTimeout(giveUp);
      if (frame) cancelAnimationFrame(frame);
    };
  }, [target, arrived]);

  return null;
};
