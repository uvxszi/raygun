import { commands } from '@raygun/bindings';
import type { TrackInput } from '@raygun/bindings';
import type { QueryClient } from '@tanstack/react-query';
import {
  Copy,
  Disc3,
  Download,
  Heart,
  HeartOff,
  ImageDown,
  ListEnd,
  ListMinus,
  ListPlus,
  ListStart,
  Pencil,
  Play,
  Scissors,
  Search,
  SearchCheck,
  Trash2,
  UserRound,
} from 'lucide-react';
import type { TFunction } from 'i18next';

import { useTrackActionsStore } from '../collections/trackActionsStore';
import { COLLECTIONS } from '../collections/useCollections';
import { unwrapResult } from '../lib/commandResult';
import type { MenuItem } from '../shell/contextMenuStore';
import { useSongEditStore } from '../sound/songEditStore';
import { creditedArtists } from './artists';
import { coverTitle, useCoverSheetStore } from './coverSheetStore';
import { useSourcePickerStore } from './sourcePickerStore';

/** A spot at the pointer for popovers opened from the menu (they need something to sit by). */
const pointAnchor = (x: number, y: number) => {
  let spot = document.getElementById('raygun-menu-anchor');
  if (!spot) {
    spot = document.createElement('div');
    spot.id = 'raygun-menu-anchor';
    Object.assign(spot.style, {
      position: 'fixed',
      width: '1px',
      height: '1px',
      pointerEvents: 'none',
    });
    document.body.append(spot);
  }
  Object.assign(spot.style, { left: `${x}px`, top: `${y}px` });
  return spot;
};

type TrackMenuOptions = {
  t: TFunction;
  navigate: (to: { to: string; search: Record<string, unknown> }) => void;
  client: QueryClient;
  /** Where the pointer was, for the Add to playlist popover. */
  at: { x: number; y: number };
  onPlay?: () => void;
  /** On a playlist page: the item, so it can be taken out. */
  playlistItemId?: string;
  favorite?: boolean;
  /** The song is downloaded (M46.36): the menu offers to remove it instead. */
  downloaded?: boolean;
};

/**
 * Opens the album a song is on (M46.36): a library album for a file on this device, else the
 * sources' copy of it, else a search for it.
 */
export const goToAlbum = async (
  track: TrackInput,
  navigate: TrackMenuOptions['navigate'],
): Promise<void> => {
  const album = track.album?.trim();
  if (!album) return;
  if (track.source?.kind === 'localFile') {
    navigate({ to: '/library/album', search: { album, artist: track.artist ?? '' } });
    return;
  }
  const found = await commands.musicFindAlbum(album, track.artist).catch(() => null);
  if (found?.status === 'ok' && found.data) {
    navigate({
      to: '/music/album',
      search: { provider: found.data.provider.id, id: found.data.id },
    });
    return;
  }
  const q = [album, track.artist].filter(Boolean).join(' ');
  navigate({ to: '/browse', search: { tab: 'search', type: 'music', view: 'albums', q } });
};

/** Takes a song's download out of the app (M46.36), by finding its download first. */
const removeDownload = async (track: TrackInput, client: QueryClient) => {
  const download = await unwrapResult(commands.songDownloadFor(track));
  if (download) await unwrapResult(commands.songDownloadRemove(download.id));
  void client.invalidateQueries({ queryKey: ['songs'] });
};

/**
 * A song's right-click options (M46.8): play it, play it next or last, save it, go to its
 * artists, find it, copy its name; on a playlist, take it out.
 */
export const trackMenu = (track: TrackInput, options: TrackMenuOptions): MenuItem[] => {
  const { t, navigate, client, at, onPlay, playlistItemId, favorite, downloaded } = options;
  const refresh = () => void client.invalidateQueries({ queryKey: [COLLECTIONS] });
  const name = [track.artist, track.title].filter(Boolean).join(' – ');
  const items: MenuItem[] = [{ kind: 'heading', label: track.title }];
  if (onPlay) items.push({ label: t('menu.play'), icon: Play, onSelect: onPlay });
  items.push(
    {
      label: t('menu.playNext'),
      icon: ListStart,
      onSelect: () => void unwrapResult(commands.playbackQueueTracks([track], true)),
    },
    {
      label: t('menu.addToQueue'),
      icon: ListEnd,
      onSelect: () => void unwrapResult(commands.playbackQueueTracks([track], false)),
    },
    { kind: 'separator' },
    {
      label: t('collections.addToPlaylist'),
      icon: ListPlus,
      onSelect: () =>
        useTrackActionsStore.getState().open(track, { anchor: pointAnchor(at.x, at.y) }),
    },
    favorite
      ? {
          label: t('collections.unfavorite'),
          icon: HeartOff,
          onSelect: () => void unwrapResult(commands.favoriteSet(track, false)).then(refresh),
        }
      : {
          label: t('collections.favorite'),
          icon: Heart,
          onSelect: () => void unwrapResult(commands.favoriteSet(track, true)).then(refresh),
        },
  );
  items.push(
    downloaded
      ? {
          label: t('songs.removeDownload'),
          icon: Trash2,
          onSelect: () => void removeDownload(track, client),
        }
      : {
          label: t('songs.download'),
          icon: Download,
          onSelect: () => void unwrapResult(commands.songsDownload([track])),
        },
    // The cover large, to save as an image (M46.26).
    ...(track.artworkUrl
      ? [
          {
            label: t('cover.menu'),
            icon: ImageDown,
            onSelect: () =>
              useCoverSheetStore
                .getState()
                .open({ url: track.artworkUrl as string, title: coverTitle(track) }),
          },
        ]
      : []),
    {
      label: t('sourcePicker.menu'),
      icon: SearchCheck,
      onSelect: () => useSourcePickerStore.getState().open(track),
    },
    {
      label: t('details.editSong'),
      icon: Pencil,
      onSelect: () =>
        useSongEditStore.getState().open(
          {
            title: track.title,
            artist: track.artist,
            album: track.album,
            durationMs: track.durationMs,
          },
          { tab: 'details' },
        ),
    },
    {
      label: t('menu.editSong'),
      icon: Scissors,
      onSelect: () =>
        useSongEditStore.getState().open(
          {
            title: track.title,
            artist: track.artist,
            album: track.album,
            durationMs: track.durationMs,
          },
          { anchor: pointAnchor(at.x, at.y) },
        ),
    },
  );
  if (playlistItemId) {
    items.push({
      label: t('collections.removeFromPlaylist'),
      icon: ListMinus,
      danger: true,
      onSelect: () => void unwrapResult(commands.playlistRemoveItem(playlistItemId)).then(refresh),
    });
  }
  const artists = creditedArtists(track.artist);
  if (artists.length > 0 || track.album) items.push({ kind: 'separator' });
  if (track.album?.trim()) {
    items.push({
      label: t('menu.goToAlbum', { name: track.album.trim() }),
      icon: Disc3,
      onSelect: () => void goToAlbum(track, navigate),
    });
  }
  for (const artist of artists) {
    items.push({
      label: t('menu.goToArtist', { name: artist }),
      icon: UserRound,
      onSelect: () => navigate({ to: '/artist', search: { name: artist } }),
    });
  }
  items.push(
    { kind: 'separator' },
    {
      label: t('menu.findSong'),
      icon: Search,
      onSelect: () =>
        navigate({ to: '/browse', search: { tab: 'search', type: 'music', q: name } }),
    },
    {
      label: t('menu.copyTitle'),
      icon: Copy,
      onSelect: () => void navigator.clipboard?.writeText(track.title).catch(() => undefined),
    },
  );
  if (track.artist) {
    const artist = track.artist;
    items.push({
      label: t('menu.copyArtist'),
      icon: Copy,
      onSelect: () => void navigator.clipboard?.writeText(artist).catch(() => undefined),
    });
  }
  return items;
};
