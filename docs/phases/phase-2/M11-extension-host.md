# M11 — Extension host (QuickJS)

**Relevant docs:** ARCHITECTURE §7.1 (manifest), §7.2 (runtimes), §7.3 (host API), §12 (content policy). ROADMAP Phase 2.

## Goal
Raygun runs sandboxed JavaScript extensions that register **providers**, and the app calls those providers from Rust. A bundled first-party extension makes the Search page useful: online search returns tracks that play straight away.

## Design

### Crate `raygun-ext-host`
- **Manifest** (`raygun-extension.json`):
  - Fields: `id` (reverse-DNS), `name`, `version` (semver), `apiVersion` (must be `1`), `runtime` (only `"js"` for now), `entry`, `provides`, `permissions { net: [host patterns], webview }`, `contentRating`, `settings: [{ key, type, default, ... }]`.
  - Validated at load. A bad manifest becomes an extension in the `failed` state with the reason, never a crash.
- **Net permissions:**
  - A pattern is an exact host (`archive.org`) or a wildcard (`*.archive.org`, which matches subdomains only).
  - Only `http` and `https` are allowed. Every request **and every redirect hop** is checked. Each extension has its own reqwest client, whose redirect policy enforces that extension's patterns.
  - Response bodies are capped at 8 MiB.
- **Runtime** (rquickjs 0.14, async API):
  - One `AsyncRuntime` + `AsyncContext` per extension: no shared globals, 64 MiB memory limit, 1 MiB stack.
  - All extensions live on **one host thread**, which runs a current-thread tokio runtime and a `LocalSet`. Requests arrive over a channel and each runs as a local task, so JS values never cross threads and one extension's network wait doesn't block another.
  - An interrupt handler stops synchronous JS that runs longer than 5 s within one call. Each provider call has an overall 30 s timeout.
- **Host API** (the `raygun` global, plus `console`):
  - `raygun.log.{debug,info,warn,error}`; `console.*` maps onto it.
  - `raygun.http.fetch(url, { method, headers, body })` → `Promise<{ status, ok, url, headers, text(), json() }>`, permission-checked.
  - `raygun.html.select(html, selector)` → `[{ text, html, inner, attrs, select(sub) }]` (`scraper` crate).
  - `raygun.storage.{get,set,remove,keys}`: per-extension JSON key–value store, kept at `extensions/.data/<id>/storage.json` (outside the code folder, so updates in M13 can replace code freely). Capped at 1 MiB.
  - `raygun.settings.get(key)`: the value set in the app, otherwise the manifest default. (Values set in the app arrive with the settings UI in M13.)
  - `raygun.providers.register({ kind, id, name, ...methods })`.
  - Plain JS is accepted for now; the `@raygun/sdk` types come in M12.
- **Providers:**
  - The registry is kept in Rust (extension, kind, id, name).
  - `call(provider, method, args)` sends JSON in and returns JSON out, awaiting the promise the method returns.
  - `music.metadata` is the first typed kind: `search({ query, limit })` → `{ tracks: [...] }`. A track may carry a playable `source` (a `MediaSource`); the resolver pipeline (M14) fills in tracks that don't have one.
- **Sources of extensions:**
  - Built-in extensions are compiled into the app.
  - Installed extensions are loaded from `extensions/<id>/`. An installed extension with the same id **overrides** the built-in one, which allows development builds and testing on a device.
  - Repositories and the install UI come in M13.

### First-party extension `extensions/internet-archive`
- Searches the Internet Archive's public audio collections (public-domain and Creative Commons recordings) through the advanced-search API.
- Fetches each hit's file list concurrently and returns MP3 tracks with direct stream URLs.
- Permissions: `archive.org` and `*.archive.org`.

### App
- **`ExtensionService`:** loads the built-in and installed extensions at startup.
- **Commands:**
  - `extensions_list` lists each extension: status, permissions and providers.
  - `online_search(query)` asks every `music.metadata` provider in parallel and returns per-provider results or an error.
- **Search page:** a debounced input. Results are grouped by provider; tapping a track plays the results list from that track. Error and empty states are shown.
- **Settings → Extensions:** a read-only list of name, version, source (built-in or installed), status and network permissions.

## Done-check
- Rust tests cover:
  - manifest validation;
  - host-pattern matching;
  - the JS runtime: register and call a provider; async fetch against a local test server; a denied host, including a redirect to one; the html select API; storage persistence; the CPU interrupt; and a thrown error surfaced as a provider error.
- The Android build compiles QuickJS for `x86_64-linux-android` using the vendored `rquickjs-sys` bindings.
- Vitest covers the Search page states and the Extensions list.
- 📱 On the emulator:
  - Searching returns Internet Archive tracks, and tapping one plays it through the native engine.
  - An extension pushed into the sandbox's `extensions/` overrides the built-in one, and Settings shows it as installed.

## Verification (2026-10-31, Pixel_4 emulator)
- **Rust:** 15 `raygun-ext-host` tests cover everything listed in the done-check, plus a sandbox check: no `__rg`, `require`, `process`, `std`, `os` or global `fetch`. Workspace total: 103 tests, clippy `-D warnings` clean.
- **Live test:** an opt-in test runs the real extension against archive.org: `cargo test -p raygun-ext-host -- --ignored internet_archive`. "louis armstrong" returned 13 MP3 tracks with titles, artists and durations.
- **Vitest:** 65 tests (7 new) cover the Search page states and the Extensions card.
- **Android build:** the debug APK for `x86_64` builds, and QuickJS compiles with the vendored `rquickjs-sys` Android bindings. On startup the log shows `extension loaded extension=dev.raygun.internet-archive providers=1`.

| Check | Result |
|---|---|
| Searching "louis armstrong" | Internet Archive results with artwork, artist · album and durations |
| Tapping "What A Wonderful World" | Native engine plays it; the media session reports PLAYING with the track's metadata |
| `KEYCODE_MEDIA_NEXT` | Moves to the next search result ("C'est Si Bon") |
| Copy with `version 1.0.1-dev` pushed into `extensions/internet-archive/`, then restart | Settings shows "Internet Archive (dev) 1.0.1-dev installed" (it overrode the built-in copy) |

## Decisions
- **Calls into one extension run one at a time.** The runtime lock is held while a call awaits its promise. A provider can still issue its own requests in parallel (`Promise.all`), and different extensions run concurrently. Lifting this means keeping promises alive across lock releases; that waits until a real extension needs it.
- **Nested `select()`** re-parses the element's HTML as a fragment. Table parts (`<tr>`, `<td>`) lose their context that way, so select them from the document directly.
- **Response bodies are decoded as UTF-8** (lossy). Charset sniffing can come with the SDK's binary/encoding helpers (M12).
- **Extension JS is plain scripts for now.** `@raygun/sdk` (M12) adds types and a bundler, so extensions can be written in TS with imports.
