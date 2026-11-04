/**
 * Every setting, for searching settings (Mihon's): its title's i18n key, the screen it's on, and
 * the group it sits in there. A result opens the screen and scrolls to the row (`useFoundPref`).
 * `only` marks rows a platform doesn't show.
 */
export type IndexedSetting = {
  /** The row's title key; the row is found by that title. */
  title: string;
  /** A Settings entry id (`sections.ts`) or a page's address. */
  at: string;
  /** The group's title key, for the result's second line. */
  group?: string;
  only?: 'desktop' | 'phone';
};

const on =
  (at: string, group?: string) =>
  (...titles: string[]): IndexedSetting[] =>
    titles.map((title) => ({ title, at, group }));

const player = (page: string) => `/settings/player/${page}`;

export const SETTINGS_INDEX: IndexedSetting[] = [
  ...on('appearance')(
    'settings.theme',
    'settings.accent',
    'settings.language',
    'settings.window.interfaceSize',
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

  ...on('player', 'settings.audioQuality.title')(
    'settings.audioQuality.streaming',
    'settings.audioQuality.cellular',
    'settings.audioQuality.download',
  ),
  ...on('player')('settings.resumeOnStart', 'settings.keepMusicPlaying'),
  ...on('player', 'player.view.musicTitle')(
    'player.view.layout',
    'player.view.width',
    'player.view.lyricsSize',
    'player.view.lyricsAlign',
    'player.view.background',
  ),
  ...on('player', 'settings.videoPlayback')(
    'settings.videoQuality',
    'settings.resumeVideos',
    'settings.watchedPercent',
    'settings.autoplayNext',
    'settings.nextPrompt',
    'settings.player.showFailedSources',
    'settings.player.showEmptySources',
  ),
  ...on(player('controls'), 'settings.player.section.controls.title')(
    'settings.player.orientation',
    'settings.player.aspect',
    'settings.player.controlsTimeout',
    'settings.player.showRemaining',
    'settings.player.rememberBrightness',
    'settings.player.defaultSpeed',
    'settings.player.speedPresets',
    'settings.player.holdSpeed',
  ),
  ...on(player('gestures'), 'settings.player.section.gestures.title')(
    'settings.player.brightnessGesture',
    'settings.player.volumeGesture',
    'settings.player.swapSides',
    'settings.player.seekGesture',
    'settings.player.doubleTapLeft',
    'settings.player.doubleTapCenter',
    'settings.player.doubleTapRight',
    'settings.seekSeconds',
    'settings.player.mediaKeys',
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
    'settings.player.ignoreEmbedded',
  ),
  ...on(player('audio'), 'settings.player.section.audio.title')(
    'settings.player.audioLanguage',
    'settings.player.audioFallback',
    'settings.player.pitchCorrection',
    'settings.player.volumeBoost',
  ),
  ...on(player('skip'), 'settings.player.section.skip.title')(
    'settings.player.skipButton',
    'settings.player.introSeconds',
    'settings.player.autoSkip',
    'settings.player.skipCountdown',
    'settings.player.aniskip',
  ),
  ...on(player('pip'), 'settings.player.section.pip.title')(
    'settings.player.pipOnLeave',
    'settings.player.pipOnBack',
  ),
  ...on(player('advanced'), 'settings.player.section.advanced.title')(
    'settings.player.decoder',
    'settings.player.titleTrailers',
    'settings.player.externalPlayer',
    'settings.player.externalPackage',
  ),

  ...on('sound', 'sound.equalizer')('sound.equalizer'),
  ...on('sound', 'sound.effects')('sound.bassBoost', 'sound.reverb', 'sound.balance', 'sound.mono'),
  ...on('sound', 'sound.volume')('sound.normalize', 'sound.level'),
  ...on('sound')('sound.crossfade'),

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
  { title: 'settings.quickAccess.autoAdd', at: 'library', only: 'desktop' },
  { title: 'settings.quickAccess.limit', at: 'library', only: 'desktop' },

  ...on('downloads')(
    'settings.saveLocation',
    'settings.chapterFormat',
    'songs.songFormat',
    'settings.pagesPerTitle',
    'settings.downloadsInBatterySaver',
  ),

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

  ...on('browse', 'settings.content')('settings.showNsfw', 'settings.explicitMusic'),
  ...on('browse', 'settings.sources')(
    'settings.extensions',
    'browse.repos',
    'watch.addons',
    'sources.failingLast',
    'settings.autoLoad.title',
    'settings.autoLoad.cooldown',
    'sources.userAgent',
  ),
  ...on('browse', 'discover.charts')('discover.region', 'discover.tagSongsTitle'),
  ...on('browse', 'websites.title')('websites.blockAds', 'websites.blockPopups', 'websites.list'),

  ...on('privacy')(
    'settings.privacy.history',
    'settings.privacy.recentlyPlayed',
    'settings.privacy.clearHistory',
  ),

  ...on('data', 'settings.backup')('settings.createBackup', 'settings.importBackup'),
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
  ...on('data', 'developer.title')('developer.add'),

  ...on('devices')(
    'link.sharing',
    'devices.deviceName',
    'devices.whatSyncs',
    'link.showCode',
    'link.scanCode',
  ),
  ...on('ai')('ai.translators', 'ai.transcription.title', 'ai.models', 'mcp.title'),
  ...on('about')('updater.check', 'settings.license'),
];
