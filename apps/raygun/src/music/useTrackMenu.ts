import type { TrackInput } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import type { MouseEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { openContextMenu } from '../shell/contextMenuStore';
import { trackMenu } from './trackMenu';

type Extras = { onPlay?: () => void; playlistItemId?: string; favorite?: boolean };

/** `onContextMenu` for a song: its right-click options (M46.8). */
export const useTrackMenu = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const client = useQueryClient();
  return (track: TrackInput, extras: Extras = {}) =>
    (event: MouseEvent) =>
      openContextMenu(
        event,
        trackMenu(track, {
          t,
          client,
          navigate: (target) => void navigate(target as never),
          at: { x: event.clientX, y: event.clientY },
          ...extras,
        }),
      );
};
