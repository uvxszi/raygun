import type { DetailTarget, TrackQuery } from '@raygun/bindings';
import { ListMusic } from 'lucide-react';
import type { FC } from 'react';

import { Artwork } from '../library/Artwork';
import { useNearScreen } from '../shell/useNearScreen';

type PlaylistCoverProps = {
  artworkUrls: string[];
  /**
   * The songs `artworkUrls` are the covers of, in the same order: a cover that fails to load is
   * looked for at other sources, as in song lists.
   */
  artworkSongs?: TrackQuery[] | null;
  /** The cover the user picked (M46.45), shown instead. */
  coverUrl?: string | null;
  className?: string;
};

/**
 * The user's cover, else one artwork, a 2×2 grid of four, or an icon without any. The grid's four
 * load together once it's near the screen, each falling back to another source's cover of its
 * song when its own fails.
 */
export const PlaylistCover: FC<PlaylistCoverProps> = ({
  artworkUrls,
  artworkSongs,
  coverUrl,
  className = '',
}) => {
  const [near, nearRef] = useNearScreen();
  const songOf = (index: number): DetailTarget | null => {
    const query = artworkSongs?.[index];
    return query ? { kind: 'song', query } : null;
  };
  if (coverUrl) return <Artwork url={coverUrl} className={className} />;
  if (artworkUrls.length === 0) {
    return (
      <div aria-hidden className={`bg-surface text-muted grid place-items-center ${className}`}>
        <ListMusic size={22} />
      </div>
    );
  }
  if (artworkUrls.length < 4) {
    return <Artwork url={artworkUrls[0]} fallback={songOf(0)} className={className} />;
  }
  return (
    <div
      ref={nearRef}
      aria-hidden
      data-testid="playlist-mosaic"
      className={`grid grid-cols-2 overflow-hidden ${className}`}
    >
      {artworkUrls
        .slice(0, 4)
        .map((url, index) =>
          near ? (
            <Artwork
              key={url}
              url={url}
              fallback={songOf(index)}
              eager
              className="size-full"
              iconSize={14}
            />
          ) : (
            <span key={url} className="bg-surface aspect-square size-full" />
          ),
        )}
    </div>
  );
};
