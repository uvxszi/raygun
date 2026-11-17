//! Typed settings (`raygun_app_base::settings`), and what puts them into effect in the app's
//! running services.

pub use raygun_app_base::settings::*;
use raygun_model::Settings;

/// Sets `apply_live`'s work (at start).
pub fn install() {
    set_apply_live(apply_boxed);
}

fn apply_boxed<'a>(
    app: &'a tauri::AppHandle,
    settings: &'a Settings,
    changed: Option<&'a [&'a str]>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
    Box::pin(apply_here(app, settings, changed))
}

/// The settings last put into effect in running services, to tell what a sync or a restore
/// changed.
static APPLIED: std::sync::Mutex<Option<Settings>> = std::sync::Mutex::new(None);

/// Puts settings that running services hold into effect: the system bars' theme, the content
/// filter, cache limits, turned-off sources, the User-Agent, sound and the charts' country. With
/// `changed`, only the keys it names; without, every key whose value differs from what was last
/// put into effect (all of them the first time), as after a sync or a restored backup.
async fn apply_here(app: &tauri::AppHandle, settings: &Settings, changed: Option<&[&str]>) {
    use tauri::Manager;

    let before = APPLIED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .replace(settings.clone());
    let differs = |key: &str, same: &dyn Fn(&Settings) -> bool| match (changed, &before) {
        (Some(keys), _) => keys.contains(&key),
        (None, Some(before)) => !same(before),
        (None, None) => true,
    };
    if differs("theme", &|before| before.theme == settings.theme) {
        crate::system_ui::apply_theme(app, settings.theme);
    }
    if differs("userAgent", &|before| {
        before.user_agent == settings.user_agent
    }) {
        raygun_net::set_user_agent(&settings.user_agent);
    }
    if differs("sound", &|before| before.sound == settings.sound) {
        app.state::<crate::playback::PlaybackService>()
            .set_sound(settings.sound.clone());
    }
    let content = differs("showNsfw", &|before| before.show_nsfw == settings.show_nsfw)
        || differs("explicitMusic", &|before| {
            before.explicit_music == settings.explicit_music
        });
    if content {
        app.state::<std::sync::Arc<crate::extensions::ExtensionService>>()
            .set_content(settings.show_nsfw, settings.explicit_music)
            .await;
    }
    if differs("contentOverrides", &|before| {
        before.content_overrides == settings.content_overrides
    }) {
        app.state::<std::sync::Arc<crate::extensions::ExtensionService>>()
            .set_content_overrides(settings.content_overrides.clone())
            .await;
    }
    if differs("cache", &|before| before.cache == settings.cache) {
        app.state::<crate::caches::Caches>().apply(&settings.cache);
    }
    if differs("disabledSources", &|before| {
        before.disabled_sources == settings.disabled_sources
    }) {
        app.state::<std::sync::Arc<crate::extensions::ExtensionService>>()
            .set_disabled_sources(&settings.disabled_sources);
    }
    if differs("downloadsInBatterySaver", &|before| {
        before.downloads_in_battery_saver == settings.downloads_in_battery_saver
    }) {
        crate::device_state::set_downloads_allowed(settings.downloads_in_battery_saver);
    }
    if differs("discovery", &|before| {
        before.discovery == settings.discovery
    }) {
        app.state::<std::sync::Arc<crate::discovery::DiscoveryService>>()
            .nudge();
    }
    // A video playing follows its settings at once (the subtitles' style, languages, seek step);
    // at start there is none to tell.
    let player = differs("videoPlayer", &|before| {
        before.video_player == settings.video_player
    }) || differs("subtitleLanguage", &|before| {
        before.subtitle_language == settings.subtitle_language
    }) || differs("autoSubtitles", &|before| {
        before.auto_subtitles == settings.auto_subtitles
    }) || differs("seekSeconds", &|before| {
        before.seek_seconds == settings.seek_seconds
    }) || differs("sound", &|before| before.sound == settings.sound);
    if player && before.is_some() {
        crate::video::settings_changed(app).await;
    }
    // Reading aloud goes on with a new voice, speed, pitch or volume from where it is.
    let read_aloud = differs("readAloud", &|before| {
        before.read_aloud == settings.read_aloud
    }) || differs("novelReader", &|before| {
        before.novel_reader.tts_speed == settings.novel_reader.tts_speed
    });
    if read_aloud && before.is_some() {
        let app = app.clone();
        tauri::async_runtime::spawn(async move { crate::tts::settings_changed(&app).await });
    }
    if differs("network", &|before| before.network == settings.network) {
        crate::network_mode::apply(app, &settings.network);
    }
    if differs("torrents", &|before| before.torrents == settings.torrents)
        && let Some(torrents) = app.try_state::<std::sync::Arc<crate::torrents::TorrentService>>()
    {
        torrents.settings_changed(&settings.torrents).await;
    }
    // YouTube's players: the audio track and codec (the next load's address), SponsorBlock's
    // parts (the song playing follows at once).
    if differs("youtube", &|before| before.youtube == settings.youtube)
        || differs("videoPlayer", &|before| {
            before.video_player.video_codec == settings.video_player.video_codec
        })
    {
        crate::youtube_embed::apply(settings);
    }
    if differs("youtube", &|before| {
        before.youtube.sponsor_block == settings.youtube.sponsor_block
    }) {
        if let Some(sponsorblock) =
            app.try_state::<std::sync::Arc<crate::sponsorblock::SponsorBlock>>()
        {
            sponsorblock.set_prefs(settings.youtube.sponsor_block);
        }
        if before.is_some() {
            app.state::<crate::playback::PlaybackService>()
                .skips_changed();
        }
    }
}
