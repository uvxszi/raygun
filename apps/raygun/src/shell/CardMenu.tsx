import type { FC } from 'react';

import { CARD_MENU_CLASS } from '../manga/CardPlayButton';
import type { MenuItem } from './contextMenuStore';
import { MenuButton } from './MenuButton';

/**
 * A card's ⋮ on its cover's top right corner (inside a `relative` box): the card's own menu,
 * never a right click or a long press. `className` places it elsewhere (`cardMenuBeside`).
 */
export const CardMenu: FC<{ label: string; items: () => MenuItem[]; className?: string }> = ({
  label,
  items,
  className = CARD_MENU_CLASS,
}) => <MenuButton label={label} items={items} className={className} iconSize={16} />;
