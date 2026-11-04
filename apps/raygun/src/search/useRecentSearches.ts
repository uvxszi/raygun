import { commands } from '@raygun/bindings';
import type { RecentSearch } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useCallback, useRef } from 'react';

import { unwrapResult } from '../lib/commandResult';
import { useSettings } from '../settings/useSettings';
import { isSearchType } from './searchTypes';
import type { SearchType } from './searchTypes';

export const RECENT_SEARCHES = ['search', 'recent'];

/** Searches kept on this device, newest first (none while Settings → Privacy says not to). */
export const useRecentSearches = () => {
  const settings = useSettings().data;
  const kept = settings !== undefined && settings.privacy.searchHistory !== false;
  const recent = useQuery({
    queryKey: RECENT_SEARCHES,
    queryFn: () => unwrapResult(commands.searchRecent()),
    staleTime: 60_000,
    enabled: kept,
  });
  return { ...recent, data: kept ? recent.data : [] };
};

/** A kept search's kind, as Search names it. */
export const kindOf = (search: RecentSearch): SearchType =>
  isSearchType(search.kind) ? search.kind : 'all';

/**
 * Remembers a search someone made: pressed Enter on, or opened something from. Rust decides
 * whether searches are kept at all; asking twice in a row for the same one asks once.
 */
export const useRememberSearch = () => {
  const client = useQueryClient();
  const last = useRef('');
  const { mutate } = useMutation({
    mutationFn: ({ query, kind }: { query: string; kind: SearchType }) =>
      unwrapResult(commands.searchRecentAdd(query, kind)),
    onSuccess: (searches) => client.setQueryData(RECENT_SEARCHES, searches),
  });
  return useCallback(
    (query: string, kind: SearchType) => {
      const wanted = query.trim();
      const key = `${kind}\n${wanted.toLocaleLowerCase()}`;
      if (wanted.length < 2 || key === last.current) return;
      last.current = key;
      mutate({ query: wanted, kind });
    },
    [mutate],
  );
};

/** Forgets one kept search, or with `null` every one. */
export const useForgetSearch = () => {
  const client = useQueryClient();
  return useMutation({
    mutationFn: async (search: RecentSearch | null) =>
      search
        ? unwrapResult(commands.searchRecentRemove(search.query, search.kind))
        : unwrapResult(commands.searchRecentClear()).then(() => []),
    onSuccess: (searches: RecentSearch[]) => client.setQueryData(RECENT_SEARCHES, searches),
  });
};
