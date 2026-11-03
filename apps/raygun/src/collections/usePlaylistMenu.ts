import type { PlaylistSummary } from '@raygun/bindings';
import { useNavigate, useRouter } from '@tanstack/react-router';
import { Merge, Play, Shuffle, Trash2 } from 'lucide-react';
import type { MouseEvent } from 'react';
import { useTranslation } from 'react-i18next';

import type { MenuItem } from '../shell/contextMenuStore';
import { openContextMenu } from '../shell/contextMenuStore';
import { usePinMenuItem } from '../shell/usePinMenuItem';
import { playlistPin } from '../shell/useRail';
import { useCollectionSheets } from './collectionSheets';
import { playCollection, useDeletePlaylist } from './useCollections';

/**
 * A playlist's right-click menu (M46.45): play, shuffle, pin, merge, and delete after a question
 * in the menu's place. Favorites, Downloaded and Local files aren't playlists, so they never get it.
 */
export const usePlaylistMenuItems = () => {
  const { t } = useTranslation();
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
    return [
      { kind: 'heading', label: playlist.name },
      { label: t('menu.play'), icon: Play, onSelect: () => play(false) },
      { label: t('collections.shuffle'), icon: Shuffle, onSelect: () => play(true) },
      ...(pin ? pinItems(playlistPin(playlist)) : []),
      {
        label: t('collections.merge.menu'),
        icon: Merge,
        onSelect: () => openMerge([playlist.id]),
      },
      { kind: 'separator' },
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
    ];
  };
};

/** `onContextMenu` for a playlist's card or row. */
export const usePlaylistMenu = () => {
  const items = usePlaylistMenuItems();
  return (playlist: Pick<PlaylistSummary, 'id' | 'name' | 'artworkUrls'>) => (event: MouseEvent) =>
    openContextMenu(event, items(playlist));
};
