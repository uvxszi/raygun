//! Watching video (M38): the native player on Android (Media3, in the audio plugin) and the
//! progress it reports, for resuming and "continue watching". On desktop the webview plays it
//! (M46.4, `video_desktop`), with the same requests, updates and reports.
//! Progress also goes to the library's episodes, and finished downloads play instead of the
//! stream (M43). Episodes of a title play through a session (M44, `video_session`), which loads
//! videos into the open player and answers its requests.
//!
//! The player's controls (M45) follow `Settings::video_player`, and each title remembers its speed,
//! aspect, tracks and subtitle delay (`video.playerState.<group>`). The player saves changes made
//! in it through `request` reports, and asks for AniSkip's chapters once it knows the length.

use raygun_db::video::{self as store, VideoEntry};
use raygun_model::{
    Accent, AnimeEpisode, ContinueWatching, SettingChange, Settings, SubtitleMode,
    VideoPlayerPrefs, VideoRequest, VideoSubtitle,
};
use raygun_net::ImageLinks;
use serde_json::{Map, Value, json};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::catalog::now_ms;
use crate::events::{OpenRouteEvent, SettingsChangedEvent, WatchChangedEvent};
use crate::startup::AppDb;
use crate::video_session::{EpisodeRef, Origin, VideoSessions};

/// The player settings video playback follows (Settings → Player, M42.1).
struct PlayerPrefs {
    resume: bool,
    /// Past this share a video counts as watched.
    watched_share: f64,
    seek_ms: u32,
    /// The app's language, unless it follows the device (the player knows the device's).
    app_language: String,
    /// The first subtitle language; empty follows the device's languages, then the app's.
    subtitle_language: String,
    /// When subtitles turn on by themselves.
    subtitle_mode: SubtitleMode,
    /// The next episode plays by itself (M44).
    autoplay: bool,
    /// When the next-episode prompt shows, before the end.
    next_prompt_ms: u32,
    /// Music may play on beside a video: the player then leaves the audio focus alone.
    keep_music_playing: bool,
    /// The accent, as the player draws it (`#rrggbb`).
    accent: String,
    /// Controls, gestures, tracks and tools (M45).
    video_player: VideoPlayerPrefs,
}

/// When subtitles turn on by themselves: the player's setting, else the older switch.
fn subtitle_mode(settings: &Settings) -> SubtitleMode {
    settings
        .video_player
        .subtitle_mode
        .unwrap_or(if settings.auto_subtitles {
            SubtitleMode::Always
        } else {
            SubtitleMode::Foreign
        })
}

/// The accent the player draws with, as the app shows it on dark backgrounds (`styles.css`).
fn accent_color(settings: &Settings) -> String {
    match settings.accent {
        Accent::Red => "#ff6a65",
        Accent::Orange => "#ffa242",
        Accent::Green => "#5edb81",
        Accent::Teal => "#47d6cf",
        Accent::Blue => "#65adff",
        Accent::Purple => "#bb91ff",
        Accent::Pink => "#ff81ba",
        Accent::Custom => settings.accent_color.as_str(),
    }
    .to_owned()
}

/// Where a title's player memory is kept, after its group.
const STATE_PREFIX: &str = "video.playerState.";
/// The brightness the player last had, for "remember brightness".
const BRIGHTNESS_KEY: &str = "video.brightness";
/// What a title remembers: speed, aspect, subtitle and audio choices, subtitle delay, intro length.
const STATE_KEYS: [&str; 8] = [
    "speed",
    "aspect",
    "subtitle",
    "secondarySubtitle",
    "subtitleDelayMs",
    "audio",
    "introMs",
    // Subtitle lines changed by hand in the desktop player, per episode and track (M46.12).
    "subtitleEdits",
];

async fn player_prefs(db: &raygun_db::Db) -> PlayerPrefs {
    // As they apply: Data Saver caps the picture size while it's on.
    let settings = crate::settings::load_in_effect(db)
        .await
        .unwrap_or_default();
    let app_language = match settings.language.as_str() {
        raygun_model::SYSTEM_LANGUAGE => String::new(),
        tag => tag.to_owned(),
    };
    PlayerPrefs {
        resume: settings.resume_videos,
        watched_share: f64::from(settings.watched_percent) / 100.0,
        seek_ms: settings.seek_seconds * 1000,
        app_language,
        subtitle_language: settings.subtitle_language.clone(),
        subtitle_mode: subtitle_mode(&settings),
        autoplay: settings.autoplay_next,
        next_prompt_ms: settings.next_prompt_seconds * 1000,
        keep_music_playing: settings.keep_music_playing,
        accent: accent_color(&settings),
        video_player: settings.video_player.clone(),
    }
}

/// `payload` with what every request to the player carries besides the video: the seek step,
/// languages, accent and User-Agent.
fn with_shared_fields(mut payload: Value, prefs: &PlayerPrefs) -> Value {
    if let Value::Object(fields) = &mut payload {
        fields.insert("seekStepMs".into(), json!(prefs.seek_ms));
        fields.insert("appLanguage".into(), json!(prefs.app_language));
        fields.insert("subtitleLanguage".into(), json!(prefs.subtitle_language));
        fields.insert("subtitleMode".into(), json!(prefs.subtitle_mode));
        fields.insert("keepMusicPlaying".into(), json!(prefs.keep_music_playing));
        fields.insert("accent".into(), json!(prefs.accent));
        // What sources were asked with: some links only play for the same User-Agent.
        fields.insert("userAgent".into(), json!(raygun_net::user_agent()));
    }
    payload
}

/// A report from the player.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlayerReport {
    id: String,
    state: String,
    position_ms: f64,
    duration_ms: Option<f64>,
    error: Option<String>,
    /// The video loops: passing the watched point on each round doesn't finish it.
    #[serde(default)]
    looping: bool,
    /// A live stream: its position is the stream's own clock, not a place to resume.
    #[serde(default)]
    live: bool,
}

/// Plays `request` in the video player, resuming where it was left (from the start once watched).
/// A session that was playing ends.
pub async fn play(app: &AppHandle, request: VideoRequest) -> Result<(), String> {
    play_from(app, request, None).await
}

/// [`play`] from `start_at` (ms) instead of where it was left, when given.
async fn play_from(
    app: &AppHandle,
    request: VideoRequest,
    start_at: Option<i64>,
) -> Result<(), String> {
    app.state::<VideoSessions>().end();
    if let Some(video_id) = request.url.strip_prefix(YOUTUBE_LINK) {
        return play_youtube(app, video_id, &request.title, request.subtitle.clone()).await;
    }
    // A link played on its own plays through a session, for the player's Sources and Quality:
    // its torrent's other videos, the sources of the movie it was matched with.
    if crate::video_links::is_link(&request) {
        return crate::video_links::play_request(app, request, start_at).await;
    }
    // A torrent takes a while to open (a magnet link's details come from peers): the player
    // opens at once with the title and waits.
    if raygun_torrent::is_torrent(&request.url, request.format.as_deref())
        && crate::video_downloads::offline(app, &request.key)
            .await
            .is_none()
    {
        if let Some(reason) = crate::torrents::unavailable_now(app).await {
            return Err(reason);
        }
        let sessions = app.state::<VideoSessions>();
        let mut shell = session_shell(
            app,
            &request.title,
            request.subtitle.as_deref().unwrap_or_default(),
        )
        .await;
        shell["session"] = json!(false);
        shell["loadingText"] = json!(sessions.label("torrentFindingPeers", "Finding peers…"));
        open_player(app, &shell)?;
        return match prepare(app, &request, None, start_at).await {
            // Opened again with the video (an update could come before the window exists).
            Ok(mut payload) => {
                payload["labels"] = sessions.labels();
                open_player(app, &payload)
            }
            Err(error) => {
                let update = json!({ "type": "error", "text": error });
                let _ = update_player(app, &update);
                // A failure at once can come before the window exists: it's told again.
                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                update_player(app, &update)
            }
        };
    }
    let mut payload = prepare(app, &request, None, start_at).await?;
    payload["labels"] = app.state::<VideoSessions>().labels();
    open_player(app, &payload)
}

/// A video source's link to a YouTube video (M46.35): `youtube:<video id>`.
pub const YOUTUBE_LINK: &str = "youtube:";

/// Plays a YouTube video in YouTube's embed with its own controls, as trailers do (M46.4); the
/// video player isn't used.
pub async fn play_youtube(
    app: &AppHandle,
    video_id: &str,
    title: &str,
    subtitle: Option<String>,
) -> Result<(), String> {
    use tauri_specta::Event;
    // YouTube's embed goes to the network itself: not while Offline Mode is on.
    raygun_net::network_mode::check_online()?;
    let embed_url = crate::youtube_embed::url(
        &app.try_state::<raygun_net::MediaLinks>()
            .ok_or("The YouTube player isn't available")?
            .youtube_embed_url(),
    );
    crate::playback::other_media_started(app).await;
    crate::events::YouTubeVideoEvent(raygun_model::YouTubeVideo {
        video_id: video_id.to_owned(),
        title: title.to_owned(),
        subtitle,
        embed_url,
    })
    .emit(app)
    .map_err(|error| error.to_string())
}

/// Plays `request` in another app (M45): the player hands it over at once and saves the position
/// that app gives back.
pub async fn play_elsewhere(app: &AppHandle, request: VideoRequest) -> Result<(), String> {
    app.state::<VideoSessions>().end();
    let mut payload = prepare(app, &request, None, None).await?;
    payload["labels"] = app.state::<VideoSessions>().labels();
    payload["external"] = json!(true);
    open_player(app, &payload)
}

/// The player's request for a video: where to start (saved progress unless `start_at`), after
/// recording that it started. `resume` replaces the request saved for resuming (a session saves
/// an episode reference). A finished download plays instead of the stream. A torrent opens now
/// and plays through the stream proxy (`torrents`); a torrent playing before closes.
pub async fn prepare(
    app: &AppHandle,
    request: &VideoRequest,
    resume: Option<String>,
    start_at: Option<i64>,
) -> Result<serde_json::Value, String> {
    let torrent = raygun_torrent::is_torrent(&request.url, request.format.as_deref());
    let downloaded = crate::video_downloads::offline(app, &request.key).await;
    let torrents = app.try_state::<std::sync::Arc<crate::torrents::TorrentService>>();
    // A torrent's own subtitle files ("Sintel.en.srt") come with its video as tracks.
    let (address, torrent_subtitles) = match (&downloaded, &torrents) {
        // Its download plays: nothing is fetched.
        (Some(offline), _) if torrent => (offline.url.clone(), Vec::new()),
        (None, Some(torrents)) if torrent => {
            let stream = torrents.address_for(request).await?;
            (stream.address, stream.subtitles)
        }
        (None, None) if torrent => return Err(crate::torrents::TORRENTS_OFF.into()),
        _ => (playable_address(app, &request.url)?, Vec::new()),
    };
    // Anything else the player loads closes the torrent it played.
    if !(torrent && downloaded.is_none())
        && let Some(torrents) = &torrents
    {
        torrents.other_video().await;
    }
    // Offline Mode: only what's on this device plays (a download, a file, the proxy's).
    raygun_net::network_mode::check(&address)?;
    let db = app.state::<AppDb>().0.clone();
    let prefs = player_prefs(&db).await;
    let saved = store::progress(&db, &request.key)
        .await
        .map_err(|error| error.to_string())?;
    let resumed = saved
        .filter(|_| prefs.resume)
        .filter(|progress| progress.watched_at.is_none())
        .filter(|progress| {
            progress.duration_ms.is_none_or(|duration| {
                (progress.position_ms as f64) < duration as f64 * prefs.watched_share
            })
        })
        .map_or(0, |progress| progress.position_ms);
    // A live stream starts at its live edge: no place in it is kept or carried over.
    let start_ms = if request.live {
        0
    } else {
        start_at.unwrap_or(resumed)
    };
    let entry = VideoEntry {
        key: request.key.clone(),
        title: request.title.clone(),
        subtitle: request.subtitle.clone(),
        // The source's address: a proxy link wouldn't outlive the app.
        poster: request
            .poster
            .as_deref()
            .map(|poster| app.state::<raygun_net::ImageLinks>().original(poster)),
        request: match resume {
            Some(resume) => resume,
            None => serde_json::to_string(&request).map_err(|error| error.to_string())?,
        },
    };
    // Incognito (M47): it doesn't go into Continue watching.
    if !crate::privacy::incognito(app) {
        store::started(&db, &entry, now_ms())
            .await
            .map_err(|error| error.to_string())?;
    }
    // A finished download plays from the device, with its saved subtitles; a torrent's file
    // plays as a file (its link's headers and format are the torrent's, not the file's).
    let (url, headers, format, subtitles) = match downloaded {
        Some(offline) => (
            offline.url,
            Default::default(),
            offline.format,
            offline.subtitles,
        ),
        None if torrent => (
            address,
            Default::default(),
            None,
            proxied_subtitles(
                app,
                &[torrent_subtitles, request.subtitles.clone()].concat(),
            ),
        ),
        None => (
            address,
            request.headers.clone(),
            request.format.clone(),
            proxied_subtitles(app, &request.subtitles),
        ),
    };
    // AI subtitles made for this video (M46.33) play like any other track; a live stream's
    // are timed by a clock of that playing only (`captions::video_audio`).
    let mut subtitles = subtitles;
    if !request.live
        && let Some(captions) = app.try_state::<std::sync::Arc<crate::captions::CaptionService>>()
        && let Some(track) = captions.video_track(&request.key).await
    {
        subtitles.push(track);
    }
    // The title's memory, and AniSkip unless the source's own chapters are enough.
    let group = request.group.clone().unwrap_or_else(|| request.key.clone());
    let title_state = title_state(&db, &group).await;
    let player = &prefs.video_player;
    let anime = request.anime.as_ref().filter(|_| {
        player.aniskip && !(player.aniskip_unless_chapters && !request.chapters.is_empty())
    });
    let settings = player_settings(&db, &prefs).await;
    // Music and video don't play over each other, unless the settings say they may.
    crate::playback::other_media_started(app).await;
    let payload = serde_json::json!({
        "group": group,
        "titleState": title_state,
        "chapters": request.chapters,
        "anime": anime,
        "prefs": settings,
        // "Always use another app" (Settings → Player → Advanced).
        "external": player.external_player,
        "id": request.key,
        "url": url,
        "title": request.title,
        // Never the title again: "Sintel" under "Sintel" isn't shown, "Show - 03" is "03".
        "subtitle": request
            .subtitle
            .as_deref()
            .map(|subtitle| raygun_resolver::video_match::without_title(subtitle, &request.title))
            .filter(|subtitle| !subtitle.is_empty()),
        "headers": headers,
        "format": format,
        "subtitles": subtitles,
        "startMs": start_ms,
        "live": request.live,
        // Anime are taken as spoken in Japanese when the stream doesn't say, so subtitles in
        // the viewer's languages turn on.
        "spokenLanguage": request.anime.as_ref().map(|_| "ja"),
        // For a TV's screen while it plays there (Play on TV): the source's address.
        "poster": request
            .poster
            .as_deref()
            .map(|poster| app.state::<raygun_net::ImageLinks>().original(poster)),
    });
    Ok(with_shared_fields(payload, &prefs))
}

/// The address the player opens for a request's `url`: a web address as it is; a file on this
/// device (`file://`, one another app opened with Raygun) through the stream proxy, so it stays
/// the request's address for resuming while the webview's player gets a web address; and on
/// Android a document another app shared (`content://`), which the player reads itself.
fn playable_address(app: &AppHandle, url: &str) -> Result<String, String> {
    if url.starts_with("https://") || url.starts_with("http://") {
        return Ok(url.to_owned());
    }
    if url.starts_with("file:") {
        let path = crate::open_with::file_path(url)
            .ok_or_else(|| "This file's address can't be read".to_owned())?;
        return Ok(app
            .state::<crate::playback::PlaybackService>()
            .register_source(&raygun_model::MediaSource::LocalFile {
                path: path.to_string_lossy().into_owned(),
            }));
    }
    if cfg!(target_os = "android") && url.starts_with("content://") {
        return Ok(url.to_owned());
    }
    Err("Only web addresses can be played".into())
}

/// What opens the player for a session before any video is known (M44): it shows the title and
/// a spinner until the first `load`.
pub async fn session_shell(app: &AppHandle, title: &str, subtitle: &str) -> serde_json::Value {
    let db = app.state::<AppDb>().0.clone();
    let prefs = player_prefs(&db).await;
    let settings = player_settings(&db, &prefs).await;
    let shell = serde_json::json!({
        "session": true,
        "prefs": settings,
        "id": "",
        "title": title,
        "subtitle": subtitle,
        "autoplay": prefs.autoplay,
        "nextPromptMs": prefs.next_prompt_ms,
        "labels": app.state::<VideoSessions>().labels(),
    });
    with_shared_fields(shell, &prefs)
}

/// Settings changed in the app (Settings → Video player, the subtitle language, the seek step)
/// while a video plays: the open player follows them at once (the subtitles' style included)
/// instead of from its next video. Nothing happens without an open player.
pub async fn settings_changed(app: &AppHandle) {
    let db = app.state::<AppDb>().0.clone();
    let prefs = player_prefs(&db).await;
    let settings = player_settings(&db, &prefs).await;
    if let Err(error) = update_player(app, &prefs_update(settings, &prefs)) {
        tracing::debug!(%error, "no player to follow the settings");
    }
}

/// The `prefs` update: the player's settings and what every request carries besides the video.
fn prefs_update(settings: Value, prefs: &PlayerPrefs) -> Value {
    with_shared_fields(json!({ "type": "prefs", "prefs": settings }), prefs)
}

/// The player's settings as it reads them, with the brightness to restore when it's remembered.
async fn player_settings(db: &raygun_db::Db, prefs: &PlayerPrefs) -> Value {
    let mut value = serde_json::to_value(&prefs.video_player).unwrap_or_default();
    if prefs.video_player.remember_brightness
        && let Ok(Some(saved)) = raygun_db::local_value(db, BRIGHTNESS_KEY).await
        && let Ok(level) = saved.parse::<f64>()
    {
        value["brightness"] = json!(level);
    }
    value
}

/// A title's player memory; null when it has none.
async fn title_state(db: &raygun_db::Db, group: &str) -> Value {
    raygun_db::local_value(db, &format!("{STATE_PREFIX}{group}"))
        .await
        .ok()
        .flatten()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .filter(Value::is_object)
        .unwrap_or(Value::Null)
}

/// Merges what the player changed into a title's memory. Null forgets a key; keys a title doesn't
/// remember are ignored.
fn merge_state(saved: &mut Map<String, Value>, change: &Map<String, Value>) {
    for (key, value) in change {
        if !STATE_KEYS.contains(&key.as_str()) {
            continue;
        }
        if value.is_null() {
            saved.remove(key);
        } else {
            saved.insert(key.clone(), value.clone());
        }
    }
}

/// Applies a change made in the player to the saved player settings: known keys only, and the
/// subtitle style field by field.
fn merge_prefs(saved: &mut Value, change: &Map<String, Value>) {
    for (key, value) in change {
        match (saved.get_mut(key), value) {
            (Some(Value::Object(fields)), Value::Object(changed)) => {
                for (field, value) in changed {
                    if fields.contains_key(field) {
                        fields.insert(field.clone(), value.clone());
                    }
                }
            }
            (Some(slot), _) => *slot = value.clone(),
            (None, _) => {}
        }
    }
}

/// Settings → Video player, as the player's "More settings" opens it.
const SETTINGS_ROUTE: &str = "/settings/video";

/// The player's own requests (M45), for sessions and single videos alike.
async fn player_request(app: &AppHandle, action: &str, raw: &Value) {
    let db = app.state::<AppDb>().0.clone();
    match action {
        "state" => {
            let group = raw.get("group").and_then(Value::as_str).unwrap_or_default();
            let Some(change) = raw.get("change").and_then(Value::as_object) else {
                return;
            };
            if group.is_empty() {
                return;
            }
            let key = format!("{STATE_PREFIX}{group}");
            let mut saved = match title_state(&db, group).await {
                Value::Object(saved) => saved,
                _ => Map::new(),
            };
            merge_state(&mut saved, change);
            let text = Value::Object(saved).to_string();
            if let Err(error) = raygun_db::set_local_value(&db, &key, &text).await {
                tracing::warn!(%error, "could not remember the player's state");
            }
        }
        "prefs" => {
            let Some(change) = raw.get("change").and_then(Value::as_object) else {
                return;
            };
            let saved = async {
                let settings = crate::settings::load(&db)
                    .await
                    .map_err(|error| error.to_string())?;
                let mut value = serde_json::to_value(&settings.video_player)
                    .map_err(|error| error.to_string())?;
                merge_prefs(&mut value, change);
                let prefs: VideoPlayerPrefs =
                    serde_json::from_value(value).map_err(|error| error.to_string())?;
                crate::settings::update(&db, SettingChange::VideoPlayer(Box::new(prefs)))
                    .await
                    .map_err(|error| error.to_string())
            };
            match saved.await {
                Ok(_) => {
                    let _ = SettingsChangedEvent("videoPlayer".into()).emit(app);
                }
                Err(error) => tracing::warn!(%error, "could not save a player setting"),
            }
        }
        "seekStep" => {
            let Some(seconds) = raw
                .get("seconds")
                .and_then(Value::as_u64)
                .and_then(|seconds| u32::try_from(seconds).ok())
            else {
                return;
            };
            match crate::settings::update(&db, SettingChange::SeekSeconds(seconds)).await {
                Ok(_) => {
                    let _ = SettingsChangedEvent("seekSeconds".into()).emit(app);
                }
                Err(error) => tracing::warn!(%error, "could not save the seek step"),
            }
        }
        // The player's "More settings": the app shows Settings → Video player while the paused
        // video waits in its small window.
        "openSettings" => {
            if let Err(error) = OpenRouteEvent(SETTINGS_ROUTE.into()).emit(app) {
                tracing::warn!(%error, "could not open the player's settings");
            }
        }
        "brightness" => {
            let Some(level) = raw
                .get("value")
                .and_then(Value::as_f64)
                .filter(|level| (0.0..=1.0).contains(level))
            else {
                return;
            };
            if let Err(error) =
                raygun_db::set_local_value(&db, BRIGHTNESS_KEY, &level.to_string()).await
            {
                tracing::warn!(%error, "could not remember the brightness");
            }
        }
        "duration" => {
            let id = raw
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let duration = raw.get("durationMs").and_then(Value::as_f64).unwrap_or(0.0);
            let Some(anime) = raw
                .get("anime")
                .cloned()
                .and_then(|anime| serde_json::from_value::<AnimeEpisode>(anime).ok())
            else {
                return;
            };
            // Off the report queue: the lookup goes over the network.
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                match crate::aniskip::chapters(&db, &anime, duration).await {
                    Ok(chapters) if !chapters.is_empty() => {
                        let _ = update_player(
                            &app,
                            &json!({ "type": "chapters", "id": id, "items": chapters }),
                        );
                    }
                    Ok(_) => {}
                    Err(error) => tracing::warn!(%error, "AniSkip lookup failed"),
                }
            });
        }
        _ => {}
    }
}

/// The language subtitles are fetched in first: the subtitle language, else the app's.
pub async fn subtitle_language(db: &raygun_db::Db) -> String {
    let settings = crate::settings::load(db).await.unwrap_or_default();
    if !settings.subtitle_language.is_empty() {
        settings.subtitle_language
    } else if settings.language != raygun_model::SYSTEM_LANGUAGE {
        settings.language
    } else {
        String::new()
    }
}

/// Marks something watched or not, in the progress store and on its library episode.
pub async fn mark(db: &raygun_db::Db, entry: &VideoEntry, watched: bool) -> Result<(), String> {
    let now = now_ms();
    store::mark(db, entry, watched, now)
        .await
        .map_err(|error| error.to_string())?;
    crate::video_library::mirror_mark(db, &entry.key, watched, now).await
}

/// Subtitle files through the stream proxy, which serves an empty track for one that can't be
/// fetched (the player would give up on the whole video) and lets release builds load plain-HTTP
/// ones. The proxied URL has no extension, so the format is guessed from the original first.
pub(crate) fn proxied_subtitles(
    app: &AppHandle,
    subtitles: &[VideoSubtitle],
) -> Vec<VideoSubtitle> {
    let Some(links) = app.try_state::<ImageLinks>() else {
        return subtitles.to_vec();
    };
    subtitles
        .iter()
        .filter(|subtitle| {
            subtitle.url.starts_with("https://") || subtitle.url.starts_with("http://")
        })
        .map(|subtitle| VideoSubtitle {
            url: links.subtitle(&subtitle.url, &Default::default()),
            format: subtitle
                .format
                .clone()
                .or_else(|| subtitle_format(&subtitle.url)),
            ..subtitle.clone()
        })
        .collect()
}

/// `srt`, `vtt`, `ass`... from a subtitle URL's file extension, when it has a known one.
fn subtitle_format(url: &str) -> Option<String> {
    let path = url.split(['?', '#']).next().unwrap_or_default();
    let name = path.rsplit('/').next().unwrap_or_default();
    let (_, extension) = name.rsplit_once('.')?;
    let extension = extension.to_ascii_lowercase();
    ["srt", "vtt", "webvtt", "ass", "ssa", "ttml", "dfxp"]
        .contains(&extension.as_str())
        .then_some(extension)
}

/// What the player opens with. While a TV is connected (Play on TV) the video plays there instead,
/// and the player shows the TV's status (`cast`).
fn with_cast(app: &AppHandle, payload: &Value) -> Value {
    let mut payload = payload.clone();
    if let Some(cast) = app.try_state::<std::sync::Arc<crate::cast::CastService>>()
        && let Some(status) = cast.follow(&payload)
    {
        payload["cast"] = json!(status);
    }
    payload
}

/// A video loaded into the player goes to the TV while one is connected (Play on TV).
fn follow_load(app: &AppHandle, update: &Value) {
    if update.get("type").and_then(Value::as_str) != Some("load") {
        return;
    }
    let Some(request) = update.get("request") else {
        return;
    };
    note_loaded(app, request);
    if let Some(cast) = app.try_state::<std::sync::Arc<crate::cast::CastService>>() {
        cast.follow(request);
    }
}

/// What a player loads: AI subtitles fetch the same stream.
fn note_loaded(app: &AppHandle, payload: &Value) {
    if let Some(subtitles) =
        app.try_state::<std::sync::Arc<crate::video_subtitles::VideoSubtitles>>()
    {
        subtitles.loaded(payload);
    }
}

#[cfg(target_os = "android")]
pub fn open_player(app: &AppHandle, payload: &serde_json::Value) -> Result<(), String> {
    note_loaded(app, payload);
    let payload = &with_cast(app, payload);
    let started = app.clone();
    tauri::async_runtime::spawn(
        async move { crate::playback::other_media_started(&started).await },
    );
    let player = app
        .try_state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
        .ok_or_else(|| "The video player isn't available".to_owned())?;
    player.video_open(payload)
}

#[cfg(not(target_os = "android"))]
pub fn open_player(app: &AppHandle, payload: &serde_json::Value) -> Result<(), String> {
    note_loaded(app, payload);
    let payload = &with_cast(app, payload);
    let started = app.clone();
    tauri::async_runtime::spawn(
        async move { crate::playback::other_media_started(&started).await },
    );
    crate::video_desktop::open(app, payload)
}

/// Sends an update to the open player (M44); nothing happens when none is open.
#[cfg(target_os = "android")]
pub fn update_player(app: &AppHandle, update: &serde_json::Value) -> Result<(), String> {
    follow_load(app, update);
    let player = app
        .try_state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
        .ok_or_else(|| "The video player isn't available".to_owned())?;
    player.video_update(update)
}

#[cfg(not(target_os = "android"))]
pub fn update_player(app: &AppHandle, update: &serde_json::Value) -> Result<(), String> {
    follow_load(app, update);
    crate::video_desktop::update(app, update)
}

/// Where the player's reports go: from Android's player over JNI, and from the webview's player
/// on desktop (`video_player_report`, M46.4).
pub struct VideoReports(tokio::sync::mpsc::UnboundedSender<String>);

impl VideoReports {
    pub fn send(&self, json: String) {
        let _ = self.0.send(json);
    }
}

/// The player's reports become saved progress and `WatchChangedEvent`s.
pub fn install(app: &AppHandle) {
    // Reports are handled one at a time, in order: two at once (the end of a video sends several)
    // would race on the same rows.
    let (sender, mut reports) = tokio::sync::mpsc::unbounded_channel::<String>();
    app.manage(VideoReports(sender.clone()));
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(json) = reports.recv().await {
            handle_report(&app, &json).await;
        }
    });
    tauri_plugin_raygun_audio::set_video_handler(move |json| {
        let _ = sender.send(json);
    });
}

async fn handle_report(app: &AppHandle, json: &str) {
    let Ok(raw) = serde_json::from_str::<serde_json::Value>(json) else {
        tracing::warn!(%json, "unreadable video player report");
        return;
    };
    // The player closed: a torrent it played closes (unless a TV plays it through this device).
    if raw.get("state").and_then(Value::as_str) == Some("closed")
        && !app
            .try_state::<std::sync::Arc<crate::cast::CastService>>()
            .is_some_and(|cast| cast.connected())
        && let Some(torrents) = app.try_state::<std::sync::Arc<crate::torrents::TorrentService>>()
    {
        torrents.player_closed().await;
    }
    // Where the viewer is: AI subtitles and translations keep ahead of there.
    let subtitles = app
        .try_state::<std::sync::Arc<crate::video_subtitles::VideoSubtitles>>()
        .map(|subtitles| subtitles.inner().clone());
    if let Some(subtitles) = &subtitles {
        subtitles.report(&raw);
    }
    // The player's own requests are handled here; the rest (next episode, another source...)
    // are the session's business.
    if raw.get("state").and_then(serde_json::Value::as_str) == Some("request") {
        let action = raw
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match action {
            "state" | "prefs" | "seekStep" | "openSettings" | "brightness" | "duration" => {
                player_request(app, action, &raw).await;
            }
            // AI subtitles in Android's player: transcribing and translating as it plays.
            "subtitleTools"
            | "transcribe"
            | "transcribeStop"
            | "transcribeSetup"
            | "transcribeSetupCancel"
            | "translateSubtitles"
            | "translateStop"
            | "openAiSettings" => {
                if let Some(subtitles) = &subtitles {
                    subtitles.on_request(action, &raw).await;
                }
            }
            _ if action.starts_with("cast") => crate::cast::on_request(app, action, &raw).await,
            _ => app.state::<VideoSessions>().on_report(&raw).await,
        }
        return;
    }
    // While a TV plays (Play on TV) its position is the one saved: the player's own reports are
    // about a paused copy. Its closing still ends a session.
    let from_cast = raw.get("cast").and_then(Value::as_bool) == Some(true);
    if !from_cast
        && app
            .try_state::<std::sync::Arc<crate::cast::CastService>>()
            .is_some_and(|cast| cast.connected())
    {
        if raw.get("state").and_then(Value::as_str) == Some("closed") {
            app.state::<VideoSessions>().on_report(&raw).await;
        }
        return;
    }
    let Ok(report) = serde_json::from_value::<PlayerReport>(raw.clone()) else {
        tracing::warn!(%raw, "unreadable video player report");
        return;
    };
    record(app, report).await;
    app.state::<VideoSessions>().on_report(&raw).await;
}

/// Whether `report` finishes the video: its end, or its watched point unless it loops (a looping
/// video comes round to the watched point again and again).
fn finishes(report: &PlayerReport, duration: Option<f64>, watched_share: f64) -> bool {
    report.state == "ended"
        || (!report.looping
            && duration.is_some_and(|duration| report.position_ms >= duration * watched_share))
}

/// Whether a report's place is kept. A failed load reports 0, which mustn't wipe saved progress;
/// a live stream keeps no place to resume and is never finished, so Continue watching doesn't
/// take it for a film (its row from `started` has no position and stays out).
fn keeps_place(report: &PlayerReport) -> bool {
    !report.live && !(report.state == "error" && report.position_ms <= 0.0)
}

async fn record(app: &AppHandle, report: PlayerReport) {
    if let Some(error) = &report.error {
        tracing::warn!(key = %report.id, %error, "video playback failed");
    }
    if !keeps_place(&report) {
        return;
    }
    let duration = report.duration_ms.filter(|duration| *duration > 0.0);
    let db = app.state::<AppDb>().0.clone();
    let watched_share = player_prefs(&db).await.watched_share;
    let watched = finishes(&report, duration, watched_share);
    // Incognito (M47): no progress is saved, and a series doesn't move on to its next episode.
    if crate::privacy::incognito(app) {
        return;
    }
    let now = now_ms();
    let position_ms = report.position_ms as i64;
    let duration_ms = duration.map(|duration| duration as i64);
    // The end comes in more than one report (ended, then switched or closed): only the first
    // finishes it.
    let newly_watched = watched
        && store::progress(&db, &report.id)
            .await
            .ok()
            .flatten()
            .is_some_and(|row| row.watched_at.is_none() || row.position_ms > 0);
    if let Err(error) =
        store::set_progress(&db, &report.id, position_ms, duration_ms, watched, now).await
    {
        tracing::warn!(%error, "could not save watching progress");
    }
    if let Err(error) = crate::video_library::mirror_progress(
        &db,
        &report.id,
        position_ms,
        duration_ms,
        watched,
        now,
    )
    .await
    {
        tracing::warn!(%error, "could not save the episode's progress");
    }
    if watched {
        app.state::<VideoSessions>()
            .on_watched(&report.id, newly_watched)
            .await;
    }
    if matches!(report.state.as_str(), "closed" | "ended" | "switched") || watched {
        let _ = WatchChangedEvent(report.id).emit(app);
    }
}

/// Most entries in Continue watching.
const CONTINUE_LIMIT: usize = 30;
/// How much of the history Continue watching is picked from.
const CONTINUE_CANDIDATES: u32 = 400;

/// What to continue: one entry per title, the episode watched last while it's unfinished (with
/// its place). A series whose last episode watched is finished shows the next one only when it
/// was queued as up next, and one removed from the list stays out until something of it plays
/// again: an older, half-watched episode never stands in for either.
pub async fn continue_watching(app: &AppHandle) -> Result<Vec<ContinueWatching>, String> {
    let db = app.state::<AppDb>().0.clone();
    let rows = store::continue_candidates(&db, CONTINUE_CANDIDATES)
        .await
        .map_err(|error| error.to_string())?;
    Ok(to_view(app, latest_per_title(rows, CONTINUE_LIMIT)).await)
}

/// Of rows newest first (with whether each was removed), each title's first, kept when it's
/// there and unfinished.
fn latest_per_title(
    rows: Vec<(store::VideoProgress, bool)>,
    limit: usize,
) -> Vec<store::VideoProgress> {
    let mut seen = std::collections::HashSet::new();
    rows.into_iter()
        .filter(|(row, _)| seen.insert(title_of(&row.entry)))
        .filter(|(row, removed)| !removed && row.watched_at.is_none() && row.position_ms > 0)
        .map(|(row, _)| row)
        .take(limit)
        .collect()
}

/// The title an entry belongs to: a session's episode its origin, a request its group (a video
/// source's title or a Stremio series), anything else itself.
fn title_of(entry: &VideoEntry) -> String {
    if let Ok(episode) = serde_json::from_str::<EpisodeRef>(&entry.request) {
        return episode.origin.group_key();
    }
    serde_json::from_str::<VideoRequest>(&entry.request)
        .ok()
        .and_then(|request| request.group)
        .unwrap_or_else(|| entry.key.clone())
}

/// Everything watched or started, most recent first.
pub async fn history(app: &AppHandle) -> Result<Vec<ContinueWatching>, String> {
    let db = app.state::<AppDb>().0.clone();
    let rows = store::history(&db, 200)
        .await
        .map_err(|error| error.to_string())?;
    Ok(to_view(app, rows).await)
}

/// The title an entry belongs to, and the movie's or episode's id there: a session's episode
/// reference says both, a request played without a session names its title (`group`) and its key
/// the episode. A link played on its own belongs to none.
fn entry_episode(entry: &VideoEntry) -> Option<(Origin, Option<String>)> {
    let origin = Origin::from_group(&title_of(entry))?;
    let episode = serde_json::from_str::<EpisodeRef>(&entry.request)
        .ok()
        .map(|reference| reference.episode)
        .or_else(|| crate::video_library::key_unit(&entry.key).map(|(_, id)| id.to_owned()));
    Some((origin, episode))
}

/// Whether a Stremio title's entry is one of its episodes: a movie (or a channel's own video)
/// plays by the title's id.
fn stremio_episode(kind: &str, id: &str, episode: &str) -> bool {
    kind != "movie" && episode != id
}

/// What an entry's card and menu need of its title: its page (`source_id`, `remote_id`), its
/// player group, the episode there, and whether it's a series. A video source's episodes are read
/// once per title (`units`).
async fn entry_view(
    db: &raygun_db::Db,
    entry: &VideoEntry,
    units: &mut std::collections::HashMap<String, Vec<raygun_db::titles::UnitRow>>,
) -> EntryView {
    // A link played on its own has a page of its own (`/watch/link`).
    if crate::video_links::is_link_key(&entry.key) && entry_episode(entry).is_none() {
        return EntryView {
            link: true,
            ..EntryView::default()
        };
    }
    let Some((origin, episode_id)) = entry_episode(entry) else {
        return EntryView::default();
    };
    let group = Some(origin.group_key());
    match origin {
        Origin::Source { title_id } => {
            let Ok(Some(row)) = raygun_db::titles::title(db, &title_id).await else {
                // Its title is no longer stored: no page, nothing to pick a source from.
                return EntryView::default();
            };
            if !units.contains_key(&title_id) {
                let listed = raygun_db::titles::units(db, &title_id)
                    .await
                    .unwrap_or_default();
                units.insert(title_id.clone(), listed);
            }
            let listed = units.get(&title_id).map(Vec::as_slice).unwrap_or_default();
            EntryView {
                page: Some((row.fields.source_id, row.fields.remote_id)),
                group,
                unit_id: episode_id.as_deref().and_then(|episode| {
                    listed
                        .iter()
                        .find(|unit| unit.fields.remote_id == episode)
                        .map(|unit| unit.id.clone())
                }),
                series: listed.len() > 1,
                episode_id,
                link: false,
            }
        }
        Origin::Stremio { kind, id } => EntryView {
            series: episode_id
                .as_deref()
                .is_some_and(|episode| stremio_episode(&kind, &id, episode)),
            page: Some((
                crate::video_library::STREMIO_SOURCE.to_owned(),
                format!("{kind}:{id}"),
            )),
            group,
            unit_id: None,
            episode_id,
            link: false,
        },
        // Never read from a group (`Origin::from_group`).
        Origin::Link { .. } => EntryView::default(),
    }
}

#[derive(Default)]
struct EntryView {
    page: Option<(String, String)>,
    group: Option<String>,
    episode_id: Option<String>,
    unit_id: Option<String>,
    series: bool,
    /// A link played on its own: its page is its own.
    link: bool,
}

async fn to_view(app: &AppHandle, rows: Vec<store::VideoProgress>) -> Vec<ContinueWatching> {
    let db = app.state::<AppDb>().0.clone();
    let mut units = std::collections::HashMap::new();
    let mut views = Vec::with_capacity(rows.len());
    for row in &rows {
        views.push(entry_view(&db, &row.entry, &mut units).await);
    }
    let images = app.state::<raygun_net::ImageLinks>();
    rows.into_iter()
        .zip(views)
        .map(|(row, view)| ContinueWatching {
            source_id: view.page.as_ref().map(|(source, _)| source.clone()),
            remote_id: view.page.map(|(_, remote)| remote),
            group: view.group,
            episode_id: view.episode_id,
            unit_id: view.unit_id,
            series: view.series,
            link: view.link,
            key: row.entry.key,
            title: row.entry.title,
            // A movie's line is its release years, saved as an addon wrote them ("2024–").
            subtitle: row
                .entry
                .subtitle
                .map(|subtitle| crate::stremio::release_label(&subtitle)),
            // Posters seen again and again: through the lasting cache (M46.2); a link's kept
            // poster through the proxy's `/local/` route.
            poster: row
                .entry
                .poster
                .map(|poster| crate::video_links::shown_poster(&images, &poster)),
            position_ms: row.position_ms as f64,
            duration_ms: row.duration_ms.map(|ms| ms as f64),
            updated_at: row.updated_at as f64,
            watched: row.watched_at.is_some(),
        })
        .collect()
}

/// Plays a "continue watching" entry again: an episode reference through a session (fresh links,
/// the last source first), else the request it was first played with.
pub async fn resume(app: &AppHandle, key: &str) -> Result<(), String> {
    resume_from(app, key, None).await
}

/// Plays a "continue watching" entry from its start; its saved place stays until the player
/// reports a new one.
pub async fn play_from_start(app: &AppHandle, key: &str) -> Result<(), String> {
    resume_from(app, key, Some(0)).await
}

async fn resume_from(app: &AppHandle, key: &str, start_at: Option<i64>) -> Result<(), String> {
    // A link played on its own plays as its page plays it (as renamed, with its poster).
    if crate::video_links::is_link_key(key) {
        return crate::video_links::play(app, key, None, start_at == Some(0)).await;
    }
    let saved = saved_entry(app, key).await?;
    if let Ok(episode) = serde_json::from_str::<EpisodeRef>(&saved.entry.request) {
        return app
            .state::<VideoSessions>()
            .start_at(episode.origin, Some(episode.episode), start_at)
            .await;
    }
    let request: VideoRequest = serde_json::from_str(&saved.entry.request)
        .map_err(|_| "Open this from its page to choose a stream".to_owned())?;
    play_from(app, request, start_at).await
}

async fn saved_entry(app: &AppHandle, key: &str) -> Result<store::VideoProgress, String> {
    let db = app.state::<AppDb>().0.clone();
    store::progress(&db, key)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "This was removed".to_owned())
}

/// Plays the episode after a "continue watching" entry's: its series' next one that is out.
/// False when there's none (the last episode, or not a series).
pub async fn play_next(app: &AppHandle, key: &str) -> Result<bool, String> {
    let saved = saved_entry(app, key).await?;
    let Some((origin, _)) = entry_episode(&saved.entry) else {
        return Ok(false);
    };
    app.state::<VideoSessions>().start_next(origin, key).await
}

/// Marks a "continue watching" entry watched without moving on to the next episode, as marking it
/// on its title's page does: it leaves Continue watching (its title too, until something of it
/// plays again) and its stored episode counts as watched. Its place in the history stays.
pub async fn mark_watched(app: &AppHandle, key: &str) -> Result<(), String> {
    let db = app.state::<AppDb>().0.clone();
    let now = now_ms();
    store::set_watched(&db, key, Some(now))
        .await
        .map_err(|error| error.to_string())?;
    crate::video_library::mirror_mark(&db, key, true, now).await?;
    let _ = WatchChangedEvent(key.to_owned()).emit(app);
    Ok(())
}

pub async fn remove(app: &AppHandle, key: &str) -> Result<(), String> {
    let db = app.state::<AppDb>().0.clone();
    store::remove(&db, key)
        .await
        .map_err(|error| error.to_string())?;
    // A link's page goes with it.
    crate::video_links::forget(app, key).await;
    Ok(())
}

/// Takes entries out of Continue watching and the history at once; returns how many there were.
pub async fn remove_many(app: &AppHandle, keys: &[String]) -> Result<u32, String> {
    let db = app.state::<AppDb>().0.clone();
    let removed = store::remove_many(&db, keys)
        .await
        .map(|count| u32::try_from(count).unwrap_or(u32::MAX))
        .map_err(|error| error.to_string())?;
    for key in keys {
        crate::video_links::forget(app, key).await;
    }
    Ok(removed)
}

/// A video link as a request: its key is the address, its title the file name (a magnet link's
/// name).
pub fn link_request(url: &str, title: Option<&str>, subtitle_url: Option<&str>) -> VideoRequest {
    let url = url.trim().to_owned();
    let torrent = raygun_torrent::is_torrent(&url, None);
    let title = title
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            raygun_torrent::parse(&url)
                .ok()
                .filter(|_| torrent)
                .and_then(|link| link.name().map(str::to_owned))
        })
        .unwrap_or_else(|| {
            url.split(['?', '#'])
                .next()
                .and_then(|path| path.trim_end_matches('/').rsplit('/').next())
                .filter(|name| !name.is_empty() && !name.starts_with("magnet:"))
                .unwrap_or("Video")
                .to_owned()
        });
    let lower = url.to_lowercase();
    let format = if torrent {
        Some(raygun_torrent::FORMAT.to_owned())
    } else if lower.contains(".m3u8") {
        Some("hls".to_owned())
    } else if lower.contains(".mpd") {
        Some("dash".to_owned())
    } else {
        None
    };
    VideoRequest {
        group: None,
        chapters: Vec::new(),
        anime: None,
        key: format!("url:{url}"),
        title,
        subtitle: None,
        poster: None,
        format,
        headers: Default::default(),
        subtitles: subtitle_url
            .map(str::trim)
            .filter(|subtitle| !subtitle.is_empty())
            .map(|subtitle| raygun_model::VideoSubtitle {
                url: subtitle.to_owned(),
                lang: None,
                label: None,
                format: None,
            })
            .into_iter()
            .collect(),
        url,
        live: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looping_reports_never_finish_at_the_watched_point() {
        let report = |state: &str, looping: bool| {
            serde_json::from_value::<PlayerReport>(json!({
                "id": "v", "state": state, "positionMs": 95.0, "durationMs": 100.0,
                "error": null, "looping": looping,
            }))
            .unwrap()
        };
        assert!(finishes(&report("progress", false), Some(100.0), 0.9));
        assert!(!finishes(&report("progress", true), Some(100.0), 0.9));
        assert!(!finishes(&report("closed", true), Some(100.0), 0.9));
        // Older players leave the field out.
        let old: PlayerReport = serde_json::from_value(
            json!({ "id": "v", "state": "progress", "positionMs": 95.0, "durationMs": 100.0 }),
        )
        .unwrap();
        assert!(finishes(&old, Some(100.0), 0.9));
    }

    #[test]
    fn live_streams_keep_no_place_to_resume() {
        let report = |json: Value| serde_json::from_value::<PlayerReport>(json).unwrap();
        assert!(keeps_place(&report(json!({
            "id": "v", "state": "progress", "positionMs": 5_000.0, "durationMs": 100_000.0
        }))));
        // At its live edge a stream sits near the end of its window: it isn't watched, and its
        // clock isn't a place to come back to.
        assert!(!keeps_place(&report(json!({
            "id": "v", "state": "progress", "positionMs": 3_600_000.0, "durationMs": null,
            "live": true
        }))));
        assert!(!keeps_place(&report(json!({
            "id": "v", "state": "error", "positionMs": 0.0, "durationMs": null
        }))));
    }

    #[test]
    fn settings_reach_an_open_player_as_a_prefs_update() {
        let mut video_player = VideoPlayerPrefs::default();
        video_player.subtitle_style.text_color = 0xFF12_3456;
        let prefs = PlayerPrefs {
            resume: true,
            watched_share: 0.9,
            seek_ms: 15_000,
            app_language: "de".into(),
            subtitle_language: "fr".into(),
            subtitle_mode: SubtitleMode::Always,
            autoplay: true,
            next_prompt_ms: 10_000,
            keep_music_playing: false,
            accent: "#65adff".into(),
            video_player: video_player.clone(),
        };
        let update = prefs_update(serde_json::to_value(&video_player).unwrap(), &prefs);
        assert_eq!(update["type"], "prefs");
        assert_eq!(
            update["prefs"]["subtitleStyle"]["textColor"],
            0xFF12_3456_u32
        );
        assert_eq!(update["seekStepMs"], 15_000);
        assert_eq!(update["subtitleLanguage"], "fr");
        assert_eq!(update["subtitleMode"], "always");
        assert_eq!(update["appLanguage"], "de");
        // Not a new video: nothing of a request's own.
        assert!(update.get("url").is_none() && update.get("session").is_none());
    }

    #[test]
    fn link_requests() {
        let hls = link_request(" https://cdn.example/show/master.m3u8?t=1 ", None, None);
        assert_eq!(hls.key, "url:https://cdn.example/show/master.m3u8?t=1");
        assert_eq!(hls.title, "master.m3u8");
        assert_eq!(hls.format.as_deref(), Some("hls"));
        assert!(hls.subtitles.is_empty());

        let mp4 = link_request(
            "https://cdn.example/film.mp4",
            Some("A film"),
            Some("https://cdn.example/film.srt"),
        );
        assert_eq!(mp4.title, "A film");
        assert_eq!(mp4.format, None);
        assert_eq!(mp4.subtitles[0].url, "https://cdn.example/film.srt");

        // A magnet link plays as a torrent, under its name.
        let magnet = link_request(
            "magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d566095a10&dn=Sintel",
            None,
            None,
        );
        assert_eq!(magnet.title, "Sintel");
        assert_eq!(magnet.format.as_deref(), Some("torrent"));
        let unnamed = link_request(
            "magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d566095a10",
            None,
            None,
        );
        assert_eq!(unnamed.title, "Video");
        let file = link_request(
            "https://archive.org/download/a/a_archive.torrent",
            None,
            None,
        );
        assert_eq!(file.format.as_deref(), Some("torrent"));
    }

    fn episode(
        key: &str,
        position_ms: i64,
        watched: bool,
        updated_at: i64,
    ) -> store::VideoProgress {
        store::VideoProgress {
            entry: VideoEntry {
                key: key.into(),
                title: "Show".into(),
                subtitle: Some(key.into()),
                poster: None,
                request: r#"{"origin":"stremio","kind":"series","id":"tt1","episode":"x"}"#.into(),
            },
            position_ms,
            duration_ms: Some(1_500_000),
            watched_at: watched.then_some(updated_at),
            updated_at,
        }
    }

    fn keys(rows: Vec<store::VideoProgress>) -> Vec<String> {
        rows.into_iter().map(|row| row.entry.key).collect()
    }

    #[test]
    fn entries_know_their_title_and_episode() {
        let entry = |key: &str, request: String| VideoEntry {
            key: key.into(),
            title: "Show".into(),
            subtitle: None,
            poster: None,
            request,
        };
        // An episode played through a session.
        let session = entry(
            "stremio:series:tt1:1:2",
            r#"{"origin":"stremio","kind":"series","id":"tt1","episode":"tt1:1:2"}"#.into(),
        );
        assert_eq!(
            entry_episode(&session),
            Some((
                Origin::Stremio {
                    kind: "series".into(),
                    id: "tt1".into()
                },
                Some("tt1:1:2".into())
            ))
        );
        assert!(stremio_episode("series", "tt1", "tt1:1:2"));
        // A source's episode played from a link picked by hand: its request names the title and
        // its key the episode.
        let mut picked = link_request("https://cdn.example/serial-2.mp4", Some("Serial"), None);
        picked.group = Some("title:t1".into());
        picked.key = "source:films:serial/2".into();
        let picked = entry(&picked.key.clone(), serde_json::to_string(&picked).unwrap());
        assert_eq!(
            entry_episode(&picked),
            Some((
                Origin::Source {
                    title_id: "t1".into()
                },
                Some("serial/2".into())
            ))
        );
        // A movie plays by its own id (IMDb's when TMDB's found it): no next episode.
        assert!(!stremio_episode("movie", "tmdb:3", "tt3"));
        assert!(!stremio_episode("channel", "yt:c1", "yt:c1"));
        // A link played on its own belongs to no title.
        let link = link_request("https://cdn.example/film.mp4", None, None);
        assert_eq!(
            entry_episode(&entry(
                &link.key.clone(),
                serde_json::to_string(&link).unwrap()
            )),
            None
        );
    }

    #[tokio::test]
    async fn links_open_their_own_page_and_titles_keep_theirs() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = raygun_db::Db::open(&temp.path().join("raygun.db"))
            .await
            .unwrap();
        let entry = |request: &VideoRequest| VideoEntry {
            key: request.key.clone(),
            title: request.title.clone(),
            subtitle: None,
            poster: None,
            request: serde_json::to_string(request).unwrap(),
        };
        let mut units = std::collections::HashMap::new();
        // Sintel from a magnet link (Open link): a page of its own, no title's.
        let magnet = link_request(
            "magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d566095a10&dn=Sintel",
            None,
            None,
        );
        let view = entry_view(&db, &entry(&magnet), &mut units).await;
        assert!(view.link);
        assert_eq!((view.page, view.group, view.episode_id), (None, None, None));
        // Its entry played from a torrent's file keeps being the link's.
        let mut file = magnet.clone();
        file.url.push_str("&so=2");
        assert!(entry_view(&db, &entry(&file), &mut units).await.link);

        // A Stremio episode keeps its title's page, group and episode.
        let episode = VideoEntry {
            key: "stremio:series:tt1:1:2".into(),
            title: "Show".into(),
            subtitle: None,
            poster: None,
            request: r#"{"origin":"stremio","kind":"series","id":"tt1","episode":"tt1:1:2"}"#
                .into(),
        };
        let view = entry_view(&db, &episode, &mut units).await;
        assert!(!view.link);
        assert_eq!(
            view.page,
            Some(("stremio".to_owned(), "series:tt1".to_owned()))
        );
        assert_eq!(view.group.as_deref(), Some("stremio:series:tt1"));
        assert_eq!(view.episode_id.as_deref(), Some("tt1:1:2"));
        assert!(view.series);
        // A link picked by hand for a Stremio movie belongs to the movie, not to itself.
        let mut picked = link_request("https://cdn.example/film.mp4", None, None);
        picked.group = Some("stremio:movie:tt9".into());
        picked.key = "stremio:movie:tt9".into();
        let view = entry_view(&db, &entry(&picked), &mut units).await;
        assert!(!view.link);
        assert_eq!(
            view.page,
            Some(("stremio".to_owned(), "movie:tt9".to_owned()))
        );
    }

    #[test]
    fn continue_watching_shows_the_episode_watched_last() {
        // Episode 1 opened for a moment, episode 7 watched well into it later.
        let rows = vec![
            (episode("stremio:series:tt1:1:7", 900_000, false, 20), false),
            (episode("stremio:series:tt1:1:1", 4_000, false, 10), false),
        ];
        assert_eq!(keys(latest_per_title(rows, 30)), ["stremio:series:tt1:1:7"]);

        // Episode 7 finished and episode 8 queued as up next.
        let rows = vec![
            (episode("stremio:series:tt1:1:8", 1, false, 30), false),
            (episode("stremio:series:tt1:1:7", 0, true, 30), false),
            (episode("stremio:series:tt1:1:1", 4_000, false, 10), false),
        ];
        assert_eq!(keys(latest_per_title(rows, 30)), ["stremio:series:tt1:1:8"]);

        // The last episode finished, or the latest removed: not back to episode 1.
        let finished = vec![
            (episode("stremio:series:tt1:1:7", 0, true, 30), false),
            (episode("stremio:series:tt1:1:1", 4_000, false, 10), false),
        ];
        assert!(latest_per_title(finished, 30).is_empty());
        let removed = vec![
            (episode("stremio:series:tt1:1:7", 900_000, false, 30), true),
            (episode("stremio:series:tt1:1:1", 4_000, false, 10), false),
        ];
        assert!(latest_per_title(removed, 30).is_empty());
    }

    #[test]
    fn entries_belong_to_their_title() {
        let mut link = episode("url:https://cdn.example/a.mp4", 1, false, 1).entry;
        link.request = "{}".into();
        assert_eq!(title_of(&link), "url:https://cdn.example/a.mp4");
        let mut picked = link.clone();
        picked.request = serde_json::to_string(&VideoRequest {
            group: Some("title:t1".into()),
            ..link_request("https://cdn.example/a.mp4", None, None)
        })
        .unwrap();
        assert_eq!(title_of(&picked), "title:t1");
        let session = episode("source:s:e1", 1, false, 1).entry;
        assert_eq!(title_of(&session), "stremio:series:tt1");
    }

    #[tokio::test]
    async fn watched_up_to_episode_seven_continues_at_seven() {
        // What the player reports: episode 1 for a moment, then the picker's episode 7, whose
        // "switched" report for episode 1 lands after episode 7 started.
        let temp = tempfile::TempDir::new().unwrap();
        let db = raygun_db::Db::open(&temp.path().join("raygun.db"))
            .await
            .unwrap();
        let entry = |number: u32| VideoEntry {
            key: format!("stremio:series:tt1:1:{number}"),
            title: "Show".into(),
            subtitle: Some(format!("S1 E{number}")),
            poster: None,
            request: format!(
                r#"{{"origin":"stremio","kind":"series","id":"tt1","episode":"tt1:1:{number}"}}"#
            ),
        };
        store::started(&db, &entry(1), 1_000).await.unwrap();
        store::set_progress(&db, &entry(1).key, 4_000, Some(1_400_000), false, 5_000)
            .await
            .unwrap();
        store::started(&db, &entry(7), 6_000).await.unwrap();
        store::set_progress(&db, &entry(1).key, 4_500, Some(1_400_000), false, 6_500)
            .await
            .unwrap();
        for at in [11_000, 16_000, 900_000] {
            store::set_progress(&db, &entry(7).key, at, Some(1_400_000), false, at)
                .await
                .unwrap();
        }
        let rows = store::continue_candidates(&db, 400).await.unwrap();
        let shown = latest_per_title(rows, 30);
        assert_eq!(keys(shown), ["stremio:series:tt1:1:7"]);
    }

    #[test]
    fn continue_watching_shows_each_title_once() {
        let row = |key: &str, request: String| store::VideoProgress {
            entry: VideoEntry {
                key: key.into(),
                title: "T".into(),
                subtitle: None,
                poster: None,
                request,
            },
            position_ms: 10,
            duration_ms: None,
            watched_at: None,
            updated_at: 0,
        };
        let session = |episode: &str| {
            serde_json::to_string(&EpisodeRef {
                origin: crate::video_session::Origin::Stremio {
                    kind: "series".into(),
                    id: "tt1".into(),
                },
                episode: episode.into(),
            })
            .unwrap()
        };
        let mut picked = link_request("https://cdn.example/e3.mp4", None, None);
        picked.group = Some("stremio:series:tt1".into());
        let rows = vec![
            row("stremio:series:tt1:1:4", session("tt1:1:4")),
            row("stremio:series:tt1:1:2", session("tt1:1:2")),
            row(
                "stremio:series:tt1:1:3",
                serde_json::to_string(&picked).unwrap(),
            ),
            row("url:https://cdn.example/a.mp4", "{}".into()),
            row("url:https://cdn.example/b.mp4", "{}".into()),
        ];
        let rows = rows.into_iter().map(|row| (row, false)).collect();
        let keys: Vec<String> = latest_per_title(rows, 30)
            .into_iter()
            .map(|row| row.entry.key)
            .collect();
        assert_eq!(
            keys,
            [
                "stremio:series:tt1:1:4",
                "url:https://cdn.example/a.mp4",
                "url:https://cdn.example/b.mp4"
            ],
            "the series at its latest episode; links are their own"
        );
    }

    #[test]
    fn subtitle_formats_from_urls() {
        let format = |url| subtitle_format(url);
        assert_eq!(
            format("https://cdn.example/a/film.SRT?t=1").as_deref(),
            Some("srt")
        );
        assert_eq!(
            format("https://cdn.example/subs.vtt#x").as_deref(),
            Some("vtt")
        );
        assert_eq!(format("https://subs.example/en/download/file/123"), None);
        assert_eq!(format("https://cdn.example.com/"), None);
    }

    fn object(value: Value) -> Map<String, Value> {
        match value {
            Value::Object(map) => map,
            _ => panic!("not an object"),
        }
    }

    #[test]
    fn a_titles_memory_takes_known_keys_and_forgets_nulls() {
        let mut saved =
            object(json!({ "speed": 125, "aspect": "crop", "audio": { "language": "ja" } }));
        merge_state(
            &mut saved,
            &object(json!({
                "speed": 150,
                "aspect": null,
                "subtitleDelayMs": -300,
                "volume": 1,
                "subtitleEdits": { "ep1|English|en": [{ "at": 1000, "startMs": 1200, "endMs": 3000, "text": "Hi" }] },
            })),
        );
        assert_eq!(
            Value::Object(saved),
            json!({
                "speed": 150,
                "audio": { "language": "ja" },
                "subtitleDelayMs": -300,
                "subtitleEdits": { "ep1|English|en": [{ "at": 1000, "startMs": 1200, "endMs": 3000, "text": "Hi" }] },
            })
        );
    }

    #[test]
    fn subtitles_follow_the_older_switch_until_chosen() {
        let mut settings = Settings::default();
        assert_eq!(subtitle_mode(&settings), SubtitleMode::Foreign);
        settings.auto_subtitles = true;
        assert_eq!(subtitle_mode(&settings), SubtitleMode::Always);
        settings.video_player.subtitle_mode = Some(SubtitleMode::Never);
        assert_eq!(subtitle_mode(&settings), SubtitleMode::Never);
    }

    #[test]
    fn every_request_says_the_languages_accent_and_user_agent() {
        let prefs = PlayerPrefs {
            resume: true,
            watched_share: 0.9,
            seek_ms: 10_000,
            app_language: "pt-BR".into(),
            subtitle_language: String::new(),
            subtitle_mode: SubtitleMode::Always,
            autoplay: true,
            next_prompt_ms: 10_000,
            keep_music_playing: true,
            accent: "#65adff".into(),
            video_player: VideoPlayerPrefs::default(),
        };
        let payload = with_shared_fields(json!({ "id": "url:a" }), &prefs);
        assert_eq!(payload["id"], "url:a");
        assert_eq!(payload["seekStepMs"], 10_000);
        assert_eq!(payload["appLanguage"], "pt-BR");
        assert_eq!(payload["subtitleLanguage"], "");
        assert_eq!(payload["subtitleMode"], "always");
        assert_eq!(payload["keepMusicPlaying"], true);
        assert_eq!(payload["accent"], "#65adff");
        assert!(
            payload["userAgent"]
                .as_str()
                .is_some_and(|agent| !agent.is_empty())
        );
    }

    #[test]
    fn the_player_draws_with_the_apps_accent() {
        let mut settings = Settings::default();
        assert_eq!(accent_color(&settings), "#ff6a65");
        settings.accent = Accent::Custom;
        settings.accent_color = "#123456".into();
        assert_eq!(accent_color(&settings), "#123456");
    }

    #[test]
    fn player_changes_merge_into_the_settings() {
        let mut saved = serde_json::to_value(VideoPlayerPrefs::default()).unwrap();
        merge_prefs(
            &mut saved,
            &object(json!({
                "defaultSpeed": 150,
                "showRemaining": true,
                "subtitleMode": "never",
                "subtitleStyle": { "size": 120, "unknown": 1 },
                "notASetting": true
            })),
        );
        let prefs: VideoPlayerPrefs = serde_json::from_value(saved).unwrap();
        assert_eq!(prefs.default_speed, 150);
        assert!(prefs.show_remaining);
        assert_eq!(prefs.subtitle_mode, Some(SubtitleMode::Never));
        assert_eq!(prefs.subtitle_style.size, 120);
        assert_eq!(
            prefs.subtitle_style.text_color,
            raygun_model::SubtitleStyle::default().text_color,
            "the rest of the style stays"
        );
        assert!(prefs.validate().is_ok());
    }
}
