# M46 — Library depth

**Relevant docs:**
- `docs/research/mihon.md` §1 (library);
- `docs/research/cloudstream.md` §1 (watch status, favorites);
- `docs/research/aniyomi.md` §9 (library and categories);
- `docs/research/shosetsu.md` §4 (selection, pins, author filters);
- M39 (categories, filters), M43 (video library);
- ARCHITECTURE §5 (synced tables).

## Goal
The manga, novel and video libraries (`MangaLibraryView`) should match Mihon's, with CloudStream's watch lists and Shosetsu's pins:
- select many titles and act on them;
- sort them into lists (Reading, Planned, Completed, On hold, Dropped) and favorites;
- show them as compact, comfortable or cover-only grids, or a list, with columns per orientation and settings per category;
- sort and filter them like Mihon (tri-state filters, more sorts, a search language);
- continue from a cover, open a random title, update one category, see duplicates when adding;
- read comics from the device (CBZ/ZIP) as a local source.

The library still never browses (finding content stays in Browse).

## Design

### Storage (migration 0017)
- **On `title`**, marks set in the app (`user_*`, synced conventions):
  - `user_list` (`reading`, `planned`, `completed`, `onHold`, `dropped`, or NULL);
  - `user_favorite_at`;
  - `user_pinned_at`.
- **On `category`:** `user_display`, JSON (`{ mode, sort, descending }`), used when per-category settings are on.
- **Library entries** (`MangaLibraryEntry`) gain:
  - the list, favorite and pinned flags;
  - authors and artists;
  - the source's name;
  - `refreshedAt` (last update check) and `latestFetchedAt` (newest chapter found);
  - `local` (an imported title).

### Display (Settings → Library, `LibraryDisplayPrefs` in `Settings`, one `serde(default)` row)
- **Mode:**
  - compact grid (the title over the cover);
  - comfortable grid (the title under it);
  - cover-only grid;
  - list (cover thumbnail, title, source, unread and downloaded counts).
- **Covers per row,** set separately for portrait and landscape (auto, or 2–8). The existing `gridColumns` becomes the portrait value.
- **Tabs:** by category (Mihon), by list (CloudStream: Favorites, Reading, Planned, Completed, On hold, Dropped), or none. An option shows the number of titles on each tab.
- **Per-category settings:** when on, each category keeps its own mode and sort (stored on the category); otherwise one setting covers all.
- **Badges:** unread and downloaded (existing), plus a local badge.
- **Continue button:** a play button on covers with unread chapters or episodes. A setting turns it off.
  - For manga and novels it opens the chapter the title page's Resume would (`continue_from`).
  - For videos it starts a playback session.
- **Browse grids** keep the existing cover style.

### Sorting and filtering (the filter sheet; the library's own state stays in local storage per medium)
- **Sorts:**
  - alphabetical;
  - total chapters;
  - last read;
  - last update check;
  - unread count;
  - latest chapter (by date);
  - chapter fetch date;
  - date added;
  - random, with a reshuffle;
  - each ascending or descending.
  - Pinned titles stay on top (an option).
  - **Mihon's rules:**
    - ties go A→Z whatever the direction;
    - missing values come first when ascending;
    - with the unread sort, titles with nothing unread are always last.
  - Tapping the active sort flips its direction.
- **Tri-state filters** (tap cycles off → only → without):
  - unread, started, downloaded, completed, favorite, local;
  - each list;
  - genres;
  - authors and artists.
- **Includes and excludes:** includes in one group (lists, genres, people) match any of them; excludes always remove.
- **Search language** in the library's search box (`libraryQuery.ts`):
  - **Words** match titles.
  - **`field:value`** matches one field:
    - `title`, `author`, `artist`;
    - `genre` or `tag`;
    - `source` or `src`;
    - `status`;
    - `list` (with `favorite`).
  - **An empty value** (`author:""`) matches titles with nothing in that field.
  - **Number fields compare:** `unread>3`, `read<=10`, `total=0`, `downloaded>0`, `added>=2026-01-01` (a local date). `seen` and `unseen` are video names for `read` and `unread`.
  - **The word `local`** matches imported titles.
  - **Operators:**
    - `-` excludes;
    - quotes keep words together;
    - `|` or `||` means either;
    - `&&` or a space means both;
    - parentheses group.
- **Remembered:** the open tab, per medium.

### Selection
- **Entering:** long-pressing a cover selects it and starts selection mode.
- **Ranges:** a later long press selects everything between the last picked title and this one (Mihon). While selecting, a tap toggles a title.
- **Top bar:** "N selected" with Select all, Invert and Close.
- **Bottom action bar:**
  - change categories (a sheet with tri-state boxes for mixed selections);
  - set list;
  - favorite;
  - pin;
  - mark read or unread;
  - download (next 1, 5, 10 or 25 unread, or all unread; manga and novels);
  - remove, in Mihon's dialog:
    - two boxes: "From the library" and "Downloaded chapters";
    - both start unchecked, and Remove stays off until one is checked;
    - the downloads box is hidden when a local title is selected.
- Selection survives switching tabs and ends with back.

### Title pages
- **List button:** manga, novel, video-source and Stremio title pages get a "List" action (a sheet of the lists) and a favorite heart.
- **Duplicates:** adding a title whose name matches one already in the library, from another source, opens a dialog naming it, with Add anyway, Open it and Cancel.
  - Names are compared by their letters and digits, lowercased.
  - One name inside the other counts, in either direction ("Frieren" and "Frieren: Beyond Journey's End").
  - Names under four characters must be equal.
- **Putting a title on a list or among favorites** also adds it to the library. Removing it clears its list, pin and favorite.

### Extras
- **Open a random title** (menu): one of those shown (after filters).
- **Update this category** (menu, on a category tab): the library update for its titles only.
- **Local comics:** Library → Manga → ⋮ → Import comics accepts one or more CBZ/ZIP files.
  - Each file becomes a chapter of a local title (source `local.comics`).
  - The title comes from ComicInfo.xml's series or the files' shared name, and imports with the same name join the same title.
  - Files go up in chunks, so large archives never pass whole through the webview.
  - Pages unpack to `downloads/comics/<title>/<chapter>/` and load through the proxy's `/local/` route.
  - The cover is the first chapter's first page.
  - **Chapter numbers** come from ComicInfo's `Number`, or else the last number in the file name ("Series 012.cbz", "Series v2 c13").
  - **Pages** are the archive's images in natural order, so page2 comes before page10.
  - **Library updates** skip local sources.

## Out of scope
These go to later milestones, each with a reason:
- **Bookmarked and tracker filters:** bookmarks are M47, trackers M59;
- **Migrating between sources:** M47 or later;
- **Bulk video downloads:** they need a link picked per episode (M49);
- **Folders and RAR/7z archives on the device:** Android's file picker gives single files; RAR needs a native library;
- **Novel updates and "Update this category" for novels:** M48.

## Done-check
- **Rust:**
  - the migration;
  - list, favorite and pin setters;
  - bulk read/unread, remove (with downloads), categories add/remove, and download of the next N unread;
  - continue unit lookup;
  - duplicate matching;
  - per-category display;
  - category-only updates;
  - CBZ import (ComicInfo, ordering, joining a title) and its pages.
- **Vitest:**
  - display modes;
  - tabs by list;
  - tri-state filters and the search language;
  - every sort;
  - selection and its actions;
  - the duplicate dialog;
  - the list button;
  - Settings → Library;
  - the continue button.
- 📱 **On the emulator:**
  - selection and bulk actions;
  - lists and favorites;
  - each display mode;
  - the filters;
  - the continue button;
  - a random title;
  - importing a CBZ and reading it;
  - the duplicate dialog;
  - the hold-for-speed badge (M45 follow-up);
  - clean up afterwards.

## Decisions
- **Storage:** the list, favorite and pin marks are columns on `title` (`user_list`, `user_favorite_at`, `user_pinned_at`), so they travel with the title like its other `user_*` fields. A category's own layout and sort is one JSON column (`category.user_display`).
- **Lists are CloudStream's, with manga words:** Reading, Completed, On hold, Dropped and Plan to read ("Watching" and "Plan to watch" for videos).
  - A title is on at most one list. Favorites are separate, so a favorite can be on any list.
  - Choosing a list or favoriting adds the title to the library, after the duplicate check. Removing a title clears its list, pin and favorite.
- **Settings:** one `libraryDisplay` row (`LibraryDisplayPrefs`, `serde(default)`). The old `gridColumns` stays the portrait value, so existing choices keep working. The UI fills missing fields with `withLibraryDisplay`.
- **Filter state stays in the UI** (local storage per medium), as in M39. Filters saved before M46 (booleans and arrays) load as the new tri-state form.
- **The filter sheet has three tabs** (Mihon's): Filter, Sort and Display. Changing the layout there writes the same setting as Settings → Library, or the category's own when per-category settings are on.
- **Selection:**
  - the selected titles' actions run in Rust in one call each (`library_*` commands take a list of ids);
  - acting ends selection mode, as in Mihon;
  - download is hidden for videos (an episode needs a link picked) and when a local title is selected.
- **Mark read** marks only released chapters (`set_titles_completed`), and keeps the read date of chapters already read.
- **Download next N** queues unread, released, not yet downloaded chapters in reading order through the existing queue.
- **Duplicates:** the check runs on the titles already in the library of the same medium, any source, comparing letters and digits only.
- **Local comics:**
  - the webview sends files to Rust in 2 MB base64 chunks (`upload_begin`, `upload_append`, `upload_cancel`) and Rust writes them under `downloads/.uploads/`, cleared at start;
  - the archive is unpacked into `downloads/comics/<series key>/<sha256>/` (through a `.part` folder), so importing the same file again replaces that chapter instead of adding another;
  - titles and pages are read with the `zip` crate already in the workspace; RAR/7z aren't read.
- **Local titles have no refresh or download actions** on their page: they are already on the device.
- **Download badges** in the library refresh a second after the download queue last changed, not on every page.
- **Sheets close on a full tap outside them.** Closing on pointer down let the same tap land on what was underneath.
- **Library → Music → All:** a new first chip (and the default view) showing everything at a glance: playlists with Favorites, albums, artists and the first songs. Each row's title opens its own view. Music keeps its chips and the other libraries their underlined tabs.
- **Hold for speed (M45 follow-up):** the badge is a small pill at the top centre ("2× »"), not a large label in the middle.

## Verification
- **Checks:**
  - `pnpm lint`, `pnpm type-check` and `pnpm test`: 286 UI tests, among them `manga/LibraryDepth.test.tsx`, `libraryQuery.test.ts` and the rewritten `libraryFilters.test.ts`;
  - `cargo test --workspace` (new: lists, favorites, pins and bulk read in `titles_tests.rs`; bulk categories and a category's display; duplicate matching; uploads; ComicInfo, file names, natural order and unpacking in `comics.rs`);
  - `cargo clippy --workspace --all-targets -- -D warnings`;
  - bindings current.
- 📱 **On the Pixel_4 emulator:**
  - **Local comics:** two CBZ files picked together in the system picker became one title, "Raygun Test Comic", with chapters 1 and 2. The cover was the first page, and ComicInfo's writer, summary and genres showed. Chapter 2's ten pages read in order (2-1, 2-2, 2-3 …), and the title page had no refresh or download.
  - **Lists and favorites:** on MangaDex titles, "Add to a list" → Reading added Frieren to the library, the star added One-Punch Man, and the heart added Chainsaw Man.
  - **Duplicates:** adding "Chainsaw Man - Digital Colored Comics" asked "Already in your library?", naming Chainsaw Man (MangaDex). Add anyway added it.
  - **Grid:** unread counts, a download count, continue buttons, and the local badge on the comic.
  - **Continue:** the button on the comic reopened chapter 2 at page 4, where reading had stopped.
  - **Selection:**
    - a long press on the first title and another on the third selected all three;
    - the list sheet showed no list chosen for the mixed selection, and On hold moved all three;
    - pin moved One-Punch Man to the top with a pin;
    - Remove opened Mihon's dialog with Remove disabled until "From the library" was ticked, then removed the title;
    - Download → Next chapter downloaded Chainsaw Man's first chapter, and its badge showed 1.
  - **Filters:** On hold included and Unread excluded showed only Frieren. The chips read "✓ On hold" and "− Unread" (struck through).
  - **Display:** the list layout and tabs by list (All 5, Favorites 1, Reading 0, Completed 0, On hold …).
  - **Search:** `unread>5` showed the two Chainsaw Man titles.
  - **Random title:** the menu item opened one of the library's titles (Frieren).
  - **Music → All:** two WAV files scanned in showed the album, the artist and both tracks, each row with its arrow. It is the first chip and the default.
  - **Hold for speed:** holding the picture of an Internet Archive film showed a small "2× »" pill at the top centre with the controls hidden.
  - **Clean-up:** the test files were deleted from Download and Music, and app data was cleared.
- **Bugs found on the device and fixed:**
  - the action bar sat right under a short grid and wrapped onto two rows (now pinned to the bottom in one row);
  - a tap outside a sheet also tapped what was under it (it opened the "On hold" tab);
  - local titles offered refresh and download;
  - library download badges didn't update when a download finished.
- **Not seen on the device:**
  - per-category layout and sort;
  - Update this category;
  - the categories sheet with mixed states (covered by `LibraryDepth.test.tsx`);
  - landscape columns;
  - the video library's lists.
