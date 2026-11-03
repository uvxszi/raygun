import { commands } from '@raygun/bindings';
import type { PlaylistSummary } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate, useRouter } from '@tanstack/react-router';
import {
  Copy,
  Download,
  ListEnd,
  ListPlus,
  ListStart,
  Merge,
  Play,
  Shuffle,
  Trash2,
} from 'lucide-react';
import type { MouseEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { buildMenu, copyText } from '../music/mediaMenu';
import { useMenuSheetsStore } from '../music/menuSheetsStore';
import { downloadSongs } from '../music/useSongDownloads';
import type { MenuItem } from '../shell/contextMenuStore';
import { openContextMenu } from '../shell/contextMenuStore';
import { showToast } from '../shell/toastStore';
import { usePinMenuItem } from '../shell/usePinMenuItem';
import { playlistPin } from '../shell/useRail';
import { useCollectionSheets } from './collectionSheets';
import { COLLECTIONS, fromSavedTrack, playCollection, useDeletePlaylist } from './useCollections';

/**
 * A playlist's right-click menu (M46.45), in the order every music menu keeps: play, shuffle,
 * play next or last; add its songs to another playlist; download them; pin; copy its name;
 * merge; and delete after a question in the menu's place. Favorites, Downloaded and Local files
 * aren't playlists, so they never get it.
 */
export const usePlaylistMenuItems = () => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const pinItems = usePinMenuItem();
  const router = useRouter();
  const navigate = useNavigate();
  const remove = useDeletePlaylist();
  const openMerge = useCollectionSheets((state) => state.openMerge);
  return (
    playlist: Pick<PlaylistSummary, 'id' | 'name' | 'artworkUrls'>,
    { pin = true }: { pin?: boolean } = {},
  ): MenuItem[] => {
    const play = (shuffle: boolean) =>
      void playCollection({
        collection: { kind: 'playlist', id: playlist.id },
        startIndex: null,
        shuffle,
      });
    const tracks = async () =>
      (
        await client.fetchQuery({
          queryKey: [COLLECTIONS, 'playlist', playlist.id],
          queryFn: () => unwrapResult(commands.playlistGet(playlist.id)),
        })
      ).items.map((item) => fromSavedTrack(item.track));
    const queue = (next: boolean) =>
      void tracks()
        .then(async (songs) => {
          await unwrapResult(commands.playbackQueueTracks(songs, next));
          showToast(t(next ? 'menu.queuedNext' : 'menu.queued', { count: songs.length }));
        })
        .catch(() => showToast(t('menu.songsFailed', { name: playlist.name })));
    return buildMenu({
      heading: { label: playlist.name, artworkUrl: playlist.artworkUrls[0] ?? null },
      play: [
        { label: t('menu.play'), icon: Play, onSelect: () => play(false) },
        { label: t('collections.shuffle'), icon: Shuffle, onSelect: () => play(true) },
        { label: t('menu.playNext'), icon: ListStart, onSelect: () => queue(true) },
        { label: t('menu.addToQueue'), icon: ListEnd, onSelect: () => queue(false) },
      ],
      save: [
        {
          label: t('collections.addToPlaylist'),
          icon: ListPlus,
          onSelect: () =>
            useMenuSheetsStore.getState().addToPlaylist({ title: playlist.name, load: tracks }),
        },
      ],
      keep: [
        {
          label: t('songs.download'),
          icon: Download,
          onSelect: () =>
            void tracks()
              .then(async (songs) => {
                await downloadSongs(songs);
                showToast(t('menu.downloading', { count: songs.length }));
              })
              .catch(() => showToast(t('menu.songsFailed', { name: playlist.name }))),
        },
        ...(pin ? pinItems(playlistPin(playlist)) : []),
      ],
      share: [{ label: t('menu.copyName'), icon: Copy, onSelect: () => copyText(playlist.name) }],
      edit: [
        {
          label: t('collections.merge.menu'),
          icon: Merge,
          onSelect: () => openMerge([playlist.id]),
        },
      ],
      remove: [
        {
          label: t('collections.delete'),
          icon: Trash2,
          danger: true,
          confirm: t('collections.deleteConfirm', { name: playlist.name }),
          onSelect: () => {
            const here = router.state.location;
            const open =
              here.pathname === '/library/playlist' &&
              (here.search as { id?: string }).id === playlist.id;
            remove.mutate(playlist.id, {
              onSuccess: () => {
                if (open) void navigate({ to: '/library', search: { view: 'playlists' } });
              },
            });
          },
        },
      ],
    });
  };
};

/** `onContextMenu` for a playlist's card or row. */
export const usePlaylistMenu = () => {
  const items = usePlaylistMenuItems();
  return (playlist: Pick<PlaylistSummary, 'id' | 'name' | 'artworkUrls'>) => (event: MouseEvent) =>
    openContextMenu(event, items(playlist));
};
