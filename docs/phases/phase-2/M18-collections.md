# M18 — Playlists, favorites, history, search across providers

**Relevant docs:** ARCHITECTURE §5 (data model, sync-ready columns), §10 (what syncs and how history merges), M07 (local library), M14 (resolver and `track_key`).

## Goal
Music the user cares about stays with them: a heart keeps a track, playlists collect tracks from any source, and Raygun remembers what was played. Search shows the user's own matching tracks first and merges what several providers found into one "Top results" list.

## Design

### Saved tracks
A saved track is metadata, not a file. The same favorite or playlist entry has to work on another device (M21), and a local file path or the stream proxy's artwork URL means nothing there.
- `SavedTrack { title, artist, album, durationMs, artworkUrl, source }`, plus `trackKey` (M14's `artist|title` key).
  - `source` keeps only portable sources: http(s) URLs and YouTube ids. Local files and `lookup` are stored as no source.
  - `artworkUrl` is kept only when it's an http(s) URL off this device; the local proxy's URLs are dropped.
- **Playing one** picks, in order:
  1. a local library track with the same `trackKey` (its file and artwork);
  2. the stored portable source;
  3. `lookup`, so the resolver finds a stream.
- **Listing them** fills in local artwork the same way, so saved local tracks keep their covers.

### Storage (migration `0005_collections.sql`, synced tables)
Every table has a UUIDv7 `id`, `updated_at_hlc`, `origin_device` and `deleted` (ARCHITECTURE §5). Track metadata is stored on each row instead of in a shared track table; then two devices that save the same song never fight over one row.
- `music_favorite`: one live row per `track_key` (a partial unique index over live rows), track columns, `added_at`. Unfavoriting sets the tombstone; favoriting again revives the row.
- `music_playlist`: `name`, `created_at`.
- `music_playlist_item`: `playlist_id`, `position` (REAL: an item added at the end gets the last position + 1; reordering takes the midpoint between neighbours, so a move touches one row), track columns, `added_at`. Deleting a playlist tombstones it and its items.
- `music_play_history`: append-only, `track_key`, track columns, `played_at`. History is a union across devices (§10), so it's never edited, only cleared (which tombstones the rows).

### What counts as a play
A pure `PlayCounter` in `raygun-app` follows each load's progress reports:
- It adds up listening time from consecutive progress reports less than 3 s apart, so seeks don't count.
- A load counts once, when it has played half its length or 4 minutes, whichever is less. A track shorter than 30 s counts when it ends.
- The counted item is written to history in the background; a failure only logs.

### Commands
- Favorites: `favorites_list`, `favorite_set(track, favorite)`, `favorite_get(track)` (whether a track is a favorite; the key is computed in Rust, so the UI never copies the normalization).
- Playlists: `playlists_list` (with item counts and up to 4 artworks), `playlist_create(name)`, `playlist_rename`, `playlist_delete`, `playlist_items`, `playlist_add(playlistId, tracks)`, `playlist_remove_item`, `playlist_move_item(itemId, beforeItemId?)`.
- History: `history_recent(limit)` (latest play per track, newest first), `history_clear`.
- Playing: `collection_play({ collection: favorites | playlist{id} | history, startIndex })` builds the queue in Rust, as the library does.
- Track input from the UI is a `TrackInput` (`QueueItemInput`'s metadata and source). The service makes it a `SavedTrack`.

### Search across providers
`online_search` returns `SearchResults { library, top, providers }`:
- `library`: up to 8 local tracks whose title, artist or album contain every word of the query.
- `top`: up to 5 tracks that at least two providers found, merged by `trackKey`. A track found by more providers ranks higher, then by its best position in a provider's list. Merging fills missing fields (artwork, duration, album, source) from the other copies.
- `providers`: the per-provider groups, unchanged.

### UI
- **Track actions:** library, search and saved rows get a "more" button that opens an action sheet: Favorite/Unfavorite, Add to playlist (pick one, or create a new one), and Remove from this playlist on playlist pages.
- **Full player:** a heart button and an "add to playlist" button for the current track.
- **Library:** new views Playlists and Favorites beside Albums, Artists and Tracks, shown even with no local folders. The playlist page (`/library/playlist?id=`) has Play, Shuffle, rename, delete and per-item remove, plus up/down moves in an edit mode.
- **Home:** "Recently played" (the last 10 tracks; tapping one plays the history from there), and shortcuts to Favorites and the first few playlists.
- **Search:** a "Your library" group and a "Top results" group above the provider groups.

## Out of scope
- Drag-to-reorder (moves use up/down buttons for now).
- Smart playlists, play counts and statistics; importing playlists from providers (Nuclear's `playlists` kind).
- Sync itself (M21); the tables are only shaped for it.

## Done-check
- **Rust:**
  - storage: favorite toggling and revival, playlist positions and moves, tombstoned playlists hidden with their items, history ordering and dedup;
  - `SavedTrack` keeps only portable sources and artwork; local matches win when playing;
  - `PlayCounter` thresholds and seek handling;
  - search merging and ranking;
  - bindings current.
- **Vitest:** the action sheet (favorite, add to a new playlist), the playlist page (play, remove, move), Home's recently played, and the search groups.
- 📱 On the emulator:
  - favorite a search result and a local track, then see both under Favorites with artwork;
  - make a playlist from Search and the player and play it (online items resolve);
  - Recently played on Home lists a track after half of it plays;
  - search shows "Your library" and "Top results".

## Verification (2026-10-31, Pixel_4 emulator)
- **Rust (raygun-db 25, raygun-app 33):** favorites toggle and revive one row; playlist append, move, renumber when positions run out of room, rename, delete with items; history is one row per track, newest first, and clears; writes carry HLC stamps and tombstones; library search escapes `%`/`_`; key lookup and key backfill; portable sources and artwork only; `PlayCounter` thresholds, seeks, short tracks and skips; Top results merging.
- **Vitest (app 89):** favoriting from the action sheet, adding to a new playlist, the playlist page (play, play from a track, move, remove), Playlists and Favorites without library folders, Home's recently played, the full player's heart, and the search groups.
- 📱 **On the emulator:**
  - The migration ran and the existing 5,003 library tracks got their keys at the first scan.
  - From Search: favorited a MusicBrainz result, created "Road trip" from the action sheet and added a second song to it. Library → Playlists shows it with the first item's cover; the playlist page and Home's shortcut show it too.
  - Favorites lists the online favorite with its cover and a local track with its local artwork (saved without a file, found again by key).
  - "Blue in Green" played through the resolver and counted at 172 s of 5:37 (half). Home's Recently played then listed it.
  - Searching "song 01 artist 01" shows "Your library" above the online groups. Search's "Top results" showed above the providers.
  - The action sheet and its "New playlist" field stay above the on-screen keyboard.

## Decisions and notes
- **Native reports were being lost.** The Android engine's report thread looked up `PlaybackService` the moment it started, before the service was managed. When the thread won that race, it panicked, and every later report (progress, ended, errors) went nowhere, so plays never counted and the position stayed at 0:00. It now looks the service up per message, survives a failing message, and panics anywhere are logged (Android drops stderr). The bug predates M18; startup timing decided whether it showed.
- **Keyboard:** edge-to-edge windows aren't resized for the keyboard, so bottom sheets ended up under it. `MainActivity` now pads the content by the keyboard's height.
- **Top results** first ranked a song only one provider found when nothing agreed (a podcast once came first). It now lists only songs at least two providers found. This is covered by unit tests; the device check above ran before the change.
- Playlist songs without an Internet Archive match ("Hello Dolly" versions) fail to resolve: the resolver's coverage, not M18.
- History reaches Home on the next visit; there's no event for "a play was counted" yet.
- The fixture library's tracks are tag-only files that don't actually play, so local plays can't be checked with them.
