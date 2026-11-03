import { Ellipsis, EllipsisVertical } from 'lucide-react';
import type { FC } from 'react';

import { openMenuAt } from './contextMenuStore';
import type { MenuItem } from './contextMenuStore';

type MenuButtonProps = {
  /** What it's the menu of ("More for Road trip"). */
  label: string;
  /** Asked when pressed, so nothing is worked out for rows nobody opens. */
  items: () => MenuItem[];
  /** ⋯ (in a row's heading) instead of ⋮. */
  horizontal?: boolean;
  className?: string;
  iconSize?: number;
};

/**
 * ⋮: the same menu a right click or a long press opens, for rows and headings (a sheet on phones,
 * the menu under the button elsewhere).
 */
export const MenuButton: FC<MenuButtonProps> = ({
  label,
  items,
  horizontal = false,
  className = 'text-muted hover:text-foreground hover:bg-surface grid size-9 shrink-0 place-items-center rounded-full',
  iconSize = 18,
}) => {
  const Icon = horizontal ? Ellipsis : EllipsisVertical;
  return (
    <button
      type="button"
      aria-label={label}
      aria-haspopup="menu"
      onClick={(event) => {
        event.preventDefault();
        event.stopPropagation();
        openMenuAt(event.currentTarget, items());
      }}
      className={className}
    >
      <Icon aria-hidden size={iconSize} />
    </button>
  );
};
