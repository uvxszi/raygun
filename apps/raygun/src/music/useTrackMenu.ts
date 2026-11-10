import type { TrackInput } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';

import { savesIn, useSavedSongs } from '../collections/savedSongs';
import type { MenuItem } from '../shell/contextMenuStore';
import { creditedArtists } from './artists';
import { useIsFollowed } from './follows';
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
  const followed = useIsFollowed();
  return (track: TrackInput, extras: TrackMenuExtras, at: { x: number; y: number }): MenuItem[] => {
    // A download made under the title with or without its " - Remastered" (M46.48).
    const keys = keysOf(track.title, track.artist);
    // Under whatever names it was saved (M46.48).
    const saves = saved.data ? savesIn(saved.data, track) : null;
    return trackMenu(track, {
      t,
      client,
      navigate: (target) => void navigate(target as never),
      at,
      downloaded: keys.some((key) => downloaded.has(key)),
      favorite: saves?.favorite ?? false,
      inPlaylist: (saves?.playlistIds.length ?? 0) > 0,
      artistFollowed: creditedArtists(track.artist).some(followed),
      hidden,
      hideActions,
      ...extras,
    });
  };
};
