import { Podcast } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';

/** Square podcast artwork, or a placeholder when it's missing or fails. */
export const PodcastArt: FC<{ url: string | null | undefined; className?: string }> = ({
  url,
  className = '',
}) => {
  const [failed, setFailed] = useState(false);
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
      loading="lazy"
      decoding="async"
      onError={() => setFailed(true)}
      className={`bg-surface aspect-square object-cover ${className}`}
    />
  );
};
