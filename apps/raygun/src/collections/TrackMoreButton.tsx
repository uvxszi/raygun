import type { TrackInput } from '@raygun/bindings';
import { EllipsisVertical } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useTrackActionsStore } from './trackActionsStore';

type TrackMoreButtonProps = {
  track: TrackInput;
  /** On a playlist page: offers removing this item. */
  playlistItemId?: string;
};

/** Opens the action sheet (favorite, add to playlist) for a track row. */
export const TrackMoreButton: FC<TrackMoreButtonProps> = ({ track, playlistItemId }) => {
  const { t } = useTranslation();
  const open = useTrackActionsStore((state) => state.open);
  return (
    <button
      type="button"
      aria-label={t('collections.moreActions', { title: track.title })}
      aria-haspopup="dialog"
      onClick={(event) => open(track, { playlistItemId, anchor: event.currentTarget })}
      className="text-muted grid size-10 shrink-0 place-items-center rounded-full"
    >
      <EllipsisVertical aria-hidden size={18} />
    </button>
  );
};
