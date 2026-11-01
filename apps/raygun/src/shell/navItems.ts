import { Compass, Ellipsis, House, Library } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';

export type NavItem = {
  to: '/' | '/library' | '/browse' | '/more';
  labelKey: string;
  icon: LucideIcon;
  testId: string;
};

/**
 * The app's tabs (Mihon and Aniyomi's shape): what you have, where to find more (Browse, which
 * also searches), everything else.
 */
export const NAV_ITEMS: NavItem[] = [
  { to: '/', labelKey: 'nav.home', icon: House, testId: 'nav-home' },
  { to: '/library', labelKey: 'nav.library', icon: Library, testId: 'nav-library' },
  { to: '/browse', labelKey: 'nav.browse', icon: Compass, testId: 'nav-browse' },
  { to: '/more', labelKey: 'nav.more', icon: Ellipsis, testId: 'nav-more' },
];
