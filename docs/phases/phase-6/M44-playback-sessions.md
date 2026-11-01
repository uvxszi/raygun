# M44 — Playback sessions: play at once, fall back, next episode

**Relevant docs:**
- `docs/research/aniyomi.md` §1 (starting playback, hosters, fallback) and §6 (episodes inside the player);
- `docs/research/stremio.md` §1–2 (series Continue watching, binge watching, last stream);
- `docs/research/cloudstream.md` §5 (player, sources dialog, next episode);
- M38 (player), M41 (Stremio), M42 (video sources), M43 (video library, downloads).

## Goal
Watching a series should feel like Aniyomi, CloudStream and Stremio, not like picking files.
- **Playback starts at once:** tap an episode and the player opens and plays the best link as soon as one resolves.
- **Failures recover:** a link that fails moves on to the next one by itself.
- **Episodes stay in the player:** previous and next episode, autoplay next with a countdown, and an episode list without leaving the player.
- **Sources stay reachable:** the servers and links can be switched from inside the player, keeping the position.
- **Continue watching follows the series:** finishing an episode moves Continue watching on to the next one.

## Design
- **A player session in Rust** (`raygun-app/src/video_session.rs`, managed state).
  - **Contents:** what is playing (a title's episode list from a video source or a saved/unsaved Stremio title, or a single link), the current episode, and its candidate links with their states.
  - **One at a time:** opening another video replaces it.
  - **Links per origin:**
    - **Video source:** the episode's servers. Ready servers give links at once; the others resolve in parallel and update the list as they arrive. Links are ordered by the preferred quality (M42), the source's own order next.
    - **Stremio:** direct streams from every addon (YouTube and torrent streams are left out). The addon and `bingeGroup` used last for this series come first (Stremio's binge matching), then addon order.
    - **Downloaded episode:** plays from the device with no resolving (M43).
  - **Playing:**
    - The first playable link starts as soon as it is known.
    - On a load error (not a decoder retry), the next candidate is tried from the same position, and the player says "Trying the next source…".
    - When every candidate failed, the player shows the error and keeps the sources sheet open.
  - **Remembered per title:** the server name (sources) or addon and `bingeGroup` (Stremio) last played, in `local_value` under the title's key; it orders the candidates next time.
- **Player ⇄ Rust:**
  - **Opening:** `video::open_session` opens `VideoPlayerActivity` straight away in a loading state (title, episode label, spinner) with no URL yet.
  - **Loading:** each video goes in as a `load` message through a new plugin command `videoUpdate`. It reaches the running activity (`singleTask`; `onNewIntent` or the live instance), which swaps the media item without closing.
  - **Requests from the player:** `NativeBridge.onVideo` with `state: "request"` and an `action`:
    - `next`, `previous`, `episode` (index), `source` (candidate index), `retry`;
    - `ended` (autoplay next if it's on).
  - **Updates to the player** (`videoUpdate`):
    - `episodes`: labels, watched and downloaded flags, current index;
    - `sources`: groups with server name, state (resolving, ready, empty, failed with message) and links (label, quality, current);
    - `loading` / `error` (text);
    - `autoplay` (on or off).
- **The player** (`VideoPlayerActivity`):
  - **Previous/next buttons** via a `ForwardingPlayer` that reports and handles previous/next. They show only when the session has a neighbour.
  - **Top-bar actions:**
    - **Episodes:** a dialog listing episodes, the current one marked, watched ones dimmed.
    - **Sources:** a dialog of servers and links with their states; choosing one switches and keeps the position.
    - **Autoplay toggle:** a switch in the episodes dialog, also in Settings → Player.
  - **Next-episode prompt:** a "Next episode in 5 s" button with Cancel, shown in the last seconds (or at the end), then the next episode loads in the same player.
  - **Reports:** unchanged, plus the new request states.
- **Continue watching follows the series:**
  - **Resume entries:** episodes played through a session save an episode reference as their resume request (origin, title id, episode) instead of a fixed URL. Resuming then resolves fresh links, since links expire, and prefers the last source.
  - **Moving on:** when an episode of a session is watched, its Continue watching entry is replaced by the next episode's (position 0, "Next: S1 E3 · Name"), unless the next isn't out yet.
  - **Ending:** when the series is finished, the entry goes.
- **Title pages:**
  - Tapping an episode, or the floating Play/Resume, opens a session instead of the picker.
  - Long-pressing an episode (or its ⋮) still opens the picker to choose a link, play it or download it.
  - Downloaded episodes play from the device (M43).
- **Settings → Player:** "Play the next episode automatically" (on) and "Next-episode prompt" (5/10/15 s before the end, or at the end).

## Out of scope
The custom player overlay, gestures, chapters and skip-intro, and subtitle styling: M45.

## Done-check
- **Rust:**
  - candidate ordering for sources (preferred quality, last server) and for Stremio (last addon and `bingeGroup`);
  - moving to the next candidate on errors;
  - the next and previous episode, skipping episodes that aren't out yet;
  - Continue watching moving on to the next episode, and episode-reference resume.
- **Vitest:**
  - tapping an episode starts a session (not the picker);
  - long-press opens the picker;
  - the new setting.
- 📱 **On the emulator** (local test addon and Internet Archive):
  - an episode plays without choosing a link;
  - a dead first link falls back to the next;
  - next and previous work, and autoplay moves on at the end with the prompt;
  - the episode and source dialogs switch without leaving the player;
  - Continue watching shows the next episode after one finishes;
  - clean up afterwards.

## Decisions
- **Sessions live in Rust** (`video_session.rs`); the player only shows what it is sent and asks for things. One session at a time; opening a single video ends it.
  - **Generations:** each episode change starts a new generation, so slow lookups for an earlier episode can't load into the player.
- **Player reports are handled one at a time, in order** (an mpsc queue in `video.rs`). Concurrent handlers raced on the same rows: finishing an episode failed with "database is locked" on the first emulator run.
- **The player asks for the session's state once it opens** (`request: ready`). Updates sent before its window existed (the episode list right after opening) were otherwise lost.
  - Every load also re-sends the episode list.
  - Opening an empty Episodes or Sources list asks again.
- **Link order:**
  - **Video sources:** servers in the source's order, the server played last for the title first; links within a server by the preferred quality (M42).
  - **Stremio:** addons in order, the addon played last first, and within it the same `bingeGroup`. A downloaded episode plays first, from the device.
  - What was played last is remembered per title in `local_value` (`video.lastSource.…`).
- **Fallback:** any load error other than a decoder retry moves on to the next untried link from the same position. With nothing left (and nothing still resolving), the player says so and opens Sources.
- **Next and previous** skip episodes that aren't out yet. Episodes are in watching order: reading order for source titles (numbers ascending), meta order for Stremio.
- **Continue watching moves on** like Stremio's `time_offset = 1`: when an episode of a session counts as watched, the next released episode gets a progress row at 1 ms with an episode reference as its resume request, unless it was already started or watched.
- **Resuming an episode reference starts a session**, so links are found afresh instead of replaying an expired one.
- **Title pages:**
  - Tapping an episode, or Play/Resume, starts a session.
  - A long press on an episode, or the top bar's new **Choose a source**, opens the picker (play or download).
  - The per-episode download button opens it in download mode.
- **The player's words** come from the app's translations (`videoPlayer.*`), sent once at start and on language changes (`video_set_player_labels`).
- **Settings → Player → Episodes:**
  - "Play the next episode automatically" (on);
  - "Next-episode prompt": at the end, or 5/10/15/30 seconds before (10).
- **Single-video titles** hide Episodes and the previous/next buttons.
- **Not done:** the custom overlay, quality chips and gestures are M45. The dialogs are Android's standard ones for now.

## Verification
- **Checks:** `cargo test --workspace` (new session tests: link order with the remembered source and `bingeGroup`, candidates skipping failed/tried/unready links, next/previous skipping unreleased episodes, episode references vs. video requests), `cargo clippy -D warnings`, `pnpm lint`, `pnpm type-check` and `pnpm test` (265; title pages start sessions, a long press and Choose a source open the pickers, the new settings and the player labels) all pass.
- 📱 **On the emulator (Android 17, debug build).** A local addon via `adb reverse` serves a generated series. Episode 1 lists a dead link first, then AES-128 HLS; episode 2 is an MP4; episode 3 is announced for Nov 8.
  - **Episode 1 played from a single tap.** The dead link was tried (four 404s) and the HLS link took over with no picker.
  - **The second time, the remembered addon and `bingeGroup` played first**, skipping the dead link.
  - **Episodes dialog:** marked the current episode and the watched one, and offered the autoplay toggle; picking episode 2 loaded it in the same player.
  - **Previous** went back to episode 1, with Next enabled and Previous greyed out.
  - **Next-episode countdown:** "Next episode in 8 s" with Cancel showed in the last 10 s, and autoplay moved on to "S1 E2 · The Pattern".
  - **Continue watching:** with autoplay cancelled, finishing episode 1 put "Raygun Test Show · S1 E2 · The Pattern" in Continue watching. Tapping it found fresh streams for episode 2 and played.
  - **Internet Archive:** His Girl Friday played from Play with no picker. Its Sources dialog listed 480p (current) and 240p, and switching to 240p kept the position.
  - **Clean-up:** app data cleared afterwards.
- **Not seen on the device:**
  - the last polish (hiding Episodes and previous/next on films, the title bar hiding with the controls after a load, the plain Continue watching placeholder);
  - the "No source could play this" path.
