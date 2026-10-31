# Raygun Roadmap

Each phase ends with something usable. Milestones (M) are sized to fit **one focused working session**: about a day of work with a clear done-check, so no session has to hold the whole system in its head. Before a milestone starts it gets a short spec in `docs/phases/phase-N/MNN-<name>.md`. The spec covers its goal, files touched, interfaces, a done-check, and which sections of `ARCHITECTURE.md` / `reference-notes.md` are relevant, so a session loads only those.

**Platform order:** Android first (verified on the `Pixel_4` emulator), Windows second. **Since M05, only the 📱 checks run; desktop work and checks are deferred until the user asks for them.** macOS, Linux and iOS are built in CI from M10 on, but are only lightly tested until later phases. Every UI milestone's done-check runs on Android before desktop.

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
- M22 **Device sync v2**: automatic triggers (start, network change, debounced local changes), Android WorkManager background sync on Wi-Fi, optional on-demand file transfer for downloads
- M23 Lyrics view with synced scrolling; Furigana/romaji via a lyrics post-processor (kuromoji-style tokenizer)

## Phase 3 — Manga
- M24 Episodic library schema (title/unit/progress/category/external_id), library service, sync rules for these tables
- M25 `manga.source` capability + SDK types (Mihon-shaped: popular/latest/search+filters/details/chapters/pages)
- M26 First-party sources: MangaDex, local CBZ/folder, Komga/Kavita
- M27 Reader: paged (LTR/RTL), webtoon, spreads, zoom, tap zones, prefetch, page cache
- M28 Library updates job (fetch intervals), updates feed, downloads queue
- M29 Trackers: AniList, MyAnimeList (OAuth via loopback/deep link), sync of progress
- M30 Backup/restore (own format) + **Mihon backup import**

## Phase 4 — Light novels
- M31 `novel.source` capability + novel reader (reflow, typography, scroll/paginated, progress by char offset)
- M32 **Shosetsu compatibility**: mlua (lua52) runtime + kotlin-lib shim, Shosetsu repo support
- M33 Local EPUB import, chapter downloads, TTS (OS voices) as a stretch goal

## Phase 5 — AI enhancement
- M34 Model manager (`models/`, checksums, licenses) + `raygun-ai` provider registry + secrets store
- M35 Upscale v1: ncnn-vulkan CLI sidecars (desktop), priority queue with generations, enhanced-page cache, reader integration
- M36 OCR providers (ONNX manga-ocr/Paddle via `ort`; OS OCR), bubble merging
- M37 Translators (Claude, OpenAI-compatible/local, DeepL), chapter batching with glossary, overlay renderer
- M38 Auto-translate on download; novel and lyrics translation with the same providers
- M39 Upscale v2: in-process `ort`/ncnn incl. Android acceleration (NNAPI/Vulkan)

## Phase 6 — Video: movies, TV, anime
- M40 Video player UI (gestures for seek/volume/brightness, PiP, subtitles .srt/.vtt/.ass via JASSUB, audio track select, OP/ED skip)
- M41 HLS/DASH in Html5Engine, header-injected proxy for segments
- M42 **Stremio addons** via `stremio-core` (`Env` impl), catalogs/meta/streams/subtitles, continue watching
- M43 `video.source` capability (CloudStream/Aniyomi-shaped: search, load, servers, resolve with streamed results)
- M44 WebView resolver (stream sniffing + Cloudflare cookie handoff)
- M45 **MpvEngine**: mpv on Android, libmpv child window on desktop
- M46 Trackers: Trakt, Simkl, AniList anime; watched-bitfield per season

## Phase 7 — Compatibility bridges + ecosystem
- M47 Android `android-dex` runtime: load Mihon/Aniyomi extension APKs (Kotlin plugin, Apache-2.0 loader)
- M48 Optional Suwayomi sidecar provider on desktop
- M49 `ui` extension runtime (widgets, views, themes), including the UI half of Nuclear plugins
- M50 iOS build hardening, store/sideload strategy
- M51 Sync beyond the LAN (optional self-hosted relay or WebDAV), still end-to-end encrypted

---

## Resolved decisions (2026-10-30)
1. License: **AGPL-3.0-only**.
2. Extension runtime: **QuickJS** primary, plus a Nuclear plugin compatibility layer with documented limitations (ARCHITECTURE §7.4).
3. Platform order: **Android first** (Pixel_4 emulator), then Windows.
4. Identifier: **`app.raygun.player`**.
5. New requirement: **automatic LAN device sync**, configurable and hands-off (ARCHITECTURE §10, M21–M22).
