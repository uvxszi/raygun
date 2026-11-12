import { useCallback, useSyncExternalStore } from 'react';

import type { Medium } from '../manga/medium';

const groupKey = (medium: Medium) => `raygun.search.grouped.${medium}`;

/** Choices kept only in memory, while the browser keeps nothing (storage blocked). */
const unsaved = new Map<Medium, boolean>();
const listeners = new Set<() => void>();

const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
};

const readGrouped = (medium: Medium) => {
  const kept = unsaved.get(medium);
  if (kept !== undefined) return kept;
  try {
    return localStorage.getItem(groupKey(medium)) === '1';
  } catch {
    return false;
  }
};

/**
 * How a kind's search arranges what several sources found: Best matches (one ranked list, the
 * default) or By source (a row each). Remembered per device and kind, and one choice for every
 * list of that kind on the screen (Videos' addons and its sources follow the same one).
 */
export const useGrouped = (medium: Medium): [boolean, (grouped: boolean) => void] => {
  const grouped = useSyncExternalStore(subscribe, () => readGrouped(medium));
  const setGrouped = useCallback(
    (next: boolean) => {
      try {
        localStorage.setItem(groupKey(medium), next ? '1' : '0');
        unsaved.delete(medium);
      } catch {
        // A per-device convenience: kept for this run only.
        unsaved.set(medium, next);
      }
      for (const listener of listeners) listener();
    },
    [medium],
  );
  return [grouped, setGrouped];
};
