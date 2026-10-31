import type { Theme } from '@raygun/bindings';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSettings, useUpdateSetting } from './useSettings';

const THEME_OPTIONS: { value: Theme; labelKey: string }[] = [
  { value: 'system', labelKey: 'settings.themeSystem' },
  { value: 'light', labelKey: 'settings.themeLight' },
  { value: 'dark', labelKey: 'settings.themeDark' },
  { value: 'amoled', labelKey: 'settings.themeAmoled' },
];

export const AppearanceCard: FC = () => {
  const { t } = useTranslation();
  const settings = useSettings();
  const updateSetting = useUpdateSetting();

  if (!settings.data) {
    return null;
  }

  return (
    <section
      data-testid="appearance-card"
      className="bg-surface flex w-full flex-col gap-3 rounded-xl px-5 py-4 text-sm"
    >
      <h2 className="text-muted">{t('settings.appearance')}</h2>
      <h3 id="theme-label" className="font-medium">
        {t('settings.theme')}
      </h3>
      <div role="radiogroup" aria-labelledby="theme-label" className="grid grid-cols-4 gap-2">
        {THEME_OPTIONS.map((option) => {
          const selected = settings.data.theme === option.value;
          return (
            <button
              key={option.value}
              type="button"
              role="radio"
              aria-checked={selected}
              disabled={updateSetting.isPending}
              onClick={() => updateSetting.mutate({ key: 'theme', value: option.value })}
              className={
                selected
                  ? 'bg-primary text-background rounded-lg px-2 py-2 font-semibold'
                  : 'bg-background text-foreground rounded-lg px-2 py-2'
              }
            >
              {t(option.labelKey)}
            </button>
          );
        })}
      </div>
      {updateSetting.isError && (
        <p role="alert" className="text-primary">
          {t('settings.saveFailed', { message: updateSetting.error.message })}
        </p>
      )}
    </section>
  );
};
