import type { TrackInput } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import type { MouseEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { useFavorites } from '../collections/useCollections';
import { openContextMenu } from '../shell/contextMenuStore';
import type { MenuItem } from '../shell/contextMenuStore';
import { useHidden, useHideActions } from './hidden';
import { trackKey } from './trackKey';
import { trackMenu } from './trackMenu';
import { useDownloadedKeys } from './useSongDownloads';

type Extras = {
  onPlay?: () => void;
  playlistItemId?: string;
  favorite?: boolean;
  /** Shown in one of Home's rows: the menu offers Not interested. */
  notInterested?: boolean;
};

/** A song's menu items (M46.8), for the right-click menu or the phone's ⋮ sheet (M46.36). */
export const useTrackMenuItems = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const client = useQueryClient();
  const downloaded = useDownloadedKeys();
  const favorites = useFavorites();
  const hidden = useHidden();
  const hideActions = useHideActions();
  return (track: TrackInput, extras: Extras, at: { x: number; y: number }): MenuItem[] => {
    const key = trackKey(track.title, track.artist);
    return trackMenu(track, {
      t,
      client,
      navigate: (target) => void navigate(target as never),
      at,
      downloaded: downloaded.has(key),
      favorite: (favorites.data ?? []).some((saved) => saved.trackKey === key),
      hidden,
      hideActions,
      ...extras,
    });
  };
};

/** `onContextMenu` for a song: its right-click options (M46.8). */
export const useTrackMenu = () => {
  const items = useTrackMenuItems();
  return (track: TrackInput, extras: Extras = {}) =>
    (event: MouseEvent) =>
      openContextMenu(event, items(track, extras, { x: event.clientX, y: event.clientY }));
};
