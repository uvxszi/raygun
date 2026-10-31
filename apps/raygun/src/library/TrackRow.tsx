import type { LibraryTrack } from '@raygun/bindings';
import type { FC } from 'react';

import { TrackMoreButton } from '../collections/TrackMoreButton';
import { fromLibraryTrack } from '../collections/useCollections';
import { formatTime } from '../player/formatTime';
import { Artwork } from './Artwork';

type TrackRowProps = {
  track: LibraryTrack;
  onPlay: (track: LibraryTrack) => void;
  /** Album pages show track numbers instead of artwork. */
  variant?: 'artwork' | 'number';
};

export const TrackRow: FC<TrackRowProps> = ({ track, onPlay, variant = 'artwork' }) => (
  <div className="flex items-center">
    <button
      type="button"
      data-testid="track-row"
      onClick={() => onPlay(track)}
      className="flex h-14 min-w-0 flex-1 items-center gap-3 text-left"
    >
      {variant === 'artwork' ? (
        <Artwork url={track.artworkUrl} className="size-10 shrink-0 rounded" iconSize={18} />
      ) : (
        <span className="text-muted w-6 shrink-0 text-right text-sm tabular-nums">
          {track.trackNo ?? ''}
        </span>
      )}
      <span className="flex min-w-0 flex-1 flex-col">
        <span className="truncate text-sm font-medium">{track.title}</span>
        <span className="text-muted truncate text-xs">
          {variant === 'artwork' ? `${track.artist} · ${track.album}` : track.artist}
        </span>
      </span>
      <span className="text-muted shrink-0 text-xs tabular-nums">
        {track.durationMs ? formatTime(track.durationMs) : ''}
      </span>
    </button>
    <TrackMoreButton track={fromLibraryTrack(track)} />
  </div>
);
