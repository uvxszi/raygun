import { commands } from '@raygun/bindings';
import type { TrackInput } from '@raygun/bindings';
import type { QueryClient } from '@tanstack/react-query';
import {
  Copy,
  Download,
  Heart,
  HeartOff,
  ListEnd,
  ListMinus,
  ListPlus,
  ListStart,
  Play,
  Scissors,
  Search,
  UserRound,
} from 'lucide-react';
import type { TFunction } from 'i18next';

import { useTrackActionsStore } from '../collections/trackActionsStore';
import { COLLECTIONS } from '../collections/useCollections';
import { unwrapResult } from '../lib/commandResult';
import type { MenuItem } from '../shell/contextMenuStore';
import { useSongEditStore } from '../sound/songEditStore';
import { creditedArtists } from './artists';

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
};

/**
 * A song's right-click options (M46.8): play it, play it next or last, save it, go to its
 * artists, find it, copy its name; on a playlist, take it out.
 */
export const trackMenu = (track: TrackInput, options: TrackMenuOptions): MenuItem[] => {
  const { t, navigate, client, at, onPlay, playlistItemId, favorite } = options;
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
    {
      label: t('songs.download'),
      icon: Download,
      onSelect: () => void unwrapResult(commands.songsDownload([track])),
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
  if (artists.length > 0) items.push({ kind: 'separator' });
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
      label: t('menu.copyName'),
      icon: Copy,
      onSelect: () => void navigator.clipboard?.writeText(name).catch(() => undefined),
    },
  );
  return items;
};
