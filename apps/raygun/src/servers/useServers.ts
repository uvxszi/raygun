import { commands } from '@raygun/bindings';
import type {
  ServerInfo,
  ServerInput,
  ServerKind,
  ServerSyncPrefs,
  ServerSyncStatus,
} from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';

const SERVERS = ['servers'] as const;

/** The servers connected; each says why it isn't working when it isn't. */
export const useServers = () =>
  useQuery({
    queryKey: SERVERS,
    queryFn: () => unwrapResult(commands.serversList()),
  });

/**
 * Adding or removing a server answers with the new list. Its sources reach the rest of the app
 * with the extensions reload Rust announces.
 */
const useServerMutation = <Args>(run: (args: Args) => Promise<ServerInfo[]>) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: run,
    onSuccess: (servers) => queryClient.setQueryData(SERVERS, servers),
  });
};

export const useAddServer = () =>
  useServerMutation((input: ServerInput) => unwrapResult(commands.serversAdd(input)));

export const useRemoveServer = () =>
  useServerMutation((id: string) => unwrapResult(commands.serversRemove(id)));

/** The kinds, in the order the add sheet offers them. */
export const SERVER_KINDS: ServerKind[] = [
  'jellyfin',
  'emby',
  'subsonic',
  'navidrome',
  'audiobookshelf',
  'opds',
];

/** Kinds that take an API key in place of a password. */
export const TAKES_API_KEY: ReadonlySet<ServerKind> = new Set([
  'jellyfin',
  'emby',
  'subsonic',
  'navidrome',
  'audiobookshelf',
]);

/** What a server brings, as the words its chips show (translation keys). */
export const BRINGS: Record<string, string> = {
  'video.source': 'servers.brings.videos',
  'music.metadata': 'servers.brings.music',
  lyrics: 'servers.brings.lyrics',
  'audiobook.directory': 'servers.brings.audiobooks',
  'manga.source': 'servers.brings.comics',
  'novel.source': 'servers.brings.books',
};

/** A server's address without its scheme, for its row. */
export const hostOf = (address: string): string => {
  try {
    const url = new URL(address);
    return url.port ? `${url.hostname}:${url.port}` : url.hostname;
  } catch {
    return address;
  }
};

/** Kinds that keep a music library Raygun syncs with: stars, playlists, plays, the queue. */
export const SYNCS: ReadonlySet<ServerKind> = new Set(['subsonic', 'navidrome']);

const syncKey = (id: string) => ['servers', 'sync', id] as const;

/** What a server keeps in step with Raygun, and how its last sync went. */
export const useServerSync = (id: string) =>
  useQuery({
    queryKey: syncKey(id),
    queryFn: () => unwrapResult(commands.serverSyncStatus(id)),
  });

const useSyncMutation = <Args>(id: string, run: (args: Args) => Promise<ServerSyncStatus>) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: run,
    onSuccess: (status) => queryClient.setQueryData(syncKey(id), status),
  });
};

/** Changes what a server keeps in step; lists just switched on sync at once. */
export const useSetServerSync = (id: string) =>
  useSyncMutation(id, (prefs: ServerSyncPrefs) => unwrapResult(commands.serverSyncSet(id, prefs)));

export const useSyncServerNow = (id: string) =>
  useSyncMutation(id, () => unwrapResult(commands.serverSyncNow(id)));

/** Plays the queue the server keeps; how many songs it holds. */
export const useTakeServerQueue = (id: string) =>
  useMutation({ mutationFn: () => unwrapResult(commands.serverQueueTake(id)) });
