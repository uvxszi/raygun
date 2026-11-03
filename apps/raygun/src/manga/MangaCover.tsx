import { BookImage, Clapperboard } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { imageLoading } from '../shell/imageMemory';

type MangaCoverProps = {
  url: string | null | undefined;
  className?: string;
  /** Videos get a clapperboard placeholder instead of a book. */
  video?: boolean;
};

/** A cover in the 2:3 shape manga covers have, or a placeholder when it's missing or fails. */
export const MangaCover: FC<MangaCoverProps> = ({ url, className = '', video = false }) => {
  // Remembered for the address that failed: a new address (a picked cover) is tried at once.
  const [failedUrl, setFailedUrl] = useState<string | null>(null);
  const failed = failedUrl !== null && failedUrl === url;
  if (!url || failed) {
    return (
      <span
        aria-hidden
        className={`bg-surface text-muted grid aspect-[2/3] place-items-center ${className}`}
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
      className={`bg-surface aspect-[2/3] object-cover ${className}`}
    />
  );
};
