import { Play } from 'lucide-react';
import type { FC } from 'react';

/**
 * A card's play button, over its cover: it plays (or resumes) without opening the page the
 * rest of the card opens.
 */
export const CardPlayButton: FC<{ label: string; onPlay: () => void; className?: string }> = ({
  label,
  onPlay,
  className = 'absolute right-1.5 bottom-1.5',
}) => (
  <button
    type="button"
    aria-label={label}
    data-testid="card-play"
    onClick={(event) => {
      // Inside the card's link: only the play happens.
      event.preventDefault();
      event.stopPropagation();
      onPlay();
    }}
    className={`bg-primary/90 text-background grid size-9 place-items-center rounded-full shadow-md shadow-black/40 ${className}`}
  >
    <Play aria-hidden size={16} fill="currentColor" />
  </button>
);

/** The ⋮ button's look on a cover: a dark disc in its top right corner. */
export const CARD_MENU_CLASS =
  'absolute top-1.5 right-1.5 grid size-8 place-items-center rounded-full bg-black/55 text-white';
