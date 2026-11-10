import type { OnlineTrack } from '@raygun/bindings';
import type { FC } from 'react';

import { TrackMoreButton } from '../collections/TrackMoreButton';
import { fromOnlineTrack } from '../collections/useCollections';
import { Artwork } from '../library/Artwork';
import { ArtistLinks } from '../music/ArtistLinks';
import { formatTime } from '../player/formatTime';
import { NowPlayingBars } from '../player/NowPlayingBars';
import { usePlayingMatch } from '../player/usePlayingMatch';
import { ExplicitMark } from '../music/ExplicitMark';
import { AdultMark } from '../shell/AdultMark';
import { songCover } from '../library/coverTargets';
import { SongMarks } from '../music/SongMarks';
import { songReveal } from '../music/reveal';
import { SourceTag } from '../shell/SourceTag';
import { Selectable } from '../select/Selectable';
import { FromListingsNote } from './FromListingsNote';

type OnlineTrackRowProps = {
  track: OnlineTrack;
  onPlay: (track: OnlineTrack) => void;
  /** The source it came from, in lists that mix sources (M46.9). */
  source?: string;
  /** That source is rated for adults: an 18+ tag after its name. */
  sourceAdult?: boolean;
  /** That source's search failed or found nothing: the song came from its charts. */
  sourceListed?: boolean;
};

/** A search result. Tracks without a stream get one from the resolver when they play (M14). */
export const OnlineTrackRow: FC<OnlineTrackRowProps> = ({
  track,
  onPlay,
  source,
  sourceAdult = false,
  sourceListed = false,
}) => {
  const playing = usePlayingMatch()(track);
  return (
    <Selectable picked={{ kind: 'song', track }} layout="row">
      <div className="flex items-center" data-reveal={songReveal(track.title, track.artist)}>
        <button
          type="button"
          data-testid="online-track-row"
          // The row itself: while picking, a tap on it picks (its ⋮ keeps its menu).
          data-pick-main=""
          onClick={() => onPlay(track)}
          className={`flex min-w-0 flex-1 items-center gap-3 text-left ${source ? 'min-h-14 py-1.5' : 'h-14'}`}
        >
          <Artwork
            url={track.artworkUrl}
            fallback={songCover(track)}
            className="size-10 shrink-0 rounded"
            iconSize={18}
          />
          <span className="flex min-w-0 flex-1 flex-col gap-1.5">
            <span
              className={`flex min-w-0 items-center gap-1.5 text-sm font-medium ${playing ? 'text-primary' : ''}`}
            >
              {playing && <NowPlayingBars playing={playing === 'playing'} />}
              <span className="truncate">{track.title}</span>
              <SongMarks title={track.title} artist={track.artist} />
              {track.explicit && <ExplicitMark />}
            </span>
            <span className="text-muted block truncate text-xs">
              <ArtistLinks credit={track.artist} nested row />
              {track.album && <> · {track.album}</>}
            </span>
            {source && (
              <span className="mt-0.5 flex min-w-0 items-center gap-1">
                <SourceTag name={source} />
                {sourceAdult && <AdultMark />}
                {sourceListed && <FromListingsNote />}
              </span>
            )}
          </span>
          <span className="text-muted shrink-0 text-xs tabular-nums">
            {track.durationMs ? formatTime(track.durationMs) : ''}
          </span>
        </button>
        <TrackMoreButton track={fromOnlineTrack(track)} extras={{ onPlay: () => onPlay(track) }} />
      </div>
    </Selectable>
  );
};
