import { useLocation } from '@tanstack/react-router';
import { useCallback, useReducer } from 'react';
import type { SetStateAction } from 'react';

/**
 * What a list had loaded or opened (how many shown after Show more, a See all opened, a season
 * picked), kept per page address (route and query) and list. Going into a title and back finds
 * the list as it was left, so the scroll memory (`scrollMemory.ts`) can return to the same spot
 * without anyone pressing Show more again. A new search or page is a new address and starts
 * afresh. Held for this run only; the oldest go first.
 */

/** Lists remembered; the oldest are forgotten first. */
const KEPT = 300;

const kept = new Map<string, unknown>();

/** Forgets every list (tests). */
export const forgetLists = () => kept.clear();

const remember = (key: string, value: unknown) => {
  kept.delete(key);
  kept.set(key, value);
  while (kept.size > KEPT) {
    const oldest = kept.keys().next().value;
    if (oldest === undefined) break;
    kept.delete(oldest);
  }
};

/**
 * `useState`, kept for this page's address under `id` (unique within the page): the value set
 * last on this address, else `initial`.
 */
export const useListMemory = <T>(id: string, initial: T) => {
  const href = useLocation({ select: (location) => location.href });
  const key = `${href}\n${id}`;
  const [, redraw] = useReducer((count: number) => count + 1, 0);
  const value = (kept.has(key) ? kept.get(key) : initial) as T;
  const set = useCallback(
    (next: SetStateAction<T>) => {
      const before = (kept.has(key) ? kept.get(key) : initial) as T;
      remember(key, typeof next === 'function' ? (next as (value: T) => T)(before) : next);
      redraw();
    },
    // `initial` is a constant of each list.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [key],
  );
  return [value, set] as const;
};
