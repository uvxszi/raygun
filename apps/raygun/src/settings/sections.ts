import {
  BookOpen,
  Compass,
  Database,
  Download,
  Info,
  LibraryBig,
  Palette,
  PlayCircle,
  Shield,
  SlidersVertical,
  Smartphone,
  Sparkles,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import type { FC } from 'react';

import { SoundSettings } from '../sound/SoundSettings';
import { DownloadSettings } from './DownloadSettings';
import { resetAll, resetKeys } from './resets';
import type { SettingsReset } from './resets';
import { AppearanceSettings } from './screens/AppearanceSettings';
import { BrowseSettings } from './screens/BrowseSettings';
import { DataSettings } from './screens/DataSettings';
import { LibrarySettings } from './screens/LibrarySettings';
import { PlayerSettings } from './screens/PlayerSettings';
import { PrivacySettings } from './screens/PrivacySettings';
import { ReaderSettings } from './screens/ReaderSettings';

/**
 * An entry of the Settings list: a category screen at `/settings/<id>` (`screen`, with its
 * Reset to defaults), or a page of its own (`to`). Its title is `settings.section.<id>.title`.
 */
export type SettingsEntry = {
  id: string;
  icon: LucideIcon;
  screen?: FC;
  to?: string;
  reset?: SettingsReset;
};

/** The Settings list, most used first. */
export const SETTINGS_ENTRIES: SettingsEntry[] = [
  {
    id: 'appearance',
    icon: Palette,
    screen: AppearanceSettings,
    reset: resetKeys(
      'theme',
      'accent',
      'accentColor',
      'colors',
      'zoom',
      'startScreen',
      'navLabels',
      'motion',
      'scrollbars',
      'windowFrame',
    ),
  },
  {
    id: 'player',
    icon: PlayCircle,
    screen: PlayerSettings,
    reset: resetKeys(
      'audioQuality',
      'resumeOnStart',
      'keepMusicPlaying',
      'playerView',
      'preferredVideoQuality',
      'resumeVideos',
      'watchedPercent',
      'autoplayNext',
      'nextPromptSeconds',
      'seekSeconds',
      'autoSubtitles',
      'subtitleLanguage',
      'videoPlayer',
    ),
  },
  { id: 'sound', icon: SlidersVertical, screen: SoundSettings, reset: resetKeys('sound') },
  {
    id: 'library',
    icon: LibraryBig,
    screen: LibrarySettings,
    reset: resetAll(
      resetKeys(
        'libraryDisplay',
        'coverSize',
        'coverStyle',
        'unreadBadges',
        'downloadBadges',
        'mangaUpdateHours',
      ),
      // The sidebar's pins are the user's own; only its switches go back.
      (current, defaults) => [
        {
          key: 'rail',
          value: { ...current.rail, autoAdd: defaults.rail.autoAdd, limit: defaults.rail.limit },
        },
      ],
    ),
  },
  {
    id: 'downloads',
    icon: Download,
    screen: DownloadSettings,
    // The picked folder stays picked, to choose again in one tap.
    reset: (current, defaults) => [
      {
        key: 'save',
        value: {
          ...defaults.save,
          folderUri: current.save.folderUri ?? null,
          folderName: current.save.folderName ?? null,
        },
      },
    ],
  },
  {
    id: 'reader',
    icon: BookOpen,
    screen: ReaderSettings,
    reset: resetKeys('mangaReader', 'novelReader'),
  },
  {
    id: 'browse',
    icon: Compass,
    screen: BrowseSettings,
    reset: resetKeys(
      'showNsfw',
      'explicitMusic',
      'failingSourcesLast',
      'autoLoad',
      'userAgent',
      'discovery',
      'websites',
    ),
  },
  { id: 'privacy', icon: Shield, screen: PrivacySettings, reset: resetKeys('privacy') },
  { id: 'devices', icon: Smartphone, to: '/settings/devices' },
  { id: 'ai', icon: Sparkles, to: '/settings/ai' },
  {
    id: 'data',
    icon: Database,
    screen: DataSettings,
    reset: resetKeys('cache', 'lyricsCache'),
  },
  { id: 'about', icon: Info, to: '/about' },
];

/** The category screens' ids: the router needs each path spelled out. */
export type SettingsSectionId =
  | 'appearance'
  | 'player'
  | 'sound'
  | 'library'
  | 'downloads'
  | 'reader'
  | 'browse'
  | 'privacy'
  | 'data';

/** The category screens, routed at `/settings/<id>`. */
export const SETTINGS_SECTIONS = SETTINGS_ENTRIES.filter(
  (entry): entry is SettingsEntry & { id: SettingsSectionId; screen: FC } =>
    entry.screen !== undefined,
);

export const settingsEntry = (id: string | undefined) =>
  SETTINGS_ENTRIES.find((entry) => entry.id === id);
