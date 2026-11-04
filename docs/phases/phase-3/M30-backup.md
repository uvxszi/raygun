# M30 — Backup, restore and Mihon import

**Relevant docs:** ARCHITECTURE §5 (synced tables, HLC), §10 (merging); M21 (sync rows); reference-notes (mihon: protobuf backups). Trackers (M29) need tracker accounts and OAuth sign-in, so M30 comes first.

## Goal
- Everything built up in the app can be saved to one file and brought back on this device or another: settings, favorites, playlists, history, manga library and reading progress, and extension repositories.
- A Mihon (or Tachiyomi fork) backup brings its library and reading progress into Raygun, where Raygun has the source.

## Design

### Raygun backup (`raygun-db::backup`)
- **Format:** gzip-compressed JSON:
  `{ "format": "raygun-backup", "version": 1, "createdAt", "deviceId", "tables": { "<table>": [rows] } }`.
  - Rows are the same JSON objects device sync sends (`json_object` of every column, tombstones included).
  - Covered: the synced tables (`setting`, `music_*`), plus `extension_repo`, `title`, `unit` and `unit_progress`.
  - Not covered: device-local tables (`local_*`), downloads and caches.
- **Restore merges** instead of replacing:
  - **Last writer wins:** a row replaces ours only when its `(updated_at_hlc, origin_device)` is newer, as in sync.
  - **Same item, different id:** rows match on their natural keys first. That is `title (source_id, remote_id)`, `unit (title_id, remote_id)` and `extension_repo url`. The incoming id is rewritten to ours, and children follow the rewrite (units to titles, progress to units).
  - **Clock:** the HLC observes the newest stamp, so later local edits win over restored ones.
- **Where files go:**
  - `Create backup` writes `backups/raygun-<yyyymmdd-hhmmss>.raygunbk` under the data root; the newest 10 are kept.
  - The Backup card lists them with Restore and Delete.
  - **Importing a file:** the webview's file chooser reads it, the bytes go to Rust as base64, and the format is detected by content.

### Mihon import (`raygun-app::mihon`)
- **Parsing:** `.tachibk` / `.proto.gz` is gzip-compressed protobuf (`Backup { backupManga = 1, backupSources = 101 }`), decoded with `prost` derive structs for the fields used:
  - manga: source, url, title, artist, author, description, genre, status, thumbnail, dateAdded, chapters, favorite;
  - chapters: url, name, scanlator, read, lastPageRead, dateUpload, chapterNumber, sourceOrder.
- **Sources** match by name: a backup source's name against installed `manga.source` providers, case-insensitively.
  - Titles from unmatched sources are skipped and counted by source name.
- **Ids:**
  - MangaDex URLs (`/manga/<uuid>`, `/chapter/<uuid>`) become bare UUIDs.
  - Other sources use the last path segment when the URL looks like a path, else the URL as-is.
- **Imported titles:**
  - stored with their chapters and status (Mihon's numbers mapped to Raygun's strings);
  - favorites are added to the library;
  - read chapters are marked read, and partly read ones keep their page.
  - Imported chapters never show as updates, because they count as found when the title joined the library.
- **Summary:** imported titles, chapters, read chapters, and skipped titles per source.

### UI
- **Settings → Backup:**
  - **Create backup:** shows the file name.
  - **Backup list:** date and size, with Restore and Delete; restore asks for confirmation.
  - **Import from file:** accepts a Raygun backup or a Mihon backup, and shows the result summary.

## Out of scope
- Scheduled automatic backups, sharing a backup file out of the app (needs an Android share/save intent), trackers and categories from Mihon backups, and restoring from Aniyomi's anime entries.

## Done-check
- **Rust:**
  - a backup round-trips into an empty database;
  - restore keeps newer local rows and rewrites ids of titles that exist under another id;
  - the Mihon decoder reads a backup built with the same structs;
  - imports map sources and ids, set read state and progress, skip unknown sources, and keep imported chapters out of the feed.
- **Vitest:** the Backup card creates, lists, restores (after confirming) and deletes backups, and shows an import summary.
- 📱 **On the emulator:**
  - create a backup and restore it;
  - import a Mihon-format backup (a synthetic one with a MangaDex title, pushed to Downloads) through the file chooser;
  - the title shows up in the library with its read chapters.

## Decisions
- **Restore and gaps:** restore is last-writer-wins per row, like sync, with one exception. When the newer local title row has no library date but the backup's does, the library date comes back as a new local write. Without it, browsing a title on the new device would silently drop it from the restored library.
- **Imports onto stored titles:** an import keeps the source's data for titles already stored here (a backup carries less: no cover headers, often no dates) and applies only library state and progress.
  - New titles are marked stale, so their first view fetches full details.
- **No file save dialog yet:** backups live in the app's `backups/` folder. Files come in through the webview's file chooser, read as base64.

## Verification (2026-10-31, Pixel_4 emulator)
- **Checks:** all pass. `cargo test --workspace` ran 234 tests; clippy, `pnpm lint`, `pnpm type-check` and `pnpm test` (115 app tests) are clean.
- **Create and restore:** Create backup saved `raygun-20261101-010155.raygunbk` (4 KB), which shows in the list. Restore asked for confirmation and reported 0 changes, since the data was identical.
- **Mihon import:** a synthetic Mihon backup in `/sdcard/Download` was picked through the Android file chooser. The result read "Imported 1 title with 14 chapters (5 read)", with "Some Scanlator (1)" skipped.
  - The title, taken out of the library beforehand, was back in it, with Chapters 1–5 read and Continue pointing at Chapter 6.
- **Re-import:** importing again after a refresh kept the source's chapter dates and scanlators, and the downloaded chapters stayed downloaded.
