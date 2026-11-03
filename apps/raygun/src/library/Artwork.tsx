import { Disc3 } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { imageLoading } from '../shell/imageMemory';

type ArtworkProps = {
  url: string | null | undefined;
  className?: string;
  iconSize?: number;
};

/** Album art, or a placeholder when there is none or it fails to load. */
export const Artwork: FC<ArtworkProps> = ({ url, className = '', iconSize = 32 }) => {
  // Remembered for the address that failed: a new address (a picked cover) is tried at once.
  const [failedUrl, setFailedUrl] = useState<string | null>(null);
  const failed = failedUrl !== null && failedUrl === url;
  if (!url || failed) {
    return (
      <span
        aria-hidden
        className={`bg-surface text-muted grid aspect-square place-items-center ${className}`}
      >
        <Disc3 size={iconSize} />
      </span>
    );
  }
  return (
    <img
      src={url}
      data-testid="artwork-image"
      alt=""
      {...imageLoading(url)}
      onError={() => setFailedUrl(url)}
      className={`bg-surface aspect-square object-cover ${className}`}
    />
  );
};
