# Music and podcasts: what Nuclear and RiPlay have that Raygun lacks

Research of 2026-11-01. It compares `reference/nuclear` and `reference/RiPlay` with Raygun's music and podcast features after M43:
- `docs/feature-parity.md`;
- the specs for M05–M21 and M37;
- `apps/raygun/src` (library, collections, player, podcasts, search, Home, settings);
- `raygun-playback`, `raygun-library`, `raygun-resolver`, `raygun-app`;
- the `raygun-audio` plugin.

**Path shorthands:**
- `RP/` = `reference/RiPlay/androidApp/src/main/kotlin/it/fast4x/riplay/`
- `RPm/` = `reference/RiPlay/modules/`
- `NP/` = `reference/nuclear/packages/player/src/`
- `NS/` = `reference/nuclear/packages/plugin-sdk/src/`

**Sizes:**
- S = a day or less;
- M = a few days, touching Rust, the UI and maybe the plugin;
- L = a milestone of its own.

## How each app separates discovery from the user's library

- **Nuclear:**
  - **Discovery:**
    - Dashboard (`NP/views/Dashboard`, widgets from `dashboard` providers: editorial playlists, top artists, top albums, new releases, top tracks);
    - Search (unified results: artists, albums, tracks, playlists);
    - artist and album pages built from `metadata` provider capabilities (`NP/views/Artist/artistWidgets.ts`).
  - **The user's own:**
    - Favorites (tracks, albums, artists: `NP/views/Favorites`);
    - Playlists (with folders);
    - History (with stats).
  - **Tools:** Plugins, Themes, Sources (provider choice), Settings.
- **RiPlay:**
  - **Home** mixes a discovery feed (Quick picks/For you, charts, moods & genres, new releases, related albums, similar artists, "playlists you might like") with library tabs (Songs, Artists, Albums, Playlists, On device): `RP/ui/screens/home`, `RP/ui/screens/home/homepages`.
  - **Search** is its own tab, with history and filters.
  - **Other pages:** Artist, album, playlist, podcast, mood and new-release pages are reached from the feed or Search. Statistics, Rewind, History, Events and Blacklist sit in menus.
- **Raygun today:**
  - **Library → Music** has only the user's content: local albums/artists/tracks, playlists, favorites.
  - **Home** shows Continue watching/reading, Recently played and Favorites.
  - **Search** finds tracks only; it has no artist, album or playlist results and no landing content.
  - **No artist or album pages for online music:** `pages/ArtistPage.tsx` and `AlbumPage.tsx` are local-file views.
- **Fit with the user's rule** (Library = own content; finding things happens in Search):
  - Nuclear's Dashboard and RiPlay's discovery feed belong in **Search's empty state** (an "Explore" landing: charts, moods/genres, new releases, radio stations, top podcasts).
  - Online artist, album and playlist pages are reached from Search results.
  - Library keeps Songs, Albums, Artists, Playlists, Favorites, Folders, plus History and Stats.

---

## 1. Player and queue (most visible gap)

| Gap | Reference | Raygun today | Size |
|---|---|---|---|
| **Play next / add to queue** from any track menu, including multi-select | Nuclear `QueueHost.addNext/addAt` (`NS/types/queue.ts`); RiPlay song menu "Play next", "Enqueue" (`RP/ui/components/themed/MediaItemMenu.kt`) | Tracks can only replace the queue (`library_play`, `collection_play`) or be appended via `playback_enqueue` with no UI to insert next | S |
| **Queue editing in the player:** drag to reorder, swipe to remove, clear, jump to item | Nuclear `QueueHost.reorder/removeByIds/clearQueue`, `ui/src/components/QueuePanel`, `QueueItemPopover`; RiPlay `RP/ui/screens/player/common/Queue.kt` with swipe actions (`QueueSwipeAction`: PlayNext, Favourite, Remove, Enqueue) | Core has `remove`/`clear`/`play_item` (`raygun-playback`); there is no `move`, and the player's "Up next" is a read-only list (`player/PlayerSheet.tsx`) | M |
| **Save queue as playlist** | Nuclear `PlaylistsHost.saveQueueAsPlaylist` | Missing | S |
| **Persistent queue** (restore the queue and position after restart) and "resume playback on start" | RiPlay `persistentQueueKey`, `resumePlaybackOnStartKey`, `currentQueuePositionKey` (`RP/extensions/preferences/Preferences.kt`) | The queue lives in memory in `PlaybackService`; it's lost on restart | M |
| **Endless radio / auto-continue:** start radio from a song, and add related songs when the queue runs out | Nuclear `discoveryService.ts` (adds 5 recommendations at the last track, "variety" slider, settings `playback.discovery`, `playback.discoveryVariety`); RiPlay "Start radio", "discover" (`discoverKey`), `autoLoadSongsInQueueKey`, `RP/extensions/experimental/recommendationstrategy` | Missing; needs a recommendations capability (§3) | M |
| **Queue info:** total duration, remaining time, songs count, "now playing" indicator in lists | RiPlay `showTotalTimeQueueKey`, `showRemainingSongTimeKey`, `nowPlayingIndicatorKey`, `playlistindicatorKey` | Missing | S |
| **Repeat behaviours:** previous-restarts threshold, skip on error, max songs in queue | Nuclear `playback.skipSeconds`; RiPlay `jumpPreviousKey`, `skipMediaOnErrorKey`, `maxSongsInQueueKey` | Previous restarts after a fixed 3 s; skip on error is always on; there are no options | S |
| **Mini player variants:** up-next peek, swipe left/right to skip, swipe up to open the queue, progress style | RiPlay `MiniPlayerType` (Essential/Modern), `show_upnext_in_miniplayer`, `disablePlayerHorizontalSwipeKey`, `swipeUpQueueKey` | One floating mini player with play/pause | S–M |
| **Full player layouts and controls:** Essential/Modern player, thumbnail sizes, carousel of next covers, timeline styles (Wavy, PinBar, ThinBar…), play-button shapes, swipe on cover to skip, tap cover for lyrics, "stats for nerds" (codec/bitrate/source), configurable action bar (video, discover, download, add to playlist, loop, shuffle, lyrics, sleep timer, equalizer, start radio, menu) | RiPlay `PlayerType`, `PlayerTimelineType`, `PlayerPlayButtonType`, `carouselKey`, `thumbnailTapEnabledKey`, `statsfornerdsKey`, `showButtonPlayer*Key` (`RP/ui/screens/player/unified/*`, `RP/ui/screens/settings/PlayerAppearanceSettings.kt`) | One fixed layout: blurred art, heart, add to playlist, seek, shuffle/prev/play/next/repeat, lyrics toggle, Up next | M (do it in steps) |
| **Player background styles:** cover gradient, theme gradient, blurred cover, animated gradient, "MidnightOdyssey", blur strength and darken | RiPlay `PlayerBackgroundColors`, `blurStrengthKey`, `blurDarkenFactorKey`, `animatedGradientKey` | Fixed blurred artwork | S |
| **Visualizer** (spectrum, level, animations; player, mini-player and EQ styles) | RiPlay `RP/extensions/nextvisualizer`, `AudioVisualizerStyle` | Missing (needs the `Visualizer` API, mic permission) | M |
| **Music video toggle:** switch a song to its video and back; PiP for video | RiPlay `showButtonPlayerVideoKey`, `RP/extensions/pip`, `enablePictureInPictureKey/AutoKey` | YouTube embeds play as video in the sheet, with no audio/video switch and no PiP for music | M |
| **Landscape/tablet player layouts** | RiPlay `LandscapeLayout` (Layout1/2), `landscapeControlsKey` | Single layout | S |

## 2. Audio processing and playback options

| Gap | Reference | Raygun today | Size |
|---|---|---|---|
| **Playback speed and pitch** (music and podcasts) | RiPlay `playbackSpeedKey`, `playbackPitchKey`; M37 left it "later" | Missing; Media3 `setPlaybackParameters` in `RaygunSessionPlayer.kt` would cover it | S |
| **Equalizer:** internal multi-band EQ with a curve editor and presets (Flat, Bass boost, Rock, Jazz, Vocal, Classical, Pop, Electronic, Dance, Acoustic), or the system EQ panel | `RP/extensions/equalizer/InternalEqualizerScreen.kt`, `InternalEqualizerCurve.kt`, `SystemEqualizer.kt`, `EqualizerPreset`, `EqualizerType` | Missing (roadmap M52) | M |
| **Bass boost, reverb presets** (small room … plate), **volume boost**, disable audio DRC | `RP/services/helpers/EqualizerHelper.kt`, `AudioDRCHelper.kt`, `PresetReverb`, `bassboostLevelKey`, `volumeBoostLevelKey`, `disableAudioDRCKey` | Missing | S–M |
| **Loudness normalization** (with base gain) | RiPlay `volumeNormalizationKey`, `loudnessBaseGainKey` | Missing | S |
| **Skip silence** (with minimum silence length) | RiPlay `skipSilenceKey`, `minimumSilenceDurationKey` | Missing (Media3 `setSkipSilenceEnabled`) | S |
| **Crossfade / fade in-out** and **pause between songs**; gapless | Nuclear `playback.crossfadeMs`; RiPlay `playbackFadeDurationKey`, `playbackFadeAudioDurationKey`, `PauseBetweenSongs`, `CrossfadeDuration` | Missing; gapless isn't possible while the session holds one item at a time (M09 design) | M |
| **Sleep timer** (minutes, end of song, stop it), auto-close the service after N minutes or when paused N minutes | RiPlay `SleepTimerListener` in `RP/services/playback/PlayerService.kt`, `closePlayerServiceAfterMinutesKey`, `closePlayerServiceWhenPausedAfterMinutesKey` | Missing (roadmap M52) | S |
| **Audio focus and device behaviour:** pause on volume zero, resume when headphones or Bluetooth connect (pick devices), resume or pause after calls, handle audio focus off | RiPlay `isPauseOnVolumeZeroEnabledKey`, `settings_bt_title_bluetooth_audio_devices`, `settings_wired_title_wired_audio_devices`, `resumeOrPausePlaybackWhenCallKey`, `handleAudioFocusEnabledKey` | Media3 defaults only, no options | S–M |
| **Gestures:** volume keys skip tracks (screen off), shake to skip | RiPlay `useVolumeKeysToChangeSongKey`, `shakeEventEnabledKey` | Missing | S |
| **Audio quality choice** (Auto/High/Medium/Low) for streams | RiPlay `AudioQualityFormat` | Missing (the resolver doesn't rank by bitrate) | S |
| **Stream handling options:** expiry, retries, community stream verification | Nuclear `playback.streamExpiryMs`, `streamResolutionRetries`, `streamVerification(Service)` (`NP/services/streamVerification.ts`) | The resolver verifies locally (M14); no options and no shared verification | S (options) / L (service) |

## 3. Discovery and the online catalogue

| Gap | Reference | Raygun today | Size |
|---|---|---|---|
| **Online artist pages:** bio, social stats, popular tracks, albums, playlists, similar artists | Nuclear `ArtistMetadataCapability` (`NS/types/search.ts`), `NP/views/Artist`; RiPlay `RP/ui/screens/artist/ArtistOverview.kt` | `music.metadata` only does `search` for tracks; the artist page is local albums only | L (SDK capability + MusicBrainz/Last.fm/Deezer-style extensions + UI) |
| **Online album pages:** tracklist, year, label, other versions, "more of" the artist | Nuclear `fetchAlbumDetails`; RiPlay `RP/ui/screens/album/AlbumDetails.kt` | Local album page only | M |
| **Search by category:** artists, albums, playlists, videos, podcasts, featured playlists, plus "unified" | Nuclear `SearchCapability`; RiPlay `SearchType`, `RP/ui/screens/search/OnlineSearch.kt` | Tracks only (plus local matches and merged Top results) | M |
| **Dashboard / Explore:** editorial playlists, top artists/albums/tracks, new releases | Nuclear `DashboardCapability`, `NP/views/Dashboard/dashboardWidgets.ts` | Missing. Raygun's Home has none, and by the new rule it would live in Search's landing | M (+ provider work) |
| **Quick picks / For you** from listening history (most played, last played, casual), trending song seed, related page | RiPlay `PlayEventsType`, `quickPicsTrendingSongKey`, `quickPicsRelatedPageKey`, `RP/ui/screens/home/homepages` | Missing | M |
| **Charts by country**, **moods & genres** pages, **new releases** (and new albums from followed artists) | RiPlay `RP/ui/screens/moodandchip`, `RP/ui/screens/newreleases`, `selectedCountryCodeKey`, `showChartsKey`, `showMoodsAndGenresKey`, `showNewAlbumsArtistsKey` | Missing | M |
| **Follow artists:** notifications for new releases (daily or weekly events) | RiPlay `RP/extensions/scheduled/PeriodicCheckNewFromArtists.kt`, `EventType.NewArtistsRelease`, `RP/ui/screens/events` | Missing (no favorite artists either) | M |
| **Recommendation strategies** (classifiers, builders) | RiPlay `RP/extensions/experimental/recommendationstrategy` | Missing | L |
| **Internet radio browsing:** by country, tag or language; favorite stations; station logos | — (Raygun's Radio Browser extension) | Stations are only search results from `music.metadata`, sorted by a setting; nothing to browse and no station favorites list | S–M |
| **Provider choice per kind** and why one is locked | Nuclear `NP/views/Sources` (active provider per kind, `LockedReasonMessage`, `ProviderPill`) | All providers are asked in parallel; no per-kind order or off switch in the UI | S–M |

## 4. Local library (the user's files)

| Gap | Reference | Raygun today | Size |
|---|---|---|---|
| **Sort and display options** for songs, albums and artists: title, artist, album, year, duration, date added, play count; ascending or descending; grid or list; item sizes | RiPlay `songSortByKey`, `albumSortByKey`, `artistSortByKey`, `OnDeviceSongSortBy`, `LibraryItemSize`, `ViewType` | Fixed order, no options | S |
| **Folders view** (browse the device by folder) with folder sort and short names | RiPlay `RP/ui/screens/ondevice`, `RP/extensions/ondevice/Folder.kt`, `showFoldersOnDeviceKey`, `OnDeviceFolderSortBy`, `shortOnDeviceFolderNameKey` | Library roots exist in Settings, but there's no folder browser | S–M |
| **Song info and tag editing** (title, artists, album; update cover; MusicBrainz lookup to fill metadata) | RiPlay song menu "update title/authors", `RP/extensions/musicbrainz` (fetchers, fillers, workers) | Read-only tags (`lofty`) | M |
| **Exclude:** songs shorter than N, folders, blacklists (artist, album, song, playlist, folder) | RiPlay `excludeSongsWithDurationLimitKey`, `RP/ui/screens/blacklist`, `BlacklistType` | Missing (roadmap M52) | S–M |
| **Explicit filter / parental control**, video-vs-song filter | RiPlay `parentalControlEnabledKey`, `filterContentTypeKey`, `excludeSongIfIsVideoKey` | Missing | S |
| **Likes and dislikes** (a disliked list, hide from Quick picks) | RiPlay `showDislikedPlaylistKey`, "hide_from_quick_picks" | Favorites only | S |
| **Favorite albums and artists** | Nuclear `NP/views/Favorites/FavoriteAlbums.tsx`, `FavoriteArtists.tsx` | Tracks only | S–M |
| **Artist page from the library:** songs by the artist across albums | RiPlay `RP/ui/screens/artist/ArtistLibrarySongs.kt` | Albums only | S |
| **Set as ringtone**, share with external apps, fast share and QR share | RiPlay song menu `set_as_ringtone`, `RP/extensions/fastshare`, `RP/extensions/qrcodeanalyzer` | Missing | S each |

## 5. Playlists and collections

| Gap | Reference | Raygun today | Size |
|---|---|---|---|
| **Import playlists** by URL (Spotify, Deezer, YouTube via providers), from TuneMyMusic CSV, from Raygun/RiPlay exports, from JSON | Nuclear `PlaylistProvider.matchesUrl/fetchPlaylistByUrl`, `NP/views/PlaylistImport`, `NP/views/Playlists/PlaylistUrlImport.test.tsx`; RiPlay `ImportPlaylistType` (Riplay, TuneMyMusicDeezer, SpotifyUrl) | Missing (roadmap M52) | M |
| **Export playlists:** CSV, M3U8, JSON; share | RiPlay `ExportType` (CSV, M3U8), `RP/extensions/exporter/Exporter.kt`; Nuclear "export as JSON" | Missing | S |
| **Playlist folders** (create, rename, move a playlist into one) | Nuclear `PlaylistsHost.createFolder/movePlaylistToFolder`, `NP/stores/PlaylistIndexTree.ts` | Flat list | S–M |
| **Pinned playlists**, **monthly playlists** (auto-created per month), **top playlist** with a period (`TopPlaylistItems`, `topPlaylistPeriodKey`) | RiPlay `showPinnedPlaylistsKey`, `enableCreateMonthlyPlaylistsKey`, `showMyTopPlaylistKey` | Missing | S–M |
| **Built-in smart playlists:** Favorites, Disliked, Cached, Downloaded, My top, On device; a choice of which show | RiPlay `BuiltInPlaylist`, `showFavoritesPlaylistKey` … `showOnDevicePlaylistKey` | Favorites and History only | S |
| **Inside a playlist:** sort, filter (all, online, videos, favorites, local, unmatched, explicit), search, drag reorder, multi-select, "match song" for unmatched entries, swipe actions | RiPlay `PlaylistSongsTypeFilter`, `playlistSongSortByKey`, `PlaylistSwipeAction`, menu `match_song`; Nuclear `PlaylistDetail/usePlaylistEditing.ts` | Up/down/remove buttons while editing | M |
| **Playlist covers** (custom or 4-tile mosaic) and descriptions | Nuclear `buildThumbnails.ts`, `ui/src/components/Mosaic` | 4-tile covers exist (`PlaylistCover.tsx`); no custom cover or description | S |

## 6. Lyrics

| Gap | Reference | Raygun today | Size |
|---|---|---|---|
| **More lyrics sources:** KuGou, SimpMusic lyrics, YouTube captions | `RPm/kugou`, `RPm/simpmusiclyrics`, `RPm/lrclib` | LRCLIB only (`extensions/lrclib`) | S per extension |
| **Word-by-word (karaoke) sync** and fake-karaoke fallback | RiPlay `isShowingSynchronizedWordByWordLyricsKey`, `RP/extensions/lyricshelper/parsers/LRCLyricsKaraokeParser.kt`, `LyricsFakeKaraokeParser.kt`; Nuclear lyrics types "wordSynced / lineSynced / plain" (`NP/views/Lyrics/components/LyricsToolbar.tsx`) | Line-synced only | M |
| **Display options:** text size, auto-scroll toggle, timing offset (show earlier/later), alignment, colour, outline, highlight, background, size animation, full-screen lyrics, lyrics over the cover | Nuclear `lyrics.textSize`, `lyrics.autoScroll`, offset in `LyricsToolbar.tsx`; RiPlay `LyricsFontSize`, `LyricsAlignment`, `LyricsColor`, `LyricsOutline`, `LyricsHighlight`, `LyricsBackground`, `lyricsSizeAnimateKey`, `expandedlyricsKey`, `showlyricsthumbnailKey` | Fixed style; tap to seek | S–M |
| **Edit lyrics, search lyrics manually, pick another result** | RiPlay lyrics menu (`RP/ui/screens/player/common/Lyrics.kt`) | Missing | S–M |
| **Translate and romanize** (Off / Original / Translated / Both) | RiPlay `Romanization` enum, translate action | Missing. `feature-parity.md` marks "furigana/romaji ✅ M23", but there is no M23 spec and no furigana/romaji code in the repo (no ruby text, no tokenizer). That row is wrong | M (+ M47 translation) |
| **Lyrics in the notification / popup message** | RiPlay `playerEnableLyricsPopupMessageKey` | Missing | S |

## 7. Search

| Gap | Reference | Raygun today | Size |
|---|---|---|---|
| **Search history** (recent queries, delete one, clear all, pause history) | RiPlay `pauseSearchHistoryKey`, "clear_search_history" | Missing | S |
| **Suggestions while typing** and **voice search** | RiPlay `enableVoiceInputKey`, `Search.kt` suggestions | Missing | S–M |
| **Go to link:** paste a YouTube/Music link to open the song, album or playlist | RiPlay `RP/ui/screens/search/GoToLink.kt` | "Open link or file" plays a raw stream URL only | S |
| **Music recognition** (identify a song playing nearby; Shazam share intent) | RiPlay `RP/extensions/audiotag`, `RPm/audiotaginfo`, `RP/extensions/shazam/HandleShazamShare.kt`, `MusicIdentifierProvider`, API key setting | Missing | M |
| **Local-only search mode** | RiPlay `LocalSongSearch.kt` | Library matches appear in music search; there's no local-only mode | S |

## 8. Offline: caching and downloads for streamed music

| Gap | Reference | Raygun today | Size |
|---|---|---|---|
| **Stream cache:** played songs kept on disk with a max size and location; pre-cache the next songs; a "Cached" playlist | RiPlay `exoPlayerDiskCacheMaxSizeKey`, `exoPlayerCacheLocationKey`, `enablePreCacheKey`, `CacheType` | No audio cache (only the image page cache) | M |
| **Download songs, albums and playlists** for offline; a "Downloaded" playlist; a download cache with its own size | RiPlay `exoPlayerDiskDownloadCacheMaxSizeKey`, `showDownloadedPlaylistKey`, player "download" button, "Music vault" for personal offline copies (`musicVaultEnabledKey`) | Podcasts and videos download (M37, M43); streamed music doesn't | M (reuse M43's downloader) |
| **Metered network rule** (no streaming or caching on mobile data) | RiPlay `isConnectionMeteredEnabledKey` | Missing | S |

## 9. History, statistics, Rewind

| Gap | Reference | Raygun today | Size |
|---|---|---|---|
| **History screen** grouped by day, with paging, row actions and a "pause history" switch | Nuclear `NP/views/History` (`HistoryList`, `useDayMarker`), `history.enabled`; RiPlay `RP/ui/screens/history`, `pauseListenHistoryKey`, `HistoryType` (local or online) | Home's Recently played (latest play per track) and Clear history | S |
| **Listening stats:** time listened, top songs, artists, albums and playlists over Today, 1 week, 1/3/6 months, 1 year or All; hourly and day-of-week charts | RiPlay `RP/ui/screens/statistics`, `Statistics` enum, `maxStatisticsItemsKey`; Nuclear `NP/views/History/hooks/useHistoryStats.ts`, `HistoryTopLists`, `ui/src/components/HistoryCharts` | Missing (roadmap M52) | M |
| **Rewind** (yearly recap slides: top songs, artists, albums, playlists, listener level) | `RP/extensions/rewind/slides/*` | Missing | M |
| **Listener levels and badges** (monthly and annual) | `RP/extensions/listenerlevel` | Missing | S |
| **Play-event rules:** minimum listening time to count a play, reset play events, reset unplayable tracks | RiPlay `exoPlayerMinTimeForEventKey`, "reset_playback_events", "reset_unplayable_tracks" | Fixed rule (half the track or 4 minutes, M18) | S |
| **Timeline view** | `RP/extensions/timeline/TimelineVertical.kt` | Missing | S |

## 10. Scrobbling, accounts and integrations

| Gap | Reference | Raygun today | Size |
|---|---|---|---|
| **Last.fm scrobbling** (simple, or with now playing), web login | `RPm/lastfm`, `RP/extensions/lastfm/LastFmAuthScreen.kt`, `LastFmScrobbleType` | Missing (roadmap M54, needs the user's account) | M |
| **ListenBrainz** (token-based; neither reference has it) | — | Missing | S–M |
| **Discord rich presence** (with token) | `RP/extensions/discord`; Nuclear `integrations.discord.enabled` | Missing | S–M |
| **YouTube Music account sync** (likes, playlists, history) | RiPlay `enableYouTubeSyncKey`, `RP/extensions/accountlogin` | Missing (and against the M15 decision on YouTube terms) | ✖ likely |
| **WebDAV:** database backup and restore, and a WebDAV music library | `RP/extensions/webdavlibrary` | Missing (roadmap M50 has WebDAV backup) | M |

## 11. Android system integration

| Gap | Reference | Raygun today | Size |
|---|---|---|---|
| **Android Auto browse tree:** library, playlists, favorites, top songs, on device, podcasts, pinned, monthly, shuffle; grid option and song limit | `RP/services/playback/MediaLibraryServiceCallback.kt`, `PlayerService.kt`, settings `aa_show_*`, `AndroidAutoPlaylistLimit` | `RaygunPlaybackService` is a `MediaSessionService` holding one item, so Auto shows only the current track | L |
| **Notification buttons:** pick two extra (favorite, repeat, shuffle, radio, search), cover on the lock screen, cover as wallpaper | RiPlay `NotificationButtons`, `notificationPlayerFirstIconKey/SecondIconKey`, `isShowingThumbnailInLockscreenKey`, `enableWallpaperKey`/`WallpaperType` | Media3 default prev/play/next | S–M |
| **Home-screen widgets** (horizontal and vertical player) | `RP/ui/widgets/PlayerHorizontalWidget.kt`, `PlayerVerticalWidget.kt` | Missing | M |
| **Cast** (Chromecast; RiPlay's own LAN receiver "RiTune") | `androidFull/.../cast/FullCastHelper.kt`, `RP/cast/ritune`, `castToRiTuneDeviceEnabledKey` | Missing (roadmap M52) | L |
| **Android TV player** | `RP/ui/screens/player/unified/TvUnifiedPlayer.kt` | Missing | L |
| **Self-closing schedule, keep screen on, close on swipe-out, ignore battery optimization** | RiPlay `isKeepScreenOnEnabledKey`, `closebackgroundPlayerKey`, "service_lifetime", "ignore_battery_optimizations" | Missing | S |

## 12. Appearance and UI settings

| Gap | Reference | Raygun today | Size |
|---|---|---|---|
| **Colour palettes:** Default, Dynamic (from the cover), Pure black, Modern black, Material You, Customized (editable light and dark colour sets: 5 backgrounds, text, secondary and disabled text, player icon, accent), custom colour | RiPlay `ColorPaletteName`, `ColorPaletteMode` (Light/Dark/PitchBlack/System), `RP/ui/screens/settings/UiSettings.kt` | Theme (system/light/dark/pure black) and seven accents | M |
| **Dynamic colours from the playing cover** across the app | RiPlay `Dynamic` palette | Missing | S–M |
| **Fonts** (Rubik, Poppins, system), font padding | RiPlay `FontType`, `useSystemFontKey`, `applyFontPaddingKey` | Missing | S |
| **Navigation:** bar position (bottom, left, right), icons only or with text, transition effects (slide, scale, fade, expand, none), menu style (list or grid), message style, swipe-to-action on lists | RiPlay `NavigationBarPosition`, `NavigationBarType`, `TransitionEffect`, `MenuStyle`, `MessageType`, `isSwipeToActionEnabledKey` + per-list swipe actions | Nav labels setting only | S–M |
| **Appearance presets:** export and import the player and appearance settings | RiPlay `export_appearance_settings`, `activeAppearancePresetIdKey` | Missing | S |
| **Theme marketplace and custom CSS themes** | Nuclear `NP/views/Themes` (Basic, Advanced, Marketplace, My themes), `services/marketplaceThemeDirService.ts` | Missing (roadmap M52) | M |
| **Home customization:** choose sections (Quick picks, tips, charts, moods, new albums, related, similar artists, playlists you might like, monthly playlists, listener levels), Classic or Tabbed home | RiPlay `RP/ui/screens/settings/HomeSettings.kt`, `HomeType`, `HomePages` | Home sections are fixed | S |
| **Fun extras:** snowfall effect, vinyl or turntable cover, cover rotation, cassette | RiPlay `showSnowfallEffectKey`, `VinylSizeKey`, `albumCoverRotationKey`, `RP/ui/components/themed/Turntable.kt`, `Cassette.kt` | Missing | S each |

## 13. Plugins and sources

| Gap | Reference | Raygun today | Size |
|---|---|---|---|
| **Plugin auto-update** setting | Nuclear `plugins.autoUpdate` | Updates are manual (M13) | S |
| **Provider kinds Raygun's SDK lacks:** `dashboard`, `discovery` (recommendations), `playlists` (import by URL), artist and album metadata capabilities | `NS/types/dashboard.ts`, `discovery.ts`, `playlists.ts`, `search.ts` | `music.metadata` (track search) and `music.stream`, `lyrics`, `podcast.directory` | M (SDK + host + nuclear-compat mapping) |
| **Nuclear-compat:** map Nuclear's dashboard, discovery and artist/album capabilities once Raygun has them | `packages/nuclear-compat` | Converts search and streaming only (M17) | M |

## 14. Podcasts

The references are thin here: RiPlay's podcasts are YouTube Music podcasts (`RP/ui/screens/podcast`). These gaps are relative to RiPlay plus the playback features above:
- **Speed, skip silence, sleep timer and a skip-back/forward step** for episodes. RiPlay has speed and skip silence app-wide. (S, shared with §2)
- **Android Auto podcast row:** RiPlay `showPodcastAAKey`. (part of §11)
- **Episode queue:** play next or add to queue from episode rows, continuing to the next episode. (S)
- **Beyond the references, but expected of a podcast app:** auto-download new episodes per show, keep the latest N downloads, delete when played, episode sort and filter (unplayed, downloaded), mark all played, OPML import/export, chapters, per-show skip intro/outro. (M together)

## 15. Data, network and diagnostics (music-adjacent)

| Gap | Reference | Raygun today | Size |
|---|---|---|---|
| **Cache sizes and locations** (image, song, download), cache cleared indicator | RiPlay `RP/ui/screens/settings/DataSettings.kt`, `CacheSpaceIndicator.kt`, `CoilDiskCacheSize`, `ExoPlayerDiskCacheMaxSize` | Fixed page-cache size | S |
| **Proxy (host, port, mode), DNS over HTTPS (presets or custom)** | RiPlay `isProxyEnabledKey`, `DnsOverHttpsType`, `customDnsOverHttpsServerKey` | Missing (roadmap M51 has DoH) | S–M |
| **Debug log export, crash log export, rescue center** | RiPlay `RP/ui/screens/settings/MiscSettings.kt`, `RP/extensions/rescuecenter`, `RP/extensions/crashreporter`; Nuclear `NP/views/Logs` | Logs exist on disk; no viewer or export (roadmap M51) | S |
| **What's new, update check, onboarding** | Nuclear `NP/views/WhatsNew`, `updates.checkForUpdates/autoInstall`; RiPlay `RP/ui/screens/onboarding`, `RP/extensions/updater` | Missing (roadmap M55) | S–M |
| **Automatic backup on a schedule** | RiPlay `RP/extensions/scheduled/PeriodicAutoBackup.kt`, `autoBackupFolderKey` | Manual backups (M30) | S |

---

## Settings inventories (for building Raygun's Settings → Music/Audio screens)

**Nuclear core settings** (`NP/services/coreSettings.ts`):
- appearance (frameless window, custom title bar, title bar style);
- theme (type, id, dark);
- playback (volume, muted, shuffle, repeat, discovery, discovery variety, skip seconds, crossfade ms, stream expiry, resolution retries, stream verification, verification service, author id);
- history enabled;
- layout sidebar widths;
- language;
- updates (check, auto install);
- plugin auto-update;
- integrations: Jam remote (enabled, URLs), MCP (enabled, URL), MPD (enabled, URL), Discord;
- lyrics (text size, auto-scroll).

**RiPlay screens** (`RP/ui/screens/settings/*`; keys in `RP/extensions/preferences/Preferences.kt`):
- **General:**
  - app: check update, language, QR actions;
  - network: metered, DNS over HTTPS (custom server), proxy (mode, host, port);
  - service lifetime, keep screen on, battery optimization, voice input, behaviour on swipe-out;
  - video content mode, jump-previous, minimum listening time, exclude short songs, pause between songs, pause on volume zero, keep the player minimized, disable swipe-down close;
  - queue: auto-load songs into the queue, max songs, content-type filter, discover, playlist and now-playing indicators, persistent queue, resume on start, close with back, skip on error;
  - audio: skip silence with a minimum length, parental control, shake, PiP (module, auto), equalizer, crossfade, loudness normalization (base gain, boost), bass boost and level, reverb, audio focus, disable DRC;
  - cast (RiTune, type), automatic playback (Bluetooth and wired devices);
  - playback events (pause history, reset events, reset unplayable), search history (pause, clear);
  - Android Auto (grid, playlist limit, monthly, podcasts, pinned, library, on device, top songs, all songs, shuffle).
- **UI:**
  - fullscreen, interface type, theme with custom colour sets, theme mode;
  - navigation bar position and type, player position, up next in the mini player, mini player in settings;
  - loader, menu and message styles, transition effect, snow effect, search button in the nav bar, stats in the nav bar, floating icon;
  - fonts and font padding, swipe actions (queue, playlist and album, left and right), image placeholders;
  - which built-in playlists show, monthly playlists, smart recommendations, statistics (max items, listening time), top playlist, listener levels, reset to defaults.
- **Player appearance:**
  - themes manager and presets, transitions and animation, top actions bar, player type, queue type;
  - thumbnail (show, swipe, fading edge, carousel and size, pause effect, lyrics on thumbnail, visualizer on thumbnail, cover animation, size, type, roundness, no blur);
  - stats for nerds, info type (album/artist, icons), mini player type, swap controls with the timeline, timeline (transparent, size, seek with tap), controls type, play button and zoom-out, like icon;
  - background (gradient type, black or bottom gradient, text outline), total queue time, remaining time, next songs (two songs, count), album cover, scrolling text, swipe toggles;
  - lyrics (tap cover to toggle, tap line, background, popup message), background progress bar;
  - visualizer (player, mini player and EQ styles);
  - action bar (transparent, even spacing, tap or swipe up for the queue, and buttons: video, discover, download, add to playlist, loop, shuffle, lyrics, sleep timer, equalizer, arrow, start radio, menu);
  - expanded player sections, notification (cover), lock screen cover, wallpaper, export and import of appearance.
- **Data:** cache (image size, song size, custom location, clear), backup and restore, database export and import.
- **Home:** home page type, listener levels, Quick picks (tips, new albums of your artists, new albums, moods & genres, charts, related albums, similar artists, playlists you might like, monthly playlists).
- **Misc:** music vault, on-device music folder, debug log, export log and crash log.
- **Accounts:** WebDAV (cloud accounts, music, backup, restore), YouTube Music login and sync, Last.fm (scrobble type), Discord presence, music identifier (provider, API key, quota).

---

## Summary: the 15 most valuable gaps

1. **Queue management:** play next or add to queue from every menu, drag to reorder, swipe to remove, clear, save as playlist. (M)
2. **Persistent queue** and resume playback on start. (M)
3. **Audio controls:** playback speed and pitch, skip silence, loudness normalization (Media3 built-ins; podcasts benefit too). (S)
4. **Sleep timer** (minutes or end of song). (S)
5. **Equalizer** with presets, bass boost, reverb, or the system EQ. (M)
6. **Online artist and album pages** (bio, top tracks, albums, similar artists) and **search by category** (artists, albums, playlists): new SDK capabilities plus extensions. (L)
7. **An Explore landing in Search:** charts, moods and genres, new releases, Quick picks from history, radio browsing; the user's library stays separate. (M)
8. **Endless radio / auto-continue:** start radio from a song, and add related tracks at the end of the queue (needs a `discovery` capability). (M)
9. **Offline for streamed music:** stream cache with a size limit, and download songs, albums and playlists (reuse M43's downloader). (M)
10. **Listening stats and history:** day-grouped history, top lists by period, charts, then Rewind and listener levels. (M)
11. **Playlist import/export:** URL (Spotify/Deezer/YouTube), CSV, M3U8, JSON; playlist folders, pinned and smart playlists. (M)
12. **Lyrics depth:**
    - more sources (KuGou, SimpMusic);
    - word-by-word sync;
    - size, offset and auto-scroll options;
    - manual search and edit;
    - romanization/translation. Fix the false "furigana ✅ M23" row.
13. **Local library options:** sort and views, folders browser, tag editing with MusicBrainz, blacklists, short-song exclusion. (S–M)
14. **Android Auto browse tree, notification buttons, home-screen widgets.** (L/M)
15. **Player customization and Last.fm/ListenBrainz scrobbling:**
    - background styles, action-bar buttons, mini-player swipes, visualizer, dynamic colours from the cover;
    - scrobbling with the user's tokens. (M)
