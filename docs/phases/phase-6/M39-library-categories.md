# M39 — Library by media type and category, with filters and tags

**Relevant docs:** M24 (episodic library), M28 (downloads), M37 (podcasts), M38 (Watch), `docs/feature-parity.md` (Manga: categories, filters, sort, badges).

## Goal
- **Media types:** Library is organised by type: Music, Podcasts, Manga, Novels, Videos.
- **Categories:** the manga and novel libraries get Mihon-style categories (user-named tabs).
- **Finding things:** a library search, filters, sort and genre tags.

## Design
- **Media types:** the Library's top switch is now a scrollable row with Music, Podcasts (moved out of the Music views; old `view=podcasts` links redirect), Manga, Novels and Videos.
- **Videos:** a history of everything watched or started (`video_history`), with progress or "Watched". Tap to resume, or remove an entry.
- **Categories** (`0013_categories.sql`, `raygun-db::categories`, synced conventions):
  - **Storage:** a `category` table per kind (manga or novel) with a position, plus the `title_category` membership table.
  - **Deleting** a category leaves its titles in the library.
  - **Commands:** `library_categories`, `library_category_create/rename/delete/reorder` and `library_set_title_categories`, all taking a `medium`.
  - **Tabs:** All, each category with its count, and Uncategorized (when any title has no category). The tabs only appear once categories exist.
  - **Edit categories page** (`/library/categories?medium=`): add, rename, move up or down, delete.
  - **Title page:** a library title's page has a category picker (checkboxes, plus a link to edit categories).
- **Library entries** carry what filtering needs:
  - genres and status;
  - date added, last read (from the progress HLC) and latest chapter date (upload date, else when it was found);
  - whether reading has started;
  - downloaded chapters;
  - category ids.
- **Controls** (`manga/libraryFilters.ts`, pure and tested; `LibraryControls`):
  - **Search:** a search box over titles.
  - **Filter panel:** Show only Unread / Started / Downloaded / Completed.
  - **Sort:** by Title, Last read, Latest chapter, Unread count or Date added, ascending or descending.
  - **Tags:** genre chips, most common first; all chosen tags must match.
  - The filter is remembered per library on the device. A dot on the filter button shows when one is active.

## Out of scope
- **Search page:** searching across media types, source filters and tags come in M40.
- **Mihon's remaining library options:** per-category sort, display modes (list/compact grid), badges settings and a default category on add (M46).

## Done-check
- **Rust:** categories create, order, rename, delete (titles become uncategorized) and memberships.
- **Vitest:**
  - filter, sort and tag logic;
  - the library shows all five media types and category tabs with counts that filter;
  - filters, tags, search and sort in the UI;
  - the categories page adds, renames, moves and deletes.
- 📱 **On the emulator:**
  - add a few MangaDex titles;
  - make categories and put titles in them;
  - switch tabs;
  - filter by Unread and a tag, sort, and search;
  - see Podcasts and Videos as library types;
  - clean up afterwards.

## Decisions
- **Podcasts move from a Music view to their own media type:** the library is organized by media type first, then by category. Old links to the Music → Podcasts view still land on Podcasts.
- **Filtering in the UI, over one library call:** a library is at most a few thousand titles, and filtering locally makes every tap instant. Rust supplies the fields; `libraryFilters.ts` is pure and unit-tested.
- **Last read comes from the HLC of the latest progress write:** that HLC already holds physical milliseconds, so no new column is needed.
- **Genres serve as tags:** sources already provide them (MangaDex genres and themes, Shosetsu and Gutenberg subjects), so tags work for every source without new extension APIs.

## Verification (2026-11-01, Pixel_4 emulator, fresh app data)
- **Checks:** all pass. `cargo test --workspace`, clippy, `pnpm lint`, `pnpm type-check` and `pnpm test` (231 app tests).
- **Media types:** the Library shows Music, Podcasts, Manga, Novels and Videos.
- **Categories:**
  - Edit categories made "Reading" and "Later"; the arrows enable and disable correctly at the ends of the list.
  - Three MangaDex titles were added: Solo Leveling went into Reading through its page's category picker, My Dress-Up Darling into Later, and The Eminence in Shadow (with chapter 1 read) into none.
  - The manga library showed the tabs All 3, Reading 1, Later 1, Uncategorized 1.
- **Filter panel:** it lists Unread, Started, Downloaded and Completed, the sort, and every genre in the library as tags.
  - The Magic tag narrowed the list to Solo Leveling and The Eminence in Shadow.
  - Adding Started left only The Eminence in Shadow.
- **Cleanup:** the app data was reset after the check.
