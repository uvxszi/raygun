# M40 — Search across media, with source filters and tags

**Relevant docs:** M18 (music search), M25 (manga sources), M31 (novel sources), M37 (podcast directory), M39 (library), `docs/feature-parity.md` (global search, source filters).

## Goal
- **Search everything at once or one type:** All, Music, Podcasts, Manga, Novels, or Your library.
- **Manga and novels:** search every source at once (Mihon's global search), or one source with its own filters, including genre tags to include or exclude.

## Design
- **Search page:**
  - **Type tabs** below the search box; the type lives in the URL (`/search?type=manga&q=…`), so Back returns to the same results.
  - **All** shows sections, each with See all, which switches to that type:
    - In your library: up to 4 of each kind;
    - Music: merged top results, else the first source's;
    - Podcasts: 3 directory results;
    - Manga and Novels: the two sources with results, 3 covers each.
  - **Music:** the existing results (library, top results, each source).
  - **Podcasts:** directory results; tap to open the show.
  - **Your library:** local matches across music tracks (new `library_search`), manga and novel titles, podcast subscriptions and watched videos.
- **Manga and Novels:**
  - **Source picker:** a picker for All sources or one source.
  - **All sources:** `catalog_global_search(medium, query)` asks every source in parallel for its first page. Sources with results come first, then empty ones, then failures; each source's failure is shown on its row. More opens that source.
  - **One source:**
    - **Search:** `catalog_search(medium, source, query, page, filters)` with Load more.
    - **Filters:** `catalog_filters(medium, source)`, drawn in a Filters panel. A source can be browsed with filters alone (an empty query is allowed once a filter is set).
- **Source filters** (new extension API, SDK `SourceFilter` and `FilterValues`):
  - **Declaring:** a `manga.source` or `novel.source` provider may implement `filters()` returning `select`, `toggle` and `tags` filters. `search({query, page, filters})` receives only the values that were changed.
  - **Checks:** the host checks declarations (known kinds, safe keys, labels up to 80 characters, at most 20 filters and 500 options). A source without `filters()` (or a Shosetsu script) simply has none.
  - **Tags** are three-state chips: tap once to include, again to exclude, again to clear. Long tag lists get a find box.
- **MangaDex 1.2.0:**
  - **Filters:** Sort by (best match, most followed, latest upload, top rated, recently added, title), Status, Demographic, and Tags, from MangaDex's tag list, fetched once per session.
  - **Content tags** only show when NSFW is on.
  - **Search:** passes `includedTags[]`, `excludedTags[]`, `status[]`, `publicationDemographic[]` and the order.
  - **Published** to the official repository.

## Out of scope
- **Video search:** none yet; Stremio catalogs in M41 add it, and Watch history is already searchable under Your library.
- **Saved searches** and Mihon's per-source filter presets.
- **Shosetsu filters:** its scripts' own `searchFilters`, mapped to these filters; later, with the novel parity work.

## Done-check
- **Rust:** filter declarations are checked (valid kinds kept, bad keys, empty selects and unknown kinds dropped).
- **Vitest:**
  - All shows each type's section and See all switches the type;
  - manga search across sources shows results and per-source errors;
  - More opens one source;
  - select, include and exclude tags, and toggle filters reach the search;
  - filters alone browse without a query;
  - Your library finds titles and watched videos;
  - the existing music search tests run under the Music type.
- 📱 **On the emulator:**
  - All for a manga name shows music, podcasts and MangaDex rows;
  - Manga → MangaDex → Filters: include a tag, exclude another, set a status, and see the results change;
  - Your library finds a library title;
  - clean up afterwards.

## Decisions
- **Filters belong to sources, declared by extensions:** sources know their own filters (MangaDex's tag ids, statuses, orders), like Mihon's `FilterList`. Three kinds cover what sources need: select, toggle, and include/exclude tags.
- **Global search runs sources in parallel with a per-source error:** a slow or broken source doesn't hide the others, which is how Mihon behaves.
- **A filters call that fails is an error, not "no filters":** on the first device run, MangaDex's tag list fetch failed once and the source looked like it had no filters. Now only "this source has no filters" gives an empty list, and other failures are retried by the UI (and logged).
- **The All view previews each type and never shows everything:** All is for finding which type has the thing; See all moves to the full results.

## Verification (2026-11-01, Pixel_4 emulator, fresh app data)
- **Checks:** all pass.
  - Rust: `cargo test --workspace`, clippy, and the live `mangadex_filters_live` test (4 filters; "completed" alone returns 24 titles).
  - Front end: `pnpm lint`, `pnpm type-check` and `pnpm test` (235 app tests).
- **All for "frieren":**
  - In your library: nothing matched.
  - Music: "Sleepy Frieren" by Evan Call.
  - Podcasts: "Frieren: Beyond the Fantasy Lore" and two more.
  - Manga: a MangaDex row (Funeral Frieren Anthology, Frieren of the Strong Fist…).
  - Novels: "Nothing found" from Gutenberg.
- **Manga across sources:** See all switched to the Manga type with MangaDex's row, and More opened MangaDex alone.
- **MangaDex filters:**
  - Manga → Source: MangaDex → Filters showed Sort by, Status, Demographic and MangaDex's full tag list with a find box.
  - With no title typed, including Fantasy and Isekai listed isekai fantasy titles (The Eminence in Shadow, That Time I Got Reincarnated as a Slime, Mushoku Tensei…).
  - Tapping Isekai again excluded it (struck through), and the results became non-isekai fantasy (Solo Leveling, Frieren, One Piece).
- **Cleanup:** the app data was reset after the check.
