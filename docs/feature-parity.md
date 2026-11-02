# Feature parity checklist

Every user-facing feature of the reference apps in `reference/`, grouped by area. Each row gives its source and where Raygun stands. It was built on 2026-11-01 from the apps' screens and settings strings (Mihon/aniyomi `moko-resources`, CloudStream fragments, Nuclear views, RiPlay screens, Shosetsu views).

Status: ✅ done · 🔜 planned (milestone) · ⏸ on hold or needs the user's accounts · ✖ won't do (reason given)

**Full gap lists:** this table keeps one row per feature family. The per-app research (2026-11-01) in `docs/research/` lists every screen, setting and behaviour the reference apps have that Raygun lacks, with file paths: `mihon.md`, `aniyomi.md`, `cloudstream.md`, `stremio.md`, `shosetsu.md`, `music.md`. The milestones M44–M54 in `docs/ROADMAP.md` work through them.

## App-wide
| Feature | From | Status |
|---|---|---|
| Themes (system/light/dark/pure black) | Mihon, Nuclear | ✅ M04 |
| App language picker, community translations | Mihon, Nuclear | ✅ M35 |
| NSFW / content-warning filter (safe, mixed, 18+), applied to installed extensions too | Mihon | ✅ M35 (one switch) |
| Date/time format, relative timestamps | Mihon | 🔜 M54 |
| Tablet / side-rail layout | Mihon, Nuclear | ✅ M04 |
| Onboarding guide, what's new, update check | Mihon, Nuclear, CloudStream | 🔜 M60 |
| App lock (biometric), lock when idle, secure screen, hide notification content | Mihon | 🔜 M54 |
| Incognito mode (no history) | Mihon | 🔜 M48 (history) / M54 |
| Backup/restore (own format), Mihon backup import | Mihon | ✅ M30 |
| Automatic backups on a schedule, backup location | Mihon | 🔜 M54 |
| Storage usage, clear caches, clear database of non-library entries | Mihon | 🔜 M49 (downloads) / M54 |
| DNS over HTTPS, custom user agent, clear cookies/WebView data | Mihon | ✅ M46.11 (default user agent; cookies and data cleared per source; the browser's data in Settings → Websites since M46.3) / 🔜 M54 (DNS over HTTPS) |
| Crash logs, verbose logging, debug info | Mihon, Nuclear (logs view) | 🔜 M54 |
| Searchable settings | Mihon | 🔜 M54 |
| MCP server, LAN remote | Nuclear | ✅ M19, M20 |
| Device sync | — | ⏸ M22 |
| Keyboard shortcuts | Nuclear | ✅ M46.8 (playback; everywhere: queued) |

## Extensions and repositories
| Feature | From | Status |
|---|---|---|
| Install/update/uninstall, per-extension settings | all | ✅ M13 |
| Several repositories ("extension stores"), add by URL | Mihon, CloudStream, Shosetsu | ✅ M13 |
| Suggested repositories, browse with search and filters (kind, language, repository, rating) | Mihon, CloudStream | ✅ M36 |
| Hide NSFW extensions; content rating shown | Mihon | ✅ M35 |
| Nuclear plugins | Nuclear | ✅ M17 |
| Shosetsu Lua scripts | Shosetsu | ✅ M32 (every script that works in Shosetsu: M46.9.1) |
| Fixed copies of broken sources (Paraline Raygun) | — | ✅ M46.9.1 |
| Charts: popular and trending, by country and genre | Spotify, Nuclear (dashboard) | ✅ M46.9 |
| Genres and moods on songs, Browse all tiles, genre pages | Spotify, Nuclear (tags) | ✅ M46.9 |
| Desktop top bar: menu, Back/Forward, Home, search, Browse, window buttons; zoom | Spotify desktop | ✅ M46.9 |
| Search ranked across sources (source named on each result), grouped by source on demand; failing sources last | Mihon (global search), Spotify | ✅ M46.11 |
| Stremio addons | Stremio | ✅ M41 |
| LNReader JS plugins | LNReader | 🔜 M51 |
| Mihon/Aniyomi APK extensions | Mihon, aniyomi | 🔜 M60 (Android dex runtime) |
| CloudStream `.cs3` plugins | CloudStream | ✖ dex plus a large Kotlin API surface; `video.source` covers the use |
| Extension hot reload for developers | — | ✅ M12 |

## Music (Nuclear, RiPlay)
| Feature | From | Status |
|---|---|---|
| Local library (artists/albums/tracks), online search, streaming providers with fallback | Nuclear, RiPlay | ✅ M07, M14 |
| Queue, repeat/shuffle, media session, lock-screen controls | all | ✅ M05, M09 |
| Playlists, favorites, history | Nuclear, RiPlay | ✅ M18 |
| Synced lyrics | Nuclear | ✅ M15 |
| Furigana/romaji, word-by-word sync, lyrics offset and sources | Nuclear, RiPlay | 🔜 M52 (was marked done; never built) |
| Internet radio | — | ✅ (Radio Browser) |
| **Podcasts**: search, subscribe, episodes, resume, downloads | RiPlay | ✅ M37 |
| Song downloads for offline play; save as the original file, WAV or FLAC with tags; recording when only YouTube has a song | Spotify, Nuclear, RiPlay | ✅ M46.10 (recording on Windows; Android: queued) |
| Music search by songs, artists and albums; album pages | Spotify, Nuclear | ✅ M46.11 |
| Swipe the player bar to change songs; Connect (devices); a miniplayer window on top | Spotify | ✅ M46.13 (Connect shows synced devices; there's no casting) |
| Cards under the player (Lyrics, About the artist, Credits); artist bios; rows that page with buttons | Spotify | ✅ M46.14 (MusicBrainz and Wikipedia; similar artists and samples not yet) |
| Connect: play on another device, "Playing on …", remote control, play here again | Spotify | ✅ M46.15 (between Raygun devices on one network; no Chromecast or DLNA) |
| Equalizer | RiPlay | ✅ M46.12 (10 fixed bands with presets on both platforms; bass boost, reverb, balance, mono) |
| Crossfade, loudness normalization, per-song start/end trims and volume | Spotify, RiPlay, Nuclear (setting only) | ✅ M46.12 (true crossfade with two players; loudness measured as songs play) |
| Sleep timer | RiPlay | 🔜 M52 |
| Blacklist (artists/tracks) | RiPlay | 🔜 M52 |
| Listening stats, "Rewind" yearly recap | RiPlay | 🔜 M52 |
| Playlist import (from URLs/files) | Nuclear | 🔜 M52 |
| Dashboard/discovery: new releases, moods, quick picks | Nuclear, RiPlay | 🔜 M52 |
| Lyrics translation | RiPlay | 🔜 M58 |
| Last.fm scrobbling | RiPlay | 🔜 M52 (own tokens) / ⏸ M59 (accounts) |
| WebDAV database backup | RiPlay | 🔜 M54 |
| Themes marketplace | Nuclear | 🔜 M52 |
| Cast (Chromecast) | RiPlay, CloudStream | 🔜 M52 |
| MPD server | Nuclear | ✖ desktop only, later |

## Manga (Mihon, aniyomi, TachiyomiAT, mihon_img_upscale)
| Feature | From | Status |
|---|---|---|
| Sources: popular/latest/search with filters, global search | Mihon | ✅ M25, M40 (filters, global search), M46.4 (sources picked with chips, several at once; popular titles before anything is typed) |
| Library with in-library toggle | Mihon | ✅ M24 |
| Categories, per-category sort/filter/display, default category | Mihon | ✅ M39 (categories), M46 (per-category layout and sort, update one category) |
| Library filters (downloaded, unread, started, completed, tracked), sort, badges, display modes | Mihon | ✅ M39 (filters, sort, tags), M46 (tri-state filters, nine sorts, search language, four layouts, columns per orientation, selection with bulk actions, continue buttons, random title; tracked: M59) |
| Duplicate detection, migration to another source | Mihon | ✅ M46 (duplicates) / 🔜 M53 (migration) |
| Title notes, custom cover, edit info | Mihon | 🔜 M47 |
| Stats | Mihon | 🔜 M54 |
| Reader: paged LTR/RTL, webtoon, zoom, tap zones, prefetch | Mihon | ✅ M27 |
| Reader: vertical paged, continuous vertical, two-page spreads | Mihon | 🔜 M50 (was marked done; M27 left them out) |
| Reader settings: scale types, zoom start, crop borders, split/rotate wide pages, tap-zone layouts and invert, volume keys, background colour, rotation lock, keep screen on, page number, e-ink flash, double-tap zoom, side padding, chapter transitions, skip read/filtered/duplicate chapters | Mihon | 🔜 M50 |
| Page actions: save, share, set as cover | Mihon | ✅ M46.1 (save to Pictures/Raygun[/title], share, copy) / 🔜 M47 (set as cover) |
| Per-series reading mode and orientation | Mihon | ✅ M27 (mode) / 🔜 M50 (orientation) |
| Updates feed, library update schedule | Mihon | ✅ M28 |
| Update restrictions (Wi-Fi, charging), skip completed/unread/unstarted, smart update | Mihon | 🔜 M48 |
| Downloads queue | Mihon | ✅ M28 |
| CBZ, Wi-Fi only, delete after reading, download ahead, auto-download new chapters, concurrency | Mihon | ✅ M46.1 (downloads also saved as CBZ or images to Downloads or a picked folder; retries with backoff and Retry-After; resumed chapters) / 🔜 M49 (the rest) |
| Export titles as CBZ, PDF, image folders, EPUB, text or video files, waiting for downloads first | (none of the reference apps) | ✅ M46.1 |
| Websites the user adds, in a built-in browser with an ad, tracker, popup and redirect blocker | CloudStream (WebView), browsers | ✅ M46.3 |
| Video on desktop: HLS and DASH with headers, subtitles, tracks and quality, sessions, keys, full screen, mpv/VLC for other codecs | Stremio desktop, CloudStream | ✅ M46.4 |
| Desktop: exports to user folders, page copy and Show in folder, read-aloud, websites browser with blocker | (desktop apps) | ✅ M46.6 |
| Trailers in their own full-window player | Stremio, CloudStream | ✅ M46.4 |
| Player bar with shuffle, repeat, seek, volume, lyrics and a queue panel | Spotify, Nuclear | ✅ M46.4 |
| Now playing side panel (art, lyrics, next up, queue), scrolling titles, a full player laid out side by side with its own settings | Spotify | ✅ M46.7 |
| Any accent colour, cover sizes, reader page fit and zoom, a page bar to arrange, pages to load ahead, scrollbars and animations as settings | Mihon (reader settings), Aniyomi | ✅ M46.7 |
| YouTube search without an API key | Grayjay, NewPipe, FreeTube | ✅ M46.7 (search through YouTube Music's web API; playback in YouTube's embed) |
| One-click and bulk extension installs, update all | Mihon, Aniyomi | ✅ M46.7 |
| Song tables: sort, filter, compact/list, columns to show, reorder and resize, multi-select | Spotify | ✅ M46.8 |
| Save with a Plus into Favorites and playlists, made on the spot | Spotify | ✅ M46.8 |
| Artist pages from any credit | Spotify, Nuclear | ✅ M46.8 (bios and facts: M46.9) |
| Right-click menus everywhere, themed | Spotify desktop | ✅ M46.8 |
| Resizable rail and panels, playlists in the rail | Spotify desktop | ✅ M46.8 |
| Play next, queue drag, save queue, sleep timer, speed, shortcuts, media keys | Spotify, Nuclear, RiPlay | ✅ M46.8 |
| Turn sources off from Browse, auto-loading pages | Mihon | ✅ M46.8 |
| Arrange tabs (add, hide, reorder, start screen), library tabs, Home, Browse and More | Aniyomi (NavStyle, start screen), Mihon (category order) | ✅ M46.5 (pinned sources: M53) |
| Cover cache kept for the library, cache sizes and clearing, extension icons | Mihon, CloudStream, RiPlay | ✅ M46.2 (separate cover and page caches with limits, background revalidation, Storage screen, repository icons) |
| Chapter swipe actions, scanlator filter, bookmarks | Mihon | 🔜 M47 |
| Trackers (AniList, MAL, Kitsu, MangaUpdates, Bangumi, Shikimori, Komga/Kavita) | Mihon | ⏸ M59 (accounts) |
| Local source (CBZ/folders) | Mihon | ✅ M46 (CBZ/ZIP with ComicInfo; folders and RAR ✖, Android's picker gives files) |
| AI upscaling | mihon_img_upscale | 🔜 M56 |
| OCR + translation overlay, auto-translate downloads | TachiyomiAT | 🔜 M57, M58 |
| WebView (open in browser, Cloudflare) | Mihon | 🔜 M55 |

## Novels (Shosetsu, LNReader)
| Feature | From | Status |
|---|---|---|
| Sources, library, reader (reflow, typography, scroll/paged) | Shosetsu | ✅ M31 |
| Shosetsu repositories | Shosetsu | ✅ M32 |
| EPUB import, chapter downloads, text-to-speech | Shosetsu, LNReader | ✅ M33 |
| Reader themes, custom CSS editor | Shosetsu | 🔜 M51 |
| Custom download ranges | Shosetsu | 🔜 M47 |
| Novel translation | — | 🔜 M58 |
| LNReader plugins | LNReader | 🔜 M51 |

## Watch: movies, TV, anime (CloudStream, aniyomi, Stremio)
| Feature | From | Status |
|---|---|---|
| **Watch tab**, home with catalog rows | CloudStream, Stremio | ✅ M38 (tab), M41 (catalogs) |
| Native player: gestures (seek/volume/brightness), double-tap skip, PiP, speed, orientation, resume | aniyomi, CloudStream | ✅ M38 |
| Subtitles (.srt/.vtt/.ass), preferred languages, audio track selection | aniyomi, CloudStream | ✅ M38 (preferred languages later) |
| HLS/DASH, header injection | CloudStream | ✅ M38 |
| Intro/outro skip (chapters, AniSkip) | aniyomi | ✅ M45 (source chapters, skip button with countdown or auto-skip, per-title intro length, AniSkip by MAL id or AniList search) |
| Mark seen at a percentage, preserve position | aniyomi | ✅ M42.1 (watched point setting), M43 (counts on library episodes) |
| Stremio catalogs, metadata, streams, subtitles | Stremio | ✅ M41 (torrent streams not yet) |
| Video sources with servers streamed as they resolve | CloudStream, aniyomi | ✅ M42 (`video.source`, Internet Archive films) |
| Source priority, quality profiles, failed/empty hosters | CloudStream, aniyomi | ✅ M42 (preferred quality, failed/empty servers) / 🔜 M44 (fallback, last server), M45 (profiles) |
| Video library, history, continue watching, upcoming episodes | aniyomi, CloudStream | ✅ M43 (sources and Stremio titles, categories, filters, updates, upcoming) |
| Video downloads | aniyomi, CloudStream | ✅ M43 (plain files with resume, HLS with AES-128; DASH not yet; foreground only) |
| Play at once with automatic fallback; sources switched inside the player | aniyomi, CloudStream | 🔜 M44 |
| Previous/next episode, autoplay next with a countdown, episode list in the player | aniyomi, CloudStream, Stremio | 🔜 M44 |
| Continue watching moves on to the next episode; last stream per series | Stremio | 🔜 M44 |
| Custom player overlay: lock, aspect, rotation, speed presets, brightness/volume bars and boost, horizontal seek, configurable double-tap | aniyomi, CloudStream | ✅ M45 (plus hold for speed, media keys) |
| Subtitle styling, delay, second track, local files; audio and subtitle language lists; tracks and speed remembered per title | aniyomi, Stremio | ✅ M45 (second track = subtitle files; audio delay ✖) |
| PiP actions, sleep timer, screenshots, decoder switch | aniyomi | ✅ M45 (screenshots as cover: M47) |
| Watch-status lists (Watching, Plan to watch, Completed, On hold, Dropped), favorites | CloudStream | ✅ M46 (for manga and novels too; tabs by list; Shosetsu's pins) |
| Episode filters, sort, bookmarks, fillers, selection and bulk actions | aniyomi | 🔜 M47 |
| New-episode notifications, per-title toggle, calendar | CloudStream, Stremio | 🔜 M48 |
| Background downloads with notifications, parallel, Wi-Fi only, auto-download, delete after watching | aniyomi, CloudStream | 🔜 M49 |
| Discover with every addon filter, search history, source home pages | Stremio, CloudStream | 🔜 M53 |
| WebView resolver | CloudStream | 🔜 M55 |
| Torrent streaming (TorrServer) | aniyomi | ✖ for now (legal risk; not in first-party code) |
| External player | aniyomi | ✅ M45 (chooser or a set app; position read back) |
| mpv scripts, custom Lua buttons, decoder tweaks | aniyomi | ✖ ExoPlayer instead of mpv on Android |
| Chromecast | CloudStream | 🔜 M52 |
| Trakt, Simkl, AniList, MAL | CloudStream, aniyomi | ⏸ M59 (accounts) |
