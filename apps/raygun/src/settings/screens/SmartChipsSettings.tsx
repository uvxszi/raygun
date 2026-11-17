import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { ChipList } from '../../chips/ChipList';

/**
 * Settings → Smart Chips: optional features installed like extensions (M46.57), each downloaded,
 * then switched on or off here or in the top bar's Smart Chips sheet.
 */
export const SmartChipsSettings: FC = () => {
  const { t } = useTranslation();
  return (
    <section aria-label={t('settings.section.chips.title')} className="flex flex-col gap-2">
      <p className="text-muted px-1 text-sm">{t('smartChips.what')}</p>
      <ChipList />
    </section>
  );
};
