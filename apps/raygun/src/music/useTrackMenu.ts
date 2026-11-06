import type { TrackInput } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';

import { savesIn, useSavedSongs } from '../collections/savedSongs';
import type { MenuItem } from '../shell/contextMenuStore';
import { useHidden, useHideActions } from './hidden';
import { keysOf } from './trackKey';
import { trackMenu } from './trackMenu';
import { useDownloadedKeys } from './useSongDownloads';

export type TrackMenuExtras = {
  onPlay?: () => void;
  playlistItemId?: string;
  favorite?: boolean;
  /** Shown in one of Home's rows: the menu offers Not interested. */
  notInterested?: boolean;
};

/** A song's menu items (M46.8), for its ⋮ menu (M46.36). */
export const useTrackMenuItems = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const client = useQueryClient();
  const downloaded = useDownloadedKeys();
  const saved = useSavedSongs();
  const hidden = useHidden();
  const hideActions = useHideActions();
  return (track: TrackInput, extras: TrackMenuExtras, at: { x: number; y: number }): MenuItem[] => {
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
