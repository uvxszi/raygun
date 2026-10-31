import { ListMusic } from 'lucide-react';
import type { FC } from 'react';

import { Artwork } from '../library/Artwork';

type PlaylistCoverProps = {
  artworkUrls: string[];
  className?: string;
};

/** One artwork, a 2×2 grid of four, or an icon for a playlist without any. */
export const PlaylistCover: FC<PlaylistCoverProps> = ({ artworkUrls, className = '' }) => {
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
        <img key={url} alt="" src={url} className="size-full object-cover" />
      ))}
    </div>
  );
};
