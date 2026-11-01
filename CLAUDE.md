# Raygun

All-in-one multimedia app (music, video, movies/TV, anime, manga, light novels) built on Tauri v2 (Rust + React/TS). License AGPL-3.0-only. Identifier `app.raygun.player`.

## Read first

- `docs/ROADMAP.md`: phases and milestones; work happens one milestone at a time.
- `docs/phases/phase-N/MNN-*.md`: the spec for the milestone in progress. Write one before starting a milestone.
- `docs/ARCHITECTURE.md`: read only the sections the milestone spec lists.
- `docs/reference-notes.md`: findings from the reference repos in `reference/` (git-ignored). Check here before re-exploring a repo.

## Layout

- `apps/raygun/`: React UI (`src/`) and the thin Tauri shell (`src-tauri/`, plus `gen/android`).
- `crates/raygun-app`: service wiring + Tauri commands. `crates/raygun-model`: types shared with TS.
- `packages/bindings`: TS generated from Rust by tauri-specta. Never edit it; run `pnpm bindings`.
- `crates/raygun-resolver`: stream matching and ranking (pure). `raygun-app` has the `ResolverService` (cache, providers) and the per-load `resolution::Tracker` that `PlaybackService` follows for lookups and fallback.
- `crates/raygun-mcp`: the MCP server's transport (streamable HTTP on 127.0.0.1:8810–8819, bearer token, Host/Origin checks, JSON-RPC dispatch). The tools live in `raygun-app/src/mcp.rs`; Settings turns the server on (off by default).
- `crates/raygun-remote`: the LAN remote (web page + JSON/SSE API on 0.0.0.0:8820–8829, QR pairing with one-time codes). The backend lives in `raygun-app/src/remote.rs`; off by default. Verify on the emulator with `adb forward tcp:8820 tcp:8820` and the browser pane (loopback, so Android 17 never blocks it; a real LAN connection is `adb emu redir add tcp:<pc port>:8820`, which arrives from 10.0.2.2).
- `crates/raygun-sync`: device sync (Noise XX channel, pairing, half-duplex protocol, mDNS discovery); storage and merge live in `raygun-db::sync`. Test against a PC with `cargo run -p raygun-sync --example peer -- <dir> serve` (the emulator reaches it at 10.0.2.2). On hold by the user's call; off by default.
- Manga (M24): `raygun-db::titles` (episodic library), `raygun-ext-host::manga` (`manga.source` calls), `raygun-app/src/manga.rs`; covers and pages load through the stream proxy's `/image/` route with the source's headers.
- Novels (M31): `novel.source` = the manga catalogue calls plus `passage`; `raygun-ext-host::novel` turns passage HTML into text blocks (no source HTML reaches the webview). `raygun-app/src/catalog.rs` holds what manga and novels share; the UI's manga pages take a `medium` (`manga/medium.ts`). First-party source: `extensions/gutenberg`.
- EPUB and speech (M33): `raygun-app/src/epub.rs` reads books (lenient parsing, chapters split at TOC anchors as `path#start#end`); imports unpack to `downloads/books/<sha256>/` with `/local`-safe names, as `local.epub` novels. Novel chapters download through the manga queue (`downloads/novel/…/passage.json`). Text-to-speech is `Speech.kt` in the `raygun-audio` plugin, reporting over JNI to `raygun-app/src/tts.rs` (`TtsEvent`).
- Shosetsu (M32): `raygun-ext-host::shosetsu` runs Shosetsu's Lua novel sources (manifest runtime `shosetsu`, net `*` = any public host). `select` is a Jsoup-compatible selector engine, `dom` the Jsoup-like DOM, `http` the OkHttp-like API, `runtime` the sandboxed Lua 5.2 state (one thread per script), `prelude.lua` kotlin-lib's helpers. Shosetsu repositories (`index.json` with `scripts`) install through the normal Extensions screen; live check: `cargo test -p raygun-ext-host -- --ignored shosetsu_official` (`SHOSETSU_SCRIPTS=shosetsu.en.x,…`).
- `crates/raygun-ai` (M34): the encrypted secrets store (`secrets.db`, key in `local_meta`), the model catalogue/downloader (`src/catalog.json`, files pinned by SHA-256) and translators (Claude over raw HTTP, OpenAI-compatible, DeepL). App side: `raygun-app/src/ai.rs`, Settings → AI.
- Content filter and languages (M35): `ContentFilter` in `raygun-ext-host` (the `showNsfw` setting; hides `nsfw`-rated extensions' providers, `raygun.content.showNsfw()` for extensions, `nsfw` on listings, adult genres in `catalog::is_adult`). Locales are `apps/raygun/src/i18n/locales/<tag>.json`, loaded on demand; `pnpm i18n:import` copies exact-match translations from `reference/` apps (see `docs/TRANSLATING.md`).
- Podcasts (M37): `raygun-db::podcasts` (feeds, episodes, progress, device-local downloads), `raygun-app/src/podcast_feed.rs` (RSS/Atom via quick-xml), `raygun-app/src/podcasts.rs` (`PodcastService`; follows playback through `on_playback`, called by `PlaybackService` with every state change), `podcast.directory` capability (`extensions/podcast-directory`, iTunes Search API). UI: Library → Music → Podcasts and `/podcast?id=`.
- UI shell (M42.1): tabs Home · Library · Browse · Search · More (`shell/navItems.ts`; Watch lives in Library → Videos, `/watch` redirects there). Every screen starts with `shell/TopBar` (title, back, icon actions, `OverflowMenu`, search mode); sub-views use `shell/Tabs`; inputs go in `shell/Sheet` dialogs or the top bar, never at the end of a list. Title screens use `shell/TitleHero` and a floating `ResumeFab`. Browse (`pages/BrowsePage.tsx`) lists sources by kind (Stremio addons with catalogues open `/watch/addon`) plus the extension lists; More (`pages/MorePage.tsx`) links everything else. Settings is a category list (`settings/sections.ts`) with one screen per category (`pages/SettingsSectionPage.tsx`) built from `settings/prefs.tsx` rows (`PrefSwitch`, `PrefChoice`, `PrefSlider`, `PrefLink`); every option is a field of the Rust `Settings` (accent, start screen, grid columns, cover style, badges, resume, watched point, seek step, subtitles...).
- Watch (M38): `VideoPlayerActivity.kt` in the `raygun-audio` plugin (Media3 ExoPlayer: HLS/DASH, headers, subtitles, gestures, PiP), opened through `NativeAudio::video_open`; reports come back over JNI (`NativeBridge.onVideo`) to `raygun-app/src/video.rs`, which saves progress (`raygun-db::video`) for Continue watching. UI: Library → Videos and the player.
- Stremio addons (M41): `raygun-app/src/stremio/` (`protocol` = the addon HTTP transport, pure; `StremioService` = addons, catalogs, meta, streams, subtitles, play), addons stored in `raygun-db::stremio`. Progress keys are `stremio:<type>:<videoId>`. Video subtitles load through the proxy's `/subtitle/` route (an empty track when a file fails). UI: `/watch` rows, `/watch/title`, `/watch/catalog`, `/watch/addons`; Search → Movies & series. Check on the emulator with a local addon at 10.0.2.2 (after allowing local network access) or through `adb reverse`.
- Local network access (M41.1): Android 17 apps targeting SDK 37 need `ACCESS_LOCAL_NETWORK` to reach private, link-local or `.local` hosts and to accept LAN connections; without it TCP times out. `raygun-net::local_network` says which hosts are local; `raygun-app/src/local_network.rs` (`LocalNetwork`) asks the `raygun-audio` plugin and fails requests to local hosts at once without it (`check`). The UI explains and asks first through `network/useLocalNetworkGate` (Addons page, remote switch).
- Library (M39): media types Music/Podcasts/Manga/Novels/Videos in `LibraryPage`; manga and novel categories in `raygun-db::categories` (commands take a `medium`); filters, sort and genre tags are computed in the UI by `manga/libraryFilters.ts` over `MangaLibraryEntry`.
- Video sources (M42): `video.source` = the manga catalogue calls (details add `type`, `year`, `cast`, `directors`; `episodes` with `season`) plus `servers` and `videos` (aniyomi's hosters: each server resolves on its own). `raygun-ext-host::video` parses them; `raygun-app/src/video_sources.rs` (`VideoSourceService`) wraps a `Catalog` of kind `video` (titles store `media_type`/`year`), orders links by the `preferredVideoQuality` setting and plays through `video::play` (progress key `source:<sourceId>:<episodeId>`). UI: Watch rows per source, `/watch/source` (the shared source page, `medium` `video`), `/watch/video` (title, episodes, server picker), Search → Movies & series, Settings → Playback. First-party source: `extensions/internet-archive/src/films.ts`.
- Video library (M43): saved movies and series are titles of kind `video` (Stremio ones have source `stremio` and remote id `<type>:<id>`, stored and refreshed by `StremioService`); watching progress is mirrored onto their episodes by progress key (`raygun-app/src/video_library.rs`), and library updates (`MangaService::run_update`) cover them. Downloads: `raygun-app/src/video_downloads/` (`VideoDownloadService`, one at a time, queue in `raygun-db::video_downloads`, files in `downloads/video/<id>/`; `hls.rs` picks the variant and rewrites keys, maps and segments to local names); `video::play` plays a finished download under the same key through the proxy's `/local/` route. UI: Library → Videos (`MangaLibraryView` with `medium` `video`), `/updates` and `/downloads` (tabs), `/watch/history`.
- Playback sessions (M44): `raygun-app/src/video_session.rs` (`VideoSessions`) plays a title's episodes: the player (`VideoPlayerActivity`, `singleTask`) opens at once, links resolve in the background (all servers / all addons, the last-used source and `bingeGroup` first), a failed link falls back to the next. Rust sends `videoUpdate` messages (`load`, `episodes`, `sources`, `loading`, `error`); the player answers with `request` reports (`next`, `previous`, `episode`, `source`, `autoplay`, `ready`), handled in order through one queue in `video.rs`. Episodes played this way resume through an `EpisodeRef`; finishing one puts the next in Continue watching at 1 ms. Title pages: tap = session, long press or Choose a source = picker.
- Search (M40): `/search?type=all|music|podcast|video|manga|novel|library&q=`; manga/novel global search (`catalog_global_search`) and source filters (`filters()` in the SDK, `SourceFilter`/`FilterValue` in the model, `catalog_filters`/`catalog_search`); MangaDex implements sort, status, demographic and include/exclude tags.
- `crates/raygun-ext-host`: extension host (QuickJS via rquickjs on one host thread, host API in `src/prelude.js`). `third_party/rquickjs-sys` is a vendored copy that adds Android bindings.
- `extensions/`: first-party extensions (TS on `@raygun/sdk`), compiled into the app from their committed `dist/` bundle (`raygun-app/src/extensions.rs`); rebuild with `pnpm build:extensions` (a test fails when a bundle is stale). An installed copy in the data root's `extensions/<id>/` overrides the built-in one.
- `repo/`: Raygun's official extension repository (`index.json` + `packages/`), served from GitHub raw and seeded into every install. Generated by `raygun-ext pack`; never hand-edit it.
- `packages/sdk`: `@raygun/sdk` (extension types/helpers, MIT) and the `raygun-ext` CLI (`init`, `build`, `dev --android`). Debug app builds poll `extensions/` and hot-reload; run `pnpm dev:android` inside an extension folder (in a Terminal-panel tab) to push every save to the emulator.
- `packages/nuclear-compat`: converts Nuclear player plugins (TS) into Raygun extensions at install. The converter and shim run in QuickJS; the bundle is committed at `crates/raygun-ext-host/assets/nuclear-converter.js`. Rebuild it with `pnpm --filter @raygun/nuclear-compat build` (a test fails when it is stale).
- `plugins-mobile/raygun-audio`: native Android audio engine (Media3 in a `MediaSessionService`; reports reach Rust over JNI). The app shell must depend on each mobile plugin directly, or Tauri won't include its Android module.

## Commands

```bash
pnpm install
pnpm dev                 # desktop
pnpm android:dev         # Android emulator (needs ANDROID_HOME and NDK_HOME)
pnpm lint && pnpm type-check && pnpm test
cargo test --workspace   # includes a check that bindings are current
cargo clippy --workspace --all-targets -- -D warnings
pnpm format && cargo fmt --all
pnpm bindings            # regenerate TS bindings after changing commands/types
cargo test -p raygun-ext-host -- --ignored   # live extension tests (network)
pnpm repo:publish        # pack first-party extensions into repo/ (the official repository; bump versions first)
```

## Android tooling gotchas (for agents)

- Run `tauri android dev|build` in a real terminal tab (Terminal panel), not a headless shell. Without a TTY, the CLI's device prompt loops forever (it once wrote a 13 GB log), and `adb shell` blocks on an open stdin pipe.
- Only one Tauri Android command can run at a time. A stuck one shows `Blocking waiting for file lock on Android` in the next. Find leftover `tauri.js android ...` node processes and stop them.
- Verify on the emulator with `adb install -r .../app-universal-debug.apk`, `adb shell am force-stop app.raygun.player`, a relaunch, then `adb exec-out screencap -p`. App logs: `adb shell "run-as app.raygun.player cat logs/<file>"`.
- Env for headless runs: `ANDROID_HOME=%LOCALAPPDATA%\Android\Sdk`, `NDK_HOME=...\Sdk\ndk\28.2.13676358`.

## Rules

- Domain state and logic live in Rust services. The UI calls generated `commands.*` and holds UI-only state.
- Platform priority: Android only for now (`Pixel_4` emulator). Don't build or verify desktop until the user asks; keep the code cross-platform.
- Desktop builds must be portable: never call OS app-dir APIs or `tauri-plugin-store`. Go through `raygun-paths` (M02).
- Synced tables use UUIDv7 ids, HLC timestamps and tombstones (ARCHITECTURE §5, §10).
- Adapted code from a reference repo keeps a header naming the source repo and its license.
- TS: `type` over `interface`, `const Component: FC<Props>`, no one-letter names. Tests use RTL queries and userEvent, and mock only the bindings/IPC layer.
