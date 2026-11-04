import type {
  AiModelProgress,
  AppInfo,
  AppUpdate,
  ConnectRequest,
  ConnectStatus,
  EngineCommand,
  ExportJob,
  ExtensionInfo,
  PermissionPrompt,
  IncognitoStatus,
  HiddenItem,
  HiddenPrefs,
  LibraryScanProgress,
  LibraryStatus,
  LyricsTranslationProgress,
  MangaUpdateStatus,
  PlaybackState,
  QueueSnapshot,
  SearchProgress,
  Settings,
  SongDownload,
  SpeechCommand,
  StorageInfo,
  TrackDuration,
  TtsUpdate,
  VideoPlayerMessage,
  YouTubeVideo,
  YtDlpStatus,
} from '@raygun/bindings';
import { act } from '@testing-library/react';

import { SAVE_DEFAULTS } from '../export/saveDefaults';
import { CACHE_DEFAULTS } from '../settings/cacheDefaults';
import { LAYOUT_DEFAULTS } from '../layout/layout';
import { LIBRARY_DISPLAY_DEFAULTS } from '../manga/libraryDisplay';
import { PLAYER_DEFAULTS } from '../settings/playerOptions';

/**
 * Stand-in for `@raygun/bindings` (the Rust IPC layer). Use with
 * `vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock)`
 * and push Rust events into the UI with `emitEvent`.
 */

type EventPayloads = {
  connectEvent: ConnectStatus;
  connectRequestEvent: ConnectRequest;
  permissionPromptEvent: PermissionPrompt;
  openRouteEvent: string;
  engineCommandEvent: EngineCommand;
  playbackStateEvent: PlaybackState;
  queueEvent: QueueSnapshot;
  libraryScanEvent: LibraryScanProgress;
  extensionsChangedEvent: ExtensionInfo[];
  searchProgressEvent: SearchProgress;
  syncChangedEvent: number;
  mangaUpdateEvent: MangaUpdateStatus;
  mangaDownloadsEvent: string;
  ttsEvent: TtsUpdate;
  aiModelEvent: AiModelProgress;
  podcastsChangedEvent: string;
  audiobooksFoundEvent: [string, string];
  watchChangedEvent: string;
  videoDownloadsEvent: string;
  songDownloadsEvent: string;
  songDownloadProgressEvent: SongDownload;
  madeForYouEvent: null;
  trackDurationsEvent: TrackDuration[];
  importEvent: string;
  captionsEvent: string;
  lyricsTranslationEvent: LyricsTranslationProgress;
  localFilesEvent: string;
  settingsChangedEvent: string;
  incognitoEvent: IncognitoStatus;
  artistReleasesEvent: null;
  exportEvent: ExportJob | null;
  videoPlayerEvent: VideoPlayerMessage;
  youTubeVideoEvent: YouTubeVideo;
  speechCommandEvent: SpeechCommand;
  appUpdateEvent: AppUpdate;
  ytDlpEvent: string;
};
type EventName = keyof EventPayloads;
type Listener = (event: { payload: unknown }) => void;

const listeners = new Map<EventName, Set<Listener>>();

const makeEvent = (name: EventName) => ({
  listen: vi.fn((listener: Listener) => {
    const set = listeners.get(name) ?? new Set<Listener>();
    set.add(listener);
    listeners.set(name, set);
    return Promise.resolve(() => {
      set.delete(listener);
    });
  }),
});

/** Connect's status while this device plays here (M46.15). */
export const CONNECT_IDLE: ConnectStatus = {
  thisDevice: 'Pixel',
  session: null,
  asking: null,
  error: null,
};

export const IDLE_STATE: PlaybackState = {
  status: 'idle',
  currentItemId: null,
  positionMs: 0,
  durationMs: null,
  volume: 1,
  repeat: 'off',
  shuffle: false,
  error: null,
  sourceLabel: null,
  rate: 1,
  sleepAt: null,
  stopAfterCurrent: false,
};

export const EMPTY_QUEUE: QueueSnapshot = { items: [], order: [] };

/** Incognito off (M47). */
export const INCOGNITO_OFF: IncognitoStatus = { on: false, sinceMs: null };

export const ANDROID_INFO: AppInfo = {
  name: 'Raygun',
  version: '0.1.0',
  platform: 'android',
  arch: 'x86_64',
  debug: true,
};

export const MOBILE_STORAGE: StorageInfo = {
  mode: 'mobile',
  root: '/data/user/0/app.raygun.player',
  logs: '/data/user/0/app.raygun.player/logs',
  webview: '/data/user/0/app.raygun.player/webview',
};

export const DEFAULT_SETTINGS: Settings = {
  theme: 'system',
  language: 'en',
  showNsfw: false,
  explicitMusic: true,
  privacy: { history: true, recentlyPlayed: true, searchHistory: true },
  hidden: { artists: [], items: [] },
  lyricsCache: { forgetAfterDays: 30, keepFavorites: true, keepDownloaded: true, keepLocal: true },
  lyricsTranslation: { language: '', automatic: true, romanized: false, localModel: '' },
  preferredVideoQuality: null,
  accent: 'red',
  startScreen: 'home',
  navLabels: 'always',
  coverStyle: 'compact',
  unreadBadges: true,
  downloadBadges: true,
  resumeVideos: true,
  watchedPercent: 90,
  seekSeconds: 10,
  autoSubtitles: false,
  subtitleLanguage: '',
  autoplayNext: true,
  nextPromptSeconds: 10,
  videoPlayer: PLAYER_DEFAULTS,
  libraryDisplay: LIBRARY_DISPLAY_DEFAULTS,
  save: SAVE_DEFAULTS,
  cache: CACHE_DEFAULTS,
  websites: {
    blockAds: true,
    blockPopups: true,
    blocklist: 'hageziMulti',
    customListUrl: null,
    desktop: false,
  },
  layout: LAYOUT_DEFAULTS,
  accentColor: '#e5484d',
  coverSize: 0,
  mangaReader: { pagedScale: 'fitScreen', webtoonSize: 0 },
  playerView: { layout: 'auto', widthOffset: 0 },
  scrollbars: 'thin',
  motion: 'lively',
  trackTables: { tables: {} },
  panels: { railWidth: 0, sidePanelWidth: 0, playerSplit: 0, nextUpRows: 0 },
  colors: { background: '', surface: '', foreground: '', muted: '' },
  disabledSources: [],
  autoLoad: { enabled: true, cooldownMs: 1500 },
  windowFrame: 'app',
  zoom: 100,
  discovery: { region: '', tagSongs: true },
  failingSourcesLast: true,
  userAgent: '',
  keepMusicPlaying: false,
  resumeOnStart: true,
  audioQuality: { streaming: 'high', cellular: 'normal', download: 'veryHigh' },
  rail: { pins: [], order: 'custom', seeded: false },
  transcription: {
    engine: 'off',
    localModel: 'whisper-tiny',
    providerId: null,
    onlineModel: 'whisper-1',
    onlineConfirmed: false,
  },
  sound: {
    eqEnabled: false,
    eqPreset: 'flat',
    eqBands: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    preamp: 0,
    bassBoost: 0,
    reverb: 'off',
    balance: 0,
    mono: false,
    normalize: false,
    normalizeTarget: -14,
    crossfadeMs: 0,
  },
  mangaUpdateHours: 12,
  novelReader: {
    textSize: 0,
    lineSpacing: 16,
    font: 'serif',
    width: 'narrow',
    paged: false,
    ttsRate: 10,
    indent: false,
  },
};

export const IDLE_UPDATE: MangaUpdateStatus = {
  running: false,
  checked: 0,
  total: 0,
  newChapters: 0,
  failed: 0,
  lastRunAt: null,
};

export const EMPTY_LIBRARY: LibraryStatus = { roots: [], trackCount: 0, scanning: false };

export const bindingsMock = {
  commands: {
    appInfo: vi.fn(),
    musicCharts: vi.fn(),
    musicChart: vi.fn(),
    musicTrackTags: vi.fn(),
    musicTagsFor: vi.fn(),
    musicTagSummary: vi.fn(),
    musicTagSongs: vi.fn(),
    musicTagSources: vi.fn(),
    madeForYou: vi.fn(),
    madeForYouMix: vi.fn(),
    madeForYouSave: vi.fn(),
    hiddenArtistSet: vi.fn(),
    hiddenItemSet: vi.fn(),
    hiddenClear: vi.fn(),
    importReadLink: vi.fn(),
    importReadText: vi.fn(),
    importReaderReady: vi.fn(),
    importPrepareReader: vi.fn(),
    importReadScreenshots: vi.fn(),
    importMatch: vi.fn(),
    trackDurations: vi.fn(),
    coverSave: vi.fn(),
    detailEdit: vi.fn(),
    trackAssignSource: vi.fn(),
    trackUseSource: vi.fn(),
    trackImportStatus: vi.fn(),
    spokenItem: vi.fn(),
    captions: vi.fn(),
    captionsVideoTranscribe: vi.fn(),
    captionsVideoMake: vi.fn(),
    videoExternalPlayers: vi.fn(),
    videoFindSubtitles: vi.fn(),
    captionsTranscribe: vi.fn(),
    captionsStop: vi.fn(),
    captionsClear: vi.fn(),
    tracksUnavailable: vi.fn(),
    detailEditSet: vi.fn(),
    detailCoverImport: vi.fn(),
    detailCoverOptions: vi.fn(),
    localFolders: vi.fn(),
    localFolderAdd: vi.fn(),
    localFolderRemove: vi.fn(),
    localFoldersRescan: vi.fn(),
    localFilesAllowAccess: vi.fn(),
    importSave: vi.fn(),
    audiobooks: vi.fn(),
    audiobook: vi.fn(),
    audiobookOpen: vi.fn(),
    audiobookPlay: vi.fn(),
    audiobookDirectory: vi.fn(),
    audiobookDownload: vi.fn(),
    podcastContinueListening: vi.fn(),
    musicTagVocabulary: vi.fn(),
    songsDownload: vi.fn(),
    musicSearchArtists: vi.fn(),
    musicSearchAlbums: vi.fn(),
    musicAlbum: vi.fn(),
    musicArtistReleases: vi.fn(),
    musicFindAlbum: vi.fn(),
    audioOutputs: vi.fn(),
    audioOutputSet: vi.fn(),
    artistsFollowed: vi.fn(),
    artistFollow: vi.fn(),
    artistUnfollow: vi.fn(),
    artistSetAlerts: vi.fn(),
    artistReleaseNews: vi.fn(),
    artistReleasesCheck: vi.fn(),
    musicArtistEvents: vi.fn(),
    coverFallback: vi.fn(),
    musicSearchPlaylists: vi.fn(),
    musicPlaylist: vi.fn(),
    catalogOpenPage: vi.fn(),
    recentContexts: vi.fn(),
    recentContextsAdd: vi.fn(),
    lyricsOptions: vi.fn(),
    lyricsTranscribe: vi.fn(),
    lyricsTranscribeMany: vi.fn(),
    lyricsForget: vi.fn(),
    lyricsCacheUsage: vi.fn(),
    lyricsCacheClear: vi.fn(),
    lyricsTranslate: vi.fn(),
    lyricsTranslateOptions: vi.fn(),
    lyricsTranslationPick: vi.fn(),
    songDownloadCancelRecording: vi.fn(),
    songDownloadUseSource: vi.fn(),
    youtubeLink: vi.fn(),
    youtubePlay: vi.fn(),
    ytdlpStatus: vi.fn(),
    ytdlpInstall: vi.fn(),
    ytdlpInstallFfmpeg: vi.fn(),
    ytdlpDownload: vi.fn(),
    ytdlpJobs: vi.fn(),
    ytdlpCancel: vi.fn(),
    ytdlpReveal: vi.fn(),
    devRemoveTestSources: vi.fn(),
    appUpdate: vi.fn(),
    appUpdateInstall: vi.fn(),
    appOpenPage: vi.fn(),
    connectLink: vi.fn(),
    devicesSetSharing: vi.fn(),
    captionsUseFeed: vi.fn(),
    songDownloads: vi.fn(),
    songDownload: vi.fn(),
    songDownloadRemove: vi.fn(),
    songDownloadsRemove: vi.fn(),
    songDownloadRetry: vi.fn(),
    songDownloadsRetryFailed: vi.fn(),
    songDownloadsReorder: vi.fn(),
    songDownloadsPaused: vi.fn(),
    songDownloadsSetPaused: vi.fn(),
    songDownloadRecord: vi.fn(),
    songsSave: vi.fn(),
    songDownloadedKeys: vi.fn(),
    songDownloadFor: vi.fn(),
    downloadedSongs: vi.fn(),
    videoPlayerReport: vi.fn(),
    ttsReport: vi.fn(),
    ttsSetAvailable: vi.fn(),
    videoPlayerExternal: vi.fn(),
    storageInfo: vi.fn(),
    settingsGet: vi.fn(),
    settingsUpdate: vi.fn(),
    settingsUpdateMany: vi.fn(),
    settingsDefaults: vi.fn(),
    playbackGetState: vi.fn(),
    playbackGetQueue: vi.fn(),
    playbackEnqueue: vi.fn(),
    playbackControl: vi.fn(),
    playbackEngineReport: vi.fn(),
    libraryStatus: vi.fn(),
    libraryConnectDeviceMusic: vi.fn(),
    libraryAddRoot: vi.fn(),
    libraryRemoveRoot: vi.fn(),
    libraryRescan: vi.fn(),
    libraryAlbums: vi.fn(),
    libraryArtists: vi.fn(),
    libraryAlbumTracks: vi.fn(),
    libraryTracks: vi.fn(),
    libraryPlay: vi.fn(),
    playbackReplace: vi.fn(),
    extensionsList: vi.fn(),
    extensionsReload: vi.fn(),
    extensionClearData: vi.fn(),
    miniplayerOpen: vi.fn(),
    miniplayerClose: vi.fn(),
    extensionOpenWebsite: vi.fn(),
    extensionRepos: vi.fn(),
    extensionRepoAdd: vi.fn(),
    extensionRepoPreview: vi.fn(),
    extensionSuggestedRepos: vi.fn(),
    extensionRepoRemove: vi.fn(),
    extensionCatalog: vi.fn(),
    extensionInstall: vi.fn(),
    extensionUninstall: vi.fn(),
    extensionSettingSet: vi.fn(),
    onlineSearch: vi.fn(),
    searchRecent: vi.fn(),
    searchRecentAdd: vi.fn(),
    searchRecentRemove: vi.fn(),
    searchRecentClear: vi.fn(),
    lyricsGet: vi.fn(),
    songEditGet: vi.fn(),
    songEditSet: vi.fn(),
    favoritesList: vi.fn(),
    favoriteGet: vi.fn(),
    collectionsSaves: vi.fn(),
    collectionsSavedIndex: vi.fn(),
    collectionsSetSaves: vi.fn(),
    musicArtist: vi.fn(),
    musicArtistInfo: vi.fn(),
    musicSongCredits: vi.fn(),
    musicOpenInfoPage: vi.fn(),
    playbackRestore: vi.fn(),
    devAddTestSources: vi.fn(),
    podcastQueue: vi.fn(),
    connectStatus: vi.fn(),
    connectDevices: vi.fn(),
    connectAdd: vi.fn(),
    connectTo: vi.fn(),
    connectDisconnect: vi.fn(),
    connectForget: vi.fn(),
    connectAnswer: vi.fn(),
    playbackQueueTracks: vi.fn(),
    favoriteSet: vi.fn(),
    favoritesSetMany: vi.fn(),
    playlistsList: vi.fn(),
    playlistGet: vi.fn(),
    playlistCreate: vi.fn(),
    playlistRename: vi.fn(),
    playlistDelete: vi.fn(),
    playlistAdd: vi.fn(),
    playlistRemoveItem: vi.fn(),
    playlistMoveItem: vi.fn(),
    historyRecent: vi.fn(),
    historyClear: vi.fn(),
    incognitoStatus: vi.fn(),
    incognitoSetEnabled: vi.fn(),
    collectionPlay: vi.fn(),
    mcpStatus: vi.fn(),
    mcpSetEnabled: vi.fn(),
    mcpNewToken: vi.fn(),
    remoteStatus: vi.fn(),
    remoteSetEnabled: vi.fn(),
    remoteNewCode: vi.fn(),
    remoteForgetDevice: vi.fn(),
    syncStatus: vi.fn(),
    syncSetEnabled: vi.fn(),
    syncOpenPairing: vi.fn(),
    syncJoin: vi.fn(),
    syncNow: vi.fn(),
    syncRemovePeer: vi.fn(),
    syncSetKinds: vi.fn(),
    syncSetDeviceName: vi.fn(),
    syncUndoLast: vi.fn(),
    mangaSources: vi.fn(),
    mangaBrowse: vi.fn(),
    mangaTitle: vi.fn(),
    mangaSetInLibrary: vi.fn(),
    mangaLibrary: vi.fn(),
    mangaChapter: vi.fn(),
    mangaSetProgress: vi.fn(),
    mangaSetRead: vi.fn(),
    mangaSetReaderMode: vi.fn(),
    mangaUpdates: vi.fn(),
    mangaUpdateLibrary: vi.fn(),
    mangaUpdateStatus: vi.fn(),
    mangaDownloads: vi.fn(),
    mangaDownload: vi.fn(),
    mangaRemoveDownloads: vi.fn(),
    mangaSetDownloadsPaused: vi.fn(),
    backupCreate: vi.fn(),
    backupList: vi.fn(),
    backupRestore: vi.fn(),
    backupDelete: vi.fn(),
    backupImport: vi.fn(),
    novelSources: vi.fn(),
    novelBrowse: vi.fn(),
    novelTitle: vi.fn(),
    novelSetInLibrary: vi.fn(),
    novelLibrary: vi.fn(),
    novelChapter: vi.fn(),
    novelSetProgress: vi.fn(),
    novelSetRead: vi.fn(),
    novelImportEpub: vi.fn(),
    novelImportEpubUpload: vi.fn(),
    ttsAvailable: vi.fn(),
    ttsSpeak: vi.fn(),
    ttsStop: vi.fn(),
    aiProviders: vi.fn(),
    aiProviderSave: vi.fn(),
    aiProviderDelete: vi.fn(),
    aiProviderTest: vi.fn(),
    aiModels: vi.fn(),
    aiModelDownload: vi.fn(),
    aiModelDelete: vi.fn(),
    podcastSubscriptions: vi.fn(),
    podcastLatest: vi.fn(),
    podcastOpenFeed: vi.fn(),
    podcastGet: vi.fn(),
    podcastSetSubscribed: vi.fn(),
    podcastRefreshAll: vi.fn(),
    podcastSearch: vi.fn(),
    podcastTop: vi.fn(),
    podcastPlay: vi.fn(),
    podcastSetPlayed: vi.fn(),
    podcastSetAllPlayed: vi.fn(),
    podcastSetEarlierPlayed: vi.fn(),
    podcastForgetPosition: vi.fn(),
    podcastSetEpisodeOrder: vi.fn(),
    podcastSetSpeed: vi.fn(),
    podcastDownload: vi.fn(),
    podcastRemoveDownload: vi.fn(),
    podcastRemoveDownloads: vi.fn(),
    podcastCancelDownload: vi.fn(),
    podcastOpenPage: vi.fn(),
    videoPlay: vi.fn(),
    videoPlayLink: vi.fn(),
    videoContinueWatching: vi.fn(),
    videoResume: vi.fn(),
    videoRemove: vi.fn(),
    libraryCategories: vi.fn(),
    libraryCategoryCreate: vi.fn(),
    libraryCategoryRename: vi.fn(),
    libraryCategoryDelete: vi.fn(),
    libraryCategoryReorder: vi.fn(),
    librarySetTitleCategories: vi.fn(),
    libraryTitleMarks: vi.fn(),
    librarySetList: vi.fn(),
    librarySetFavorite: vi.fn(),
    librarySetPinned: vi.fn(),
    libraryMarkRead: vi.fn(),
    librarySetChaptersRead: vi.fn(),
    libraryMarkPreviousRead: vi.fn(),
    libraryRemove: vi.fn(),
    libraryChangeCategories: vi.fn(),
    libraryDownloadNext: vi.fn(),
    libraryContinue: vi.fn(),
    libraryDuplicates: vi.fn(),
    librarySetCategoryDisplay: vi.fn(),
    libraryUpdateCategory: vi.fn(),
    uploadBegin: vi.fn(),
    uploadAppend: vi.fn(),
    uploadCancel: vi.fn(),
    mangaImportComic: vi.fn(),
    savePickFolder: vi.fn(),
    songsPickFolder: vi.fn(),
    saveFolderState: vi.fn(),
    exportPlan: vi.fn(),
    exportStart: vi.fn(),
    exportCancel: vi.fn(),
    exportDismiss: vi.fn(),
    exportJobs: vi.fn(),
    pageSave: vi.fn(),
    pageShare: vi.fn(),
    pageCopy: vi.fn(),
    storageUsage: vi.fn(),
    storageClear: vi.fn(),
    websiteList: vi.fn(),
    websiteAdd: vi.fn(),
    websiteEdit: vi.fn(),
    websiteRemove: vi.fn(),
    websiteReset: vi.fn(),
    websiteSetBlockAds: vi.fn(),
    websiteOpen: vi.fn(),
    websiteBlocklistStatus: vi.fn(),
    websiteBlocklistUpdate: vi.fn(),
    websiteClearData: vi.fn(),
    videoHistory: vi.fn(),
    videoSources: vi.fn(),
    videoBrowse: vi.fn(),
    videoTitle: vi.fn(),
    videoServers: vi.fn(),
    videoLinks: vi.fn(),
    videoSourcePlay: vi.fn(),
    videoSetEpisodeWatched: vi.fn(),
    videoSetInLibrary: vi.fn(),
    videoLibrary: vi.fn(),
    videoUpdates: vi.fn(),
    videoUpcoming: vi.fn(),
    stremioSetInLibrary: vi.fn(),
    videoDownloads: vi.fn(),
    videoSourceDownload: vi.fn(),
    stremioDownload: vi.fn(),
    videoDownloadRetry: vi.fn(),
    videoDownloadRemove: vi.fn(),
    videoDownloadPlay: vi.fn(),
    videoDownloadsSetPaused: vi.fn(),
    videoSourceWatch: vi.fn(),
    stremioWatch: vi.fn(),
    videoSetPlayerLabels: vi.fn(),
    stremioAddons: vi.fn(),
    stremioSuggestions: vi.fn(),
    stremioPreview: vi.fn(),
    stremioAdd: vi.fn(),
    stremioRemove: vi.fn(),
    stremioSetEnabled: vi.fn(),
    stremioSetOrder: vi.fn(),
    stremioConfigure: vi.fn(),
    stremioOpenLink: vi.fn(),
    stremioCatalogs: vi.fn(),
    stremioHome: vi.fn(),
    stremioCatalog: vi.fn(),
    stremioSearch: vi.fn(),
    stremioSearchCatalog: vi.fn(),
    stremioMeta: vi.fn(),
    stremioStreams: vi.fn(),
    stremioPlay: vi.fn(),
    stremioSetWatched: vi.fn(),
    localNetworkAccess: vi.fn(),
    localNetworkRequest: vi.fn(),
    localNetworkOpenSettings: vi.fn(),
    permissionsList: vi.fn(),
    permissionRequest: vi.fn(),
    permissionOpenSettings: vi.fn(),
    permissionPromptAnswer: vi.fn(),
    permissionPromptPending: vi.fn(),
    notificationRouteTake: vi.fn(),
    catalogFilters: vi.fn(),
    catalogSearch: vi.fn(),
    catalogGlobalSearch: vi.fn(),
    librarySearch: vi.fn(),
  },
  events: {
    connectEvent: makeEvent('connectEvent'),
    connectRequestEvent: makeEvent('connectRequestEvent'),
    permissionPromptEvent: makeEvent('permissionPromptEvent'),
    openRouteEvent: makeEvent('openRouteEvent'),
    engineCommandEvent: makeEvent('engineCommandEvent'),
    playbackStateEvent: makeEvent('playbackStateEvent'),
    queueEvent: makeEvent('queueEvent'),
    libraryScanEvent: makeEvent('libraryScanEvent'),
    extensionsChangedEvent: makeEvent('extensionsChangedEvent'),
    searchProgressEvent: makeEvent('searchProgressEvent'),
    syncChangedEvent: makeEvent('syncChangedEvent'),
    mangaUpdateEvent: makeEvent('mangaUpdateEvent'),
    mangaDownloadsEvent: makeEvent('mangaDownloadsEvent'),
    ttsEvent: makeEvent('ttsEvent'),
    aiModelEvent: makeEvent('aiModelEvent'),
    podcastsChangedEvent: makeEvent('podcastsChangedEvent'),
    audiobooksFoundEvent: makeEvent('audiobooksFoundEvent'),
    watchChangedEvent: makeEvent('watchChangedEvent'),
    videoDownloadsEvent: makeEvent('videoDownloadsEvent'),
    songDownloadsEvent: makeEvent('songDownloadsEvent'),
    songDownloadProgressEvent: makeEvent('songDownloadProgressEvent'),
    madeForYouEvent: makeEvent('madeForYouEvent'),
    trackDurationsEvent: makeEvent('trackDurationsEvent'),
    importEvent: makeEvent('importEvent'),
    captionsEvent: makeEvent('captionsEvent'),
    lyricsTranslationEvent: makeEvent('lyricsTranslationEvent'),
    localFilesEvent: makeEvent('localFilesEvent'),
    settingsChangedEvent: makeEvent('settingsChangedEvent'),
    incognitoEvent: makeEvent('incognitoEvent'),
    artistReleasesEvent: makeEvent('artistReleasesEvent'),
    exportEvent: makeEvent('exportEvent'),
    videoPlayerEvent: makeEvent('videoPlayerEvent'),
    youTubeVideoEvent: makeEvent('youTubeVideoEvent'),
    speechCommandEvent: makeEvent('speechCommandEvent'),
    appUpdateEvent: makeEvent('appUpdateEvent'),
    ytDlpEvent: makeEvent('ytDlpEvent'),
  },
};

/** yt-dlp on a phone: not there (M46.47). */
const YTDLP_OFF: YtDlpStatus = {
  supported: false,
  version: null,
  ffmpeg: false,
  installing: false,
  message: null,
};

/** No release newer than this copy (M46.47). */
export const NO_UPDATE: AppUpdate = {
  current: '1.0.0',
  latest: null,
  available: false,
  notes: null,
  pageUrl: 'https://github.com/uvxszi/raygun/releases/latest',
  repoUrl: 'https://github.com/uvxszi/raygun',
  installKind: 'android',
  assetName: null,
  assetSize: null,
  checkedAt: null,
  state: 'idle',
  progress: null,
  message: null,
};

/** Default answers for every command, as an idle app on Android would give. */
export const seedBindings = () => {
  listeners.clear();
  const { commands } = bindingsMock;
  commands.appInfo.mockResolvedValue(ANDROID_INFO);
  commands.videoPlayerReport.mockResolvedValue(null);
  commands.ttsReport.mockResolvedValue(null);
  commands.ttsSetAvailable.mockResolvedValue(null);
  commands.videoPlayerExternal.mockResolvedValue({ status: 'ok', data: 'mpv' });
  commands.storageInfo.mockResolvedValue(MOBILE_STORAGE);
  // Saved changes are kept and read back, as Rust keeps them; tests may answer otherwise.
  let stored: Settings = DEFAULT_SETTINGS;
  commands.settingsGet.mockImplementation(async () => ({ status: 'ok', data: stored }));
  const applied = async (changes: { key: string; value: unknown }[]) => {
    const current = (await commands.settingsGet()) as { data: Settings } | undefined;
    stored = changes.reduce<Settings>(
      (next, change) => ({ ...next, [change.key]: change.value }),
      current?.data ?? DEFAULT_SETTINGS,
    );
    return { status: 'ok' as const, data: stored };
  };
  commands.settingsUpdate.mockImplementation((change: { key: string; value: unknown }) =>
    applied([change]),
  );
  commands.settingsUpdateMany.mockImplementation((changes: { key: string; value: unknown }[]) =>
    applied(changes),
  );
  commands.settingsDefaults.mockResolvedValue(DEFAULT_SETTINGS);
  commands.playbackGetState.mockResolvedValue(IDLE_STATE);
  commands.playbackGetQueue.mockResolvedValue(EMPTY_QUEUE);
  commands.playbackEnqueue.mockResolvedValue(undefined);
  commands.playbackControl.mockResolvedValue(undefined);
  commands.playbackEngineReport.mockResolvedValue(undefined);
  const ok = <T>(data: T) => ({ status: 'ok' as const, data });
  commands.musicCharts.mockResolvedValue(ok({ charts: [], errors: [] }));
  commands.musicChart.mockResolvedValue(ok([]));
  commands.musicTrackTags.mockResolvedValue(ok([]));
  commands.musicTagsFor.mockResolvedValue(ok([]));
  commands.musicTagSummary.mockResolvedValue(ok([]));
  commands.musicTagSongs.mockResolvedValue(ok([]));
  commands.musicTagSources.mockResolvedValue(ok({ rows: [], failures: [] }));
  commands.madeForYou.mockResolvedValue(
    ok({
      mixes: [],
      stats: {
        playsThisWeek: 0,
        minutesThisWeek: 0,
        topArtist: null,
        topSong: null,
        peakHour: null,
        songsKnown: 0,
      },
      madeAt: null,
      making: false,
    }),
  );
  commands.madeForYouSave.mockResolvedValue(ok('playlist-1'));
  // Hiding saves into the settings, as Rust does, and answers with them.
  const hide = async (change: (hidden: HiddenPrefs) => HiddenPrefs) => {
    const answer = (await commands.settingsGet()) as { data?: Settings } | undefined;
    const current = answer?.data ?? DEFAULT_SETTINGS;
    const next = { ...current, hidden: change(current.hidden) };
    commands.settingsGet.mockResolvedValue(ok(next));
    return ok(next);
  };
  const same = (left: string, right: string) => left.toLowerCase() === right.toLowerCase();
  commands.hiddenArtistSet.mockImplementation((name: string, hidden: boolean) =>
    hide((prefs) => ({
      ...prefs,
      artists: [
        ...prefs.artists.filter((artist) => !same(artist.name, name)),
        ...(hidden ? [{ name, hiddenAt: 1 }] : []),
      ],
    })),
  );
  commands.hiddenItemSet.mockImplementation((item: HiddenItem, hidden: boolean) =>
    hide((prefs) => ({
      ...prefs,
      items: [...prefs.items.filter((other) => other.key !== item.key), ...(hidden ? [item] : [])],
    })),
  );
  commands.hiddenClear.mockImplementation((artists: boolean, items: boolean) =>
    hide((prefs) => ({
      artists: artists ? [] : prefs.artists,
      items: items ? [] : prefs.items,
    })),
  );
  commands.importReaderReady.mockResolvedValue(true);
  commands.importMatch.mockResolvedValue(ok([]));
  commands.trackDurations.mockResolvedValue(ok([]));
  commands.coverSave.mockResolvedValue(ok('Pictures/Raygun/Cover.png'));
  commands.detailEdit.mockResolvedValue(
    ok({ title: null, artist: null, album: null, artworkUrl: null }),
  );
  commands.tracksUnavailable.mockResolvedValue(ok([]));
  commands.trackAssignSource.mockResolvedValue(ok(null));
  commands.trackUseSource.mockResolvedValue(ok(null));
  commands.trackImportStatus.mockResolvedValue(ok({ imported: false, unavailable: false }));
  commands.spokenItem.mockResolvedValue(ok(null));
  commands.captionsTranscribe.mockResolvedValue(ok(null));
  commands.captionsStop.mockResolvedValue(null);
  commands.captionsClear.mockResolvedValue(ok(null));
  commands.captions.mockImplementation(async (episodeId: string) =>
    ok({
      episodeId,
      cues: [],
      source: null,
      covered: [],
      complete: false,
      running: false,
      preparing: false,
      error: null,
      canTranscribe: false,
      engine: 'off',
    }),
  );
  commands.captionsVideoTranscribe.mockResolvedValue(ok(null));
  commands.captionsVideoMake.mockResolvedValue(ok(null));
  commands.videoExternalPlayers.mockResolvedValue([]);
  commands.videoFindSubtitles.mockResolvedValue(ok({ tracks: [], suggestion: null, note: null }));
  commands.detailEditSet.mockImplementation(async (_target: unknown, edit: unknown) => ok(edit));
  commands.localFolders.mockResolvedValue(ok([]));
  commands.localFolderAdd.mockResolvedValue(ok(null));
  commands.localFolderRemove.mockResolvedValue(ok([]));
  commands.localFoldersRescan.mockResolvedValue(ok(null));
  commands.localFilesAllowAccess.mockResolvedValue(ok(true));
  commands.audiobooks.mockResolvedValue(ok([]));
  commands.audiobookDirectory.mockResolvedValue(ok({ books: [], hasMore: false }));
  commands.audiobookPlay.mockResolvedValue(ok(null));
  commands.audiobookDownload.mockResolvedValue(ok(0));
  commands.podcastContinueListening.mockResolvedValue(ok([]));
  commands.musicTagVocabulary.mockResolvedValue([]);
  commands.songsDownload.mockResolvedValue(ok(null));
  commands.musicSearchArtists.mockResolvedValue(ok([]));
  commands.musicSearchAlbums.mockResolvedValue(ok([]));
  commands.musicArtistReleases.mockResolvedValue(ok([]));
  commands.musicFindAlbum.mockResolvedValue(ok(null));
  commands.audioOutputs.mockResolvedValue({ outputs: [], chosen: null });
  commands.audioOutputSet.mockResolvedValue(ok(null));
  commands.artistsFollowed.mockResolvedValue(ok([]));
  commands.artistFollow.mockResolvedValue(ok(null));
  commands.artistUnfollow.mockResolvedValue(ok(null));
  commands.artistSetAlerts.mockResolvedValue(ok(null));
  commands.artistReleaseNews.mockResolvedValue(ok([]));
  commands.artistReleasesCheck.mockResolvedValue(ok([]));
  commands.musicArtistEvents.mockResolvedValue(ok([]));
  commands.coverFallback.mockResolvedValue(ok(null));
  commands.musicSearchPlaylists.mockResolvedValue(ok([]));
  commands.catalogOpenPage.mockResolvedValue(ok(null));
  commands.recentContexts.mockResolvedValue(ok([]));
  commands.recentContextsAdd.mockResolvedValue(ok(null));
  commands.detailCoverOptions.mockResolvedValue(ok([]));
  commands.lyricsOptions.mockResolvedValue(ok([]));
  commands.lyricsTranscribe.mockResolvedValue(ok('song:|'));
  commands.lyricsTranscribeMany.mockResolvedValue(ok(null));
  commands.lyricsForget.mockResolvedValue(ok(null));
  commands.lyricsCacheUsage.mockResolvedValue(ok({ songs: 0, transcriptions: 0, translations: 0 }));
  commands.lyricsCacheClear.mockResolvedValue(ok(0));
  commands.lyricsTranslateOptions.mockResolvedValue(ok([]));
  commands.lyricsTranslationPick.mockResolvedValue(ok(null));
  commands.songDownloadCancelRecording.mockResolvedValue(null);
  commands.songDownloadUseSource.mockResolvedValue(ok(null));
  commands.youtubeLink.mockResolvedValue(ok(null));
  commands.youtubePlay.mockResolvedValue(ok(null));
  commands.ytdlpStatus.mockResolvedValue(ok(YTDLP_OFF));
  commands.ytdlpInstall.mockResolvedValue(ok(YTDLP_OFF));
  commands.ytdlpInstallFfmpeg.mockResolvedValue(ok(YTDLP_OFF));
  commands.ytdlpDownload.mockResolvedValue(ok('job'));
  commands.ytdlpJobs.mockResolvedValue([]);
  commands.ytdlpCancel.mockResolvedValue(null);
  commands.ytdlpReveal.mockResolvedValue(ok(null));
  commands.devRemoveTestSources.mockResolvedValue(ok(0));
  commands.appUpdate.mockResolvedValue(ok(NO_UPDATE));
  commands.appUpdateInstall.mockResolvedValue(ok(NO_UPDATE));
  commands.appOpenPage.mockResolvedValue(ok(null));
  commands.connectLink.mockResolvedValue(ok([]));
  commands.devicesSetSharing.mockResolvedValue(ok(null));
  commands.captionsUseFeed.mockResolvedValue(ok(null));
  commands.songDownloads.mockResolvedValue(ok([]));
  commands.songDownload.mockResolvedValue(ok(null));
  commands.songDownloadRemove.mockResolvedValue(ok(null));
  commands.songDownloadsRemove.mockResolvedValue(ok(null));
  commands.songDownloadRetry.mockResolvedValue(ok(null));
  commands.songDownloadsRetryFailed.mockResolvedValue(ok(0));
  commands.songDownloadsReorder.mockResolvedValue(ok(null));
  commands.songDownloadsPaused.mockResolvedValue(false);
  commands.songDownloadsSetPaused.mockResolvedValue(null);
  commands.songDownloadRecord.mockResolvedValue(ok(null));
  commands.songsSave.mockResolvedValue(ok({ saved: [], failed: [] }));
  commands.songDownloadedKeys.mockResolvedValue([]);
  commands.songDownloadFor.mockResolvedValue(ok(null));
  commands.downloadedSongs.mockResolvedValue(ok([]));
  commands.libraryStatus.mockResolvedValue(ok(EMPTY_LIBRARY));
  commands.libraryConnectDeviceMusic.mockResolvedValue(ok(EMPTY_LIBRARY));
  commands.libraryAddRoot.mockResolvedValue(ok(EMPTY_LIBRARY));
  commands.libraryRemoveRoot.mockResolvedValue(ok(EMPTY_LIBRARY));
  commands.libraryRescan.mockResolvedValue(undefined);
  commands.libraryAlbums.mockResolvedValue(ok([]));
  commands.libraryArtists.mockResolvedValue(ok([]));
  commands.libraryAlbumTracks.mockResolvedValue(ok([]));
  commands.libraryTracks.mockResolvedValue(ok([]));
  commands.libraryPlay.mockResolvedValue(ok(null));
  commands.playbackReplace.mockResolvedValue(undefined);
  commands.extensionsList.mockResolvedValue(ok([]));
  commands.extensionsReload.mockResolvedValue(ok([]));
  commands.extensionClearData.mockResolvedValue(ok([]));
  commands.miniplayerOpen.mockResolvedValue(ok(null));
  commands.miniplayerClose.mockResolvedValue(ok(null));
  commands.extensionOpenWebsite.mockResolvedValue(ok(null));
  commands.extensionRepos.mockResolvedValue(ok([]));
  commands.extensionRepoAdd.mockResolvedValue(ok([]));
  commands.extensionRepoPreview.mockResolvedValue({ status: 'error', error: 'not mocked' });
  commands.extensionSuggestedRepos.mockResolvedValue(ok([]));
  commands.extensionRepoRemove.mockResolvedValue(ok([]));
  commands.extensionCatalog.mockResolvedValue(ok([]));
  commands.extensionInstall.mockResolvedValue(ok([]));
  commands.extensionUninstall.mockResolvedValue(ok([]));
  commands.extensionSettingSet.mockResolvedValue(ok([]));
  commands.onlineSearch.mockResolvedValue(ok({ library: [], top: [], providers: [] }));
  commands.searchRecent.mockResolvedValue(ok([]));
  commands.searchRecentAdd.mockResolvedValue(ok([]));
  commands.searchRecentRemove.mockResolvedValue(ok([]));
  commands.searchRecentClear.mockResolvedValue(ok(null));
  commands.lyricsGet.mockResolvedValue(ok(null));
  commands.songEditGet.mockResolvedValue({});
  commands.songEditSet.mockImplementation(async (_query: unknown, edit: unknown) => ok(edit));
  commands.favoritesList.mockResolvedValue(ok([]));
  commands.favoriteGet.mockResolvedValue(ok(false));
  commands.collectionsSaves.mockResolvedValue(ok({ favorite: false, playlistIds: [] }));
  commands.collectionsSavedIndex.mockResolvedValue(ok([]));
  commands.collectionsSetSaves.mockResolvedValue(ok(null));
  commands.musicArtist.mockResolvedValue(ok({ name: '', popular: [], albums: [], tracks: [] }));
  commands.musicArtistInfo.mockResolvedValue(ok(null));
  commands.musicSongCredits.mockResolvedValue(ok(null));
  commands.musicOpenInfoPage.mockResolvedValue(ok(null));
  commands.playbackRestore.mockResolvedValue({ status: 'ok', data: null });
  commands.devAddTestSources.mockResolvedValue({
    status: 'ok',
    data: { added: [], already: [], failed: [] },
  });
  commands.podcastQueue.mockResolvedValue({ status: 'ok', data: null });
  commands.connectStatus.mockResolvedValue(CONNECT_IDLE);
  commands.connectDevices.mockResolvedValue(ok([]));
  commands.connectAdd.mockResolvedValue({ status: 'error', error: 'not mocked' });
  commands.connectTo.mockResolvedValue(ok(CONNECT_IDLE));
  commands.connectDisconnect.mockResolvedValue(ok(CONNECT_IDLE));
  commands.connectForget.mockResolvedValue(ok([]));
  commands.connectAnswer.mockResolvedValue(null);
  commands.favoriteSet.mockResolvedValue(ok(null));
  commands.favoritesSetMany.mockResolvedValue(ok(null));
  commands.playlistsList.mockResolvedValue(ok([]));
  commands.playlistGet.mockResolvedValue(
    ok({ playlist: { id: 'p', name: 'Playlist', itemCount: 0, artworkUrls: [] }, items: [] }),
  );
  commands.playlistCreate.mockResolvedValue(
    ok({ id: 'new', name: 'New', itemCount: 0, artworkUrls: [] }),
  );
  commands.playlistRename.mockResolvedValue(ok(null));
  commands.playlistDelete.mockResolvedValue(ok(null));
  commands.playlistAdd.mockImplementation(async (_id: string, tracks: unknown[]) =>
    ok({ added: tracks.length, already: 0 }),
  );
  commands.playlistRemoveItem.mockResolvedValue(ok(null));
  commands.playlistMoveItem.mockResolvedValue(ok(null));
  commands.historyRecent.mockResolvedValue(ok([]));
  commands.historyClear.mockResolvedValue(ok(null));
  commands.incognitoStatus.mockResolvedValue(INCOGNITO_OFF);
  commands.incognitoSetEnabled.mockImplementation(async (on: boolean) =>
    ok(on ? { on, sinceMs: Date.now() } : INCOGNITO_OFF),
  );
  commands.collectionPlay.mockResolvedValue(ok(null));
  const mcpOff = { enabled: false, url: null, token: 'token-1', error: null };
  commands.mcpStatus.mockResolvedValue(ok(mcpOff));
  commands.mcpSetEnabled.mockResolvedValue(ok(mcpOff));
  commands.mcpNewToken.mockResolvedValue(ok(mcpOff));
  const remoteOff = {
    enabled: false,
    url: null,
    pairUrl: null,
    pairQrSvg: null,
    devices: [],
    error: null,
  };
  commands.remoteStatus.mockResolvedValue(ok(remoteOff));
  commands.remoteSetEnabled.mockResolvedValue(ok(remoteOff));
  commands.remoteNewCode.mockResolvedValue(ok(remoteOff));
  commands.remoteForgetDevice.mockResolvedValue(ok(remoteOff));
  const syncOff = {
    enabled: false,
    deviceName: 'Android phone',
    fingerprint: 'ab12 cd34 ef56 7890',
    pairingText: null,
    pairingQrSvg: null,
    kinds: ['settings', 'library', 'history'],
    peers: [],
    log: [],
    error: null,
  };
  commands.syncStatus.mockResolvedValue(ok(syncOff));
  commands.syncSetEnabled.mockResolvedValue(ok(syncOff));
  commands.syncOpenPairing.mockResolvedValue(ok(syncOff));
  commands.syncJoin.mockResolvedValue(ok(syncOff));
  commands.syncNow.mockResolvedValue(ok(syncOff));
  commands.syncRemovePeer.mockResolvedValue(ok(syncOff));
  commands.syncSetKinds.mockResolvedValue(ok(syncOff));
  commands.syncSetDeviceName.mockResolvedValue(ok(syncOff));
  commands.syncUndoLast.mockResolvedValue(ok(syncOff));
  commands.mangaSources.mockResolvedValue(ok([]));
  commands.mangaBrowse.mockResolvedValue(ok({ items: [], hasNextPage: false }));
  commands.mangaTitle.mockResolvedValue(ok(null));
  commands.mangaSetInLibrary.mockResolvedValue(ok(null));
  commands.mangaLibrary.mockResolvedValue(ok([]));
  commands.mangaChapter.mockResolvedValue(ok(null));
  commands.mangaSetProgress.mockResolvedValue(ok(null));
  commands.mangaSetRead.mockResolvedValue(ok(null));
  commands.mangaSetReaderMode.mockResolvedValue(ok(null));
  commands.mangaUpdates.mockResolvedValue(ok([]));
  commands.mangaUpdateLibrary.mockResolvedValue(ok({ ...IDLE_UPDATE, running: true }));
  commands.mangaUpdateStatus.mockResolvedValue(ok(IDLE_UPDATE));
  commands.mangaDownloads.mockResolvedValue(ok({ paused: false, items: [], downloaded: 0 }));
  commands.mangaDownload.mockResolvedValue(ok(null));
  commands.mangaRemoveDownloads.mockResolvedValue(ok(null));
  commands.mangaSetDownloadsPaused.mockResolvedValue(ok(null));
  commands.backupCreate.mockResolvedValue(
    ok({ name: 'raygun-20261031-120000.raygunbk', sizeBytes: 2048, createdAt: 0 }),
  );
  commands.backupList.mockResolvedValue(ok([]));
  commands.backupRestore.mockResolvedValue(ok({ kind: 'raygun', received: 0, changed: 0 }));
  commands.backupDelete.mockResolvedValue(ok(null));
  commands.backupImport.mockResolvedValue(ok({ kind: 'raygun', received: 0, changed: 0 }));
  commands.novelSources.mockResolvedValue(ok([]));
  commands.novelBrowse.mockResolvedValue(ok({ items: [], hasNextPage: false }));
  commands.novelTitle.mockResolvedValue(ok(null));
  commands.novelSetInLibrary.mockResolvedValue(ok(null));
  commands.novelLibrary.mockResolvedValue(ok([]));
  commands.novelChapter.mockResolvedValue(ok(null));
  commands.novelSetProgress.mockResolvedValue(ok(null));
  commands.novelSetRead.mockResolvedValue(ok(null));
  commands.novelImportEpub.mockResolvedValue(ok({ sourceId: 'local.epub', novelId: 'abc' }));
  commands.ttsAvailable.mockResolvedValue(ok(false));
  commands.ttsSpeak.mockResolvedValue(ok(null));
  commands.ttsStop.mockResolvedValue(ok(null));
  commands.aiProviders.mockResolvedValue(ok([]));
  commands.aiProviderSave.mockResolvedValue(ok(null));
  commands.aiProviderDelete.mockResolvedValue(ok(null));
  commands.aiProviderTest.mockResolvedValue(ok(''));
  commands.aiModels.mockResolvedValue([]);
  commands.aiModelDownload.mockResolvedValue(ok(null));
  commands.aiModelDelete.mockResolvedValue(ok(null));
  commands.podcastSubscriptions.mockResolvedValue(ok([]));
  commands.podcastLatest.mockResolvedValue(ok([]));
  commands.podcastSearch.mockResolvedValue(ok([]));
  commands.podcastTop.mockResolvedValue(ok([]));
  commands.podcastRefreshAll.mockResolvedValue(ok(0));
  commands.podcastSetSubscribed.mockResolvedValue(ok(null));
  commands.podcastPlay.mockResolvedValue(ok(null));
  commands.podcastSetPlayed.mockResolvedValue(ok(null));
  commands.podcastSetAllPlayed.mockResolvedValue(ok(null));
  commands.podcastSetEarlierPlayed.mockResolvedValue(ok(0));
  commands.podcastForgetPosition.mockResolvedValue(ok(null));
  commands.podcastSetEpisodeOrder.mockResolvedValue(ok(null));
  commands.podcastSetSpeed.mockResolvedValue(ok(null));
  commands.podcastDownload.mockResolvedValue(ok(null));
  commands.podcastRemoveDownload.mockResolvedValue(ok(null));
  commands.podcastRemoveDownloads.mockResolvedValue(ok(null));
  commands.podcastCancelDownload.mockResolvedValue(ok(null));
  commands.podcastOpenPage.mockResolvedValue(ok(null));
  commands.videoPlay.mockResolvedValue(ok(null));
  commands.videoPlayLink.mockResolvedValue(ok(null));
  commands.videoContinueWatching.mockResolvedValue(ok([]));
  commands.videoResume.mockResolvedValue(ok(null));
  commands.videoRemove.mockResolvedValue(ok(null));
  commands.libraryCategories.mockResolvedValue(ok([]));
  commands.libraryCategoryCreate.mockResolvedValue(ok(null));
  commands.libraryCategoryRename.mockResolvedValue(ok(null));
  commands.libraryCategoryDelete.mockResolvedValue(ok(null));
  commands.libraryCategoryReorder.mockResolvedValue(ok(null));
  commands.librarySetTitleCategories.mockResolvedValue(ok(null));
  commands.libraryTitleMarks.mockResolvedValue(
    ok({ inLibrary: false, list: null, favorite: false, pinned: false }),
  );
  for (const command of [
    commands.librarySetList,
    commands.librarySetFavorite,
    commands.librarySetPinned,
    commands.libraryMarkRead,
    commands.libraryRemove,
    commands.libraryChangeCategories,
    commands.librarySetCategoryDisplay,
    commands.uploadCancel,
  ]) {
    command.mockResolvedValue(ok(null));
  }
  commands.libraryDownloadNext.mockResolvedValue(ok(0));
  commands.libraryContinue.mockResolvedValue(ok(null));
  commands.libraryDuplicates.mockResolvedValue(ok([]));
  commands.libraryUpdateCategory.mockResolvedValue(
    ok({ running: true, checked: 0, total: 0, newChapters: 0, failed: 0, lastRunAt: null }),
  );
  commands.uploadBegin.mockResolvedValue(ok('upload-1'));
  commands.uploadAppend.mockResolvedValue(ok(0));
  commands.mangaImportComic.mockResolvedValue(ok({ sourceId: 'local.comics', remoteId: 'series' }));
  commands.savePickFolder.mockResolvedValue(ok(DEFAULT_SETTINGS));
  commands.songsPickFolder.mockResolvedValue(ok(null));
  commands.saveFolderState.mockResolvedValue(ok({ granted: true, name: null }));
  commands.exportPlan.mockResolvedValue(ok({ chapters: 0, missing: 0, files: 0, problem: null }));
  commands.exportStart.mockResolvedValue({ status: 'error', error: 'not mocked' });
  commands.exportCancel.mockResolvedValue(null);
  commands.exportDismiss.mockResolvedValue(null);
  commands.exportJobs.mockResolvedValue([]);
  commands.pageSave.mockResolvedValue(ok('Pictures/Raygun/page.jpg'));
  commands.pageShare.mockResolvedValue(ok(null));
  commands.pageCopy.mockResolvedValue(ok(null));
  commands.storageUsage.mockResolvedValue(ok([]));
  commands.storageClear.mockResolvedValue(ok([]));
  commands.websiteList.mockResolvedValue(ok([]));
  for (const command of [
    commands.websiteEdit,
    commands.websiteRemove,
    commands.websiteReset,
    commands.websiteSetBlockAds,
    commands.websiteOpen,
    commands.websiteClearData,
  ]) {
    command.mockResolvedValue(ok(null));
  }
  const blocklist = { domains: 0, updatedAt: null, source: null, error: null };
  commands.websiteBlocklistStatus.mockResolvedValue(ok(blocklist));
  commands.websiteBlocklistUpdate.mockResolvedValue(ok(blocklist));
  commands.videoHistory.mockResolvedValue(ok([]));
  commands.videoSources.mockResolvedValue(ok([]));
  commands.videoBrowse.mockResolvedValue(ok({ items: [], hasNextPage: false }));
  commands.videoTitle.mockResolvedValue({ status: 'error', error: 'not mocked' });
  commands.videoServers.mockResolvedValue(ok([]));
  commands.videoLinks.mockResolvedValue(ok([]));
  commands.videoSourcePlay.mockResolvedValue(ok(null));
  commands.videoSetEpisodeWatched.mockResolvedValue(ok(null));
  commands.videoSetInLibrary.mockResolvedValue(ok(null));
  commands.videoLibrary.mockResolvedValue(ok([]));
  commands.videoUpdates.mockResolvedValue(ok([]));
  commands.videoUpcoming.mockResolvedValue(ok([]));
  commands.stremioSetInLibrary.mockResolvedValue(ok(null));
  commands.videoDownloads.mockResolvedValue(ok({ paused: false, items: [] }));
  commands.videoSourceDownload.mockResolvedValue(ok(null));
  commands.stremioDownload.mockResolvedValue(ok(null));
  commands.videoDownloadRetry.mockResolvedValue(ok(null));
  commands.videoDownloadRemove.mockResolvedValue(ok(null));
  commands.videoDownloadPlay.mockResolvedValue(ok(null));
  commands.videoDownloadsSetPaused.mockResolvedValue(ok(null));
  commands.videoSourceWatch.mockResolvedValue(ok(null));
  commands.stremioWatch.mockResolvedValue(ok(null));
  commands.videoSetPlayerLabels.mockResolvedValue(undefined);
  commands.stremioAddons.mockResolvedValue(ok([]));
  commands.stremioSuggestions.mockResolvedValue(ok([]));
  commands.stremioPreview.mockResolvedValue({ status: 'error', error: 'not mocked' });
  commands.stremioAdd.mockResolvedValue(ok(null));
  commands.stremioRemove.mockResolvedValue(ok(null));
  commands.stremioSetEnabled.mockResolvedValue(ok(null));
  commands.stremioSetOrder.mockResolvedValue(ok(null));
  commands.stremioConfigure.mockResolvedValue(ok(null));
  commands.stremioOpenLink.mockResolvedValue(ok(null));
  commands.stremioCatalogs.mockResolvedValue(ok([]));
  commands.stremioHome.mockResolvedValue(ok([]));
  commands.stremioCatalog.mockResolvedValue({ status: 'error', error: 'not mocked' });
  commands.stremioSearch.mockResolvedValue(ok([]));
  commands.stremioSearchCatalog.mockResolvedValue(
    ok({
      addonId: '',
      addonName: '',
      kind: 'movie',
      catalogId: '',
      name: '',
      items: [],
      genres: [],
      hasMore: false,
      error: null,
    }),
  );
  commands.stremioMeta.mockResolvedValue({ status: 'error', error: 'not mocked' });
  commands.stremioStreams.mockResolvedValue(ok([]));
  commands.stremioPlay.mockResolvedValue(ok(null));
  commands.stremioSetWatched.mockResolvedValue(ok(null));
  // Other platforms and public addresses: nothing to ask.
  const open = { needed: false, granted: true, blocked: false };
  commands.localNetworkAccess.mockResolvedValue(ok(open));
  commands.localNetworkRequest.mockResolvedValue(ok(open));
  commands.localNetworkOpenSettings.mockResolvedValue(ok(null));
  // Desktop's answer: notifications, allowed.
  const permissions = [{ kind: 'notifications', state: 'granted', count: null }];
  commands.permissionsList.mockResolvedValue(ok(permissions));
  commands.permissionRequest.mockResolvedValue(ok(permissions));
  commands.permissionOpenSettings.mockResolvedValue(ok(null));
  commands.permissionPromptAnswer.mockResolvedValue(ok(permissions));
  commands.permissionPromptPending.mockResolvedValue(null);
  commands.notificationRouteTake.mockResolvedValue(null);
  commands.catalogFilters.mockResolvedValue(ok([]));
  commands.catalogSearch.mockResolvedValue(ok({ items: [], hasNextPage: false }));
  commands.catalogGlobalSearch.mockResolvedValue(ok([]));
  commands.librarySearch.mockResolvedValue(ok([]));
};

export const emitEvent = <Name extends EventName>(name: Name, payload: EventPayloads[Name]) => {
  act(() => {
    for (const listener of listeners.get(name) ?? []) {
      listener({ payload });
    }
  });
};
