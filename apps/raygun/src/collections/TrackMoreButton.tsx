import type { TrackInput } from '@raygun/bindings';
import { EllipsisVertical } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import type { TrackMenuExtras } from '../music/useTrackMenu';
import { useTrackActionsStore } from './trackActionsStore';

type TrackMoreButtonProps = {
  track: TrackInput;
  /** On a playlist page: offers removing this item. */
  playlistItemId?: string;
  /** What the row adds to the song's menu (play from here, Not interested…). */
  extras?: TrackMenuExtras;
  className?: string;
  iconSize?: number;
};

/** ⋮ on a song's row: favorite, add to a playlist, then the rest of the song's menu. */
export const TrackMoreButton: FC<TrackMoreButtonProps> = ({
  track,
  playlistItemId,
  extras,
  className = 'text-muted grid size-10 shrink-0 place-items-center rounded-full',
  iconSize = 18,
}) => {
  const { t } = useTranslation();
  const open = useTrackActionsStore((state) => state.open);
  return (
    <button
      type="button"
      aria-label={t('collections.moreActions', { title: track.title })}
      aria-haspopup="dialog"
      onClick={(event) => {
        // On a card's cover: only the menu opens.
        event.preventDefault();
        event.stopPropagation();
        open(track, { playlistItemId, anchor: event.currentTarget, extras });
      }}
      className={className}
    >
      <EllipsisVertical aria-hidden size={iconSize} />
    </button>
  );
};
