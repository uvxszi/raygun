import type { LibraryTrack } from '@raygun/bindings';
import { useVirtualizer } from '@tanstack/react-virtual';
import { useLayoutEffect, useRef, useState } from 'react';
import type { FC } from 'react';

import { useRootPx } from '../player/playerLayout';
import { TrackRow } from './TrackRow';

/** A row's height (TrackRow's `h-14`): rem, so it follows the screen and Interface size. */
const ROW_REM = 3.5;

/**
 * Virtualized list of every track: only rows near the viewport are rendered, so thousands of
 * tracks scroll smoothly. It scrolls with the page's main scroll container.
 */
export const TrackList: FC<{ tracks: LibraryTrack[]; onPlay: (track: LibraryTrack) => void }> = ({
  tracks,
  onPlay,
}) => {
  const listRef = useRef<HTMLDivElement>(null);
  const [scrollMargin, setScrollMargin] = useState(0);
  const rootPx = useRootPx();

  useLayoutEffect(() => {
    setScrollMargin(listRef.current?.offsetTop ?? 0);
  }, []);

  // The app doesn't use the React Compiler, so the rule's memoization concern doesn't apply.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: tracks.length,
    getScrollElement: () => listRef.current?.closest('main') ?? null,
    estimateSize: () => Math.round(ROW_REM * rootPx),
    overscan: 12,
    scrollMargin,
  });

  return (
    <div
      ref={listRef}
      data-testid="track-list"
      className="relative w-full"
      style={{ height: virtualizer.getTotalSize() }}
    >
      {virtualizer.getVirtualItems().map((row) => {
        const track = tracks[row.index];
        if (!track) {
          return null;
        }
        return (
          <div
            key={track.id}
            className="absolute top-0 left-0 w-full"
            style={{ transform: `translateY(${row.start - virtualizer.options.scrollMargin}px)` }}
          >
            <TrackRow track={track} onPlay={onPlay} />
          </div>
        );
      })}
    </div>
  );
};
