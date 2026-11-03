import { commands } from '@raygun/bindings';
import type { PlayContext } from '@raygun/bindings';
import { useEffect } from 'react';

/** What a page plays from (M46.37), without the time it's remembered at. */
export type PageContext = Omit<PlayContext, 'playedAt'>;

/** The page on screen that songs would be played from, if it is one. */
let current: PageContext | null = null;

/**
 * Says what the page plays from (an album, a playlist, a mix...), so a play started here shows
 * in Home's Your music (M46.37). `null` while it's still loading.
 */
export const usePlayContext = (context: PageContext | null) => {
  const key = context ? JSON.stringify(context) : null;
  useEffect(() => {
    if (!key) return;
    const mine = JSON.parse(key) as PageContext;
    current = mine;
    return () => {
      if (current === mine) current = null;
    };
  }, [key]);
};

/** Remembers the page a play started from; plays from anywhere else aren't remembered. */
export const rememberPlay = () => {
  if (current) {
    void commands.recentContextsAdd({ ...current, playedAt: 0 }).catch(() => undefined);
  }
};
