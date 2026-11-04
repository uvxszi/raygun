import { commands } from '@raygun/bindings';
import type { StremioPlayRequest } from '@raygun/bindings';
import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { useSettings } from '../settings/useSettings';

const STREMIO = ['stremio'];

/** The Watch home: the first page of every addon catalog. */
export const useStremioHome = (enabled = true) =>
  useQuery({
    queryKey: [...STREMIO, 'home'],
    queryFn: () => unwrapResult(commands.stremioHome()),
    enabled,
    staleTime: 10 * 60 * 1000,
  });

/** One catalog, page after page, optionally for a genre. */
export const useStremioCatalog = (
  addonId: string,
  kind: string,
  catalogId: string,
  genre: string | null,
) =>
  useInfiniteQuery({
    queryKey: [...STREMIO, 'catalog', addonId, kind, catalogId, genre],
    queryFn: ({ pageParam }) =>
      unwrapResult(commands.stremioCatalog(addonId, kind, catalogId, genre, pageParam)),
    initialPageParam: 0,
    getNextPageParam: (last, pages) =>
      last.hasMore && last.items.length > 0
        ? pages.reduce((count, page) => count + page.items.length, 0)
        : undefined,
    enabled: addonId !== '' && catalogId !== '',
    staleTime: 10 * 60 * 1000,
  });

export const useStremioSearch = (query: string) =>
  useQuery({
    queryKey: [...STREMIO, 'search', query],
    queryFn: () => unwrapResult(commands.stremioSearch(query)),
    enabled: query.trim() !== '',
    staleTime: 5 * 60 * 1000,
  });

/** A title's details; progress shown on it follows the player (`shell/useLiveInvalidation`). */
export const useStremioMeta = (kind: string, id: string) =>
  useQuery({
    queryKey: [...STREMIO, 'meta', kind, id],
    queryFn: () => unwrapResult(commands.stremioMeta(kind, id)),
    enabled: kind !== '' && id !== '',
  });

/** Streams for a movie or episode; `videoId` is null until one is picked. */
export const useStremioStreams = (kind: string, videoId: string | null) =>
  useQuery({
    queryKey: [...STREMIO, 'streams', kind, videoId],
    queryFn: () => unwrapResult(commands.stremioStreams(kind, videoId ?? '')),
    enabled: videoId !== null,
    staleTime: 5 * 60 * 1000,
  });

/** Plays a stream, with subtitles in the subtitle language (Settings → Player) first. */
export const useStremioPlay = () => {
  const { i18n } = useTranslation();
  const subtitleLanguage = useSettings().data?.subtitleLanguage || i18n.language;
  return useMutation({
    mutationFn: (request: StremioPlayRequest) =>
      unwrapResult(commands.stremioPlay(request, subtitleLanguage)),
  });
};

export type WatchedMark = {
  kind: string;
  videoId: string;
  title: string;
  subtitle: string | null;
  poster: string | null;
  watched: boolean;
};

export const useSetWatched = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (mark: WatchedMark) =>
      unwrapResult(
        commands.stremioSetWatched(
          mark.kind,
          mark.videoId,
          mark.title,
          mark.subtitle,
          mark.poster,
          mark.watched,
        ),
      ),
    // The title's page and what to continue watching.
    onSettled: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: [...STREMIO, 'meta'] }),
        queryClient.invalidateQueries({ queryKey: ['watch'] }),
      ]),
  });
};

export const useStremioAddons = () =>
  useQuery({
    queryKey: [...STREMIO, 'addons'],
    queryFn: () => unwrapResult(commands.stremioAddons()),
  });

export const useStremioSuggestions = () =>
  useQuery({
    queryKey: [...STREMIO, 'suggestions'],
    queryFn: () => unwrapResult(commands.stremioSuggestions()),
  });

/** Adding or removing an addon changes every catalog, search and title. */
const useAddonChange = <T>(mutationFn: (value: string) => Promise<T>) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn,
    onSettled: () => queryClient.invalidateQueries({ queryKey: STREMIO }),
  });
};

export const useAddAddon = () =>
  useAddonChange((url: string) => unwrapResult(commands.stremioAdd(url)));

export const useRemoveAddon = () =>
  useAddonChange((id: string) => unwrapResult(commands.stremioRemove(id)));
