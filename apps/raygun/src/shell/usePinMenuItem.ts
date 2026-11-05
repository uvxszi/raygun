import { Pin, PinOff } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import type { MenuItem } from './contextMenuStore';
import type { PinTarget } from './useRail';
import { useRail } from './useRail';

/**
 * Menu items that pin a card's page to quick access, or unpin it: pins stay on top of the
 * desktop's sidebar and the phone's drawer, which hold the same pins, so every platform gets them.
 * The audio library is listed there anyway; other pages show only while pinned.
 */
export const usePinMenuItem = () => {
  const { t } = useTranslation();
  const rail = useRail();
  return (target: PinTarget): MenuItem[] => [
    rail.isPinned(target)
      ? { label: t('rail.unpin'), icon: PinOff, onSelect: () => rail.unpin(target) }
      : { label: t('rail.pin'), icon: Pin, onSelect: () => rail.pin(target) },
  ];
};
