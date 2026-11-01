import type { Theme } from '@raygun/bindings';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { LANGUAGES, SYSTEM_LANGUAGE, nativeName, resolveLanguage } from '../i18n/languages';
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
      <div className="flex flex-col gap-2">
        <span id="language-label" className="font-medium">
          {t('settings.language')}
        </span>
        <select
          aria-labelledby="language-label"
          value={
            LANGUAGES.includes(settings.data.language) ? settings.data.language : SYSTEM_LANGUAGE
          }
          disabled={updateSetting.isPending}
          onChange={(event) => updateSetting.mutate({ key: 'language', value: event.target.value })}
          className="bg-background rounded-lg px-3 py-2 outline-none"
        >
          <option value={SYSTEM_LANGUAGE}>
            {t('settings.languageSystem', { name: nativeName(resolveLanguage(SYSTEM_LANGUAGE)) })}
          </option>
          {LANGUAGES.map((tag) => (
            <option key={tag} value={tag}>
              {nativeName(tag)}
            </option>
          ))}
        </select>
        <span className="text-muted text-xs">{t('settings.languageHint')}</span>
      </div>
      {updateSetting.isError && (
        <p role="alert" className="text-primary">
          {t('settings.saveFailed', { message: updateSetting.error.message })}
        </p>
      )}
    </section>
  );
};
