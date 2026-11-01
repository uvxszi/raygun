import { Compass, Ellipsis, House, Library, Search } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';

export type NavItem = {
  to: '/' | '/library' | '/browse' | '/search' | '/more';
  labelKey: string;
  icon: LucideIcon;
  testId: string;
};

/** The app's tabs (Mihon and Aniyomi's shape): what you have, where to find more, everything else. */
export const NAV_ITEMS: NavItem[] = [
  { to: '/', labelKey: 'nav.home', icon: House, testId: 'nav-home' },
  { to: '/library', labelKey: 'nav.library', icon: Library, testId: 'nav-library' },
  { to: '/browse', labelKey: 'nav.browse', icon: Compass, testId: 'nav-browse' },
  { to: '/search', labelKey: 'nav.search', icon: Search, testId: 'nav-search' },
  { to: '/more', labelKey: 'nav.more', icon: Ellipsis, testId: 'nav-more' },
];
