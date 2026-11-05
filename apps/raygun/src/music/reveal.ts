import { useMemo } from 'react';
import { create } from 'zustand';

import { keysOf, sameSong } from './trackKey';

/**
 * Showing the playing song on the page it was played from (the player's "Playing from"): the
 * player asks for a song by its keys, the page's rows say which song each is (`data-reveal`), and
 * `RevealHost` scrolls to the first match once the page and its list are there, then flashes it.
 * Lists that draw a part at a time draw far enough first (`useRevealIndex`).
 */

/** A row's attribute naming its song. */
export const REVEAL_ATTRIBUTE = 'data-reveal';

/** How a row is named: the keys a song with these names answers to. */
export const songReveal = (title: string, artist: string | null | undefined) =>
  keysOf(title, artist).join('\t');

/** How an episode's or a chapter's row is named. */
export const episodeReveal = (episodeId: string) => `episode:${episodeId}`;

/**
 * How a release's tile is named (an artist's discography): its source and id, as Rust names it
 * in a new release's notification (`follows::release_route`).
 */
export const releaseReveal = (release: { provider: { id: string }; id: string }) =>
  `release:${release.provider.id}/${release.id}`;

/** Names that match only exactly: episodes and releases go by their ids, not like songs. */
const exactOnly = (name: string) => name.startsWith('episode:') || name.startsWith('release:');

/** Whether a row named `value` is one of `keys`. Episodes and releases match only by their id. */
export const matchesReveal = (value: string, keys: string[]) =>
  value
    .split('\t')
    .some((row) =>
      keys.some((key) => key === row || (!exactOnly(key) && !exactOnly(row) && sameSong(key, row))),
    );

/** A song to show, on the page at `to` with `search`. */
export type RevealTarget = {
  keys: string[];
  to: string;
  search: Record<string, string>;
  /** How long its page may take to load the list, when longer than {@link REVEAL_WAIT_MS}. */
  waitMs?: number;
};

type RevealStore = {
  target: RevealTarget | null;
  request: (target: RevealTarget) => void;
  done: () => void;
};

/** How long a page may take to open and load its list before the song isn't looked for. */
export const REVEAL_WAIT_MS = 10_000;

/** How long `target`'s page may take to load its list. */
export const revealWait = (target: RevealTarget) => target.waitMs ?? REVEAL_WAIT_MS;

export const useRevealStore = create<RevealStore>((set, get) => ({
  target: null,
  request: (target) => {
    set({ target });
    // A page never reached (another one opened meanwhile) doesn't scroll when opened later.
    setTimeout(
      () => {
        if (get().target === target) set({ target: null });
      },
      revealWait(target) * 2,
    );
  },
  done: () => set({ target: null }),
}));

/**
 * Where the asked-for song is in `rows` (-1: not asked, or not here), for lists that draw only
 * part of themselves to draw that far. `nameOf` should be stable (a module-level function).
 */
export const useRevealIndex = <Row>(rows: Row[], nameOf: (row: Row) => string): number => {
  const keys = useRevealStore((store) => store.target?.keys ?? null);
  return useMemo(
    () => (keys ? rows.findIndex((row) => matchesReveal(nameOf(row), keys)) : -1),
    [keys, rows, nameOf],
  );
};
