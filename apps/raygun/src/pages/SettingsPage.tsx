import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { AppearanceCard } from '../settings/AppearanceCard';
import { ExtensionsCard } from '../settings/ExtensionsCard';
import { McpCard } from '../settings/McpCard';
import { RemoteCard } from '../settings/RemoteCard';
import { AboutCard } from './AboutCard';
import { PageHeader } from './PageHeader';

export const SettingsPage: FC = () => {
  const { t } = useTranslation();
  return (
    <section data-testid="settings-page" className="flex max-w-2xl flex-col gap-4">
      <PageHeader title={t('settings.title')} />
      <AppearanceCard />
      <ExtensionsCard />
      <RemoteCard />
      <McpCard />
      <AboutCard />
    </section>
  );
};
