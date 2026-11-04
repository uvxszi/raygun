import type { FC } from 'react';

import { BackupCard } from '../BackupCard';
import { DeveloperCard } from '../DeveloperCard';
import { StorageSettings } from '../StorageSettings';

/** Settings → Data and backup: backups, what's stored and its limits, and the test sources. */
export const DataSettings: FC = () => (
  <>
    <BackupCard />
    <StorageSettings />
    <DeveloperCard />
  </>
);
