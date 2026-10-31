import type { LibraryTrack } from '@raygun/bindings';
import { useVirtualizer } from '@tanstack/react-virtual';
import { useLayoutEffect, useRef, useState } from 'react';
import type { FC } from 'react';

import { TrackRow } from './TrackRow';

const ROW_HEIGHT = 56;

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

  useLayoutEffect(() => {
    setScrollMargin(listRef.current?.offsetTop ?? 0);
  }, []);

  // The app doesn't use the React Compiler, so the rule's memoization concern doesn't apply.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: tracks.length,
    getScrollElement: () => listRef.current?.closest('main') ?? null,
    estimateSize: () => ROW_HEIGHT,
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
