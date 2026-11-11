import { Check } from 'lucide-react';
import type { FC } from 'react';

/**
 * Where the circle sits: on a cover's top left corner (clear of the ⋮ on its top right), over
 * the artwork at a row's start, or in the row's own flow.
 */
export type CirclePlace = 'cover' | 'row' | 'inline';

const PLACES: Record<CirclePlace, string> = {
  cover: 'absolute top-1.5 left-1.5',
  row: 'absolute top-1/2 left-2 -translate-y-1/2',
  inline: '',
};

/**
 * An item's pick circle: shown on every item while picking, else on hover (or keyboard focus)
 * where there's a mouse.
 */
export const SelectCircle: FC<{
  checked: boolean;
  /** Shown whatever the pointer does (while picking). */
  shown: boolean;
  /** "Select Frieren". */
  label: string;
  onToggle: () => void;
  place?: CirclePlace;
  /** Replaces the place's position (a card padded around its cover). */
  className?: string;
}> = ({ checked, shown, label, onToggle, place = 'cover', className }) => (
  <button
    type="button"
    role="checkbox"
    aria-checked={checked}
    aria-label={label}
    data-testid="select-circle"
    onClick={(event) => {
      event.preventDefault();
      event.stopPropagation();
      onToggle();
    }}
    className={`z-10 grid size-6 shrink-0 place-items-center rounded-full border-2 transition-opacity ${
      checked
        ? 'border-primary bg-primary text-background shadow'
        : place === 'inline'
          ? 'border-muted text-transparent'
          : 'border-white/90 bg-black/40 text-transparent shadow'
    } ${
      shown || checked
        ? 'opacity-100'
        : 'pointer-events-none opacity-0 group-hover/pick:pointer-events-auto group-hover/pick:opacity-100 focus-visible:pointer-events-auto focus-visible:opacity-100'
    } ${className ?? PLACES[place]}`}
  >
    {/* Unpicked, the tick is see-through: lists of hundreds don't draw one for each. */}
    {checked && <Check aria-hidden size={14} strokeWidth={3} />}
  </button>
);
