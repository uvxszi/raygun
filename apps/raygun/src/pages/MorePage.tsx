import {
  BellDot,
  Download,
  Info,
  LayoutList,
  Package,
  Puzzle,
  Settings,
  Smartphone,
  Sparkles,
  Tag,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useLayout } from '../layout/useLayout';
import { PrefLink } from '../settings/prefs';
import { useNavTabs } from '../shell/navTabs';
import { OverflowMenu } from '../shell/OverflowMenu';
import { TopBar } from '../shell/TopBar';

type MoreRow = {
  to: string;
  search?: Record<string, unknown>;
  icon: LucideIcon;
  label: string;
};

/**
 * More (Mihon's): tabs the bar doesn't hold (hidden ones and, on phones, those past five; as in
 * Aniyomi), then everything else in the order the user set (M46.5). Rows are plain names.
 */
export const MorePage: FC = () => {
  const { t } = useTranslation();
  const layout = useLayout();
  const { inMore } = useNavTabs();
  const entries: Record<string, MoreRow> = {
    updates: { to: '/updates', icon: BellDot, label: t('more.updates') },
    downloads: { to: '/downloads', icon: Download, label: t('more.downloads') },
    categories: {
      to: '/library/categories',
      search: { medium: 'manga' },
      icon: Tag,
      label: t('more.categories'),
    },
    addons: { to: '/watch/addons', icon: Puzzle, label: t('watch.addons') },
    repos: { to: '/extensions/repos', icon: Package, label: t('browse.repos') },
    ai: { to: '/settings/ai', icon: Sparkles, label: t('more.ai') },
    devices: { to: '/settings/devices', icon: Smartphone, label: t('more.devices') },
    settings: { to: '/settings', icon: Settings, label: t('settings.title') },
    about: { to: '/about', icon: Info, label: t('more.about') },
  };
  const rows = layout.moreEntries
    .filter((item) => item.visible)
    .flatMap((item) => entries[item.id] ?? []);
  // A tab that's also an entry (Updates, Downloads, Settings) isn't listed twice.
  const tabRows: MoreRow[] = inMore
    .filter(
      (tab) =>
        !rows.some((row) => row.to === tab.to && !row.search && !Object.keys(tab.search).length),
    )
    .map((tab) => ({ to: tab.to, search: tab.search, icon: tab.icon, label: t(tab.labelKey) }));
  const link = (row: MoreRow) => (
    <PrefLink
      key={`${row.to}?${JSON.stringify(row.search ?? {})}`}
      to={row.to}
      search={row.search}
      icon={row.icon}
      title={row.label}
    />
  );

  return (
    <section data-testid="more-page" className="flex max-w-2xl flex-col gap-2">
      <TopBar
        title={t('nav.more')}
        actions={
          <OverflowMenu
            items={[
              { label: t('layout.editMore'), icon: LayoutList, to: '/settings/layout/moreEntries' },
              { label: t('layout.editTabs'), icon: LayoutList, to: '/settings/layout/tabs' },
            ]}
          />
        }
      />
      {tabRows.length > 0 && (
        <ul aria-label={t('more.otherTabs')} className="border-surface flex flex-col border-b pb-2">
          {tabRows.map(link)}
        </ul>
      )}
      <ul className="flex flex-col">{rows.map(link)}</ul>
    </section>
  );
};
