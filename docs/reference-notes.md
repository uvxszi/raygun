# Reference Repo Notes

Condensed findings from the repos in `reference/` (git-ignored, shallow clones, cloned 2026-10-30).
Read this instead of re-exploring the repos. Each section: what to take, what to avoid, where to look.

## License map (decides what Raygun may copy and what it may only re-implement)

| Repo | License | Implication for Raygun |
|---|---|---|
| stremio-core | MIT | Can depend on it as a crate / copy freely |
| mihon, aniyomi, TachiyomiAT, mihon_img_upscale | Apache-2.0 | Can copy/adapt with attribution + NOTICE |
| RiPlay, shosetsu, cloudstream | GPL-3.0 | Copying code makes Raygun GPL-3.0 |
| nuclear | AGPL-3.0 | Copying code makes Raygun AGPL-3.0 |
| miru-app | AGPL-3.0 | Same as nuclear |
| miru-repo (extensions) | MIT (a few AGPL/GPL-3.0) | Can be bundled or converted with attribution |
| Nuvio (NuvioTV, NuvioMobile; not cloned) | GPL-3.0 | Read for the plugin format; copying code makes Raygun GPL-3.0 |

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
- The local clone (2026-11-02) no longer has the prebuilt ncnn Android libraries (`third_party/ncnn-*`) or the model weights (`assets/*-models/*.bin`); the `.param` files and code remain. The weights come from the Real-CUGAN/Real-ESRGAN/waifu2x releases, and Raygun downloads its own, pinned by SHA-256.

## TachiyomiAT (Mihon fork) — OCR + translation

- Pipeline (`app/.../translation/ChapterTranslator.kt`): runs on **downloaded** chapters → ML Kit text recognition (CJK) → `TextBlock`s → `smartMergeBlocks` (merge nearby blocks into bubbles) → `TextTranslator.translate(pages)` (batch per chapter) → saved as a **JSON sidecar** next to the chapter.
- Model: `PageTranslation{blocks[], imgWidth, imgHeight}`, `TranslationBlock{text, translation, x, y, width, height, symWidth, symHeight, angle}`.
- Rendering is an **overlay** (`PagerTranslationsView`, `WebtoonTranslationsView`, `SmartTranslationBlock`) with a chosen font — no image inpainting. Non-destructive and cheap.
- Translators: ML Kit (on-device), Google (web), Gemini, OpenRouter (model/temperature/max tokens configurable).

---

## stremio-core (Rust, MIT) — catalog/stream addon protocol

- **Elm architecture**: `runtime/` (`Msg` → `update` → `Effects`), models in `models/` (`ctx`, `catalog_with_filters`, `meta_details`, `library_by_type`, `continue_watching_preview`, `player`, `streaming_server`, ...).
- **`Env` trait** (`runtime/env.rs`) abstracts the platform: `fetch`, `get_storage/set_storage`, `exec_concurrent/sequential`, `now`, `log`, analytics. Raygun implements it once (reqwest + its SQLite/kv) and reuses all models.
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

## miru (Flutter, AGPL-3.0; abandoned since 2024-04) and miru-repo (MIT) — JS extensions
Cloned 2026-11-01 into `reference/miru-app` (commit 8599d0d) and `reference/miru-repo` (145 extensions: 97 bangumi/video, 39 manga, 9 fikushon/novel; 35 NSFW). Basis for M46.7.
- **Repo:**
  - `index.json` is an array of the header fields as strings (`name, package, type, lang, license, version, webSite, url`, optional `icon`, `nsfw` "true"/"false", `description`, `tags`). `main.go` generates it from the `// ==MiruExtension==` header.
  - Install URL: `<repo>/repo/<package>.js`; the default repo is `https://miru-repo.0n0.dev`.
  - The installed version is compared with `!=`.
  - `parseExtension` reads `@key value` anywhere in the file.
- **Source:**
  - `export default class extends Extension {…}`. Miru rewrites that line to a named class, evaluates the script in its own runtime (QuickJS on Android and Windows), creates the instance, and calls `load()` without awaiting it.
  - **Methods:**
    - `latest(page)` and `search(kw, page, filter)` return `[{title, url, cover?, update?, headers?}]`;
    - `createFilter(filter?)` returns `{key: {title, min, max, default, options: {key: label}}}`;
    - `detail(url)` returns `{title, cover?, desc?, episodes: [{title, urls: [{name, url}]}], headers?}`;
    - `watch(url)` returns:
      - bangumi: `{type: hls|mp4|torrent, url, headers?, subtitles?: [{title, url, language?}], audioTrack?}`;
      - manga: `{urls, headers?}`;
      - fikushon: `{title, subtitle?, content: [paragraphs]}`.
  - Missing headers default to `{Referer: last request URL, User-Agent, Cookie: jar}` (a replacement, not a merge).
- **Runtime** (`lib/data/services/extension_service.dart:448-616`):
  - **`this.request(url, options)`:**
    - the address is `(options.headers["Miru-Url"] || webSite) + url`, plain concatenation, with the header still sent;
    - options: `method` (default get), `data` (an object is sent as JSON, or as a form with that content type), `queryParameters`;
    - the default UA is Edge 120;
    - it returns parsed JSON when the body parses, else text; non-2xx throws; a persistent cookie jar per domain.
  - **HTML helpers** (Dart `html` 0.15; every call re-parses):
    - `querySelector(html, sel)` → `Element{content, selector}` with async `text`/`outerHTML`/`innerHTML`/`querySelector`/`getAttributeText`/`removeSelector`;
    - `await querySelectorAll` → Elements over each match's outerHTML;
    - `getAttributeText(html, sel, attr)`;
    - `queryXPath(html, xpath)` → `{attr, attrs (JSON string), text, outerHTML, allHTML (a stringified list; extensions iterate it)}`;
    - `:nth-child(n)` is a 0-based literal index among all child nodes.
  - **Settings:** `registerSetting({title, key, type: input|radio|toggle, defaultValue, description?, options: {label: value}})` and `getSetting(key)`, all strings.
  - **Bundled:** CryptoJS, JSEncrypt, the npm `md5`, `console.log`, `setTimeout`, a `fetch` polyfill.
  - **Not provided:** atob/btoa, URL, TextEncoder, DOM.
- **Player:**
  - **Video:** media_kit with the headers; subtitles start off.
  - **Torrents:** go through a local bt-server.
  - **Manga:** pages all use `watch.headers`.
  - **Novels:** plain-text paragraphs.
- **Hard cases:**
  - `openWebView` (hanime1, mydramanovel, yomovies);
  - packed JS with eval;
  - CryptoJS in 20 extensions;
  - third-party key and extractor services (hianime, sflix, kisskh, filmpalast, wtr-lab, youtube);
  - torrents (nyaa, eztv, yts, mikanani, acg.rip);
  - `vod.api.xml.bajie` (no export);
  - `kuromanhua.xyz` (a package clash with komiku.com).
- **Official or public APIs among them:**
  - de.ccc.media (media.ccc.de talks, CC);
  - mangadex.org (official API);
  - mangacross.jp (Akita Shoten's own free chapters);
  - royalroad.com (original web fiction);
  - iptv-org (public playlists);
  - api.yinyuetai.
  - The rest are unofficial streaming, scanlation or torrent sites.

---

## Nuvio / NuvioTV (Kotlin, GPL-3.0) — Stremio addons plus "local scraper" plugins

Researched 2026-11-01 from the public repos (source only; nothing installed or run; the sites are JS-only, and the community guide `yoruix/nuvio-providers/DOCUMENTATION.md` is partly outdated).

| Repo | Platform / stack | License |
|---|---|---|
| github.com/NuvioMedia/NuvioTV (branch `dev`) | Android TV: Kotlin, Compose + TV Material 3, Media3, libmpv, QuickJS (`com.dokar.quickjs`), Jsoup | GPL-3.0 |
| github.com/NuvioMedia/NuvioMobile (branch `cmp-rewrite`) | Android + iOS, Kotlin/Compose Multiplatform (the old React Native app is `tapframe/NuvioMobile`) | GPL-3.0 |
| github.com/NuvioMedia/NuvioDesktop | Compose Desktop, alpha | GPL-3.0 |
| github.com/NuvioMedia/NuvioTVSmart | Tizen/webOS (JS) | GPL-3.0 |
| github.com/NuvioMedia/self-host | Supabase-style sync backend | Apache-2.0 |

- "Bring your own sources": nothing bundled, no default addons or repositories. A `playstore` flavour removes the plugin runtime (`FEATURE_PLUGINS_ENABLED=false`).
- Account sync (profiles, library, progress, addons, plugins and their settings) through Supabase; debrid (Real-Debrid, Torbox, Premiumize), a torrent engine, Emby/Jellyfin, watch party.

**Stremio addons** (Raygun has these since M41). Beyond the core protocol it reads:
- catalogs' `pageSize` and `showInHome` (home row toggle);
- streams' `clientResolve` (client-side torrent/debrid resolution, AIOStreams/Torrentio style) and `behaviorHints.audioDelayMs`/`audioDelayLanguage`;
- `kitsu:`/`mal:`/`anilist:`/`anidb:` ids;
- `videoHash`/`videoSize`/`filename` extras for subtitles.

**Plugin repositories:** added by URL; `/manifest.json` is appended unless it ends in `.json`. Unknown keys are ignored; required are `name`, `version` and a non-empty `scrapers`, and per scraper `id`, `name`, `version`, `filename`.

```jsonc
{
  "name": "My Repo", "version": "1.0.0", "description": "...", "author": "...",
  "scrapers": [{
    "id": "myprov", "name": "My Provider", "description": "...", "version": "1.0.3",
    "filename": "providers/myprov.js",   // relative to the manifest, or an absolute URL
    "supportedTypes": ["movie", "tv"],   // "series"/"show"/"other" count as "tv"
    "enabled": true, "hasSettings": false, "logo": "https://...png",
    "contentLanguage": ["en"], "formats": ["mp4", "m3u8"],
    "supportedPlatforms": ["android"], "disabledPlatforms": [],
    "supportsExternalPlayer": true, "limited": false
  }]
}
```

**Runtime:** one single-file script per scraper (no `import`), wrapped as `var module={exports:{}}; (function(){ CODE })();`, in a fresh QuickJS context per call. Limits on TV: 10 scrapers at once, 60 s each, results deduped by URL and capped at 150. No filesystem, cookies or storage; `fetch` has no host allow-list.

```js
module.exports = { getStreams, onSettings };   // or globalThis.getStreams
async function getStreams(tmdbId /*string*/, mediaType /*"movie"|"tv"|"series"*/, season, episode) {
  return [{ name, title, url /*required*/, quality, size, language, type: "hls|mp4|torrent",
            seeders, peers, infoHash, headers: { Referer, "User-Agent" },
            subtitles: [{ url, language, name, id, headers }], behaviorHints: { audioDelayMs } }];
}
// Globals: SCRAPER_ID, SCRAPER_SETTINGS, TMDB_API_KEY, console, fetch (method, headers, string or
// binary body, signal, redirect; response ok/status/url/headers.get/text/json/arrayBuffer),
// AbortController, atob/btoa, URL, URLSearchParams, cheerio (Jsoup-backed; require('cheerio') and
// its RN aliases), require('crypto-js') / CryptoJS. Mobile adds setTimeout (≤ 60 s), crypto.subtle,
// TextEncoder/TextDecoder, WebAssembly. onSettings() returns header/info/text/toggle/select fields,
// saved per scraper and passed back as SCRAPER_SETTINGS.
```

- **Arguments:**
  - the Stremio id is turned into a TMDB id with TMDB `find`;
  - anime ids pass `season = null` and the absolute episode; without a TMDB mapping, the raw `kitsu:123` id is passed as is.
- **Results:** they join the stream list as an addon named after the scraper (`bingeGroup: "local-plugin-<id>"`, headers as `proxyHeaders.request`).
- **CloudStream:** the TV app also loads CloudStream `.cs3` DEX plugins (Android only; not reusable).

**Other integrations:**
- **Trakt** (device-code OAuth): scrobbling, playback progress, watched/history, watchlist, lists as catalogs. Simkl works the same way.
- **MDBList** (user key): ratings from IMDb, Trakt, TMDB, Letterboxd, Rotten Tomatoes and Metacritic, each with a toggle and an order.
- **Skip segments:** IntroDB (intro, recap, outro, post-credits), AniSkip and Anime-Skip.
- **Subtitles:** auto-sync that corrects a constant delay or drift against the embedded track.
- **TMDB** with a build-time key (users can override it on mobile).
- **Home:** layouts (classic, grid, modern hero), catalog order, collections.
- **TV:**
  - a left rail that expands with D-pad Left;
  - focus restored per row by content id;
  - text entry sent from a phone through a QR-linked local page.

**Repositories in the wild:** the official `tapframe/nuvio-providers` holds only a template. Community collections (several unlicensed, many obfuscated) almost all scrape unlicensed streaming, download, anime and regional sites.

**For Raygun:**
- Raygun can support the format on its QuickJS host: `manifest.json` repositories and `getStreams` mapped onto Raygun streams, with headers going through the proxy.
  - Host API to add: binary `fetch`, `URL`/`URLSearchParams`, `atob`/`btoa`, `AbortController`, `setTimeout`, `TextEncoder`.
  - `cheerio` can sit on the Shosetsu Jsoup-compatible selector; crypto-js comes bundled.
  - It needs `onSettings` and the three globals.
- The same rule as Miru applies: scrapers only from repositories added by hand; Raygun ships and pre-adds none.
- Stremio additions worth taking: `showInHome`, `audioDelayMs`, the subtitle hash extras, and MDBList ratings (a user key, like OMDb).

## Kreate and RiMusic (M46.39, `reference/Kreate`, `reference/RiMusic`)

Android music players (ViMusic forks, Kotlin, GPL-3.0). What they draw on:
- **YouTube Music's web API** (`extensions/environment` in RiMusic, `modules/innertube` in Kreate): search filters for songs, videos, albums, artists, community playlists (`EgeKAQQoAEABagoQAxAEEAoQCRAF`) and featured playlists (`EgeKAQQoADgBagwQDhAKEAMQBRAJEAQ%3D`); playlists by `browse` `VL<id>`; endpoint names are obfuscated constants in `Environment.kt`.
- **Lyrics:** LRCLIB (`extensions/lrclib`), KuGou (`extensions/kugou/KuGou.kt`: `mobileservice.kugou.com/api/v3/search/song` → by hash within 5 s, else keyword → `krcs.kugou.com/download` LRC in base64, credit lines dropped), and YouTube Music's own (`requests/Lyrics.kt`: `next` → second tab's `browseId` → `browse` → `musicDescriptionShelfRenderer`).
- **Streams:** Piped and Invidious instances (`extensions/piped`, `extensions/invidious`). Raygun doesn't use them (M46.39 Decisions).
- **YouTube audio streams (M46.51):**
  - Kreate resolves a song's stream when ExoPlayer opens it (`di/InnertubeResolvingDataSource.kt`, kept until `expiresInSeconds` minus 30 s). `InnerTubeX.kt` hands that to MetrolistGroup's `innertubex` library (0.9.2, a dependency, not in the repo): a client fallback chain, signature and `n` deciphering (`YouTubeCipherService`), and proof-of-origin tokens made by BotGuard in a WebView (`PoTokenGenerator.kt`, from Metrolist). HLS and SABR streams are left out; ExoPlayer caches 512 KB fragments (`PlayerModule.kt` `CHUNK_LENGTH`).
  - RiMusic (`extensions/players/SimplePlayer.kt`) asks the web music client first, with a fixed token in `PlayerBody.kt`, then `TVHTML5_SIMPLY_EMBEDDED_PLAYER` and an obfuscated third client. Signatures and `n` go through NewPipeExtractor's `YoutubeJavaScriptPlayerManager` (`utils/NewPipe.kt`), which runs YouTube's player JavaScript; each address gets a HEAD check first.
  - Grayjay's YouTube plugin (`reference/grayjay-plugin-youtube`, AGPL-3.0) falls back from the web client to `ANDROID`, `ANDROID_VR` 1.65.10 and `VISIONOS` 1.04 `player` requests (`requestAndroidVRStreamingData`, `requestVisionOSStreamingData`), whose formats carry plain addresses, each checked with `verifyDirectPlayback`.
  - Measured on 2026-11-03 with six YouTube Music songs and visitor data from `www.youtube.com/sw.js_data`:
    - VISIONOS: plain addresses that downloaded for every song.
    - ANDROID, SDK-less ANDROID, ANDROID_VR, IOS: plain addresses whose downloads answered 403 (their streams want a proof-of-origin token).
    - ANDROID_MUSIC: wants a sign-in. TVHTML5: wants the player JavaScript's signature timestamp.
    - Without visitor data every client got "Sign in to confirm you're not a bot".
  - googlevideo sends a request without `Range` at about playback speed and cut it off at 99 %; any range (`bytes=0-` too) arrives at full speed (6 MB in 0.3 s).
  - Raygun (M46.51) does what Grayjay does: the YouTube extension's `music.stream` `resolve` asks VISIONOS → ANDROID_VR → IOS → ANDROID with visitor data, picks Opus to play or M4A to read (`decodable`), checks two bytes, and keeps the embed as the fallback. Nothing is deciphered and no token is made. NewPipeExtractor (GPL-3.0) or Piped/Invidious instances would work too, but run YouTube's JavaScript or a third party's server, and aren't needed while a client gives plain addresses.
- **Discord:** Kizzy (`modules/kizzy`), with a personal account token. Raygun doesn't (M46.36).
