import { useRouterState } from '@tanstack/react-router';
import {
  BellDot,
  BookImage,
  BookOpen,
  Clapperboard,
  Compass,
  Download,
  Ellipsis,
  House,
  Library,
  Music,
  Podcast,
  Search,
  Settings,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';

import { LAYOUT_ENTRIES } from '../layout/layout';
import { useLayout } from '../layout/useLayout';
import { useSideRail } from './useWideScreen';

/** Every screen that can be a tab (M46.5); Home, Library, Browse and More show by default. */
export type NavTabId =
  | 'home'
  | 'library'
  | 'browse'
  | 'more'
  | 'watch'
  | 'music'
  | 'comics'
  | 'books'
  | 'podcasts'
  | 'search'
  | 'updates'
  | 'downloads'
  | 'settings';

export type NavTab = {
  id: NavTabId;
  to: string;
  /** What the tab's address must also say (Watch is Library with `media=video`). */
  search: Record<string, string>;
  labelKey: string;
  icon: LucideIcon;
};

export const NAV_TABS: Record<NavTabId, NavTab> = {
  home: { id: 'home', to: '/', search: {}, labelKey: 'nav.home', icon: House },
  library: { id: 'library', to: '/library', search: {}, labelKey: 'nav.library', icon: Library },
  browse: { id: 'browse', to: '/browse', search: {}, labelKey: 'nav.browse', icon: Compass },
  more: { id: 'more', to: '/more', search: {}, labelKey: 'nav.more', icon: Ellipsis },
  watch: {
    id: 'watch',
    to: '/library',
    search: { media: 'video' },
    labelKey: 'nav.watch',
    icon: Clapperboard,
  },
  music: {
    id: 'music',
    to: '/library',
    search: { media: 'music' },
    labelKey: 'nav.music',
    icon: Music,
  },
  comics: {
    id: 'comics',
    to: '/library',
    search: { media: 'manga' },
    labelKey: 'nav.comics',
    icon: BookImage,
  },
  books: {
    id: 'books',
    to: '/library',
    search: { media: 'novel' },
    labelKey: 'nav.books',
    icon: BookOpen,
  },
  podcasts: {
    id: 'podcasts',
    to: '/library',
    search: { view: 'podcasts' },
    labelKey: 'nav.podcasts',
    icon: Podcast,
  },
  search: {
    id: 'search',
    to: '/browse',
    search: { tab: 'search' },
    labelKey: 'nav.search',
    icon: Search,
  },
  updates: { id: 'updates', to: '/updates', search: {}, labelKey: 'nav.updates', icon: BellDot },
  downloads: {
    id: 'downloads',
    to: '/downloads',
    search: {},
    labelKey: 'nav.downloads',
    icon: Download,
  },
  settings: {
    id: 'settings',
    to: '/settings',
    search: {},
    labelKey: 'nav.settings',
    icon: Settings,
  },
};

export const isNavTabId = (value: string): value is NavTabId => value in NAV_TABS;

const SHOWN_BY_DEFAULT = new Set(
  LAYOUT_ENTRIES.tabs.filter(([, visible]) => visible).map(([id]) => id),
);

/** The most tabs a phone's bottom bar holds (Material's limit). */
export const PHONE_TABS = 5;

/**
 * How well `tab` matches where the app is: 0 when it doesn't, more for a closer match (Watch
 * beats Library on Library → Videos).
 */
export const tabScore = (tab: NavTab, pathname: string, search: Record<string, unknown>) => {
  const onPath =
    tab.to === '/' ? pathname === '/' : pathname === tab.to || pathname.startsWith(`${tab.to}/`);
  if (!onPath) return 0;
  let score = 1;
  for (const [key, value] of Object.entries(tab.search)) {
    if (String(search[key] ?? '') !== value) return 0;
    score += 1;
  }
  return score;
};

/**
 * The tabs as the user arranged them: those in the bar (at most five on phones, More last when
 * some don't fit; all on the desktop rail), those More lists (shown ones that don't fit, hidden
 * default ones), and the one that's open.
 */
export const useNavTabs = () => {
  const layout = useLayout();
  const rail = useSideRail();
  const location = useRouterState({ select: (state) => state.location });
  const shown = layout.tabs
    .filter((item) => item.visible && isNavTabId(item.id))
    .map((item) => NAV_TABS[item.id as NavTabId]);
  const fits = rail || shown.length <= PHONE_TABS;
  const bar = fits
    ? shown
    : [...shown.filter((tab) => tab.id !== 'more').slice(0, PHONE_TABS - 1), NAV_TABS.more];
  // More lists the tabs turned on that don't fit, and default tabs hidden (so they stay
  // reachable); screens that could be added but weren't are reached the usual way.
  const inMore = layout.tabs
    .filter(
      (item) =>
        isNavTabId(item.id) &&
        item.id !== 'more' &&
        (item.visible || SHOWN_BY_DEFAULT.has(item.id)),
    )
    .map((item) => NAV_TABS[item.id as NavTabId])
    .filter((tab) => !bar.includes(tab));
  const search = location.search as Record<string, unknown>;
  let active: NavTabId | null = null;
  let best = 0;
  for (const tab of bar) {
    const score = tabScore(tab, location.pathname, search);
    if (score > best) {
      best = score;
      active = tab.id;
    }
  }
  return { bar, inMore, active };
};
