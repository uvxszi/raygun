import type { FC } from 'react';

import { NeedsAppNote, useNeedsApp } from '../../shell/NeedsApp';
import { BackupCard } from '../BackupCard';
import { DataCategoriesCard } from '../DataCategoriesCard';
import { DeveloperCard } from '../DeveloperCard';
import { EraseCard } from '../EraseCard';
import { StorageSettings } from '../StorageSettings';

/**
 * Settings → Data and backup: backups, what each kind of data holds here with Clear, what's
 * stored and its limits, the test sources, and erasing everything. Raygun on the web keeps its
 * data in the browser and has none of these (docs/WEB.md).
 */
export const DataSettings: FC = () => {
  const needsApp = useNeedsApp();
  if (needsApp) return <NeedsAppNote />;
  return (
    <>
      <BackupCard />
      <DataCategoriesCard />
      <StorageSettings />
      <DeveloperCard />
      <EraseCard />
    </>
  );
};
