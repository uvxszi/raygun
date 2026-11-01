import { Link } from '@tanstack/react-router';
import {
  BellDot,
  ChevronRight,
  Download,
  Info,
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

/** More (Mihon's): everything that isn't a tab of its own, then Settings and About. */
export const MorePage: FC = () => {
  const { t } = useTranslation();
  const activity: MoreRow[] = [
    { to: '/updates', icon: BellDot, label: t('more.updates'), hint: t('more.updatesHint') },
    {
      to: '/downloads',
      icon: Download,
      label: t('more.downloads'),
      hint: t('more.downloadsHint'),
    },
    {
      to: '/library/categories',
      search: { medium: 'manga' },
      icon: Tag,
      label: t('more.categories'),
      hint: t('more.categoriesHint'),
    },
  ];
  const sources: MoreRow[] = [
    { to: '/watch/addons', icon: Puzzle, label: t('watch.addons'), hint: t('more.addonsHint') },
    { to: '/extensions/repos', icon: Package, label: t('browse.repos'), hint: t('more.reposHint') },
    { to: '/settings/ai', icon: Sparkles, label: t('more.ai'), hint: t('more.aiHint') },
    {
      to: '/settings/devices',
      icon: Smartphone,
      label: t('more.devices'),
      hint: t('more.devicesHint'),
    },
  ];
  const app: MoreRow[] = [
    { to: '/settings', icon: Settings, label: t('settings.title'), hint: t('more.settingsHint') },
    { to: '/about', icon: Info, label: t('more.about'), hint: t('more.aboutHint') },
  ];
  return (
    <section data-testid="more-page" className="flex max-w-2xl flex-col gap-2">
      <TopBar title={t('nav.more')} />
      {[activity, sources, app].map((group, index) => (
        <ul key={index} className="border-surface flex flex-col border-b pb-2 last:border-b-0">
          {group.map((row) => (
            <Row key={row.to} row={row} />
          ))}
        </ul>
      ))}
    </section>
  );
};
