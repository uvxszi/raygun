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
- M46.5 **Customizable layout** (asked 2026-11-01: "make it very customizable, so you can add and move around tabs and options and such, similar to mihon"): navigation tabs added, hidden and reordered (five on phones, the rest in More; any of them as the start screen), library tabs, Audio's chips, Home's sections, Browse's tabs and sections, and More's entries, edited by dragging or with buttons in Settings → Appearance → Layout. Pinned sources follow in M53.
- M46.6 **Desktop: saving, sharing, read-aloud and websites** (asked 2026-11-01: "yes desktop gaps"): exports to the user's folders and a picked folder, Show in folder and copying pages, read-aloud with the system's voices, and the websites browser in windows of its own with the blocker; readers sized for wide windows.
- M46.7 **Customization** (asked 2026-11-01): any accent colour; cover sizes (smaller by default on desktop, a setting); the manga reader's page fit, webtoon width, zoom (Ctrl+scroll, Ctrl±, buttons), pages to load ahead and a narrower page bar with chosen buttons; novels zoom with Ctrl+scroll; F11 full screen; hover labels on icon buttons; Search's catalog rows named; LRCLIB without an artist; long titles scroll; a Now playing side panel on desktop (art, lyrics, queue); YouTube search without an API key; a playhead that follows the audio; a wider full player with its own layout settings and Up next as a view; popovers beside buttons instead of full-window menus; one-click and bulk extension installs; scrollbars and animations as settings; no crash on script errors on Windows (release builds unwind); searches and addon requests a few at a time, rows loading as they're seen; the new app icon.
- M46.8 **Music for power users** (asked 2026-11-02): song tables everywhere (sort, compact/list, columns shown, hidden, reordered and resized, filter, play in the shown order); a Plus that saves to Favorites and opens Spotify's Add to playlist popover; artist links and artist pages; resizable rail, side panel and full player, with playlists in the rail; a clickable video in the side panel; Play next, Add to queue and queue reordering; multi-select and right-click menus; shortcuts, media keys and the system media overlay; a sleep timer and playback speed; song details.
- M46.9 **Music discovery and tags** (asked 2026-11-02):
  - popular and trending music from open charts (Deezer charts, Apple's top songs feed, ListenBrainz's most played, YouTube Music's charts), as Home and Browse rows;
  - songs tagged by genre and mood like Spotify's (Rock, Rap, Jazz, Breakcore, Energetic, Chill…) from MusicBrainz and Deezer genres mapped to moods;
  - tag chips that filter your songs; Browse → Music by tag.
  - Added: Spotify's desktop top bar (⋯ menu with File, Edit, View, Playback and Help, Back and Forward, Home, search with Browse, the window's buttons) and Browse all's tiles.
- M46.9.1 **Sources that work** (asked 2026-11-02): fix the sources and converters that fail where they worked in their own apps (Shosetsu scripts, Nuclear plugins); fixed copies of sources for legitimate sites go to the separate **Paraline Raygun** repository (`uvxszi/paraline-raygun`, published with the user's go-ahead of 2026-11-02). ✅ Shosetsu hashes, TLS and DOM gaps; Nuclear hosts, kinds, page stubs and re-offered conversions; Paraline with Syosetsu and Wattpad.
- M46.10 **Music downloads**, then everything downloadable (asked 2026-11-02):
  - songs downloaded into the app to play offline, from the stream a source gives;
  - exported as files (as downloaded, WAV or FLAC) to Music or a chosen folder;
  - when no source gives a file (YouTube's embed), recording what plays: the audio element on desktop, the app's own sound through WASAPI process loopback on Windows, AudioPlaybackCapture on Android.
  - ✅ Downloads → Music, offline playback by the song's key, files as downloaded, WAV or FLAC with tags, recording on Windows. Android recording (AudioPlaybackCapture) is queued.
- M46.11 **Search and sources, and window polish** (asked 2026-11-02):
  - search results ranked across sources with each source named, "By source" to group them again; music search by songs, artists and albums, with album pages;
  - a finder for source chips; failing sources last (a setting, and Browse's ⋮ menu);
  - desktop panels that shrink instead of disappearing, a full player sized by the window, live column dividers, placeholders the size of the covers, paragraph indents for novels;
  - a default User-Agent setting, cookies kept per source with "Clear cookies and data", a source's website in Raygun's browser, one running copy on desktop.
  - Not built: solving captchas or Cloudflare checks for sources, automatically or by moving the browser's cookies into a source; Royal Road, Scribble Hub and Webnovel block Raygun's requests, so Paraline has no copies of them.
- M46.12 **Sound and lyrics editing** (asked 2026-11-01, started 2026-11-02): ✅ part 1, the sound: a 10-band equalizer with presets, bass boost, reverb, balance, mono, loudness normalization learned per song, crossfade, and each song's trims and volume, on desktop (Web Audio) and Android (Media3 processors). Part 2: the lyrics editor and subtitle cues.
- M46.13 **The phone's player bar, Connect, and the desktop miniplayer** (asked 2026-11-02): swipe the phone's bar to change songs, Connect beside the song (devices that sync), and Spotify's miniplayer window on desktop.
- M46.14 **Rows that page, and the phone's full player** (asked 2026-11-02): ✅
  - rows with side buttons and Shift+wheel, and no scrollbars under rows, chips or tabs;
  - the phone's player with an "Up next" pill under the cover and its controls higher up, then Lyrics, About the artist and Credits cards;
  - artist facts and credits through the new `music.info` capability (MusicBrainz with Wikipedia), also shown on artist pages.
- M46.15 **Connect: play on another device** (asked 2026-11-02): ✅
  - Raygun devices find each other (mDNS, or an address) and ask once on the device being played on.
  - The queue and the position move there, and this device shows "Playing on …" and controls it. "Play here" takes the queue back.
- M46.16 **A player you can trust, and polish around it** (asked 2026-11-02): ✅
  - Pause always holds and play always answers; a stream that doesn't start is skipped.
  - The playing mark in lists; reopening on the last song, paused; desktop sites reopen on their last page.
  - No mouse text selection, with right-click copy and selectable lyrics; the miniplayer opens in the app's colours and its text scales.
  - "Keep music playing with other media"; artist and credits cards in the side panel; podcast menus; Discord presence.
- M46.17 **Calm source failures, a test pack, and video sources that learn** (asked 2026-11-02): ✅
  - "Issue with source" cards with More info replace red errors.
  - A developer test pack: broken-on-purpose extensions, legal test films and channels, every known repository.
  - Video sources that failed lately are tried last and marked; a silent source no longer stalls searches (12 s).
- M46.18 **Audio quality and downloads** (asked 2026-11-02): ✅
  - Streaming, mobile data and download quality settings that steer which stream plays (nearest bitrate among equal matches).
  - A download button beside + with an attached confirm popover; Library → Audio → Downloaded.
  - Save as MP3 (LAME), and choose a folder from the save sheet.
- M46.19 **Fuller genres and more kinds of charts** (asked 2026-11-02): ✅
  - Any genre or mood has songs from every source that has some (`tagSongs`), above the user's own.
  - Charts grouped by kind (top, trending, new, underground, decades, genres); Apple genre charts, Deezer decades, ListenBrainz fresh singles.
  - Audius, a new first-party source: trending and underground charts, search and full streams.
- M46.20 **Made for You** (asked 2026-11-02): ✅
  - Daily Mixes, Discover Weekly, Release Radar, On Repeat, a time-of-day mix, Repeat Rewind, "Because you listened to", artist radios, genre and mood mixes, listening numbers.
  - A Home row, a Browse all tile, mix pages with Play, Shuffle and Save as playlist.
  - A new `music.recommend` kind (Deezer: similar artists, top songs, new releases, radio).
- M46.21 **Importing songs** (asked 2026-11-02): ✅
  - Spotify playlist, album and song links (the public embed page), CSV exports and pasted lists.
  - Screenshots of song lists, read by Windows' OCR or ocrs (downloaded once).
  - Matched against the sources (covers, albums, lengths), then saved as a playlist.
- M46.22 **Audiobooks** (asked 2026-11-01): ✅
  - A kind of its own: Library → Audio → Audiobooks with progress, book pages with Resume and chapters in order, and a browse page.
  - LibriVox through the Internet Archive (new `audiobook.directory` kind).
- M46.23 **What Raygun can run** (asked 2026-11-02): ✅
  - A page listing every add-on system and what replaces QuietPlay's and Musike's (neither has add-ons).
  - Nuclear dashboards and discovery plugins now run (as charts and song radios).
- M46.24 **Player and shell polish** (asked 2026-11-02): ✅
  - Cards no longer cut off; a slim lyrics card when there are none; no Show video overlay.
  - Close the player (swipe down on phones, × on desktop); Next in queue grows by rows.
  - Ctrl+scroll zoom and an Interface size on phones; a mascot button ("coming soon").
- M46.25 **Search pills** (asked 2026-11-02): ✅ Music with Relevant, Songs, Artists, Albums and Podcasts (podcasts merged in); Movies & series with Relevant, Movies and Series.
- M46.26 **Song lengths and cover images** (asked 2026-11-02): ✅ lengths learned and looked up for every list; covers shown large and saved as PNG, JPEG or WebP; downloaded songs keep their cover offline.
- M46.27 **Local Files** (asked 2026-11-02): ✅ folders scanned for songs, comics, books and videos alike, on desktop and Android.
- M46.28 **Paraline Community** (asked 2026-11-02): ✅ a local-only test repository of sources, and local repositories in Raygun.
- M46.29 **Library blocks and a sidebar you arrange** (asked 2026-11-02): ✅ Downloaded (blue) and Local files (green) always shown; pin anything to the sidebar, drag it, or sort by recently played or opened; Local Files opens from the Library, not Browse.
- M46.30 **Edit details** (asked 2026-11-02): ✅ your own titles, names and covers for songs, albums and artists, shown everywhere, with Reset.
- M46.31 **Import review** (asked 2026-11-02): ✅ edit, add and reorder songs, compare sources with previews, import while matching continues, keep unavailable songs and find them a source later.
- M46.32 **Spoken word** (asked 2026-11-02): ✅ audiobook authors and podcast shows link to their pages; captions from feeds or transcribed by Whisper on the device or an online provider, in 30-second batches.
- M46.33 **Video player tools** (asked 2026-11-02): ✅ panels that fit and scroll; subtitles found in addons, typed delay, AI subtitles while watching or for a download; volume to 200 %; pick the external player; keys and a sheet listing them.
- M46.34 **Title pages and trailers** (asked 2026-11-02): ✅ content higher on the backdrop; a small trailer plays beside it, paused, stopped or turned off in Settings.
- M46.35 **Browse and search** (asked 2026-11-02): ✅ Comics, Books and Videos pills starting on Relevant; YouTube and PeerTube pills from their extensions; Sources by kind with a name field; search in more lists; Search → All's comics and books ranked across sources.
- M46.36 **Music menus, explicit songs, artists** (asked 2026-11-02): ✅ Remove from downloads, Go to album, one set of copy items; Explicit music apart from 18+; discographies; Discord presence removed; lyrics and captions follow trims, lyrics from any source or heard by AI, captions from the feed or AI; covers from other sources; podcasts as a grid.
- M46.37 **Shell** (asked 2026-11-02): ✅ Updates and Downloads popovers; All tabs in Library, Downloads and Updates; Your music with what was played from lately and the latest favorites; a closable "Nothing to pick up yet"; panels that don't jump; covers that stay shown.
- M46.39 **Playlists and lyrics sources** (asked 2026-11-02): ✅ playlist search with a Featured chip (YouTube Music, Deezer), playlist pages saved to the library, KuGou and YouTube Music lyrics beside LRCLIB, as Kreate and RiMusic have them.
- M46.38 **Comics and books** (asked 2026-11-02): ✅ chapter sort and filters; a working website button; paged novels as narrow as scrolled ones; find a title in other sources; JPEG XL read from sources and saved as covers; other apps' repositories named. The reader's AI switches wait for the upscale and bubble-translation engines (item 8 above).
- **Asked 2026-11-01, queued in this order** (each becomes a milestone with a spec):
  1. **Sound and lyrics editing** (now M46.12):
     - edit lyrics (words and timings) and shift them by an offset; the same for video subtitles;
     - per-song start and end trims and per-song volume;
     - loudness normalization, an equalizer and other sound effects for all songs;
     - crossfade into the next song, adjustable or off;
     - desktop through Web Audio, Android through Media3's audio processors. YouTube's embedded player can only change its volume.
  2. **Artist and song info** (open sources, Spotify's "About the artist" and credits). Mostly ✅ in M46.14: bio, picture, facts, members and credits; similar artists, an online discography, samples and covers are still to do.
     - an online artist page (bio, picture, facts, members, discography, similar artists);
     - song credits and facts (writers, producers, samples, covers);
     - from Wikipedia/Wikidata, MusicBrainz and ListenBrainz, through an extension capability;
     - shown in the Now playing panel and the full player.
  3. **Keyboard and sorting:**
     - keyboard navigation through the whole app, with shortcuts everywhere and no mouse needed;
     - sorting everywhere, including podcasts and their episodes;
     - ratings beside IMDb (Rotten Tomatoes and Metacritic through OMDb, with a key the user pastes in; no scraping);
     - using the spare space on desktop for more buttons and panels.
  4. **Read-aloud:** pause and resume; choosing voices (and their speed and pitch).
  5. **Miru extensions** (M46.8 in the research notes): run Miru's JavaScript extensions and install Miru repositories. Raygun's own repository may hold converted copies of the licensed and public ones.
  6. **Nuvio / NuvioTV support:** what NuvioTV offers beyond Stremio addons (its plugin repositories and their scrapers, run for repositories the user adds; Trakt; its TV layout). Same rule as Miru: no piracy repositories shipped or pre-added. Research in `docs/reference-notes.md`.
  7. **Paraline Raygun:** a separate collection repository. It holds Raygun's own extensions, website presets and links to other repositories, as one thing to add. Neutral and legal content only, created locally first and published with the user's go-ahead.
  8. **AI for manga** (pulls forward M56–M58):
     - upscaling and translation, on and off separately or together, on desktop and phones;
     - translations drawn over the speech bubbles;
     - offline translation models: download from the list or a link, manage, uninstall;
     - a "translate this automatically" path offered when a title isn't in the user's language.
  9. **The manga reader:** two pages side by side (spreads) and the rest of M50.
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
