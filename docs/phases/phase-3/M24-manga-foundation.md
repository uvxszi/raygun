# M24 — Manga foundation: library, sources, MangaDex

This milestone covers roadmap items M24 (episodic library), M25 (`manga.source`) and the MangaDex part of M26, built as one slice so manga is usable end to end: browse a source, open a title, add it to the library, read a chapter. The full reader is M27.

**Relevant docs:** ARCHITECTURE §5 (episodic library schema, `remote_`/`user_`/`state_` prefixes), §7 (extensions), reference-notes (mihon: source API, schema; aniyomi), M11–M13 (extension host).

## Goal
- **Library → Manga** lists the manga you added.
- **Browse** lists the installed manga sources. A source shows popular, latest and search results as a cover grid.
- **A title page** shows details and chapters, with an add/remove-from-library button and read state.
- **A chapter opens** in a simple vertical reader. Finishing a chapter marks it read, and the title page shows where you are.

## Design

### Extension contract: `manga.source` (Mihon-shaped)
```ts
popular({ page }) → { items: MangaSummary[], hasNextPage }
latest?({ page }) → same
search({ query, page }) → same
details({ id }) → MangaDetails   // title, coverUrl, authors, artists, description, genres, status, url
chapters({ id }) → { chapters: MangaChapter[] }   // id, name, number, volume, scanlator, dateUpload
pages({ chapterId }) → { pages: { url, headers? }[] }
```
- Provider fields: `lang` (BCP 47) and `nsfw`.
- The host validates results: http(s) URLs only, ids as strings, unknown statuses become `unknown`. Filters come later.
- `@raygun/sdk` gets the types.

### Images
- Covers and pages go through the stream proxy's new `/image/<token>` route, so per-image headers (Referer) apply and no hotlink check sees the webview's origin.
- Tokens are a hash of URL and headers: the same image always gets the same link, and the browser cache works.
- About 4,000 links are remembered. The route only serves registered tokens, so it can't fetch arbitrary URLs.

### Storage (migration `0007_titles.sql`)
These are synced-shape tables (UUIDv7, HLC, tombstones), not yet in the sync registry (sync is on hold).
- **`title`:** `kind` (`manga`), `source_id` (`ext/provider`), `remote_id`, `remote_title`, `remote_cover`, `remote_authors`, `remote_artists`, `remote_description`, `remote_genres` (JSON), `remote_status`, `remote_url`, `user_in_library_at`, `state_refreshed_at`.
  - `UNIQUE(source_id, remote_id)` over live rows.
- **`unit`:** `title_id`, `kind` (`chapter`), `remote_id`, `remote_name`, `number`, `volume`, `remote_scanlator`, `remote_date`, `source_order`.
  - `UNIQUE(title_id, remote_id)`.
- **`unit_progress`:** `unit_id`, `position` (page index), `total`, `completed_at`.
- Viewing a title stores it with its chapters (as Mihon does), so progress has rows to attach to. Refreshing updates the `remote_*` columns and adds new chapters; a chapter that disappeared keeps its row and read state.

### Service and commands (`raygun-app::manga`)
- `manga_sources()`.
- `manga_browse(sourceId, mode: popular|latest|search, query?, page)` → cards with `inLibrary`.
- `manga_title(sourceId, mangaId, refresh)` → details, chapters with read state, and the chapter to continue.
- `manga_set_in_library`.
- `manga_library()` → titles with an unread count.
- `manga_pages(titleId, unitId)` → proxied page URLs.
- `manga_set_progress(unitId, position, total)`: reaching the last page completes the chapter.

### First-party extension: `extensions/mangadex`
- MangaDex API: popular by follows, latest by upload, title search, details with cover, authors and artists, chapter feed (paginated, in the chosen language), and at-home server pages.
- Settings: chapter language (default `en`), content rating (default safe + suggestive), data saver.
- Net permission `api.mangadex.org` only: covers and pages go through the app's image proxy, not the extension.
- Built in, like LRCLIB.

### UI
- **Library:** a top-level switch between **Music** and **Manga**. The Manga view is a cover grid of library titles with unread badges, plus a "Browse sources" button.
- **`/manga/browse`:** the source list.
- **`/manga/source?id=`:** Popular / Latest tabs and a search box, a cover grid, and "Load more".
- **`/manga/title?source=&id=`:** cover, title, authors, status, genres, description (expandable), Add/Remove library, Start/Continue reading, and chapters newest first with read ones dimmed.
- **`/manga/read?title=&unit=`:** vertical strip of lazily loaded pages, a progress bar, previous/next chapter at the end; progress is saved as you scroll.

## Out of scope (next milestones)
- M27: paged reader, RTL, zoom, tap zones, spreads, prefetch, disk page cache.
- Filters, categories, local CBZ, Komga/Kavita, downloads, updates feed, trackers, backup.

## Done-check
- **Rust:**
  - title, unit and progress storage (upsert keeps read state, unread counts);
  - `manga.source` parsing and validation;
  - image proxy tokens (stable, bounded, unknown tokens 404);
  - the MangaDex bundle against fixture responses.
- **Vitest:** the Library switch, browse grid with paging, title page (add to library, continue reading), reader progress.
- 📱 **On the emulator:**
  - browse MangaDex popular and search;
  - open a title with covers showing;
  - add it to the library;
  - read a chapter (pages load) and see it marked read.

## Verification (2026-11-01, Pixel_4 emulator)
- **Rust:**
  - `raygun-db` titles (3 tests): refresh keeps `user_*` and progress; units keep read state and the source's order, with vanished chapters last; completed chapters stay read when going back; unread counts and library membership.
  - `raygun-ext-host`: `manga.source` parsing (4 tests: ids, http(s)-only covers, unknown statuses, chapter names from numbers, page headers), plus the MangaDex bundle against a fixture API (English alternative titles, cover URLs and headers, genres without format tags, external chapters dropped, at-home page URLs with headers).
  - `raygun-net`: image links are stable per URL and headers, send their headers, only successful answers are cached, and unknown tokens 404.
  - `raygun-app`: reading order, the continue chapter, chapter info.
- **Vitest (app 105):** the Library's Music/Manga switch with unread badges; browse with Load more and search; the title page (continue link, add to library, mark read, refresh); the reader (pages, page indicator, progress saved on the last page, next chapter, no bottom bar).
- 📱 **On the emulator:**
  - MangaDex Popular showed covers with English titles (Solo Leveling, My Dress-Up Darling, Frieren…).
  - From Latest, a title page showed authors, status, genres, description and 14 chapters.
  - Add to library worked. Start reading opened Chapter 1, and its 17 pages loaded through the image proxy.
  - Scrolling to the end showed Page 17 of 17 and Next chapter. Back on the title page, the button read "Continue: Chapter 2". Library → Manga showed the title with 13 unread.

## Decisions and notes
- **MangaDex's bot challenge:** browser-like User-Agents (Raygun's default for extensions) get a 302 to a challenge page, for the API and for covers alike. The extension identifies itself as Raygun and passes that header for covers (new `coverHeaders`, stored in `title.remote_cover_headers` by migration `0008`) and pages.
- **External chapters:** many popular MangaDex titles list English chapters hosted elsewhere (MangaPlus, Webnovel). These are left out, so such titles show no chapters; the message on the title page says so.
- **Migrations are append-only once a device has run them.** Editing `0007` after the emulator applied it stopped the app at startup ("migration 7 was previously applied but has been modified"), so the column went into `0008`.
- The reader is a vertical strip for now; paged reading, RTL, zoom and caching are M27.
