import { Link } from '@tanstack/react-router';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSettings } from '../settings/useSettings';
import { NAV_ITEMS } from './navItems';

/** The phone's navigation bar (Material 3, as in Mihon): the active tab's icon sits in a pill. */
export const BottomTabs: FC = () => {
  const { t } = useTranslation();
  const selectedOnly = useSettings().data?.navLabels === 'selected';
  return (
    <nav
      aria-label={t('nav.main')}
      data-testid="bottom-tabs"
      className="bg-surface grid grid-flow-col auto-cols-fr pb-[env(safe-area-inset-bottom)] lg:hidden"
    >
      {NAV_ITEMS.map(({ to, labelKey, icon: Icon, testId }) => (
        <Link
          key={to}
          to={to}
          data-testid={`${testId}-tab`}
          className="group text-muted flex flex-col items-center gap-1 pt-3 pb-3 text-xs font-medium"
          activeProps={{ className: 'text-foreground', 'aria-current': 'page' }}
          activeOptions={{ exact: to === '/', includeSearch: false }}
        >
          <span className="group-aria-[current=page]:bg-primary/20 group-aria-[current=page]:text-primary grid h-8 w-16 place-items-center rounded-full transition-colors">
            <Icon aria-hidden size={22} />
          </span>
          <span className={selectedOnly ? 'hidden group-aria-[current=page]:block' : undefined}>
            {t(labelKey)}
          </span>
        </Link>
      ))}
    </nav>
  );
};
