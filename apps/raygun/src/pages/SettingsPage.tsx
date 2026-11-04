import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PrefGroup, PrefLink } from '../settings/prefs';
import { SETTINGS_ENTRIES } from '../settings/sections';
import { SettingsSearchResults } from '../settings/SettingsSearch';
import { TopBar } from '../shell/TopBar';
import { shownOn, useIsDesktop } from '../shell/useIsDesktop';
import { useSearchMode } from '../shell/useSearchMode';

/** Settings (Mihon's): a category per screen, and search across every setting. */
export const SettingsPage: FC = () => {
  const { t } = useTranslation();
  const search = useSearchMode(t('settings.search'));
  const searching = search.query.trim() !== '';
  const desktop = useIsDesktop();
  return (
    <section data-testid="settings-page" className="flex max-w-2xl flex-col gap-1">
      <TopBar
        back
        fallback="/more"
        title={t('settings.title')}
        search={search.search}
        actions={search.action}
      />
      {searching ? (
        <SettingsSearchResults query={search.query} />
      ) : (
        <PrefGroup>
          {SETTINGS_ENTRIES.filter((entry) => shownOn(entry.only, desktop)).map((entry) => (
            <PrefLink
              key={entry.id}
              to={entry.to ?? `/settings/${entry.id}`}
              icon={entry.icon}
              title={t(`settings.section.${entry.id}.title`)}
            />
          ))}
        </PrefGroup>
      )}
    </section>
  );
};
