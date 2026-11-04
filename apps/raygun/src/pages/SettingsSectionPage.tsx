import { useRouterState } from '@tanstack/react-router';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { SettingsSaveIssue } from '../settings/Problem';
import { settingsEntry } from '../settings/sections';
import { useFoundPref } from '../settings/useSettingsSearch';
import { useResetToDefaults } from '../settings/useResetToDefaults';
import { TopBar } from '../shell/TopBar';

/** One settings category (`/settings/<section>`), with Reset to defaults in ⋮. */
export const SettingsSectionPage: FC = () => {
  const { t } = useTranslation();
  const id = useRouterState({ select: (state) => state.location.pathname.split('/')[2] });
  const entry = settingsEntry(id);
  const title = t(`settings.section.${id}.title`);
  const reset = useResetToDefaults(title, entry?.reset);
  useFoundPref();
  const Screen = entry?.screen;
  return (
    <section data-testid={`settings-${id}-page`} className="flex max-w-2xl flex-col">
      <TopBar back fallback="/settings" title={title} actions={reset.menu} />
      {Screen && <Screen />}
      <SettingsSaveIssue />
      {reset.sheet}
    </section>
  );
};
