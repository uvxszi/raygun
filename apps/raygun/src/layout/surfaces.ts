import type { TFunction } from 'i18next';
import { BellDot, Download, Info, Puzzle, Settings, Smartphone, Sparkles, Tag } from 'lucide-react';

import { NAV_TABS, isNavTabId } from '../shell/navTabs';
import type { LayoutSurface } from './layout';
import type { LayoutEntry } from './LayoutEditor';

const fromIds = (ids: string[], label: (id: string) => string): Record<string, LayoutEntry> =>
  Object.fromEntries(ids.map((id) => [id, { label: label(id) }]));

/** What each entry of a surface is called (and its icon), for the layout editor (M46.5). */
export const surfaceEntries = (
  surface: LayoutSurface,
  ids: string[],
  t: TFunction,
): Record<string, LayoutEntry> => {
  switch (surface) {
    case 'tabs':
      return Object.fromEntries(
        ids
          .filter(isNavTabId)
          .map((id) => [id, { label: t(NAV_TABS[id].labelKey), icon: NAV_TABS[id].icon }]),
      );
    case 'libraryMedia':
      return fromIds(ids, (id) => t(`library.tabs.${id}`));
    case 'audioViews':
      return fromIds(ids, (id) => t(`library.view.${id}`));
    case 'homeSections':
      return fromIds(ids, (id) => t(`layout.home.${id}`));
    case 'browseTabs':
      return fromIds(ids, (id) => t(`layout.browseTab.${id}`));
    case 'browseSections':
      return fromIds(ids, (id) => t(`layout.browseSection.${id}`));
    case 'moreEntries': {
      const icons: Record<string, LayoutEntry['icon']> = {
        updates: BellDot,
        downloads: Download,
        categories: Tag,
        sources: Puzzle,
        ai: Sparkles,
        devices: Smartphone,
        settings: Settings,
        about: Info,
      };
      return Object.fromEntries(
        ids.map((id) => [id, { label: t(`layout.moreEntry.${id}`), icon: icons[id] }]),
      );
    }
    default:
      return {};
  }
};
