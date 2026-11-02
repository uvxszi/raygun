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
