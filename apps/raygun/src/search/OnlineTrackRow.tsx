import type { OnlineTrack } from '@raygun/bindings';
import type { FC } from 'react';

import { TrackMoreButton } from '../collections/TrackMoreButton';
import { fromOnlineTrack } from '../collections/useCollections';
import { Artwork } from '../library/Artwork';
import { ArtistLinks } from '../music/ArtistLinks';
import { useTrackMenu } from '../music/useTrackMenu';
import { formatTime } from '../player/formatTime';

type OnlineTrackRowProps = {
  track: OnlineTrack;
  onPlay: (track: OnlineTrack) => void;
  /** The source it came from, in lists that mix sources (M46.9). */
  source?: string;
};

/** A search result. Tracks without a stream get one from the resolver when they play (M14). */
export const OnlineTrackRow: FC<OnlineTrackRowProps> = ({ track, onPlay, source }) => {
  const menu = useTrackMenu();
  return (
    <div
      className="flex items-center"
      onContextMenu={menu(fromOnlineTrack(track), { onPlay: () => onPlay(track) })}
    >
      <button
        type="button"
        data-testid="online-track-row"
        onClick={() => onPlay(track)}
        className="flex h-14 min-w-0 flex-1 items-center gap-3 text-left"
      >
        <Artwork url={track.artworkUrl} className="size-10 shrink-0 rounded" iconSize={18} />
        <span className="flex min-w-0 flex-1 flex-col">
          <span className="truncate text-sm font-medium">{track.title}</span>
          <span className="text-muted block truncate text-xs">
            <ArtistLinks credit={track.artist} nested />
            {track.album && <> · {track.album}</>}
            {source && <span className="text-primary/80"> · {source}</span>}
          </span>
        </span>
        <span className="text-muted shrink-0 text-xs tabular-nums">
          {track.durationMs ? formatTime(track.durationMs) : ''}
        </span>
      </button>
      <TrackMoreButton track={fromOnlineTrack(track)} />
    </div>
  );
};
