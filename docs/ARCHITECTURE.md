# Raygun Architecture

Status: **approved** (2026-10-30). Evidence for each decision is in [reference-notes.md](reference-notes.md).

Settled decisions: license **AGPL-3.0-only** (so code from Nuclear, and from GPL-3.0 RiPlay/Shosetsu/CloudStream, may be adapted with attribution) · app identifier **`app.raygun.player`** · primary extension runtime **QuickJS**, plus a Nuclear plugin compatibility layer (§7.4) · platform order **Android first, then Windows** · **automatic LAN device sync** (§10).

## 1. Guiding principles

1. **Rust owns the domain, React owns the pixels.** Library, queue, playback state, extensions, downloads, AI jobs and settings all live in Rust services. The UI, the MCP server and the LAN remote are three *clients* of the same services.
   *Why this differs from Nuclear:* Nuclear keeps state in Zustand stores and needs a Rust→JS bridge so MCP can reach it. That works for a music player, but Raygun has background work that must keep running when no UI is mounted: library updates, downloads, upscale/translate queues, and mobile background playback.
2. **Metadata is separate from playable sources.** A track, episode or chapter is pure metadata. *Sources* (streams, pages, passages) are resolved lazily from providers, cached, scored and allowed to fail. This is Nuclear's candidate model, generalized with Aniyomi's hoster model and CloudStream's streamed `loadLinks`.
3. **Every capability is a provider behind an interface.** Metadata, streams, manga sources, novel sources, lyrics, trackers, translators and upscalers all work this way. First-party features use the same interfaces as extensions, and everything registered is automatically reachable from MCP.
4. **Portable by construction.** All file paths go through a single resolver, and no code calls OS app dirs directly (§3).
5. **One responsive UI.** Every platform uses the same routes and components. Layout adapts by breakpoint and input type, not by maintaining separate desktop and mobile apps.
6. **Grow by vertical slices.** Each phase ships an end-to-end usable feature on top of shared crates, so no layer gets built speculatively.

## 2. System overview

```
┌──────────────────────────── React UI (webview) ────────────────────────────┐
│ Routes: Home · Library · Browse · Search · Player · Reader · Settings       │
│ Playback engines living in the DOM:  Html5Engine  ·  EmbedEngine (YT iframe)│
│ TanStack Query over typed commands · Zustand for UI-only state              │
└──────────────▲─────────────────────────────────────────────▲───────────────┘
   tauri-specta typed commands + events                      │ engine reports
┌──────────────┴──────────── Rust core (src-tauri = thin shell) ─────────────┐
│ raygun-app (service wiring, command layer)                                  │
│ ┌────────────┐ ┌────────────┐ ┌─────────────┐ ┌────────────┐ ┌───────────┐ │
│ │ library    │ │ playback   │ │ reader      │ │ ext-host   │ │ ai        │ │
│ │ (titles,   │ │ (queue,    │ │ (manga/novel│ │ (registry, │ │ (upscale, │ │
│ │ progress,  │ │ resolver,  │ │ page/passage│ │ JS + Lua   │ │ OCR,      │ │
│ │ trackers)  │ │ engines)   │ │ pipeline)   │ │ runtimes)  │ │ translate)│ │
│ └────────────┘ └────────────┘ └─────────────┘ └────────────┘ └───────────┘ │
│ ┌────────────┐ ┌────────────┐ ┌─────────────┐ ┌────────────┐ ┌───────────┐ │
│ │ db (sqlx)  │ │ net (http, │ │ jobs (queue,│ │ api (MCP + │ │ paths     │ │
│ │            │ │ cookies,   │ │ scheduler,  │ │ LAN REST/  │ │ (portable)│ │
│ │            │ │ proxy)     │ │ downloads)  │ │ SSE)       │ │           │ │
│ └────────────┘ └────────────┘ └─────────────┘ └────────────┘ └───────────┘ │
└──────▲─────────────────────▲──────────────────────────▲────────────────────┘
       │ mobile plugins      │ sidecars (desktop)       │ loopback HTTP
  ExoPlayer / AVPlayer   yt-dlp, ffmpeg, ncnn CLIs   MCP clients, LAN remote,
  Android dex loader     libmpv (later)              YouTube embed page
```

## 3. Portability (requirement 1)

### 3.1 Path resolver: `raygun-paths`
This is the only module allowed to compute filesystem locations. A CI lint (`clippy::disallowed_methods`) bans `app_data_dir`, `app_config_dir`, `app_cache_dir`, `app_local_data_dir`, `dirs::*` and `tauri-plugin-store` everywhere else.

Root resolution order:
1. `RAYGUN_HOME` env var (dev/testing).
2. **Portable mode** (desktop default) uses the folder next to the executable:
   - Windows/Linux: `current_exe().parent()`.
   - Linux AppImage: dirname of `$APPIMAGE`, because the exe itself sits on a read-only squashfs mount.
   - macOS: the folder containing `Raygun.app`, because the bundle is signed and must not change. Detect **App Translocation** (path under `/private/var/folders/.../AppTranslocation/`) and show a one-time "move the app or clear quarantine" screen instead of silently writing to a temp copy.
3. If that folder isn't writable (for example under `Program Files`), show an explicit choice: pick a folder, or run in "installed mode" using OS dirs. Raygun never falls back silently.
4. Mobile uses the app sandbox dirs (portability doesn't apply there).

### 3.2 Layout
```
Raygun/                      ← the user moves/copies this folder
  Raygun(.exe|.app|.AppImage)
  raygun-data/
    raygun.db                 SQLite (WAL), all structured state
    settings.toml             optional device-local bootstrap overrides (read before the DB opens);
                              user settings live in the DB so they can sync per key
    secrets.db                encrypted API keys/tokens (key derived per install)
    extensions/<id>/          installed extensions + their kv storage
    models/                   AI models (upscale .param/.bin, ONNX, OCR)
    bin/                      yt-dlp, ffmpeg, realcugan/realesrgan-ncnn-vulkan
    cache/{images,http,streams,pages,enhanced}/
    downloads/                media the user downloaded (relocatable)
    webview/                  webview profile (cookies, localStorage, IndexedDB)
    logs/
```

### 3.3 Hidden writers to neutralize
| Leak | Fix |
|---|---|
| WebView2 profile → `%LOCALAPPDATA%\<id>\EBWebView` | Create the window in Rust with `WebviewWindowBuilder::data_directory(paths.webview())`, not from `tauri.conf.json`. WebKitGTK honors the same option. |
| macOS WKWebView data store → `~/Library/WebKit` | Only partly fixable: use a non-persistent store plus our own cookie persistence (`net` crate). Document this as a known limitation. |
| `tauri-plugin-store`, `window-state`, `log` default paths | Don't use them; settings/window state go in our DB, logs use our `tracing` file appender in `logs/`. |
| Deep-link / protocol handler registration (registry) | Disabled in portable mode. OAuth uses a loopback redirect (`http://localhost:<port>/callback`); mobile uses deep links. |
| Updater | Portable mode updates in place: download the zip, swap the exe on restart. No installer. |
| WebView2 runtime | Use the evergreen runtime by default. An optional "fully self-contained" build bundles the Fixed Version runtime next to the exe. |

**Verification:** a CI job runs the Windows portable build under a fresh user profile, drives a smoke script, then diffs `%APPDATA%`, `%LOCALAPPDATA%` and `HKCU\Software` before and after. Any new entry fails the build.

## 4. Repository layout

pnpm + Turborepo for TypeScript and a Cargo workspace for Rust, both in one repo (same tooling family as Nuclear).

```
raygun/
  apps/raygun/
    src/                     React app (routes, features, engines)
    src-tauri/               thin Tauri shell: window setup, plugin init, calls raygun-app
    src-tauri/gen/android|apple/
  crates/
    raygun-paths/            §3
    raygun-model/            domain types (serde + specta) shared with TS
    raygun-db/               sqlx migrations + repositories
    raygun-net/              reqwest client, cookie jar, rate limits, stream proxy, webview resolver API
    raygun-jobs/             persistent job queue, priority scheduler, downloads
    raygun-playback/         queue, resolver pipeline, engine protocol, media session glue
    raygun-library/          titles/units/progress/categories, update checks, trackers
    raygun-reader/           page/passage pipeline, prefetch, chapter cache
    raygun-ext-host/         manifest, permissions, registry, runtimes (js, lua, stremio)
    raygun-ai/               upscale / OCR / translate providers + model manager
    raygun-api/              MCP server (rmcp) + LAN REST/SSE, auth
    raygun-sync/             mDNS discovery, pairing, encrypted peer sync (§10)
    raygun-app/              wires services, exposes tauri commands/events
  packages/
    sdk/                     @raygun/sdk – extension authoring types + helpers (published)
    bindings/                generated TS from tauri-specta (never hand-edited)
    ui/                      component library (Storybook)
    i18n/
  extensions/                first-party extensions, built with the same SDK
  plugins-mobile/            Tauri mobile plugins (Kotlin/Swift): native audio, dex loader
  docs/
  reference/                 git-ignored clones
```

## 5. Data model

There are two schemas, because music is relational (artists ↔ albums ↔ tracks ↔ playlists) and queue-driven, while everything else is a *title made of ordered units*.

**Music** (`music_*` tables): `artist`, `album`, `track`, `playlist`, `playlist_item`, `favorite`, `play_history`, `stream_candidate` (cached per track and provider, with `verification`, `failed_at` and `resolved_at`).

**Episodic library** (manga, novel, anime, series, movie, podcast):
```
title(id, kind, source_id, remote_url, remote_title, remote_cover, remote_*, user_favorite_at,
      user_*, state_last_update, state_next_update, state_*)  UNIQUE(source_id, remote_url)
unit(id, title_id, kind[chapter|episode|movie|passage], number, season, remote_url, remote_*, state_*)
unit_progress(unit_id, position, total, completed_at, updated_at)   -- page idx / char offset / ms
category, title_category
tracker_binding(title_id, tracker_id, remote_id, status, score, last_unit, ...)  -- Mihon's manga_track, generalized
download(unit_id, status, path, bytes)
artifact(unit_id, kind[enhanced_page|translation_overlay|subtitle], key, path)  -- AI outputs, cache keys
```
**Sync-ready from the first migration** (so §10 needs no schema rewrite later): every synced table uses a **UUIDv7 `id`** (not autoincrement integers, which collide across devices), plus `updated_at_hlc` (a hybrid logical clock value), `origin_device` and a `deleted` tombstone flag instead of hard deletes. Per-device data (caches, downloads, window state) lives in tables marked `local_*` and is never synced.

This reuses Mihon's column-prefix convention: `remote_*` comes from the source and is overwritten on refresh, `user_*` is never touched by refresh, and `state_*` is app bookkeeping. External IDs (AniList, MAL, TMDB, IMDb, MusicBrainz) go in an `external_id(title_id, namespace, value)` table, so Stremio `tt…` IDs and tracker IDs map onto the same row.

## 6. Media engine (requirement 3)

### 6.1 Resolution pipeline (`raygun-playback`)
```
PlayableRef (track | episode | movie)
   └─► Resolver.candidates(ref)  ── asks every enabled stream provider in parallel,
                                    results streamed as they arrive (CloudStream-style)
         └─► SourceCandidate { provider, label, quality, kind, score, failed, data }
               └─► Resolver.resolve(candidate)  (lazy, cached with expiry)
                     └─► PlayableStream {
                           kind: File | Http | Hls | Dash | YouTubeEmbed{id} | Torrent(later)
                           url, headers{Referer, UA, Cookie}, expires_at,
                           subtitles[], audio_tracks[], timestamps[OP/ED], engine_hint
                         }
```
Candidates are scored on provider priority, past success rate, duration match against metadata and quality preference. A failed resolution or playback marks the candidate and advances to the next one automatically. This gives the "unblockable" behavior: **direct extraction → proxied direct → embed fallback.**

### 6.2 Engines (RiPlay HybridPlayer, generalized)
`PlaybackService` (Rust) owns the queue and the canonical player state. It drives exactly one active engine through a small protocol: `load(stream) · play · pause · seek · setVolume · setRate`, with the engine reporting `state/position/duration/error`.

| Engine | Where | Plays | Notes |
|---|---|---|---|
| **Html5Engine** | webview `<audio>/<video>` + hls.js / dash.js | files, http, HLS, DASH | All remote URLs go through the `net` loopback proxy, which injects `headers` (Referer/UA/Cookie), adds CORS, passes Range through and serves local files. This is Nuclear's `stream_server` plus CloudStream's header requirements. |
| **EmbedEngine** | iframe → page served by our loopback at **`http://localhost:PORT/embed/youtube`** | YouTube IFrame API | Must use the hostname `localhost`, never `127.0.0.1`: YouTube's embed check reads the Referer string (RiPlay finding). Control goes over `postMessage`. Video can be shown or hidden, so the same engine covers audio-only and music video. |
| **NativeAudioEngine** | Tauri mobile plugin: ExoPlayer/Media3 (Android), AVPlayer (iOS) | direct audio streams | Required for background playback and lock-screen controls on mobile, where webview audio is suspended. Phase 1 spike. |
| **MpvEngine** (later) | libmpv embedded in a child native window (desktop), mpv-android | MKV, HEVC, ASS subs, anything else | Same approach as Aniyomi. Added for movies/anime. |

The **mixed queue** holds audio and video items side by side; each `QueueItem` stores its `media_kind`, and the UI shows a video surface only when the active item has one. OS media controls (SMTC on Windows, MPRIS on Linux, Now Playing on macOS, Media3 session on Android) always read the canonical state from `PlaybackService`, never from an engine. RiPlay has to synthesize player events to achieve this; with one Rust-owned state it comes for free.

### 6.3 YouTube direct extraction
- Desktop: a `yt-dlp` sidecar in `raygun-data/bin`, auto-downloaded and signature-checked (Nuclear's approach).
- Mobile: no yt-dlp, so the **EmbedEngine is the primary path** there (exactly RiPlay's approach). A JS InnerTube extractor running in the extension runtime is a possible later upgrade.

### 6.4 Scaling to movies/TV/anime
- **Stremio addons:** depend on `stremio-core` (MIT) and implement its `Env` trait over `raygun-net` and `raygun-db`. Catalog, meta, stream and subtitle resources map straight onto our provider kinds, and `StreamSource::Url/YouTube` map onto `PlayableStream`. Torrent streaming is a separate later decision.
- **CloudStream/Aniyomi-style sources:** a `video.source` capability with `search · load(url)→Title+Units · listServers(unit) · resolve(server)→PlayableStream[]`, where results are streamed to the UI.
- **WebView resolver** (`net.webview_resolve(url, intercept_regex, timeout)`): opens a hidden webview using our profile and returns the first request matching the pattern. It handles Cloudflare challenges (cookies are copied into the reqwest jar) and embed hosts whose stream URL only appears at runtime (CloudStream's `WebViewResolver`).

## 7. Extensions (requirement 4)

### 7.1 Manifest
```jsonc
// raygun-extension.json
{
  "id": "dev.raygun.mangadex", "name": "MangaDex", "version": "1.2.0",
  "apiVersion": 1,
  "runtime": "js",                     // js | lua-shosetsu | stremio | ui | android-dex
  "entry": "dist/index.js",
  "provides": ["manga.source"],        // music.metadata, music.stream, lyrics, video.source,
                                       // novel.source, tracker, translator, upscaler, ...
  "permissions": { "net": ["api.mangadex.org", "uploads.mangadex.org"], "webview": false },
  "contentRating": "safe",
  "settings": [ { "key": "lang", "type": "select", "options": ["en","ja"] } ]
}
```
Repositories use an `index.json` listing extensions and shared libraries (the Shosetsu and Mihon repo format).

### 7.2 Runtimes
| Runtime | Engine | Purpose |
|---|---|---|
| `js` (**primary**) | **rquickjs** (QuickJS) inside Rust, one isolated context per extension | All first-party and new community providers. Headless, so it works in background jobs and on mobile, and its sandbox is real: no ambient FS or network. QuickJS has no JIT, so it's allowed on iOS. |
| `lua-shosetsu` | **mlua** (lua52) + shim re-implementing `kotlin-lib` globals over reqwest/scraper | Run community Shosetsu novel extensions unmodified. |
| `stremio` | no code: HTTP addon via stremio-core | Movie/series/anime catalogs and streams. |
| `nuclear-compat` | rquickjs + shim (§7.4) | Run unmodified Nuclear plugins' provider logic headlessly. |
| `ui` | webview, Nuclear-style module eval with whitelisted `require` | Later. Dashboard widgets, themes, custom views, and the UI half of Nuclear plugins. |
| `android-dex` | Tauri Kotlin plugin using `DelegateLastClassLoader` (Mihon's loader, Apache-2.0) | Later, Android-only. Loads Mihon/Aniyomi extension APKs (CloudStream `.cs3` possibly) and bridges `Source` calls into Rust. |

On desktop, Mihon/Aniyomi compatibility could come from an optional **Suwayomi-Server sidecar** (JVM) exposed as a remote source provider. It's heavy, so it stays opt-in and outside the portable core.

### 7.3 Host API (same for all code runtimes, implemented once in Rust)
`http.fetch` (permission-checked host allowlist, shared cookie jar, rate limiting) · `html.parse` → selector API (`scraper` crate) · `json`, `crypto`, `encoding` · `storage` (per-extension kv) · `settings` · `log` · `webview.resolve` (only when the extension holds the `webview` permission) · `cache`.

Providers register through the SDK (`api.providers.register({kind:'manga.source', ...})`). The registry is in Rust, so the UI, jobs and MCP all see the same set.

### 7.4 Nuclear plugin compatibility layer
Nuclear plugins are npm-style folders (`package.json` with a `nuclear` field, TS or JS entry) that receive a `NuclearPluginAPI` in `onLoad/onEnable`. Raygun runs them through an **import converter plus a runtime shim**:

1. **Converter (at install):** read the manifest, transpile TS/TSX to a CommonJS bundle in Rust (`oxc` transformer, no Node needed), and record which `require()` targets are used. The output is a normal Raygun extension (`runtime: "nuclear-compat"`) with a generated `raygun-extension.json`. Its permissions come from the Nuclear manifest plus the hosts seen in the code, and the user confirms them.
2. **Shim (at runtime, in QuickJS):** `require('@nuclearplayer/plugin-sdk')` returns a `NuclearPluginAPI` built on the Raygun host API. Domain hosts are mapped as follows: `providers` → Raygun registry (Nuclear kinds `metadata · streaming · lyrics · dashboard · playlists · discovery` map onto `music.*` kinds); `http` → `http.fetch`; `settings`, `logger`, `queue`, `playback`, `favorites`, `playlists` and `events` → the matching Rust services; `ytdlp` → the yt-dlp sidecar (desktop only).
3. **Limitations (shown on the extension's page):**
   - **UI code doesn't run headless.** `react`, `react/jsx-runtime` and `@nuclearplayer/ui` resolve to inert stubs. Plugins that register React views or widgets load without them, until the `ui` runtime (Phase 7) can host that half in the webview.
   - **No DOM or Node APIs** (`window`, `document`, `fs`) beyond what the shim provides; such plugins are flagged "unsupported" by the converter.
   - **`shell`/`navigation` hosts are no-ops**, and the API version is pinned to the plugin-sdk version the shim targets, with a compatibility table kept per SDK release.

Nuclear's model types (`Track`, `StreamCandidate`, `Stream`, ...) are adapted from `packages/model` (AGPL, attributed) so their provider outputs convert losslessly into Raygun's models.

## 8. AI subsystem (`raygun-ai`)

### 8.1 Upscaling
- **v1 (desktop):** `realcugan-ncnn-vulkan` / `realesrgan-ncnn-vulkan` / `waifu2x-ncnn-vulkan` CLI binaries (MIT) in `bin/`, with models in `models/`. This works on Windows, Linux and macOS with any Vulkan/MoltenVK GPU and needs no FFI.
- **v2:** in-process inference via ncnn FFI, or `ort` (ONNX Runtime with DirectML/CoreML/NNAPI execution providers), which also covers mobile. GLSL Anime4K in the webview as a zero-install fallback.
- **Queue** (mihon_img_upscale design): priority = visible page → spread partner → promoted → preload by distance. Requests carry a reader *generation*, so a chapter change drops stale work. Results are cached under `hash(page bytes) + model + scale + params` in `cache/enhanced`. The original image is always shown first, and errors never block reading.

### 8.2 OCR + translation (TachiyomiAT design, extended)
```
page image ─► OcrProvider ─► blocks{bbox, text, angle} ─► merge into bubbles
           ─► TranslatorProvider (batched per chapter, with title-level glossary/context)
           ─► TranslationOverlay JSON (artifact table) ─► React overlay with fit-text fonts
```
- OCR providers: an ONNX model (manga-ocr / PaddleOCR) via `ort`, OS OCR (Windows.Media.Ocr, Apple Vision), or a vision-LLM single pass that returns boxes plus translation in one call.
- Translators: Claude (default `claude-sonnet-5-5`), any OpenAI-compatible endpoint (covers local Ollama/LM Studio), DeepL, Google.
- Output is non-destructive: an overlay, not inpainting (inpainting can come later). The same translator providers translate novel chapters and lyrics.
- Model manager: downloads models into `models/` with checksums and shows sizes/licenses. Secrets live in `secrets.db`.

## 9. MCP server and remote API (`raygun-api`)

- **Transport:** rmcp streamable HTTP on `http://localhost:<port>/mcp`, plus a tiny `raygun-mcp` stdio shim binary for clients that only speak stdio.
- **Auth:** a bearer token generated per install (shown in Settings), plus `Host`/`Origin` validation against DNS rebinding. Nuclear's server has no auth; ours needs it because it can control the whole app.
- **Tool surface:** Nuclear's meta-tool pattern — `list_domains`, `list_methods(domain)`, `method_details`, `describe_type`, `call(method, args)`. The method catalog is generated from the Rust service layer and the provider registry, so new domains and extension providers show up automatically without bloating the agent's tool list. A few high-value direct tools are added for ergonomics: `search`, `play`, `now_playing`, `queue_add`, `library_list`, `open_reader`.
- **Resources:** `raygun://now-playing`, `raygun://queue`, `raygun://library/{kind}`, and reading-progress resources.
- **LAN remote:** the same Axum server serves REST + SSE and a remote-control web UI, with QR-code pairing that issues a scoped token. It's off by default and binds to the LAN only when the user enables it.

## 10. Device sync (`raygun-sync`)

Goal: "set it and forget it". Paired devices on the same network sync automatically whenever they can see each other, with no server and no account.

- **Discovery:** mDNS/DNS-SD (`mdns-sd` crate) advertises `_raygun._tcp` with the device ID and a public-key fingerprint. Android needs a `WifiManager.MulticastLock` (held by a small mobile plugin) while sync is active.
- **Pairing (once per device pair):** one device shows a QR code or 6-digit code. The pair exchanges long-term **Ed25519** device keys, and each side stores the other's key as trusted. After that, unknown devices are ignored.
- **Transport:** a direct encrypted connection between paired devices (Noise `XX` handshake via the `snow` crate, or TLS with pinned self-signed certificates), on its own port, separate from MCP/LAN remote.
- **Data model:** per-row change log with **hybrid logical clocks**; merge is last-writer-wins *per field*, with tombstones for deletes. Exceptions: reading/watch progress merges as max-progress-wins, and play counts/history are append-only unions. Each device stores a vector of "last HLC seen per peer", so a sync session only exchanges the delta.
- **What syncs (each item is a toggle):** library and categories, progress/history, playlists and favorites, tracker bindings, settings (per-key opt-out for device-specific ones), installed extensions list (each device installs them itself), and AI glossaries. Optional: downloaded files and AI artifacts, pulled on demand. Never synced: caches, secrets (unless explicitly enabled, end-to-end encrypted), window state.
- **When:** on app start, on network change, every *N* minutes while both are reachable, and shortly after local changes (debounced). On Android a WorkManager job syncs periodically on unmetered Wi-Fi only, so the phone catches up even without opening the app.
- **Conflicts and safety:** a sync log is viewable in Settings → Devices, plus an "undo last sync" option backed by a pre-sync snapshot of changed rows.

## 11. UI (requirement 2)

- **Stack** (matches Nuclear so its patterns transfer): React 19, TypeScript strict, Vite, Tailwind v4, TanStack Router + Query + Virtual, Zustand for UI-only state, `motion` + `@use-gesture/react`, Lucide, Sonner, Zod, Vitest + RTL, Storybook.
- **Mobile-first shell:** bottom tab bar (Home · Library · Browse · Search · More), a persistent mini-player that swipes up into a full-screen player sheet (swipe down to close, horizontal swipe to skip), pull-to-refresh, long-press context sheets, and safe-area insets.
- **Wide layouts:** at ≥ 1024 px the tab bar becomes a side rail, the player sheet becomes a docked bar plus an expandable panel, and lists become grids. Keyboard shortcuts and hover states are enabled when a pointer is detected.
- **Readers:** manga (paged LTR/RTL, vertical webtoon, double-page spreads, tap zones, pinch zoom) and novel (reflowable text, typography controls, scroll or paginated) share prefetch and progress hooks.
- **Theming:** CSS variables with light/dark plus AMOLED; user themes come later through UI extensions.

## 12. Cross-cutting

- **Typed boundary:** tauri-specta generates `packages/bindings`, and the UI only calls generated functions.
- **Jobs:** a persistent queue in SQLite (library updates, downloads, AI work) with priorities and per-source rate limits. It resumes after restart, and on Android it hands off to WorkManager/foreground service through a mobile plugin.
- **Networking:** reqwest + rustls (no OpenSSL, which simplifies Android/iOS cross-compiles), with the cookie jar persisted in `webview/`-adjacent storage and per-host concurrency limits.
- **Observability:** `tracing` to rotating files in `logs/`, plus an in-app log viewer (Nuclear's startup ring buffer idea).
- **Testing:** Rust unit/integration tests per crate, Vitest + RTL with test-wrappers for views (Nuclear convention), the portable-leak CI job, and recorded HTTP fixtures for extension tests.
- **Content policy:** first-party extensions target legitimate sources only (local files, YouTube, MangaDex, Jellyfin/Komga/Kavita, Stremio official/community addons the user adds). Raygun ships the *runtimes*; users choose repositories.

## 13. Known risks

| Risk | Mitigation |
|---|---|
| Mobile background audio in webview | NativeAudioEngine spike in Phase 1; if blocked, audio-only items fall back to the native engine on mobile from day one. |
| YouTube embed/extraction policy changes | Multiple resolvers per item, automatic fallback, and resolvers implemented as updatable extensions, not baked into the core. |
| Codec gaps in system webviews (MKV/HEVC/AC3) | MpvEngine before the movies/anime phase ships. |
| iOS rules on downloaded code | The interpreter-only runtime (QuickJS) lowers the risk. Expect sideload/AltStore distribution for the full extension feature set. |
| License hygiene | Raygun is AGPL-3.0-only. Code adapted from another repo keeps a header naming the source and its license, and Apache-2.0 sources are listed in `NOTICE`. |
| Sync on Android (multicast, Doze) | MulticastLock while syncing, WorkManager on unmetered Wi-Fi, and a manual "sync now" fallback. |
| Scope explosion | Strict phase exit criteria, and each milestone gets its own short spec in `docs/phases/`. |
