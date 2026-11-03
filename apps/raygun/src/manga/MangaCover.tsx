import { BookImage, Clapperboard } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { imageLoading } from '../shell/imageMemory';

type MangaCoverProps = {
  url: string | null | undefined;
  className?: string;
  /** Videos get a clapperboard placeholder instead of a book. */
  video?: boolean;
  /** 16:9, for video sites' thumbnails (M46.45: YouTube, PeerTube). */
  landscape?: boolean;
};

/** A cover in the 2:3 shape manga covers have, or a placeholder when it's missing or fails. */
export const MangaCover: FC<MangaCoverProps> = ({
  url,
  className = '',
  video = false,
  landscape = false,
}) => {
  const shape = landscape ? 'aspect-video' : 'aspect-[2/3]';
  // Remembered for the address that failed: a new address (a picked cover) is tried at once.
  const [failedUrl, setFailedUrl] = useState<string | null>(null);
  const failed = failedUrl !== null && failedUrl === url;
  if (!url || failed) {
    return (
      <span
        aria-hidden
        className={`bg-surface text-muted grid ${shape} place-items-center ${className}`}
      >
        {video ? <Clapperboard size={28} /> : <BookImage size={28} />}
      </span>
    );
  }
  return (
    <img
      src={url}
      alt=""
      {...imageLoading(url)}
      onError={() => setFailedUrl(url)}
      className={`bg-surface ${shape} object-cover ${className}`}
    />
  );
};
