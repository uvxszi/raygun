import { Link } from '@tanstack/react-router';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { NAV_ITEMS } from './navItems';

export const BottomTabs: FC = () => {
  const { t } = useTranslation();
  return (
    <nav
      aria-label={t('nav.main')}
      data-testid="bottom-tabs"
      className="bg-surface border-background grid grid-flow-col auto-cols-fr border-t pb-[env(safe-area-inset-bottom)] lg:hidden"
    >
      {NAV_ITEMS.map(({ to, labelKey, icon: Icon, testId }) => (
        <Link
          key={to}
          to={to}
          data-testid={`${testId}-tab`}
          className="text-muted flex flex-col items-center gap-1 py-2 text-xs"
          activeProps={{ className: 'text-primary', 'aria-current': 'page' }}
          activeOptions={{ exact: to === '/', includeSearch: false }}
        >
          <Icon aria-hidden size={22} />
          <span>{t(labelKey)}</span>
        </Link>
      ))}
    </nav>
  );
};
