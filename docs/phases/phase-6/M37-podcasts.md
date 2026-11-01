# M37 — Podcasts

**Relevant docs:** M05 (playback core), M07 (local library), M13 (extensions), M28 (downloads pattern), M36 (official repository), `docs/feature-parity.md` (Music: podcasts, from RiPlay's `PodcastScreen`).

## Goal
- **Podcasts in the music tab:** find shows, subscribe, see new episodes, and play episodes that resume where you stopped.
- **Episode state:** mark episodes played, and download them to listen offline.

## Design
- **Storage** (`0011_podcasts.sql`, `raygun-db::podcasts`):
  - **Tables:** `podcast` (feed URL, `remote_*` from the feed, `user_subscribed_at`) and `podcast_episode` (guid, audio URL and type, duration, date, `user_position_ms`, `user_played_at`).
  - **Conventions:** both follow the synced conventions (UUIDv7, HLC, tombstones), so device sync can carry them later.
  - **Refreshing:** a feed refresh rewrites only `remote_*` columns, matching episodes by guid.
  - **Downloads:** episode downloads live in the device-local `local_podcast_download`.
- **Feeds** (`raygun-app::podcast_feed`, `quick-xml`):
  - **Formats:** RSS 2.0 with the iTunes tags, and Atom. Real-feed quirks are tolerated: CDATA, entity-escaped HTML, `HH:MM:SS` or seconds durations, RFC 2822 dates with unknown zones.
  - **Episodes:** only items with an audio or video enclosure count; a missing guid falls back to the audio URL.
  - **Text:** descriptions become plain text with paragraph breaks.
  - **Limit:** feeds are capped at 16 MB.
- **Directories** (`podcast.directory` capability, `raygun-ext-host::podcast`):
  - **Provider calls:** a provider answers `search({query, limit})` and optionally `top({limit})` with `{title, author, feedUrl, artworkUrl}`.
  - **Checks:** results are checked (http(s) feeds only, no repeats).
  - **First-party extension:** Podcast directory (`extensions/podcast-directory`), built into the app and published to the official repository.
    - It uses Apple's public iTunes Search API for search, and Apple's top-shows chart plus `lookup` for top shows.
    - The chart country is a setting.
- **`PodcastService`:**
  - **Opening a show:** `open_feed(url)` stores a show on first sight without subscribing (`feed://` and bare hosts are accepted), and refetches it after an hour.
  - **Subscriptions:** subscribe and unsubscribe; `refresh_all` runs on start when the last refresh is over 6 hours old, or from the Refresh button.
  - **Directory search:** asks every installed directory and merges results by feed, marking subscribed ones.
  - **Play** (`replace_from`, a new `PlaybackCore::replace_at` that starts at a position):
    - **Source:** a downloaded file if there is one, else the feed's URL.
    - **Start:** where listening stopped, or the start once the episode is played or past 95%.
  - **Following playback:**
    - The playback service passes each state change and the current item to `on_playback`.
    - A new item is matched to an episode by audio URL or download path, in the background.
    - The position is saved every 10 s while playing and on pause. Ending, or passing 95%, marks the episode played.
  - **Downloads:**
    - Files go to `downloads/podcasts/<podcast>/<episode>.<ext>`, written to a `.part` file first and capped at 2 GB.
    - Progress is saved and announced every MB (`PodcastsChangedEvent`). The download can be deleted.
- **UI:**
  - **Library → Music → Podcasts:**
    - subscriptions (artwork row with new-episode counts) and Refresh;
    - New episodes across shows;
    - Find podcasts: search, top shows before searching, and "paste an RSS feed address".
  - **Podcast page (`/podcast?id=`):** Subscribe, Refresh, an expandable description and the episodes.
  - **Each episode:** date, length or "N min left" or Played, a progress bar, Play, mark played, Download or Delete download, and tap for notes.

## Out of scope
- **Later playback features:** playback speed and skip-silence (they come with the M50 music parity work, alongside the equalizer and sleep timer).
- **Formats and sync:** OPML import and export, and chapters inside episodes. Podcasts in backups and device sync come with M48 data parity.
- **Video:** video podcasts play through the audio engine; the Watch tab's player (M38) takes them later.

## Done-check
- **Rust:**
  - **Storage:** a refresh keeps subscription and progress; counts of unplayed episodes and the latest episodes; marking played rewinds; downloads per episode.
  - **Feed parsing:** RSS with iTunes tags (CDATA, entities, durations, dates, enclosure-less posts skipped) and Atom; non-feeds are refused.
  - **Other:** directory result checks; feed address normalising; download file types; `replace_at` starting position.
- **Vitest:**
  - Podcasts in the music library: top shows, search, opening a result;
  - the podcast page: subscribe, play, mark played, download, notes;
  - download state followed from events.
- 📱 **On the emulator:**
  - top shows and search load from the directory;
  - open a show and subscribe; it appears with new episodes;
  - play an episode and pause; it shows "N min left" and resumes there;
  - download a short episode and play it offline;
  - clean up afterwards.

## Decisions
- **Dedicated tables, not the episodic `title`/`unit` ones:**
  - **Why:** episodes need an audio URL, type and duration, and progress in milliseconds.
  - **What stays the same:** the column conventions match, so sync and backups can treat them alike later.
- **Playback reports to podcasts, not the other way round:** `PlaybackService` hands each state change and the current item to `PodcastService::on_playback`.
  - It matches items by audio URL or download path, so queue items need no new fields.
  - Any way of playing the URL (even "Open a stream") still saves progress.
- **"New" means found after subscribing:** subscribing to a show with 1,584 episodes showed "1584 new". Only unplayed episodes that arrive in later refreshes count now, as in other podcast apps.
- **Apple's public endpoints for the directory:** the iTunes Search API and the top-shows chart need no key. The extension is a separate, replaceable directory, so others (Podcast Index, fyyd) can be added as extensions.
- **Artwork goes through the image proxy:** artwork is cached, so a downloaded episode shows its artwork offline.

## Verification (2026-11-01, Pixel_4 emulator)
- **Checks:** all pass. `cargo test --workspace`, clippy, `pnpm lint`, `pnpm type-check` and `pnpm test` (223 app tests).
- **Finding and opening shows:**
  - Library → Music shows a Podcasts chip. Top shows loaded with artwork (The Daily, The Joe Rogan Experience, Crime Junkie…).
  - Searching "short wave npr" found NPR's Short Wave.
  - Opening it parsed all 1,584 episodes, with dates and lengths.
- **Subscribing:** Subscribe switched to "Subscribed". The show appeared under Subscriptions, and its episodes under New episodes.
- **Playing:**
  - Playing an 11-minute episode used its own artwork in the mini player.
  - After pausing at about 0:30, the episode showed "11 min left" with a progress bar.
  - Playing it again resumed at 0:38.
- **Offline:**
  - Download saved a 12.9 MB file under `downloads/podcasts/<show>/<episode>.mp3`. The row showed "10 min left · Downloaded" and Delete download.
  - In airplane mode the episode played from the file, resumed at its saved position, and still showed its artwork.
- **"New" count:** the "1584 new" count it showed then was fixed afterwards (see Decisions), with a test.
- **Cleanup:** the app data was reset after the check.
