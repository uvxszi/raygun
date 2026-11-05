import {
  BookOpen,
  Compass,
  Database,
  Download,
  Info,
  LibraryBig,
  Music,
  Palette,
  PanelLeft,
  PlayCircle,
  Shield,
  ShieldCheck,
  Smartphone,
  Sparkles,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import type { FC } from 'react';

import type { PlatformOnly } from '../shell/useIsDesktop';
import { DownloadSettings } from './DownloadSettings';
import { PermissionsSettings } from './PermissionsSettings';
import { resetAll, resetKeys } from './resets';
import type { SettingsReset } from './resets';
import { AppearanceSettings } from './screens/AppearanceSettings';
import { BrowseSettings } from './screens/BrowseSettings';
import { DataSettings } from './screens/DataSettings';
import { LibrarySettings } from './screens/LibrarySettings';
import { MusicPlayerSettings } from './screens/MusicPlayerSettings';
import { PrivacySettings } from './screens/PrivacySettings';
import { QuickAccessSettings } from './screens/QuickAccessSettings';
import { ReaderSettings } from './screens/ReaderSettings';
import { VideoPlayerSettings } from './screens/VideoPlayerSettings';

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
  /** The one platform the entry shows on. */
  only?: PlatformOnly;
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
    id: 'music',
    icon: Music,
    screen: MusicPlayerSettings,
    reset: resetAll(
      resetKeys('audioQuality', 'resumeOnStart', 'keepMusicPlaying', 'playerView', 'sound'),
      // Of the panel sizes, only the queue's rows are this screen's.
      (current, defaults) => [
        {
          key: 'panels',
          value: { ...current.panels, nextUpRows: defaults.panels.nextUpRows },
        },
      ],
    ),
  },
  {
    id: 'video',
    icon: PlayCircle,
    screen: VideoPlayerSettings,
    reset: resetKeys(
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
  {
    id: 'reader',
    icon: BookOpen,
    screen: ReaderSettings,
    reset: resetKeys('mangaReader', 'novelReader'),
  },
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
    ),
  },
  {
    id: 'quickAccess',
    icon: PanelLeft,
    screen: QuickAccessSettings,
    // The pins are the user's own, and when pages were played from stays; the rest goes back.
    reset: (current, defaults) => [
      {
        key: 'rail',
        value: {
          ...current.rail,
          order: defaults.rail.order,
          shown: defaults.rail.shown,
          kinds: defaults.rail.kinds,
          pinnedFirst: defaults.rail.pinnedFirst,
          autoAdd: defaults.rail.autoAdd,
          limit: defaults.rail.limit,
        },
      },
    ],
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
      { key: 'downloadsInBatterySaver', value: defaults.downloadsInBatterySaver },
    ],
  },
  {
    id: 'browse',
    icon: Compass,
    screen: BrowseSettings,
    reset: resetKeys(
      'showNsfw',
      'explicitMusic',
      'contentOverrides',
      'failingSourcesLast',
      'browseStartTab',
      'manualBrowserChecks',
      'autoLoad',
      'userAgent',
      'discovery',
      'websites',
    ),
  },
  { id: 'privacy', icon: Shield, screen: PrivacySettings, reset: resetKeys('privacy') },
  // What Android lets Raygun do; nothing here is a setting to reset.
  { id: 'permissions', icon: ShieldCheck, screen: PermissionsSettings, only: 'phone' },
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
  | 'music'
  | 'video'
  | 'reader'
  | 'library'
  | 'quickAccess'
  | 'downloads'
  | 'browse'
  | 'privacy'
  | 'permissions'
  | 'data';

/** The category screens, routed at `/settings/<id>`. */
export const SETTINGS_SECTIONS = SETTINGS_ENTRIES.filter(
  (entry): entry is SettingsEntry & { id: SettingsSectionId; screen: FC } =>
    entry.screen !== undefined,
);

export const settingsEntry = (id: string | undefined) =>
  SETTINGS_ENTRIES.find((entry) => entry.id === id);
