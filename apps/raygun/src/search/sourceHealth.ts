import { create } from 'zustand';

const KEY = 'raygun.sourceHealth';
/** A failure older than this no longer counts: the source may work again. */
const REMEMBER_MS = 3 * 24 * 60 * 60_000;

const read = (): Record<string, number> => {
  try {
    const saved: unknown = JSON.parse(localStorage.getItem(KEY) ?? '{}');
    if (!saved || typeof saved !== 'object') return {};
    const now = Date.now();
    return Object.fromEntries(
      Object.entries(saved as Record<string, unknown>).filter(
        (entry): entry is [string, number] =>
          typeof entry[1] === 'number' && now - entry[1] < REMEMBER_MS,
      ),
    );
  } catch {
    return {};
  }
};

const write = (failures: Record<string, number>) => {
  try {
    localStorage.setItem(KEY, JSON.stringify(failures));
  } catch {
    // A per-device convenience.
  }
};

/** Offline Mode, as the shell marks the document (`network/useNetworkMode`): nothing was asked. */
const offlineMode = () =>
  typeof document !== 'undefined' && document.documentElement.hasAttribute('data-offline');

type SourceHealth = {
  /** Sources whose last search or listing failed, and when. */
  failures: Record<string, number>;
  failed: (id: string) => void;
  worked: (id: string) => void;
};

/**
 * Which sources failed lately (M46.9), so lists can put them last. Remembered on this device for
 * a few days; a source that answers again is cleared at once.
 */
export const useSourceHealth = create<SourceHealth>((set, get) => ({
  failures: read(),
  failed: (id) => {
    // A failure while Offline Mode is on isn't the source's.
    if (offlineMode()) return;
    const failures = { ...get().failures, [id]: Date.now() };
    write(failures);
    set({ failures });
  },
  worked: (id) => {
    if (!(id in get().failures)) return;
    const failures = { ...get().failures };
    delete failures[id];
    write(failures);
    set({ failures });
  },
}));

/** How a source answered a search: with results, with none, or with an error. */
export type SourceAnswer = 'results' | 'empty' | 'failed';

const ANSWER_RANK: Record<SourceAnswer, number> = { results: 0, empty: 1, failed: 2 };

/**
 * `sources` as a search shows them: the ones that answered with results first, then the ones
 * that answered with none, then the ones that failed. A source whose answer isn't known yet
 * (`answer` gives `undefined`) keeps its place; each group keeps its order.
 */
export const answeredFirst = <T>(
  sources: T[],
  answer: (source: T) => SourceAnswer | undefined,
): T[] => {
  const known = sources.flatMap((source, index) => {
    const found = answer(source);
    return found === undefined ? [] : [{ source, index, rank: ANSWER_RANK[found] }];
  });
  const ranked = [...known].sort((a, b) => a.rank - b.rank || a.index - b.index);
  const shown = [...sources];
  known.forEach((slot, at) => {
    shown[slot.index] = ranked[at]?.source ?? slot.source;
  });
  return shown;
};

/** `sources` with the ones that failed lately moved to the end (keeping each group's order). */
export const failingLast = <T extends { id: string }>(
  sources: T[],
  failures: Record<string, number>,
  enabled = true,
) =>
  enabled
    ? [
        ...sources.filter((source) => !(source.id in failures)),
        ...sources.filter((source) => source.id in failures),
      ]
    : sources;
