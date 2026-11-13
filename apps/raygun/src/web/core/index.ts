// Raygun on the web's core (docs/WEB.md): answers the commands the page sends, by their Rust
// names, from services that keep their data in the browser. Commands without a web
// implementation get an empty answer or a clear "not on the web" error (fallback.ts).
import type {
  AppInfo,
  CollectionPlayRequest,
  DeviceState,
  EngineReport,
  ExtensionInstallRequest,
  FilterValue,
  LibraryList,
  MangaBrowseMode,
  ReaderMode,
  VideoSourcePlay,
  PlayContext,
  PlaybackControl,
  QueueItem,
  QueueItemInput,
  QuickAccessItem,
  QuickAccessList,
  QuickAccessNames,
  SettingChange,
  SettingValue,
  StorageInfo,
  SyncKind,
  TrackInput,
  TrackSaves,
} from '@raygun/bindings';

import { ExtensionsService } from '../extensions/service';
import { SyncEngine } from '../sync/engine';
import type { Connect } from '../sync/engine';
import { isLinkText, parseInvite } from '../sync/protocol';
import { connectWebRtc } from '../sync/webrtc';
import { CatalogService, mediumOf } from './catalog';
import { ChartsService } from './charts';
import { CollectionsService, queueItemOf } from './collections';
import { SyncedRows, createContext, fail } from './context';
import type { CoreContext } from './context';
import { fallbackAnswer } from './fallback';
import type { CommandAnswers } from './fallback';
import { PlaybackService, queueInputOf } from './playback';
import type { Found } from './playback';
import { SETTINGS_DEFAULTS, SettingsService } from './settings';
import { Store } from './store';
import type { Backend } from './store';

export type Handler = (args: Record<string, unknown>) => unknown;

/** What a remote answers: `handled: false` lets the page's own fallback answer stand. */
export type RemoteAnswer = { handled: true; value: unknown } | { handled: false };
/** Answers commands the page has no implementation of (a linked app; docs/WEB.md). */
export type RemoteHandler = (
  command: string,
  args: Record<string, unknown>,
) => Promise<RemoteAnswer>;

let remoteHandler: RemoteHandler | null = null;

/** Sets (or clears) where commands without a web implementation go before their fallback. */
export const setRemoteHandler = (handler: RemoteHandler | null) => {
  remoteHandler = handler;
};

export type WebCoreOptions = {
  backend: Backend;
  answers: CommandAnswers;
  /** Raygun's version (the app's package). */
  version: string;
  /** Where the YouTube embed host page is served (`embed/youtube.html` beside the app). */
  youTubeEmbed: string;
  now?: () => number;
  debug?: boolean;
  /** How to reach another device (WebRTC; tests connect two cores directly). */
  connect?: Connect;
};

/** Linked devices sync at least this often while the page is open. */
const SYNC_EVERY_MS = 5 * 60_000;
/** How often the change log is looked at for changes to send. */
const WATCH_EVERY_MS = 10_000;

export type WebCore = {
  invoke: (command: string, args: Record<string, unknown>) => Promise<unknown>;
  context: CoreContext;
  settings: SettingsService;
  collections: CollectionsService;
  playback: PlaybackService;
  sync: SyncEngine;
  /** Starts timers (the playback watchdog); the page calls it once. */
  start: () => void;
  /** Waits for every change so far to be saved (tests). */
  flush: () => Promise<void>;
};

const motion = (): DeviceState['motion'] =>
  typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches
    ? 'reduced'
    : 'full';

const visible = () => typeof document === 'undefined' || document.visibilityState !== 'hidden';

const fixedItem = (kind: string, to: string, search: Record<string, string>, name: string) =>
  ({
    kind,
    to,
    search,
    name,
    subtitle: null,
    artworkUrl: null,
    artworkUrls: [],
    pinned: false,
    library: true,
    removable: false,
    addedAt: 0,
    playedAt: 0,
  }) satisfies QuickAccessItem;

export const createWebCore = async (options: WebCoreOptions): Promise<WebCore> => {
  const store = await Store.open(options.backend);
  const context = createContext(store, options.now);
  const settings = new SettingsService(context);
  const collections = new CollectionsService(context);
  const extensions = new ExtensionsService(context, () => settings.stored());
  const catalog = new CatalogService(
    context,
    extensions,
    () => settings.stored(),
    (request) => context.emit('videoPlayerEvent', { kind: 'open', json: JSON.stringify(request) }),
  );
  const charts = new ChartsService(extensions);

  const find = async (item: QueueItem): Promise<Found> => {
    const source = item.source;
    switch (source.kind) {
      case 'url':
        return { kind: 'url', url: source.url };
      case 'youTube':
        return { kind: 'youTube', videoId: source.videoId };
      case 'localFile':
        return {
          kind: 'error',
          message: "This song's file is on another device, so it can't play in the browser",
        };
      case 'lookup': {
        const found = await extensions.streamFor(item).catch(() => null);
        if (found?.kind === 'url') return { kind: 'url', url: found.url };
        if (found?.kind === 'youTube') return { kind: 'youTube', videoId: found.videoId };
        return {
          kind: 'error',
          message:
            'No source in this browser plays this song. Add sources in Browse → Extensions; only those whose sites allow web pages work here.',
        };
      }
    }
  };

  const playback = new PlaybackService(context, {
    find,
    settings: () => settings.stored(),
    played: (item) => {
      const kept = settings.stored().privacy.history;
      if (!kept) return;
      collections.recordPlay({
        title: item.title,
        artist: item.artist,
        album: item.album,
        durationMs: item.durationMs,
        artworkUrl: item.artworkUrl,
        source: item.source,
        explicit: item.explicit,
      });
    },
    youTubeEmbed: () => options.youTubeEmbed,
  });

  const quickAccess = (names: QuickAccessNames): QuickAccessList => {
    const items: QuickAccessItem[] = [
      fixedItem('favorites', '/library', { view: 'favorites' }, names.favorites),
    ];
    for (const playlist of collections.playlists()) {
      items.push({
        ...fixedItem('playlist', '/library/playlist', { id: playlist.id }, playlist.name),
        artworkUrls: playlist.artworkUrls,
        removable: true,
      });
    }
    // Comics, books and videos show while pinned (quick_access.rs `title_pin`).
    for (const title of catalog.pinned()) {
      const to =
        title.medium === 'novel'
          ? '/novel/title'
          : title.medium === 'video'
            ? '/watch/video'
            : '/manga/title';
      items.push({
        ...fixedItem(title.medium, to, { source: title.sourceId, id: title.remoteId }, title.title),
        subtitle: title.authors[0] ?? null,
        artworkUrl: title.coverUrl,
        pinned: true,
        removable: true,
        addedAt: title.addedAt ?? 0,
        playedAt: title.lastReadAt ?? 0,
      });
    }
    return { items, more: 0 };
  };

  const sync = new SyncEngine(context, options.connect ?? connectWebRtc, (summary) => {
    // What came in shows at once, as on a phone after a sync.
    playback.settingsChanged(settings.stored());
    context.emit('syncChangedEvent', summary.changed);
    context.emit('settingsChangedEvent', 'sync');
    context.emit('quickAccessChangedEvent', 'sync');
  });

  /** Links with the app whose link code was scanned or pasted, then syncs with it. */
  const linkWith = async (text: string) => {
    const invite = parseInvite(text);
    if (!invite) {
      fail(
        isLinkText(text)
          ? "That device's Raygun can't link with a browser yet. Update it, then show its code again."
          : "That isn't a Raygun link code. Open Devices on the other device and scan the code it shows.",
      );
      return;
    }
    const peer = await sync.link(invite);
    await sync.syncWith(peer.deviceId).catch(() => undefined);
  };

  const syncStatus = () => sync.status();

  const handlers: Record<string, Handler> = {
    // ---- This device ----
    app_info: (): AppInfo => ({
      name: 'Raygun',
      version: options.version,
      platform: 'web',
      arch: 'browser',
      debug: options.debug ?? false,
    }),
    storage_info: (): StorageInfo => ({
      mode: 'portable',
      root: 'This browser (IndexedDB)',
      logs: '',
      webview: '',
    }),
    device_state: (): DeviceState => ({
      foreground: visible(),
      powerSave: false,
      motion: motion(),
    }),
    device_set_visible: () => null,
    network_status: () => ({
      offline: typeof navigator !== 'undefined' && navigator.onLine === false,
      dataSaver: false,
      mobileData: false,
    }),

    // ---- Settings ----
    settings_get: () => settings.load(),
    settings_defaults: () => ({ ...structuredClone(SETTINGS_DEFAULTS), windowFrame: 'system' }),
    settings_update: ({ change }) => {
      const next = settings.update([change as SettingChange]);
      playback.settingsChanged(settings.stored());
      return next;
    },
    settings_update_many: ({ changes }) => {
      const next = settings.update(changes as SettingChange[]);
      playback.settingsChanged(settings.stored());
      return next;
    },

    // ---- Favorites, playlists, history ----
    favorites_list: () => collections.favorites(),
    favorite_get: ({ track }) => collections.isFavorite(track as TrackInput),
    favorite_set: ({ track, favorite }) => {
      collections.setFavorite(track as TrackInput, favorite === true);
      return null;
    },
    favorites_set_many: ({ tracks, favorite }) => {
      collections.setFavorites(tracks as TrackInput[], favorite === true);
      return null;
    },
    favorite_move: ({ trackKey, beforeKey }) => {
      collections.moveFavorite(String(trackKey), (beforeKey as string | null) ?? null);
      return null;
    },
    favorites_set_order: ({ trackKeys }) => {
      collections.setFavoritesOrder(trackKeys as string[]);
      return null;
    },
    collections_saves: ({ track }) => collections.saves(track as TrackInput),
    collections_set_saves: ({ track, saves }) => {
      collections.setSaves(track as TrackInput, saves as TrackSaves);
      return null;
    },
    collections_saved_index: () => collections.savedIndex(),
    collections_saved_keys: () => collections.savedKeys(),
    playlists_list: () => collections.playlists(),
    playlist_get: ({ id }) => collections.playlist(String(id)),
    playlist_create: ({ name }) => collections.createPlaylist(String(name)),
    playlist_rename: ({ id, name }) => {
      collections.renamePlaylist(String(id), String(name));
      return null;
    },
    playlist_delete: ({ id }) => {
      collections.deletePlaylist(String(id));
      return null;
    },
    playlist_add: ({ id, tracks, duplicates }) =>
      collections.addToPlaylist(String(id), tracks as TrackInput[], duplicates === true),
    playlist_remove_item: ({ itemId }) => {
      collections.removePlaylistItem(String(itemId));
      return null;
    },
    playlist_move_item: ({ itemId, beforeItemId }) => {
      collections.movePlaylistItem(String(itemId), (beforeItemId as string | null) ?? null);
      return null;
    },
    playlist_set_order: ({ playlistId, itemIds }) => {
      collections.setPlaylistOrder(String(playlistId), itemIds as string[]);
      return null;
    },
    playlists_merge: ({ ids, name, skipDuplicates, removeMerged }) =>
      collections.mergePlaylists(
        ids as string[],
        String(name),
        skipDuplicates === true,
        removeMerged === true,
      ),
    history_recent: ({ limit }) => collections.history(Number(limit) || 50),
    history_clear: () => {
      collections.clearHistory();
      return null;
    },
    recent_contexts: () => collections.recentContexts(),
    recent_contexts_add: ({ context: played }) => {
      const kept = settings.stored().privacy.recentlyPlayed;
      if (kept) collections.addRecentContext(played as PlayContext);
      return null;
    },
    quick_access_list: ({ names }) => quickAccess(names as QuickAccessNames),
    collection_play: ({ request }) => {
      const asked = request as CollectionPlayRequest;
      const tracks = collections.tracksOf(asked.collection, asked.order);
      if (tracks.length === 0) fail('Nothing to play here yet');
      playback.replace(tracks.map(queueItemOf), asked.startIndex, asked.shuffle);
      return null;
    },

    // ---- Playback ----
    playback_get_state: () => playback.core.state(),
    playback_get_queue: () => playback.core.queue(),
    playback_enqueue: ({ items, playNow }) => {
      playback.enqueue(items as QueueItemInput[], playNow === true);
      return null;
    },
    playback_replace: ({ items, startIndex }) => {
      playback.replace(items as QueueItemInput[], (startIndex as number | null) ?? null);
      return null;
    },
    playback_queue_tracks: ({ tracks, next }) => {
      const inputs = (tracks as TrackInput[]).map(queueInputOf);
      if (next === true) playback.enqueueNext(inputs);
      else playback.enqueue(inputs, false);
      return null;
    },
    playback_control: ({ control }) => {
      playback.control(control as PlaybackControl);
      return null;
    },
    playback_engine_report: ({ report }) => {
      playback.report(report as EngineReport);
      return null;
    },
    playback_arm_context: ({ context: armed }) => {
      playback.arm((armed as PlayContext | null) ?? null);
      return null;
    },
    playback_restore: () => {
      playback.restore();
      return null;
    },
    playback_set_video: () => null,
    youtube_embed_url: () => options.youTubeEmbed,

    // ---- Devices and sync ----
    sync_status: syncStatus,
    sync_set_enabled: syncStatus,
    sync_now: async ({ deviceId }) => {
      if (typeof deviceId === 'string') await sync.syncWith(deviceId);
      else await sync.syncAll();
      return syncStatus();
    },
    sync_remove_peer: ({ deviceId }) => {
      sync.removePeer(String(deviceId));
      return syncStatus();
    },
    sync_set_kinds: ({ kinds }) => {
      sync.setKinds(kinds as SyncKind[]);
      return syncStatus();
    },
    sync_set_device_name: ({ name }) => {
      sync.setName(String(name));
      return syncStatus();
    },
    sync_join: async ({ text }) => {
      await linkWith(String(text));
      return syncStatus();
    },
    sync_open_pairing: () =>
      fail(
        'A browser can’t be reached by other devices. Show the code on the other device and scan it here.',
      ),
    sync_undo_last: () => fail('Undoing a sync isn’t available in the browser.'),
    connect_link_code: async ({ text }) => {
      await linkWith(String(text));
      return [];
    },
    connect_devices: () => [],
    connect_status: () => ({ thisDevice: sync.name(), session: null, asking: null, error: null }),
    devices_set_sharing: () => null,

    // ---- Sources (Raygun's own extensions, sandboxed) ----
    extensions_list: () => extensions.list(),
    extensions_reload: () => {
      extensions.reload();
      return extensions.list();
    },
    extension_uninstall: ({ extensionId }) => {
      extensions.uninstall(String(extensionId));
      return extensions.list();
    },
    extension_setting_set: ({ extensionId, key, value }) => {
      extensions.setSetting(String(extensionId), String(key), value as SettingValue);
      return extensions.list();
    },
    extension_clear_data: ({ extensionId }) => {
      extensions.clearData(String(extensionId));
      return extensions.list();
    },
    extension_repos: () => extensions.repos(),
    extension_suggested_repos: () => extensions.suggested(),
    extension_repo_add: ({ url }) => extensions.addRepo(String(url)),
    extension_repo_remove: ({ id }) => extensions.removeRepo(String(id)),
    extension_catalog: ({ refresh }) => extensions.catalog(refresh === true),
    extension_install: async ({ repoId, extensionId }) => {
      await extensions.install(String(repoId), String(extensionId));
      extensions.reload(String(extensionId));
      await extensions.start();
      return extensions.list();
    },
    extension_installs_enqueue: ({ entries }) =>
      extensions.enqueue(entries as ExtensionInstallRequest[]),
    extension_installs_status: () => extensions.installStatus(),
    extension_installs_dismiss: () => extensions.dismiss(),
    extension_installs_cancel: ({ extensionId }) => extensions.cancel(String(extensionId)),
    extension_installs_cancel_all: () => extensions.cancel(null),
    extension_installs_retry: () => extensions.retry(),
    extension_repo_preview: ({ url }) => extensions.preview(String(url)),
    extension_catalog_refresh_repo: async ({ repoId }) => {
      const repo = extensions.repos().find((candidate) => candidate.id === repoId);
      if (repo) await extensions.catalog(true, [repo]);
      return extensions.catalog(false);
    },
    extension_open_website: ({ id }) => {
      const website = extensions.list().find((extension) => extension.id === id)?.website;
      if (!website) fail('This source has no website.');
      window.open(String(website), '_blank', 'noopener');
      return null;
    },
    online_search: async ({ query }) => {
      const trimmed = String(query).trim();
      const providers = trimmed ? await extensions.searchMusic(trimmed) : [];
      // The best of each source first: one with a stream of its own leads.
      const top = providers
        .flatMap((result) => result.tracks.slice(0, 2))
        .sort((a, b) => Number(b.source !== null) - Number(a.source !== null))
        .slice(0, 5);
      return { library: [], top, providers };
    },
    music_charts: ({ region }) => charts.charts((region as string | null) ?? null),
    music_chart: ({ providerId, chartId, region }) =>
      charts.chart(String(providerId), String(chartId), (region as string | null) ?? null),

    // ---- Comics, books and videos from sources ----
    manga_sources: () => catalog.sources('manga'),
    novel_sources: () => catalog.sources('novel'),
    video_sources: () => catalog.sources('video'),
    manga_browse: ({ sourceId, mode, query, page }) =>
      catalog.browse(
        'manga',
        String(sourceId),
        mode as MangaBrowseMode,
        query as string | null,
        Number(page) || 1,
      ),
    novel_browse: ({ sourceId, mode, query, page }) =>
      catalog.browse(
        'novel',
        String(sourceId),
        mode as MangaBrowseMode,
        query as string | null,
        Number(page) || 1,
      ),
    video_browse: ({ sourceId, mode, query, page }) =>
      catalog.browse(
        'video',
        String(sourceId),
        mode as MangaBrowseMode,
        query as string | null,
        Number(page) || 1,
      ),
    catalog_search: ({ medium, sourceId, query, page, filters }) =>
      catalog.search(
        mediumOf(medium),
        String(sourceId),
        String(query ?? ''),
        Number(page) || 1,
        (filters as Record<string, FilterValue> | null) ?? {},
      ),
    catalog_filters: ({ sourceId }) => catalog.filters(String(sourceId)),
    catalog_global_search: ({ medium, query, sources }) =>
      catalog.globalSearch(
        mediumOf(medium),
        String(query ?? ''),
        (sources as string[] | null) ?? [],
      ),
    catalog_open_page: async ({ sourceId, url }) => {
      const address =
        (url as string | null) ??
        extensions.list().find((extension) => String(sourceId).startsWith(`${extension.id}/`))
          ?.website ??
        null;
      if (!address) fail('This source has no website.');
      window.open(String(address), '_blank', 'noopener');
      return null;
    },
    manga_title: ({ sourceId, mangaId, refresh }) =>
      catalog.mangaTitle('manga', String(sourceId), String(mangaId), refresh === true),
    novel_title: ({ sourceId, novelId, refresh }) =>
      catalog.mangaTitle('novel', String(sourceId), String(novelId), refresh === true),
    video_title: ({ sourceId, remoteId, refresh }) =>
      catalog.videoTitle(String(sourceId), String(remoteId), refresh === true),
    manga_chapters: ({ titleId }) => catalog.chapters(String(titleId)),
    manga_chapter: ({ titleId, chapterId }) =>
      catalog.chapterPages(String(titleId), String(chapterId)),
    novel_chapter: ({ titleId, chapterId }) =>
      catalog.novelChapter(String(titleId), String(chapterId)),
    manga_set_reader_mode: ({ titleId, mode }) => {
      catalog.setReaderMode(String(titleId), (mode as ReaderMode | null) ?? null);
      return null;
    },
    manga_set_progress: ({ chapterId, page, total }) => {
      catalog.setProgress(String(chapterId), Number(page), Number(total));
      return null;
    },
    novel_set_progress: ({ chapterId, offset, total }) => {
      catalog.setProgress(String(chapterId), Number(offset), Number(total));
      return null;
    },
    manga_set_read: ({ chapterId, read }) => {
      catalog.setRead([String(chapterId)], read === true);
      return null;
    },
    novel_set_read: ({ chapterId, read }) => {
      catalog.setRead([String(chapterId)], read === true);
      return null;
    },
    library_set_chapters_read: ({ chapterIds, read }) => {
      catalog.setRead(chapterIds as string[], read === true);
      return null;
    },
    library_mark_read: ({ titleIds, read }) => {
      catalog.markTitlesRead(titleIds as string[], read === true);
      return null;
    },
    library_mark_previous_read: ({ chapterId }) => {
      catalog.markPreviousRead(String(chapterId));
      return null;
    },
    manga_set_in_library: ({ titleId, inLibrary }) => {
      catalog.setInLibrary(String(titleId), inLibrary === true);
      return null;
    },
    novel_set_in_library: ({ titleId, inLibrary }) => {
      catalog.setInLibrary(String(titleId), inLibrary === true);
      return null;
    },
    video_set_in_library: ({ titleId, inLibrary }) => {
      catalog.setInLibrary(String(titleId), inLibrary === true);
      return null;
    },
    manga_library: () => catalog.library('manga'),
    novel_library: () => catalog.library('novel'),
    video_library: () => catalog.library('video'),
    library_title_marks: ({ titleId }) => catalog.marks(String(titleId)),
    library_set_list: ({ titleIds, list }) => {
      catalog.setMark(titleIds as string[], { list: (list as LibraryList | null) ?? null });
      return null;
    },
    library_set_favorite: ({ titleIds, favorite }) => {
      catalog.setMark(titleIds as string[], { favorite: favorite === true });
      return null;
    },
    library_set_pinned: ({ titleIds, pinned }) => {
      catalog.setMark(titleIds as string[], { pinned: pinned === true });
      return null;
    },
    library_remove: ({ titleIds }) => {
      catalog.remove(titleIds as string[]);
      return null;
    },
    library_continue: ({ titleId }) => catalog.continueChapter(String(titleId)),
    video_servers: ({ sourceId, episodeId }) =>
      catalog.servers(String(sourceId), String(episodeId)),
    video_links: ({ sourceId, serverId }) => catalog.links(String(sourceId), String(serverId)),
    video_source_play: ({ request }) => {
      catalog.play(request as VideoSourcePlay);
      return null;
    },
    video_source_watch: async ({ titleId, episodeId }) => {
      await catalog.watch(String(titleId), (episodeId as string | null) ?? null);
      return null;
    },
    video_set_episode_watched: ({ titleId, episodeId, watched }) => {
      catalog.setWatched(String(titleId), String(episodeId), watched === true);
      return null;
    },
    video_continue_watching: () => catalog.continueWatching(),
    video_history: () => catalog.continueWatching(true),
    video_resume: async ({ key }) => {
      await catalog.resume(String(key), false);
      return null;
    },
    video_play_from_start: async ({ key }) => {
      await catalog.resume(String(key), true);
      return null;
    },
    video_remove: ({ key }) => {
      catalog.forgetProgress([String(key)]);
      return null;
    },
    video_remove_many: ({ keys }) => catalog.forgetProgress(keys as string[]),
    video_mark_watched: ({ key }) => {
      catalog.setRead([String(key)], true);
      return null;
    },
    video_player_report: ({ report }) => {
      catalog.report(String(report));
      return null;
    },
  };

  const invoke = async (command: string, args: Record<string, unknown>) => {
    const handler = handlers[command];
    if (!handler) {
      if (remoteHandler) {
        const answer = await remoteHandler(command, args);
        if (answer.handled) return answer.value;
      }
      if (options.debug) console.debug('[raygun web] no web implementation:', command);
      // What fell back, for checking the web build's screens (docs/WEB.md, the audit).
      (globalThis as { __RAYGUN_WEB_FALLBACKS__?: string[] }).__RAYGUN_WEB_FALLBACKS__?.push(
        command,
      );
      return fallbackAnswer(options.answers, command);
    }
    return handler(args);
  };

  /**
   * Keeps linked devices in sync while the page is open and shown: soon after a change here, and
   * every few minutes anyway (device_link.rs does the same in the apps).
   */
  const autoSync = () => {
    let seen = -1;
    let lastRound = 0;
    setInterval(() => {
      if (!visible() || sync.peers().length === 0) return;
      const latest = new SyncedRows(context).latestSeq();
      const now = context.now();
      if (latest === seen && now - lastRound < SYNC_EVERY_MS) return;
      seen = latest;
      lastRound = now;
      void sync.syncAll().then(() => {
        seen = new SyncedRows(context).latestSeq();
      });
    }, WATCH_EVERY_MS);
  };

  return {
    invoke,
    context,
    settings,
    collections,
    playback,
    sync,
    start: () => {
      playback.start();
      autoSync();
      void extensions.start().then(() => extensions.installStarters());
    },
    flush: () => store.flush(),
  };
};
