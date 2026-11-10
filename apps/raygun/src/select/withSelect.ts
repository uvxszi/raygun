import type { TFunction } from 'i18next';
import { CheckSquare } from 'lucide-react';

import type { MenuItem } from '../shell/contextMenuStore';

/**
 * An item's ⋮ menu with Select first (after its heading), in lists whose items can be picked;
 * `onSelect` is left out once the item is picked.
 */
export const withSelect = (
  items: MenuItem[],
  t: TFunction,
  onSelect: (() => void) | null,
): MenuItem[] => {
  if (!onSelect) return items;
  const first = items.findIndex((item) => item.kind !== 'heading');
  const at = first < 0 ? items.length : first;
  return [
    ...items.slice(0, at),
    { label: t('menu.select'), icon: CheckSquare, onSelect },
    { kind: 'separator' },
    ...items.slice(at),
  ];
};
