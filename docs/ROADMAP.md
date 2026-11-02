# Raygun Roadmap

Each phase ends with something usable. Milestones (M) are sized to fit **one focused working session**: about a day of work with a clear done-check, so no session has to hold the whole system in its head. Before a milestone starts it gets a short spec in `docs/phases/phase-N/MNN-<name>.md`. The spec covers its goal, files touched, interfaces, a done-check, and which sections of `ARCHITECTURE.md` / `reference-notes.md` are relevant, so a session loads only those.

**Platform order:** Android first (verified on the `Pixel_4` emulator), Windows second. **Since M05, only the 📱 checks run; desktop work and checks are deferred until the user asks for them. Since 2026-11-01: mobile only until the app is great.** macOS, Linux and iOS are built in CI from M10 on, but are only lightly tested until later phases. Every UI milestone's done-check runs on Android before desktop.

Legend: 📱 Android · 🖥 Windows · ⚗ spike (time-boxed; the outcome is a decision, not production code)

---

## Phase 1 — Foundation + "it plays"

**Goal:** an Android APK and a portable Windows build share one responsive UI and can play local music plus a YouTube track, with all desktop state stored next to the exe.

| M | Milestone | Done when |
|---|---|---|
| M01 | **Monorepo scaffold**: pnpm/turbo + Cargo workspace, `apps/raygun` Tauri v2 + React/Vite/Tailwind v4, lint/format/test wiring, tauri-specta bindings pipeline, Android project generated | 📱 app runs on the emulator and 🖥 `pnpm dev` opens a window; one typed command round-trips on both; `pnpm test` and `cargo test` pass |
| M02 | **`raygun-paths` + portable mode**: root resolution (exe dir / AppImage / .app + translocation check / `RAYGUN_HOME` / Android sandbox), webview `data_directory`, `tracing` logs to `logs/`, clippy ban on OS dir APIs | 🖥 portable zip runs from a USB folder with zero new files/keys in `%APPDATA%`, `%LOCALAPPDATA%` and `HKCU`; 📱 paths resolve inside the app sandbox |
| M03 | **`raygun-db` + settings**: sqlx SQLite (WAL), embedded migrations with **sync-ready conventions** (UUIDv7 ids, HLC columns, tombstones), typed settings stored in the DB (per-key, syncable), settings service + UI | Settings survive restart (📱) and moving the folder (🖥) |
| M04 | **Responsive app shell**: routes (Home, Library, Search, Settings, Player), bottom tabs ↔ side rail, mini-player ↔ full-screen sheet with gestures, theming tokens, i18n | 📱 gestures work on the emulator; usable at 360 px and 1440 px |
| M05 | **Playback core**: `PlaybackService` (queue, repeat/shuffle, canonical state, events), engine protocol, **Html5Engine**, `net` loopback stream proxy (Range, CORS, header injection, local files) | A queue of HTTP and local URLs plays, seeks and skips on 📱 and 🖥 |
| M06 | ⚗ **Mobile native audio**: minimal Tauri Kotlin plugin wrapping Media3 for one direct audio URL with notification + background play | Decision recorded: native audio engine on Android yes/no and its scope |
| M07 | **Local music**: folder scan (`lofty` tags + embedded art; Android media store / SAF picker), music schema, Library → Artists/Albums/Tracks, play album/shuffle | A 5k-track folder scans and is browsable and playable |
| M08 | ⚗ **EmbedEngine**: loopback page at `http://localhost:PORT/embed/youtube`, iframe + postMessage control, hidden/visible video, error → next candidate | A YouTube ID plays on 📱 **and** 🖥; result recorded in a decision note |
| M09 | **OS media integration**: Android media session (per M06's outcome), SMTC on Windows, MPRIS on Linux; Android status-bar icon color follows the app theme | Lock-screen controls and hardware media keys work |
| M10 | **CI**: GitHub Actions builds Android APK, Windows portable zip, Linux AppImage, macOS .app; leak-check job on Windows | Artifacts appear on every main-branch push |

## Phase 2 — Extension host, MCP, online music, device sync
- M11 `raygun-ext-host`: manifest, permissions, registry, **rquickjs** runtime, host API (`http.fetch`, `html.parse`, storage, settings, log)
- M12 `@raygun/sdk` package + extension template + build tooling, hot-reload in dev
- M13 Repository format (`index.json`), install/update/uninstall UI, per-extension settings UI
- M14 Resolver pipeline: candidate scoring, verification, caching, automatic fallback (direct → proxied → embed)
- M15 First-party extensions: YouTube (official Data API with the user's key; embed streams), MusicBrainz/Cover Art, LRCLIB synced lyrics. *Decision (M15): no InnerTube direct-stream extraction in first-party code; it works around YouTube's protections and terms. Background YouTube audio on Android stays unavailable.*
- M16 yt-dlp sidecar (desktop): download, verification, `music.stream` provider
- M17 **Nuclear plugin compatibility**: Sucrase TS→CJS converter run in QuickJS at install, `NuclearPluginAPI` shim over the host API, React/UI stubs, compatibility report per plugin; tested against several real plugins from the Nuclear registry
- M18 Playlists, favorites, history, search across providers
- M19 **MCP server**: rmcp streamable HTTP, token + origin auth, meta-tools generated from the service catalog, resources, stdio shim
- M20 LAN remote: REST/SSE + paired remote web UI (QR)
- M21 **Device sync v1**: mDNS discovery (+ Android MulticastLock), QR/code pairing with Ed25519 keys, encrypted peer channel, HLC change log + per-field merge, Settings → Devices (toggles, log, undo)
- M22 *(on hold by the user's call, 2026-10-31)* **Device sync v2**: automatic triggers (start, network change, debounced local changes), Android WorkManager background sync on Wi-Fi, optional on-demand file transfer for downloads
- M23 Lyrics view with synced scrolling; Furigana/romaji via a lyrics post-processor (kuromoji-style tokenizer)

## Phase 3 — Manga
- M24 Episodic library schema (title/unit/progress/category/external_id), library service, sync rules for these tables *(built together with M25 and MangaDex from M26 as the "manga foundation" slice; categories and sync rules later)*
- M25 `manga.source` capability + SDK types (Mihon-shaped: popular/latest/search+filters/details/chapters/pages)
- M26 First-party sources: MangaDex, local CBZ/folder, Komga/Kavita
- M27 Reader: paged (LTR/RTL), webtoon, spreads, zoom, tap zones, prefetch, page cache
- M28 Library updates job (fetch intervals), updates feed, downloads queue
- M29 Trackers: AniList, MyAnimeList (OAuth via loopback/deep link), sync of progress *(waits for the user's tracker accounts; M30 went first)*
- M30 Backup/restore (own format) + **Mihon backup import**

## Phase 4 — Light novels
- M31 `novel.source` capability + novel reader (reflow, typography, scroll/paginated, progress by char offset)
- M32 **Shosetsu compatibility**: mlua (lua52) runtime + kotlin-lib shim, Shosetsu repo support
- M33 Local EPUB import, chapter downloads, TTS (OS voices) as a stretch goal

## Phase 5 — AI groundwork
- M34 Model manager (`models/`, checksums, licenses) + `raygun-ai` provider registry + secrets store

## Re-plan (2026-11-01): mobile first, the user's asks, then full parity
The user asked for: mobile only until the app is great; a **Watch** tab for movies and TV; **podcasts** in the music tab; **browsing extensions across repositories** (Raygun's official one and others), with basic essentials in the official repo; an **NSFW toggle** (off by default) that hides NSFW extensions and content; **language switching** set up for contributors, reusing the reference apps' existing translations; and **every feature the reference apps have**. The milestones below replace the old M35–M51 numbering. `docs/feature-parity.md` is the full checklist the parity milestones work through.

## Phase 6 — The user's asks
- M35 **Content filter + languages**: Settings toggle "Show NSFW content" (off by default) hiding NSFW extensions, sources and titles (repo `contentRating`, Stremio `adult`, Shosetsu/LNReader flags, adult genres), passed to extensions so they filter too; language picker (system default + every locale present), i18n structure and contributor guide, and translations imported from the reference apps where the English text matches exactly
- M36 **Extension browsing across repositories**: suggested repositories to add with one tap (Raygun official, Shosetsu, Nuclear plugins, Stremio addons), Browse with search and filters (kind, language, repository, content rating), repository trust note; essentials published in the official repo (MangaDex, Project Gutenberg, podcast directory, public-domain films)
- M37 **Podcasts** in the music tab: subscriptions, RSS/Atom feeds, `podcast.directory` capability + an Apple Podcasts directory extension, episode lists, resume position, played state, new-episode refresh, episode downloads
- M38 **Watch tab + native video player**: Media3 ExoPlayer activity in the Android plugin (gestures for seek/volume/brightness, PiP, subtitles .srt/.vtt/.ass, audio/subtitle track select, speed, resume, OP/ED skip), HLS/DASH, header injection
- M39 **Library by media type and category** (asked for 2026-11-01): Music, Podcasts, Manga, Novels, Videos; Mihon-style categories; search, filters, sort and genre tags
- M40 **Search across media**: All or one type (music, podcasts, manga, novels, videos, your library), source filters and genre tags (include/exclude), sources that declare filters
- M41 **Stremio addons**: catalogs/meta/streams/subtitles over the HTTP addon protocol, Cinemeta for metadata, Watch home (popular movies and series, continue watching), title pages with seasons and episodes, watched state
- M42 `video.source` capability (CloudStream/Aniyomi-shaped: search, load, servers streamed as they resolve) + first-party public-domain films; anime as a kind
- M43 Video library, downloads, updates for followed series

## Phase 6.5 — Depth from the reference apps (re-planned 2026-11-01)
The user asked for the full depth of the reference apps ("its not meant to be a simple app"), researched per app in `docs/research/` (mihon, aniyomi, cloudstream, stremio, shosetsu, music). Each milestone below names the report sections it works through; its spec lists the exact rows. The Library holds only your content; finding content lives in Browse (its search bar, sources and extensions).

- M44 **Playback sessions** (aniyomi §1, §6; stremio §1–2; cloudstream §5):
  - an episode plays at once from the best link, falling back on errors;
  - sources switch inside the player;
  - previous/next episode, autoplay next with a countdown, and an episode list in the player;
  - Continue watching moves on to the next episode;
  - the last server or addon is remembered per series.
- M45 **Player controls and settings** (aniyomi §2–7; cloudstream §5; stremio §2, §8):
  - **Overlay:** a custom overlay with lock, aspect, rotation and default orientation.
  - **Speed:** presets, a default, and hold for 2×.
  - **Gestures:** brightness and volume bars with boost, horizontal seek, configurable double-tap and media keys.
  - **Chapters:** chapters, skip intro/outro with a countdown, AniSkip.
  - **Subtitles and audio:** subtitle styling, delay, a second track and local files; audio and subtitle language lists.
  - **Remembered per title:** tracks, speed and scale.
  - **Tools:** PiP actions, sleep timer, screenshots, external player, decoder switch.
  - **Settings:** Settings → Player categories.
- M46 **Library depth** (mihon §1; cloudstream §1; aniyomi §9; shosetsu library):
  - **Lists:** selection mode with bulk actions; status lists (Reading/Watching, Plan to, Completed, On hold, Dropped) and favorites.
  - **Display:** display modes (list, comfortable, compact, cover only), per-category display and sort, columns per orientation.
  - **Sorting and filters:** more sorts, tri-state filters and badges.
  - **Extras:** continue buttons, duplicate detection, random pick, local source (folders, CBZ, EPUB).
- M46.1 **Save to the device and export** (asked 2026-11-01: "downloading to a specific folder like downloads", "manga screenshots", "full manga chapter or entire thing downloads in different formats"; pulls forward parts of M49 and M50):
  - **Storage:** where downloads go: app storage (the default) or a folder the user picks.
  - **Exports:** a chapter, a selection or a whole title, as an image folder, CBZ or PDF for manga; EPUB or text for novels; video files copied out. Missing chapters are downloaded first, at the source's pace.
  - **Pages:** save, share or set as cover from the reader.
- M46.2 **Image cache and cache settings** (asked 2026-11-01: "extension icons and other shows and thumbnails … cache them so users dont have to keep loading them, and update when needed … have cache settings"):
  - covers, posters, thumbnails and extension icons kept on disk and refreshed when they change;
  - Settings → Data: cache sizes, limits and clear buttons per cache.
- M46.3 **Websites, with an ad and popup blocker** (asked 2026-11-01): sites the user enters, opened in a built-in browser in its own process, with a downloaded ad and tracker blocklist and popup and redirect blocking. Raygun bundles no particular site.
- M46.4 **Desktop video, trailers, the player bar and Search** (asked 2026-11-01 while testing the Windows build):
  - Watch plays on desktop in the webview, speaking the Android player's protocol; streams go through the proxy's new `/media/` route, which rewrites HLS and DASH playlists; mpv or VLC take what the webview can't decode.
  - Trailers open in a full-window player instead of the music queue.
  - The player bar gets Spotify's controls and a queue panel on wide windows.
  - Search picks sources with chips (several at once) and shows popular titles before anything is typed.
- M46.5 **Customizable layout** (asked 2026-11-01: "make it very customizable, so you can add and move around tabs and options and such, similar to mihon"): tabs, library media tabs, Browse sections, Home sections and More entries that can be hidden and reordered, with a start screen and hidden tabs moving into More (research: aniyomi's NavStyle, Mihon's category editor, CloudStream's pinned providers).
- M47 **Title pages and chapter/episode lists** (mihon §2; aniyomi §8; cloudstream §4; stremio §4; shosetsu novel screen):
  - **Lists:** filters, sort and display with defaults; selection with bulk actions (mark previous, download, delete, bookmark); bookmarks and fillers; swipe actions; download menus (next N, all unread).
  - **Title extras:** cover dialog and custom cover, notes, edit info, share, links (genres, authors and cast open Search).
- M48 **Updates, history, notifications, calendar** (mihon §4–5, §7; stremio §3; aniyomi §10, §12; shosetsu updates):
  - **Updates:** novels in the library update; Updates filters and bulk actions.
  - **History:** history per medium by day; incognito.
  - **Notifications:** Android notifications for new chapters and episodes, with a per-title toggle.
  - **Update rules:** Wi-Fi and charging restrictions, categories, skip rules, smart intervals; cheap Stremio `lastVideosIds` checks.
  - **Calendar** of upcoming episodes.
- M49 **Downloads depth** (mihon §6; aniyomi §11; cloudstream §7; shosetsu downloads):
  - a foreground service that keeps downloading with the app closed, with notifications;
  - parallel downloads; Wi-Fi only;
  - auto-download of new chapters and episodes, download ahead, delete after reading or watching;
  - queue reorder and sort; CBZ; a storage screen.
- M50 **Manga reader parity** (mihon §3):
  - the missing modes: vertical paged, continuous vertical, two-page spreads;
  - every reader setting: scale types, zoom start, crop borders, split/rotate wide pages, background, page number, keep screen on, rotation, transitions, e-ink flash;
  - tap-zone layouts with invert and overlay; volume keys;
  - long-tap page actions (save, share, set as cover).
- M51 **Novel parity** (docs/research/shosetsu.md):
  - **Reader:** reader themes and custom colours; paragraph spacing, indent, alignment and a wider size range; formatted chapters (bold, italics, lists, tables, links); continuous or swipe chapter changes; text selection and copy; keep screen on, volume keys, rotation lock.
  - **Read-aloud:** voice, pitch, pause, screen-off playback.
  - **Sources:** Shosetsu filters and extension settings in the UI.
  - **Extras:** custom CSS; LNReader plugins.
- M52 **Music parity** (docs/research/music.md):
  - **Queue and playback:** queue management and persistence; speed, pitch, skip silence, loudness normalization; sleep timer; equalizer.
  - **Discovery:** online artist and album pages; an Explore page in Search; endless radio.
  - **Offline:** stream cache and music downloads.
  - **Library and stats:** listening stats and history; playlist import/export and smart playlists; local library options and tag editing.
  - **Lyrics depth:** sources, word sync, offset, romanization.
  - **Integrations:** Android Auto, widgets, scrobbling with your own tokens.
- M53 **Search and discovery** (cloudstream §2–3; stremio §5, §7; mihon §8):
  - **Search:** history and suggestions; choosing and pinning sources; grouped or merged results.
  - **Explore in Search:** Stremio Discover with every extra, source home pages with a hero carousel.
  - **Addons and extensions:** addon management (details, configure, reorder); extension filters and auto-updates.
- M54 **Security, data, statistics** (mihon §9–12; cloudstream §10):
  - **Security:** app lock, secure screen, incognito.
  - **Backups:** options, automatic backups and their location.
  - **Data:** storage usage and clearing; statistics.
  - **Network:** DNS over HTTPS, user agent, proxy and per-site delay.
  - **Logs:** crash logs and verbose logging.
- M55 WebView resolver (stream sniffing + Cloudflare cookie handoff) for sources that need a browser

## Phase 7 — AI on the phone
- M56 Upscale: in-process ncnn (Vulkan) on Android, priority queue with generations, enhanced-page cache, reader toggle
- M57 OCR on the phone (on-device model via `ort`), bubble merging
- M58 Translation overlay: chapter batching with glossary through the M34 translators; auto-translate on download; novel and lyrics translation

## Phase 8 — Accounts and the rest
- M59 Trackers: AniList, MyAnimeList, Kitsu, MangaUpdates, Trakt, Simkl (needs the user's accounts)
- M60 Onboarding, what's new, update check, deep links, Android-dex runtime for Mihon/Aniyomi APK extensions
- On hold: M22 device sync v2. Later, when the user asks: desktop (Windows first), iOS, sync beyond the LAN.

---

## Resolved decisions (2026-10-30)
1. License: **AGPL-3.0-only**.
2. Extension runtime: **QuickJS** primary, plus a Nuclear plugin compatibility layer with documented limitations (ARCHITECTURE §7.4).
3. Platform order: **Android first** (Pixel_4 emulator), then Windows. *2026-11-01: mobile only until the app is great; no desktop-only designs (CLI sidecars, libmpv windows) until then.*
4. Identifier: **`app.raygun.player`**.
5. New requirement: **automatic LAN device sync**, configurable and hands-off (ARCHITECTURE §10, M21–M22).
