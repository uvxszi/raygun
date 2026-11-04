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
  Link2,
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
import { hideArtistItem, notInterestedItem, songTarget } from './hidden';
import type { HiddenFilter, HideActions } from './hidden';
import { buildMenu, copyText } from './mediaMenu';
import { useSourcePickerStore } from './sourcePickerStore';

/** A spot at the pointer for popovers opened from the menu (they need something to sit by). */
export const pointAnchor = (x: number, y: number) => {
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
  /** What's hidden, and how to hide: Don't show this artist (and Not interested in Home's rows). */
  hidden?: HiddenFilter;
  hideActions?: HideActions;
  /** Shown in one of Home's rows: Not interested takes it out of them. */
  notInterested?: boolean;
};

/** The song's own page on the web, when its source has one. */
export const songLink = (track: TrackInput): string | null =>
  track.source?.kind === 'youTube'
    ? `https://www.youtube.com/watch?v=${encodeURIComponent(track.source.videoId)}`
    : null;

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
 * A song's menu (M46.8), in the order every music menu keeps (`buildMenu`): play it, play it
 * next or last; add it to a playlist or Favorites; go to its album or artists; download it; copy
 * its link or name; edit it; don't show its artist, or (in Home's rows) say Not interested.
 */
export const trackMenu = (track: TrackInput, options: TrackMenuOptions): MenuItem[] => {
  const { t, navigate, client, at, onPlay, playlistItemId, favorite, downloaded } = options;
  const refresh = () => void client.invalidateQueries({ queryKey: [COLLECTIONS] });
  const name = [track.artist, track.title].filter(Boolean).join(' – ');
  const artists = creditedArtists(track.artist);
  const link = songLink(track);
  const query = {
    title: track.title,
    artist: track.artist,
    album: track.album,
    durationMs: track.durationMs,
  };
  const { hidden, hideActions } = options;
  return buildMenu({
    heading: { label: track.title, subtitle: track.artist, artworkUrl: track.artworkUrl },
    play: [
      onPlay && { label: t('menu.play'), icon: Play, onSelect: onPlay },
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
    ],
    save: [
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
      playlistItemId && {
        label: t('collections.removeFromPlaylist'),
        icon: ListMinus,
        danger: true,
        onSelect: () =>
          void unwrapResult(commands.playlistRemoveItem(playlistItemId)).then(refresh),
      },
    ],
    goTo: [
      Boolean(track.album?.trim()) && {
        label: t('menu.goToAlbum'),
        icon: Disc3,
        onSelect: () => void goToAlbum(track, navigate),
      },
      ...artists.map((artist) => ({
        label: t('menu.goToArtist', { name: artist }),
        icon: UserRound,
        onSelect: () => navigate({ to: '/artist', search: { name: artist } }),
      })),
      {
        label: t('menu.findSong'),
        icon: Search,
        onSelect: () =>
          navigate({ to: '/browse', search: { tab: 'search', type: 'music', q: name } }),
      },
    ],
    keep: [
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
    ],
    share: [
      link && { label: t('menu.copySongLink'), icon: Link2, onSelect: () => copyText(link) },
      { label: t('menu.copyTitle'), icon: Copy, onSelect: () => copyText(track.title) },
      track.artist && {
        label: t('menu.copyArtist'),
        icon: Copy,
        onSelect: () => copyText(track.artist as string),
      },
    ],
    edit: [
      {
        label: t('details.editSong'),
        icon: Pencil,
        onSelect: () => useSongEditStore.getState().open(query, { tab: 'details' }),
      },
      {
        label: t('menu.editSong'),
        icon: Scissors,
        onSelect: () =>
          useSongEditStore.getState().open(query, { anchor: pointAnchor(at.x, at.y) }),
      },
      {
        label: t('sourcePicker.menu'),
        icon: SearchCheck,
        onSelect: () => useSourcePickerStore.getState().open(track),
      },
      // The cover large, to save as an image (M46.26).
      track.artworkUrl && {
        label: t('cover.menu'),
        icon: ImageDown,
        onSelect: () =>
          useCoverSheetStore
            .getState()
            .open({ url: track.artworkUrl as string, title: coverTitle(track) }),
      },
    ],
    hide:
      hidden && hideActions
        ? [
            artists[0] && hideArtistItem(artists[0], hidden, hideActions, t),
            options.notInterested && notInterestedItem(songTarget(track), hideActions, t),
          ]
        : [],
  });
};
