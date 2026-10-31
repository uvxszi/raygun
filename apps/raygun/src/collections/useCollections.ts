import { commands } from '@raygun/bindings';
import type {
  CollectionPlayRequest,
  LibraryTrack,
  OnlineTrack,
  QueueItem,
  SavedTrack,
  TrackInput,
} from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';

/** Every favorites, playlists and history query lives under this key. */
export const COLLECTIONS = 'collections';

/** Recently played tracks shown on Home. */
export const HOME_HISTORY_LIMIT = 10;

export const useFavorites = () =>
  useQuery({
    queryKey: [COLLECTIONS, 'favorites'],
    queryFn: () => unwrapResult(commands.favoritesList()),
  });

/** Whether `track` is a favorite; Rust matches it by artist and title. */
export const useIsFavorite = (track: TrackInput | null) =>
  useQuery({
    queryKey: [COLLECTIONS, 'isFavorite', track?.artist ?? null, track?.title ?? null],
    queryFn: () => unwrapResult(commands.favoriteGet(track as TrackInput)),
    enabled: track !== null,
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

export const useDeletePlaylist = () =>
  useCollectionMutation((id: string) => unwrapResult(commands.playlistDelete(id)));

export const useAddToPlaylist = () =>
  useCollectionMutation(({ id, tracks }: { id: string; tracks: TrackInput[] }) =>
    unwrapResult(commands.playlistAdd(id, tracks)),
  );

export const useRemovePlaylistItem = () =>
  useCollectionMutation((itemId: string) => unwrapResult(commands.playlistRemoveItem(itemId)));

export const useMovePlaylistItem = () =>
  useCollectionMutation(
    ({ itemId, beforeItemId }: { itemId: string; beforeItemId: string | null }) =>
      unwrapResult(commands.playlistMoveItem(itemId, beforeItemId)),
  );

export const useClearHistory = () =>
  useCollectionMutation(() => unwrapResult(commands.historyClear()));

export const playCollection = (request: CollectionPlayRequest) =>
  unwrapResult(commands.collectionPlay(request));

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
