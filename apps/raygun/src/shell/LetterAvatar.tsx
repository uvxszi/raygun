import { useState } from 'react';
import type { FC } from 'react';

const SIZES = {
  md: 'size-10 rounded-xl text-base',
  lg: 'size-16 rounded-2xl text-2xl',
};

/** A source's or extension's logo, or its first letter on a tonal tile (Mihon's source icons). */
export const LetterAvatar: FC<{
  name: string;
  logo?: string | null;
  size?: keyof typeof SIZES;
}> = ({ name, logo, size = 'md' }) => {
  const [failed, setFailed] = useState<string | null>(null);
  return logo && failed !== logo ? (
    <img
      src={logo}
      alt=""
      // Long lists (a repository's thousands of entries) fetch only the icons coming into view.
      loading="lazy"
      decoding="async"
      onError={() => setFailed(logo)}
      className={`bg-surface shrink-0 object-contain ${SIZES[size]}`}
    />
  ) : (
    <span
      aria-hidden
      className={`bg-primary/15 text-primary grid shrink-0 place-items-center font-semibold ${SIZES[size]}`}
    >
      {name.slice(0, 1).toUpperCase()}
    </span>
  );
};
