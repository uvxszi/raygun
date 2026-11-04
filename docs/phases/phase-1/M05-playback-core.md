# M05 — Playback core

**Relevant docs:** ARCHITECTURE §6.1–6.2 (resolution pipeline, engines). reference-notes → nuclear (`stream_server.rs`, queue model), RiPlay (HybridPlayer: one facade, swappable engines), cloudstream (`ExtractorLink` headers/referer).

## Goal
Rust owns the queue and the canonical player state. A webview `<audio>` engine executes commands and reports back, and a loopback proxy makes any HTTP stream or local file playable.

## Design
- **`crates/raygun-playback`**: a pure state machine with no I/O or Tauri. Every operation returns `Vec<Effect>`, so all queue logic is unit-testable.
  - **State:** queue, current index, play order (shuffle), repeat (`off|all|one`), status (`idle|loading|playing|paused|ended|error`), position/duration, volume, and a `load_id` that increments on every load.
  - **Operations:** `enqueue(items, play_now)`, `play_at`, `next`, `previous` (past 3 s it restarts the track), `toggle`/`play`/`pause`, `seek`, `set_volume`, `set_repeat`, `set_shuffle`, `remove`, `clear`.
  - **Engine reports:** `progress`, `playing`, `paused`, `buffering`, `ended`, `error`, each tagged with `load_id`. Stale reports from a previous track are ignored.
    - `ended` auto-advances and honors repeat-one. A manual skip ignores repeat-one.
    - `error` skips to the next item, and stops once every item in a row has failed.
  - **Effects:** `Load { load_id, item, start_ms, autoplay }`, `Engine(Play|Pause|Seek|SetVolume|Stop)`, `StateChanged`, `QueueChanged`.
- **`crates/raygun-net`**:
  - **HTTP client:** reqwest with rustls on `ring` and bundled Mozilla roots. aws-lc and the platform verifier are avoided because both complicate Android builds.
  - **`StreamProxy`:** an Axum server on `127.0.0.1:<random>`. `register(source) -> url` returns `/stream/<token>`. Only registered sources are served, so it is not an open proxy.
    - Remote sources forward `Range` plus the source's own headers (Referer, User-Agent, Cookie), pass through 200/206 and the content headers, and add CORS.
    - Local files are served with Range support (`tower-http` `ServeFile`).
- **Android:** `network_security_config.xml` allows cleartext only to `127.0.0.1`/`localhost`, so release builds can reach the proxy.
- **App layer:**
  - `PlaybackService` holds the core in a mutex. It turns `Load` into a proxy URL and emits typed tauri-specta events: `EngineCommandEvent`, `PlaybackStateEvent`, `QueueEvent`.
  - Commands: `playback_get_state`, `playback_get_queue`, `playback_enqueue`, `playback_control`, and `playback_engine_report` for the engine's reports.
- **UI:**
  - **`Html5Engine`:** one hidden `<audio>` that executes engine commands and reports back (progress throttled to 4 Hz).
  - **Hooks:** `usePlaybackState` and `useQueue`, fed by the events.
  - **Mini-player and player sheet:** title/artist, play/pause/previous/next, a seek slider, repeat and shuffle, and an up-next list.
  - **Library page:** an "Open URL or file path" field, until the real library arrives in M07.

## Done-check
- Rust: core tests (ordering, repeat, shuffle, ended/error handling, stale reports); proxy tests (header injection, Range → 206, local file Range, unknown token → 404).
- Vitest: the engine executes commands and reports; the player UI reflects state and sends controls.
- 📱 and 🖥: a queue of an HTTP stream (it needs a Referer, which proves header injection) and a local file plays, pauses, seeks and skips.

## Verification (2026-10-31)
- 📱 Pixel_4, debug APK:
  - A Referer-protected HTTP stream played through the proxy. The test origin logged `referer=https://raygun.test/ range=bytes=0-`.
  - The local file `files/tone-b.wav` was queued and played.
  - Auto-advance on end, next, pause/resume, seek, repeat-off end-of-queue and previous all worked, and up-next updated correctly.
- Bug found and fixed on device: "up next" showed the playing item after an auto-advance. The queue snapshot no longer carries its own cursor; the UI derives position from `currentItemId`. A regression test was added.
- Polish: "previous" after the queue ended now replays the last item (unit-tested).
- 🖥 Deferred: Android only for now.
