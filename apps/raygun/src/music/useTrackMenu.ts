import type { TrackInput } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import type { MouseEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { savesIn, useSavedSongs } from '../collections/savedSongs';
import { openContextMenu } from '../shell/contextMenuStore';
import type { MenuItem } from '../shell/contextMenuStore';
import { useHidden, useHideActions } from './hidden';
import { keysOf } from './trackKey';
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
  const saved = useSavedSongs();
  const hidden = useHidden();
  const hideActions = useHideActions();
  return (track: TrackInput, extras: Extras, at: { x: number; y: number }): MenuItem[] => {
    // A download made under the title with or without its " - Remastered" (M46.48).
    const keys = keysOf(track.title, track.artist);
    return trackMenu(track, {
      t,
      client,
      navigate: (target) => void navigate(target as never),
      at,
      downloaded: keys.some((key) => downloaded.has(key)),
      // Under whatever names it was favorited (M46.48).
      favorite: saved.data ? savesIn(saved.data, track).favorite : false,
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
