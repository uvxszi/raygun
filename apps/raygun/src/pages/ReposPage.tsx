import { Plus } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { AddRepoSheet, ReposTab } from '../extensions/ReposTab';
import { TopBar, TopBarAction } from '../shell/TopBar';

/** Extension repositories (Mihon's): the list, suggestions, and "+" to add one by address. */
export const ReposPage: FC = () => {
  const { t } = useTranslation();
  const [adding, setAdding] = useState(false);
  return (
    <section data-testid="repos-page" className="flex max-w-2xl flex-col gap-3">
      <TopBar
        back
        fallback="/browse"
        title={t('browse.repos')}
        actions={
          <TopBarAction
            label={t('extensions.addRepo')}
            icon={Plus}
            onClick={() => setAdding(true)}
          />
        }
      />
      <ReposTab />
      {adding && <AddRepoSheet onClose={() => setAdding(false)} />}
    </section>
  );
};
