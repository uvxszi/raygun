import {
  Bell,
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
  WifiOff,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import type { FC } from 'react';

import { SAVER_DEFAULTS } from '../network/dataSaver';
import { NetworkSettings } from '../network/OfflineSwitch';
import type { PlatformOnly } from '../shell/useIsDesktop';
import { DownloadSettings } from './DownloadSettings';
import { PermissionsSettings } from './PermissionsSettings';
import { resetAll, resetKeys } from './resets';
import type { SettingsReset } from './resets';
import { AiSettings } from './screens/AiSettings';
import { AppearanceSettings } from './screens/AppearanceSettings';
import { BrowseSettings } from './screens/BrowseSettings';
import { DataSettings } from './screens/DataSettings';
import { LibrarySettings } from './screens/LibrarySettings';
import { MusicPlayerSettings } from './screens/MusicPlayerSettings';
import { NotificationSettings } from './screens/NotificationSettings';
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
    reset: resetAll(
      resetKeys(
        'theme',
        'accent',
        'accentColor',
        'colors',
        'zoom',
        'textSize',
        'startScreen',
        'navLabels',
        'motion',
        'scrollbars',
        'windowFrame',
      ),
      // Of the fonts, only the interface's is this screen's.
      (current, defaults) => [
        { key: 'fonts', value: { ...current.fonts, interface: defaults.fonts?.interface ?? '' } },
      ],
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
    reset: resetAll(resetKeys('mangaReader', 'novelReader', 'dictionary'), (current, defaults) => [
      // The book reader's font of the device's goes too; the interface's stays.
      { key: 'fonts', value: { ...current.fonts, reading: defaults.fonts?.reading ?? '' } },
    ]),
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
      { key: 'downloadsPopover', value: defaults.downloadsPopover },
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
      'searchListings',
      'browseStartTab',
      'manualBrowserChecks',
      'runSourceCode',
      'autoLoad',
      'userAgent',
      'discovery',
      'websites',
    ),
  },
  {
    id: 'notifications',
    icon: Bell,
    screen: NotificationSettings,
    reset: resetKeys('notifications'),
  },
  { id: 'privacy', icon: Shield, screen: PrivacySettings, reset: resetKeys('privacy') },
  {
    id: 'network',
    icon: WifiOff,
    screen: NetworkSettings,
    // Data Saver goes back to its defaults; Offline Mode stays as it is.
    reset: (current, defaults) => [
      {
        key: 'network',
        value: {
          offline: current.network?.offline ?? false,
          dataSaver: defaults.network?.dataSaver ?? SAVER_DEFAULTS,
        },
      },
    ],
  },
  // What Android lets Raygun do; nothing here is a setting to reset.
  { id: 'permissions', icon: ShieldCheck, screen: PermissionsSettings, only: 'phone' },
  { id: 'devices', icon: Smartphone, to: '/settings/devices' },
  // Providers and models are what's on this device, not settings: they stay.
  { id: 'ai', icon: Sparkles, screen: AiSettings, reset: resetKeys('transcription', 'readerAi') },
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
  | 'network'
  | 'permissions'
  | 'ai'
  | 'data';

/** The category screens, routed at `/settings/<id>`. */
export const SETTINGS_SECTIONS = SETTINGS_ENTRIES.filter(
  (entry): entry is SettingsEntry & { id: SettingsSectionId; screen: FC } =>
    entry.screen !== undefined,
);

export const settingsEntry = (id: string | undefined) =>
  SETTINGS_ENTRIES.find((entry) => entry.id === id);
