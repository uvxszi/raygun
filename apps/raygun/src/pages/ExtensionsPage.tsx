import { Link, useSearch } from '@tanstack/react-router';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { BrowseTab } from '../extensions/BrowseTab';
import { InstalledTab } from '../extensions/InstalledTab';
import { ReposTab } from '../extensions/ReposTab';
import { EXTENSION_TABS } from '../extensions/tabs';
import { PageHeader } from './PageHeader';

export const ExtensionsPage: FC = () => {
  const { t } = useTranslation();
  const { tab = 'installed' } = useSearch({ from: '/extensions' });

  return (
    <section data-testid="extensions-page" className="flex max-w-2xl flex-col gap-5">
      <PageHeader title={t('extensions.title')} />
      <nav className="flex gap-2" aria-label={t('extensions.tabs')}>
        {EXTENSION_TABS.map((option) => (
          <Link
            key={option}
            to="/extensions"
            search={{ tab: option }}
            replace
            data-testid={`extensions-tab-${option}`}
            aria-current={option === tab ? 'page' : undefined}
            className={
              option === tab
                ? 'bg-primary text-background rounded-full px-4 py-1.5 text-sm font-semibold'
                : 'bg-surface rounded-full px-4 py-1.5 text-sm'
            }
          >
            {t(`extensions.tab.${option}`)}
          </Link>
        ))}
      </nav>
      {tab === 'installed' && <InstalledTab />}
      {tab === 'browse' && <BrowseTab />}
      {tab === 'repos' && <ReposTab />}
    </section>
  );
};
