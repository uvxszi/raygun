# M14 — Resolver pipeline

**Relevant docs:** ARCHITECTURE §6.1 (resolution pipeline), §6.2 (engines), §6.3 (YouTube on mobile), M11–M13 (extensions), reference-notes (Nuclear stream candidates: search → lazy resolve → verification).

## Goal
A track no longer needs a stream URL to be playable. Raygun asks every `music.stream` provider for **candidates**, scores them against the track, plays the best one, and falls back to the next candidate automatically when a stream fails. Successful picks are cached and verified, so replaying is instant.

## Design

### Model
- New `MediaSource::Lookup { title, artist, album, durationMs }`: "find a stream for this track".
  - The UI uses it for search results that have no source (MusicBrainz, below).
  - Extensions can't return it.
- `PlaybackState.sourceLabel` names where the current item's stream came from (e.g. "Internet Archive"). The full player shows it as "via …".

### `music.stream` providers (extension contract)
- `candidates({ title, artist, album, durationMs, limit })` returns `{ candidates: [{ id, title, artist?, durationMs?, quality?, source? }] }`.
- `resolve({ id })` returns `{ source, expiresAt? }`. It is only called for candidates without an inline `source` (e.g. signed URLs that expire).
- `@raygun/sdk` gets the `MusicStreamProvider` type; the host validates sources (http(s) URLs and YouTube ids only).

### Crate `raygun-resolver` (pure, no I/O)
- **Normalization:** lowercase, accents folded, bracketed suffixes dropped (`(Remastered 2011)`, `[Official Video]`), `feat.` parts dropped, punctuation removed.
- **`track_key(query)`** is `artist|title` after normalization. It keys the cache.
- **Scoring** (higher is better):
  - Title similarity: token overlap with the query, 0–40.
  - Artist present in the candidate's artist or title: +20, or −10 when the candidate names a different artist.
  - Duration: within 3 s +25, within 8 s +15, within 20 s +5, more than 30 s off −30.
  - Unwanted variants not asked for (live, cover, karaoke, instrumental, remix, sped up, slowed, 8D, nightcore, reaction): −25 each.
  - Kind: direct stream +10; YouTube embed +0, a fallback because the embed pauses in the background on Android.
  - Provider record: up to ±10 from past success and failure counts.
  - Verified earlier for this track: +50.
- **Threshold:** candidates scoring under 35 are dropped, so Raygun skips a track rather than play the wrong song.

### Cache and verification (`raygun-db`, migration `0004_streams.sql`, device-local)
- `local_stream_candidate (track_key, provider_id, candidate_id, title, source json, expires_at, verified_at, failed_at, failure)`.
- `local_stream_provider_stats (provider_id, successes, failures)`.
- **Verified:** the engine reported `Playing` for the stream. **Failed:** resolve or playback errored. A failed candidate is skipped for 24 h.

### `ResolverService` (app)
1. **Cache:** a verified, unexpired, not-failed candidate plays immediately.
2. **Candidates:** every `music.stream` provider is asked in parallel. The scored list is kept in memory for 10 minutes, so a fallback doesn't search again.
3. **Pick:** the best candidate not yet tried. If it has no inline source, the provider's `resolve` is called, and a resolve failure moves on to the next.

### Playback integration (`PlaybackService`)
- **Loading a `Lookup` item:** the status stays `Loading` while an async resolve runs. The result is sent to the engine only if that load is still current (`load_id`). The same `load_id` is reused, so engine reports keep matching.
- **Engine `Error` on a resolved item:** the candidate is marked failed and the next one loads. Only when none are left does the error reach the core, which skips to the next item as before.
- **Direct sources fall back too:** when a concrete `Url`/`YouTube` item with a title fails, the resolver gets one chance to find another stream for it, excluding the failed one. This is the "unblockable" path: direct, then other providers, then embed.
- **`Playing` on a resolved item:** marks it verified and counts a success for the provider.

### Extensions
- **Internet Archive 1.1.0** adds a `music.stream` provider: it searches title + creator, reads item file lists and returns matching MP3 files as candidates.
- **MusicBrainz 1.0.0** (new, metadata only) does a recording search. It returns title, artists, album, length and Cover Art Archive artwork, with no source, so every result plays through the resolver. Requests follow MusicBrainz's rules: a descriptive User-Agent and one search per query. This pulls part of M15's MusicBrainz work forward; M15 adds Cover Art and lyrics.

### UI
- Search results without a source are playable; they queue as `Lookup` items.
- Mini player and full player:
  - "Finding a source…" while a `Lookup` item loads;
  - "via {sourceLabel}" once it plays;
  - the error when nothing playable is found.

## Done-check
- Resolver tests:
  - Normalization and track keys.
  - Scoring picks the right recording over live, cover and wrong-artist versions and duration mismatches.
  - The threshold drops mismatches.
  - Ranking uses verification and provider stats.
- Database round trips for candidates and stats.
- Ext-host tests: `music.stream` candidates and resolve; invalid sources dropped.
- Playback tests:
  - A `Lookup` load waits for its resolution.
  - A stale resolution (newer load) is ignored.
  - An error advances to the next candidate with the same `load_id`.
  - An exhausted list reaches the core.
- Vitest: source-less results play as `Lookup`; "Finding a source…" and "via …" in the players.
- 📱 On the emulator:
  - A MusicBrainz result plays through the Internet Archive stream provider.
  - Replaying hits the cache (log).
  - A development extension with a broken top-ranked candidate falls back to the next one automatically.

## Verification (2026-10-31, Pixel_4 emulator)
- **Tests:**
  - Rust, 129 in total:
    - `raygun-resolver` (7): normalization and keys; the studio recording beats live, cover and karaoke; asked-for variants aren't penalized; "Artist - Title" names count; duration mismatches; the threshold; embeds as fallback; history.
    - Playback tracker (5): lookups wait; stale resolutions dropped; errors walk the candidates and then reach the core; a direct source falls back once; local files never fall back.
    - Core: a long queue stops after 3 failures in a row.
    - Database round trips for candidates and provider stats.
    - Ext-host: `music.stream` candidates and resolve with invalid sources dropped; concise error messages; `raygun.time.sleep`.
  - Vitest: 75 app tests (source-less results queue as `Lookup`; "Finding a source…" then "via …") and 18 SDK tests.
- **Live tests (opt-in):**
  - MusicBrainz → Internet Archive streams → ranking picks a real Louis Armstrong file.
  - Two back-to-back MusicBrainz searches both succeed.
  - A duet credit with a decorated title still searches by the main artist.

| Check | Result |
|---|---|
| Search "what a wonderful world armstrong" | Internet Archive and MusicBrainz groups; MusicBrainz leads with Louis Armstrong's recordings |
| Tap a MusicBrainz result (no source) | Mini player: "Finding a source…"; log `stream candidates for "What a Wonderful World": 4 items, 4 matching files` → `stream picked … score=40`; native engine PLAYING; full player "via Internet Archive" |
| Skip ahead and back to it | `playing cached stream`, playing within a second, no search |
| Dev extension with a broken, perfectly matching candidate; play "Hello, Dolly" (Louis Armstrong & His All-Stars) | Broken candidate picked (score 85) → `engine reported an error` → `stream failed, trying the next` → Internet Archive candidate picked → PLAYING |
| Missing cover art | Players fall back to the placeholder instead of a broken image |

Found and fixed during verification:
- **MusicBrainz answered 503 "busy".** Its limit is one request per second per IP, and a second request 1.5 s later already failed. The extension now paces its requests and retries twice. This needed a new host API, `raygun.time.sleep` (the runtime has no timers).
- **Provider errors showed raw request URLs and JS stack traces.** Users now see the thrown message ("archive.org didn't answer in time"); stacks go to the log.
- **Internet Archive's stream search hid failures.** It reported "no candidates" when its searches failed. It now throws when every search fails and logs partial failures.
- **Internet Archive searched with the full credit and title.** Searches now use the main artist ("Louis Armstrong & His All-Stars" → "Louis Armstrong") and the title without brackets, with a title-only search as a last resort (the ranking filters weak matches).
- **A long list of unplayable results failed through dozens of items.** The core now stops after 3 failures in a row.
- **The players showed a broken image for missing covers.** They now use the shared `Artwork` placeholder.

## Decisions
- **Resolution state is per load and lives in the service.** The pure `Tracker` decides and `PlaybackService` acts, so the playback core stays free of I/O and async.
- **The cache is device-local.** Stream URLs expire and differ per network. Verified picks are what make replays instant, and failures cool down for 24 h.
- **Scores are deliberately conservative** (threshold 35). A metadata length that disagrees with every stream (MusicBrainz sometimes lists a different version) still lets a strong title and artist match through, because the penalty is −30, not a veto.
- **MusicBrainz is built in** alongside Internet Archive: metadata from one, streams from the other, which is the M14 path in one tap. M15 adds Cover Art, YouTube Music and lyrics.
