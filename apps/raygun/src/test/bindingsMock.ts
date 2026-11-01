import type {
  AiModelProgress,
  AppInfo,
  EngineCommand,
  ExtensionInfo,
  LibraryScanProgress,
  LibraryStatus,
  MangaUpdateStatus,
  TtsUpdate,
  PlaybackState,
  QueueSnapshot,
  Settings,
  StorageInfo,
} from '@raygun/bindings';
import { act } from '@testing-library/react';

/**
 * Stand-in for `@raygun/bindings` (the Rust IPC layer). Use with
 * `vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock)`
 * and push Rust events into the UI with `emitEvent`.
 */

type EventPayloads = {
  engineCommandEvent: EngineCommand;
  playbackStateEvent: PlaybackState;
  queueEvent: QueueSnapshot;
  libraryScanEvent: LibraryScanProgress;
  extensionsChangedEvent: ExtensionInfo[];
  syncChangedEvent: number;
  mangaUpdateEvent: MangaUpdateStatus;
  mangaDownloadsEvent: string;
  ttsEvent: TtsUpdate;
  aiModelEvent: AiModelProgress;
  podcastsChangedEvent: string;
  watchChangedEvent: string;
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
};

export const EMPTY_QUEUE: QueueSnapshot = { items: [], order: [] };

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
  preferredVideoQuality: null,
  mangaUpdateHours: 12,
  novelReader: {
    fontSize: 18,
    lineSpacing: 16,
    font: 'serif',
    width: 'narrow',
    paged: false,
    ttsRate: 10,
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
    storageInfo: vi.fn(),
    settingsGet: vi.fn(),
    settingsUpdate: vi.fn(),
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
    extensionRepos: vi.fn(),
    extensionRepoAdd: vi.fn(),
    extensionSuggestedRepos: vi.fn(),
    extensionRepoRemove: vi.fn(),
    extensionCatalog: vi.fn(),
    extensionInstall: vi.fn(),
    extensionUninstall: vi.fn(),
    extensionSettingSet: vi.fn(),
    onlineSearch: vi.fn(),
    lyricsGet: vi.fn(),
    favoritesList: vi.fn(),
    favoriteGet: vi.fn(),
    favoriteSet: vi.fn(),
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
    podcastDownload: vi.fn(),
    podcastRemoveDownload: vi.fn(),
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
    videoHistory: vi.fn(),
    videoSources: vi.fn(),
    videoBrowse: vi.fn(),
    videoTitle: vi.fn(),
    videoServers: vi.fn(),
    videoLinks: vi.fn(),
    videoSourcePlay: vi.fn(),
    videoSetEpisodeWatched: vi.fn(),
    stremioAddons: vi.fn(),
    stremioSuggestions: vi.fn(),
    stremioAdd: vi.fn(),
    stremioRemove: vi.fn(),
    stremioHome: vi.fn(),
    stremioCatalog: vi.fn(),
    stremioSearch: vi.fn(),
    stremioMeta: vi.fn(),
    stremioStreams: vi.fn(),
    stremioPlay: vi.fn(),
    stremioSetWatched: vi.fn(),
    localNetworkAccess: vi.fn(),
    localNetworkRequest: vi.fn(),
    localNetworkOpenSettings: vi.fn(),
    catalogFilters: vi.fn(),
    catalogSearch: vi.fn(),
    catalogGlobalSearch: vi.fn(),
    librarySearch: vi.fn(),
  },
  events: {
    engineCommandEvent: makeEvent('engineCommandEvent'),
    playbackStateEvent: makeEvent('playbackStateEvent'),
    queueEvent: makeEvent('queueEvent'),
    libraryScanEvent: makeEvent('libraryScanEvent'),
    extensionsChangedEvent: makeEvent('extensionsChangedEvent'),
    syncChangedEvent: makeEvent('syncChangedEvent'),
    mangaUpdateEvent: makeEvent('mangaUpdateEvent'),
    mangaDownloadsEvent: makeEvent('mangaDownloadsEvent'),
    ttsEvent: makeEvent('ttsEvent'),
    aiModelEvent: makeEvent('aiModelEvent'),
    podcastsChangedEvent: makeEvent('podcastsChangedEvent'),
    watchChangedEvent: makeEvent('watchChangedEvent'),
  },
};

/** Default answers for every command, as an idle app on Android would give. */
export const seedBindings = () => {
  listeners.clear();
  const { commands } = bindingsMock;
  commands.appInfo.mockResolvedValue(ANDROID_INFO);
  commands.storageInfo.mockResolvedValue(MOBILE_STORAGE);
  commands.settingsGet.mockResolvedValue({ status: 'ok', data: DEFAULT_SETTINGS });
  commands.playbackGetState.mockResolvedValue(IDLE_STATE);
  commands.playbackGetQueue.mockResolvedValue(EMPTY_QUEUE);
  commands.playbackEnqueue.mockResolvedValue(undefined);
  commands.playbackControl.mockResolvedValue(undefined);
  commands.playbackEngineReport.mockResolvedValue(undefined);
  const ok = <T>(data: T) => ({ status: 'ok' as const, data });
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
  commands.extensionRepos.mockResolvedValue(ok([]));
  commands.extensionRepoAdd.mockResolvedValue(ok([]));
  commands.extensionSuggestedRepos.mockResolvedValue(ok([]));
  commands.extensionRepoRemove.mockResolvedValue(ok([]));
  commands.extensionCatalog.mockResolvedValue(ok([]));
  commands.extensionInstall.mockResolvedValue(ok([]));
  commands.extensionUninstall.mockResolvedValue(ok([]));
  commands.extensionSettingSet.mockResolvedValue(ok([]));
  commands.onlineSearch.mockResolvedValue(ok({ library: [], top: [], providers: [] }));
  commands.lyricsGet.mockResolvedValue(ok(null));
  commands.favoritesList.mockResolvedValue(ok([]));
  commands.favoriteGet.mockResolvedValue(ok(false));
  commands.favoriteSet.mockResolvedValue(ok(null));
  commands.playlistsList.mockResolvedValue(ok([]));
  commands.playlistGet.mockResolvedValue(
    ok({ playlist: { id: 'p', name: 'Playlist', itemCount: 0, artworkUrls: [] }, items: [] }),
  );
  commands.playlistCreate.mockResolvedValue(
    ok({ id: 'new', name: 'New', itemCount: 0, artworkUrls: [] }),
  );
  commands.playlistRename.mockResolvedValue(ok(null));
  commands.playlistDelete.mockResolvedValue(ok(null));
  commands.playlistAdd.mockResolvedValue(ok(null));
  commands.playlistRemoveItem.mockResolvedValue(ok(null));
  commands.playlistMoveItem.mockResolvedValue(ok(null));
  commands.historyRecent.mockResolvedValue(ok([]));
  commands.historyClear.mockResolvedValue(ok(null));
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
  commands.podcastDownload.mockResolvedValue(ok(null));
  commands.podcastRemoveDownload.mockResolvedValue(ok(null));
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
  commands.videoHistory.mockResolvedValue(ok([]));
  commands.videoSources.mockResolvedValue(ok([]));
  commands.videoBrowse.mockResolvedValue(ok({ items: [], hasNextPage: false }));
  commands.videoTitle.mockResolvedValue({ status: 'error', error: 'not mocked' });
  commands.videoServers.mockResolvedValue(ok([]));
  commands.videoLinks.mockResolvedValue(ok([]));
  commands.videoSourcePlay.mockResolvedValue(ok(null));
  commands.videoSetEpisodeWatched.mockResolvedValue(ok(null));
  commands.stremioAddons.mockResolvedValue(ok([]));
  commands.stremioSuggestions.mockResolvedValue(ok([]));
  commands.stremioAdd.mockResolvedValue(ok(null));
  commands.stremioRemove.mockResolvedValue(ok(null));
  commands.stremioHome.mockResolvedValue(ok([]));
  commands.stremioCatalog.mockResolvedValue({ status: 'error', error: 'not mocked' });
  commands.stremioSearch.mockResolvedValue(ok([]));
  commands.stremioMeta.mockResolvedValue({ status: 'error', error: 'not mocked' });
  commands.stremioStreams.mockResolvedValue(ok([]));
  commands.stremioPlay.mockResolvedValue(ok(null));
  commands.stremioSetWatched.mockResolvedValue(ok(null));
  // Other platforms and public addresses: nothing to ask.
  const open = { needed: false, granted: true, blocked: false };
  commands.localNetworkAccess.mockResolvedValue(ok(open));
  commands.localNetworkRequest.mockResolvedValue(ok(open));
  commands.localNetworkOpenSettings.mockResolvedValue(ok(null));
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
