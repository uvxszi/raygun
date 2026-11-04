import { commands } from '@raygun/bindings';
import type { SavedTrack } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import {
  ArrowDownUp,
  Download,
  FolderPlus,
  ListChecks,
  ListEnd,
  ListStart,
  Play,
  RefreshCw,
  Shuffle,
  SquareArrowOutUpRight,
} from 'lucide-react';
import type { MouseEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { COLLECTIONS, fromSavedTrack, playCollection } from '../collections/useCollections';
import { unwrapResult } from '../lib/commandResult';
import { useLocalFolderActions } from '../localFiles/useLocalFolders';
import { buildMenu } from '../music/mediaMenu';
import { DOWNLOADED } from '../music/songDownloadCache';
import { useSortRequest } from '../music/sortRequest';
import { downloadSongs, useSongDownloadActions } from '../music/useSongDownloads';
import type { MenuItem } from '../shell/contextMenuStore';
import { openContextMenu } from '../shell/contextMenuStore';
import { showToast } from '../shell/toastStore';
import { usePinMenuItem } from '../shell/usePinMenuItem';
import type { PinTarget } from '../shell/useRail';
import { FIXED_PINS } from '../shell/useRail';

/** The Library's fixed blocks (M46.29): Favorites, Downloaded and Local files. */
export type FixedTile = 'favorites' | 'downloaded' | 'localFiles';

const pinOf = (tile: FixedTile): PinTarget =>
  FIXED_PINS.find((target) => target.kind === tile) ?? FIXED_PINS[0]!;

/**
 * A fixed block's menu, on its tiles and its quick access pin: play it, open it, pin it, its own
 * tools (Download all, Check downloads, Add a folder and Scan again), then Sort, which opens the
 * page with Sort by open. `pin: false` leaves out Pin/Unpin (a pin's menu has its own Unpin).
 */
export const useFixedTileMenuItems = () => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const navigate = useNavigate();
  const pinItems = usePinMenuItem();
  const { check } = useSongDownloadActions();
  const folders = useLocalFolderActions();

  return (tile: FixedTile, { pin = true }: { pin?: boolean } = {}): MenuItem[] => {
    const name =
      tile === 'favorites'
        ? t('collections.favorites')
        : tile === 'downloaded'
          ? t('library.view.downloaded')
          : t('localFiles.title');
    const open = () =>
      void (tile === 'localFiles'
        ? navigate({ to: '/local-files' })
        : navigate({ to: '/library', search: { view: tile } }));
    const songs = async () =>
      (
        await client.fetchQuery<SavedTrack[]>({
          queryKey: tile === 'favorites' ? [COLLECTIONS, 'favorites'] : [...DOWNLOADED, 'songs'],
          queryFn: () =>
            unwrapResult(
              tile === 'favorites' ? commands.favoritesList() : commands.downloadedSongs(),
            ),
        })
      ).map(fromSavedTrack);
    const failed = () => showToast(t('menu.songsFailed', { name }));
    const play = (shuffle: boolean) =>
      void playCollection({
        collection: { kind: tile === 'favorites' ? 'favorites' : 'downloaded' },
        startIndex: null,
        shuffle,
      }).catch(failed);
    const queue = (next: boolean) =>
      void songs()
        .then(async (tracks) => {
          if (tracks.length === 0) return;
          await unwrapResult(commands.playbackQueueTracks(tracks, next));
          showToast(t(next ? 'menu.queuedNext' : 'menu.queued', { count: tracks.length }));
        })
        .catch(failed);
    const music = tile !== 'localFiles';

    return buildMenu({
      heading: { label: name },
      play: music
        ? [
            { label: t('menu.play'), icon: Play, onSelect: () => play(false) },
            { label: t('collections.shuffle'), icon: Shuffle, onSelect: () => play(true) },
            { label: t('menu.playNext'), icon: ListStart, onSelect: () => queue(true) },
            { label: t('menu.addToQueue'), icon: ListEnd, onSelect: () => queue(false) },
          ]
        : [],
      goTo: [{ label: t('menu.open'), icon: SquareArrowOutUpRight, onSelect: open }],
      keep: [
        tile === 'favorites' && {
          label: t('album.download'),
          icon: Download,
          onSelect: () =>
            void songs()
              .then(async (tracks) => {
                if (tracks.length === 0) return;
                await downloadSongs(tracks);
                showToast(t('menu.downloading', { count: tracks.length }));
              })
              .catch(failed),
        },
        ...(pin ? pinItems(pinOf(tile)) : []),
      ],
      edit: [
        tile === 'downloaded' && {
          label: t('songs.check'),
          icon: ListChecks,
          onSelect: () =>
            check.mutate(null, {
              onSuccess: (checked) =>
                showToast(
                  checked.problems > 0
                    ? t('songs.checkedProblems', { count: checked.problems })
                    : t('songs.checkedFine', { count: checked.checked }),
                ),
            }),
        },
        tile === 'localFiles' && {
          label: t('localFiles.add'),
          icon: FolderPlus,
          onSelect: () =>
            folders.add.mutate(undefined, {
              onError: () => showToast(t('localFiles.addFailed')),
            }),
        },
        tile === 'localFiles' && {
          label: t('localFiles.rescan'),
          icon: RefreshCw,
          onSelect: () => folders.rescan.mutate(null),
        },
        music && {
          label: t('tracks.sortBy'),
          icon: ArrowDownUp,
          onSelect: () => {
            useSortRequest.getState().request(tile);
            open();
          },
        },
      ],
    });
  };
};

/** `onContextMenu` for a fixed block's tile. */
export const useFixedTileMenu = () => {
  const items = useFixedTileMenuItems();
  return (tile: FixedTile) => (event: MouseEvent) => openContextMenu(event, items(tile));
};
