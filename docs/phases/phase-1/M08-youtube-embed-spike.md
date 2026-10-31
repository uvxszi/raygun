# M08 ⚗ YouTube embed engine (spike)

**Relevant docs:** ARCHITECTURE §6.2 (EmbedEngine), §13 (risk: YouTube policy changes). reference-notes → RiPlay (embed origin rules, `PlayerHtmlServer`, JS bridge).

## Question
Can YouTube videos play inside Raygun on Android through the official IFrame Player API, hosted on a loopback page, with Raygun's queue and controls driving it? What breaks (background, errors)?

## Design
- **Embed page:** the existing loopback server serves `GET /embed/youtube`, a self-contained HTML page that loads `https://www.youtube.com/iframe_api` and creates a player with `playsinline`, no YouTube controls, and `origin` set to the page's own origin.
  - The app loads it as `http://localhost:<port>/embed/youtube`, using the hostname **`localhost`, never `127.0.0.1`**. RiPlay found that YouTube's embed check reads the Referer string and refuses literal IPs.
  - Release Android builds already allow cleartext to `localhost` (M05 network security config).
- **Protocol:** `postMessage` between the app's webview and the embed iframe.
  - App → page: `{raygunCommand, type: load|play|pause|seek|volume|stop}`.
  - Page → app: `{raygunEmbed, type: ready|state|progress|error}`.
  - The app accepts messages only from the embed iframe's window and origin.
- **Model:**
  - `MediaSource::YouTube { video_id }`.
  - `EngineCommand::LoadYouTube { load_id, embed_url, video_id, start_ms, autoplay, volume }`.
  - YouTube error codes (2, 5, 100, 101, 150, 152, 153) become `EngineEvent::Error` with a readable message, so the core skips to the next item.
- **Engine routing per item:** `PlaybackService` decides per load. YouTube items always go to the webview's embed engine. Other items go to the native engine on Android and to `<audio>` elsewhere. When the engine changes, the previous one gets `Stop`, and play/pause/seek go to whichever engine is active. The decision is a pure function with tests.
- **Webview host:** `useEngineHost` owns both the `<audio>` engine and the embed engine and routes commands by type.
  - The embed iframe lives in one fixed-position container that is never moved in the DOM (moving an iframe reloads it).
  - While the player sheet is open on a video item, the container is positioned over the artwork area. Otherwise it sits off-screen at 320×180, so audio keeps going.
- **Entry point:** the "Open a stream or file" form recognizes `youtube.com/watch?v=`, `youtu.be/`, `/shorts/`, `music.youtube.com` and bare 11-character IDs.

## Done-check
- Tests:
  - Rust: routing decision, embed page served (HTML, no frame blocking), YouTube sources refused by `/stream`.
  - TS: embed engine command/report mapping, origin check, error mapping, YouTube URL parsing.
- 📱 A YouTube video plays from the form. The video shows in the player sheet, and pause/seek/next drive it. An invalid ID errors and skips to the next queue item.
- Switching between a YouTube item and a local track stops the other engine.
- Background behavior measured and recorded with the decision below.

## Results (2026-10-31, Pixel_4 emulator, debug APK)
| Check | Result |
|---|---|
| YouTube video plays from the form (`M7lc1UVf-VE`, the IFrame API's demo video) | **yes**: the embed loaded with `origin=http://localhost:<port>`, video in the player sheet, thumbnail in the mini-player |
| Progress and duration from the embed (0:12 / 22:23) | yes |
| Pause, seek (to 11:14), resume from Raygun's controls | yes |
| Invalid ID (`AAAAAAAAAAA`) queued before a local track | error reported and skipped to the local track within 1.5 s |
| Engine switch (YouTube embed → native Media3) | yes: the next item played on the native engine (its media session showed the local track) |
| Background (HOME for 30 s) | **paused**: the embed stops when the app isn't visible (0:11 → 0:12) |

Notes:
- While paused, YouTube draws its own title/share overlay even with `controls=0`.
- The embed reported the old position right after `seekTo`; it now reports 400 ms later (unit-checked only).
- On Android the URL fields capitalized "Https" and autocorrected; they now use `inputMode=url` with autocapitalize, autocorrect and spellcheck off (unit-checked only).
- Device automation note: Android's Back key navigates app history unless the keyboard is open, and the keyboard resizes the page (`interactive-widget=resizes-content`). Drive forms with care.

## Decision
**Keep the YouTube embed engine as a foreground video player and as the last-resort fallback.** It works with Raygun's queue and controls and needs no extraction, but it can't play in the background or on the lock screen (YouTube pauses hidden embeds, and its terms don't allow background embeds).

Consequences for the roadmap:
- **Background YouTube audio on Android needs direct stream extraction** played through the native engine. yt-dlp (M16) is desktop-only, so Android needs an InnerTube-based resolver running in the extension runtime (M14/M15). The resolver order stays direct → proxied → embed (ARCHITECTURE §6.1).
- The embed engine is the fallback when extraction fails, and the normal path for watching videos in the foreground.
