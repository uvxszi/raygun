import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { AiCard } from '../settings/AiCard';
import { AppearanceCard } from '../settings/AppearanceCard';
import { BackupCard } from '../settings/BackupCard';
import { ContentCard } from '../settings/ContentCard';
import { DevicesCard } from '../settings/DevicesCard';
import { ExtensionsCard } from '../settings/ExtensionsCard';
import { MangaCard } from '../settings/MangaCard';
import { PlaybackCard } from '../settings/PlaybackCard';
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
      <ContentCard />
      <PlaybackCard />
      <MangaCard />
      <ExtensionsCard />
      <AiCard />
      <BackupCard />
      <DevicesCard />
      <RemoteCard />
      <McpCard />
      <AboutCard />
    </section>
  );
};
