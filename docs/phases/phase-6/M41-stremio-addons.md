# M41 — Stremio addons in the Watch tab

**Relevant docs:** ARCHITECTURE §6.4 and the stremio-core notes in `docs/reference-notes.md`, M38 (player, Continue watching), M40 (search types), `docs/feature-parity.md` (Watch).

## Goal
- **A Watch tab worth opening:** movie and series catalogs, title pages with seasons and episodes, and streams and subtitles, all from Stremio addons.
- **The addons:** Raygun suggests Cinemeta and OpenSubtitles, and the user can add any addon by URL. Search gains a Videos type.

## Design
- **Protocol** (`raygun-app::stremio::protocol`, pure and tested): the HTTP addon transport, with types modeled on stremio-core's (MIT).
  - **Manifest:** `<base>/manifest.json`.
  - **Resources:** `<base>/<resource>/<type>/<id>[/<extra>].json` for `catalog` (extras `search`, `genre`, `skip`), `meta`, `stream` and `subtitles`.
  - **Leniency:** a resource may be a name or an object with its own types and idPrefixes; a catalog's extras may be required (`isRequired` or `extraRequired`). Unknown fields are ignored and broken entries dropped.
- **Storage** (`0014_stremio.sql`, synced conventions): `stremio_addon` holds the transport URL, cached manifest JSON, its fetch time and a position. Fresh installs get Cinemeta and OpenSubtitles v3.
- **`StremioService`:**
  - **Addons:** list, add (`stremio://` and bare hosts accepted; the manifest is fetched and checked first), remove, and refresh manifests older than a day. Adult addons (`behaviorHints.adult`) are hidden while NSFW is off.
  - **Home:** every catalog that needs no extra, first page, catalogs in parallel; each failing catalog says so on its own row.
  - **Catalog page:** one catalog with its genre options and paging (`skip`).
  - **Search:** every catalog with a `search` extra.
  - **Meta:** the first addon (in order) that serves `meta` for the type and id prefix. Series episodes are grouped by season; trailers come from YouTube ids.
  - **Streams:** every `stream` addon for the type and id prefix, in parallel, results per addon. Each stream is classified:
    - **Direct:** `url`, with `behaviorHints.proxyHeaders.request` and stream subtitles; plays in the M38 player.
    - **YouTube:** `ytId`; plays in the embedded YouTube player.
    - **External:** `externalUrl`; opens outside.
    - **Torrent:** `infoHash`; listed but "not supported yet".
  - **Subtitles:** gathered from every `subtitles` addon when a stream plays. The UI language comes first, at most 20 tracks (see Decisions).
  - **Play:** the content key is `stremio:<type>:<videoId>`, so Continue watching and episode progress work as in M38. The title is the show's name; the subtitle is "S1 E2 · Episode name".
- **UI:**
  - **Watch:**
    - Continue watching;
    - a row per catalog, with See all;
    - Play a video link;
    - an Addons button.
  - **Title page** (`/watch/title?type=&id=`):
    - backdrop, poster, year, runtime and rating, genres, description and cast;
    - a Trailer button;
    - for movies, the streams;
    - for series, a season picker, episodes with progress, and an episode's streams.
  - **Catalog page** (`/watch/catalog`): a grid with a genre picker and Load more.
  - **Addons page** (`/watch/addons`): installed addons with what they provide, add by URL, suggestions and remove.
  - **Search:** a new Videos type with rows per addon catalog.

## Out of scope
- **Torrent streams:** they need a torrent engine. **Addons that restrict their use:** WatchHub answers that only clients Stremio authorised may use it, so it isn't suggested.
- **Stremio account sync, library and addon configuration pages** (`configurationRequired`). Raygun's own video library and watched-state sync come in M43.

## Done-check
- **Rust:**
  - manifests (resource names and objects, required extras, adult flag, bad entries dropped) and resource URLs (extras encoded);
  - catalog, meta (episodes by season) and stream parsing (direct with headers, YouTube, external, torrent);
  - subtitles ordering;
  - URL normalising.
- **Vitest:**
  - Watch shows catalog rows and opens a title;
  - a series title lists seasons and episodes and finds an episode's streams;
  - playing a direct stream sends the request with headers and subtitles;
  - torrent streams are marked unsupported;
  - the Addons page adds and removes;
  - Search → Videos shows addon results.
- 📱 **On the emulator:**
  - Watch shows Cinemeta's Popular movies and series;
  - a series shows seasons and episodes;
  - a local test addon (served from the PC, reached through `adb reverse`) provides a direct HLS stream, which plays with OpenSubtitles subtitles available and resumes from Continue watching;
  - Search → Videos finds a movie;
  - clean up afterwards.

## Decisions
- **Subtitles go through the stream proxy** (`/subtitle/<token>` in `raygun-net`).
  - Media3 fetches a side-loaded subtitle when it's selected, and gives up on the whole video when that fails. The proxy serves an empty WebVTT track instead of a failed file (404, a web page, too large, timed out), so a dead link costs a track, not the film.
  - Release builds allow plain HTTP only to loopback, so addon subtitles on `http://` load through the proxy too.
  - This applies to every video, including M38's Play a link. The format is guessed from the original URL, since the proxied one has no extension.
- **At most 20 subtitle tracks**, down from the planned 40: a menu of 40 near-identical entries isn't usable, and the UI language comes first anyway.
- **The add-addon field is a text field with a URL keyboard**, not `type="url"`: the webview's own validation refused bare hosts (`addon.example`), which the service accepts.
- **Marked-watched entries without a play** keep an empty request; resuming one from History says to open it from its page to choose a stream.
- **Manifests refresh lazily:** when an addon list is needed and its cached manifest is older than a day. An unreachable addon keeps working from its cached manifest.
- **Requests to addons** time out after 20 s and stop reading after 8 MB; the home shows at most 24 catalogs. A failed request says why (its innermost cause, e.g. "operation timed out"), not reqwest's generic "error sending request".
- **Search rows** are labelled by kind ("Movies", "Series") and addon: an addon's search catalog keeps its browse name ("Popular"), which says nothing about the results.
- **Android 17 local network protection** (fixed in M41.1, which requests `ACCESS_LOCAL_NETWORK`): with target SDK 37, apps can't connect to private (RFC 1918) addresses without the new local-network permission, which Raygun didn't request yet. Addons on the LAN (self-hosted, `http://192.168.…`) can't be reached for now; public and loopback addresses work. The emulator check therefore reaches the PC's test addon through `adb reverse tcp:7010 tcp:7010` and `http://127.0.0.1:7010`, not 10.0.2.2.

## Verification
- **Checks:** `cargo test --workspace`, `cargo clippy --workspace --all-targets -D warnings`, `pnpm lint`, `pnpm type-check` and `pnpm test` all pass. New tests:
  - the protocol module (manifests, URLs and addresses, catalogs and meta, streams and subtitles);
  - `raygun-db::stremio` and episode progress by prefix;
  - the proxy's `/subtitle/` route (served, refused, missing, unknown);
  - subtitle formats from URLs;
  - five Vitest cases (`watch/Stremio.test.tsx`).
- 📱 **Pixel_4 emulator (Android 17), debug build:**
  - **Watch home:** Cinemeta's Popular, Featured and other rows with posters, seeded on a fresh install.
  - **Addons page:**
    - lists Cinemeta and OpenSubtitles v3 with what they provide;
    - a bare host that only speaks HTTP fails after the time-out and says why ("operation timed out");
    - the local test addon (Python, on the PC, through `adb reverse`) adds and its row appears on the home;
    - removing it works.
  - **Big Buck Bunny from the test addon:**
    - the title page shows facts, genre and source;
    - the picker shows the HLS stream and marks the torrent "Torrent streams aren't supported";
    - the HLS stream plays in the native player;
    - the subtitle menu lists English and French. Selecting the dead French link fetched it through the proxy (404 → empty track) and playback went on. English loaded through the proxy.
  - **Continue watching:** the film appears with its progress and resumes at 3:45; the title page then offers Resume with a progress bar.
  - **A Cinemeta series (Carrie):**
    - episodes with thumbnails, dates and overviews;
    - marking episode 1 watched fills its check;
    - episode 2's picker ("Carrie · S1 E2 · …") plays the test stream;
    - OpenSubtitles offered English tracks first, then other languages, and the English one rendered on screen;
    - Continue watching shows "S1 E2 · FOREIGN LANGUAGE" with the episode's thumbnail.
  - **Catalog page:** Popular · Movies shows a grid; the Animation genre changes the results; More loads the next page.
  - **Search → Movies & series:** "matrix" finds the Matrix films and series from Cinemeta.
  - **Clean-up:** the app's data was cleared afterwards (`pm clear`), the port reverse removed and the test addon stopped.
