# Raygun on the web

Raygun's own interface, built for a browser tab and published with GitHub Pages
(`https://<owner>.github.io/raygun/`). It keeps its library, settings and history in the browser,
links and syncs with the Raygun apps on phones and computers, finds and plays music through Raygun's
own sources, and installs as a web app.

```bash
pnpm --filter @raygun/app dev:pages      # dev server: http://localhost:1430/raygun/
pnpm --filter @raygun/app build:pages    # apps/raygun/dist-web (RAYGUN_WEB_BASE sets the base path)
pnpm --filter @raygun/app preview:pages  # serves dist-web: http://localhost:1431/raygun/
```

The Tauri builds are unchanged: `vite.config.ts`, `index.html` and `src/main.tsx` stay theirs.
`vite.pages.config.ts` extends that config and swaps the page's entry for `src/web/entry.tsx`.

## Why this shape

Raygun's logic is Rust behind Tauri commands. Three ways to put it in a browser were weighed:

- **Rust compiled to WebAssembly.** The pure crates would compile (`raygun-model`,
  `raygun-playback`, `raygun-resolver`), but the services they serve don't: `raygun-app` is built on
  Tauri, tokio, SQLite through sqlx, QuickJS and the network stack, and pieces like
  `raygun-playback` need clocks and randomness that `wasm32-unknown-unknown` lacks without
  wasm-bindgen. It would mean a second build toolchain for every web build and still a browser
  version of every service.
- **The page as a remote for a running Raygun.** A page served over https can't call `http://`
  addresses on the local network (mixed content), nor open sockets. It would need a relay server,
  and do nothing without a device nearby.
- **A web core in the page, plus sync** (chosen). The page answers the commands the interface sends
  itself, for what a browser can do, and links with the apps for the rest of the user's world. A
  browser can reach a device on the local network over WebRTC, so the apps gained a WebRTC
  listener for device sync, and nothing else is needed: no server, no relay.

## The web core (`apps/raygun/src/web/`)

- `ipc.ts` stands in for Tauri's IPC: `window.__TAURI_INTERNALS__.invoke` and the event plugin.
  The generated bindings (`packages/bindings`) are used unchanged; commands go to the core by their
  Rust names, events come back through the bindings' own `listen`. A thrown string is a command's
  error, as Rust's `Err` is.
- `core/index.ts` routes commands to services. Commands without a web implementation get an
  answer of their own type from `fallback.ts`: reads get an empty value (`[]`, `null`, `false`,
  objects field by field), actions answering a `Result` get the error "This isn't available in
  Raygun on the web". The empty values are made at build time from the bindings' types by
  `web-build/commandAnswers.ts` (the TypeScript compiler; a virtual module), so new commands are
  covered without changes here.
- Storage (`core/store.ts`): IndexedDB (`raygun-web`), read into memory when the page opens and
  written through on each change. A version that lacks a table gets it on the next open. The page
  asks the browser to keep it (`navigator.storage.persist()`); a private window that refuses
  storage still runs, keeping nothing.
- Settings (`core/settings.ts`): one synced `setting` row per key with the value as JSON, as
  Rust keeps them, over Rust's defaults: `pnpm bindings` writes
  `packages/bindings/src/settings-defaults.json` beside the bindings, and the bindings test fails
  when it is stale. The window frame is the browser's own.
- Favorites, playlists, history and what was played from (`core/collections.ts`): the synced
  tables with Rust's columns, HLC stamps (BigInts: they don't fit a JS number), tombstones and
  order rules (positions, midpoints, renumbering), song identity from `music/trackKey.ts`.
- Playback (`core/playbackCore.ts`, `core/playback.ts`): `raygun-playback`'s state machine ported
  line for line, and a small service that finds what each song plays from and drives the page's
  HTML5 and YouTube engines (`engine/useEngineHost`) with the same `EngineCommand`s and reports as
  the desktop app. YouTube songs play in the embed host page from `raygun-net` (served as
  `embed/youtube.html`). Plays count for history as in `raygun-app` `history.rs`. The queue is kept
  and comes back paused.
- Device sync (`sync/`), sources (`extensions/`): below.

`appInfo().platform` is `web`. Screens that can't work in a tab hide their parts there (Devices
shows no sharing switch, no nearby devices and no code of its own).

## Sync with the apps

A browser can't be reached, so it always connects; the apps listen.

- **Listener** (`crates/raygun-sync/src/web.rs`): an ICE-lite WebRTC peer (str0m) on UDP
  8840–8849, on every LAN address and loopback, with fixed ICE credentials and a fixed DTLS
  certificate kept in `local_meta` (`sync_web_config`), so everything a browser needs fits in the
  link code and nothing has to travel back before the connection is up. Each browser gets a
  negotiated data channel (id 0). Its bytes are a sync connection exactly as over TCP: the Noise
  `XX` handshake, then the length-prefixed JSON messages, handled by the same `Engine` code
  (`Channel` is generic over its stream).
- **Link code**: while sync runs, the code Devices → Show code makes (`LinkInvite`) also carries
  `sync`, `key`, `pair` (a sync pairing code), `ice`, `fp` (the certificate's SHA-256), `udp` and
  `ips`. Apps reading it pass over those fields. The browser scans it (camera) or takes it pasted,
  builds the app's side of the connection from it (`sync/webrtc.ts`), checks the app's Noise key
  against the code before sending anything, and pairs with the code.
- **Security**: the browser's DTLS certificate can't be known beforehand, so the app doesn't check
  it; the Noise handshake inside authenticates both ends (the browser checks the app's key from the
  code; the app knows the browser by its key once paired with the one-time code), as over plain TCP.
- **Crypto**: str0m's own RustCrypto backend pulls in aws-lc for certificates, so the listener
  gives str0m dimpl's RustCrypto backend directly and makes its certificate with rcgen on `ring`
  (already in the tree). Browser connections carry no media; SRTP is never keyed.
- **Browser side**: Noise XX in TypeScript (`sync/noise.ts`, `@noble` ciphers, curves and hashes),
  the same framing and messages (`sync/channel.ts`, JSON keeping 64-bit integers whole), the merge
  rules of `raygun-db::sync` (`sync/merge.ts`: last writer wins by `(updated_at_hlc,
  origin_device)`, one live favorite per song, device-only settings stay), sessions and peers
  (`sync/engine.ts`). It syncs on changes and every five minutes while the page is open and shown.
- **On the apps**: a browser is a sync peer without an address; the apps skip it when they sync
  with linked devices, and it syncs whenever it connects. Android 17's local network permission
  covers the listener as it covers the sync server.

Check it from a PC: `cargo run -p raygun-sync --example peer -- <dir> serve --bind 127.0.0.1
--web` prints a link code for browsers; paste it into Devices → Scan a link code.

## The app as a back end

While a linked app is reachable, the page uses it for everything a browser can't do itself: every
source and format the app runs (Raygun's extensions without the browser's CORS limits, Stremio,
Kodi, Mihon, Shosetsu, Nuclear, servers), searches, catalogues and readers, and downloads, which
land on that device. The page keeps its own library, settings, history and player, which sync
anyway.

- **Channel**: the browser opens a second negotiated data channel (id 1, `raygun-remote`) on a
  connection of its own and runs the same Noise `XX` handshake on it (`raygun-sync` `remote.rs`).
  The app answers only a browser whose key is a paired device's, then keeps the channel open both
  ways. Each frame's plaintext starts with a kind byte: JSON in parts, or a piece of a body (a
  request id, then bytes). Either lane starts with its first bytes, and the listener queues what
  doesn't fit SCTP's send buffer until the browser acknowledges more.
- **Commands** (`raygun-app` `web_remote.rs`): `call {id, cmd, args}` runs through Tauri's own
  invoke path on the main window's webview (`Webview::on_message`, the app's invoke key), so a
  command answers exactly as for the app's own screens; the answer is `ok`, `err` or `denied`.
  Denied: the app's settings, player, windows, pairing, backups and data, permissions, files
  outside Raygun's data, and what each side keeps itself (library, history, quick access)
  (`DENIED`, `DENIED_PREFIXES`; a test checks each names real commands). While the app's window is
  closed (Android, swiped away) commands answer that Raygun needs opening there.
- **Events**: the events the page's screens follow (sources, searches as they answer, downloads,
  catalogue and podcast changes) come as `event {event, payload}`; the app's player, prompts and
  settings stay its own.
- **Pictures and media**: answers carry the app's proxy links (`http://127.0.0.1:<port>/cover/…`).
  The page shows them as its own `__remote/…` paths (`web/remote/rewrite.ts`) and turns them back
  into the app's form in what it sends or keeps, so stored and synced rows read the same
  everywhere. The service worker (`web-build/remoteWorker.ts`) answers those paths, and app links
  still in their own form, by asking the page over a MessagePort; the page fetches through the app
  (`fetch {id, path, range}` → `head`, body pieces, `end`, from the proxy on loopback, its routes
  only) in 512 KB ranges as the browser reads, so audio and video seek and stream
  (`web/remote/media.ts`). HLS and DASH playlists get their links rewritten on the way.
- **Page side** (`web/remote/`): `backend.ts` connects to the chosen linked app (else the one
  synced with last), sets the web core's `setRemoteHandler`, and sends commands there unless the
  page keeps them (`routing.ts`) or the app denied them; when the app goes away mid-call or before,
  the page answers by itself, and it tries again after 2 s, 5 s, 15 s, 30 s, then every minute
  (and when the tab is shown or the network comes back). `web/remoteState.ts` tells the screens:
  Devices has a "Sources from your apps" switch and a choice of app, Browse says "via <device>".

Check it from a PC: `peer -- <dir> serve --bind 127.0.0.1 --web --remote` answers the back-end
channel as a stand-in (the command `test_echo`, any proxy path as a 3 MB file with ranges).

## Sources

Raygun's own JavaScript extensions run in the browser (`extensions/`). Each runs in an iframe
sandboxed without `allow-same-origin`: an opaque origin with no access to Raygun's storage,
cookies or page. Inside, the `raygun` host API is built as the apps' QuickJS prelude builds it, over
the browser's `fetch`, `DOMParser` and timers, with the manifest's network permissions enforced;
the page calls providers by `postMessage`. Packages come from repositories (Paraline by default),
checked against their SHA-256 and unpacked in the browser.

The browser makes the requests, so a source works only where its site lets web pages read it
(CORS). Among the first-party sources' APIs, the Internet Archive (and so its music, Live Music
Archive and Great 78 sources), MusicBrainz, ListenBrainz, Wikimedia and fyyd do; incompetech,
LibriVox and gutenberg.org don't. A source that fails says why. The web core wires them into Search
→ Music (`online_search`) and into playback (`music.stream` candidates for songs found elsewhere).
Other formats (Nuclear, Shosetsu, Kodi, Mihon, Stremio addons) need the app.

Sessions and cookies: a sandboxed source sends no cookies and keeps none (the browser doesn't let
a page set other sites' cookies), so sources that need a sign-in work only with keys set in their
settings. Extensions' own storage (`raygun.storage`) is kept per extension in IndexedDB.

## Installable

The build adds `manifest.webmanifest`, icons and a service worker (`sw.js`, written by
`web-build/plugin.ts` after the build) that keeps the app's files for starts without a network:
the page itself is asked for first, the hashed files come from the cache. Routes live in the URL
fragment, so Pages needs no fallback page.

## What works and what doesn't

Works: the interface, settings, favorites, playlists, history and what was played from (kept and
synced); linking and syncing with the apps; Search → Music and playback through Raygun's sources
that allow web pages; songs with a stream address or a YouTube id; the queue and its restore;
YouTube's player; installing as a web app. With a linked app reachable, also its sources in every
format, catalogues, searches, readers' pages and downloads to that device.

Not in the browser, and why:

- Files on this computer: not yet (the next step: File System Access for folders, files copied
  into IndexedDB elsewhere). Files on other devices can't be reached.
- Comics, books, video sources, podcasts and audiobooks: their sources' catalogue commands aren't
  wired into the web core yet; podcast feeds are mostly not readable by web pages (CORS).
- Downloads, exports, recordings, yt-dlp, torrents, casting, the LAN remote, Connect (playing on
  another device), on-device AI (transcription, translation, OCR) and the apps' system integrations:
  they need the app; downloads and exports work through a linked app, landing on it.
- Through a linked app, playing a title's videos (they open the app's own player, so they're
  refused) and songs whose stream only the app's sources find (the page's player looks among its
  own sources) aren't wired yet.
- Sources whose sites don't allow web pages, and sources in other formats: they need the app.

## Publishing

`.github/workflows/pages.yml` builds `dist-web` and publishes it on pushes to `main` (and when
run by hand). GitHub Pages must be switched on once for the repository with GitHub Actions as its
source (Settings → Pages, or `gh api -X POST repos/<owner>/<repo>/pages -f build_type=workflow`).
The `github-pages` environment allows the default branch to publish.

## Next steps

- Local files: File System Access (`showDirectoryPicker`) where the browser has it, files kept in
  IndexedDB elsewhere, played as object URLs; `local_folders_*` and the library commands.
- Catalogue sources (comics, books, videos) through the same sandboxes and the readers.
- More of the shared logic as WebAssembly where it pays off (the resolver's ranking).
