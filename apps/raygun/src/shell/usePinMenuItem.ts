import { Pin, PinOff } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import type { MenuItem } from './contextMenuStore';
import { useIsDesktop } from './useIsDesktop';
import type { PinTarget } from './useRail';
import { useRail } from './useRail';

/** Right-click menu items that pin a card's page to the sidebar, or unpin it (desktop). */
export const usePinMenuItem = () => {
  const { t } = useTranslation();
  const desktop = useIsDesktop();
  const rail = useRail();
  return (target: PinTarget): MenuItem[] =>
    desktop
      ? [
          rail.isPinned(target)
            ? { label: t('rail.unpin'), icon: PinOff, onSelect: () => rail.unpin(target) }
            : { label: t('rail.pin'), icon: Pin, onSelect: () => rail.pin(target) },
        ]
      : [];
};
