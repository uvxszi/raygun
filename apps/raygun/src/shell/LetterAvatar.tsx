import type { FC } from 'react';

import { Picture } from './Picture';

const SIZES = {
  md: 'size-10 rounded-xl text-base',
  lg: 'size-16 rounded-2xl text-2xl',
};

/**
 * A source's or extension's logo, fading in on its tile, or its first letter on a tonal tile
 * (Mihon's source icons). Long lists (a repository's thousands of entries) fetch only the icons
 * coming into view.
 */
export const LetterAvatar: FC<{
  name: string;
  logo?: string | null;
  size?: keyof typeof SIZES;
}> = ({ name, logo, size = 'md' }) => (
  <Picture
    src={logo}
    fit="bg-surface object-contain"
    placeholder={
      <span className="bg-primary/15 text-primary grid size-full place-items-center font-semibold">
        {name.slice(0, 1).toUpperCase()}
      </span>
    }
    className={`shrink-0 ${SIZES[size]}`}
  />
);
