import { House, Library, Search, Settings } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';

export type NavItem = {
  to: '/' | '/library' | '/search' | '/settings';
  labelKey: string;
  icon: LucideIcon;
  testId: string;
};

export const NAV_ITEMS: NavItem[] = [
  { to: '/', labelKey: 'nav.home', icon: House, testId: 'nav-home' },
  { to: '/library', labelKey: 'nav.library', icon: Library, testId: 'nav-library' },
  { to: '/search', labelKey: 'nav.search', icon: Search, testId: 'nav-search' },
  { to: '/settings', labelKey: 'nav.settings', icon: Settings, testId: 'nav-settings' },
];
