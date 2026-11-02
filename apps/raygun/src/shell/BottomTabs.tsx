import { Link } from '@tanstack/react-router';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSettings } from '../settings/useSettings';
import { useNavTabs } from './navTabs';

/**
 * The phone's navigation bar (Material 3, as in Mihon): the active tab's icon sits in a pill. The
 * tabs are the user's (M46.5): at most five, the rest listed in More.
 */
export const BottomTabs: FC = () => {
  const { t } = useTranslation();
  const selectedOnly = useSettings().data?.navLabels === 'selected';
  const { bar, active } = useNavTabs();
  return (
    <nav
      aria-label={t('nav.main')}
      data-testid="bottom-tabs"
      className="bg-surface grid grid-flow-col auto-cols-fr pb-[env(safe-area-inset-bottom)] lg:hidden"
    >
      {bar.map(({ id, to, search, labelKey, icon: Icon }) => (
        <Link
          key={id}
          to={to}
          search={search}
          data-testid={`nav-${id}-tab`}
          aria-current={active === id ? 'page' : undefined}
          // The bar decides which tab is current; the link only counts an exact match.
          activeOptions={{ exact: true, includeSearch: true }}
          className={`group flex flex-col items-center gap-1 pt-3 pb-3 text-xs font-medium ${active === id ? 'text-foreground' : 'text-muted'}`}
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
