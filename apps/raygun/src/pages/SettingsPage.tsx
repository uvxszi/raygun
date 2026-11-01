import { Info } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PrefGroup, PrefLink } from '../settings/prefs';
import { SETTINGS_SECTIONS } from '../settings/sections';
import { TopBar } from '../shell/TopBar';

/** Settings (Mihon's): a category per screen, each saying what's inside. */
export const SettingsPage: FC = () => {
  const { t } = useTranslation();
  return (
    <section data-testid="settings-page" className="flex max-w-2xl flex-col gap-1">
      <TopBar back fallback="/more" title={t('settings.title')} />
      <PrefGroup>
        {SETTINGS_SECTIONS.map((section) => (
          <PrefLink
            key={section.id}
            to={`/settings/${section.id}`}
            icon={section.icon}
            title={t(`settings.section.${section.id}.title`)}
            summary={t(`settings.section.${section.id}.summary`)}
          />
        ))}
        <PrefLink to="/about" icon={Info} title={t('more.about')} summary={t('more.aboutHint')} />
      </PrefGroup>
    </section>
  );
};
