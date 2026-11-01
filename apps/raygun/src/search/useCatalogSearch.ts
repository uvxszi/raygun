import { commands } from '@raygun/bindings';
import type { FilterValue } from '@raygun/bindings';
import { useInfiniteQuery, useQuery } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';
import type { Medium } from '../manga/medium';

const STALE_MS = 5 * 60_000;

export type FilterValues = Record<string, FilterValue>;

/** Every source's first page of results for `query` (manga or novels). */
export const useGlobalSearch = (medium: Medium, query: string, enabled = true) =>
  useQuery({
    queryKey: [medium, 'globalSearch', query.trim()],
    queryFn: () => unwrapResult(commands.catalogGlobalSearch(medium, query.trim())),
    enabled: enabled && query.trim() !== '',
    staleTime: STALE_MS,
  });

export const useSourceFilters = (medium: Medium, sourceId: string) =>
  useQuery({
    queryKey: [medium, 'filters', sourceId],
    queryFn: () => unwrapResult(commands.catalogFilters(medium, sourceId)),
    enabled: sourceId !== '',
    staleTime: 30 * 60_000,
  });

/** One source's results with its filters, page by page. */
export const useFilteredSearch = (
  medium: Medium,
  sourceId: string,
  query: string,
  filters: FilterValues,
) => {
  const active = query.trim() !== '' || Object.keys(filters).length > 0;
  return useInfiniteQuery({
    queryKey: [medium, 'filteredSearch', sourceId, query.trim(), filters],
    queryFn: ({ pageParam }) =>
      unwrapResult(commands.catalogSearch(medium, sourceId, query.trim(), pageParam, filters)),
    initialPageParam: 1,
    getNextPageParam: (last, pages) => (last.hasNextPage ? pages.length + 1 : undefined),
    enabled: sourceId !== '' && active,
    staleTime: STALE_MS,
  });
};

/** Tracks in the device's library matching `query`. */
export const useLibraryTrackSearch = (query: string, enabled = true) =>
  useQuery({
    queryKey: ['library', 'search', query.trim()],
    queryFn: () => unwrapResult(commands.librarySearch(query.trim())),
    enabled: enabled && query.trim() !== '',
    staleTime: 60_000,
  });
