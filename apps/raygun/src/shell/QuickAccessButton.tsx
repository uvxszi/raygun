import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useQuickAccessShown, useQuickAccessStore } from './quickAccessStore';

/**
 * Raygun's icon at the right of the Home, Library and Browse top bars where there's no sidebar
 * (phones, narrow tablets): it opens quick access, the sidebar's pins in a drawer.
 */
export const QuickAccessButton: FC = () => {
  const { t } = useTranslation();
  const shown = useQuickAccessShown();
  const open = useQuickAccessStore((state) => state.open);
  const setOpen = useQuickAccessStore((state) => state.setOpen);
  if (!shown) return null;
  return (
    <button
      type="button"
      aria-label={t('quickAccess.open')}
      aria-haspopup="dialog"
      aria-expanded={open}
      data-testid="quick-access-button"
      onClick={() => setOpen(true)}
      className="hover:bg-surface grid size-11 shrink-0 place-items-center rounded-full"
    >
      <img src={`${import.meta.env.BASE_URL}icon.svg`} alt="" className="size-7" />
    </button>
  );
};
