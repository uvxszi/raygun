import type { QueryClient, QueryKey } from '@tanstack/react-query';

/** How deep an answer is searched: lists of results inside pages inside answers. */
const MAX_DEPTH = 6;

/**
 * Visits every object in an answer (M46.40: what a list or a search showed, for a page to start
 * from), depth first; `visit` returns true to stop.
 */
const walk = (value: unknown, visit: (record: Record<string, unknown>) => boolean, depth = 0) => {
  if (depth > MAX_DEPTH || value === null || typeof value !== 'object') return false;
  if (Array.isArray(value)) {
    for (const item of value) if (walk(item, visit, depth + 1)) return true;
    return false;
  }
  const record = value as Record<string, unknown>;
  if (visit(record)) return true;
  for (const child of Object.values(record)) {
    if (child && typeof child === 'object' && walk(child, visit, depth + 1)) return true;
  }
  return false;
};

/** The first thing `match` finds in the answers cached under `prefix`. */
export const findInCache = <T>(
  client: QueryClient,
  prefix: QueryKey,
  match: (record: Record<string, unknown>) => T | null,
): T | null => {
  let found: T | null = null;
  for (const [, data] of client.getQueriesData<unknown>({ queryKey: prefix })) {
    walk(data, (record) => {
      found = match(record);
      return found !== null;
    });
    if (found !== null) return found;
  }
  return null;
};

/**
 * Everything `match` finds in the answers cached under `prefix`, each once by `key`: the first
 * find, or with `merge`, the first combined with each later one (to fill what it lacked).
 */
export const collectFromCache = <T>(
  client: QueryClient,
  prefix: QueryKey,
  match: (record: Record<string, unknown>) => T | null,
  key: (item: T) => string,
  merge?: (kept: T, found: T) => T,
): T[] => {
  const seen = new Map<string, T>();
  for (const [, data] of client.getQueriesData<unknown>({ queryKey: prefix })) {
    walk(data, (record) => {
      const item = match(record);
      if (item === null) return false;
      const kept = seen.get(key(item));
      if (kept === undefined) seen.set(key(item), item);
      else if (merge) seen.set(key(item), merge(kept, item));
      return false;
    });
  }
  return [...seen.values()];
};
