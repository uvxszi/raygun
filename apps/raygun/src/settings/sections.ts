import {
  BookOpen,
  Cable,
  Compass,
  Database,
  Download,
  LibraryBig,
  Palette,
  PlayCircle,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';

/** The settings categories, one screen each (`/settings/<id>`). */
export type SettingsSection =
  'appearance' | 'library' | 'reader' | 'player' | 'downloads' | 'browse' | 'data' | 'connections';

export const SETTINGS_SECTIONS: { id: SettingsSection; icon: LucideIcon }[] = [
  { id: 'appearance', icon: Palette },
  { id: 'library', icon: LibraryBig },
  { id: 'reader', icon: BookOpen },
  { id: 'player', icon: PlayCircle },
  { id: 'downloads', icon: Download },
  { id: 'browse', icon: Compass },
  { id: 'data', icon: Database },
  { id: 'connections', icon: Cable },
];
