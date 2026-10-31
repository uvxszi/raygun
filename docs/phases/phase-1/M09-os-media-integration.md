# M09 — OS media integration (Android)

**Relevant docs:** ARCHITECTURE §6.2 (canonical state; OS controls read it), M06 decision (follow-ups), M04/M07 follow-ups (status bar). Desktop media keys (SMTC/MPRIS) are deferred, since the app is Android-only for now.

## Goal
The phone's own media controls drive Raygun's queue, and Raygun looks right in the system UI:
- notification, lock screen and headset/Bluetooth buttons: play/pause, **next/previous through Raygun's queue**
- artwork in the notification
- status bar icons readable in every theme, with nothing scrolling under them
- Android Back closes the full player instead of leaving the page

## Design
- **Next/previous:** the media session gets a `ForwardingPlayer` around ExoPlayer.
  - It advertises `COMMAND_SEEK_TO_NEXT`/`PREVIOUS` (and the `*_MEDIA_ITEM` variants), so Android shows the buttons even though the session holds one item at a time.
  - It overrides `seekToNext*`/`seekToPrevious*` to send a **control** over JNI (`NativeBridge.onControl("next" | "previous")`) instead of seeking.
  - Rust maps the control to `PlaybackControl::Next` / `Previous`, keeping the queue's own rules (restart after 3 s, repeat modes). A separate JNI entry and handler keep controls apart from engine reports.
- **Artwork:** `EngineCommand::Load` gains `artwork_url`, which the native engine sets as the session's artwork URI. Media3 loads it from the loopback proxy, which the release network config already allows.
- **Status bar:** the plugin gets an `applyTheme(theme)` command.
  - It resolves `system` against the OS night mode and sets `WindowInsetsControllerCompat.isAppearanceLightStatusBars` and the navigation-bar equivalent. It re-applies on resume, so a system dark-mode switch is picked up.
  - Rust calls it at startup and after every theme change.
  - The UI adds a fixed top scrim, `env(safe-area-inset-top)` tall in the background colour, so scrolled content doesn't run under the icons.
- **Back closes the sheet:** opening the sheet pushes a history entry (same URL, marked state), and `popstate` while open closes it. Closing it any other way pops that entry, so Back keeps working normally afterwards.

## Done-check
- Kotlin builds; Rust and TS tests cover control parsing and the back-button behavior.
- 📱 The notification shows previous/next and the artwork. `KEYCODE_MEDIA_NEXT`/`PREVIOUS`/`PLAY_PAUSE` move through Raygun's queue and the app UI follows.
- 📱 Status bar icons are light on dark/AMOLED and dark on light. Content doesn't show behind the status bar when scrolled.
- 📱 With the sheet open, Back closes it and stays on the page; a second Back navigates as usual.

## Verification (2026-10-31, Pixel_4 emulator)
Test album: `make_test_library <dir> 1 1 3 120 "AAA Long "` (three 2-minute tracks).

| Check | Result |
|---|---|
| Notification controls | title, artist, pause, **previous and next**; the card is tinted from the album artwork (artwork reached the session) |
| `KEYCODE_MEDIA_NEXT` ×2 / `PREVIOUS` / `PLAY_PAUSE` ×2 | Song 01 → 02 → 03 → 02, paused, playing (through Raygun's queue) |
| Notification next button | advanced to the next queue item |
| App follows system controls | mini-player showed the new track and the paused state |
| Status bar icons | dark on Light, light on Dark/AMOLED, dark on System with the OS in light mode |
| Scrolled content under the status bar | hidden by the scrim |
| Back with the sheet open | closes the sheet, stays on the page; a second Back navigates to the previous page |

Found and fixed during verification: the first session player wrapped listeners with Kotlin `by` delegation. That **doesn't forward Java default interface methods**, and every `Player.Listener` method is one, so the session received no events (stuck in BUFFERING, no metadata). It is now built on Media3's `ForwardingSimpleBasePlayer`, which derives events from state; adding commands is just a `getState()` change, and `handleSeek` intercepts next/previous.

## Follow-ups
- The `raygun-audio` plugin now also owns the storage permission and the system-bar styling. Rename it to `raygun-android` when the next native feature lands.
- With theme = System, the bars re-check the OS mode on resume. A live dark-mode switch while the app is in front updates them on the next resume.
