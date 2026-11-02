import type { SavedTrack } from '@raygun/bindings';
import type { FC, ReactNode } from 'react';

import { Artwork } from '../library/Artwork';
import { ArtistLinks } from '../music/ArtistLinks';
import { useTrackMenu } from '../music/useTrackMenu';
import { formatTime } from '../player/formatTime';
import { TrackMoreButton } from './TrackMoreButton';
import { fromSavedTrack } from './useCollections';

type SavedTrackRowProps = {
  track: SavedTrack;
  onPlay: () => void;
  /** On a playlist page: lets the action sheet remove this item. */
  playlistItemId?: string;
  /** Replaces the more button, e.g. with reorder controls while editing. */
  actions?: ReactNode;
};

/** A favorite, playlist item or history entry. */
export const SavedTrackRow: FC<SavedTrackRowProps> = ({
  track,
  onPlay,
  playlistItemId,
  actions,
}) => {
  const menu = useTrackMenu();
  return (
    <div
      className="flex items-center"
      onContextMenu={menu(fromSavedTrack(track), { onPlay, playlistItemId })}
    >
      <button
        type="button"
        data-testid="saved-track-row"
        onClick={onPlay}
        className="flex h-14 min-w-0 flex-1 items-center gap-3 text-left"
      >
        <Artwork url={track.artworkUrl} className="size-10 shrink-0 rounded" iconSize={18} />
        <span className="flex min-w-0 flex-1 flex-col">
          <span className="truncate text-sm font-medium">{track.title}</span>
          <span className="text-muted block truncate text-xs">
            <ArtistLinks credit={track.artist} nested />
            {track.album && <> · {track.album}</>}
          </span>
        </span>
        <span className="text-muted shrink-0 text-xs tabular-nums">
          {track.durationMs ? formatTime(track.durationMs) : ''}
        </span>
      </button>
      {actions ?? <TrackMoreButton track={fromSavedTrack(track)} playlistItemId={playlistItemId} />}
    </div>
  );
};
