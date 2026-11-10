import { UserRound, UsersRound } from 'lucide-react';
import type { TFunction } from 'i18next';

import type { MenuItem } from '../shell/contextMenuStore';
import { useMenuSheetsStore } from './menuSheetsStore';

/**
 * One item for a song's or an album's artists: Go to artist opens the one artist's page; Go to
 * artists, for several, shows their pictures to pick from. `anchor` gives where the menu was, so
 * the pictures pop up there on wide windows. Nothing for a credit without a name.
 */
export const goToArtistsItem = (
  t: TFunction,
  names: string[],
  open: (name: string) => void,
  anchor?: () => HTMLElement | null,
): MenuItem | false => {
  const [only] = names;
  if (only === undefined) return false;
  if (names.length === 1) {
    return { label: t('menu.goToTheArtist'), icon: UserRound, onSelect: () => open(only) };
  }
  return {
    label: t('menu.goToArtists'),
    icon: UsersRound,
    onSelect: () => useMenuSheetsStore.getState().pickArtist({ names, anchor: anchor?.() }),
  };
};
