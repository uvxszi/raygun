import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useFoundPref } from '../settings/useSettingsSearch';
import { TopBar } from '../shell/TopBar';
import { AppUpdateCard } from '../updates/AppUpdateCard';
import { AboutCard } from './AboutCard';

/** About: updates (M46.47), the version, platform, where Raygun keeps its data, and licences. */
export const AboutPage: FC = () => {
  const { t } = useTranslation();
  useFoundPref();
  return (
    <section data-testid="about-page" className="flex max-w-2xl flex-col">
      <TopBar back fallback="/more" title={t('more.about')} />
      <AppUpdateCard />
      <AboutCard />
    </section>
  );
};
