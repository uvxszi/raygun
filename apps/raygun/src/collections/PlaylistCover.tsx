import { ListMusic } from 'lucide-react';
import type { FC } from 'react';

import { Artwork } from '../library/Artwork';

type PlaylistCoverProps = {
  artworkUrls: string[];
  /** The cover the user picked (M46.45), shown instead. */
  coverUrl?: string | null;
  className?: string;
};

/** The user's cover, else one artwork, a 2×2 grid of four, or an icon without any. */
export const PlaylistCover: FC<PlaylistCoverProps> = ({
  artworkUrls,
  coverUrl,
  className = '',
}) => {
  if (coverUrl) return <Artwork url={coverUrl} className={className} />;
  if (artworkUrls.length === 0) {
    return (
      <div aria-hidden className={`bg-surface text-muted grid place-items-center ${className}`}>
        <ListMusic size={22} />
      </div>
    );
  }
  if (artworkUrls.length < 4) {
    return <Artwork url={artworkUrls[0]} className={className} />;
  }
  return (
    <div aria-hidden className={`grid grid-cols-2 overflow-hidden ${className}`}>
      {artworkUrls.slice(0, 4).map((url) => (
        <Artwork key={url} url={url} className="size-full" iconSize={14} />
      ))}
    </div>
  );
};
