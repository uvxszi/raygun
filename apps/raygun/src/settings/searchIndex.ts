/**
 * Every setting, for searching settings (Mihon's): its title's i18n key, the screen it's on, and
 * the group it sits in there. A result opens the screen and scrolls to the row (`useFoundPref`).
 * `only` marks rows a platform doesn't show.
 */
import type { PlatformOnly } from '../shell/useIsDesktop';
import { playerPagePath } from './playerOptions';
import type { PlayerSettingsPage } from './playerOptions';

export type IndexedSetting = {
  /** The row's title key; the row is found by that title. */
  title: string;
  /** A Settings entry id (`sections.ts`) or a page's address. */
  at: string;
  /** The group's title key, for the result's second line. */
  group?: string;
  only?: PlatformOnly;
};

const on =
  (at: string, group?: string) =>
  (...titles: string[]): IndexedSetting[] =>
    titles.map((title) => ({ title, at, group }));

/** The same rows, shown on one platform only. */
const onlyOn = (only: PlatformOnly, rows: IndexedSetting[]): IndexedSetting[] =>
  rows.map((row) => ({ ...row, only }));

const player = (page: PlayerSettingsPage) => playerPagePath(page);

export const SETTINGS_INDEX: IndexedSetting[] = [
  ...on('appearance')(
    'settings.theme',
    'settings.accent',
    'settings.language',
    'settings.window.interfaceSize',
    'settings.window.textSize',
    'settings.fonts.interface',
    'settings.startScreen',
    'settings.motion',
    'settings.scrollbars',
    'layout.title',
  ),
  { title: 'settings.navLabels', at: 'appearance', only: 'phone' },
  { title: 'settings.window.frame', at: 'appearance', only: 'desktop' },
  ...on('appearance', 'settings.colors.title')(
    'settings.colors.background',
    'settings.colors.surface',
    'settings.colors.foreground',
    'settings.colors.muted',
  ),

  ...on('music', 'settings.audioQuality.title')(
    'settings.audioQuality.streaming',
    'settings.audioQuality.cellular',
    'settings.audioQuality.download',
  ),
  ...on('music')('settings.resumeOnStart', 'settings.keepMusicPlaying'),
  ...on('music', 'player.view.musicTitle')(
    'player.view.layout',
    'player.view.width',
    'player.view.lyricsSize',
    'player.view.lyricsAlign',
    'player.view.background',
  ),
  { title: 'settings.nextUpRows', at: 'music', group: 'settings.musicQueue', only: 'desktop' },
  ...on('music', 'sound.equalizer')('sound.equalizer'),
  ...on('music', 'sound.effects')('sound.bassBoost', 'sound.reverb', 'sound.balance', 'sound.mono'),
  ...on('music', 'sound.volume')('sound.normalize', 'sound.level'),
  ...on('music')('sound.crossfade'),
  ...on('music', 'sponsorBlock.title')(
    'sponsorBlock.enabled',
    'sponsorBlock.categories.sponsor',
    'sponsorBlock.categories.selfpromo',
    'sponsorBlock.categories.interaction',
    'sponsorBlock.categories.intro',
    'sponsorBlock.categories.outro',
    'sponsorBlock.categories.preview',
    'sponsorBlock.categories.musicOfftopic',
    'sponsorBlock.categories.filler',
    'sponsorBlock.trimDownloads',
  ),
  ...on('music', 'sponsorBlock.youtube')('sponsorBlock.originalAudio'),

  ...on('video', 'settings.videoPlayback')(
    'settings.videoQuality',
    'settings.resumeVideos',
    'settings.watchedPercent',
    'settings.autoplayNext',
    'settings.nextPrompt',
    'settings.player.showFailedSources',
    'settings.player.showEmptySources',
  ),
  ...on('video', 'settings.torrents.title')(
    'settings.torrents.enabled',
    'settings.torrents.wifiOnly',
    'settings.torrents.cache',
    'settings.torrents.seed',
    'settings.torrents.dht',
    'settings.torrents.port',
  ),
  ...on(player('controls'), 'settings.player.section.controls.title')(
    'settings.player.aspect',
    'settings.player.controlsTimeout',
    'settings.player.showRemaining',
    'settings.player.defaultSpeed',
    'settings.player.speedPresets',
  ),
  // The phone player's own rows (Media3); the desktop player has none of them.
  ...onlyOn(
    'phone',
    on(player('controls'), 'settings.player.section.controls.title')(
      'settings.player.orientation',
      'settings.player.rememberBrightness',
      'settings.player.holdSpeed',
    ),
  ),
  ...onlyOn(
    'phone',
    on(player('gestures'), 'settings.player.section.gestures.title')(
      'settings.player.brightnessGesture',
      'settings.player.volumeGesture',
      'settings.player.swapSides',
      'settings.player.seekGesture',
      'settings.player.seekByDragging',
      'settings.player.doubleTapLeft',
      'settings.player.doubleTapCenter',
      'settings.player.doubleTapRight',
      'settings.seekSeconds',
      'settings.player.mediaKeys',
    ),
  ),
  ...on(player('subtitles'), 'settings.player.section.subtitles.title')(
    'settings.autoSubtitles',
    'settings.subtitleLanguage',
    'settings.player.subtitleFallback',
    'settings.player.preferWords',
    'settings.player.skipWords',
    'settings.player.subtitleSize',
    'settings.player.subtitleFont',
    'settings.player.textColor',
    'settings.player.backgroundColor',
    'settings.player.subtitlePosition',
    'settings.player.subtitleMaxWidth',
  ),
  ...onlyOn(
    'phone',
    on(
      player('subtitles'),
      'settings.player.section.subtitles.title',
    )('settings.player.ignoreEmbedded'),
  ),
  ...on(player('audio'), 'settings.player.section.audio.title')(
    'settings.player.audioLanguage',
    'settings.player.audioFallback',
    'settings.player.pitchCorrection',
  ),
  ...onlyOn(
    'phone',
    on(player('audio'), 'settings.player.section.audio.title')('settings.player.volumeBoost'),
  ),
  ...on(player('skip'), 'settings.player.section.skip.title')(
    'settings.player.skipButton',
    'settings.player.introSeconds',
    'settings.player.autoSkip',
    'settings.player.aniskip',
  ),
  ...onlyOn('phone', [
    ...on(player('skip'), 'settings.player.section.skip.title')('settings.player.skipCountdown'),
    ...on(player('pip'), 'settings.player.section.pip.title')(
      'settings.player.pipOnLeave',
      'settings.player.pipOnBack',
    ),
    ...on(player('advanced'), 'settings.player.section.advanced.title')(
      'settings.player.decoder',
      'settings.player.externalPlayer',
      'settings.player.externalPackage',
    ),
  ]),
  ...on(player('advanced'), 'settings.player.section.advanced.title')(
    'settings.player.videoCodec',
    'settings.player.titleTrailers',
  ),
  ...on(player('effects'), 'settings.player.effects.picture')(
    'settings.player.effects.look',
    'videoPlayer.effectBrightness',
    'videoPlayer.effectContrast',
    'videoPlayer.effectSaturation',
    'videoPlayer.effectGamma',
    'videoPlayer.effectHue',
    'videoPlayer.effectSharpen',
    'videoPlayer.effectDeflicker',
    'videoPlayer.effectDenoise',
  ),
  ...on(player('effects'), 'settings.player.effects.sound')(
    'videoPlayer.smartDialogue',
    'videoPlayer.smartNight',
    'videoPlayer.smartNormalize',
    'videoPlayer.smartLeveling',
  ),

  ...on('library')(
    'library.display.mode',
    'settings.coverWidth',
    'settings.browseCovers',
    'library.display.tabs',
    'library.display.showCounts',
    'library.display.perCategory',
    'library.display.pinsOnTop',
    'library.display.continueReading',
  ),
  ...on('library', 'settings.badges')(
    'settings.unreadBadges',
    'settings.downloadBadges',
    'library.display.localBadge',
  ),
  ...on('library', 'settings.libraryUpdates')('settings.mangaUpdates'),
  ...on('library', 'library.categories.title')(
    'library.categories.mangaTitle',
    'library.categories.novelTitle',
  ),
  ...on('library')('settings.musicFolders'),
  ...on('quickAccess')(
    'settings.quickAccess.shown',
    'settings.quickAccess.order',
    'settings.quickAccess.pinnedFirst',
  ),
  ...on('quickAccess', 'settings.quickAccess.listed')(
    'collections.favorites',
    'library.view.downloaded',
    'localFiles.title',
    'library.view.playlists',
    'library.view.albums',
    'library.view.artists',
    'library.view.podcasts',
    'library.view.audiobooks',
  ),
  ...on('quickAccess', 'settings.quickAccess.otherKinds')(
    'settings.quickAccess.autoAdd',
    'settings.quickAccess.limit',
  ),

  ...on('downloads')(
    'settings.saveLocation',
    'settings.chapterFormat',
    'songs.songFormat',
    'settings.pagesPerTitle',
  ),
  { title: 'settings.downloadsInBatterySaver', at: 'downloads', only: 'phone' },

  ...on('reader', 'reader.display.mangaTitle')(
    'reader.display.pagedScale',
    'reader.display.webtoonWidth',
    'reader.display.barWidth',
    'reader.display.preload',
  ),
  ...on('reader', 'settings.novelReader')(
    'settings.fontSize',
    'settings.lineSpacing',
    'settings.font',
    'settings.pageWidth',
    'settings.indent',
    'settings.paged',
    'settings.speechRate',
  ),
  ...on('reader', 'dictionary.settings.title')(
    'dictionary.settings.enabled',
    'dictionary.settings.sources',
    'dictionary.settings.language',
  ),

  ...on('browse', 'settings.content')(
    'settings.browseStartTab',
    'settings.showNsfw',
    'settings.explicitMusic',
  ),
  ...on('browse', 'settings.sources')(
    'settings.extensions',
    'sources.title',
    'sources.failingLast',
    'settings.searchListings.title',
    'settings.searchListings.entries',
    'settings.searchListings.pages',
    'settings.autoLoad.title',
    'settings.autoLoad.cooldown',
    'sources.userAgent',
  ),
  ...on('browse', 'answers.settings.title')('answers.settings.inAll', 'answers.settings.online'),
  ...on('browse', 'discover.charts')('discover.region', 'discover.tagSongsTitle'),
  ...on('browse', 'websites.title')('websites.blockAds', 'websites.blockPopups', 'websites.list'),

  ...on('notifications')(
    'settings.notifications.artistReleases',
    'settings.notifications.comics',
    'settings.notifications.books',
    'settings.notifications.videos',
    'settings.notifications.podcasts',
    'settings.notifications.audiobooks',
    'settings.notifications.appUpdates',
  ),

  ...on('privacy')(
    'settings.privacy.history',
    'settings.privacy.recentlyPlayed',
    'settings.privacy.clearHistory',
  ),

  ...on('network')('offline.title'),
  ...on('network', 'dataSaver.title')(
    'dataSaver.enabled',
    'dataSaver.mobileDataOnly',
    'dataSaver.limitVideo',
    'dataSaver.limitMusic',
    'dataSaver.fewerSources',
    'dataSaver.noPrefetch',
    'dataSaver.noBackgroundLookups',
    'dataSaver.smallerCovers',
    'dataSaver.noTrailers',
    'dataSaver.imagesWhenVisible',
  ),

  ...on('data', 'data.backups.title')(
    'settings.createBackup',
    'settings.importBackup',
    'data.backups.scan',
  ),
  ...on('data', 'data.usage.title')(
    'data.categories.pins',
    'data.categories.history',
    'data.categories.searches',
    'data.categories.aiModels',
    'data.categories.caches',
  ),
  ...on('data', 'data.erase.group')('data.erase.title'),
  ...on('data', 'storage.limits')(
    'storage.coverLimit',
    'storage.pageLimit',
    'storage.refresh',
    'storage.clearPagesOnStart',
  ),
  ...on('data', 'lyricsCache.title')(
    'lyricsCache.forget',
    'lyricsCache.keepFavorites',
    'lyricsCache.keepDownloaded',
    'lyricsCache.keepLocal',
  ),
  ...on('data', 'developer.title')('developer.remove'),

  ...on('devices')(
    'link.sharing',
    'devices.deviceName',
    'devices.whatSyncs',
    'link.showCode',
    'link.scanCode',
  ),
  ...on('ai')(
    'ai.translation.title',
    'ai.transcription.title',
    'ai.textReading.title',
    'ai.readAloud.title',
    'ai.providers.title',
    'ai.onDevice.title',
    'mcp.title',
  ),
  ...on('ai', 'ai.translation.title')(
    'ai.translation.comics',
    'ai.translation.books',
    'ai.translation.lyrics',
    'ai.translation.subtitles',
  ),
  ...on('ai', 'ai.transcription.title')('ai.transcription.engine'),
  ...on('ai', 'ai.textReading.title')('ai.textReading.reader', 'ai.textReading.import'),
  ...on('ai', 'ai.readAloud.title')(
    'ai.readAloud.engine',
    'ai.readAloud.pitch',
    'ai.readAloud.volume',
    'novel.readAloudOptions.detectLanguages',
    'novel.readAloudOptions.romanTitles',
    'ai.readAloud.preview',
  ),
  ...on('ai', 'ai.onDevice.title')('ai.onDevice.storage'),
  ...on('about')('updater.check', 'updater.autoCheck', 'settings.license'),
];
