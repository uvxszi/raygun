# M15 — First-party extensions: lyrics, YouTube, cover art

**Relevant docs:** ARCHITECTURE §6.3 (YouTube on mobile), §7 (extensions), §12 (content policy: legitimate sources only), M14 (resolver), ROADMAP Phase 2.

## Goal
- Synced lyrics in the full player, which follow playback and seek when a line is tapped.
- YouTube as a search and stream source that fills the gaps the Internet Archive leaves.
- Cover art for MusicBrainz results. The MusicBrainz extension already links Cover Art Archive images (M14), so this part is done.

## Design

### Lyrics (`lyrics` provider kind)
- **Contract:** `lyrics({ title, artist, album, durationMs })` returns `{ synced?: [{ timeMs, text }], plain?: string } | null`.
- **Host:** `ExtensionHost::lyrics(query)` asks every `lyrics` provider in parallel and keeps the best answer: synced beats plain, then provider order. Malformed lines are dropped and lines are sorted by time.
- **Model:** `Lyrics { providerName, synced: [LyricLine { timeMs, text }], plain }`.
- **Command:** `lyrics_get(query)`. The UI caches answers per track (React Query), so reopening the view doesn't fetch again.
- **SDK:** `LyricsProvider`, `LyricsQuery`, `LyricsResult` types.

### LRCLIB extension (`dev.raygun.lrclib`, built in)
- An open, keyless lyrics database.
- It tries `GET /api/get` (exact title, artist, album and duration) first. If that fails, `/api/search` picks the result closest in duration.
- LRC timestamps (`[mm:ss.xx]`, several per line) are parsed into lines.
- Instrumental tracks answer "no lyrics".
- It sends a User-Agent with a contact URL, as LRCLIB asks.

### Lyrics view (full player)
- A Lyrics button switches the artwork area to lyrics.
- **Synced lyrics:** the current line is highlighted and kept centred. Between engine progress reports, the position is interpolated from the last report while playing. Tapping a line seeks there.
- **Plain lyrics:** scroll.
- **States:** loading, "No lyrics found", and an error with the provider message.

### YouTube extension (`dev.raygun.youtube`, installable from the repository, not built in)
- **API:** the official **YouTube Data API v3** with the user's own API key, an extension setting.
  - `music.metadata`: video search in the Music category.
  - `music.stream`: candidates for a track, played through YouTube's own embedded player (`MediaSource::YouTube`).
- **Not included: direct stream extraction (InnerTube signature deciphering).** It works around YouTube's protections and breaks YouTube's terms, so it doesn't belong in a first-party extension (ARCHITECTURE §12). The consequence: YouTube audio pauses when the app is in the background on Android, as M08 found. The resolver's kind score already makes the embed a fallback behind direct streams.
- **Without a key:** the extension loads, and its providers answer with an error asking for one in Settings → Extensions.
- **Quota:** a search costs 100 of the 10,000 daily units, about 100 searches a day. The UI shows quota errors as provider errors.

## Done-check
- **Rust:**
  - The host picks the best lyrics across providers and validates lines.
  - The LRCLIB extension against a local fixture server: exact match, search fallback, instrumental, LRC parsing with several timestamps per line.
  - The YouTube extension against a local fixture server: search results, embed candidates, the missing-key error.
- **Vitest:** the lyrics view (synced highlight follows position, tap seeks, plain fallback, empty and error states).
- 📱 On the emulator: lyrics show and follow a playing track once LRCLIB is reachable. YouTube can't be checked on the device without the user's API key, so the fixture tests cover it.

## Verification (2026-10-31, Pixel_4 emulator)
- **Rust:** 133 tests.
  - The host picks the best lyrics: synced beats plain, bad lines are dropped and the rest sorted; nothing found gives `None`; when every provider fails, the error comes through.
  - **LRCLIB against a fixture server**, using the real bundle with its API base rewritten:
    - exact match, with LRC carrying two stamps on one line, a metadata tag and CRLF endings;
    - instrumental gives `None`;
    - the search fallback keeps synced over plain, closest length within 10 s;
    - an exact plain-only hit prefers a synced twin from search;
    - nothing found.
  - **YouTube against a fixture server:**
    - the missing-key message;
    - search with HTML-unescaped titles, "- Topic" channels turned into artists, ISO-8601 lengths, channels skipped, embed sources;
    - stream candidates;
    - invalid-key and quota messages.
- **Vitest:** 79 app tests. New: the current synced line follows the position, tapping a line seeks, the lyrics/artwork toggle, plain lyrics, no lyrics, and the error.

| Check | Result |
|---|---|
| LRCLIB built in | `extension loaded extension=dev.raygun.lrclib providers=1` |
| Lyrics while LRCLIB is down (its CDN answered 502 all session) | "Couldn't load lyrics: LRCLIB answered HTTP 502"; it took ~10 s because of React Query's default retries, so the query now retries once |
| Synced view on the device, with a temporary test provider giving timed lines | At 1:19 the highlight sat on line 16 (75–80 s), centred, advancing between progress reports; tapping a line seeked (position 70 000 ms = that line); the failing LRCLIB didn't hide the working provider's answer |
| YouTube | Not checked on the device: it needs the user's own API key (fixture tests above) |

**Not verified live:** LRCLIB's real service, which was unreachable all session (HTTP 502 from its CDN). Run `cargo test -p raygun-ext-host -- --ignored lrclib_live` once it's back.

## Decisions
- **YouTube uses only the official Data API and embedded player** (user's key, about 100 searches a day). Direct stream extraction is out of scope for first-party code. As a result, background YouTube audio on Android stays unavailable, and the resolver ranks embeds below direct streams.
- **YouTube is installable, not built in**, because it does nothing without a key. LRCLIB and MusicBrainz are keyless and built in.
- **Lyrics are cached per track in the UI only.** No database cache yet: LRCLIB is fast, and lyrics sync can come with device sync.
