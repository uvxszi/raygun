# M38 — Watch tab and native video player

**Relevant docs:** M06 (the native audio plugin and its JNI bridge), M05 (playback), `docs/feature-parity.md` (Watch: the player rows from aniyomi and CloudStream).

## Goal
- **A Watch tab** for movies and TV, starting with the player it needs: a native Android player that handles real-world streams (HLS, DASH, MP4/MKV with request headers and subtitles).
- **Mobile niceties:** gestures and picture-in-picture.
- **Remembering progress:** the player keeps where each video was left, for resuming and Continue watching.

## Design
- **`VideoPlayerActivity`** (Kotlin, in the `raygun-audio` plugin), on Media3 ExoPlayer with `PlayerView`:
  - **Controls:** the stock controller (seek bar, rewind and forward 10 s, speed, audio-track and subtitle menus).
  - **Streams:** HLS and DASH through `media3-exoplayer-hls`/`-dash`; MP4, MKV and WebM through the default extractors. Request headers are applied to every request (a `User-Agent` and `Referer` many hosts require).
  - **Subtitles:** side-loaded subtitles as `SubtitleConfiguration`s (SRT, WebVTT, SSA/ASS, TTML; the type comes from the format or extension).
  - **Gestures:**
    - double-tap the left or right third to seek 10 s, or the middle to play and pause;
    - tap to toggle the controls;
    - swipe up and down on the left half for brightness and the right half for volume, with an on-screen hint.
  - **Window:**
    - landscape, immersive and screen-on;
    - leaving while playing enters picture-in-picture at the video's aspect ratio;
    - leaving while not playing pauses.
  - **Reports:** `{id, state, positionMs, durationMs, error}` every 5 s while playing, on pause, at the end and on close, sent through `NativeBridge.onVideo` (JNI) to Rust.
- **Rust:**
  - **The plugin:** `NativeAudio::video_open` and `set_video_handler`.
  - **`raygun-app::video`:**
    - `play(VideoRequest)` pauses music, records the entry and opens the player at the saved position (from the start once watched or past 95%).
    - Reports save progress (`0012_video.sql`, `raygun-db::video`, synced conventions); past 90%, or at the end, the video counts as watched.
    - It emits `WatchChangedEvent`.
  - **Requests:** a `VideoRequest` holds a content key (`url:<address>` for links, `stremio:…` from M39), title, poster, URL, headers, format and subtitles.
  - **Resuming:** the request is stored, so Continue watching replays exactly what was played.
  - **Other platforms:** not available ("only on Android for now"), following the mobile-first rule.
- **UI:**
  - **Navigation:** a new Watch tab (Home, Library, Watch, Search, Settings).
  - **Continue watching:** posters with a progress bar; tap to resume, or ✕ to forget.
  - **Play a video link:** a video address plus an optional subtitle address.
  - **Later:** catalogs fill the rest of the tab in M39.

## Out of scope
- **Remaining video work:** Stremio catalogs and streams (M39), video sources (M40), and video downloads and library (M41).
- **Player extras:** OP/ED skip (needs chapter data from sources, M40), external players, and video podcasts in this player.

## Done-check
- **Rust:** watching progress resumes, finishes (rewinding) and is removed; link requests (key, title, HLS/DASH detection, subtitles).
- **Vitest:** the Watch tab is in the navigation and plays a link with subtitles; Continue watching resumes, removes and refreshes on events.
- 📱 **On the emulator:**
  - play an HLS test stream (Mux's public test stream) in landscape, and use double-tap seek and a volume swipe;
  - leave the app for picture-in-picture;
  - come back, close the player, see it under Continue watching, and resume at the same spot.

## Decisions
- **ExoPlayer in a native activity, not `<video>` in the webview:** it handles HLS and DASH with request headers, MKV, side-loaded ASS subtitles, track menus, picture-in-picture and decoder selection, which the webview's element doesn't, and it's what CloudStream uses. aniyomi's mpv is heavier and needs native libraries; ExoPlayer is already in the app for audio.
- **Recovering from decoder failures:** a hardware decoder that fails mid-playback triggers one retry from the same spot with software decoders first. Media3's own decoder fallback is also on, for decoders that fail to start. The emulator's H.264 decoder failed on the 1080p variant of the test stream, and phones with buggy decoders hit the same thing.
- **Navigation:** the tab bar now uses equal columns for however many tabs there are, so the fifth tab doesn't wrap.

## Verification (2026-11-01, Pixel_4 emulator)
- **Checks:** all pass. `cargo test --workspace`, clippy, `pnpm lint`, `pnpm type-check` and `pnpm test` (225 app tests).
- **Navigation:** the tab bar shows Home, Library, Watch, Search and Settings in one row.
- **First try:** pasting Mux's HLS test stream opened the player in landscape (10:35 long), but the emulator's hardware H.264 decoder failed and the picture stayed black. That led to the software-decoder retry.
- **Playing and gestures:**
  - After the fix, the stream played (Big Buck Bunny).
  - Double-tapping the right side jumped forward 10 s, with a "10s »" hint.
- **Picture-in-picture:** pressing Home moved the video into a picture-in-picture window that kept playing over the home screen and over Raygun.
- **Continue watching:** back in Watch, the video was listed with a progress bar.
- **Cleanup:** the app data was reset after the check.
