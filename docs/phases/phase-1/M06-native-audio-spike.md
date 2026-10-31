# M06 ⚗ Native Android audio (spike)

**Relevant docs:** ARCHITECTURE §6.2 (engines), §13 (risk: mobile background audio). reference-notes → RiPlay (`HybridPlayer`: native player behind the same facade).

## Question
Can the webview `<audio>` engine stay the Android audio engine, or does Android need a native engine for background playback and lock-screen controls?

## Baseline measured on Pixel_4, M05 build
- The webview `<audio>` element keeps playing (`dumpsys audio`: `state:started`) after HOME and for 60 s with the screen off.
- **But** the backgrounded process sits at `oom_score_adj 900` (cached), with no foreground service and no media session. Android may freeze or kill it at any time on a real device. There's also no notification, no lock-screen controls and no headset buttons.

So the webview alone is not acceptable for a music app on Android.

## Spike design
- **`plugins-mobile/raygun-audio`** (Tauri plugin crate `tauri-plugin-raygun-audio`, Kotlin package `app.raygun.audio`):
  - `RaygunPlaybackService`: a Media3 `MediaSessionService` owning an `ExoPlayer`. It handles audio focus, becoming-noisy (headphones unplugged) and a network wake lock. Media3's default notification provides the notification and lock-screen controls.
  - `RaygunAudioPlugin`: Tauri commands `load|play|pause|seek|setVolume|stop`, executed through a `MediaController` connected to the service, which starts the foreground service while playing.
  - Events go back to Rust **over JNI** (`NativeBridge.onEngineEvent(json)`), not through the webview. Auto-advance therefore works even while the webview's JavaScript is paused in the background.
  - The JSON is exactly the `EngineReport` shape, so `PlaybackCore` doesn't know which engine is running.
- **Rust:** `PlaybackService` gets an engine target. On Android, engine commands go to the plugin; elsewhere, to the webview event. The JNI handler hands reports to the async runtime, so the main thread is never blocked.
- **Protocol:** `EngineCommand::Load` gains `title`/`artist` so native engines can fill the media session metadata.
- **Sources:** ExoPlayer plays the same loopback proxy URLs (header injection works unchanged).

## Done-check
- 📱 Play, HOME, screen off for over 60 s: the audio continues and `oom_score_adj` is at foreground-service level (≤ 250).
- 📱 A media notification and lock-screen controls show the title. Pause from the notification pauses, and the in-app state follows.
- 📱 Auto-advance to the next queue item works while the app is in the background.
- Decision recorded below.

## Results (2026-10-31, Pixel_4 emulator, Android 17 image)
| Check | Webview `<audio>` (M05) | Native Media3 engine |
|---|---|---|
| Plays after HOME and 60 s with the screen off | yes | yes |
| Process priority in background (`oom_score_adj`) | **900** (cached, freezable/killable) | **50** (HOME), **200** (screen off) |
| Media session / notification / lock-screen controls | none | active session, foreground media notification with title, pause and seek |
| Pause from the system controls reaches app state | n/a | yes (the mini-player switched to Play) |
| Auto-advance while backgrounded with the screen off | not guaranteed (JS may be paused or frozen) | yes (`tone-a.wav` → `long.wav` after 40 s, over JNI) |

Integration notes:
- Tauri only includes a plugin's Android module when the app shell crate depends on the plugin **directly** (Cargo `links` metadata does not reach indirect dependents).
- JNI reports go into an `mpsc` queue drained by one worker thread. That keeps them ordered and keeps the Android main thread free.

## Decision
**Android plays through the native Media3 engine, always.** The webview `<audio>` engine stays for desktop and for previews.

Follow-ups:
- **M09:** route the notification/lock-screen/headset *next* and *previous* buttons to the Rust queue. Today the session holds one item at a time, so the system only offers previous (restart) and no next. This needs a `MediaSession.Callback` that forwards skip commands over JNI as playback controls.
- **M09:** artwork in the media session metadata, once items have artwork (M07).
- **Video (Phase 6):** video items will need either a native surface or the webview engine in the foreground; decide with MpvEngine/M45.
- Not checked here: a visible lock screen (the emulator has no screen lock set; the lock screen shows the same session) and Doze on a real device.
