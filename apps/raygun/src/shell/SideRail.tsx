import { Link } from '@tanstack/react-router';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { NAV_ITEMS } from './navItems';

export const SideRail: FC = () => {
  const { t } = useTranslation();
  return (
    <nav
      aria-label={t('nav.main')}
      data-testid="side-rail"
      className="bg-surface hidden w-56 shrink-0 flex-col gap-1 p-3 lg:flex"
    >
      <span className="text-primary px-3 pt-2 pb-4 text-2xl font-black tracking-tight">Raygun</span>
      {NAV_ITEMS.map(({ to, labelKey, icon: Icon, testId }) => (
        <Link
          key={to}
          to={to}
          data-testid={`${testId}-rail`}
          className="text-muted hover:bg-background flex items-center gap-3 rounded-lg px-3 py-2"
          activeProps={{ className: 'text-primary bg-background', 'aria-current': 'page' }}
          activeOptions={{ exact: true }}
        >
          <Icon aria-hidden size={20} />
          <span>{t(labelKey)}</span>
        </Link>
      ))}
    </nav>
  );
};
