import { commands } from '@raygun/bindings';
import type {
  CollectionPlayRequest,
  LibraryTrack,
  OnlineTrack,
  PlaylistDetail,
  QueueItem,
  SavedTrack,
  TrackInput,
  TrackSaves,
} from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';
import { rememberPlay } from '../music/playContext';
import type { TableReorder, TableTrack } from '../music/tableView';

/** Every favorites, playlists and history query lives under this key. */
export const COLLECTIONS = 'collections';

/** Recently played tracks shown on Home. */
export const HOME_HISTORY_LIMIT = 10;

export const useFavorites = () =>
  useQuery({
    queryKey: [COLLECTIONS, 'favorites'],
    queryFn: () => unwrapResult(commands.favoritesList()),
  });

export const usePlaylists = () =>
  useQuery({
    queryKey: [COLLECTIONS, 'playlists'],
    queryFn: () => unwrapResult(commands.playlistsList()),
  });

export const usePlaylist = (id: string) =>
  useQuery({
    queryKey: [COLLECTIONS, 'playlist', id],
    queryFn: () => unwrapResult(commands.playlistGet(id)),
  });

export const useHistory = (limit: number) =>
  useQuery({
    queryKey: [COLLECTIONS, 'history', limit],
    queryFn: () => unwrapResult(commands.historyRecent(limit)),
  });

/** A mutation that refreshes every collection query once it lands. */
const useCollectionMutation = <Variables, Result>(
  run: (variables: Variables) => Promise<Result>,
) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: run,
    onSettled: () => queryClient.invalidateQueries({ queryKey: [COLLECTIONS] }),
  });
};

export const useSetFavorite = () =>
  useCollectionMutation(({ track, favorite }: { track: TrackInput; favorite: boolean }) =>
    unwrapResult(commands.favoriteSet(track, favorite)),
  );

export const useCreatePlaylist = () =>
  useCollectionMutation((name: string) => unwrapResult(commands.playlistCreate(name)));

export const useRenamePlaylist = () =>
  useCollectionMutation(({ id, name }: { id: string; name: string }) =>
    unwrapResult(commands.playlistRename(id, name)),
  );

/**
 * Deletes a playlist, then runs `after` (leaving its page) before lists refresh: refreshing first
 * would fetch the deleted playlist again, with retries, and hold the page for seconds.
 */
export const useDeletePlaylist = (after?: () => unknown) =>
  useCollectionMutation(async (id: string) => {
    await unwrapResult(commands.playlistDelete(id));
    await after?.();
  });

/**
 * Adds songs to a playlist. Songs it already holds stay out unless `duplicates` (Add anyway,
 * M46.48); the answer says how many were added and how many it had already.
 */
export const useAddToPlaylist = () =>
  useCollectionMutation(
    ({ id, tracks, duplicates }: { id: string; tracks: TrackInput[]; duplicates?: boolean }) =>
      unwrapResult(commands.playlistAdd(id, tracks, duplicates ?? false)),
  );

export const useRemovePlaylistItem = () =>
  useCollectionMutation((itemId: string) => unwrapResult(commands.playlistRemoveItem(itemId)));

export const useMovePlaylistItem = () =>
  useCollectionMutation(
    ({ itemId, beforeItemId }: { itemId: string; beforeItemId: string | null }) =>
      unwrapResult(commands.playlistMoveItem(itemId, beforeItemId)),
  );

/**
 * Keeps a dragged order (M46.44): shown at once in the cached list, then saved as one move, or
 * as the whole order when the table was sorted.
 */
const useReorder = <Data>(
  key: readonly unknown[],
  save: (change: TableReorder) => Promise<unknown>,
  reorder: (data: Data, order: TableTrack[]) => Data,
) => {
  const client = useQueryClient();
  return useMutation({
    mutationFn: save,
    onMutate: async (change: TableReorder) => {
      await client.cancelQueries({ queryKey: key });
      client.setQueryData<Data>(key, (data) => (data ? reorder(data, change.order) : data));
    },
    onSettled: () => client.invalidateQueries({ queryKey: [COLLECTIONS] }),
  });
};

/** Picks `items` in the order of `order`, matched by `keyOf` and the rows' keys. */
const inOrder = <Item>(items: Item[], order: TableTrack[], keyOf: (item: Item) => string) => {
  const byKey = new Map(items.map((item) => [keyOf(item), item]));
  const placed = order.flatMap((row) => {
    const item = byKey.get(row.playlistItemId ?? row.trackKey);
    return item ? [item] : [];
  });
  return placed.length === items.length ? placed : items;
};

export const useReorderPlaylist = (id: string) =>
  useReorder<PlaylistDetail>(
    [COLLECTIONS, 'playlist', id],
    ({ order, moved, before, whole }) =>
      unwrapResult(
        whole
          ? commands.playlistSetOrder(
              id,
              order.flatMap((row) => (row.playlistItemId ? [row.playlistItemId] : [])),
            )
          : commands.playlistMoveItem(moved.playlistItemId ?? '', before?.playlistItemId ?? null),
      ),
    (detail, order) => ({ ...detail, items: inOrder(detail.items, order, (item) => item.id) }),
  );

export const useReorderFavorites = () =>
  useReorder<SavedTrack[]>(
    [COLLECTIONS, 'favorites'],
    ({ order, moved, before, whole }) =>
      unwrapResult(
        whole
          ? commands.favoritesSetOrder(order.map((row) => row.trackKey))
          : commands.favoriteMove(moved.trackKey, before?.trackKey ?? null),
      ),
    (favorites, order) => inOrder(favorites, order, (track) => track.trackKey),
  );

export const useClearHistory = () =>
  useCollectionMutation(() => unwrapResult(commands.historyClear()));

export const playCollection = async (request: CollectionPlayRequest) => {
  await rememberPlay();
  return unwrapResult(commands.collectionPlay(request));
};

// ---- Tracks from anywhere, as something to save ----

export const fromOnlineTrack = (track: OnlineTrack): TrackInput => ({
  title: track.title,
  artist: track.artist,
  album: track.album,
  durationMs: track.durationMs,
  artworkUrl: track.artworkUrl,
  source: track.source,
});

/** Local tracks are saved without their file; Rust finds them again by artist and title. */
export const fromLibraryTrack = (track: LibraryTrack): TrackInput => ({
  title: track.title,
  artist: track.artist,
  album: track.album,
  durationMs: track.durationMs,
  artworkUrl: track.artworkUrl,
  source: null,
});

export const fromQueueItem = (item: QueueItem): TrackInput => ({
  title: item.title,
  artist: item.artist,
  album: item.album,
  durationMs: item.durationMs,
  artworkUrl: item.artworkUrl,
  source: item.source,
});

export const fromSavedTrack = (track: SavedTrack): TrackInput => ({
  title: track.title,
  artist: track.artist,
  album: track.album,
  durationMs: track.durationMs,
  artworkUrl: track.artworkUrl,
  source: track.source,
});

// ---- Where a song is saved (M46.8; read with `savedSongs.ts`'s `useSongSaves`, M46.48) ----

export const useSetTrackSaves = () =>
  useCollectionMutation(({ track, saves }: { track: TrackInput; saves: TrackSaves }) =>
    unwrapResult(commands.collectionsSetSaves(track, saves)),
  );
