import { commands } from '@raygun/bindings';
import type { FilterValue, SourceSearchResult } from '@raygun/bindings';
import { useInfiniteQuery, useQueries, useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';

import { unwrapResult } from '../lib/commandResult';
import type { Medium } from '../manga/medium';
import type { SourceAnswer } from './sourceHealth';
import { selectSourceResult, useSearchFilter } from './searchFilter';
import type { SearchFilter } from './searchFilter';

const STALE_MS = 5 * 60_000;

/** Source requests Search keeps in flight at once (M46.7); the rest wait their turn. */
export const MAX_IN_FLIGHT = 6;
let inFlight = 0;
const waiting: (() => void)[] = [];
/** Requests asking now, by what they ask: a second ask for the same waits for the first. */
const asking = new Map<string, Promise<unknown>>();

const startNext = () => {
  if (inFlight < MAX_IN_FLIGHT) waiting.shift()?.();
};

/**
 * Takes one of the places, waiting for it when they're all taken; a search nobody waits for any
 * more leaves the line.
 */
const turn = (signal: AbortSignal | undefined) =>
  new Promise<void>((resolve, reject) => {
    if (signal?.aborted) {
      reject(signal.reason ?? new Error('aborted'));
      return;
    }
    if (inFlight < MAX_IN_FLIGHT && waiting.length === 0) {
      inFlight += 1;
      resolve();
      return;
    }
    const go = () => {
      signal?.removeEventListener('abort', leave);
      inFlight += 1;
      resolve();
    };
    const leave = () => {
      const at = waiting.indexOf(go);
      if (at >= 0) waiting.splice(at, 1);
      reject(signal?.reason ?? new Error('aborted'));
    };
    waiting.push(go);
    signal?.addEventListener('abort', leave, { once: true });
  });

/**
 * Runs `task` when one of the few places is free (M46.7). A search whose query changed before
 * its turn (TanStack aborts it) never runs, so the new query's sources aren't kept waiting
 * behind the old one's. Once asked, a source's answer is waited for even if the query moves on
 * (the call can't be taken back), and asking the same again meanwhile waits for that answer.
 */
export const queued = async <T>(
  key: string,
  task: () => Promise<T>,
  signal?: AbortSignal,
): Promise<T> => {
  const running = asking.get(key);
  if (running) return running as Promise<T>;
  await turn(signal);
  // The same was asked while this one waited for its turn: its answer does for both.
  const started = asking.get(key);
  if (started) {
    inFlight -= 1;
    startNext();
    return started as Promise<T>;
  }
  const answer = task().finally(() => {
    asking.delete(key);
    inFlight -= 1;
    startNext();
  });
  asking.set(key, answer);
  return answer;
};

/**
 * One source's first page of results for `query` (its popular titles while blank), queued with
 * the other sources' (M46.7): rows ask as they come into view, a few at a time. With a `filter`
 * (Search's "No 18+"), what was kept from before leaves the same out as Rust does.
 */
export const sourceSearchOptions = (
  medium: Medium,
  sourceId: string,
  query: string,
  filter?: SearchFilter,
) => ({
  ...(filter ? { select: selectSourceResult(filter) } : {}),
  queryKey: [medium, 'sourceSearch', sourceId, query.trim()],
  queryFn: async ({ signal }: { signal?: AbortSignal }): Promise<SourceSearchResult | null> => {
    const results = await queued(
      JSON.stringify([medium, sourceId, query.trim()]),
      () => unwrapResult(commands.catalogGlobalSearch(medium, query.trim(), [sourceId])),
      signal,
    );
    return results.find((found) => found.source.id === sourceId) ?? null;
  },
  staleTime: STALE_MS,
});

export const useSourceSearch = (
  medium: Medium,
  sourceId: string,
  query: string,
  enabled: boolean,
) => {
  const filter = useSearchFilter();
  return useQuery({ ...sourceSearchOptions(medium, sourceId, query, filter), enabled });
};

/** How a source's search answered, or `undefined` while it hasn't (`''` as a key part). */
const answerOf = (result: {
  isError: boolean;
  isSuccess: boolean;
  data?: SourceSearchResult | null;
}): SourceAnswer | '' => {
  if (result.isError || result.data?.error) return 'failed';
  if (!result.isSuccess) return '';
  return (result.data?.items.length ?? 0) > 0 ? 'results' : 'empty';
};

/**
 * How each of `sourceIds` answered `query` so far, from their rows' searches already in the query
 * cache: nothing is asked from here, and one that hasn't answered gives `undefined`.
 */
export const useSourceAnswers = (medium: Medium, sourceIds: string[], query: string) => {
  // One string, so the answers only re-render what shows them when one of them changes.
  const answers = useQueries({
    queries: sourceIds.map((id) => ({ ...sourceSearchOptions(medium, id, query), enabled: false })),
    combine: (results) => results.map(answerOf).join(','),
  });
  const ids = sourceIds.join(',');
  return useMemo(() => {
    const byId = new Map<string, SourceAnswer>();
    const found = answers.split(',');
    ids.split(',').forEach((id, index) => {
      const answer = found[index];
      if (answer) byId.set(id, answer as SourceAnswer);
    });
    return (id: string) => byId.get(id);
  }, [answers, ids]);
};

export type FilterValues = Record<string, FilterValue>;

/**
 * Every source's first page of results for `query` (manga, novels or videos), or only `sources`';
 * each source's popular titles while `query` is blank.
 */
export const useGlobalSearch = (
  medium: Medium,
  query: string,
  enabled = true,
  sources: string[] = [],
) =>
  useQuery({
    queryKey: [medium, 'globalSearch', query.trim(), sources],
    queryFn: () => unwrapResult(commands.catalogGlobalSearch(medium, query.trim(), sources)),
    enabled,
    staleTime: STALE_MS,
  });

export const useSourceFilters = (medium: Medium, sourceId: string) =>
  useQuery({
    queryKey: [medium, 'filters', sourceId],
    queryFn: () => unwrapResult(commands.catalogFilters(medium, sourceId)),
    enabled: sourceId !== '',
    staleTime: 30 * 60_000,
  });

/** One source's results with its filters, page by page; its popular titles before either. */
export const useFilteredSearch = (
  medium: Medium,
  sourceId: string,
  query: string,
  filters: FilterValues,
) =>
  useInfiniteQuery({
    queryKey: [medium, 'filteredSearch', sourceId, query.trim(), filters],
    queryFn: ({ pageParam }) =>
      unwrapResult(commands.catalogSearch(medium, sourceId, query.trim(), pageParam, filters)),
    initialPageParam: 1,
    getNextPageParam: (last, pages) => (last.hasNextPage ? pages.length + 1 : undefined),
    enabled: sourceId !== '',
    staleTime: STALE_MS,
  });

/** Tracks in the device's library matching `query`. */
export const useLibraryTrackSearch = (query: string, enabled = true) =>
  useQuery({
    queryKey: ['library', 'search', query.trim()],
    queryFn: () => unwrapResult(commands.librarySearch(query.trim())),
    enabled: enabled && query.trim() !== '',
    staleTime: 60_000,
  });
