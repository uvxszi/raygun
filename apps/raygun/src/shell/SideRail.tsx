import { Link } from '@tanstack/react-router';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useNavTabs } from './navTabs';

/** The desktop's navigation: every tab the user shows (M46.5), in their order. */
export const SideRail: FC = () => {
  const { t } = useTranslation();
  const { bar, active } = useNavTabs();
  return (
    <nav
      aria-label={t('nav.main')}
      data-testid="side-rail"
      className="bg-surface hidden w-56 shrink-0 flex-col gap-1 p-3 lg:flex"
    >
      <span className="text-primary px-3 pt-2 pb-4 text-2xl font-black tracking-tight">Raygun</span>
      {bar.map(({ id, to, search, labelKey, icon: Icon }) => (
        <Link
          key={id}
          to={to}
          search={search}
          data-testid={`nav-${id}-rail`}
          aria-current={active === id ? 'page' : undefined}
          // The bar decides which tab is current; the link only counts an exact match.
          activeOptions={{ exact: true, includeSearch: true }}
          className={`flex items-center gap-3 rounded-full px-4 py-2.5 font-medium ${active === id ? 'text-primary bg-primary/15' : 'text-muted hover:bg-background'}`}
        >
          <Icon aria-hidden size={20} />
          <span>{t(labelKey)}</span>
        </Link>
      ))}
    </nav>
  );
};
