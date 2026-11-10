import type { FC, ReactNode } from 'react';

import { SelectCircle } from './SelectCircle';
import type { CirclePlace } from './SelectCircle';
import { usePickPress } from './usePickPress';
import type { PickState } from './usePickPress';

/**
 * A card or row that can be picked (`usePickPress`): its circle on the cover's top left (or at
 * the row's start), and a ring and tint while picked. Its ⋮ and buttons keep their own presses.
 */
export const PickTarget: FC<{
  pick: PickState;
  /** The circle's name: "Select Frieren". */
  label: string;
  place?: CirclePlace;
  /** Replaces the circle's position (a card padded around its cover). */
  circleClassName?: string;
  className?: string;
  testId?: string;
  children: ReactNode;
}> = ({ pick, label, place = 'cover', circleClassName, className = '', testId, children }) => {
  const press = usePickPress(pick);
  return (
    <div
      data-selected={pick.selected || undefined}
      data-testid={testId}
      {...press}
      className={`group/pick relative rounded-xl transition-colors [-webkit-touch-callout:none] ${
        pick.selected ? 'bg-primary/15 ring-primary ring-2' : ''
      } ${className}`}
    >
      {children}
      <SelectCircle
        checked={pick.selected}
        shown={pick.selecting}
        label={label}
        onToggle={pick.toggle}
        place={place}
        className={circleClassName}
      />
    </div>
  );
};
