import { commands } from '@raygun/bindings';
import type {
  StremioAddon,
  StremioCatalogInfo,
  StremioCatalogRow,
  StremioConfigValue,
  StremioExtraValue,
  StremioPlayRequest,
} from '@raygun/bindings';
import {
  useInfiniteQuery,
  useMutation,
  useQueries,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { useSettings } from '../settings/useSettings';

const STREMIO = ['stremio'];

/** Every catalog of the addons that answer: board rows, searches and what Discover browses. */
export const useStremioCatalogs = (enabled = true) =>
  useQuery({
    queryKey: [...STREMIO, 'catalogs'],
    queryFn: () => unwrapResult(commands.stremioCatalogs()),
    staleTime: 10 * 60 * 1000,
    enabled,
  });

/** A catalog's first page, once its row is about to be seen (`enabled`). */
export const useStremioRow = (catalog: StremioCatalogInfo, enabled: boolean) =>
  useQuery({
    queryKey: [
      ...STREMIO,
      'catalog',
      catalog.addonId,
      catalog.kind,
      catalog.catalogId,
      [],
      'first',
    ],
    queryFn: () =>
      unwrapResult(
        commands.stremioCatalog(catalog.addonId, catalog.kind, catalog.catalogId, [], 0),
      ),
    enabled,
    staleTime: 10 * 60 * 1000,
    retry: false,
  });

/** One catalog, page after page, with the extras chosen (a genre, a year...). */
export const useStremioCatalog = (
  addonId: string,
  kind: string,
  catalogId: string,
  extra: StremioExtraValue[],
) =>
  useInfiniteQuery({
    queryKey: [...STREMIO, 'catalog', addonId, kind, catalogId, extra],
    queryFn: ({ pageParam }) =>
      unwrapResult(commands.stremioCatalog(addonId, kind, catalogId, extra, pageParam)),
    initialPageParam: 0,
    getNextPageParam: (last, pages) =>
      last.hasMore && last.items.length > 0
        ? pages.reduce((count, page) => count + page.items.length, 0)
        : undefined,
    enabled: addonId !== '' && catalogId !== '',
    staleTime: 10 * 60 * 1000,
  });

export type SearchRows = {
  /** Rows with results, in the catalogs' order (failing addons last), as they arrive. */
  rows: StremioCatalogRow[];
  /** Some catalog hasn't answered yet. */
  pending: boolean;
  /** Every catalog answered (or failed) and none found anything. */
  empty: boolean;
};

/**
 * Every searching catalog's results for `query`, each asked on its own so a slow addon holds
 * nothing up; a newer query stops the older searches in Rust. Not `enabled`, nothing is asked.
 */
export const useStremioSearchRows = (query: string, enabled = true): SearchRows => {
  const catalogs = useStremioCatalogs(enabled);
  const trimmed = enabled ? query.trim() : '';
  const searchable = trimmed ? (catalogs.data ?? []).filter((catalog) => catalog.searchable) : [];
  return useQueries({
    queries: searchable.map((catalog) => ({
      queryKey: [...STREMIO, 'search', catalog.addonId, catalog.kind, catalog.catalogId, trimmed],
      queryFn: () =>
        unwrapResult(
          commands.stremioSearchCatalog(catalog.addonId, catalog.kind, catalog.catalogId, trimmed),
        ),
      staleTime: 5 * 60 * 1000,
      retry: false,
    })),
    combine: (results) => {
      const pending =
        enabled &&
        (catalogs.isPending || (trimmed !== '' && results.some((result) => result.isPending)));
      const rows = results.flatMap((result) =>
        result.data && result.data.items.length > 0 ? [result.data] : [],
      );
      return { rows, pending, empty: !pending && rows.length === 0 };
    },
  });
};

/** A title's details; progress shown on it follows the player (`shell/useLiveInvalidation`). */
export const useStremioMeta = (kind: string, id: string) =>
  useQuery({
    queryKey: [...STREMIO, 'meta', kind, id],
    queryFn: () => unwrapResult(commands.stremioMeta(kind, id)),
    enabled: kind !== '' && id !== '',
  });

/** Streams for a movie or episode of the title `metaId`; `videoId` is null until one is picked. */
export const useStremioStreams = (kind: string, metaId: string, videoId: string | null) =>
  useQuery({
    queryKey: [...STREMIO, 'streams', kind, metaId, videoId],
    queryFn: () => unwrapResult(commands.stremioStreams(kind, metaId || null, videoId ?? '')),
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

/** Opens a web page or (desktop) a magnet link with the system. */
export const useOpenStremioLink = () =>
  useMutation({ mutationFn: (url: string) => unwrapResult(commands.stremioOpenLink(url)) });

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

/** What an addon is before it's added: what it provides, whether it must be set up first. */
export const useAddonPreview = () =>
  useMutation({ mutationFn: (url: string) => unwrapResult(commands.stremioPreview(url)) });

/** An addon has Raygun's own setup form when its manifest declares the settings it takes. */
export const hasConfigForm = (addon: Pick<StremioAddon, 'config'>) =>
  (addon.config?.length ?? 0) > 0;

/** Opens an addon's setup page in the browser. */
export const useConfigureAddon = () =>
  useMutation({ mutationFn: (url: string) => unwrapResult(commands.stremioConfigure(url)) });

/** Adding, removing, turning off or reordering addons changes every catalog, search and title. */
const useAddonChange = <T, V>(mutationFn: (value: V) => Promise<T>) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn,
    onSettled: () => queryClient.invalidateQueries({ queryKey: STREMIO }),
  });
};

export const useAddAddon = () =>
  useAddonChange((url: string) => unwrapResult(commands.stremioAdd(url)));

/**
 * Sets up an addon with the settings its manifest asks for (Raygun's own form), then adds it at
 * the address that makes, in place of the same addon added before.
 */
export const useAddConfiguredAddon = () =>
  useAddonChange(({ url, values }: { url: string; values: StremioConfigValue[] }) =>
    unwrapResult(commands.stremioAddConfigured(url, values)),
  );

export const useRemoveAddon = () =>
  useAddonChange((id: string) => unwrapResult(commands.stremioRemove(id)));

export const useSetAddonEnabled = () =>
  useAddonChange(({ id, enabled }: { id: string; enabled: boolean }) =>
    unwrapResult(commands.stremioSetEnabled(id, enabled)),
  );

export const useSetAddonOrder = () =>
  useAddonChange((ids: string[]) => unwrapResult(commands.stremioSetOrder(ids)));
