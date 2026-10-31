# Reference Repo Notes

Condensed findings from the repos in `reference/` (git-ignored, shallow clones, cloned 2026-10-30).
Read this instead of re-exploring the repos. Each section: what to take, what to avoid, where to look.

## License map (decides what we may copy vs. only re-implement)

| Repo | License | Implication for Raygun |
|---|---|---|
| stremio-core | MIT | Can depend on it as a crate / copy freely |
| mihon, aniyomi, TachiyomiAT, mihon_img_upscale | Apache-2.0 | Can copy/adapt with attribution + NOTICE |
| RiPlay, shosetsu, cloudstream | GPL-3.0 | Copying code makes Raygun GPL-3.0 |
| nuclear | AGPL-3.0 | Copying code makes Raygun AGPL-3.0 |

Raygun is **AGPL-3.0-only**, so code from all of these may be adapted. Keep a header naming the source repo and license on adapted files, and list Apache-2.0 sources in `NOTICE`.

---

## nuclear (Tauri v2 + React, pnpm/turbo monorepo) — closest stack match

**Layout:** `packages/player` (app, `src-tauri/` Rust), `plugin-sdk` (published to npm), `model`, `ui`, `themes`, `hifi`, `i18n`.
Frontend stack: React, Tailwind v4, Zustand, TanStack Query v5 + Router, Zod, Vitest/RTL, Lucide, Sonner.

**Plugin system** (`player/src/services/plugins/PluginLoader.ts`):
- Plugin = folder with `package.json` (`nuclear` field: displayName, categories, icon, permissions) + entry file.
- Optionally compiled from TS at load (`pluginCompiler`), then evaluated in the **webview** with `new Function('exports','module','require', code)`. `require` is a whitelist: plugin-sdk, ui, react, jsx-runtime.
- Lifecycle: `onLoad/onUnload/onEnable/onDisable(api)`.
- No real sandbox: plugin code shares the UI's JS realm.

**Domain pattern ("Host" architecture):** for each feature area — `types/<domain>.ts` declares a `XHost` interface → SDK class wraps it → player implements it in `services/<domain>Host.ts` over a Zustand store. Domains: metadata, streaming, lyrics, queue, playback, playlists, favorites, dashboard, discovery, settings, http, ytdlp, shell, events, logger.

**Provider registry** (`plugin-sdk/src/types/providers.ts`): `ProviderDescriptor{id, kind, name, pluginId}`; kinds `metadata | streaming | lyrics | dashboard | playlists | discovery`; one *active* provider per kind.

**Metadata vs. stream separation** (`model/src/streaming.ts`): a `Track` is pure metadata. A `StreamingProvider.searchForTrack(artist,title)` returns `StreamCandidate[]` (`{id,title,durationMs,thumbnail,failed,source,lastResolvedAtIso}`); `getStreamUrl(candidateId)` lazily resolves a `Stream{url, protocol: file|http|https|hls, mimeType, bitrate, codec, ...}`. Candidates are cached on the track and verified (`StreamVerificationStatus`: unverified → weaklyVerified → verified → verifiedByUser).

**Rust side** (`src-tauri/src/`):
- `bridge/` — Rust → frontend RPC: emits `bridge:request`, frontend answers via `bridge_respond` command. Needed because domain state lives in the frontend.
- `mcp/` — `rmcp` streamable-HTTP server on `127.0.0.1:8800-8809/mcp`. Only **4 meta-tools**: `list_methods`, `method_details`, `describe_type`, `call`. Keeps the agent's tool list tiny while exposing the whole SDK surface. Calls are forwarded through the bridge.
- `http_api/` — Axum REST + SSE for "Nuclear Jam" LAN remote (the remote UI is the Vite app bound to 0.0.0.0).
- `mpd/` — MPD protocol server (ncmpcpp etc.).
- `stream_server.rs` — loopback proxy `/<base64url(url)>` adding CORS + Range passthrough so `<audio>` can play streams that block browsers.
- `http.rs` — `http_fetch` command: CORS-free fetch for the frontend/plugins.
- `ytdlp.rs` / `ytdlp_setup.rs` / `ytdlp_verify.rs` — auto-downloaded yt-dlp binary, signature-verified (pgp).
- `db.rs` — sqlx SQLite; `tauri-specta` generates TS bindings for commands.
- `profile.rs` — `--profile <name>` changes the app identifier → separate data dir.

**Not portable:** uses `app_data_dir()` and `tauri-plugin-store`; webview data goes to the OS profile.

**Furigana:** not in the core player (lives in a lyrics plugin); only lyrics view tests reference it.

---

## RiPlay (Kotlin Multiplatform: Android + Compose Desktop)

**HybridPlayer** (`androidApp/.../services/playback/HybridPlayer.kt`): a Media3 `ForwardingPlayer` wrapping **two engines** — ExoPlayer (local/direct) and a YouTube IFrame player in a WebView (`YouTubeControl` interface: play/pause/seek/position/duration/volume/rate). `activeEngine` switches per item; when YouTube is active it *synthesizes* Media3 events (timeline/play-state/position every 200 ms) so the OS media session, notification and Android Auto keep working. Queue is mixed audio/video.

**YouTube embed origin rules** (`composeApp/src/jvmMain/.../kcef/PlayerHtmlServer.kt`, verified by the author Oct 2026): the embed decision is based on the **Referer string**:
- no Referer → error 152/153; `https://www.youtube.com/` → unavailable; `http://127.0.0.1:PORT/` → unplayable (150/152)
- `http://localhost:PORT/` → **OK**; `https://youtube.com/` (apex) → OK; normal third-party domains → OK
- Desktop fix: serve the player HTML from a loopback server and load it as `http://localhost:PORT/player.html`. JS bridge functions (`riplayLoad/Play/Pause/SeekTo/SetVolume/SetRate/State`) polled every 250 ms; watchdog re-sends load if the player is stuck.
- Android: `loadDataWithBaseURL("https://youtube.com")`.

**Other modules:** `modules/environment` (InnerTube client models/requests), `modules/ayp` (android-youtube-player fork + `ayp_youtube_player.html`), cast module. Features worth copying conceptually: blacklist, listening stats/"rewind", sleep timer, lyrics translate.

---

## shosetsu (Android, Kotlin) — light novels via Lua extensions

- Extensions are **Lua scripts** run with **LuaJ (Lua 5.2)**; the API lives in the separate `kotlin-lib` (`app.shosetsu.lib`), which exposes globals to scripts (HTTP `GET/POST`, jsoup-backed `Document`/selectors, JSON, etc.).
- Extension contract (`IExtension`): `id, name, baseURL, imageURL, listings[], startIndex, chapterType, hasSearch, settings, searchFilters`, functions `parseNovel(url, loadChapters)`, `getPassage(chapterURL)`, `search(data)`.
- Repos: `<repo>/index.json` → `libraries` (shared Lua libs, e.g. Madara helpers) + `extensions` (`RepositoryUpdateWorker.kt`, `RemoteExtRepoDataSource.kt`).
- Compat plan for Raygun: run scripts in `mlua` (lua52 feature) with a Rust shim re-implementing the kotlin-lib globals on top of `reqwest` + `scraper`.

---

## mihon (Android, Kotlin) — manga core

- **Source API** (`source-api/.../source/`): `Source{id,name,lang}`, `CatalogueSource` (`getPopularManga/getLatestUpdates/getSearchManga(page,query,FilterList)`, `getMangaUpdate`, `getPageList(chapter)`), `HttpSource`/`ParsedHttpSource`, `ConfigurableSource`, `SourceFactory`. Models `SManga`, `SChapter`, `Page`, `FilterList`.
- **Extensions are APKs** loaded with a `DelegateLastClassLoader` (`app/.../extension/util/ExtensionLoader.kt`); manifest metadata `tachiyomi.extension.class`, `tachiyomix.extensionLib` (supported 1.4, 1.6), NSFW/content-warning flags. Android-only by nature.
- **Schema** (`data/src/main/sqldelight/tachiyomi/data/*.sq`): column prefixes separate concerns — `remote_*` (from source), `user_*` (user choices), `state_*` (app bookkeeping). `UNIQUE(source_id, remote_url)`. `manga_track(manga_id, tracker_id, remote_id, last_chapter_read, total_chapters, status, score, start/finish_date, private)`.
- **Trackers** (`app/.../data/track/`): AniList, MAL, Kitsu, MangaUpdates, Bangumi, Shikimori, Komga/Kavita (enhanced/self-hosted), etc.
- Other: categories, library update scheduling with fetch intervals, download queue, backup (protobuf).

## mihon_img_upscale (Mihon fork) — AI upscaling

- Engine: **ncnn + Vulkan** (`app/src/main/cpp/`, CMake `find_package(ncnn)`), plus Qualcomm QNN / MediaTek LiteRT NPU backends; Anime4K GLSL shaders as a cheap option.
- Models (`app/src/main/assets/*-models/*.param|.bin`): Real-CUGAN SE/Pro/Nose (2/3/4x, denoise levels), Real-ESRGAN animevideov3 / general-x4v3, Waifu2x, W2xEX, AnimeJaNai, SPAN.
- **Queue** (`util/waifu2x/ImageEnhancer.kt`): `PriorityBlockingQueue`; order = visible primary page → visible secondary (spread) → promoted → preload, tie-break by distance to the current page; requests carry a *generation* so stale requests are dropped on chapter change; low→high priority upgrade re-enqueues.
- Cache keyed by page + model + scale + filter settings; skip enhancement above a resolution threshold; on failure always show the original.

## TachiyomiAT (Mihon fork) — OCR + translation

- Pipeline (`app/.../translation/ChapterTranslator.kt`): runs on **downloaded** chapters → ML Kit text recognition (CJK) → `TextBlock`s → `smartMergeBlocks` (merge nearby blocks into bubbles) → `TextTranslator.translate(pages)` (batch per chapter) → saved as a **JSON sidecar** next to the chapter.
- Model: `PageTranslation{blocks[], imgWidth, imgHeight}`, `TranslationBlock{text, translation, x, y, width, height, symWidth, symHeight, angle}`.
- Rendering is an **overlay** (`PagerTranslationsView`, `WebtoonTranslationsView`, `SmartTranslationBlock`) with a chosen font — no image inpainting. Non-destructive and cheap.
- Translators: ML Kit (on-device), Google (web), Gemini, OpenRouter (model/temperature/max tokens configurable).

---

## stremio-core (Rust, MIT) — catalog/stream addon protocol

- **Elm architecture**: `runtime/` (`Msg` → `update` → `Effects`), models in `models/` (`ctx`, `catalog_with_filters`, `meta_details`, `library_by_type`, `continue_watching_preview`, `player`, `streaming_server`, ...).
- **`Env` trait** (`runtime/env.rs`) abstracts the platform: `fetch`, `get_storage/set_storage`, `exec_concurrent/sequential`, `now`, `log`, analytics. We implement it once for Raygun (reqwest + our SQLite/kv) and reuse all models.
- **Addon manifest** (`types/addon/manifest.rs`): `id, version, name, types[], resources[] (catalog|meta|stream|subtitles|addon_catalog), idPrefixes, catalogs[] (with extra: search/genre/skip), behaviorHints`. Transport = plain HTTP GET of `/{resource}/{type}/{id}/{extra}.json` (`addon_transport/http_transport`).
- **`StreamSource`** (`types/resource/stream.rs`): `Url`, `YouTube{yt_id}`, `Torrent{infoHash, fileIdx, announce}`, `Rar/Zip/Zip7` archives, `External`, `PlayerFrame`; behaviorHints (proxyHeaders, notWebReady, bingeGroup).
- `stremio-watched-bitfield` — compact per-episode watched state.
- Torrent/archive playback relies on the separate Stremio streaming server (Node) — out of scope initially.

## cloudstream (Android + desktopApp, Kotlin) — multi-server extraction

- **`MainAPI`** (`library/src/commonMain/.../MainAPI.kt`): `mainPage`, `getMainPage`, `search`, `quickSearch`, `load(url) → LoadResponse` (Movie/TvSeries/Anime with episodes), `loadLinks(data, isCasting, subtitleCallback, callback)` — **streams results via callbacks**, so the UI can show servers as they arrive. Per-call timeouts; `usesWebView`, `providerType`, `supportedTypes`.
- **`ExtractorLink`**: `source, name, url, referer, quality, headers, type (VIDEO|M3U8|DASH|...), audioTracks, extractorData`. Referer/headers are mandatory for most hosts → player must inject them.
- `extractors/` (~105 host extractors: Dailymotion, Blogger, ...) shared by all providers; `network/WebViewResolver.kt` loads a page in a hidden WebView and **intercepts requests matching a regex** to sniff the real stream URL (also used for Cloudflare).
- Plugins: `.cs3` (dex) — Android-only. Sync providers (AniList, MAL, Simkl) in `syncproviders/`.

## aniyomi (Mihon fork, Apache-2.0) — unified manga + anime

- Parallel `animesource` API (`source-api/.../animesource/`): `AnimeSource`, `AnimeCatalogueSource`, `SAnime`, `SEpisode`, **`Hoster`** (servers are resolved lazily: hoster list first, then `videoList` per hoster — same idea as Nuclear's candidates).
- `Video{videoUrl, videoTitle, resolution, bitrate, headers, preferred, subtitleTracks, audioTracks, timestamps(OP/ED/Recap), mpvArgs, ffmpeg*Args}`.
- Player: **mpv-android** (libmpv) — needed for MKV/ASS/HEVC that platform players handle poorly.
- Separate DB tables/screens for anime vs manga but shared category/tracker/backup infrastructure.
