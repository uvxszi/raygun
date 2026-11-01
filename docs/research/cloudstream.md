# CloudStream vs. Raygun — feature gaps (research, 2026-11-01)

Source: `reference/cloudstream` (GPL-3.0; adapted code needs a header naming it). Paths are under
`app/src/main/java/com/lagradost/cloudstream3/` unless they start with `res/`.

Compared against Raygun after M43:
- `docs/feature-parity.md` and the M38–M43 specs;
- `apps/raygun/src` (watch/, search/, Home, Browse, Search and both title pages, settings);
- `VideoPlayerActivity.kt`;
- `raygun-app` video, stremio, video_sources and video_downloads.

Sizes: **S** is a day or less, **M** is a few days, **L** is a milestone or more.

---

## 0. How CloudStream splits discovery from "your stuff"

- **Bottom navigation** (`res/menu/bottom_nav_menu.xml`): **Home · Search · Library · Downloads · Settings**.
- **Discovery is Home and Search only.**
  - Home is one provider's home page at a time (picked in a provider dialog), plus your resume row and bookmark lists on top.
  - Search is where every provider is queried.
  - Nothing in Library or Downloads links out to browse.
- **Library** (`ui/library/LibraryFragment.kt`, `syncproviders/providers/LocalList.kt`) is only what you saved, as **lists**:
  - Watching, Completed, On hold, Dropped, Plan to watch, Favorites and Subscriptions locally;
  - or the lists of a linked account (MAL, AniList, Simkl, Kitsu), switched from a dropdown.
  - It has search and sort, and an "open with" choice per list (`LibraryOpenerType`: default, provider, browser, search).
- **Downloads** is a tab of its own.
- **Raygun differs:**
  - Raygun has a Browse tab of sources, and Library → Videos linked to Browse and Addons until now.
  - Raygun's Library uses Mihon categories and has no watch-status lists.
- **Gap:** Library should be lists of your titles only, with discovery in Search and source home pages (see §2).

---

## 1. Watch status, favorites, subscriptions (Library) — most valuable

| Gap | CloudStream | Raygun today | Size |
|---|---|---|---|
| Watch status per title | `ui/WatchType.kt`: Watching, Completed, On hold, Dropped, Plan to watch, None. Set from a bookmark dropdown on the result page (`result_bookmark_button`, `result_bookmark_fab`). | Only in library or not, plus categories. | M |
| Status lists as Library tabs | `LocalList.library()` builds one page per status, plus Favorites and Subscriptions; tabs in `ui/library/ViewpagerAdapter.kt`. | Category tabs only. Statuses could be built-in lists next to categories. | M |
| Favorites (separate from bookmarks) | `DataStoreHelper.setFavoritesData`; heart on the result page (`result_favorite`). | None for video (music has favorites). | S |
| Subscriptions with new-episode notifications | Bell on the result page (`result_subscribe`). `services/SubscriptionWorkManager.kt` is a periodic WorkManager job that checks subscribed series and posts notifications. | M43 library update finds new episodes, but there's no notification and no per-title subscribe. | M |
| Library sorting | `ListSorting`: rating high/low, updated new/old, A–Z/Z–A, release date new/old, plus a query filter (`ui/library/LibraryViewModel.kt`). | Title, last watched, latest episode, unwatched, added. No rating or release date (Raygun stores no rating). | S |
| Open-with per list | `LibraryOpener`: open in provider, browser, search, or default. | Always opens the title page. | S |
| Random | Random button on the Library and Home (`random_button_settings`). | None. | S |
| Delete or move many | Multi-select on Home bookmarks (`HomeViewModel.deleteBookmarks`), and delete in the expanded resume list (`homeExpandedDelete`). | One at a time. | S |

---

## 2. Home screen (discovery)

| Gap | CloudStream | Raygun today | Size |
|---|---|---|---|
| A provider's home page as Home | `ui/home/HomeViewModel.load(api)` loads `MainAPI.getMainPage` rows. The provider is picked in `HomeFragment.selectHomepage`: a dialog listing providers, filtered by media type chips, with **pinned providers** (long-press) and a settings cog per provider. | Home is resume rows. Source catalogues live in Browse (Stremio rows, per-source pages). | M |
| Hero preview carousel | `HomeParentItemAdapterPreview.kt` and `HomeScrollAdapter.kt`: a swipeable backdrop carousel with title, tags, a Play/Info pair and bookmark. Infinite load via `loadMoreHomeScrollResponses`. | None. | M |
| Expandable rows | `expandAndReturn` / `expand` load more pages of a row, opened as a full-screen grid (`homeExpanded*`). | Stremio rows have "See all", which opens `/watch/catalog`. Source rows have no rows. | S |
| Media type chips | `HomeFragment.getPairList`: Anime, Cartoons, TV, Documentaries, Movies, Asian, Livestream, Torrent, NSFW, Other. They filter providers and rows. | None. | S |
| Resume row with delete-all and per-item remove | `getResumeWatching`, `deleteResumeWatching`. Episode, season and progress shown; also "from download". | Continue watching exists (remove per item). No "clear all", and no episode or season shown separately from the subtitle. | S |
| Bookmark rows on Home | `loadStoredData(preferredWatchStatus)`: chips for each status you use, showing that list. | None. | S |
| Android TV "Continue watching" channel | `addProgramsToContinueWatching` (TV only). | N/A (phone first). | — |

---

## 3. Search

| Gap | CloudStream | Raygun today | Size |
|---|---|---|---|
| Choose providers to search | `SearchFragment`, `selectedApis` with `DataStoreHelper.searchPreferenceProviders`: a dialog with type chips; the choice persists. | Searches every Stremio catalog and every video source. No per-search provider choice. | S |
| Type chips in search | The same `tvtypes_chips` as Home filter results by type. | Search types are medium (music, video, manga...), not movie/series/anime/documentary. | S |
| Search history | `SearchHistoryAdaptor.kt`; `searchViewModel.updateHistory()`. Shown when the query is empty, remembers the types, swipe or long-press to remove, "clear history" dialog. | None. | S |
| Search suggestions | `ui/search/SearchSuggestionApi.kt` (TMDB `search/multi` autocomplete) behind the "Search suggestions" setting. | None. | S |
| Grouped vs. merged results | "Advanced search" setting (`advanced_search`): grouped by provider (`searchMasterRecycler`), or one merged grid (`searchAutofitResults`). | Grouped rows only. | S |
| Long-press a result | `SEARCH_ACTION_SHOW_METADATA` shows a quick info sheet; `SEARCH_ACTION_PLAY_FILE` plays at once. | None. | S |
| Quick search to link titles | `ui/quicksearch/QuickSearchFragment.kt`: search other providers for the same title, from a result or a library item. | None ("search other sources for this title"). | M |
| Hide qualities in results | `pref_filter_search_quality`: hide results tagged with chosen qualities (CAM, etc.). | None. | S |

---

## 4. Result (title) page

Ids from `res/layout/fragment_result*.xml`; logic in `ui/result/ResultViewModel2.kt`.

| Gap | CloudStream | Raygun today | Size |
|---|---|---|---|
| Watch status, favorite, subscribe buttons | `result_bookmark_button`, `result_favorite`, `result_subscribe`. | Add to library and categories only. | (§1) |
| Episode ranges | `EpisodeRange` and `rangeSelections`: long series are split into 1–50, 51–100... with a selector. | Season tabs only; a 1000-episode anime is one long list. | S |
| Dub/Sub selection | `DubStatus` with `dubSubSelections` / `selectedDubStatus`, and a "Display Dubbed/Subbed" provider setting. | None (aniyomi-style sources can't say dub or sub). | M (needs SDK fields) |
| Episode sort | `EpisodeSortType`: number asc/desc, rating high/low, date newest/oldest (`result_sort_button`). | Source order only. | S |
| Next airing countdown | `nextAiringDate` / `nextAiringEpisode` (`result_next_airing*`): "Episode 5 in 3d 4h". | Upcoming is only on Updates → Episodes. The title page doesn't show it. | S |
| Coming-soon state | `result_coming_soon`. | Upcoming episodes are listed like aired ones. | S |
| Recommendations | `recommendations`, `result_recommendations` (+ toggle button). | None. Stremio metas don't carry them; sources would need an SDK field. | M |
| Cast with photos | `ActorAdaptor.kt` and `result_cast_items`, with the "Show cast panel" setting. | Cast as plain text. | S |
| Trailer inline in the header | `ResultTrailerPlayer.kt` auto-plays the trailer muted behind the header ("Show trailers" setting). | Trailer button for Stremio only, which opens the YouTube player. | M |
| Batch download | `result_batch_download_button`: download a season or range. | One episode at a time. | S |
| Resume card | `result_resume_parent`: next episode with progress and a "Resume S1E3" button. | Floating Resume/Start button; no progress card. | S |
| Next-series button | `result_next_series_button`. | None. | S |
| Fillers | `fillers` (and a "Show filler episode for anime" setting) marks filler episodes. | None. | M (needs a filler source) |
| Meta chips | `result_meta_*`: type, year, rating, status, duration, content rating, site. | Type, year, source (no rating, duration or content rating for sources). | S |
| Share / open in browser / search | `result_share`, `result_open_in_browser`, `result_search`. | Video source pages have open in browser; no share; no "search this title elsewhere". | S |
| Sync panel | `result_add_sync`, `result_mini_sync`, `ui/result/SyncViewModel.kt`: link the title to MAL/AniList/Simkl and set score, status and episodes from the page. | ⏸ M54 (accounts). | L |
| Per-episode long-press menu | `EpisodeAdapter` actions (lines 46–67): play in app, Chromecast episode/mirror, download, download mirror, reload links, show description, download subtitle (or its mirror), mark watched, **mark watched up to here**, copy link, external players. | Tap plays or opens the picker; separate download and watched buttons. No "mark watched up to here", no reload, no episode description. | M |
| Episode thumbnails, descriptions and ratings for sources | `EpisodeAdapter` shows poster, description and rating. | Stremio episodes have thumbnails and overview; source episodes are text only. | S (SDK fields) |
| VPN / connection error help | `result_vpn`, `result_reload_connection*`. | Generic error text. | S |

---

## 5. Player

Files: `ui/player/FullScreenPlayer.kt` and `GeneratorPlayer.kt`; controls in `res/layout/player_custom_layout.xml`.

| Gap | CloudStream | Raygun today | Size |
|---|---|---|---|
| Sources dialog inside the player | `showMirrorsDialogue` (GeneratorPlayer:1015): one sheet listing **all sources and subtitles side by side**, plus "load from file" and subtitle settings. `nextMirror()` falls back to the next link on error. | Links are picked before playback; to switch you leave the player. No automatic next mirror. | M |
| Next episode | `player_skip_episode`, `nextEpisode()`, the "Autoplay next episode" setting. | None: back to the title page. | M |
| Episode list overlay | `showEpisodesOverlay` (`player_episode_overlay`, `player_episodes_button`). | None. | M |
| Skip intro/outro | `player_skip_op` with stamps from `utils/videoskip/` (`AniSkip.kt`, `AnimeSkip.kt`, `IntroDbSkip.kt`, `TheIntroDBSkip.kt`). `SkipType`: opening, ending, recap, mixed OP/ED, credits, intro, preview. Plus a "Skip OP" +85 s button (`video_skip_op`). | None (parity row: 🔜). | M |
| Online subtitle search | `addFirstSub` / `SubtitleSearch` (GeneratorPlayer:594–1100) across subtitle providers: `syncproviders/providers/OpenSubtitlesApi.kt`, `Subdl.kt`, `SubSource.kt`, `Addic7ed.kt`. | Stremio subtitle addons only, outside the player. | M |
| Load subtitles from a file | "Load from file" footer (GeneratorPlayer:1037). | None. | S |
| Subtitle offset (sync) | `showSubtitleOffsetDialog` (`player_subtitle_offset_btt`, `SubtitleOffsetItemAdapter.kt`): shows the cues around now to line them up. | None. | S |
| Subtitle styling | `ui/subtitles/SubtitlesFragment.kt` `SaveCaptionStyle`: foreground, background and window colours; edge type and colour; font or custom typeface file; elevation; fixed size; edge size; remove captions ([SDH] cues); remove bloat; upper case; bold; italic; background radius; alignment. A separate style for Chromecast. | Media3 defaults. | M |
| Speed dialog and long-press 2× | `showSpeedDialog`, `player_speedup_button` (LongPress Speed Toggle). | Speed via the stock Media3 menu; no long-press. | S |
| Resize / zoom | `player_resize_btt`: fit, fill, zoom (the "Player resize button" setting). | None. | S |
| Lock controls | `player_lock`. | None. | S |
| Rotate and auto-rotate | `player_rotate_btt`; "Rotate" and "Auto rotate" settings. | Fixed landscape. | S |
| Seek-bar preview thumbnails | `PreviewGenerator.kt` ("Seekbar preview"). | None. | M |
| Dynamic range compressor | `DynamicRangeCompressor.kt` (`player_compressor_btt`). | None. | S |
| Extra brightness | "Extra brightness" filter past 100%. | Brightness swipe up to system max. | S |
| Swipe to seek; double-tap to pause | Gesture settings (`pref_category_gestures`). | Double-tap seek and swipes for brightness and volume, with no toggles. | S |
| Metadata overlay on pause | `showPlayerMetadata` (`player_metadata_overlay`: logo, title, overview). | None. | S |
| Title, info and clock | Title max chars, show name, resolution and media info (`limit_title_rez`), "Hide names of the player's controls", clock (`player_video_clock`). | Title only. | S |
| Restart and go-back buttons | `player_restart`, `player_go_back` / `player_go_forward`. | None. | S |
| Software decoding | "Software decoding" setting. | None. | S |
| Buffer settings | Video cache on disk, buffer size, buffer length, clear cache (`video_buffer_*`). | Fixed. | S |
| Quality by network | "Preferred watch quality (WiFi)" and "(Mobile Data)". | One preferred quality. | S |
| External players | `actions/temp/*`: VLC, MPV, mpv-kt, mpvRx, Just Player, Next Player, Web Video Cast, FCast, Only Player, Play in browser, copy link, view M3U8, plus Aria2/BiglyBT/LibreTorrent for downloads. "Preferred video player" setting with an always-ask option (`AlwaysAskAction.kt`). | None (parity row says 🔜 M38). | M |
| Chromecast | `player_media_route_button`, `ACTION_CHROME_CAST_*`, `ui/subtitles/ChromecastSubtitles*`. | 🔜 M52. | L |
| Livestream player | `ui/player/live/`. | Live HLS plays, with no live UI. | S |
| Torrent playback | `ui/player/Torrent.kt`. | ✖ (legal risk; parity row). | — |
| Open local files | `DownloadedPlayerActivity.kt`, `OfflinePlaybackHelper.kt`: open a video file or intent with CloudStream. | "Open link or file" sheet in Library. | S |

---

## 6. Source priority and quality profiles

`ui/player/source_priority/`:
- **Profiles** (`QualityDataHelper.getProfiles`) with types: WiFi, mobile data, download, Chromecast...
- **Each profile** sets a priority per source name (`setSourcePriority`) and per quality (`setQualityPriority`).
- **Dialogs:** `SourcePriorityDialog.kt`, `QualityProfileDialog.kt` and `SourceProfileSettingsDialog.kt` edit them.
- **Use:** links are sorted by the active profile.

Raygun has one preferred height and the source's order (parity row: "priority across sources 🔜"). **M**

---

## 7. Downloads

| Gap | CloudStream | Raygun today | Size |
|---|---|---|---|
| Foreground service and notifications | `services/DownloadQueueService.kt`, `VideoDownloadService.kt`: a persistent notification with counts and per-download progress; `receivers/VideoDownloadRestartReceiver.kt` restarts after a kill. | Downloads run only while the app is open; no notification. | M |
| Parallel downloads and connections | Settings "Parallel downloads" and "Concurrent connections". | One at a time; four HLS segments in parallel. | S |
| Download folder | "Download path" (system picker, SAF). | Fixed `downloads/video/`. | M |
| Queue screen | `ui/download/queue/DownloadQueueFragment.kt`, menu `cancel_all`. | Videos tab shows queued and running, with no "cancel all". | S |
| Grouped by series | `DownloadFragment` lists series; `DownloadChildFragment` lists that series' episodes. | Flat list. | S |
| Storage bar | `updateStorageInfo`: used by the app, by others, and free. | Total size of saved videos. | S |
| Multi-select delete | `updateDeleteButton(count, selectedBytes)`. | One at a time. | S |
| Download subtitles separately | `ACTION_DOWNLOAD_EPISODE_SUBTITLE`. | Subtitles come with the video (up to 8). | S |
| Play a stream from Downloads | `showStreamInputDialog` (URL, referer, HLS switch). | Library "Open link or file". | — |
| Battery optimisation prompt | "Disable Battery optimization" in General. | None. | S |

---

## 8. Extensions and repositories UI

`ui/settings/extensions/`:
- **Repositories list** (`ExtensionsFragment`, `RepoAdapter`) with **plugin stats**: downloaded, disabled, not downloaded (`PluginStats`, `loadStats`).
- **Plugins per repository** (`PluginsFragment`, `PluginsViewModel`):
  - search, filtered by TV types (`filterTvTypes`) and language (`filterLang`, menu `lang_filter`);
  - **download all** (`downloadAll`, menu `download_all`).
- **Plugin details** (`PluginDetailsFragment`): description, size, authors, types, language, GitHub link, settings, **votes** (upvote).
- **Updates:** "Automatic plugin updates" and "Automatically download plugins" (new plugins from your repos); "Update plugins" now.
- **Provider settings:** "Extension languages" and "Test all extensions" (`ui/settings/testing/`: runs each provider's search/load/links and reports).

Raygun has repositories, browse, install and settings with detail pages. Missing:
- language and type filters;
- download all;
- size and votes;
- automatic updates of installed extensions;
- an extension self-test screen.

**S–M each**

---

## 9. Accounts, profiles, sync (UI only)

- **Sync accounts** (`syncproviders/providers/`): MAL, Kitsu, AniList, Simkl (list sync, scores, episode progress; the "Update watch progress" setting syncs automatically). Subtitle accounts: OpenSubtitles, Subdl; skip accounts: AnimeSkip. ⏸ M54 in Raygun. **L**
- **Local profiles** (`ui/account/`):
  - several profiles with images, each with its own bookmarks, progress and settings;
  - a PIN lock per profile (`AccountHelper.showPinInputDialog`);
  - an account picker at startup ("Skip account selection at startup").
  - Raygun has none. **M**
- **App lock:** "Lock with Biometrics" (Security). Raygun has none. **S**

---

## 10. Backup, updates, setup, misc.

- **Backup** (`utils/BackupUtils.kt`, `services/BackupWorkManager.kt`):
  - manual backup and restore;
  - **automatic backup frequency**;
  - **backup folder location**.
  - Raygun has manual backup and restore only. **S**
- **App updates** (`ui/settings/ApkUpdater.kt`, `GithubReleases.kt`): check for update, pre-release channel, APK installer choice. Raygun has none (store or sideload). **M**
- **First-run setup** (`ui/setup/`): language → provider languages → preferred media (movies/anime/...) → layout → extensions (add the default repo and pick plugins). Raygun has none. **M**
- **Network:**
  - **DNS over HTTPS** (Cloudflare, Google, AdGuard...);
  - "Clone site" (add a copy of a provider under a new domain);
  - **GitHub proxy** (jsDelivr) for repositories in blocked regions.
  - Raygun has none. **S–M**
- **Logcat viewer** (`ui/settings/logcat/`) and **Redo setup**. Raygun has logs only through adb. **S**
- **Confirm before exiting** (UI setting). **S**

---

## 11. Settings screens — exact options (CloudStream) and Raygun status

**General** (`res/xml/settings_general.xml`):
- app language ✅;
- legal notice;
- download path ✖;
- parallel downloads ✖;
- concurrent connections ✖;
- battery optimisation ✖;
- clone site ✖;
- DNS over HTTPS ✖;
- GitHub proxy ✖;
- links (GitHub, Discord, wiki).

**Player** (`settings_player.xml`):
- **Defaults:**
  - update watch progress (sync) ⏸;
  - preferred quality on Wi-Fi ✅ (one setting) and on mobile data ✖;
  - preferred video player ✖.
- **Layout:**
  - title max chars ✖;
  - show name, resolution and media info ✖;
  - hide control names ✖.
- **Subtitles:** subtitle style ✖; Chromecast subtitle style ✖.
- **Features:**
  - source priority ✖;
  - PiP ✅ (always on; no toggle);
  - resize button ✖;
  - playback speed ✖ (stock menu);
  - dynamic range compressor ✖;
  - long-press speed ✖;
  - autoplay next ✖;
  - skip OP ✖;
  - rotate and auto-rotate ✖;
  - seek-bar preview ✖;
  - software decoding ✖;
  - extra brightness ✖.
- **Gestures:**
  - swipe to seek ✖;
  - swipe for brightness/volume ✅ (no toggle);
  - double-tap to seek ✅ (no toggle);
  - double-tap to pause ✖;
  - seek amount ✅.
- **Cache:** disk cache ✖; buffer size ✖; buffer length ✖; clear cache ✖.

**Providers** (`settings_providers.xml`):
- extension languages ✖;
- preferred media ✖;
- dubbed/subbed display ✖;
- NSFW ✅;
- test all extensions ✖.

**UI** (`settings_ui.xml`):
- primary colour ✅ (accent);
- theme ✅;
- app layout (phone, TV, emulator) ✖;
- poster title location ✅ (cover style);
- overscan ✖;
- poster size ✅ (grid columns);
- toggle UI elements on posters ✅ (badges);
- advanced search ✖;
- search suggestions ✖;
- show trailers ✖;
- Kitsu posters ✖;
- cast panel ✖;
- fillers ✖;
- player metadata overlay ✖;
- clock (TV) —;
- random button ✖;
- confirm exit ✖;
- hide qualities in search ✖.

**Updates** (`settings_updates.xml`):
- check for update ✖;
- pre-release ✖;
- APK installer ✖;
- backup ✅;
- backup frequency ✖;
- restore ✅;
- backup folder ✖;
- automatic plugin updates ✖;
- auto-download plugins ✖;
- update plugins ✖ (Raygun reloads);
- logcat ✖;
- redo setup ✖.

**Account** (`settings_account.xml`):
- MAL, Kitsu, AniList, Simkl ⏸;
- OpenSubtitles ✖;
- Subdl ✖;
- AnimeSkip ✖;
- skip account select ✖;
- biometric lock ✖.

---

## Summary — 15 most valuable gaps

1. **Library as your lists.** Watch statuses (Watching, Plan to watch, On hold, Dropped, Completed), Favorites and Subscriptions as Library tabs next to categories, with sort and random. No discovery links in Library. (§0, §1)
2. **Subscriptions with new-episode notifications** (a WorkManager-style periodic check). (§1)
3. **Next episode and autoplay next**, plus an in-player episode list. (§5)
4. **In-player sources and subtitles sheet**, with automatic fallback to the next link on error. (§5)
5. **Skip intro/outro** (AniSkip, IntroDB-style stamps) and a +85 s skip button. (§5)
6. **Subtitle styling, offset sync, load from file and online subtitle search** (OpenSubtitles/Subdl). (§5)
7. **Downloads in a foreground service** with notifications, parallel downloads and grouping by series. (§7)
8. **Source priority and quality profiles**, Wi-Fi vs. mobile quality. (§6)
9. **Home with a provider's home page**, a hero carousel and media-type chips (discovery outside Library). (§2)
10. **Search history, suggestions, provider choice, type chips, grouped/merged toggle.** (§3)
11. **Title page depth:** episode ranges, sort, next-airing countdown, batch download, resume card, cast photos, inline trailer, "mark watched up to here". (§4)
12. **External players** (VLC, MPV...) with a preferred-player setting and always-ask. (§5)
13. **Player controls:** resize/zoom, lock, long-press 2×, rotate, seek preview, metadata overlay, gesture toggles, buffer settings. (§5)
14. **Extensions:** language and type filters, download all, auto-update, self-test, details with size. (§8)
15. **Profiles with PIN, biometric lock, automatic backups, a first-run setup wizard, and DNS over HTTPS.** (§9, §10)
