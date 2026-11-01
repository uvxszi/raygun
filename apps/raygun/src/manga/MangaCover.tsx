import { BookImage, Clapperboard } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';

type MangaCoverProps = {
  url: string | null | undefined;
  className?: string;
  /** Videos get a clapperboard placeholder instead of a book. */
  video?: boolean;
};

/** A cover in the 2:3 shape manga covers have, or a placeholder when it's missing or fails. */
export const MangaCover: FC<MangaCoverProps> = ({ url, className = '', video = false }) => {
  const [failed, setFailed] = useState(false);
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
      loading="lazy"
      decoding="async"
      onError={() => setFailed(true)}
      className={`bg-surface aspect-[2/3] object-cover ${className}`}
    />
  );
};
