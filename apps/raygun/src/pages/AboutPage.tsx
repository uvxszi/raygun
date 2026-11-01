import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { TopBar } from '../shell/TopBar';
import { AboutCard } from './AboutCard';

/** About: the version, platform and where Raygun keeps its data. */
export const AboutPage: FC = () => {
  const { t } = useTranslation();
  return (
    <section data-testid="about-page" className="flex max-w-2xl flex-col gap-3">
      <TopBar back fallback="/more" title={t('more.about')} />
      <AboutCard />
    </section>
  );
};
