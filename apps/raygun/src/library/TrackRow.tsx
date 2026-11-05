import type { LibraryTrack } from '@raygun/bindings';
import type { FC } from 'react';

import { TrackMoreButton } from '../collections/TrackMoreButton';
import { fromLibraryTrack } from '../collections/useCollections';
import { formatTime } from '../player/formatTime';
import { ArtistLinks } from '../music/ArtistLinks';
import { Artwork } from './Artwork';
import { NowPlayingBars } from '../player/NowPlayingBars';
import { usePlayingMatch } from '../player/usePlayingMatch';
import { songCover } from './coverTargets';
import { SongMarks } from '../music/SongMarks';
import { songReveal } from '../music/reveal';

type TrackRowProps = {
  track: LibraryTrack;
  onPlay: (track: LibraryTrack) => void;
  /** Album pages show track numbers instead of artwork. */
  variant?: 'artwork' | 'number';
};

export const TrackRow: FC<TrackRowProps> = ({ track, onPlay, variant = 'artwork' }) => {
  const playing = usePlayingMatch()(track);
  return (
    <div className="flex items-center" data-reveal={songReveal(track.title, track.artist)}>
      <button
        type="button"
        data-testid="track-row"
        onClick={() => onPlay(track)}
        className="flex h-14 min-w-0 flex-1 items-center gap-3 text-left"
      >
        {variant === 'artwork' ? (
          <Artwork
            url={track.artworkUrl}
            fallback={songCover(track)}
            className="size-10 shrink-0 rounded"
            iconSize={18}
          />
        ) : (
          <span className="text-muted w-6 shrink-0 text-right text-sm tabular-nums">
            {track.trackNo ?? ''}
          </span>
        )}
        <span className="flex min-w-0 flex-1 flex-col gap-1.5">
          <span
            className={`flex min-w-0 items-center gap-1.5 text-sm font-medium ${playing ? 'text-primary' : ''}`}
          >
            {playing && <NowPlayingBars playing={playing === 'playing'} />}
            <span className="truncate">{track.title}</span>
            <SongMarks title={track.title} artist={track.artist} />
          </span>
          <span className="text-muted block truncate text-xs">
            <ArtistLinks credit={track.artist} nested row />
            {variant === 'artwork' && <> · {track.album}</>}
          </span>
        </span>
        <span className="text-muted shrink-0 text-xs tabular-nums">
          {track.durationMs ? formatTime(track.durationMs) : ''}
        </span>
      </button>
      <TrackMoreButton track={fromLibraryTrack(track)} extras={{ onPlay: () => onPlay(track) }} />
    </div>
  );
};
