# Mihon gap report (manga), 2026-11-01

What Mihon (plus TachiyomiAT and mihon_img_upscale) does that Raygun lacks or has only in a simpler form. Compared against Raygun at commit `8887875` (after M43): `docs/feature-parity.md`, the phase specs, `apps/raygun/src` and the Rust crates.

**Path shorthands** (all under `reference/mihon/`):
- `P/` = `app/src/main/java/eu/kanade/presentation/`
- `UI/` = `app/src/main/java/eu/kanade/tachiyomi/ui/`
- `F/` = `app/src/main/java/mihon/feature/`
- `D/` = `domain/src/main/java/tachiyomi/domain/`
- `S` = `i18n/src/commonMain/moko-resources/base/strings.xml`
- Preference keys come from `D/library/service/LibraryPreferences.kt`, `UI/reader/setting/ReaderPreferences.kt`, `DownloadPreferences.kt`, `SourcePreferences.kt`, `UiPreferences.kt`, `BasePreferences.kt` and `core/common/.../security/SecurityPreferences.kt`.

**Size:** S = a day or less; M = a few days; L = a week or more.

## How Mihon separates discovery from the Library

This bears directly on Raygun's rule of no browsing in the Library.

- **Library tab** (`UI/library/LibraryTab.kt`, `P/library/components/*`): only saved entries, by category tab. Its toolbar has:
  - filter, search, update library or category, open random entry;
  - select all / select inverse in selection mode.
- **The Library offers no way to browse sources.**
  - The empty library shows "Your library is empty" with a single "Getting started guide" action (`LibraryTab.kt:168`), not a link to sources.
  - The library search filters what you own. When nothing matches, a "Search for "x" globally" row (`P/library/components/GlobalSearchItem.kt`) hands the query to global search.
  - So the Library's only path to new content is through search.
- **Browse tab** holds discovery: Sources, Extensions and Migrate. Global search (`P/browse/GlobalSearchScreen.kt`) is reached from Browse and from that library-search row.
- **More** holds the rest: Downloaded only and Incognito switches, Download queue, Categories, Statistics, Data and storage, Settings, About, Help.
- **Raygun today:** library menus and empty states link to Browse ("Browse sources", and "Browse"/"Addons" in the Videos menu). The video ones were removed (M43 follow-up); check the manga and novel empty states and Home too.

---

## 1. Library

| Gap | Mihon | Raygun today | Size |
|---|---|---|---|
| **Selection mode and bulk actions:** long-press a cover to select; select all / inverse; bottom bar: set categories, mark read/unread, download (next 1/5/10/25, unread), delete from library (optionally deleting downloads) | `P/library/components/LibraryToolbar.kt`, `P/manga/components/MangaBottomActionMenu.kt` (`LibraryBottomActionMenu`), `P/library/DeleteLibraryMangaDialog.kt` | No selection; covers only open the title | M |
| **Display modes:** compact grid, comfortable grid, cover-only grid, list | `D/library/model/LibraryDisplayMode.kt`, `P/library/components/Library{Compact,Comfortable}Grid.kt`, `LibraryList.kt` | Compact and comfortable grids only, no list or cover-only | S |
| **Items per row, set separately for portrait and landscape** | `pref_library_columns_portrait_key`, `_landscape_key` | One `gridColumns` value | S |
| **Per-category display and sort** ("categorized display"): each category keeps its own display mode and sort | `categorized_display`, `D/category/interactor/SetDisplayMode.kt`, `SetSortModeForCategory` | Global only | M |
| **Sort options:** alphabetical, total chapters, last read, last update check, unread count, latest chapter, chapter fetch date, date added, tracker score, random (re-seedable), each ascending or descending | `P/library/LibrarySettingsDialog.kt`, `library_sorting_mode`, `library_random_sort_seed` | Title, last read, latest chapter, unread, added | S |
| **Tri-state filters (include / exclude / off):** downloaded, unread, started, bookmarked, completed, custom update interval, each tracker | `LibrarySettingsDialog.kt`, `pref_filter_library_*_v2` | Include-only booleans (unread, started, downloaded, completed) plus tags; no exclude, bookmarked, interval or tracker filters | S |
| **Badges:** downloaded count, unread count, local source, language; Continue reading button on covers | `P/library/components/LibraryBadges.kt`, `display_*_badge`, `display_continue_reading_button` | Unread and downloaded badges; no language, local or continue-reading button | S |
| **Category tabs options:** show or hide tabs; show the number of items per tab; remember the last used category | `display_category_tabs`, `display_number_of_items`, `last_used_category` | Tabs appear when categories exist; counts always shown; no memory | S |
| **Library search query language:** AND / OR / NOT, field filters (title, author, artist, description, genre, source, notes, language, source id), comparisons | `F/library/QueryNodeExtensions.kt` | Plain substring search | M |
| **Open random entry** | `LibraryToolbar.kt` (`action_open_random_manga`) | No | S |
| **Update one category** from the toolbar | `action_update_category` | Whole-library update only | S |
| **Downloaded only mode** (app-wide switch in More; the library shows only downloaded entries) | `pref_downloaded_only`, `P/more/MoreScreen.kt` | No | S |
| **Pull to refresh** updates the library | `LibraryTab.kt` | No | S |
| **Unread count on the Updates icon** | `pref_library_update_show_tab_badge`, `library_unseen_updates_count` | No nav badges | S |

## 2. Title (manga) screen

| Gap | Mihon | Raygun today | Size |
|---|---|---|---|
| **Chapter selection and bulk actions:** long-press, range select, select all/inverse; bottom bar with bookmark/unbookmark, mark read/unread, mark previous as read, download, delete | `P/manga/MangaScreen.kt` (`onMultiBookmarkClicked`, …), `P/manga/components/MangaBottomActionMenu.kt` | Per-row mark read and download only | M |
| **Bookmarks** on chapters (bookmark filter, a bookmarked download action, keeping bookmarked downloads) | `action_bookmark`, `pref_remove_bookmarked` | None (no bookmark column) | M (DB + UI) |
| **Chapter settings sheet:** filter (downloaded, unread, bookmarked tri-state; excluded scanlators), sort (by source, chapter number, upload date, alphabetical; asc/desc), display (source title or "Chapter N"), "Set as default" for all titles | `P/manga/ChapterSettingsDialog.kt`, `P/manga/components/ScanlatorFilterDialog.kt`, `default_chapter_*` | None; a fixed list | M |
| **Chapter swipe actions** (left and right, each one of: toggle read, toggle bookmark, download, disabled) | `D/.../LibraryPreferences.kt:229`, `pref_chapter_swipe_*` | No swipes | S |
| **Download menu:** next 1/5/10/25, unread, bookmarked | `P/manga/MangaScreenConstants.kt` (`DownloadAction`) | "Download N unread" only | S |
| **Missing chapter indicators** (gaps such as "3 missing chapters" between numbers; hideable) | `P/manga/components/MissingChapterCountListItem.kt`, `pref_hide_missing_chapter_indicators` | No | S |
| **Migration** to another source: choose sources, search, or enter a title manually; copy or move chapters' read state, categories, tracking, custom cover, notes; delete old downloads | `F/migration/*`, `P/browse/MigrateSourceScreen.kt`, `MigrateSearchScreen.kt`, `F/common/utils/MigrationFlag.kt` | None | L |
| **Duplicate detection when adding:** possible duplicates (same title in other sources) with "Add anyway" | `P/manga/DuplicateMangaDialog.kt` | None | S |
| **Cover dialog:** full-screen cover, share, save, edit (pick a custom image), delete the custom cover | `P/manga/components/MangaCoverDialog.kt`, `MangaScreenConstants.kt` (`EditCoverAction`) | No; the cover isn't tappable | M |
| **Notes:** Markdown notes per title, shown under the description, searchable in the library | `P/manga/MangaNotesScreen.kt`, `MangaNotesSection.kt` | None | S |
| **Share** (title link) | `MangaToolbar.kt` (`action_share`) | No (open in browser only) | S |
| **Tap an author or genre to search; long-press the title to copy** | `P/manga/components/MangaInfoHeader.kt` (`onSearch(query, global)`) | Genres are plain chips | S |
| **Description:** Markdown with images (`pref_display_images_description`), expand/collapse | `P/manga/components/MarkdownRender.kt` | Plain text | S |
| **Status labels:** licensed, publishing finished, cancelled, on hiatus | `MangaInfoHeader.kt` | Ongoing, completed, unknown and the source's own | S |
| **Update interval** ("smart update"): the predicted next release ("in around 3 days, checking every 2"), and a per-title custom interval | `P/manga/components/MangaDialogs.kt`, `manga_interval_*` strings | None | M |
| **Trackers button** on the header (status, score, chapters, dates, private) | `P/track/*`, `UI/manga/track` | ⏸ M54 | L |
| **Edit categories, refresh and migrate in the toolbar; WebView refresh** | `P/manga/components/MangaToolbar.kt` | Categories as a chip under the header; refresh in the top bar | S |

## 3. Reader

Raygun has webtoon, paged LTR and paged RTL; tap zones in thirds; swipes; arrow keys; double-tap 2× zoom; a page slider; per-title mode; prefetch.

**Correction for the parity table:** `feature-parity.md` marks "paged LTR/RTL, vertical, webtoon, spreads" ✅ M27. M27 actually left out spreads and vertical paged mode (`docs/phases/phase-3/M27-reader.md:41`), and continuous vertical is missing too. That row should be split.

| Gap | Mihon | Raygun today | Size |
|---|---|---|---|
| **Reading modes:** add paged vertical and continuous vertical (webtoon with gaps) | `UI/reader/setting/ReadingMode.kt` | 3 of 5 | M |
| **Scale types** (fit screen, stretch, fit width, fit height, original size, smart fit) and **zoom start** (automatic, left, right, center) | `pref_image_scale_type_key`, `pref_zoom_start_key`, strings `scale_type_*`, `zoom_start_*` | Fit only | M |
| **Crop borders** (paged and webtoon separately) | `crop_borders`, `crop_borders_webtoon`, `PageImageProcessing.kt` | No | M |
| **Wide pages:** split (and invert the split), rotate to fit (and flip); for paged and webtoon separately | `pref_dual_page_split/_invert/_rotate/_rotate_invert(_webtoon)` | No | M |
| **Dual page view** (spreads) | `pref_dual_page_view`, `UI/reader/viewer/DualPageSettings.kt` | No | L |
| **Landscape zoom** (zoom into wide images) and **pan wide images** | `landscape_zoom`, `navigate_pan` | No | S |
| **Tap zone layouts:** default, L-shaped, Kindle-ish, Edge, Right-and-Left, Disabled; **invert** (none, horizontal, vertical, both); a per-mode choice; a tap-zone overlay shown when the reader opens | `UI/reader/viewer/navigation/*`, `reader_navigation_mode_pager/_webtoon`, `reader_tapping_inverted(_webtoon)`, `reader_navigation_overlay_on_start` | Fixed thirds, mirrored in RTL | M |
| **Volume keys** to turn pages (and invert) | `reader_volume_keys(_inverted)` | No (needs a hook in the Android shell) | M |
| **Long-tap page actions:** set as cover, share, save to device, copy to clipboard | `P/reader/ReaderPageActionsDialog.kt`, `reader_long_tap`, `pref_create_folder_per_manga` | No | M |
| **Colour filters:** custom brightness (including below system), RGBA colour overlay with blend modes (overlay, multiply, screen, lighten, darken), grayscale, inverted | `P/reader/settings/ColorFilterPage.kt`, `pref_custom_brightness_key`, `color_filter_*`, `pref_grayscale`, `pref_inverted_colors` | No | M |
| **Background colour** (black, gray, white, automatic) | `pref_reader_theme_key` | Theme background | S |
| **Display:** fullscreen, show content in the cutout, keep screen on, show page number, default rotation (free, portrait, landscape, locked portrait/landscape, reverse portrait), rotation per series | `fullscreen`, `cutout_short`, `pref_keep_screen_on_key`, `pref_show_page_number_key`, `UI/reader/setting/ReaderOrientation.kt` | Page shown in the chrome only; no rotation, keep-on or cutout options | M (native hooks) |
| **E-ink flash on page change** (duration, every N pages, black / white / both) | `pref_reader_flash*` | No | S |
| **Animated page transitions**; double-tap animation speed; double-tap zoom on or off; webtoon "disable zoom out"; webtoon side padding; menu-hide sensitivity on scroll | `pref_enable_transitions_key`, `pref_double_tap_anim_speed`, `pref_enable_double_tap_zoom_webtoon`, `webtoon_disable_zoom_out`, `webtoon_side_padding`, `reader_hide_threshold` | No | S–M |
| **Chapter transition page** (finished / next / missing-chapter warning / "there's no next chapter"); always-show option; download state of the next chapter | `P/reader/ChapterTransition.kt`, `always_show_chapter_transition`, `TransitionDownloadState.kt` | Previous/next buttons in the chrome; no transition page | M |
| **Skip** chapters marked read / filtered / duplicate when moving to the next one | `skip_read`, `skip_filtered`, `skip_dupe` | No | S |
| **Vertical chapter navigator** (choose the modes, left or right side, height) | `pref_vertical_navigator*` | Horizontal slider | S |
| **Load ahead only on Wi-Fi** | `pref_reader_load_ahead_only_over_wifi` | No | S |
| **Show reading mode briefly when opening** | `pref_show_reading_mode` | No | S |
| **Bottom-bar quick buttons** (reading mode, rotation, crop borders, settings); in-reader settings sheet with Reading mode / General / Colour filter tabs | `P/reader/appbars/*`, `P/reader/settings/ReaderSettingsDialog.kt` | Reading-mode sheet only | M |
| **Bookmark the current chapter, open in WebView/browser** from the reader's top bar | `P/reader/appbars/*` | No | S |
| **Reset per-series reader settings** (Advanced) | `pref_reset_viewer_flags` | No | S |
| **mihon_img_upscale additions:** book page-turn animations; GPU transitions (flip, stack, sphere, cube, fade) and filter shaders | `UI/reader/viewer/webgpu/*`, `transition_animation_*` strings | No (upscaling is planned for M45) | L |

## 4. Updates and upcoming

| Gap | Mihon | Raygun today | Size |
|---|---|---|---|
| **Selection and bulk actions:** mark read/unread, bookmark, download, delete downloads; select all / inverse | `P/updates/UpdatesScreen.kt` | A download button per row | M |
| **Filters:** downloaded, unread, started, bookmarked (tri-state), excluded scanlators, categories to include/exclude | `P/updates/UpdatesFilterDialog.kt`, `pref_filter_update_categories_details` | None | S |
| **Upcoming calendar for manga** (predicted releases by month, next/previous month, category filter) | `F/upcoming/*` | Episodes only (M43 strip); none for manga | M |
| **New-chapter notifications** grouped per title, with Mark as read / View chapters / Download actions; "Updating library… (n/m)" progress; large-update warning | `app/src/main/java/eu/kanade/tachiyomi/data/library/LibraryUpdateNotifier.kt` | No OS notifications for updates | M |
| **Home-screen widget** of recent updates (grid, cover-screen variant) | `presentation-widget/` | No | M |

## 5. History (manga and novels)

| Gap | Mihon | Raygun today | Size |
|---|---|---|---|
| **History screen:** every chapter read, newest first, with search; resume; remove one entry or everything for a title; clear all | `P/history/HistoryScreen.kt`, `UI/history/*` | Home's Continue reading and music history only; nothing for manga or novels | M |
| **Incognito mode** (pauses history; in More, with a notification to turn it off); incognito per extension | `incognito_mode`, `incognito_extensions`, `P/browse/ExtensionDetailsScreen.kt` | None (M51) | S |
| **Clear database** of history for entries not in the library | `pref_clear_database` | None | S |

## 6. Downloads

| Gap | Mihon | Raygun today | Size |
|---|---|---|---|
| **Queue management:** drag to reorder; sort by upload date or chapter number (asc/desc); move to top or bottom (item or whole series); cancel all; grouped by source | `UI/download/DownloadQueueScreen.kt` | Pause all, retry and cancel per row | M |
| **Delete after reading** (slots: last read, second-to-last, …), **after being marked read**, allow deleting bookmarked, excluded categories | `remove_after_read_slots`, `pref_remove_after_marked_as_read_key`, `pref_remove_bookmarked`, `remove_exclude_categories` | None | S |
| **Auto-download new chapters** (categories to include/exclude, skip duplicates already read) | `download_new*` | None | S |
| **Download ahead while reading** (only when this and the next chapter are downloaded) | `auto_download_while_reading`, `download_ahead_info` | None | S |
| **Wi-Fi only**, **save as CBZ**, **concurrent sources and pages per source** | `pref_download_only_over_wifi_key`, `save_chapter_as_cbz`, `download_parallel_source_limit`, `download_parallel_page_limit` | Fixed 2 pages at a time; folders of images | S–M |
| **Low-storage check; bulk-download warning** | `download_insufficient_space`, `download_queue_size_warning` | None | S |
| **Download notifications** (progress, paused, errors, no Wi-Fi) and **background downloading** (a foreground service) | `download_notifier_*` | Runs only while the app is open; no notification | L |
| **Reindex downloads** (rescan the disk) | `pref_invalidate_download_cache` | None | S |
| **Non-ASCII-safe filenames, chapter-name hash** | `disallow_non_ascii_filenames`, `pref_enable_chapter_name_hash` | Not applicable yet (Raygun names files by id) | — |

## 7. Library update rules

| Gap | Mihon | Raygun today | Size |
|---|---|---|---|
| **Intervals:** 6 h, 12 h, daily, 2 days, 3 days, weekly | `update_*hour`, `update_weekly` | Has interval choices (check they cover these) | S |
| **Device restrictions** (Wi-Fi only, unmetered, charging) | `library_update_restriction` | None | S |
| **Entry restrictions:** skip entries with unread chapters, with "Completed" status, not yet started, or outside their predicted release window (smart update) | `library_update_manga_restriction`, `pref_update_only_*` | Scheduled runs skip completed/cancelled only | M (smart update needs fetch intervals) |
| **Categories to include/exclude** | `library_update_categories(_exclude)` | All library titles | S |
| **Refresh metadata** (cover, details) during updates; update titles to match the source | `auto_update_metadata`, `pref_update_library_manga_titles` | Details are refetched with chapters (check whether covers change) | S |
| **Mark duplicate read chapters read** (after reading, or after fetching new chapters) | `mark_duplicate_read_chapter_read` | None | S |
| **Background work with the app closed** (WorkManager) | `LibraryUpdateWorker.kt` | Scheduler runs while the app is open | L |

## 8. Browse, sources, extensions

| Gap | Mihon | Raygun today | Size |
|---|---|---|---|
| **Pin sources, "last used" section, disable sources, languages filter** (enable or disable languages and single sources) | `P/browse/SourcesScreen.kt`, `SourcesFilterScreen.kt`, `pinned_catalogues`, `hidden_catalogues`, `source_languages`, `last_catalogue_source` | Sources listed by kind with Latest; no pinning, hiding or language filter | M |
| **Browse display mode** (compact, comfortable, list) per browse screen | `P/browse/components/BrowseSourceToolbar.kt`, `pref_display_mode_catalogue` | Grid only | S |
| **Long-press a browse result to add or remove it from the library**, with a category prompt and duplicate check; an in-library badge; "hide entries already in library" | `P/browse/components/BrowseSourceDialogs.kt`, `browse_hide_in_library_items` | Must open the title; in-library badge exists | S |
| **Source WebView** from the browse toolbar; source settings; help | `BrowseSourceToolbar.kt` | M44 (WebView) | M |
| **Global search:** cards per source; filter to pinned only / all / has results | `P/browse/components/GlobalSearchToolbar.kt` (`pinned_sources`, `all`, `has_results`) | Search tab global search, no pinned / has-results toggle | S |
| **Extension details:** enable or disable each source, per-source settings, clear cookies, incognito for the extension, uninstall, app info, version, language, content warning | `P/browse/ExtensionDetailsScreen.kt` | Details, settings, capabilities; no per-source toggle, cookies or incognito | S |
| **Extension trust and load states** (untrusted, unsigned, unsupported, malformed, load failed with stack trace, obsolete/orphaned), revoke trust, update all with a pending count, extension language filter | `P/browse/ExtensionsScreen.kt`, `ExtensionFilterScreen.kt`, `ext_*` strings | Failed loads are reported; no trust model, update-all count or language filter | M |
| **Migrate tab:** sources with library counts, sort alphabetically or by count | `P/browse/MigrateSourceScreen.kt` | None | (with migration) |
| **Deep links:** opening a source URL finds the title in the installed sources | `UI/deeplink/*` | Open link handles video and audio only | M |

## 9. Data, backup, storage

| Gap | Mihon | Raygun today | Size |
|---|---|---|---|
| **Backup options:** entries, chapters, tracking, history, categories, app settings, extension repo settings, source settings, sensitive settings | `app/.../data/backup/create/BackupOptions.kt`, `P/more/settings/screen/data/CreateBackupScreen.kt` | One full backup, no options | S |
| **Automatic backups** (interval, kept copies) and a **storage location** folder picker | `backup_interval`, `pref_storage_location` | Manual only (M50) | M |
| **Restore report:** missing sources and missing trackers, duration | `backup_restore_missing_*`, `restore_duration` | Basic result | S |
| **Storage usage** (bar by category); clear chapter cache; clear on app launch | `P/more/settings/screen/data/StorageInfo.kt`, `pref_clear_chapter_cache`, `pref_auto_clear_chapter_cache` | About shows the data folder | S |
| **Export library list** | `app/.../data/export/LibraryExporter.kt` | None | S |

## 10. Statistics

| Gap | Mihon | Raygun today | Size |
|---|---|---|---|
| **Stats screen:** entries in library, read duration, completed; in global update, started, local; total, read and downloaded chapters; trackers (tracked, mean score, used) | `UI/stats/*`, `P/more/stats/*` | None (M48) | M (read duration needs tracking time in the reader) |

## 11. Security and privacy

| Gap | Mihon | Raygun today | Size |
|---|---|---|---|
| **App lock** (biometric or device credential), lock after (always, minutes, never) | `use_biometric_lock`, `lock_app_after`, `UI/security/UnlockActivity.kt` | None (M51) | M (native) |
| **Secure screen** (always, incognito only, never): no screenshots, hidden in recents | `secure_screen_v2`, `SecureScreenMode` | None | S (FLAG_SECURE) |
| **Hide notification content** | `hide_notification_content` | None | S |
| **Incognito mode** | see §5 | None | S |

## 12. Appearance and general

| Gap | Mihon | Raygun today | Size |
|---|---|---|---|
| **Theme palettes:** Dynamic (Monet), Catppuccin, Tokyo Night, Green Apple, Lavender, Midnight Dusk, Monochrome, Nord, Strawberry Daiquiri, Tako, Teal & Turquoise, Yin & Yang, Yotsuba, Tidal Wave; plus pure black | `UiPreferences.kt` (`pref_app_theme`), `theme_*` strings | Light/dark/system and 7 accents | M |
| **Date format, time format, relative timestamps** | `app_date_format`, `app_time_format`, `relative_time_v2` | Fixed ISO dates in places | S |
| **Searchable settings** | `P/more/settings/screen/SettingsSearchScreen.kt` | None (M51) | M |
| **Onboarding:** theme, storage folder, permissions (notifications, battery optimisation), guides | `P/more/onboarding/*` | None (M55) | M |
| **Advanced:** share crash logs, verbose logging, redact headers, debug info, manage notifications, battery optimisation, clear cookies, clear WebView data, DNS over HTTPS (Cloudflare, Google, AdGuard, Quad9 and more), default user agent, refresh library covers | `P/more/settings/screen/SettingsAdvancedScreen.kt` | None (M51) | M |

## 13. Translation and enhancement forks

| Gap | Source | Raygun today | Size |
|---|---|---|---|
| **Page translation (TachiyomiAT):** OCR language, target language, engine (with model, API key, temperature, max tokens), font, auto-translate after download | `reference/TachiyomiAT/domain/src/main/java/tachiyomi/domain/translation/TranslationPreferences.kt` | Planned M46/M47 (translators exist since M34) | L |
| **Image enhancement (mihon_img_upscale):** Real-CUGAN / Real-ESRGAN / waifu2x models via Vulkan (or NPU), scale, input resolution, skip above a resolution, preload range, priority to visible pages, enhanced-page cache; ink filters | `reference/mihon_img_upscale/README.md` | Planned M45 | L |

---

## Each Mihon settings screen, option by option

✗ = Raygun lacks it; ✓ = Raygun has it; ~ = partly.

- **Appearance:**
  - App theme (palettes) ~; pure black ✓; app language ✓; tablet UI ✓ (rail);
  - date format ✗; time format ✗; relative timestamps ✗; images in descriptions ✗.
- **Library:**
  - categories (edit, default category ✗, per-category settings ✗);
  - global update: automatic updates interval ✓, device restrictions ✗, categories include/exclude ✗, refresh metadata ✗, skip unread/completed/unstarted/outside release period ~ (completed only), Updates icon badge ✗;
  - behaviour: chapter swipe left/right ✗, mark duplicate read chapters ✗, hide missing-chapter indicators ✗.
- **Reader:**
  - general: default reading mode ~, double-tap animation speed ✗, show reading mode ✗, tap-zone overlay ✗, animated transitions ✗, load ahead only on Wi-Fi ✗;
  - display: default rotation ✗, background colour ✗, fullscreen ✗, cutout ✗, keep screen on ✗, page number ✗, e-ink flash (4 options) ✗;
  - reading: skip read/filtered/duplicate ✗, always show chapter transition ✗;
  - paged: tap zones ✗, invert ✗, scale type ✗, zoom start ✗, crop borders ✗, landscape zoom ✗, pan wide ✗, split/rotate wide pages (with invert) ✗;
  - webtoon: tap zones ✗, invert ✗, side padding ✗, menu-hide sensitivity ✗, crop borders ✗, split/rotate ✗, double-tap zoom ✗, disable zoom out ✗;
  - navigation: volume keys (and invert) ✗, vertical navigator (modes, side, height) ✗;
  - actions: long-tap actions ✗, folder per title for saved pages ✗.
- **Downloads:**
  - Wi-Fi only ✗, save as CBZ ✗, concurrent sources ✗, concurrent pages ✗;
  - delete after marked read ✗, after reading (slots) ✗, allow deleting bookmarked ✗, excluded categories ✗;
  - download new chapters ✗ (and unread only ✗, categories ✗);
  - download ahead ✗.
- **Tracking:** update progress after reading / when marked read, tracker logins (⏸ M54).
- **Browse:**
  - hide entries in library ✗, extension repos ✓;
  - allowed content warnings ~ (one NSFW switch versus safe/mixed/18+ choices), apply to installed extensions ✓.
- **Data and storage:**
  - storage location ✗;
  - create backup ✓ (options ✗), restore ✓, automatic backup frequency ✗;
  - storage usage ✗, clear chapter cache ✗, clear on launch ✗, export library ✗.
- **Security and privacy:** require unlock ✗, lock when idle ✗, secure screen ✗, hide notification content ✗.
- **Advanced:**
  - crash logs ✗, verbose logging ✗, redact headers ✗, debug info ✗, onboarding guide ✗, manage notifications ✗, battery optimisation ✗;
  - reindex downloads ✗, clear database ✗, clear cookies ✗, clear WebView data ✗, DoH ✗, user agent ✗;
  - refresh library covers ✗, reset per-series reader settings ✗, update titles to match source ✗, non-ASCII filenames ✗, chapter name hash ✗, high-quality renderer ✗.
- **More screen:** Downloaded only ✗, Incognito ✗, Download queue ✓, Categories ✓, Statistics ✗, Data and storage ✓, Settings ✓, About ✓, Help ✗.

Most library, update, download, history and selection gaps apply to novels and saved videos too: Raygun shares one title store and `MangaLibraryView` across all three, so building them once covers every medium.

## Summary: the 15 most valuable gaps

1. **Library selection mode with bulk actions** (categories, read state, download, remove). M.
2. **Chapter selection and bulk actions plus bookmarks** (DB column, filters, download bookmarked). M.
3. **Chapter settings sheet:** filter (tri-state, scanlators), sort, display, set as default. M.
4. **Reader settings depth:** scale types, zoom start, crop borders, split/rotate wide pages, background colour, page number, keep screen on, rotation. M–L.
5. **Tap zone layouts with invert and overlay; volume keys; long-tap page actions** (save, share, set as cover). M.
6. **Missing reading modes:** vertical paged, continuous vertical, dual-page spreads. M–L.
7. **History screen** for manga and novels, with incognito mode. M.
8. **Download rules:** delete after reading, auto-download new chapters, download ahead, Wi-Fi only, CBZ, concurrency. S–M.
9. **Download queue management** (reorder, sort, move to top/bottom, cancel all). M.
10. **Library display:** list and cover-only modes, per-orientation columns, per-category display/sort, more sorts, tri-state filters, more badges and a Continue reading button. M.
11. **Updates:** filters and bulk actions; new-chapter notifications. M.
12. **Library update rules:** device and entry restrictions, include/exclude categories, smart update intervals. M.
13. **Migration** between sources and **duplicate detection** when adding. L / S.
14. **Title extras:** cover dialog (save, share, custom cover), notes, share, tap author/genre to search, missing-chapter indicators. M.
15. **Privacy and data:** app lock, secure screen, statistics, backup options, automatic backups, storage usage. M.

Also: the parity table overclaims M27 (spreads and vertical paged mode were out of scope), and Mihon's Library has no path to sources except search ("Search globally"), which matches Raygun's rule.
