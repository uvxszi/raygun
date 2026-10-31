# M07 — Local music library

**Relevant docs:** ARCHITECTURE §5 (data model), §6 (playback), §11 (UI). reference-notes → nuclear (`LocalFileInfo`), RiPlay (on-device library UX).

## Goal
Find the music on the phone, read its tags and artwork, and browse it as Albums / Artists / Tracks. Tapping an album or track replaces the queue and plays it, with shuffle. It has to stay fast with 5,000 tracks.

## Design decisions
- **One scanner for every platform:** Rust walks folders (`walkdir`) and reads tags and embedded artwork with `lofty`. Android doesn't use MediaStore metadata (lower quality, Android-only).
  - **Android access:** the plugin requests `READ_MEDIA_AUDIO` (API 33+) or `READ_EXTERNAL_STORAGE` (≤ 32) and returns the public Music and Download folders.
  - With that permission, scoped storage lets native code read audio files by path, so the scanner works unchanged.
- **Local-only tables** (`local_library_root`, `local_track`): file paths are device-specific, so these rows never sync (ARCHITECTURE §5 `local_*` rule).
  - Albums and artists are derived with `GROUP BY` over two stored grouping columns: `group_artist` = album artist, else artist, else "Unknown artist"; `group_album` = album, else "Unknown album".
  - Synced entities (favorites, playlists) will refer to tracks by metadata later.
- **Incremental rescans:** files whose size and modification time haven't changed are skipped. Files that disappeared are removed. Tags are read in chunks of 200, with a progress event after each chunk.
- **Artwork:** the first embedded picture is written once to `cache/images/<blake3>.<ext>`, so it's shared by every track on the album. The loopback proxy serves it at `/art/<file>` (only hash-shaped names).
- **Playing:** `library_play(selection)` with `Album | Artist | AllTracks`, an optional start track and `shuffle`. Rust builds the queue (local file sources) and calls a new core operation, `replace(items, start, shuffle)`.
- **UI:**
  - Library tabs (Albums grid / Artists / Tracks, the last virtualized with `@tanstack/react-virtual`).
  - Album page (art, Play, Shuffle, tracks) and artist page (albums, Play all, Shuffle).
  - A "Find music on this device" first-run action and a scan progress bar.
  - The "Open a stream or file" form moves below the library.

## Done-check
- Rust: scanner tests (tags, missing tags fall back to the file name, artwork dedupe, extension filter), incremental rescan (unchanged skipped, removed deleted), album/artist grouping, `replace` with shuffle.
- Vitest: tabs render data, album page plays from a track, first-run button triggers connect + scan.
- 📱 Pixel_4 with a generated 5,000-track library in `/sdcard/Music`: grant the permission in the system dialog, scan completes with progress, the album grid shows artwork, scrolling Tracks stays smooth, Play and Shuffle on an album work, and a rescan with nothing changed is fast.

## Verification (2026-10-31, Pixel_4 emulator)
- Generated library: `cargo run --release -p raygun-library --example make_test_library -- <dir>` gives 5,000 one-second tagged MP3s (50 artists × 10 albums × 10 tracks, a distinct cover per album). It was pushed to `/sdcard/Music/RaygunTest`.
- "Find music on this device" → system dialog "Allow Raygun to access music and audio" → Allow. Music and Download were added and the progress banner showed.
- **Full scan: 5,000 found, 5,000 read, 0 failed, 14.7 s** (4 reader threads).
- Album grid with artwork; artists list; Tracks view of 5,000 rows virtualized. Six fast flings: 4.9 % janky frames, p50 17 ms, p90 27 ms (emulator, software GPU).
- Playing from a track (all tracks), album Play, and album Shuffle (shuffle on, random up-next) all worked. The mini-player and sheet show the artwork.
- **Unchanged rescan at launch: read 0, 5.7 s.**
- Fixed during verification: the active view pill had unreadable text; page scroll carried over between routes (pages now start at the top); lofty logged one warning per MP3 (lofty logging is now `error` only).

## Follow-ups
- The unchanged-rescan time is dominated by `stat`ing 5,000 files through Android's FUSE layer. Options: parallel directory walk, or MediaStore generation numbers to skip unchanged folders. Revisit if real libraries feel slow.
- Content scrolls under the transparent status bar. Add a top scrim with the M09 status-bar work.
- Artwork pops in shortly after rows appear (lazy image decode). Consider small thumbnails generated at scan time.
