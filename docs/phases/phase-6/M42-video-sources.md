# M42 — Video sources (`video.source`)

**Relevant docs:**
- the cloudstream and aniyomi notes in `docs/reference-notes.md`;
- M24 and M31 (catalogue calls, `Catalog`), M38 (player, progress), M40 (filters, global search), M41 (Watch home, title page, stream picker);
- `docs/feature-parity.md` (Watch).

## Goal
- **Video sources:** extensions that browse, search and play movies, series and anime, shaped like aniyomi's anime sources and CloudStream's providers.
  - **Servers:** a title's episode lists its servers (hosters) first; each server's videos resolve on their own, and the UI shows each server as it resolves.
- **A first-party public-domain film source:** the Internet Archive's film collections.

## Design
- **`video.source` calls** (SDK `VideoSourceProvider`):
  - **Catalogue calls, as in `manga.source`** (`popular`, `latest?`, `search` with filters, `filters?`, `details`, `episodes`):
    - `details` adds `type` (`movie`, `series`, `anime`, `other`), `year`, `cast` and `directors`;
    - an episode adds `season` and `number`;
    - a movie is a title with one episode.
  - **Servers and videos, the aniyomi hoster model:**
    - `servers({ episodeId })` lists servers (`id`, `name`), optionally with their `videos` already resolved;
    - `videos({ serverId })` resolves one server.
  - **A video:**
    - `url` (http(s) only), `label` (e.g. "1080p"), `quality` (height in pixels), `headers` and `format` (`hls`, `dash` or `progressive`);
    - `subtitles` (`url`, `lang`, `label`).
- **Extension host** (`raygun-ext-host::video`):
  - **Catalogue answers:** checked by the `manga.source` parsers (shared), with the video fields read alongside. A season becomes the unit's volume.
  - **Servers and videos:** checked here.
    - URLs must be http(s), and headers lose hop-by-hop and host names.
    - At most 50 servers per episode and 50 videos per server.
- **App** (`raygun-app::video_sources`, `VideoSourceService`):
  - **Catalogue:** a `Catalog` with kind `video` covers sources, browse, search with filters, global search, stored titles and episodes, covers through the proxy, and the NSFW filter. Commands taking a `medium` accept `video`.
  - **Title view:** details, type, year, cast and directors. Episodes are grouped by season, each with watching progress and a watched mark. The progress key is `source:<sourceId>:<episodeId>`.
  - **Servers:** `video_servers` (fast) and `video_links` (one server). Links are ordered by the preferred quality: closest at or below it, then above it, unknown quality last.
  - **Preferred quality:** a setting (highest, 1080p, 720p, 480p, 360p).
  - **Play:** goes through the M38 player with the link's headers and subtitles (subtitles through the proxy). The title is the show, the subtitle "S1 E2 · Name", the poster the cover.
- **Internet Archive films:** a `video.source` provider in the existing Internet Archive extension (1.2.0).
  - **Browse:**
    - popular means most viewed, latest means newest additions;
    - search covers the film collections;
    - filters pick a collection (feature films, animation and cartoons, silent films, film noir, sci-fi and horror, comedy films) and a sort order.
  - **Details:** the item's metadata (description as plain text, year, creators, subjects as genres).
  - **Episodes:** one per original video file (a single file is one "Film" episode).
  - **Servers:** one, the Internet Archive, resolved at once. Its videos are the item's MP4/Ogg derivatives with their heights, plus matching `.srt`/`.vtt` subtitles.
- **UI:**
  - **Watch home:** after the addon rows, a row per video source (its popular page) with See all. See all opens the source page (shared with manga and novels: popular, latest, search and filters).
  - **Title page** (`/watch/video?source=&id=`):
    - cover, type, year, genres, description, cast and directors;
    - Play for a movie;
    - for a series, seasons and episodes with progress and watched marks.
  - **Server picker:** every server resolves at once. Each one shows its links as they arrive, the preferred one marked. Empty or failed servers say so and sink to the end.
  - **Search → Movies & series:** results from video sources (by source) under the addon rows.
  - **Settings → Playback:** preferred video quality.

## Out of scope
- **Saving video titles to the library, downloads, updates for followed series:** M43.
- **Sources that need a browser** (WebView resolving, Cloudflare): M44.
- **Third-party video sources:** only the first-party one ships. The protocol is what third-party sources build on.
- **Source priority across sources:** sources show in extension order.

## Done-check
- **Rust:**
  - video fields, servers and videos parsed and checked (bad URLs dropped, headers filtered, limits);
  - quality ordering;
  - the title view (seasons, progress, watched);
  - a fixture extension with two servers, one failing;
  - the Internet Archive provider through the fixture host where practical.
- **Vitest:**
  - the Watch home shows a source row;
  - a series title lists seasons and episodes;
  - the server picker shows servers as each resolves, marks the preferred link and the empty or failed servers, and plays a link;
  - the preferred quality setting;
  - Search shows video source results.
- 📱 **On the emulator:**
  - Internet Archive films appear on Watch;
  - a film's page plays an MP4 at the preferred quality, resumes from Continue watching and can be marked watched;
  - filters change results;
  - Search finds a film;
  - clean up afterwards.

## Decisions
- **No separate stack for video:** video sources reuse the `manga.source` parsers and the shared `Catalog`.
  - Titles live in the same tables with kind `video`, and their units have kind `episode`.
  - Type and year are two new title columns (migration 0015, also backed up).
  - Cast and directors are stored in the authors and artists columns; `VideoTitleView` names them properly.
  - `Medium` in the UI now includes `video` for sources, browsing and search. The reading-only hooks (title, library, read marks) take `ReadingMedium`.
- **A movie** is a title with one episode and no season. Its page offers Play and Mark watched instead of an episode list.
- **Servers resolve in the UI:**
  - `video_servers` answers fast. Each unresolved server is then its own `video_links` query, so a slow or broken host holds up nothing else.
  - Servers are ordered by state: resolved, resolving, empty, failed. A server that came resolved isn't asked again.
- **The preferred link** is the first after ordering by the preferred quality: closest at or below it, then the smallest above it, unknown heights last. With no preference, the highest comes first.
- **The poster** saved with watching progress is the source's own cover URL. A proxy link wouldn't survive a restart.
- **Search:** video sources go through the shared `CatalogSearch` (every source, or one with its filters), under the addon rows. All shows the top sources under the addon results.
- **Internet Archive films:**
  - **MP4 derivatives only:** Android's player can't play the Ogg Theora and MPEG-2 files many items also carry.
  - **One server,** resolved at once.
  - **Subtitles:** `.asr` subtitle files are the archive's automatic transcripts and are labelled so.
  - **Details:** descriptions become plain text, and every title is a `movie`.
- **The official repository** gets Internet Archive 1.2.0. Repacking also produced different bytes for `podcast-directory` 1.0.0 (same version, unchanged source). That change was left out, so the published 1.0.0 stays as it is.
- **Not done:** priority across sources. Sources appear in extension order.

## Verification
- **Checks:** `cargo test --workspace`, `cargo clippy --workspace --all-targets -D warnings`, `pnpm lint`, `pnpm type-check` and `pnpm test` all pass. New tests:
  - **Extension host:** video servers, videos and episodes parsed and checked; an inline video source whose third server fails while the second resolves; the Internet Archive provider against a mock archive (browse, collection filter, paging, details, episodes for a film and a multi-file item, MP4s only best first, automatic subtitles).
  - **App:** link ordering by preferred quality; the quality setting's validation.
  - **UI:** four Vitest cases (`watch/VideoSources.test.tsx`, `watch/StremioRow.test.tsx`).
- 📱 **Pixel_4 emulator (Android 17), debug build, fresh data:**
  - **Watch home:** "Popular · Internet Archive Films" sits under the addon rows with thumbnails.
  - **Charlie Chaplin Festival:**
    - the title page reads "Movie · 1938 · Internet Archive Films" with its genre and a plain-text description;
    - Play opens the server picker: Internet Archive with 480p h.264 (Preferred) and 240p, each with a subtitle;
    - 480p plays from archive.org in the native player (0:33 of 1:17:26);
    - Continue watching shows it with its year and cover, and resumes at 0:56;
    - with Settings → Playback at "Up to 360p", 240p becomes the first, preferred link;
    - Mark watched turns Resume back into Play.
  - **Search → Movies & series:** "chaplin" shows Cinemeta's rows, then "From your sources" with the archive's matches. Picking the source opens its filters, and Collection → Silent films changes the results.
  - **See all:** opens the shared source page (Popular, Latest, search), and its cards open the video title page (His Girl Friday, 1940).
  - **Clean-up:** the app's data was cleared afterwards (`pm clear`).
- **Found on the device and fixed:** a poster URL that fails to load (seen on a Cinemeta search result) showed the browser's broken-image icon. Posters now fall back to the placeholder.
