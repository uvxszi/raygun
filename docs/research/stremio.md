# Research: stremio-core vs Raygun (video, Stremio addons)

Source: `reference/stremio-core` (MIT). Compared with Raygun after M43 (`crates/raygun-app/src/stremio/`, `video_library.rs`, `video.rs`, `apps/raygun/src/watch/`, `pages/Watch*.tsx`, `pages/StremioAddonPage.tsx`, `search/VideoResults.tsx`, Settings → Player, `VideoPlayerActivity.kt`).

Sizes: **S** = a day or less, **M** = a few days, **L** = a milestone of its own.

## How Stremio organises things

- **Board (home):**
  - Continue watching first (`models/continue_watching_preview.rs`).
  - Then a preview row of every catalog of every installed addon (`models/catalogs_with_extra.rs` with no extra).
- **Discover** (`models/catalog_with_filters.rs`), one screen, three choices:
  - **type:** movie, series, channel, tv… ordered by `TYPE_PRIORITIES`;
  - **catalog:** across all addons that list that type;
  - **every extra with options:** genre, year, sort or whatever the manifest declares. Pages of 100.
- **Search:**
  - `catalogs_with_extra.rs` asks every catalog that has a `search` extra and shows one row per catalog;
  - `local_search.rs` gives autocomplete from Cinemeta's `feed.json` index, weighted by IMDb rating and popularity;
  - `types/search_history/` keeps recent queries, cleared with `ClearSearchHistory`.
- **Library** (`models/library_with_filters.rs`, `library_by_type.rs`) holds only what you added:
  - a type filter;
  - sorts: last watched, name A–Z and Z–A, times watched, watched, not watched;
  - pages.
  - No catalogs and no discovery: this matches the user's 2026-11-01 ask that Library holds only your things.
- **Calendar** (`models/calendar.rs`): a month grid of every library series' episodes, past and future, from addons' `calendarVideosIds` catalog extra.
- **Notifications** (`models/ctx/update_notifications.rs`): new episodes of library series, shown as badges on Continue watching.

**Raygun today:**
- Home (Continue watching and reading);
- Browse → one addon's rows (`StremioAddonPage`) or one catalog grid (`WatchCatalogPage`, genre only);
- Search → rows from searchable catalogs;
- Library → Videos (saved titles, categories, filters);
- Updates → Episodes (new, then an Upcoming strip).

There's no Board across addons, no Discover with type or catalog switching or other extras, no search history or suggestions, and no calendar.

---

## 1. Continue watching and watched state (most valuable)

| Gap | Stremio | Raygun | Size |
|---|---|---|---|
| **Series-level Continue watching that moves to the next episode** | `LibraryItem.state.video_id` points at the episode to continue. Past the credits threshold (`CREDITS_THRESHOLD_COEF = 0.9`) or on a mark, `advance_to_video` moves it to `MetaItem::next_video` (next released episode, never into specials) with `time_offset = 1`, so the series stays in Continue watching showing the next episode (`types/library/library_item.rs`, `models/player.rs` `library_item_state_update`) | Continue watching is one entry per progress key (episode). Finishing an episode drops it, and the series disappears instead of offering E+1 (`raygun-db::video::continue_watching`) | M |
| **Continue watching includes series with new episodes** | An item shows if `is_in_continue_watching()` *or* it has notifications. It's sorted by the newest notification's release date, else `mtime`, and carries a notification count (`continue_watching_preview.rs`) | Only unfinished videos | S (after notifications) |
| **Watched means time actually watched** | `time_watched` (time played, not position) > 70% of duration (`WATCHED_THRESHOLD_COEF`) sets `flagged_watched` and the episode's bit. Seeking doesn't add watched time (`player.rs` Seek vs TimeChanged) | Position ≥ watched point (default 90%), so seeking to the end marks it watched | S |
| **Mark season / whole series watched** | `MarkSeasonAsWatched(season, bool)`, `MarkAsWatched` for the whole item. `reconcile_series_resume_after_watched_change` moves the resume pointer past watched episodes | One episode at a time (`WatchTitlePage` check buttons), plus movies | S |
| **Times watched and rewatch** | `times_watched`, `overall_time_watched`, `last_watched`; `RewindLibraryItem` resets progress without unwatching | Only watched-at; no rewatch count, no rewind | S |
| **Watched bitfield sync format** | `stremio-watched-bitfield` crate: compact bitfield anchored to the last video id, robust to new episodes | Per-unit rows (fine for Raygun; only matters for importing Stremio accounts) | note |
| **Temp library items** | Watching anything creates a `temp`/`removed` library item, so Continue watching works for unsaved titles; saving clears `removed` | `video_progress` rows do the same job | — (parity) |

## 2. Player and playback

| Gap | Stremio | Raygun | Size |
|---|---|---|---|
| **Next episode and binge watching** | **Binge:** `next_video` is computed for the playing episode. `next_streams` are requested ahead, and `next_stream` is the stream of the next episode with the same `behaviorHints.bingeGroup` (`player.rs` `next_stream_update`, `Stream::is_binge_match`). **Settings:** `binge_watching` (autoplay) and `next_video_notification_duration` (when to show the "Next episode" prompt). **Player:** has a NextVideo action | No next button (`setShowNextButton(false)`), no prompt, no autoplay. The player closes at the end | M |
| **Remember the stream per series** | `StreamsBucket` stores the last stream per (meta, video). `last_stream_item` looks back up to 30 earlier episodes. Meta details shows a "last used" stream matched by source, else by bingeGroup (`types/streams/`, `meta_details.rs` `last_used_stream_update`) | The stream picker starts fresh every time | M |
| **Per-stream player state** | `StreamItemState` remembers, for each stream: subtitle track, on/off, subtitle delay, size and vertical offset, audio track, audio delay, playback speed, player type and video scale (contain, cover, fill). It's restored on return and carried to the next episode (`types/streams/streams_item.rs`, `ActionPlayer::StreamStateChanged`) | Tracks, speed and scale reset every time | M |
| **Subtitle and audio preferences** | Settings: `audio_language`, `secondary_audio_language`, `subtitles_language`, `secondary_subtitles_language`, `subtitles_auto_select`, size, font, bold, offset, text, background and outline colours, opacity, ASS styling (`types/profile/settings.rs`) | Auto subtitles on/off and one language | M (Media3 `CaptionStyleCompat`, `setPreferredAudioLanguage`) |
| **Two seek steps** | `seek_time_duration` and `seek_short_time_duration` | One step | S |
| **Playback settings** | `play_in_background` (audio continues when leaving), `hardware_decoding`, `gpu_video_processing`, `frame_rate_matching_strategy`, `audio_passthrough`, `surround_sound`, `pause_on_minimize`, `player_type` (external player) | PiP only (pauses when it isn't PiP) | M |
| **Intro/outro skip** | `IntroOutro` / `SkipGaps` from Stremio's API (premium, account) (`player.rs` `skip_gaps_update`) | None (M38 lists chapters-based skip) | note (account) |
| **Subtitle matching by file** | `VideoParams {hash, size, filename}` go to subtitle addons as `videoHash`, `videoSize` and `filename` extras (`VIDEO_*_EXTRA_PROP`), so OpenSubtitles returns exact matches | Subtitles are asked by id only (`stremio::subtitles`). `behaviorHints.filename/videoSize/videoHash` aren't parsed | S |
| **Seek log** | `seek_history` collected for series before 10 minutes (`PLAYER_IGNORE_SEEK_AFTER`), to learn intros | None | note |

## 3. Notifications and calendar

| Gap | Stremio | Raygun | Size |
|---|---|---|---|
| **Cheap new-episode checks** | **Request:** every 6 hours (`REQUEST_LAST_VIDEOS_EVERY`), one catalog request with the `lastVideosIds` extra lists every library series (id and type, newest first, up to 100). Addons such as Cinemeta answer with their latest videos. **Notification:** a video becomes one when released after the item's `last_watched` and not in the future. **Exclusions:** movies, live items, `defaultVideoId` items and items with `no_notif` are skipped (`update_notifications.rs`, `LibraryItem::should_pull_notifications`) | The library update fetches the full meta of every saved series, one request each (`StremioService::refresh`) | M |
| **Per-title notification toggle** | `ToggleLibraryItemNotifications(id, bool)` (`state.no_notif`) | None. Scheduled updates skip only finished series | S |
| **Dismiss notifications** | `DismissNotificationItem(meta_id)` clears a title's badges; storage keeps them across restarts | The Updates feed has no dismiss or clear | S |
| **Calendar** | A month grid (`MonthInfo`: today, days, first weekday; previous/next month), each day listing library series' episodes released that day, past and future, from the `calendarVideosIds` extra (`models/calendar.rs`) | An Upcoming strip, future only, from stored units (refreshed only by library updates) | M |

## 4. Title details and streams

| Gap | Stremio | Raygun | Size |
|---|---|---|---|
| **Links** | `MetaItemPreview.links` (`Link {name, category, url}`): IMDb rating link (`IMDB_LINK_CATEGORY`), genre links to Discover filtered by that genre (`GENRES_LINK_CATEGORY`), cast and director links to Search, and addon-defined categories ("Writers"…) | Genres are plain chips; cast and director are plain text; `links` isn't parsed | S |
| **Streams inside the meta** | `Video.streams` (e.g. YouTube channel videos) plays without stream addons. `Video::stream()` returns the single stream, or a YouTube stream for `yt_id:` ids. `meta_streams` are listed first (`meta_details.rs` `meta_streams_update`) | Ignored: episodes always ask stream addons, so channel-type addons can't play | S |
| **Default and featured video** | `behaviorHints.defaultVideoId` (a multi-video item that plays one video), `featuredVideoId`, `hasScheduledVideos`. "Guess stream" opens streams directly for movies and live items (`selected_guess_stream_update`) | Not parsed | S |
| **More stream source kinds** | `StreamSource`: Url, YouTube, Torrent, Rar/Zip/7z/Tgz/Tar archives, Nzb, PlayerFrame (embedded web player), External (`externalUrl`, `androidTvUrl`…) (`types/resource/stream.rs`) | Direct, YouTube, External (refused with "opens outside Raygun"), Torrent (refused) | S (external: open the URL or an Android intent); archives and NZB need a streaming server (note) |
| **Stream hints** | `notWebReady` (needs the server to transcode), `bingeGroup`, `countryWhitelist` (hide streams not available in the user's country), `proxyHeaders.request/response`, `filename`, `videoHash`, `videoSize`; stream `thumbnail` | Only `proxyHeaders.request` (five allowed headers). No thumbnail, no country filter, no binge group | S |
| **Stream list grouping and filtering** | Streams grouped by addon, with an addon filter; the last-used stream pinned on top | Grouped by addon, with no filter and no pinning (`StreamPicker`) | S |
| **Per-video trailers** | `Video.trailer_streams`; meta `trailerStreams` of any source | YouTube trailers for the title only | S |
| **Meta from several addons** | Meta requested from every addon that serves the id; the first ready one is used, the others kept as fallbacks | First addon that answers | — (parity) |
| **Released date and logo** | `released` (movies), `logo` shown as title art, `background` | Logo parsed but unused (`TitleHero` shows the text title) | S |
| **Ratings and likes** | `rating_info` (`types/rating/`): likes and dislikes for tt, tmdb and kitsu movies and series, through the Stremio account | None | note (account) |

## 5. Discover, Board and Search

| Gap | Stremio | Raygun | Size |
|---|---|---|---|
| **Discover screen** | **Type:** a type selector across all addons. **Catalog:** a catalog selector for that type across addons. **Extras:** every extra with options (`SelectableExtra` with `options` and `is_required`; `options_limit`), not just genre. **Pages:** 100 per page (`CATALOG_PAGE_SIZE`) (`catalog_with_filters.rs`) | Per-catalog grid with genre chips and "has more" paging (`WatchCatalogPage`). Other extras such as `year` aren't offered; no type or catalog switching | M |
| **Board across addons** | Continue watching, then preview rows of every addon's catalogs together | Rows per addon (Browse → addon). The cross-addon home (`stremio_home`) exists in Rust but has no screen since M42.1 | S |
| **Search suggestions** | Local fuzzy index of Cinemeta's catalog feed (`local_search.rs`, `Searchable {id, name, type, poster, imdbRating, popularity}`), weighted half by rating and half by popularity | None | M |
| **Search history** | `SearchHistoryBucket` (recent queries with dates), clearable | None | S |
| **Addon catalogs** | `manifest.addonCatalogs`: catalogs of addons (community collections) browsed like content in Discover (`catalog_with_filters.rs` "addon_catalog") | None | M |

## 6. Library (Stremio's view)

| Gap | Stremio | Raygun | Size |
|---|---|---|---|
| **Type filter** | All, movie, series, channel… (`library_with_filters.rs` `SelectableType`) | Categories and status filters; no movie/series/anime split (`title.remote_media_type` is stored but unused) | S |
| **Sorts** | Last watched, name, name reversed, times watched, watched, not watched | Title, last read, latest episode, unwatched count, date added | S |
| **Progress on cards** | Library and Continue watching cards show a progress bar and watched marks | Badges only (unwatched and downloaded counts) | S |
| **Card actions** | Remove from library, rewind, mark watched/unwatched, toggle notifications from the item's menu | No long-press or selection menu on library cards | M (Mihon-style selection mode covers it) |

## 7. Addon management

| Gap | Stremio | Raygun | Size |
|---|---|---|---|
| **Addon details** | Manifest, types, catalogs, resources, version, `configurable` / `configurationRequired` (opens `/configure` on the addon), install, upgrade, uninstall (`addon_details.rs`, `ActionCtx::InstallAddon/UpgradeAddon/UninstallAddon`) | A list row with name, description and resources; "needs setup on its website" is refused at add; no configure button, no details screen | M |
| **Ordering** | The installed order sets the priority of meta, streams and catalogs | `position` exists in `raygun-db::stremio`, but there's no reorder command or UI | S |
| **Official and protected addons** | `DescriptorFlags {official, protected}`: protected ones (Cinemeta, local files) can't be removed; official ones are labelled. The official list ships with the app (`OFFICIAL_ADDONS`) | Cinemeta and OpenSubtitles are seeded but removable; two suggestions | S |
| **Installed-addon filters** | `installed_addons_with_filters.rs` filters installed addons by type | None | S |
| **Adult and P2P hints** | `behaviorHints.adult` (Raygun honours it), `p2p` (torrent addons), `epgProvider` | Only `adult` | S |

## 8. Settings Stremio has that Raygun lacks

- **Interface:** `hide_spoilers` (blur titles and thumbnails of unwatched episodes), `interface_scale`, `gamepad_support`.
- **Playback:**
  - `binge_watching`, `next_video_notification_duration`, `play_in_background`, `pause_on_minimize`;
  - `hardware_decoding`, `gpu_video_processing`, `frame_rate_matching_strategy`, `video_mode`;
  - `audio_passthrough`, `surround_sound`, `player_type` (external player).
- **Subtitles:**
  - secondary subtitle language, `subtitles_auto_select`;
  - size, font, bold, offset;
  - text, background and outline colour, opacity, `ass_subtitles_styling`.
- **Audio:** `audio_language`, `secondary_audio_language`.
- **Seeking:** `seek_time_duration`, `seek_short_time_duration`.
- **Streaming server:** URL, warning dismissal, `server_in_foreground`.

## 9. Notes (not now, or account-bound)

- **Streaming server and torrents:**
  - `streaming_server.rs` and `types/streaming_server/` cover cache size, BitTorrent profile, transcoding, proxy streams, remote HTTPS, statistics, and conversion of torrents and archives to HTTP;
  - `models/streaming_server/casting.rs` covers Chromecast through the server.
  - Raygun refuses torrents (feature-parity: ✖ for now); casting is M52.
- **Live TV:** `live_tv_guide.rs` (EPG by date), `live_tv_continue_watching.rs`, `VideoEpgInfo`, `programs_at`. Channels with programmes; Raygun has none (L).
- **Accounts:** auth and sync of library and addons (`update_profile.rs`, `update_library.rs` push/pull), Trakt addon (`update_trakt_addon.rs`), user likes, events/modals, data export. On hold with sync and accounts (M54).

---

## Most valuable gaps, in order

1. Series-level Continue watching that moves to the next episode.
2. Next-episode prompt and autoplay (binge), matching the stream by `bingeGroup`.
3. Remember the last stream per series and offer it first.
4. Per-stream player state (subtitle and audio track, delays, size, speed, scale), restored and carried to the next episode.
5. Subtitle and audio preferences: languages with fallback, size, colours, outline, background.
6. Cheap new-episode notifications (`lastVideosIds`) with badges on Continue watching, per-title toggle and dismiss.
7. Calendar month view of library episodes.
8. A Discover screen: type, catalog and every extra (not just genre).
9. Search history and title suggestions.
10. Addon details: configure (`/configure`), upgrade, reorder, protected and official addons.
11. Mark season or series watched; watched by time actually watched; rewatch count.
12. Streams inside the meta (`Video.streams`, `yt_id:`) and `defaultVideoId`.
13. Clickable links: genres to Discover, cast to Search, IMDb.
14. Library type filter, more sorts and progress bars on cards.
15. Stream hints: thumbnails, country filter, filename/hash/size passed to subtitle addons; open external streams instead of refusing them.
