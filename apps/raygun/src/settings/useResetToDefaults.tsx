import { RotateCcw } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { OverflowMenu } from '../shell/OverflowMenu';
import { ConfirmSheet } from './ConfirmSheet';
import type { SettingsReset } from './resets';
import { useSettings, useSettingsDefaults, useUpdateSettings } from './useSettings';

/** A screen's Reset to defaults: the ⋮ item, and the question before it. */
export const useResetToDefaults = (name: string, reset: SettingsReset | undefined) => {
  const { t } = useTranslation();
  const settings = useSettings().data;
  const defaults = useSettingsDefaults().data;
  const update = useUpdateSettings();
  const [asking, setAsking] = useState(false);
  const ready = reset && settings && defaults;
  return {
    menu: ready ? (
      <OverflowMenu
        items={[{ label: t('settings.reset'), icon: RotateCcw, onSelect: () => setAsking(true) }]}
      />
    ) : null,
    sheet:
      ready && asking ? (
        <ConfirmSheet
          title={t('settings.resetQuestion', { name })}
          confirm={t('settings.resetConfirm')}
          onConfirm={() => update.mutate(reset(settings, defaults))}
          onClose={() => setAsking(false)}
        />
      ) : null,
  };
};
