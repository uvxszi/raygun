import { commands } from '@raygun/bindings';
import type { ServerInfo, ServerInput, ServerKind } from '@raygun/bindings';
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
  'audiobookshelf',
  'opds',
];

/** Kinds that take an API key in place of a password. */
export const TAKES_API_KEY: ReadonlySet<ServerKind> = new Set([
  'jellyfin',
  'emby',
  'subsonic',
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
