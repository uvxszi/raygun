import { Podcast } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { imageLoading } from '../shell/imageMemory';

/** Square podcast artwork, or a placeholder when it's missing or fails. */
export const PodcastArt: FC<{ url: string | null | undefined; className?: string }> = ({
  url,
  className = '',
}) => {
  // Remembered for the address that failed: a new address (a picked cover) is tried at once.
  const [failedUrl, setFailedUrl] = useState<string | null>(null);
  const failed = failedUrl !== null && failedUrl === url;
  if (!url || failed) {
    return (
      <span
        aria-hidden
        className={`bg-surface text-muted grid aspect-square place-items-center ${className}`}
      >
        <Podcast size={28} />
      </span>
    );
  }
  return (
    <img
      src={url}
      alt=""
      // A long press picks a show or a book; the webview would start dragging the picture.
      draggable={false}
      {...imageLoading(url)}
      onError={() => setFailedUrl(url)}
      className={`bg-surface aspect-square object-cover ${className}`}
    />
  );
};
