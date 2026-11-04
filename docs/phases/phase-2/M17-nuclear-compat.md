# M17 — Nuclear plugin compatibility

**Relevant docs:** ARCHITECTURE §7.4 (converter + shim, limitations), §7.2 (runtimes), M11–M14 (host, SDK, repositories, resolver), reference-notes (Nuclear plugin model). M16 (yt-dlp, desktop only) is skipped while Raygun is Android-only.

## Goal
Unmodified provider plugins from Nuclear's plugin registry install and run in Raygun:
- metadata plugins answer Search;
- streaming plugins feed the resolver;
- lyrics plugins feed the lyrics view.

Each plugin gets a compatibility report that says which parts work.

## What Nuclear plugins look like (from the four test plugins and Nuclear's loader)
- **Package:** a zip of `package.json` (`main`, `nuclear { displayName, categories, permissions }`) plus `src/*.ts`, with relative imports. The only bare import is `@nuclearplayer/plugin-sdk`. UI plugins also import `react` and `@nuclearplayer/ui`.
- **Entry:** a default export `{ onLoad?, onEnable?, onDisable?, onUnload? }`, each receiving a `NuclearPluginAPI`.
- **Providers:** registered through `api.Providers.register`:
  - `metadata`: `search(params)` → `{ tracks, artists, albums }`, plus `searchTracks` and others;
  - `streaming`: `searchForTrack(artist, title, album)` → `StreamCandidate[]`, and `getStreamUrl(id)` → `Stream { url, protocol }`;
  - `lyrics`: `getLyrics(track)` → `Lyrics` (plain, line-synced, word-synced, instrumental).
- **Network:** `api.Http.fetch`, a `fetch`-like function returning a `Response`. The plugins also use browser globals: `URL`, `URLSearchParams`, `Headers`.

## Design

### Converter (`packages/nuclear-compat`, runs inside QuickJS at install)
- Each file is transpiled with **Sucrase** (MIT; transforms `typescript`, `jsx` and `imports`), giving one CommonJS module per file. Test files and `.d.ts` files are skipped.
- **Output:** a single plain script for Raygun's existing `js` runtime. It contains:
  - the compatibility shim;
  - a tiny module table with `require`: relative paths with extension and `index` resolution; the bare modules are mapped to the shim or to stubs;
  - a bootstrap that runs the plugin's `onLoad` and `onEnable` and publishes the ready promise as `globalThis.__rg_ready`. The host awaits that promise, with the load timeout, so asynchronous registrations count.
- **Generated Raygun manifest:**
  - id `nuclear.<package-name>`, display name, version, description;
  - `provides` from the plugin's categories: metadata → `music.metadata`, streaming → `music.stream`, lyrics → `lyrics`;
  - network permissions = the hosts found in string literals of the source;
  - the original package metadata kept under `nuclear`.
- **Compatibility report:**
  - supported provider kinds;
  - unsupported categories (dashboard, playlists, discovery, scrobbling, other);
  - API domains the plugin uses that Raygun stubs (Queue, Playback, Favorites, Playlists, Dashboard, Shell, Navigation, Ytdlp, Events);
  - UI imports (React, `@nuclearplayer/ui`), which are inert here;
  - detected hosts.
  - A plugin with **no** supported provider kind is rejected at install, with the report as the reason.
- **Packaging:** the converter is built with esbuild into `crates/raygun-ext-host/assets/nuclear-converter.js` and committed, like the extension bundles, with a staleness test. Rust runs it in a throwaway QuickJS context (`nuclear::convert(files)`).

### Shim (`NuclearPluginAPI` over the Raygun host API)
- **`Http.fetch`:** `raygun.http.fetch`, wrapped in a minimal `Response` (`ok`, `status`, `statusText`, `url`, `headers.get`, `text()`, `json()`). `Headers`, `URL` and `URLSearchParams` polyfills come with it.
- **`Logger`:** maps to `raygun.log`.
- **`Settings`:** `register(defs)` records defaults; `get` and `set` use `raygun.storage`.
- **`Providers.register`:** wraps each provider as a Raygun provider:
  - `metadata` → `music.metadata.search`, through `search` (types `tracks`) or `searchTracks`;
  - `streaming` → `music.stream.candidates`, through `searchForTrackV2` or `searchForTrack`, and `resolve` through `getStreamUrl`, giving `{ kind: 'url', url }`; HLS and file streams are skipped;
  - `lyrics` → `lyrics`. Line- and word-synced sections flatten to timed lines; plain sections join to text; instrumental means none.
- **Everything else:** a stub that throws "`<Domain>` isn't available in Raygun yet". The report lists the domains each plugin uses.

### Install sources
- **Nuclear's registry as a repository:** `https://cdn.jsdelivr.net/gh/NuclearPlayer/plugin-registry@master/plugins.json`.
  - The repository parser recognizes this format (`{ version, plugins: [{ id, name, downloadUrl, sha256?, categories }] }`).
  - Entries are marked `nuclear`, and network access shows as "detected at install".
- **Installing one:**
  1. Download the zip (10 MiB cap; `sha256` checked when the registry lists one).
  2. Convert it, write the generated extension under `extensions/<id>/`, and record the install.
- Browse shows a "Nuclear" badge. The extension's page shows the compatibility report.
- The registry isn't seeded; it is added by hand under Repositories. Nuclear's plugins are third-party code.

## Done-check
- **Converter, Vitest in `packages/nuclear-compat`:**
  - module table and `require` resolution;
  - provider wrapping for all three kinds, against a fake `raygun` global;
  - lyrics flattening;
  - the `URL` / `URLSearchParams` polyfills;
  - the report.
- **Rust:**
  - converting the **four real registry plugins** (MusicBrainz, LRCLIB, Kugou, Discogs; AGPL-3.0, kept as test fixtures with attribution) into extensions that load;
  - the LRCLIB plugin answering lyrics against the fixture server;
  - the Nuclear registry format parsing;
  - a UI-only plugin rejected with its report.
- 📱 On the emulator:
  - add Nuclear's registry;
  - install the MusicBrainz plugin; its results appear in Search and play through the resolver;
  - the extension page shows the compatibility report.

## Verification (2026-10-31, Pixel_4 emulator)
- **Vitest, `packages/nuclear-compat` (13 tests):** module table and `require`, provider wrapping for the three kinds, lyrics flattening, the `URL`/`URLSearchParams`/text/base64/`Headers` polyfills, Blob → `DecompressionStream` → `Response` (the Kugou path), the report, and a check that the committed converter bundle is current.
- **Rust:** the four real plugins convert and load; the LRCLIB plugin answers lyrics from the fixture server; the Nuclear registry parses, its ids match and checksums are checked; a dashboard-only plugin is refused with its report.
- 📱 **On the emulator:**
  - Nuclear's registry added under Repositories shows as "Nuclear plugins". Browse lists its plugins with a Nuclear badge; dashboard-only ones are left out. The install sheet explains the conversion and the detected hosts.
  - Installing the MusicBrainz plugin ran Sucrase inside QuickJS on the device. The log shows `hosts=[coverartarchive.org, en.wikipedia.org, musicbrainz.org, www.wikidata.org]` and the extension loading with 1 provider.
  - Searching "blue in green davis" gave 30 tracks from the plugin next to Raygun's own MusicBrainz. Tapping one played "Blue in Green" through the resolver (Internet Archive candidate, score 82).
  - The extension page lists the four hosts, the provider and "Converted from the Nuclear plugin nuclear-plugin-musicbrainz. Everything this plugin provides works in Raygun."

## Decisions and notes
- Both MusicBrainz sources are called "MusicBrainz", so Search adds the extension id to a group's header when two groups share a name. This is covered by a Vitest test; it isn't on the device build yet.
- The plugin doesn't pace its requests. Searching while Raygun's own MusicBrainz extension runs can get a 503 from MusicBrainz's one-request-per-second limit; the plugin's group then shows the error and the next search works. Pacing per host in the host's `http.fetch` would fix it for every extension; it's left for later.
- Plugin search results carry no artwork, because the plugin's track search doesn't return covers.
