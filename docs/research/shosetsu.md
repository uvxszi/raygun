# Shosetsu vs. Raygun: novels

Research of 2026-11-01. It compares `reference/shosetsu` (with `shosetsu-extensions` and `shosetsu-kotlin-lib`) with Raygun as of M43 (commit `8887875`).

- **Paths:** Shosetsu paths are relative to `reference/shosetsu/android/src/main/java/app/shosetsu/android/` unless they start with `reference/`. Strings come from `reference/shosetsu/android/src/main/res/values/strings.xml`.
- **Size:** **S** is under a day, **M** is one to three days, **L** is a milestone or most of one.
- **Raygun's novel stack today:**
  - **Reader (`apps/raygun/src/pages/NovelReaderPage.tsx`):** text size 14–30 px, line height 1.2–2.2, serif or sans, narrow or wide column, scroll or paged, percent progress, and read-aloud (rate 0.5–2×, paragraph back/replay/next/stop, hold a paragraph to start there).
  - **Settings:** `Settings.novel_reader` (`NovelReaderPrefs`) holds those values.
  - **Title page:** the shared manga page (`MangaTitlePage.tsx`), with per-chapter read and download toggles and "Download N unread".
  - **Library:** the shared M39 view (categories, unread/started/downloaded/completed toggles, include-only genre tags, five sorts).
  - **Downloads:** the shared chapter queue.
  - **Speech:** `Speech.kt` speaks with a rate only.

---

## 1. Updates and background work (most valuable)

| # | Shosetsu | Raygun | Size |
|---|---|---|---|
| 1.1 | **Library updates cover novels.** `backend/workers/onetime/NovelUpdateWorker.kt` fetches every library novel's chapters and records new ones in Updates. | **Novels are not updated at all.** `MangaService::run_update` checks only kind `manga` and `video` (M31 left novels out; M43 added video, not novels). The Updates → Chapters feed lists manga only (`titles::updates(KIND="manga")`). | M |
| 1.2 | **Update settings** (`ui/settings/sub/LibrarySettingsFragment.kt`, keys in `common/SettingKey.kt`): frequency (disabled, hourly 1–11 h, 12 h, daily, 2–6 days, weekly); only update ongoing (toggle); update on startup or boot; categories to include and exclude in updates; download new chapters on update; categories to include and exclude for auto-download; device restrictions (low battery, low storage, metered connection, only when idle); progress notification, notification style, classic "Completed update" finish; dates M/D/Y. | One `mangaUpdateHours` interval, run only while the app is open. Finished series are always skipped on schedules (not a choice). No categories, auto-download, restrictions or notifications. | M |
| 1.3 | **Background work with the app closed** (WorkManager: `NovelUpdateCycleWorker`, `DownloadWorker`, `BackupCycleWorker`), with progress notifications. | Updates, downloads and speech stop with the app (M33 and M43 list this as out of scope). | L |
| 1.4 | **Updates screen** (`ui/updates/`): grouped by day, "Library last updated: …", **clear updates** (all, or before a date picked in a dialog), refresh. | Day groups and the last-checked time exist. No clearing; novels are absent (1.1). | S |
| 1.5 | **Update one category** from the library's refresh action (`ui/library/Actions.kt` `RefreshButton` → `startUpdateManager(categoryID)`). | Update library always checks everything. | S |

## 2. Reader

### 2.1 Look

| # | Shosetsu | Raygun | Size |
|---|---|---|---|
| 2.1.1 | **Reader themes** independent of the app theme (`domain/usecases/load/LoadReaderThemes.kt`): Light, Light-dark, Sepia (wheat), AMOLED. User colour pairs are stored in `ReaderUserThemes` and picked in Reader settings (`ui/settings/sub/ReaderSettingsFragment.kt`). | The reader always uses the app's background and foreground. There's no sepia, no black, and no reader-only theme. | M |
| 2.1.2 | **Text size 7–50** (`ReaderSettingsViewModel.textSizeOption`), also in the reader's bottom sheet. | 14–30 px, in the reader's panel and Settings. | S |
| 2.1.3 | **Paragraph spacing 0–10 em** and **paragraph indent 0–10 em**, as sliders in the reader sheet (`ui/reader/content/ChapterReaderBottomSheetContent.kt`). | Neither exists. | S |
| 2.1.4 | **Text alignment:** start, center, justified or end (`ReaderTextAlignment`, `R.array.text_alignments`). | Fixed alignment. | S |
| 2.1.5 | **Custom CSS editor** (`ui/css/CSSEditor*.kt`): an editor with a live preview pager, paste, undo, redo, save, an example and export. It injects a user stylesheet after Shosetsu's own (`ui/reader/page/ShosetsuStyle.kt`), and Material colours are exposed as CSS variables (`viewmodel/abstracted/ShosetsuCssViewModelComponent.kt`). | None. M53 has it planned ("Reader themes, custom CSS editor"). | M |
| 2.1.6 | **Table hack:** wide tables scroll sideways (`ReaderTableHack`). | Raygun's passage model has no tables (2.4.1). | S |
| 2.1.7 | **Fonts:** a `ReaderFont` key exists but has no UI in the current release. | Serif or sans only. To be better than Shosetsu: system fonts plus imported TTF/OTF and dyslexia-friendly fonts. | M |
| 2.1.8 | **Margins and line spacing** (`ReaderMarginSize`, `ReaderLineSpacing`) are legacy keys with no UI now. Side padding comes from system insets with a 0.5 em minimum. | Line spacing exists. Width is narrow or wide; there are no margin sliders. | S |

### 2.2 Behaviour and navigation

| # | Shosetsu | Raygun | Size |
|---|---|---|---|
| 2.2.1 | **Continuous scroll** between chapters (`ReaderContinuousScroll`), or **horizontal chapter swipe** (`ReaderHorizontalPageSwap`) with **invert swipe** (`ReaderIsInvertedSwipe`). Chapters sit in a pager (`ui/reader/content/ChapterReaderPager.kt`). | One chapter per view, with previous/next buttons and an end card. | M |
| 2.2.2 | **Chapter divider page** between chapters, showing the current and next chapter (`ui/reader/page/DividerPage.kt`, `ReaderShowChapterDivider`). | End card only. | S |
| 2.2.3 | **Focus mode** (hide UI) on tap or double tap (`ReaderDoubleTapFocus`, `ReaderIsFirstFocus`). Fullscreen hides system bars (`ReaderEnableFullscreen`), optionally matched to focus (`ReaderMatchFullscreenToFocus`). Double tap can toggle system UI (`ReaderDoubleTapSystem`). | A tap toggles the chrome. No fullscreen setting, no double-tap choices. | S |
| 2.2.4 | **Keep screen on** (`ReaderKeepScreenOn`). | Not available for the novel reader (only the video player keeps the screen on). | S |
| 2.2.5 | **Volume keys scroll** (`ReaderVolumeScroll`) and **tap to scroll** (`ReaderIsTapToScroll`). Both are hidden in the current UI, but the keys exist. | Arrow keys only (desktop). No volume keys. | S |
| 2.2.6 | **Rotation lock** button in the reader bar (`ChapterReaderBottomSheetContent.kt`). | None. | S |
| 2.2.7 | **Chapter bookmark** toggle in the reader bar (`toggleBookmark`). | No chapter bookmarks anywhere (see 3.2). | S |
| 2.2.8 | **Text selection** is on by default, with a setting to disable it (`ReaderDisableTextSelection`). | The reader is `select-none`: nothing can be selected, copied, shared or looked up. | S |
| 2.2.9 | **Reading marks** (`ReadingMarkingType`): *on view* (read when opened) or *on scroll* (read when scrolled through). **Mark read as reading** (`ReaderMarkReadAsReading`) re-marks a read chapter as reading when it's reopened. Chapters have three states: unread, reading, read (`common/enums/ReadingStatus.kt`). | A chapter completes at the end; there's no setting. The state is effectively unread, started or read (from position). | S |
| 2.2.10 | **Resume first unread** (`ChaptersResumeFirstUnread`): Resume ignores in-progress chapters and opens the first unread. | Continue picks the partly read chapter or the next unread; there's no setting. | S |
| 2.2.11 | **Deduplicate titles** (`ReaderDeDupChapterTitle`): drops repeated `<h1>`s after the first (`ChapterReaderViewModel.kt` ~line 332). | Not done. Sources often repeat the chapter title. | S |
| 2.2.12 | **Track long reading:** a 20-20-20 eye-break reminder (`ReaderTrackLongReading`, `trackLongReading`). | None. | S |
| 2.2.13 | **Delete after reading** the current chapter, or N chapters back (`DeleteReadChapter`, `ChapterReaderViewModel.deletePrevious`). | None. | S |

### 2.3 In-reader settings sheet

| # | Shosetsu | Raygun | Size |
|---|---|---|---|
| 2.3.1 | The bottom sheet (`ui/reader/ChapterReaderView.kt` lines 150–175) holds nearly every reader option: text size, paragraph spacing and indent, invert swipe, keep screen on, fullscreen, match fullscreen, divider, double-tap focus and system, table hack, text selection, long reading, TTS pitch, speed, engine, language, voice, test, read next chapter, and the next-chapter alert. | The "Text settings" panel has size, spacing, font, width, speed, and scroll or paged. | S (once 2.1 and 2.2 exist) |

### 2.4 Content fidelity

| # | Shosetsu | Raygun | Size |
|---|---|---|---|
| 2.4.1 | HTML chapters (`chapterType = HTML`; **39 of 60** official scripts) render in a WebView with the source's markup: bold, italics, links, lists, tables, blockquotes, `<hr>`, images (`ui/reader/page/HTMLPage.kt`, `ChapterReaderWebview.kt`). | `raygun-ext-host::novel` reduces chapters to `Paragraph`, `Heading`, `Separator` and `Image` blocks (deliberately: no source HTML in the webview). Emphasis, lists, tables, quotes, links and footnotes are lost. Add safe inline spans and more block kinds (still sanitised). | M |
| 2.4.2 | Text chapters (`STRING`) render as text pages. | Equivalent. | — |
| 2.4.3 | n/a | Chapter images aren't downloaded with novel chapters (M33 out of scope); offline chapters lose pictures. | S |

## 3. Novel (title) screen

| # | Shosetsu | Raygun | Size |
|---|---|---|---|
| 3.1 | **Per-novel chapter filter and sort** (`ui/novel/NovelFilterMenuBuilder.kt`, `domain/model/local/NovelSettingEntity.kt`): sort by source or upload date, reverse order, show only unread/reading/read, only bookmarked, only downloaded, and a text filter. Saved per novel. | None: chapters show in source order, unfiltered. | M |
| 3.2 | **Chapter bookmarks**, with a bookmarked filter. | None. | S |
| 3.3 | **Selection mode** on chapters: select all, inverse, between. Then bookmark/unbookmark, mark read/unread, download, delete from storage, and (with "Show true chapter delete") remove the chapter from the database (`ui/novel/Actions.kt`, `ExposeTrueChapterDelete`). | Only single-chapter read and download toggles, plus "Download N unread". | M |
| 3.4 | **Download menu** (`ui/novel/Actions.kt`): next chapter, next 5, next 10, **custom** (a range dialog: `NovelCustomDownloadDialog.kt`), unread, all. | "Download N unread" only (M53 plans custom ranges). | S |
| 3.5 | **Jump to chapter** by number or by title search (`fragment_novel_jump_dialog_*`). | None. | S |
| 3.6 | **Migrate source** (`ui/migration/MigrationFragment.kt`). Shosetsu's own screen says it's "under construction"; Mihon's migration is the model to follow. | None. | L |
| 3.7 | **Share** a novel (a `share.shosetsu.app` link and QR code: `reference/shosetsu-kotlin-lib/.../share/NovelLink.kt`), and **open in WebView** with the app's cookies. | "Open in browser" only; no share. | S |
| 3.8 | **Chapter row details:** release date, reading position ("Position: x%"), bookmark, download state. | Date, percent read and download state; no bookmark. | — |
| 3.9 | **"You have read all the chapters"** snackbar when Resume has nothing left. | No equivalent. | S |

## 4. Library

| # | Shosetsu | Raygun | Size |
|---|---|---|---|
| 4.1 | **Selection mode** (long press) with select all, inverse and between, then set categories, pin/unpin, remove, migrate (`ui/library/LibraryFragment.kt` ~lines 470–525, `Actions.kt`). | No multi-select or bulk actions in any library. | M |
| 4.2 | **Pin to top** per novel, with a "pins on top" option (`LibraryFilterState.arePinsOnTop`). | None. | S |
| 4.3 | **Filters** (`domain/model/local/LibraryFilterState.kt`, `ui/library/LibraryFilterMenuBuilder.kt`): unread and downloaded are **tri-state** (include, exclude, off); genre, **author**, **artist** and tag filters with **include or exclude** per value. | Unread, started, downloaded and completed are include-only toggles; genre tags are include-only. No author or artist filters. | S |
| 4.4 | **Sort:** title, unread count, added (ID), last updated, read time; reversible (`common/enums/NovelSortType.kt`). | Title, last read, latest chapter, unread, added: equivalent. | — |
| 4.5 | **Display:** card types Normal, Compressed or Cozy (`novel_card_type_selector_title`); **No images** for compressed cards; separate columns for portrait and landscape (`ChapterColumnsInPortait`/`Landscape`, 0 = auto); unread-badge toast explaining the badge. | Compact or comfortable, one column count, badges on/off. No list or "no images" mode, no landscape count. | S |
| 4.6 | Categories as a swipeable pager with tabs. | Category tabs, not swipeable. | S |

## 5. Downloads

| # | Shosetsu | Raygun | Size |
|---|---|---|---|
| 5.1 | **Concurrency:** download threads and threads per extension (`DownloadThreadPool`, `DownloadExtThreads`). | One chapter at a time. Pages download two at a time for manga; novels have a single passage. | S |
| 5.2 | **Restrictions:** low battery, low storage, metered connection, only when idle (`DownloadsSettingsFragment.kt`). | None. | S (M with background work) |
| 5.3 | **Auto-download new chapters** on update, filtered by category (1.2), and **download chapter updates** (`DownloadNewNovelChapters`). | None. | S once 1.1 exists |
| 5.4 | **Queue actions** (`ui/downloads/`): per-item states (pending, waiting, downloading, paused, error), pause/start, restart, delete, **set all pending**, **delete all**, select. | Pause/resume all, retry, cancel. No "restart all" or "delete all". | S |
| 5.5 | **Notifications** for chapter downloads (`DownloadNotifyChapters`) and extension installs (`NotifyExtensionDownload`). | None. | S |
| 5.6 | **Bookmark on download:** adds the novel to the library when one of its chapters is downloaded (`BookmarkOnDownload`). | None. | S |

## 6. Read-aloud (TTS)

| # | Shosetsu | Raygun | Size |
|---|---|---|---|
| 6.1 | **Engine, language and voice** pickers, plus **Test TTS** (`ReaderSettingsViewModel.readerEngineOption` / `readerLanguageOption` / `readerVoiceOption` / `readerTestOption`; voices labelled by quality: Lowest to Highest). | The system default engine and voice only (M33 lists voice choice as out of scope). | M |
| 6.2 | **Pitch 0.1–3.0** and **speed 0.1–3.0** (`ReaderPitch`, `ReaderSpeed`). | Speed 0.5–2.0; no pitch. | S |
| 6.3 | **Read the next chapter** automatically (`ReaderNextChapter`), with a spoken **alert before moving on** (`ReaderNextChapterAlert`). | Continues into the next chapter, without an alert or a setting. | S |
| 6.4 | **Pause and resume** separately from stop. The current element is highlighted (`.tts-border-style`), and tapping an element while paused moves reading there. Long texts are split at sentence breaks to fit the engine's maximum length (`ui/reader/ChapterReaderTTS.kt`). | Stop and replay only; no pause. The highlight and "hold a paragraph" exist. Check paragraphs longer than `getMaxSpeechInputLength`. | S |
| 6.5 | n/a. Shosetsu's TTS also stops in the background. | **Do better:** a media-session foreground service, so reading continues with the screen off, with notification controls (M33 out of scope). | M |

## 7. History and statistics

| # | Shosetsu | Raygun | Size |
|---|---|---|---|
| 7.1 | **Reading history** screen (`ui/history/HistoryFragment.kt`): chapters read, newest first, tap to reopen, clear all or before a date. | Home's "Continue reading" row only; there's no manga or novel history screen (video got one in M43). | S |
| 7.2 | **Statistics** (`ui/analytics/AnalyticsFragment.kt`): total reading time, library counts, chapters (unread/reading/read), top genres, top extensions, per-novel counts. | No reading-time tracking or statistics. | M |

## 8. Extensions and repositories (Shosetsu API coverage)

| # | Shosetsu | Raygun | Size |
|---|---|---|---|
| 8.1 | **Search filters** (`searchFiltersModel`, `reference/shosetsu-kotlin-lib/src/main/kotlin/app/shosetsu/lib/Filter.kt`) shown in the catalogue's filter sheet (`ui/catalogue/CatalogueFilterMenu.kt`). **30 of 60** official scripts declare them. | The Lua runtime only applies their defaults (`raygun-ext-host/src/shosetsu/runtime.rs` `data()`). They aren't mapped to the M40 `SourceFilter` UI (M40: "later, with the novel parity work"). | M |
| 8.2 | **Extension settings** (`settingsModel` + `updateSetting`) on the Configure screen (`ui/extensionsConfigure/`), together with the **default listing** choice and uninstall. **5 of 60** scripts have settings. | `ShosetsuExtension::user_settings` is always empty (M32 out of scope). | S |
| 8.3 | **All listings** of a script (`listings`, any number, with `isIncrementing`). | Only the first two are used (M32). | S |
| 8.4 | **Cloudflare and cookies:** `hasCloudFlare` sources (**9 of 60**) open a WebView to pass the check; cookies are shared; **Clear cookies** sits in the catalogue and Advanced settings. | These sources fail; there's no cookie jar UI. M44 (WebView resolver) is the place. | L (M44) |
| 8.5 | **Repositories:** enable/disable toggle, disable on fail, undo removal, share, force sync, device restrictions for repository updates (`ui/repository/`, `BrowseSettingsFragment.kt`, `AdvancedSettingsFragment.kt`). | Add/remove; checksums are always verified (Shosetsu makes that optional: `VerifyCheckSum`). | S |
| 8.6 | **Obsolete extension** warning and "Update: vX" labels (`ui/browse/BrowseFragment.kt`). | Update labels exist; no "obsolete" state. | S |
| 8.7 | **Catalogue extras:** long press adds a novel to the library in the background (`fragment_catalogue_toast_background_add*`); WebView; report extension; "No more content". | No long-press add from source listings. | S |
| 8.8 | `styles` and `js` entries in the repository index (`reference/shosetsu-extensions/index.json`). Both are empty in the official repo. | Ignored. Low value. | — |

## 9. Backup

| # | Shosetsu | Raygun | Size |
|---|---|---|---|
| 9.1 | **Automatic backups** (2.1–6 days, weekly), with a chosen **storage folder**, **include chapters** and **include settings** toggles, restrictions, and an in-progress indicator (`ui/settings/sub/BackupFragment.kt`). | Manual create, restore and delete only (`settings/BackupCard.tsx`). | M |

## 10. Settings beyond the reader

| # | Shosetsu | Raygun | Size |
|---|---|---|---|
| 10.1 | **Network** (`AdvancedSettingsFragment.kt`): custom **User-Agent**, optional Shosetsu UA, **HTTP proxy** (`UseProxy`, `ProxyHost`, `view/compose/setting/ProxySettingsContent.kt`), **site protection delay** (ms between requests to one site, default 300), **clear cookies**. | A fixed desktop-Chrome UA (`raygun-net` `DEFAULT_USER_AGENT`); no proxy or per-site delay settings. | S–M |
| 10.2 | **Maintenance:** purge the cache of novels not in the library, kill/restart background workers, log to file, error reporting, verify checksums. | No purge of browsed (non-library) titles and chapters, which accumulate in the title store since M24. | S |
| 10.3 | **Behaviour:** require double back to exit, legacy navigation. | — | S |

## 11. Sharing and system integration

| # | Shosetsu | Raygun | Size |
|---|---|---|---|
| 11.1 | **Share links and QR codes** for novels, extensions, repositories and styles (`share.shosetsu.app`, `ui/add/AddShareFragment.kt`, "Add share link" in More; `AutoBookmarkFromQR`). | None. Raygun already renders QR codes for remote pairing (`qrcode` crate). | M |
| 11.2 | **System search intent** (`android.intent.action.SEARCH`, assistant search) opens global search (`ui/deeplink/DeepLinkActivity.kt`, `ui/main/IntentHandler.kt`). Launcher actions open Library, Updates or Catalogue. | None. | S |

## 12. Discovery vs. library

- **How Shosetsu organises it:**
  - The navigation is Library · Updates · Browse · More.
  - Browse lists extensions; one opens its Catalogue (listings, search, filters), and Global Search sits in Browse's top bar.
  - The Library holds only saved ("bookmarked") novels. Its only link out is the empty-state button "Go to Browse".
- **Raygun's rule (2026-11-01):** the Library carries no discovery at all, not even empty-state links to browsing; finding content belongs to Search.
  - Library empty states should point to Search (with `type=novel`).
  - Novel discovery (global search, plus per-source search and filters once 8.1 lands) should be reachable from the Search tab.

## 13. Where Raygun is already ahead
- **EPUB import and reading** (M33). Shosetsu only has a plan (`reference/shosetsu/EpubPlan.md`).
- **Several media in one app**, **AI translation back ends** (M34), **NSFW filtering** and **categories shared** across manga and novels.

---

## Settings map: every Shosetsu key → Raygun

| Shosetsu key (`common/SettingKey.kt`) | Raygun |
|---|---|
| `ReaderTheme`, `ReaderUserThemes` | missing (2.1.1) |
| `ReaderTextSize` (7–50) | `novelReader.fontSize` (14–30) |
| `ReaderParagraphSpacing`, `ReaderIndentSize` | missing (2.1.3) |
| `ReaderTextAlignment` | missing (2.1.4) |
| `ReaderMarginSize`, `ReaderLineSpacing`, `ReaderFont` (legacy) | width, `lineSpacing`, `font` (serif/sans) |
| `ReaderType`, `ReaderHorizontalPageSwap`, `ReaderContinuousScroll`, `ReaderIsInvertedSwipe` | `paged` only (2.2.1) |
| `ReaderIsTapToScroll`, `ReaderVolumeScroll` | missing (2.2.5) |
| `ReadingMarkingType`, `ReaderMarkReadAsReading`, `ChaptersResumeFirstUnread` | missing (2.2.9, 2.2.10) |
| `ReaderIsFirstFocus`, `ReaderDoubleTapFocus`, `ReaderDoubleTapSystem`, `ReaderEnableFullscreen`, `ReaderMatchFullscreenToFocus` | missing (2.2.3) |
| `ReaderHtmlCss` | missing (2.1.5) |
| `ReaderKeepScreenOn` | missing (2.2.4) |
| `ReaderTableHack`, `ReaderShowChapterDivider`, `ReaderDisableTextSelection`, `ReaderDeDupChapterTitle`, `ReaderTrackLongReading` | missing (2.1.6, 2.2.2, 2.2.8, 2.2.11, 2.2.12) |
| `ReaderPitch`, `ReaderSpeed`, `ReaderVoice`, `ReaderEngine`, `ReaderLanguage`, `ReaderNextChapter`, `ReaderNextChapterAlert` | `ttsRate` only (6.x) |
| `IsDownloadPaused` | in-memory pause (not persisted across restarts) |
| `DeleteReadChapter`, `BookmarkOnDownload`, `DownloadThreadPool`, `DownloadExtThreads`, `DownloadNotifyChapters`, `DownloadOn*`, `DownloadOnlyWhenIdle` | missing (5.x, 2.2.13) |
| `DownloadNewNovelChapters`, `OnlyUpdateOngoingNovels`, `UpdateNovelsOnStartup`, `Include/ExcludedCategoriesInUpdate`, `Include/ExcludedCategoriesToDownload`, `NovelUpdateCycle`, `NovelUpdateOn*`, `NovelUpdateOnlyWhenIdle`, `UpdateNotificationStyle`, `NovelUpdateShowProgress`, `NovelUpdateClassicFinish`, `NovelUpdateDateMDY` | `mangaUpdateHours` only, and novels aren't included (1.x) |
| `RepoUpdateOn*`, `RepoUpdateDisableOnFail`, `VerifyCheckSum` | missing (8.5) |
| `ChapterColumnsInPortait/Landscape`, `SelectedNovelCardType`, `NoImages`, `NovelBadgeToast` | `gridColumns`, `coverStyle`, badges (4.5) |
| `BackupStorageLocation`, `ShouldBackupChapters`, `ShouldBackupSettings`, `BackupCycle`, `BackupOn*`, `BackupOnlyWhenIdle`, `BackupIndicator` | manual backups only (9.1) |
| `ExposeTrueChapterDelete`, `LogToFile`, `UserAgent`, `UseProxy`, `ProxyHost`, `UseShosetsuAgent`, `ACRAEnabled`, `SiteProtectionDelay`, `RequireDoubleBackToExit`, `AutoBookmarkFromQR`, `LibraryFilter` | mostly missing (10.x, 11.1); `LibraryFilter` ≈ the M39 filter in local storage |
| `BrowseFilteredLanguages`, `BrowseOnlyInstalled` | language filter exists in the extension lists |
| `AppUpdate*` | n/a (distribution decision) |

---

## Summary: the 15 most valuable gaps
1. **Novels in library updates** (1.1). They're never checked today, and the Chapters feed is manga-only. M.
2. **Update and download policy** (1.2, 5.3): only ongoing, on startup, categories to include/exclude, auto-download new chapters. M.
3. **Reader themes** (light, sepia, dark, black, custom colours) independent of the app (2.1.1). M.
4. **Paragraph spacing, indent and alignment, plus a text size range of 7–50** (2.1.2–2.1.4). S.
5. **Richer chapter rendering:** emphasis, lists, tables, quotes and links in sanitised blocks (2.4.1). M.
6. **Chapter filters and sort per novel** (read state, bookmarked, downloaded, text, by date or source, reverse) (3.1). M.
7. **Chapter selection with bulk actions**, and **bookmarks** (3.2, 3.3). M.
8. **Download next 1/5/10, custom range, all** (3.4); **delete after reading** (2.2.13). S.
9. **Continuous scroll or swipe between chapters, with divider pages** (2.2.1, 2.2.2). M.
10. **Reader behaviour:** keep screen on, volume keys, fullscreen and double-tap focus, rotation lock, text selection (2.2.3–2.2.8). S each.
11. **TTS:** voice, engine and language pickers, pitch, pause, next-chapter alert, background playback with notification controls (6.1–6.5). M.
12. **Shosetsu search filters and extension settings in the UI** (8.1, 8.2). 30 of 60 scripts use filters. M.
13. **Library multi-select** (categories, pin, remove, migrate); **tri-state and exclude filters, author filters** (4.1–4.3). M.
14. **Reading history screen and statistics** with reading time (7.1, 7.2). S–M.
15. **Custom CSS editor with preview** (2.1.5), then **automatic backups** (9.1) and **network settings** (User-Agent, proxy, per-site delay, clear cookies) (10.1). M.
