import type { OnlineTrack } from '@raygun/bindings';
import type { FC } from 'react';

import { Artwork } from '../library/Artwork';
import { formatTime } from '../player/formatTime';

type OnlineTrackRowProps = {
  track: OnlineTrack;
  onPlay: (track: OnlineTrack) => void;
};

/** A search result. Tracks without a stream get one from the resolver when they play (M14). */
export const OnlineTrackRow: FC<OnlineTrackRowProps> = ({ track, onPlay }) => (
  <button
    type="button"
    data-testid="online-track-row"
    onClick={() => onPlay(track)}
    className="flex h-14 w-full items-center gap-3 text-left"
  >
    <Artwork url={track.artworkUrl} className="size-10 shrink-0 rounded" iconSize={18} />
    <span className="flex min-w-0 flex-1 flex-col">
      <span className="truncate text-sm font-medium">{track.title}</span>
      <span className="text-muted truncate text-xs">
        {[track.artist, track.album].filter(Boolean).join(' · ')}
      </span>
    </span>
    <span className="text-muted shrink-0 text-xs tabular-nums">
      {track.durationMs ? formatTime(track.durationMs) : ''}
    </span>
  </button>
);
