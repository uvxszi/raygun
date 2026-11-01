# Aniyomi (anime side) vs. Raygun — gap research

Researched 2026-11-01 against `reference/aniyomi` (Apache-2.0) and Raygun at M43 (`8887875`).
Covers what Aniyomi adds for anime beyond Mihon, and what Raygun lacks or has only in a simpler form.

**Path prefixes**
- `A/` = `reference/aniyomi/app/src/main/java/eu/kanade/`
- `D/` = `reference/aniyomi/domain/src/main/java/tachiyomi/domain/`
- `S/` = `reference/aniyomi/source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/`

**Raygun files compared**
- `plugins-mobile/raygun-audio/.../VideoPlayerActivity.kt` (369 lines: stock Media3 `PlayerView` controller, double-tap seek, brightness/volume swipes, PiP on leave);
- `apps/raygun/src/pages/VideoTitlePage.tsx`, `WatchTitlePage.tsx`, `watch/*`, `settings/prefs.tsx`;
- `crates/raygun-app/src/{video,video_sources,video_library}.rs`, `video_downloads/`;
- `crates/raygun-ext-host/src/video.rs`, `packages/sdk/src/types.ts`.

**Sizes:** S ≈ under a day, M ≈ 1–3 days, L ≈ a milestone.

---

## 0. How Aniyomi separates discovery from the library

- **Tabs:** Anime library · Manga library · Updates · History · Browse · More (`A/domain/ui/model/NavStyle.kt`).
  - A *Navigation style* setting moves one of Manga, Updates, History or Browse into More.
  - The start screen is a setting.
- **Library tabs show only saved entries.**
  - The empty state is "Getting started guide", a help link, never "Browse" (`A/tachiyomi/ui/library/anime/AnimeLibraryTab.kt:183`).
  - Searching the library filters it. When there's a query, a **"Search globally for …"** row is appended at the end of the grid (`A/presentation/library/components/GlobalSearchItem.kt`, `LazyLibraryGrid.kt:31`). It hands the query to global search, which lives in Browse.
- **Discovery is only in Browse:**
  - Sources (per source: popular, latest, search with filters);
  - global search across sources;
  - Extensions;
  - Migrate.
- **Raygun today (contradicts the user's latest feedback):**
  - Library → Videos' menu has Browse and Addons.
  - Manga, Novels and Videos empty states have a "Browse sources" button.
  - Home's empty card links Browse.
- **Gap, S:**
  - Remove every discovery entry from Library.
  - Empty states: plain text, or a link to Search with the right type.
  - Library search: add a "Search everywhere for '…'" row that opens `/search?type=<medium>&q=…`.

---

## 1. Starting playback: hosters, quality, fallback (highest value)

| # | Aniyomi | Raygun | Size |
|---|---|---|---|
| 1.1 | **Plays immediately.** Opening an episode loads all hosters in parallel and starts the source's `preferred` video (or `HosterLoader.selectBestVideo`) as soon as one resolves (`A/tachiyomi/ui/player/PlayerViewModel.kt:1366 loadHosters`). | A server-picker sheet must be opened and a link tapped before anything plays. | M |
| 1.2 | **Automatic fallback.** If the chosen video fails to load, `loadVideo` returns false and the next best is tried (`PlayerViewModel.kt:1459`). | A failed link ends playback with an error. | M |
| 1.3 | **Quality/Hoster sheet inside the player** (`controls/components/sheets/QualitySheet.kt`): <br>• hosters are expandable groups with states (loading, "tap to load" for `lazy` hosters, failed with an error icon); <br>• the current video is ticked; <br>• switching keeps the position. | Switching quality or server means leaving the player. There is no lazy-hoster state. | M |
| 1.4 | **Episode options dialog** (long-press an episode; `A/presentation/entries/anime/EpisodeOptionsDialogScreen.kt`): <br>• choose quality, then *Play internally / Play externally / Download internally / Download externally / Copy link*. | Pickers offer play or download only; no copy link, no external player or downloader. | S |
| 1.5 | **Settings → Player → Hosters:** "Show failed hosters", "Show empty hosters" (`PlayerPreferences.showFailedHosters/showEmptyHosters`). | Failed and empty servers are always shown, sorted last. | S |

## 2. Player controls and overlays (`A/tachiyomi/ui/player/controls/*`)

| # | Aniyomi | Raygun | Size |
|---|---|---|---|
| 2.1 | **Custom overlay**, not stock controls: <br>• top-left back + title + episode name; <br>• top-right: audio track, subtitle track, quality, ⋮ More; <br>• middle: previous episode · play/pause · next episode; <br>• bottom-left: lock, rotate, speed chip (tap cycles presets, long-press opens the speed sheet), current-chapter chip; <br>• bottom-right: skip-intro / custom button, aspect, PiP. | Stock `PlayerControlView` with prev/next hidden, a settings gear, a CC button and a 3.5 s timeout. | L |
| 2.2 | **Seek bar** with chapter markers and **thumbnail previews** while dragging: source tiles from `getVideoThumbnails` / `ThumbnailInfo` (`ThumbnailPreview.kt`, `SeekBar.kt`). <br>The duration can show as remaining time (`invertDuration`, toggled by tapping it). | Stock seek bar: no chapters, no previews, no inverted time. | M (previews L) |
| 2.3 | **Lock controls** (lock icon; all gestures off until unlocked). | — | S |
| 2.4 | **Aspect cycle:** Fit / Crop / Stretch, remembered (`VideoAspect`, `aspectState`), with an on-screen toast. | — (fixed fit). | S |
| 2.5 | **Rotation button**, plus a default orientation setting with 8 choices: free, video, portrait, reverse portrait, sensor portrait, landscape, reverse landscape, sensor landscape (`PlayerOrientation`, default sensor-landscape). | Rotates with the device; no button and no setting. | S |
| 2.6 | **Speed sheet** (`PlaybackSpeedSheet.kt`): <br>• a slider plus editable preset chips (`speedPresets`, 0.25–4.0); <br>• "Make default" (`playerSpeed`); <br>• a pitch-correction toggle. | Stock speed menu (0.25–2×); speed is not remembered. | S |
| 2.7 | **Brightness and volume slider overlays** (`VerticalSliders.kt`, `BrightnessOverlay.kt`): <br>• vertical bars while swiping; <br>• volume shown as a percentage (`displayVolPer`); <br>• **volume boost above 100%** up to a cap (`volumeBoostCap`, default 30%); <br>• brightness below 0 dims with an overlay. | A text hint ("🔊 40%") and system volume only. | M |
| 2.8 | **Remember brightness / remember volume** (`rememberPlayerBrightness/Volume`). | — | S |
| 2.9 | **Display settings:** <br>• fullscreen (draw under the cutout); <br>• hide controls entirely; <br>• show the system status bar; <br>• reduce motion; <br>• controls timeout (`playerTimeToDisappear`, default 4 s, list); <br>• panel opacity slider; <br>• show loading circle; <br>• show current-chapter indicator. | Only a fixed 3.5 s timeout. | S each, M together |
| 2.10 | **Allow gestures while panels are open** (`allowGestures`). | n/a (no panels yet) | S |
| 2.11 | **Player update toasts:** a centered text pill for speed changes, aspect, seek amount and errors (`PlayerUpdates`). | A hint text for seek, brightness and volume only. | S |

## 3. Gestures (`A/tachiyomi/ui/player/controls/GestureHandler.kt`, `settings/GesturePreferences.kt`)

| # | Aniyomi | Raygun | Size |
|---|---|---|---|
| 3.1 | **Horizontal swipe to seek**, with a ±time preview and a release-to-seek (`gestureHorizontalSeek`); optional **smooth seek** (exact, slower; `playerSmoothSeek`); optional "show seek bar while seeking". | — (no horizontal seek). | M |
| 3.2 | **Double-tap left/center/right each configurable:** Seek, Play/Pause, Switch episode, Custom, None. Defaults: seek · play-pause · seek. Repeated taps accumulate (+10, +20…) with animated triangles (`DoubleTapSeekTriangles.kt`). | Left/right seek a fixed step; no center action, no accumulation, not configurable. | M |
| 3.3 | **Media keys / headset buttons** (previous, play/pause, next) mapped the same way (`mediaPreviousGesture` etc., default previous/next switch episode). | — | S |
| 3.4 | **Toggle volume/brightness gestures; swap sides** (`gestureVolumeBrightness`, `swapVolumeBrightness`). | Always on, fixed sides. | S |
| 3.5 | **Long-press opens the Screenshot sheet** (pauses playback). | — | S (with §6) |
| 3.6 | **Seek-step setting:** 30/20/10/5/3 s or disabled (`skipLengthPreference`), plus the **default intro length** (85 s). | Seek step 5/10/15/30/60 s (M42.1). Intro length missing. | S |

## 4. Tracks, subtitles and audio

| # | Aniyomi | Raygun | Size |
|---|---|---|---|
| 4.1 | **Subtitle tracks sheet** with **two tracks at once** (primary + secondary), add a subtitle file from the device, and "Off" (`SubtitleTracksSheet.kt`, `selectSub`, `addSubtitle`). | Stock single-track menu; can't add local files. | M |
| 4.2 | **Audio tracks sheet** plus **add an external audio file**; sources can supply separate audio tracks (`Video.audioTracks`) such as dubs. | Stock audio menu; the SDK `Video` has no `audioTracks`. | M |
| 4.3 | **Subtitle styling panel** (`panels/SubtitleSettings*Card.kt`), all persisted: <br>• font (from a fonts folder), size, scale, bold, italic; <br>• text colour, border colour, border size; <br>• border style: outline + shadow / opaque box / background box; <br>• shadow offset, background colour, justification (left/center/right/auto); <br>• vertical position; <br>• override ASS styles; <br>• reset. | None: system caption style only. | M |
| 4.4 | **Subtitle delay panel**, including speed and secondary-track delay (`SubtitleDelayPanel.kt`, `subtitlesDelay/Speed/SecondaryDelay`). | — | S–M (Media3 offset via renderer) |
| 4.5 | **Audio delay panel** (`AudioDelayPanel.kt`). | — | M (Media3 needs an audio processor) |
| 4.6 | **Preferred languages are lists:** subtitle languages (comma list), audio languages, a **subtitle whitelist and blacklist** of track titles (e.g. hide "Signs") (`PlayerSettingsSubtitleScreen.kt`, `TrackSelect.kt`). | One subtitle language; no audio-language preference; no white/blacklist. | S |
| 4.7 | **Audio settings:** <br>• pitch correction; <br>• channels: auto-safe / auto / mono / stereo / reverse stereo; <br>• volume boost cap slider. | — | S (cap, pitch), M (channels) |

## 5. Chapters, intro/outro skip, AniSkip

| # | Aniyomi | Raygun | Size |
|---|---|---|---|
| 5.1 | **Chapters** merge three sources: the video's embedded chapters, source timestamps (`Video.timestamps`, type Opening/Ending/Recap/MixedOp/Other), and AniSkip (`A/tachiyomi/ui/player/utils/ChapterUtils.kt`). Shown as a **Chapters sheet** (`ChaptersSheet.kt`) and markers on the seek bar. | None; the SDK `Video` has no `timestamps`. | M |
| 5.2 | **Skip-intro button** (bottom-right): <br>• shows "Skip Opening/Ending/Recap" inside a typed chapter, else "+85 s" (the per-title intro length); <br>• **Netflix-style** countdown that auto-skips after N seconds (5–10, `waitingTimeIntroSkip`); <br>• **auto-skip** option; <br>• per-anime intro length (title menu "Skip intro length", `setAnimeSkipIntroLength`). | — | M |
| 5.3 | **AniSkip** (`utils/AniSkipApi.kt`): <br>• fetches OP/ED times from `api.aniskip.com` by MAL id (from a tracker, or an AniList → MAL lookup) and episode number; <br>• a setting to turn it on, "disable when the video has chapters", and an info text. | — (⏸ in parity: needs a tracker). Could work without login from AniList id search. | M |

## 6. Episodes inside the player, PiP and other tools

| # | Aniyomi | Raygun | Size |
|---|---|---|---|
| 6.1 | **Previous/next episode buttons**; the episode list follows the title's filters (bookmarked, fillers) (`changeEpisode`, `filterEpisodeList`). | One video per player session; the next episode means going back to the title page. | M |
| 6.2 | **Autoplay next episode** switch on the controls (`AutoPlaySwitch.kt`, `autoplayEnabled`). | — | M (with 6.1) |
| 6.3 | **Episode list dialog** in the player (`dialogs/EpisodeListDialog.kt`): jump to any episode; bookmark and filler toggles. | — | S (with 6.1) |
| 6.4 | **PiP rules** (`PipActions.kt`): <br>• enable PiP; <br>• PiP on exit (back enters PiP); <br>• PiP episode toasts; <br>• "replace previous episode" (PiP previous/next actions); <br>• PiP remote actions: play/pause, ±seek, previous/next. | PiP on home/leave only; no actions, no settings. | M |
| 6.5 | **Sleep timer** (More sheet → timer picker, remaining-time display, cancel). | — (music has one). | S |
| 6.6 | **Screenshots** (`ScreenshotSheet.kt`): with or without subtitles (`screenshotSubtitles`); save, share, or **set as cover / background / episode thumbnail**. | — | M |
| 6.7 | **Video filters panel:** brightness, saturation, contrast, gamma, hue (`VideoFiltersPanel.kt`, mpv properties). | — (ExoPlayer would need a GL effect). | L |
| 6.8 | **Decoder:** HW / HW+ / SW / auto switch in the More sheet; settings for try HW decoding, gpu-next, debanding (none/CPU/GPU), YUV420P (`DecoderPreferences`). | ExoPlayer defaults; no fallback to software on decoder errors. M43 hit a 4:4:4 file the emulator couldn't decode: offer "retry with software decoder" / extension renderers. | M |
| 6.9 | **Statistics page overlay** (More sheet chip; mpv stats pages). | — | S (Media3 `DebugTextViewHelper`) |
| 6.10 | **External player** (`ExternalIntents.kt`): <br>• always use an external player, or pick per episode; <br>• a list of known players (mpv, MX, VLC, Web Video Caster…), with headers passed as extras and the position read back for resume. | — (parity row says 🔜 M38, not built). | M |
| 6.11 | **Custom buttons** (Lua/mpv, `presentation/more/settings/screen/player/custombutton/*`) and an **mpv.conf/input.conf editor**. | ✖ (no mpv). Could offer simple built-in actions: +85 s, ×2 hold, toggle subs. | — |
| 6.12 | **Hold for 2× speed** (`DoubleSpeed` update, commented out in this version). | — | S |

## 7. Player settings screens (exact Aniyomi options) and Raygun status

Settings → Player is a category list (`A/presentation/more/settings/screen/player/PlayerSettingsMainScreen.kt`):
Player · Gestures · Decoder · Subtitles · Audio · Custom buttons · Editor · Torrents · Advanced.

- **Player** (`PlayerSettingsPlayerScreen.kt`):
  - **Progress:**
    - mark watched at 70/75/80/85/90/95/100% — Raygun has 50–100% ✅;
    - preserve watching position (don't rewind once watched) — Raygun ✖.
  - **Orientation:** default orientation ✖.
  - **Controls** ✖ (all): allow gestures in panels, show loading circle, show chapter indicator, remember brightness, remember volume.
  - **Hosters:** show failed, show empty ✖.
  - **Display** ✖ (all): fullscreen, hide controls, volume as %, show system status bar, reduce motion, controls timeout, panel opacity.
  - **Skip intro:** enable, auto-skip, Netflix style, waiting time, AniSkip, disable AniSkip on chapters ✖.
  - **PiP:** enable, episode toasts, on exit, replace with previous ✖.
  - **External player:** always use, which player ✖.
- **Gestures** (`PlayerSettingsGesturesScreen.kt`):
  - sliders on, swap sides;
  - horizontal seek, show seek bar, default intro length, skip length (Raygun has the step), smooth seek;
  - double tap left/center/right;
  - media previous/play/next.
  - Raygun has only the seek step.
- **Decoder:** try HW, gpu-next, debanding, YUV420P ✖.
- **Subtitles:** preferred languages (list), whitelist, blacklist. Raygun has auto subtitles and one language.
- **Audio:** preferred languages, pitch correction, channels, boost cap ✖.
- **Torrents:**
  - TorrServer: enable, port, trackers list with reset, proxy mode none/tracker/peers/full, proxy URL;
  - Raygun ✖ by policy.
- **Advanced:** mpv user files, mpv.conf, input.conf ✖ (n/a).

**Raygun gap: L** in total. An ExoPlayer-feasible subset is ~30 options.

## 8. Title screen and episode list (`A/presentation/entries/anime/*`)

| # | Aniyomi | Raygun | Size |
|---|---|---|---|
| 8.1 | **Episode filters** (`EpisodeSettingsDialog.kt`): unseen, downloaded, bookmarked, **fillermarked** (tri-state). | Season tabs only. | S |
| 8.2 | **Episode sort:** by source, number or upload date, ascending/descending. | Source order only. | S |
| 8.3 | **Episode display:** title or "Episode N", **episode thumbnail previews**, **episode summaries**; "set as default for all" and "also set for the library". | Stremio shows thumbnails and overviews; video sources show neither (the SDK `VideoEpisode` has no `summary`/`preview`). | S–M |
| 8.4 | **Episode fields from sources:** `fillermark`, `summary`, `preview_url`, `scanlator` (fansub group), `date_upload` (`S/model/SEpisode.kt`). | SDK episode: id, name, number, season, date only. | S |
| 8.5 | **Bookmark episodes**, and **filler marks** (toggle; badge on the row). | — | S |
| 8.6 | **Multi-select bulk actions:** mark watched/unwatched, **mark previous as watched**, bookmark, fillermark, download, delete (`AnimeScreen.kt:147-150`). | One episode at a time. | M |
| 8.7 | **Swipe actions** on episode rows: start/end swipe each configurable (toggle seen, bookmark, filler, download, none) (`LibraryPreferences.swipeEpisodeStart/EndAction`). | — | S |
| 8.8 | **Row progress text:** "Progress 12:34", the fansub group, the date. | A progress bar and date only. | S |
| 8.9 | **Download menu on the title:** next 1/5/10/25 unwatched, all unwatched (`EntryScreenConstants.DownloadAction`). Per-episode indicator actions: start, **start now** (jump the queue), cancel, delete, **choose quality** (`EpisodeDownloadAction`). | A per-episode button that opens a picker; no bulk, no "start now". | M |
| 8.10 | **Seasons as entries** (`FetchType.Seasons`, `getSeasonList`, `SeasonSettingsDialog.kt`): <br>• a title can list seasons as child entries (grid or list, with badges and continue buttons) and its own filters and sort; <br>• "update seasons on refresh/library update". | Seasons only as an episode field. | L |
| 8.11 | **Related titles rows** (`getRelatedAnimeList`, `RelatedAnimeRows.kt`). | — | M |
| 8.12 | **Header extras:** <br>• background image (`SAnime.background_url`); <br>• cover/background dialog (save, share, edit, delete custom, `AnimeImagesDialog.kt`); <br>• status includes Upcoming, Licensed, On hiatus; <br>• "expected next update" interval chip; <br>• tracking button; <br>• share; <br>• WebView; <br>• long-press the title to search it globally or copy it. | Blurred cover header; open-in-browser action; no edit, share, tracking or global-search shortcut. | M |
| 8.13 | **Title menu:** edit categories, **edit fetch interval**, **migrate** to another source, **duplicate detection** on add (`DuplicateAnimeDialog.kt`), skip-intro length. | Categories only. | M (migration L) |
| 8.14 | **Continue/Start FAB** with episode progress ("Resume E5 12:34"). | ✅ Resume FAB. | — |

## 9. Library and categories

| # | Aniyomi | Raygun | Size |
|---|---|---|---|
| 9.1 | **Display modes:** compact grid, comfortable grid, cover-only grid, list; separate portrait and landscape columns; per-category display (`SeasonSettingsDialog.kt` mirrors the library sheet). | Compact or comfortable; one column count. | S–M |
| 9.2 | **Badges:** unseen, downloaded, **local**, **language**; "continue watching" play button on covers (`showContinueViewingButton`); category tabs with item counts. | Unwatched and downloaded badges; counts on tabs ✅. No continue button on covers. | S |
| 9.3 | **Filters:** downloaded, unseen, started, bookmarked, completed, **tracked per tracker** (tri-state include/exclude). | Unread/started/downloaded/completed as on/off; no tri-state, bookmarked or tracked. | S |
| 9.4 | **Sort:** alphabetical, last watched, last update check, unseen count, total episodes, latest episode, episode fetch date, date added, tracker score, **random** (`randomAnimeSortSeed`); per-category sort. | Title, last read, latest, unread count, added. | S |
| 9.5 | **Library long-press selection:** change category, mark watched/unwatched, download unwatched, delete, migrate. | — | M |
| 9.6 | **Default category** for new entries; ask each time. | — | S |
| 9.7 | **"Downloaded only" mode** (global toggle in More) and **incognito mode** (no history). | — | S each |

## 10. Updates, history, stats, storage

| # | Aniyomi | Raygun | Size |
|---|---|---|---|
| 10.1 | **Updates:** <br>• anime/manga tabs; <br>• filters: unread, downloaded, started, bookmarked, fillermarked; <br>• selection: mark watched, download, delete; <br>• swipe; <br>• "last updated N ago"; <br>• new-update count badge on the tab (`newShowUpdatesCount`). | Chapters/Episodes tabs ✅, Upcoming ✅. No filters, selection or badge. | M |
| 10.2 | **History:** anime/manga tabs, grouped by day, resume the last episode, delete one entry or **all of a title**, clear all, search history. | One flat list with per-item remove. | S–M |
| 10.3 | **Stats** (`A/presentation/more/stats/AnimeStatsScreenContent.kt`): <br>• titles in library, completed, started, local, in global update; <br>• episodes total, watched, downloaded; <br>• **watched duration**; <br>• tracked titles, mean score. | — | M |
| 10.4 | **Storage screen** (`A/presentation/more/storage/*`): space used per title and category, with a delete action. | — (Downloads shows a total only). | M |

## 11. Downloads (`A/tachiyomi/data/download/anime/*`, `D/download/service/DownloadPreferences.kt`)

| # | Aniyomi | Raygun | Size |
|---|---|---|---|
| 11.1 | **Parallel downloads:** "Number of simultaneous downloads" (`numberOfDownloads`) and a **speed limit** (`downloadSpeedLimit`). | One at a time; HLS parts four at a time. | S–M |
| 11.2 | **Only over Wi-Fi.** | — | S |
| 11.3 | **Auto-download new episodes** after a library update, by category include/exclude, with "only unwatched" (`downloadNewEpisodes*`). | — | M |
| 11.4 | **Download ahead while watching:** once ~35% in and the current and next episodes are downloaded, queue the next N (`autoDownloadWhileWatching`, `PlayerViewModel.kt:1712`). | — | S (needs 6.1) |
| 11.5 | **Delete after watched:** the Nth-previous episode is removed once watched (`removeAfterReadSlots`); "delete when marked watched"; keep bookmarked; exclude categories. | — | S |
| 11.6 | **Fillers:** "download filler episodes" toggle (`downloadFillermarkedItems`). | — | S (needs 8.4) |
| 11.7 | **External downloader** (ADM, 1DM…) per download or always (`useExternalDownloader`, `externalDownloaderSelection`). | — | S |
| 11.8 | **Mux to MKV** with ffmpeg: video, audio and subtitle tracks in one file, plus progress (`AnimeDownloader.kt:568`). | HLS saved as segments + playlist; subtitles as side files. Equivalent for playback; not portable to other players. | — |
| 11.9 | **Queue screen:** anime/manga tabs, drag to reorder, sort by upload date or episode number, move a series to the top or bottom, cancel all for a series, cancel all. | A newest-first list; pause all; no reorder. | M |
| 11.10 | **Background downloads** with a foreground notification (`AnimeDownloadJob`, `AnimeDownloadNotifier`). | Foreground only (an M43 decision). | M |
| 11.11 | **Torrent downloads** through TorrServer. | ✖ by policy. | — |

## 12. Library update settings (`D/library/service/LibraryPreferences.kt`)

All are **S–M** in Raygun, which today has the interval only and always skips finished series:
- auto-update interval (off/6/12/24/48 h/weekly);
- **device restrictions:** Wi-Fi only, not metered, charging;
- **entry restrictions:** skip completed, skip with unwatched, skip not started, **skip outside the release period** (smart fetch interval from release history, `filterIntervalCustom`);
- **categories to include/exclude** in updates;
- **auto-refresh metadata** (`autoUpdateMetadata`);
- **mark duplicate episodes watched** (new / existing: same episode number from another group);
- **update seasons** on refresh and on library update.

## 13. Anime extension API (`S/`)

| # | Aniyomi | Raygun SDK | Size |
|---|---|---|---|
| 13.1 | `Video`: <br>• `videoTitle`, `resolution`, **`bitrate`**, `headers`, `preferred`; <br>• `subtitleTracks`, **`audioTracks`**, **`timestamps`** (typed chapters); <br>• `mpvArgs` / `ffmpegStreamArgs` / `ffmpegVideoArgs`; <br>• `internalData`, `initialized` plus **`resolveVideo(video)`** to resolve a video only when picked. | url, label, quality, headers, format, subtitles. Missing: bitrate, audio tracks, timestamps, preferred flag, lazy resolve. | S–M |
| 13.2 | `Hoster`: `hosterName`, `videoList?`, **`lazy`** (resolve only when the user taps it); `sortHosters()` / `sortVideos()` (source-side ordering by user preferences). | Server: id, name, videos?. No lazy flag. App-side ordering only by height. | S |
| 13.3 | `SEpisode`: `fillermark`, `summary`, `preview_url`, `scanlator`. | Missing all four. | S |
| 13.4 | `SAnime`: `background_url`, `update_strategy` (only fetch details once), **`fetch_type` Seasons/Episodes**, `season_number`, status incl. **Upcoming**. | Missing background, update strategy, seasons-as-entries. | S (L for seasons) |
| 13.5 | **Extra calls:** <br>• `getRelatedAnimeList`; <br>• `getAnimeEpisodeUpdate` / `getAnimeSeasonUpdate` (combined, cheaper refresh); <br>• `getVideoThumbnails` + `getImageTile` (seek previews); <br>• `createHttpServer` (a source-hosted local server); <br>• `ResolvableAnimeSource` (open a pasted URL as an entry). | None. | M |
| 13.6 | **Extension preferences** (`ConfigurableAnimeSource`, `PreferenceScreen`): the usual "Preferred quality / server / language / sub-or-dub". | Raygun has extension settings (M17), but the IA extension exposes no preferred server or language. | S per extension |

## 14. Trackers (`A/tachiyomi/data/track/*`)

- **Trackers:** AniList, MyAnimeList, Kitsu, Shikimori, Simkl, Bangumi; Jellyfin, Komga, Kavita and Suwayomi as "enhanced" trackers (auto-bound).
- **Behaviour:**
  - tracking sheet per title (status, episode, score, start/finish dates);
  - auto-update the episode count when watched;
  - "track on add";
  - tracked filters and sort in the library;
  - AniSkip uses the MAL id.
- **Raygun:** ⏸ M54 (accounts). **L.**

## 15. Other anime features

- **Local anime source** (`reference/aniyomi/source-local/.../entries/anime/LocalAnimeSource.kt`): folders of video files become library entries, with cover/background managers and episode ordering. Raygun has local music only. **M.**
- **Migration** between sources, keeping progress, categories and tracking (`A/tachiyomi/ui/browse/anime/migration`). **L.**
- **Navigation style / start screen** options. Raygun has start screen and nav labels ✅.
- **WebView** for a title or episode (cookies for Cloudflare): Raygun M44 is planned.
- **Torrent streaming** (TorrServer settings in §7): ✖ by policy.

---

## Suggested order (most valuable first)

1. Library has no discovery entry points; global-search row in library search (§0). **S**
2. Play immediately with the preferred link, automatic fallback, and a quality/server sheet inside the player (§1). **M–L**
3. Next/previous episode, autoplay next, episode list in the player; download-ahead and delete-after-watched hooks (§6.1–6.3, §11.4–11.5). **M**
4. Custom player overlay: lock, aspect, rotation, speed sheet with presets, seek and volume slider overlays, horizontal seek, configurable double-tap and media keys (§2–3). **L**
5. Chapters, skip-intro button (Netflix style, auto-skip, per-title length), AniSkip without login (§5). **M**
6. Subtitle styling, delay, two subtitle tracks, local subtitle/audio files, language lists and white/blacklist (§4). **M**
7. Episode list: filters, sort, bookmarks, fillers, summaries, thumbnails, multi-select, swipe actions, mark previous watched (§8.1–8.9). **M**
8. SDK additions: episode summary/preview/filler/scanlator, video audio tracks, timestamps, preferred flag, lazy servers, background image (§13). **S–M**
9. Downloads: parallel slots, Wi-Fi only, auto-download new, queue reorder, background notification, external downloader (§11). **M**
10. Library update restrictions and include/exclude categories; tri-state filters; more sorts and display modes (§9, §12). **M**
11. PiP settings and actions, sleep timer, screenshots (set as cover), external player (§6.4–6.10). **M**
12. History with day groups and per-title delete; Updates filters and selection; Stats and Storage screens (§10). **M**
13. Related titles, background art, cover editing, duplicate detection on add (§8.11–8.13). **M**
14. Seasons as entries and local anime folders (§8.10, §15). **L**
15. Trackers with AniSkip integration, and migration (§14–15). **L**
