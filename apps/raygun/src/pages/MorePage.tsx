import { Link } from '@tanstack/react-router';
import {
  BellDot,
  ChevronRight,
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
import { useNavTabs } from '../shell/navTabs';
import { OverflowMenu } from '../shell/OverflowMenu';
import { TopBar } from '../shell/TopBar';

type MoreRow = {
  to: string;
  search?: Record<string, unknown>;
  icon: LucideIcon;
  label: string;
  hint: string;
};

const Row: FC<{ row: MoreRow }> = ({ row }) => (
  <li>
    <Link
      to={row.to}
      search={row.search}
      className="hover:bg-surface flex items-center gap-4 rounded-xl px-2 py-3"
    >
      <row.icon aria-hidden size={22} className="text-primary shrink-0" />
      <span className="flex min-w-0 flex-1 flex-col">
        <span className="font-medium">{row.label}</span>
        <span className="text-muted truncate text-xs">{row.hint}</span>
      </span>
      <ChevronRight aria-hidden size={18} className="text-muted shrink-0" />
    </Link>
  </li>
);

/**
 * More (Mihon's): tabs the bar doesn't hold (hidden ones and, on phones, those past five; as in
 * Aniyomi), then everything else in the order the user set (M46.5).
 */
export const MorePage: FC = () => {
  const { t } = useTranslation();
  const layout = useLayout();
  const { inMore } = useNavTabs();
  const entries: Record<string, MoreRow> = {
    updates: {
      to: '/updates',
      icon: BellDot,
      label: t('more.updates'),
      hint: t('more.updatesHint'),
    },
    downloads: {
      to: '/downloads',
      icon: Download,
      label: t('more.downloads'),
      hint: t('more.downloadsHint'),
    },
    categories: {
      to: '/library/categories',
      search: { medium: 'manga' },
      icon: Tag,
      label: t('more.categories'),
      hint: t('more.categoriesHint'),
    },
    addons: {
      to: '/watch/addons',
      icon: Puzzle,
      label: t('watch.addons'),
      hint: t('more.addonsHint'),
    },
    repos: {
      to: '/extensions/repos',
      icon: Package,
      label: t('browse.repos'),
      hint: t('more.reposHint'),
    },
    ai: { to: '/settings/ai', icon: Sparkles, label: t('more.ai'), hint: t('more.aiHint') },
    devices: {
      to: '/settings/devices',
      icon: Smartphone,
      label: t('more.devices'),
      hint: t('more.devicesHint'),
    },
    settings: {
      to: '/settings',
      icon: Settings,
      label: t('settings.title'),
      hint: t('more.settingsHint'),
    },
    about: { to: '/about', icon: Info, label: t('more.about'), hint: t('more.aboutHint') },
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
    .map((tab) => ({
      to: tab.to,
      search: tab.search,
      icon: tab.icon,
      label: t(tab.labelKey),
      hint: t('more.tabHint'),
    }));

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
          {tabRows.map((row) => (
            <Row key={`${row.to}?${JSON.stringify(row.search)}`} row={row} />
          ))}
        </ul>
      )}
      <ul className="flex flex-col">
        {rows.map((row) => (
          <Row key={row.to} row={row} />
        ))}
      </ul>
    </section>
  );
};
