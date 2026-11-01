# M28 — Library updates, updates feed, downloads

**Relevant docs:** ARCHITECTURE §3.2 (data folders), §5 (local vs synced tables), M24 (manga foundation), M27 (page cache); reference-notes (mihon: library update job, updates screen, download queue).

## Goal
- **Library updates:** the library keeps itself current. Titles are re-fetched on a schedule or by hand, and new chapters show in an Updates feed.
- **Downloads:** chapters can be downloaded for offline reading, and the download queue can be watched, paused and cancelled.

## Design

### Library updates
- **New column:** `unit.state_found_at` (migration `0010`) records when this device first stored a chapter.
  - `sync_units` sets it on insert only and returns how many units were new.
  - Rows from before the migration keep NULL.
- **`MangaService::update_library(manual)`** refreshes every library title, two at a time.
  - It skips titles whose status is `completed` or `cancelled`, except on a manual run.
  - Only one run happens at a time.
  - **Progress** is `MangaUpdateStatus { running, checked, total, newChapters, failed, lastRunAt }`. Every change goes out as a `MangaUpdateEvent`.
  - **Failures** are logged per title and counted; they never stop the run.
- **Scheduler:**
  - A background task checks every 15 minutes, starting 1 minute after launch.
  - It starts a run once `now - lastRunAt >= interval`. `lastRunAt` is kept in `local_meta` as `manga.lastUpdate`.
  - **Interval** setting `mangaUpdateHours`: 0 (off), 6, 12 (the default), 24, 48 or 168.
  - Runs only while the app is open. Android background work (WorkManager) comes later.
- **Updates feed** (`manga_updates`):
  - **Contents:** chapters of library titles with `found_at > title.user_in_library_at`, newest first, at most 300.
  - **Excluded:** the chapters that were already there when the title was added.
  - **Each row** carries title, cover, chapter, read state and download state.

### Downloads
- **Queue:** the device-local table `local_manga_download`.
  - Columns: `unit_id` (PK), `title_id`, `state` (`queued`, `downloading`, `done` or `failed`), `pages_done`, `pages_total`, `error`, `queued_at`, `finished_at`.
  - Files are per device, so the table is never synced.
- **Files:**
  - Pages are stored at `downloads/manga/<title_id>/<unit_id>/NNN.<ext>`.
  - A chapter is written to `<unit_id>.part/` and renamed when complete, so a folder without `.part` is always whole.
  - The extension comes from sniffing the image bytes.
- **Worker:** one task in `MangaDownloads`, woken by a `Notify`.
  - **Order:** chapters download in queue order, one at a time, with 2 pages in flight.
  - **Page list:** fetched from the source, then each page is fetched with the source's headers.
    - Non-image answers and images over 30 MB fail the chapter.
    - The error is stored and the partial folder removed.
  - **Pause/resume** is global and in memory. A pause stops after the page in flight; the chapter goes back to `queued`.
  - **Restart:** at startup, any `downloading` rows go back to `queued`.
  - **Cancel** removes a queued or failed row. On a done chapter it also deletes the folder.
  - **Events:** each state change and each finished page emits `MangaDownloadsEvent`, and the UI refetches.
- **Reading offline:**
  - When a chapter's download is `done` and its folder exists, `manga_chapter` lists the local files and makes no network call. A done row whose folder is missing is dropped.
  - Pages are served by a new proxy route `/local/<path>` over the downloads folder. Every path segment must be `[A-Za-z0-9._-]` and must not start with a dot.
- `raygun-net::download_images(client, pages, dir, concurrency, on_page)` does the fetching. It is generic and tested against a local server.

### UI
- **Library → Manga:** buttons for Browse, Updates and Downloads, plus Update library, which shows `checked/total` while running.
- **Updates page** (`/manga/updates`):
  - rows grouped by day (today, yesterday, date);
  - read chapters are dimmed;
  - tapping a row opens the reader;
  - each row has a download button;
  - pull-to-refresh is replaced by an Update button.
- **Downloads page** (`/manga/downloads`): active and failed items with progress bars, pause/resume all, cancel per item, retry for failed items, and a count of downloaded chapters.
- **Title page:**
  - each chapter row gets a download button: download → queued → % → done (tap to delete) → failed (tap to retry);
  - a "Download unread" action downloads every unread chapter.
- **Settings → Manga:** the update interval.

## Out of scope
- Background updates while the app is closed, and new-chapter notifications (needs Android WorkManager/notifications).
- CBZ packing of downloads, a download location picker, delete-after-read, and per-category update rules (categories come later).

## Done-check
- **Rust:**
  - `sync_units` marks new units with `found_at` and counts them;
  - the feed hides chapters that existed when the title was added;
  - the download queue moves through its states and resets interrupted downloads at startup;
  - `download_images` saves sniffed files, rejects non-images and removes partial folders;
  - the local route serves files and refuses `..` and hidden segments;
  - update skipping rules.
- **Vitest:** Updates page grouping and opening, the Downloads page (pause and cancel), the title page download button states, and the interval setting.
- 📱 **On the emulator:**
  - download a MangaDex chapter and watch its progress;
  - turn on airplane mode and read it;
  - run Update library and see the feed. To force a new chapter, delete the newest chapter row from a pulled copy of the database, push it back, then update.

## Decisions
- **TanStack Query runs in `networkMode: 'always'`.** The default `online` mode paused every query while the device was offline. That left the reader spinning in airplane mode even though its data comes over local IPC.
- **Downloads are plain image folders, not CBZ.** The proxy serves them as-is, and an atomic folder rename gives the same never-half-written guarantee.
- **Download progress is written by a side task.** The page callback stays synchronous and the database writes never block the fetch stream.
- **Fetch closures became named async fns.** A closure over borrowed items made the spawned future fail the `Send` check.

## Verification (2026-10-31, Pixel_4 emulator)
- **Checks:** all pass. `cargo test --workspace` ran 225 tests; clippy, `pnpm lint`, `pnpm type-check` and `pnpm test` (6 new Vitest cases) are clean.
- **Scheduler:** it ran its first library update one minute after launch (1 title, 0 new, 0.4 s).
- **Downloads:**
  - Queued Chapters 3 and 4 of "I Met the Male Lead in Prison". Chapter 4 showed as queued while Chapter 3's progress ring filled.
  - Both finished: 19 and 17 pages, about 19 s each, saved as `downloads/manga/<title>/<chapter>/001.png…`.
- **Offline reading:** in airplane mode, Chapter 3 opened in paged RTL with 19 pages and turned pages from disk. Before the `networkMode` fix it spun forever.
- **Feed:** dropped Chapter 14 from the database, ran Update library, and the Updates page listed Chapter 14 under Today.
  - "Last checked" survived a restart.
- **Downloads page:** shows "2 chapters on this device" and an empty queue.
