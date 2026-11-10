import { Link } from '@tanstack/react-router';
import { useMemo } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useDragScroll } from '../shell/useDragScroll';
import { BrowseTab } from './BrowseTab';
import { updateCount } from './catalogFilters';
import { InstallProgress } from './InstallProgress';
import { InstalledTab } from './InstalledTab';
import { EXTENSION_LISTS } from './tabs';
import type { ExtensionList } from './tabs';
import { useCatalog } from './useExtensions';

/** Where a list's pill leads: the page showing the catalogue, with that list in its address. */
export type ExtensionListLink = (list: ExtensionList) => {
  to: string;
  search: Record<string, unknown>;
};

/**
 * The extension catalogue: Browse → Extensions, and Sources' Extension catalogue. Every
 * repository's extensions by list (All, Installed, Not installed, and Updates while there are
 * some), installed, updated and removed in place, with the install queue's progress.
 */
export const ExtensionCatalog: FC<{ list: ExtensionList; linkOf: ExtensionListLink }> = ({
  list,
  linkOf,
}) => {
  const { t } = useTranslation();
  const listRow = useDragScroll<HTMLElement>();
  const catalog = useCatalog();
  const updates = useMemo(() => updateCount(catalog.data), [catalog.data]);
  // Updates shows only while there are some (or while it's open).
  const lists = EXTENSION_LISTS.filter(
    (option) => option !== 'updates' || updates > 0 || list === 'updates',
  );
  return (
    <div className="flex flex-col gap-4">
      <nav
        ref={listRow}
        aria-label={t('extensions.tabs')}
        className="-mx-4 flex gap-2 no-scrollbar overflow-x-auto px-4 lg:mx-0 lg:px-0"
      >
        {lists.map((option) => {
          const link = linkOf(option);
          return (
            <Link
              key={option}
              to={link.to}
              search={link.search as never}
              replace
              data-testid={`extensions-tab-${option}`}
              aria-current={option === list ? 'page' : undefined}
              className={`shrink-0 rounded-full px-4 py-1.5 text-sm ${option === list ? 'bg-primary/20 text-primary font-semibold' : 'bg-surface'}`}
            >
              {option === 'updates'
                ? t('extensions.tab.updatesCount', { count: updates })
                : t(`extensions.tab.${option}`)}
            </Link>
          );
        })}
      </nav>
      <InstallProgress />
      {list === 'installed' ? <InstalledTab /> : <BrowseTab show={list} />}
    </div>
  );
};
