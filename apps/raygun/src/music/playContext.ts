import { commands } from '@raygun/bindings';
import type { PlayContext } from '@raygun/bindings';
import { useEffect } from 'react';

/** What a page plays from (M46.37), without the time it's remembered at. */
export type PageContext = Omit<PlayContext, 'playedAt'>;

/** Contexts only the player shows ("Playing from"); Home's Your music leaves them out. */
const NOT_REMEMBERED = new Set(['favorites', 'downloaded', 'search']);

/** The page on screen that songs would be played from, if it is one. */
let current: PageContext | null = null;

/**
 * Says what the page plays from (an album, a playlist, a mix...), so a play started here shows
 * in Home's Your music (M46.37) and as the player's "Playing from". `null` while it's still
 * loading.
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

/**
 * Tells Rust where the play about to start comes from (the page on screen, or nowhere), so the
 * queue it makes carries it; and remembers the page for Home. Awaited before the play command.
 */
export const rememberPlay = async () => {
  const context = current ? { ...current, playedAt: 0 } : null;
  await Promise.resolve(commands.playbackArmContext(context)).catch(() => undefined);
  if (context && !NOT_REMEMBERED.has(context.kind)) {
    void Promise.resolve(commands.recentContextsAdd(context)).catch(() => undefined);
  }
};
