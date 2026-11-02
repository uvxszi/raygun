# M45 — Player controls and settings

**Relevant docs:**
- `docs/research/aniyomi.md` §2–7 (overlay, gestures, tracks, chapters and skip intro, tools, settings);
- `docs/research/cloudstream.md` §5 (player);
- `docs/research/stremio.md` §2 and §8 (per-stream player state, subtitle and audio preferences);
- M38 (player), M44 (sessions).

## Goal
The video player should match Aniyomi's and CloudStream's, within what Media3 can do:
- its own controls instead of Media3's stock ones;
- gestures that can be set up;
- chapters and skipping intros;
- full control of subtitles and audio;
- tools (sleep timer, screenshots, another app, decoder);
- the per-title memory Stremio has;
- Settings → Player split into categories, like Aniyomi's.

## Design

### Overlay
`VideoPlayerActivity` keeps `PlayerView` for the picture and subtitles (`useController = false`) and draws its own controls (`PlayerControls.kt`).
- **Top bar:**
  - back, the title with the episode;
  - then Episodes and Sources (sessions), Audio, Subtitles, and ⋮ More.
- **Centre:** previous · play/pause · next, and a spinner while buffering.
- **Seek row:** position · seek bar (Media3's `DefaultTimeBar`, chapter markers) · duration. Tapping the duration shows the time left instead, and that choice is remembered.
- **Bottom row:**
  - left: lock, rotate, the speed chip (a tap cycles the presets, a long press opens the speed sheet), the current chapter (opens Chapters);
  - right: the skip button, aspect, picture-in-picture.
- **Timeout and toasts:** controls hide after the timeout (a setting). A centred pill reports speed, aspect, seeking, track and decoder changes.
- **Lock:** hides everything but an unlock button, which shows on a tap. Gestures and keys do nothing while locked.
- **Next-episode prompt and dialogs:** the prompt (M44) stays. The sheets are bottom sheets in the player's dark style, not system dialogs.

### Gestures (`PlayerGestures.kt`)
- **Brightness and volume:** vertical swipes on the left (brightness) and right (volume) halves, each with a vertical bar and a percentage.
  - Each can be turned off, and the sides swapped.
  - **Volume boost:** past 100 % the volume keeps rising, up to the boost cap (`LoudnessEnhancer` on the player's audio session).
  - **Dimming:** below 0 % brightness, the picture dims with a black layer.
  - **Remember brightness** (a setting) restores the last brightness.
- **Horizontal seek:** a horizontal swipe previews "+0:23 · 12:34" and seeks on release. It can be turned off.
- **Double tap:**
  - left, centre and right can each be set to seek, play/pause, switch episode (previous on the left, next on the right) or nothing;
  - repeated taps add up (+10 s, +20 s…).
- **Long press:** holds the speed at 2× (a setting: off, 1.5×, 2×, 3×) until released.
- **Keys:**
  - media previous/next switch episodes or seek (a setting); play/pause toggles;
  - on a keyboard or remote: the arrows seek, space toggles, and up/down change the volume.

### Speed, aspect, orientation
- **Speed sheet:**
  - a slider (0.25–4×) and the presets as chips;
  - "Make default", and pitch correction (on: the voice keeps its pitch).
  - The speed is remembered per title; new titles start at the default.
  - The presets are edited in Settings.
- **Aspect:** Fit · Crop · Stretch, remembered per title; the default is a setting.
- **Orientation:**
  - a default setting: free, follow the video, portrait, reverse portrait, sensor portrait, landscape, reverse landscape, sensor landscape (the default);
  - the rotate button switches between portrait and landscape for this viewing.

### Chapters and skipping
- **Where chapters come from:**
  - **Sources:** the SDK's `Video` gets `timestamps` (start, end, name, `type`: opening, ending, recap, mixedOp, other), carried through `VideoRequest.chapters`;
  - **AniSkip** for anime (Settings → Player → Skip intro, off by default). Anime means a video-source title of type `anime`, or a Stremio id from Kitsu, MAL or AniList.
    - The MAL id comes from the id itself, or from AniList's public GraphQL search by title, kept per title.
    - The skip times come from `api.aniskip.com/v2` for the episode number and length.
    - "Not when the video has chapters" (on) skips the lookup when the source gave chapters.
- **Showing chapters:** markers on the seek bar, a Chapters sheet (jump to one), and the chip with the current chapter.
- **Skip button:**
  - inside an opening, ending or recap it reads "Skip opening" (or ending, recap) and jumps to that chapter's end;
  - otherwise it reads "+85 s", the intro length: per title, with a default in settings. A long press on it sets the title's length.
  - It can be hidden.
- **Auto-skip** skips typed chapters at once.
- **Netflix style** puts a countdown on the button instead (3/5/7/10 s), skipping unless cancelled.

### Subtitles and audio (`PlayerTracks.kt`)
- **Subtitles sheet:**
  - the tracks plus Off, and a second track (a subtitle file) shown at the same time at the top of the picture;
  - add a subtitle file from the device;
  - delay (±0.1 s / ±1 s, reset);
  - a link to the style sheet.
- **Delay:** a text renderer wrapper renders cues at the position plus the delay (CloudStream's approach), for both tracks.
- **Second track:** a subtitle file parsed in the player (Media3's parsers) and shown in its own `SubtitleView`; Media3 maps every text track to one renderer, so a second renderer would never get one.
- **Audio sheet:** the tracks (language, label, channels).
- **Choosing tracks by themselves:**
  - subtitle language and a fallback (Stremio's secondary language), audio language and a fallback;
  - words that make a track preferred or skipped (Aniyomi's whitelist/blacklist; "Signs" skips sign-only tracks).
- **Style** (Settings → Player → Subtitles, and from the player):
  - size, font (default, sans, serif, mono), bold, italic;
  - text colour, edge (none, outline, shadow, raised, depressed) and its colour, background colour and opacity;
  - vertical position, and ignoring the file's own styles.
  - Settings shows a live preview.
- **Remembered per title:** the subtitle choice (language and label, or off), the second track, the delay, and the audio choice (language and label).

### Tools (⋮ More sheet)
- **Sleep timer:** 15/30/45/60 minutes or the end of this episode. It shows the time left in the sheet and the top bar, can be cancelled, and pauses when it runs out.
- **Screenshot:**
  - with or without subtitles (Pixel Copy of the video surface, subtitles drawn on top);
  - saved to Pictures/Raygun, then offered to share.
- **Open in another app:** the system chooser, or the app set in Settings. "Always use another app" opens it straight away.
  - It's passed the title, headers, subtitles and position, with MX Player's and VLC's extras.
  - The position they return is saved as progress.
- **Decoder:** Auto (hardware, falling back to software) · Software first, applied by re-preparing at the same position; the default is a setting.
- **Statistics:** Media3's debug text over the picture.

### Picture-in-picture
- **When it starts:**
  - on leaving the app while playing (a setting, on);
  - back can enter it instead of closing (a setting, off).
- **Window actions:** previous / play-pause / next in a session, or rewind / play-pause / forward, through a receiver registered only for this app.

### Rust
- **Settings:** `Settings.video_player: VideoPlayerPrefs`, one row, `#[serde(default)]` so later fields need no migration.
  - It holds every new option, including the subtitle style.
  - The existing player fields stay where they are.
- **Requests:** `VideoRequest` gains `group` (the title its memory belongs to), `chapters` and `anime` (title and episode number for AniSkip). All are `serde(default)`, so saved requests still read.
- **What the player is sent:** the load payload also carries `prefs`, `titleState` (the per-title memory) and `chapters`.
- **What the player asks for** (new requests, for sessions and single videos alike):
  - `state` merges the per-title memory into `local_value` `video.playerState.<group>`;
  - `prefs` saves a change made in the player (default speed, remaining time, style…) through the settings service, so the app's Settings follow;
  - `duration` starts the AniSkip lookup, answered with a `chapters` update;
  - `brightness` remembers the brightness.
- **AniSkip:** `aniskip.rs`. Pure parsing, tested; the network calls go through `raygun_net`.

### UI
- **Settings → Player** becomes a list of screens:
  - **Playback:** quality, resume, watched point, seek step, episodes, sources dialog;
  - **Controls:** orientation, aspect, controls timeout, time left, speed presets, default and pitch, long-press speed, remember brightness;
  - **Gestures:** brightness/volume swipes and sides, horizontal seek, double tap left/centre/right, media keys, boost cap;
  - **Subtitles:** auto, language and fallback, preferred and skipped words, style with preview;
  - **Audio:** language and fallback, pitch correction;
  - **Skip intro:** button, intro length, auto-skip, Netflix style and wait, AniSkip and its rule;
  - **Picture-in-picture**;
  - **Advanced:** decoder, another app (always use, which app).
- **Pickers:** the episode link picker also offers "Open in another app" and "Copy link".
- **The player's words:** come from `videoPlayer.*` as in M44.

## Out of scope
These go to later milestones, each with a reason:
- **Seek-bar thumbnails:** they need a source API for thumbnails (M47, SDK);
- **Video filters:** brightness, contrast and the like need a GL effect pipeline;
- **Audio delay, channel mixing and external audio files:** they need custom audio processors or merged sources;
- **mpv's custom buttons and config files:** there's no mpv;
- **Online subtitle search:** OpenSubtitles needs an API key, so it waits for accounts;
- **Chromecast:** M52;
- **Screenshot as cover:** custom covers are M47.

## Done-check
- **Rust:**
  - prefs validation;
  - the per-title state merge;
  - the `prefs` request applying a partial change;
  - AniSkip parsing and the anime check;
  - chapters parsed from extensions (`timestamps`).
- **Vitest:**
  - the Player category list and each screen's rows;
  - the subtitle style preview;
  - the picker's new actions.
- 📱 **On the emulator:**
  - the overlay (lock, aspect, rotate, speed sheet and chip, remaining time);
  - gestures (bars, boost, horizontal seek, double-tap accumulation, hold for 2×);
  - chapters and the skip button with a chaptered test source;
  - subtitles: tracks, a second track, delay, style, file from the device;
  - audio tracks;
  - the remembered speed, aspect and tracks on the next episode;
  - sleep timer, screenshot, another app (chooser), decoder switch, statistics;
  - PiP actions;
  - Settings → Player screens;
  - clean up afterwards.

## Decisions
- **Settings:** one `videoPlayer` row (`VideoPlayerPrefs`, `#[serde(default)]` on the struct).
  - A stored row missing later fields still loads. The generated TS type marks every field optional, so the UI fills the defaults (`withDefaults`, `PLAYER_DEFAULTS`).
  - The player's own changes (default speed, time left, pitch, decoder, subtitle style) go through a `prefs` request. Rust merges them into the saved settings and sends `SettingsChangedEvent`, so open Settings screens refresh.
- **Per-title memory** (`video.playerState.<group>`): speed, aspect, subtitle choice (language and label, or off), second track, subtitle delay, audio choice, intro length.
  - `VideoRequest::group` names the title: `title:<id>` for sources, `stremio:<type>:<id>` for Stremio. Sessions fill it in for every link; Stremio pickers send `metaId`.
  - Track choices are found again by language and label, so the next episode's matching tracks play.
- **Subtitle delay:** `OffsetTextRenderer` (a `ForwardingRenderer`, so Java's default methods are forwarded too) renders cues at the position minus the delay.
  - A change seeks to the same position after 0.7 s, so cues the renderer already dropped are read again.
  - The same delay applies to the second track.
- **Second track:** only subtitle files (the source's or one added from the device), not tracks inside the video. Media3 maps every text track to one renderer.
- **Subtitles are drawn over the whole screen,** in a `SubtitleView` of the player's own fed from `onCues`. The `PlayerView`'s subtitle view is hidden: it sits inside the picture's frame, which crop enlarges past the screen.
  - **Position:** the view's bottom padding sets it, not Media3's bottom-padding fraction, which doesn't move WebVTT cues (Media3 puts them on the last line).
  - **With the controls showing,** subtitles rise above the bottom bar, to 24 % of the height.
- **Crop:** Media3's zoom mode. The aspect is applied again when the video size arrives.
- **AniSkip:**
  - **Types:** plain openings and endings win over the "mixed" ones (kept only without a plain one), and overlapping times are dropped. AniSkip answers one result per type, and they overlap.
  - **MAL id:** comes from the Stremio id (`mal:`). For Kitsu, AniList and source titles of type `anime` it comes from AniList's search by name, kept per title (0 for "not found").
  - **Episode length:** sent, so times made for differently cut releases are left out.
- **Skip button:**
  - inside an opening, ending or recap: Skip, a countdown (Netflix style; it waits while paused), or an automatic skip, each once per chapter;
  - otherwise "+85 s" while the controls show. A long press sets the title's own length (Off, or 30–180 s).
- **Gestures (Aniyomi's):**
  - double-tap zones split at 2/5 and 3/5 of the width;
  - double taps on one side within a second add up;
  - sideways seek moves 0.15 s per pixel (scaled by density) and seeks on release;
  - swipes don't start within 32 dp of an edge, leaving the system's gestures alone;
  - brightness goes from -75 % (a black layer dims below the screen's lowest) to 100 %;
  - volume goes past 100 % by the boost cap, through `LoudnessEnhancer`, attached once the audio session id arrives (it starts as 0 in Media3 1.11).
- **Speed presets:** Aniyomi's twelve, 0.25×–4×. The chip goes to the next preset up, round to the first.
- **Orientation:** "Follow the video" turns to landscape or portrait once the video size is known; the rotate button overrides it for the viewing. Unlike Aniyomi, "reverse landscape" is reverse landscape.
- **Picture-in-picture:**
  - Android 12+ enters by itself through auto-enter, kept current with the playing state; earlier versions enter from `onUserLeaveHint`.
  - The window has previous / play-pause / next in a session, else rewind / play-pause / forward, through a not-exported receiver.
  - Statistics hide in the window.
- **Another app:**
  - the chooser when no app is set;
  - MX Player's `headers` array, VLC's long `position` and `subtitles_location`, mpv's `subs`;
  - the position comes back from `extra_position` or `position`, and `end_by == playback_completion` counts as the end.
  - "Always use another app" hands over each video as it loads and closes the player when that app returns.
- **Stremio subtitles:** an addon's optional `label` is now read. A label that is only a language code shows as the language's name, normalised by Media3 ("ger" and "deu" are German).
- **Sleep timer:** 15/30/45/60/90 minutes or the end of the episode, instead of Aniyomi's time picker. It pauses; at an episode's end it also stops autoplay.
- **Screenshots:** need Android 10+ (MediaStore's `RELATIVE_PATH`); earlier versions say they couldn't take one.
- **Not done:** listed under Out of scope, plus volume keys don't boost past 100 % (the system volume keys are left alone).

## Verification
- **Checks:**
  - `cargo test --workspace` passes, with new tests for:
    - the settings row: partial rows load, and bad values are refused;
    - the title memory merge;
    - player changes merging into the settings;
    - AniSkip parsing, with real overlapping answers;
    - the AniList MAL id;
    - the Stremio anime check and group keys;
    - extension `timestamps` becoming chapters.
  - `cargo clippy -D warnings`, `pnpm lint`, `pnpm type-check` and `pnpm test` pass (273 tests). New UI tests cover:
    - the Player category list;
    - controls, gestures, subtitles with the live preview, and skip/AniSkip and another app;
    - reading settings again on `SettingsChangedEvent`;
    - the pickers' Open in another app and Copy link.
  - The audio plugin's Kotlin compiles (`compileDebugKotlin`).
- 📱 **On the emulator (Android 17, debug build).** A local Stremio addon served "Attack on Titan" (`mal:16498`), two episodes of a 24-minute test video. Each episode has an English and a Japanese audio track, plus English, German and "English Signs" subtitle files.
  - **Settings:** Settings → Player lists its seven screens. AniSkip, the countdown, auto subtitles and the skip word "Signs" were turned on there.
  - **AniSkip:** the episode played, and the opening and ending marks appeared on the seek bar a few seconds after loading. Inside the opening the button counted down ("Skipping in 3 s", with Don't skip), then jumped past it.
  - **Controls:**
    - the speed chip cycled to 1.25×;
    - aspect cycled, with hints;
    - lock left only an unlock button, and swipes did nothing;
    - the time left and the next/previous buttons followed the session.
  - **Gestures:**
    - a sideways swipe previewed "+1:17 · 2:27 / 24:07" and seeked on release;
    - double taps on the right gave "10 s »", then "20 s »" when repeated;
    - volume went to 130 % (boost shown in red);
    - brightness went to -37 %, dimming the picture;
    - holding the picture played at 2× ("2× »").
  - **Tracks:**
    - the audio sheet listed English and Japanese, and picking Japanese played it (statistics showed the track);
    - subtitles listed English, German and English Signs by name;
    - a second track (German) showed at the top while the main one showed at the bottom;
    - with the controls up, subtitles sat above the bottom bar.
  - **Remembered per title:** the next episode and a fresh start kept 1.25×, crop (filling the screen), the Japanese audio, and both subtitle choices.
  - **Speed sheet:** a slider, the presets, Make default, Reset and pitch correction.
  - **More:**
    - statistics overlay;
    - a screenshot saved to Pictures/Raygun as "Attack on Titan S1 E1 · To You, in 2000 Years <time>.png", with Share offered.
  - **Picture-in-picture:** the window showed previous / pause / next, and pause worked from the window.
  - **Clean-up:** the saved screenshot was deleted and app data cleared.
- **Bugs found on the device and fixed:**
  - crop on the first load (restored aspects weren't applied once the video size arrived);
  - the second track drawn in the middle of the screen;
  - German shown as "ger";
  - a crash from placing subtitles before the layout existed;
  - the chapter chip still saying "Opening" after it ended;
  - subtitles hidden under the controls.
- **Not seen on the device:**
  - the sleep timer running out;
  - Open in another app (the emulator has no other video player);
  - the decoder switch;
  - adding a subtitle file from the device;
  - the subtitle delay and the style sheet in the player;
  - media keys;
  - PiP's previous/next;
  - auto-skip without the countdown.
