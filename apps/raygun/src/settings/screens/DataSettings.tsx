import type { FC } from 'react';

import { BackupCard } from '../BackupCard';
import { DataCategoriesCard } from '../DataCategoriesCard';
import { DeveloperCard } from '../DeveloperCard';
import { EraseCard } from '../EraseCard';
import { StorageSettings } from '../StorageSettings';

/**
 * Settings → Data and backup: backups, what each kind of data holds here with Clear, what's
 * stored and its limits, the test sources, and erasing everything.
 */
export const DataSettings: FC = () => (
  <>
    <BackupCard />
    <DataCategoriesCard />
    <StorageSettings />
    <DeveloperCard />
    <EraseCard />
  </>
);
