import { commands } from '@raygun/bindings';
import type { FilterValue } from '@raygun/bindings';
import { useInfiniteQuery, useQuery } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';
import type { Medium } from '../manga/medium';

const STALE_MS = 5 * 60_000;

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
