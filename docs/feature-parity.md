# Feature parity checklist

Every user-facing feature of the reference apps in `reference/`, grouped by area. Each row gives its source and where Raygun stands. It was built on 2026-11-01 from the apps' screens and settings strings (Mihon/aniyomi `moko-resources`, CloudStream fragments, Nuclear views, RiPlay screens, Shosetsu views).

Status: ✅ done · 🔜 planned (milestone) · ⏸ on hold or needs the user's accounts · ✖ won't do (reason given)

## App-wide
| Feature | From | Status |
|---|---|---|
| Themes (system/light/dark/pure black) | Mihon, Nuclear | ✅ M04 |
| App language picker, community translations | Mihon, Nuclear | ✅ M35 |
| NSFW / content-warning filter (safe, mixed, 18+), applied to installed extensions too | Mihon | ✅ M35 (one switch) |
| Date/time format, relative timestamps | Mihon | 🔜 M49 |
| Tablet / side-rail layout | Mihon, Nuclear | ✅ M04 |
| Onboarding guide, what's new, update check | Mihon, Nuclear, CloudStream | 🔜 M53 |
| App lock (biometric), lock when idle, secure screen, hide notification content | Mihon | 🔜 M49 |
| Incognito mode (no history) | Mihon | 🔜 M49 |
| Backup/restore (own format), Mihon backup import | Mihon | ✅ M30 |
| Automatic backups on a schedule, backup location | Mihon | 🔜 M48 |
| Storage usage, clear caches, clear database of non-library entries | Mihon | 🔜 M48 |
| DNS over HTTPS, custom user agent, clear cookies/WebView data | Mihon | 🔜 M49 |
| Crash logs, verbose logging, debug info | Mihon, Nuclear (logs view) | 🔜 M49 |
| Searchable settings | Mihon | 🔜 M49 |
| MCP server, LAN remote | Nuclear | ✅ M19, M20 |
| Device sync | — | ⏸ M22 |
| Keyboard shortcuts | Nuclear | ✖ desktop only, later |

## Extensions and repositories
| Feature | From | Status |
|---|---|---|
| Install/update/uninstall, per-extension settings | all | ✅ M13 |
| Several repositories ("extension stores"), add by URL | Mihon, CloudStream, Shosetsu | ✅ M13 |
| Suggested repositories, browse with search and filters (kind, language, repository, rating) | Mihon, CloudStream | ✅ M36 |
| Hide NSFW extensions; content rating shown | Mihon | ✅ M35 |
| Nuclear plugins | Nuclear | ✅ M17 |
| Shosetsu Lua scripts | Shosetsu | ✅ M32 |
| Stremio addons | Stremio | 🔜 M39 |
| LNReader JS plugins | LNReader | 🔜 M51 |
| Mihon/Aniyomi APK extensions | Mihon, aniyomi | 🔜 M53 (Android dex runtime) |
| CloudStream `.cs3` plugins | CloudStream | ✖ dex plus a large Kotlin API surface; `video.source` covers the use |
| Extension hot reload for developers | — | ✅ M12 |

## Music (Nuclear, RiPlay)
| Feature | From | Status |
|---|---|---|
| Local library (artists/albums/tracks), online search, streaming providers with fallback | Nuclear, RiPlay | ✅ M07, M14 |
| Queue, repeat/shuffle, media session, lock-screen controls | all | ✅ M05, M09 |
| Playlists, favorites, history | Nuclear, RiPlay | ✅ M18 |
| Synced lyrics, furigana/romaji | Nuclear | ✅ M23 |
| Internet radio | — | ✅ (Radio Browser) |
| **Podcasts**: search, subscribe, episodes, resume, downloads | RiPlay | ✅ M37 |
| Equalizer | RiPlay | 🔜 M50 |
| Sleep timer | RiPlay | 🔜 M50 |
| Blacklist (artists/tracks) | RiPlay | 🔜 M50 |
| Listening stats, "Rewind" yearly recap | RiPlay | 🔜 M50 |
| Playlist import (from URLs/files) | Nuclear | 🔜 M50 |
| Dashboard/discovery: new releases, moods, quick picks | Nuclear, RiPlay | 🔜 M50 |
| Lyrics translation | RiPlay | 🔜 M45 |
| Last.fm scrobbling | RiPlay | ⏸ M52 (account) |
| WebDAV database backup | RiPlay | 🔜 M48 |
| Themes marketplace | Nuclear | 🔜 M50 |
| Cast (Chromecast) | RiPlay, CloudStream | 🔜 M50 |
| MPD server | Nuclear | ✖ desktop only, later |

## Manga (Mihon, aniyomi, TachiyomiAT, mihon_img_upscale)
| Feature | From | Status |
|---|---|---|
| Sources: popular/latest/search with filters, global search | Mihon | ✅ M25 / 🔜 M46 global search |
| Library with in-library toggle | Mihon | ✅ M24 |
| Categories, per-category sort/filter/display, default category | Mihon | 🔜 M46 |
| Library filters (downloaded, unread, started, completed, tracked), sort, badges, display modes | Mihon | 🔜 M46 |
| Duplicate detection, migration to another source | Mihon | 🔜 M46 |
| Title notes, custom cover, edit info | Mihon | 🔜 M46 |
| Stats | Mihon | 🔜 M46 |
| Reader: paged LTR/RTL, vertical, webtoon, spreads, zoom, tap zones, prefetch | Mihon | ✅ M27 |
| Reader settings: scale types, zoom start, crop borders, split/rotate wide pages, tap-zone layouts and invert, volume keys, background colour, rotation lock, keep screen on, page number, e-ink flash, double-tap zoom, side padding, chapter transitions, skip read/filtered/duplicate chapters | Mihon | 🔜 M47 |
| Page actions: save, share, set as cover | Mihon | 🔜 M47 |
| Per-series reading mode and orientation | Mihon | ✅ M27 (mode) / 🔜 M47 (orientation) |
| Updates feed, library update schedule | Mihon | ✅ M28 |
| Update restrictions (Wi-Fi, charging), skip completed/unread/unstarted, smart update | Mihon | 🔜 M48 |
| Downloads queue | Mihon | ✅ M28 |
| CBZ, Wi-Fi only, delete after reading, download ahead, auto-download new chapters, concurrency | Mihon | 🔜 M48 |
| Chapter swipe actions, scanlator filter, bookmarks | Mihon | 🔜 M47 |
| Trackers (AniList, MAL, Kitsu, MangaUpdates, Bangumi, Shikimori, Komga/Kavita) | Mihon | ⏸ M52 (accounts) |
| Local source (CBZ/folders) | Mihon | 🔜 M46 |
| AI upscaling | mihon_img_upscale | 🔜 M43 |
| OCR + translation overlay, auto-translate downloads | TachiyomiAT | 🔜 M44, M45 |
| WebView (open in browser, Cloudflare) | Mihon | 🔜 M42 |

## Novels (Shosetsu, LNReader)
| Feature | From | Status |
|---|---|---|
| Sources, library, reader (reflow, typography, scroll/paged) | Shosetsu | ✅ M31 |
| Shosetsu repositories | Shosetsu | ✅ M32 |
| EPUB import, chapter downloads, text-to-speech | Shosetsu, LNReader | ✅ M33 |
| Reader themes, custom CSS editor | Shosetsu | 🔜 M51 |
| Custom download ranges | Shosetsu | 🔜 M51 |
| Novel translation | — | 🔜 M45 |
| LNReader plugins | LNReader | 🔜 M51 |

## Watch: movies, TV, anime (CloudStream, aniyomi, Stremio)
| Feature | From | Status |
|---|---|---|
| **Watch tab**, home with catalog rows | CloudStream, Stremio | 🔜 M38, M39 |
| Native player: gestures (seek/volume/brightness), double-tap skip, PiP, speed, orientation, resume | aniyomi, CloudStream | 🔜 M38 |
| Subtitles (.srt/.vtt/.ass), preferred languages, audio track selection | aniyomi, CloudStream | 🔜 M38 |
| HLS/DASH, header injection | CloudStream | 🔜 M38 |
| Intro/outro skip (chapters, AniSkip) | aniyomi | 🔜 M38 (chapters), ⏸ AniSkip (needs a tracker) |
| Mark seen at a percentage, preserve position | aniyomi | 🔜 M41 |
| Stremio catalogs, metadata, streams, subtitles | Stremio | 🔜 M39 |
| Video sources with servers streamed as they resolve | CloudStream, aniyomi | 🔜 M40 |
| Source priority, quality profiles, failed/empty hosters | CloudStream, aniyomi | 🔜 M40 |
| Video library, history, continue watching, upcoming episodes | aniyomi, CloudStream | 🔜 M41 |
| Video downloads | aniyomi, CloudStream | 🔜 M41 |
| WebView resolver | CloudStream | 🔜 M42 |
| Torrent streaming (TorrServer) | aniyomi | ✖ for now (legal risk; not in first-party code) |
| External player | aniyomi | 🔜 M38 |
| mpv scripts, custom Lua buttons, decoder tweaks | aniyomi | ✖ ExoPlayer instead of mpv on Android |
| Chromecast | CloudStream | 🔜 M50 |
| Trakt, Simkl, AniList, MAL | CloudStream, aniyomi | ⏸ M52 (accounts) |
