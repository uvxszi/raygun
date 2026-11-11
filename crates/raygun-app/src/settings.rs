//! Typed settings over the per-key `setting` table.
//!
//! A stored row is the JSON value of one [`SettingChange`] variant, so loading reuses the same
//! deserializer that validates UI input. Rows that no longer parse (an old version's key, a value
//! edited by hand) are skipped with a warning and the default is used instead.

use raygun_db::{Db, DbError, settings};
use raygun_model::{SettingChange, Settings};

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error(transparent)]
    Db(#[from] DbError),
    #[error("{0}")]
    Invalid(String),
}

/// Keys of removed settings, which old rows may still hold.
const RETIRED: &[&str] = &["discord"];

/// Keys of the fixed cover sizes before sizes followed the screen (M46.48): covers per row, and a
/// cover width in px; [`cover_size_from`] turns them into a cover size.
const OLD_COVER_KEYS: [&str; 2] = ["gridColumns", "coverWidth"];

/// The automatic cover width on a phone (6.5 rem), which old fixed sizes are compared with.
const PHONE_COVER_PX: f64 = 104.0;

/// The top-level video height of older versions (M42), now `VideoPlayerPrefs::preferred_resolution`.
const OLD_VIDEO_QUALITY_KEY: &str = "preferredVideoQuality";

pub async fn load(db: &Db) -> Result<Settings, SettingsError> {
    let mut settings = Settings::default();
    let (mut old_columns, mut old_width, mut cover_size_saved) = (0, 0, false);
    let (mut old_quality, mut resolution_saved) = (None, false);
    for (key, value) in settings::load(db).await? {
        // Settings that no longer exist (M46.36: Discord presence) are left alone.
        if RETIRED.contains(&key.as_str()) {
            continue;
        }
        if OLD_COVER_KEYS.contains(&key.as_str()) {
            let number = value.trim().parse::<u32>().unwrap_or(0);
            if key == "gridColumns" {
                old_columns = number;
            } else {
                old_width = number;
            }
            continue;
        }
        if key == OLD_VIDEO_QUALITY_KEY {
            old_quality = Some(serde_json::from_str::<Option<u32>>(&value).ok().flatten());
            continue;
        }
        cover_size_saved |= key == "coverSize";
        resolution_saved |= key == "videoPlayer" && has_field(&value, "preferredResolution");
        match parse_row(&key, &migrate_row(&key, &value)) {
            Ok(change) => settings.apply(change),
            Err(error) => tracing::warn!(%key, %value, %error, "ignoring unreadable setting"),
        }
    }
    if !cover_size_saved {
        settings.cover_size = cover_size_from(old_columns, old_width);
    }
    if let (Some(height), false) = (old_quality, resolution_saved) {
        settings.video_player.preferred_resolution =
            raygun_model::VideoResolution::from_height(height);
    }
    // Quick access saved by a version that pinned the audio library by hand.
    settings.rail = crate::quick_access::migrated(settings.rail);
    Ok(settings)
}

/// Whether a stored object row has `field`.
fn has_field(value: &str, field: &str) -> bool {
    matches!(
        serde_json::from_str::<serde_json::Value>(value),
        Ok(serde_json::Value::Object(fields)) if fields.contains_key(field)
    )
}

/// A cover size for what an older version stored (M46.48): more covers per row are smaller
/// covers, three being a phone's own; a width in px against a phone's own; neither keeps the
/// screen's size. Rounded to 5 %.
fn cover_size_from(columns: u32, width: u32) -> i32 {
    let ratio = if columns > 0 {
        3.0 / f64::from(columns)
    } else if width > 0 {
        f64::from(width) / PHONE_COVER_PX
    } else {
        return 0;
    };
    let percent = ((ratio - 1.0) * 20.0).round() as i32 * 5;
    percent.clamp(
        *raygun_model::COVER_SIZES.start(),
        *raygun_model::COVER_SIZES.end(),
    )
}

/// Rows stored by older versions, in today's shape. Before sizes followed the screen (M46.48):
/// the novel text's size in px becomes px more or less than the screen's own (18 px then), and
/// the manga reader's widths in percent of the window become percent more or less than the
/// screen's own. The novel reader's paragraph indent was a switch: on is the indent it gave
/// (2 em), off none.
fn migrate_row(key: &str, value: &str) -> String {
    if key == "rail" {
        return crate::quick_access::migrate_row(value);
    }
    type Rename = (&'static str, &'static str, fn(i64) -> i64);
    const NOVEL: &[Rename] = &[("fontSize", "textSize", old_novel_text)];
    const READER: &[Rename] = &[
        ("webtoonWidth", "webtoonSize", old_reader_width),
        ("barWidth", "barSize", old_reader_width),
    ];
    let renames = match key {
        "novelReader" => NOVEL,
        "mangaReader" => READER,
        _ => return value.to_owned(),
    };
    let Ok(serde_json::Value::Object(mut fields)) = serde_json::from_str(value) else {
        return value.to_owned();
    };
    for (old, new, convert) in renames {
        if let Some(number) = fields.remove(*old).and_then(|old| old.as_i64())
            && !fields.contains_key(*new)
        {
            fields.insert((*new).to_owned(), convert(number).into());
        }
    }
    if key == "novelReader"
        && let Some(indent) = fields.get_mut("indent")
        && let Some(switch) = indent.as_bool().or(indent.is_null().then_some(false))
    {
        *indent = if switch {
            raygun_model::NOVEL_INDENT_SWITCHED_ON
        } else {
            0
        }
        .into();
    }
    serde_json::Value::Object(fields).to_string()
}

/// Novel text in px (18 being the size then) as px more or less than the screen's own.
fn old_novel_text(px: i64) -> i64 {
    (px - 18).clamp(-6, 12)
}

/// A reader width in percent of the window (0: the screen's) as percent more or less than the
/// screen's own, which phones show at full width.
fn old_reader_width(percent: i64) -> i64 {
    if percent == 0 {
        0
    } else {
        (percent - 100).clamp(-70, 100)
    }
}

pub async fn update(db: &Db, change: SettingChange) -> Result<Settings, SettingsError> {
    update_many(db, vec![change]).await
}

/// Changes several settings together (a settings screen's reset): every change is checked first,
/// so a refused one leaves all of them unwritten.
pub async fn update_many(
    db: &Db,
    mut changes: Vec<SettingChange>,
) -> Result<Settings, SettingsError> {
    if turns_adult_on(&changes) {
        let before = load(db).await?;
        if let Some(pill) = pill_after(&before, &changes) {
            changes.push(SettingChange::SearchFilter(pill));
        }
    }
    for change in &mut changes {
        if let SettingChange::SearchFilter(prefs) = change {
            *prefs = one_adult_switch(*prefs);
        }
    }
    let mut rows = Vec::with_capacity(changes.len());
    for change in &changes {
        change.validate().map_err(SettingsError::Invalid)?;
        rows.push((change.key(), stored_value(change)?));
    }
    for (key, value) in rows {
        settings::put(db, key, &value).await?;
    }
    load(db).await
}

fn turns_adult_on(changes: &[SettingChange]) -> bool {
    changes
        .iter()
        .any(|change| matches!(change, SettingChange::ShowNsfw(true)))
}

/// Search's "No 18+" pill after `changes`: turning 18+ content on (from off) switches the pill on
/// (and "18+ only" off), so 18+ results show only once it's switched off too, unless the same
/// changes set the pill. Search's other filters stay as they were.
fn pill_after(
    before: &Settings,
    changes: &[SettingChange],
) -> Option<raygun_model::SearchFilterPrefs> {
    let sets_pill = changes
        .iter()
        .any(|change| matches!(change, SettingChange::SearchFilter(_)));
    (!before.show_nsfw && turns_adult_on(changes) && !sets_pill).then_some(
        raygun_model::SearchFilterPrefs {
            no_adult: true,
            adult_only: false,
            ..before.search_filter
        },
    )
}

/// "No 18+" and "18+ only" turn each other off; asked for both, the pill wins (it hides more).
fn one_adult_switch(prefs: raygun_model::SearchFilterPrefs) -> raygun_model::SearchFilterPrefs {
    raygun_model::SearchFilterPrefs {
        adult_only: prefs.adult_only && !prefs.no_adult,
        ..prefs
    }
}

/// The JSON a change is stored as: its value alone.
fn stored_value(change: &SettingChange) -> Result<String, SettingsError> {
    match serde_json::to_value(change).map_err(|error| SettingsError::Invalid(error.to_string()))? {
        serde_json::Value::Object(mut fields) => {
            Ok(fields.remove("value").unwrap_or_default().to_string())
        }
        _ => unreachable!("SettingChange serializes as an object"),
    }
}

/// The settings last put into effect in running services, to tell what a sync or a restore
/// changed.
static APPLIED: std::sync::Mutex<Option<Settings>> = std::sync::Mutex::new(None);

/// Puts settings that running services hold into effect: the system bars' theme, the content
/// filter, cache limits, turned-off sources, the User-Agent, sound and the charts' country. With
/// `changed`, only the keys it names; without, every key whose value differs from what was last
/// put into effect (all of them the first time), as after a sync or a restored backup.
pub async fn apply_live(app: &tauri::AppHandle, settings: &Settings, changed: Option<&[&str]>) {
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
    });
    if player && before.is_some() {
        crate::video::settings_changed(app).await;
    }
    if differs("network", &|before| before.network == settings.network) {
        crate::network_mode::apply(app, &settings.network);
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

/// The settings as services use them: what Data Saver changes while it applies
/// (`Settings::with_data_saver`) changed; what's stored stays as it is.
pub async fn load_in_effect(db: &Db) -> Result<Settings, SettingsError> {
    Ok(load(db)
        .await?
        .with_data_saver(raygun_net::network_mode::data_saver()))
}

/// After rows arrived from elsewhere (a sync, a restored backup, an undone sync): what they
/// changed is put into effect.
pub fn apply_stored(app: &tauri::AppHandle) {
    use tauri::Manager;

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let db = app.state::<crate::startup::AppDb>().inner().0.clone();
        match load(&db).await {
            Ok(settings) => apply_live(&app, &settings, None).await,
            Err(error) => tracing::warn!(%error, "settings unreadable after a sync"),
        }
    });
}

fn parse_row(key: &str, value: &str) -> Result<SettingChange, String> {
    let value: serde_json::Value =
        serde_json::from_str(value).map_err(|error| error.to_string())?;
    let change: SettingChange =
        serde_json::from_value(serde_json::json!({ "key": key, "value": value }))
            .map_err(|error| error.to_string())?;
    change.validate()?;
    Ok(change)
}

#[cfg(test)]
mod tests {
    use raygun_model::Theme;
    use tempfile::TempDir;

    use super::*;

    async fn open(temp: &TempDir) -> Db {
        Db::open(&temp.path().join("raygun.db")).await.unwrap()
    }

    #[tokio::test]
    async fn turning_adult_content_on_switches_the_no_adult_pill_on() {
        use raygun_model::SearchFilterPrefs;
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        let on = update(&db, SettingChange::ShowNsfw(true)).await.unwrap();
        assert!(on.search_filter.no_adult);

        // The pill switched off stays off while 18+ content stays on.
        let off = SearchFilterPrefs::default();
        update(&db, SettingChange::SearchFilter(off)).await.unwrap();
        let again = update(&db, SettingChange::ShowNsfw(true)).await.unwrap();
        assert!(!again.search_filter.no_adult);

        // Off, then on again: the pill is on again.
        update(&db, SettingChange::ShowNsfw(false)).await.unwrap();
        assert!(!load(&db).await.unwrap().search_filter.no_adult);
        let back = update(&db, SettingChange::ShowNsfw(true)).await.unwrap();
        assert!(back.search_filter.no_adult);

        // Changes that set the pill themselves keep what they set.
        update(&db, SettingChange::ShowNsfw(false)).await.unwrap();
        let both = update_many(
            &db,
            vec![
                SettingChange::ShowNsfw(true),
                SettingChange::SearchFilter(off),
            ],
        )
        .await
        .unwrap();
        assert!(!both.search_filter.no_adult);
    }

    #[tokio::test]
    async fn the_pill_and_adult_only_turn_each_other_off_and_other_filters_stay() {
        use raygun_model::SearchFilterPrefs;
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        let filters = SearchFilterPrefs {
            adult_only: true,
            playable_videos: true,
            hide_saved_books: true,
            ..SearchFilterPrefs::default()
        };
        update(&db, SettingChange::SearchFilter(filters))
            .await
            .unwrap();

        // Turning 18+ content on switches the pill on, "18+ only" off, and keeps the rest.
        let on = update(&db, SettingChange::ShowNsfw(true)).await.unwrap();
        assert_eq!(
            on.search_filter,
            SearchFilterPrefs {
                no_adult: true,
                adult_only: false,
                ..filters
            }
        );

        // Asked for both, the pill wins.
        let both = update(
            &db,
            SettingChange::SearchFilter(SearchFilterPrefs {
                no_adult: true,
                adult_only: true,
                ..filters
            }),
        )
        .await
        .unwrap();
        assert!(both.search_filter.no_adult && !both.search_filter.adult_only);
        assert!(both.search_filter.playable_videos && both.search_filter.hide_saved_books);
    }

    #[tokio::test]
    async fn defaults_when_nothing_is_stored() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;

        assert_eq!(load(&db).await.unwrap(), Settings::default());
    }

    #[tokio::test]
    async fn updates_persist_across_reopen() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;

        let updated = update(&db, SettingChange::Theme(Theme::Amoled))
            .await
            .unwrap();
        update(&db, SettingChange::Language("pt-BR".into()))
            .await
            .unwrap();
        db.close().await;
        let reopened = open(&temp).await;

        assert_eq!(updated.theme, Theme::Amoled);
        assert_eq!(
            load(&reopened).await.unwrap(),
            Settings {
                theme: Theme::Amoled,
                language: "pt-BR".into(),
                ..Settings::default()
            }
        );
    }

    #[tokio::test]
    async fn several_changes_are_written_together_or_not_at_all() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;

        let refused = update_many(
            &db,
            vec![
                SettingChange::Theme(Theme::Dark),
                SettingChange::CoverSize(400),
            ],
        )
        .await;
        assert!(matches!(refused, Err(SettingsError::Invalid(_))));
        assert_eq!(
            load(&db).await.unwrap(),
            Settings::default(),
            "nothing written"
        );

        let saved = update_many(
            &db,
            vec![
                SettingChange::Theme(Theme::Dark),
                SettingChange::CoverSize(-20),
            ],
        )
        .await
        .unwrap();
        assert_eq!((saved.theme, saved.cover_size), (Theme::Dark, -20));
    }

    #[test]
    fn every_default_is_a_change_the_ui_can_send_back() {
        // A screen's reset sends `{ key, value }` with each field of the defaults.
        let defaults = serde_json::to_value(Settings::default()).unwrap();
        let fields = defaults.as_object().unwrap();
        for (key, value) in fields {
            let change =
                parse_row(key, &value.to_string()).unwrap_or_else(|error| panic!("{key}: {error}"));
            assert_eq!(change.key(), key);
        }
        let mut applied = Settings::default();
        for (key, value) in fields {
            applied.apply(parse_row(key, &value.to_string()).unwrap());
        }
        assert_eq!(applied, Settings::default());
    }

    #[tokio::test]
    async fn rejects_invalid_values() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;

        assert!(
            update(&db, SettingChange::MangaUpdateHours(5))
                .await
                .is_err()
        );
        assert_eq!(
            update(&db, SettingChange::MangaUpdateHours(24))
                .await
                .unwrap()
                .manga_update_hours,
            24
        );
        let result = update(&db, SettingChange::Language("not a tag!".into())).await;

        assert!(matches!(result, Err(SettingsError::Invalid(_))));
        assert_eq!(
            load(&db).await.unwrap(),
            Settings {
                manga_update_hours: 24,
                ..Settings::default()
            },
            "invalid values leave the stored ones alone"
        );
    }

    #[tokio::test]
    async fn the_video_resolution_starts_at_1080p_and_takes_over_the_old_quality() {
        use raygun_model::{VideoPlayerPrefs, VideoResolution};
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        async fn resolution(db: &Db) -> VideoResolution {
            load(db).await.unwrap().video_player.preferred_resolution
        }
        assert_eq!(resolution(&db).await, VideoResolution::P1080);

        // An older version's choice: none was the highest.
        settings::put(&db, OLD_VIDEO_QUALITY_KEY, "null")
            .await
            .unwrap();
        assert_eq!(resolution(&db).await, VideoResolution::Highest);
        settings::put(&db, OLD_VIDEO_QUALITY_KEY, "720")
            .await
            .unwrap();
        assert_eq!(resolution(&db).await, VideoResolution::P720);
        // A player row saved before the field existed keeps the old choice.
        settings::put(&db, "videoPlayer", r#"{"controlsSeconds":5}"#)
            .await
            .unwrap();
        assert_eq!(resolution(&db).await, VideoResolution::P720);

        // Once chosen, the new field wins.
        let prefs = VideoPlayerPrefs {
            preferred_resolution: VideoResolution::P2160,
            ..VideoPlayerPrefs::default()
        };
        let saved = update(&db, SettingChange::VideoPlayer(Box::new(prefs)))
            .await
            .unwrap();
        assert_eq!(
            saved.video_player.preferred_resolution,
            VideoResolution::P2160
        );
        assert_eq!(resolution(&db).await, VideoResolution::P2160);
        assert!(
            serde_json::from_str::<SettingChange>(
                r#"{"key":"videoPlayer","value":{"preferredResolution":"900p"}}"#
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn video_player_settings_read_partial_rows_and_refuse_bad_values() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        // A row saved before later fields existed keeps their defaults.
        raygun_db::settings::put(
            &db,
            "videoPlayer",
            r#"{"swapSides":true,"subtitleStyle":{"size":150}}"#,
        )
        .await
        .unwrap();
        let loaded = load(&db).await.unwrap().video_player;
        assert!(loaded.swap_sides);
        assert_eq!(loaded.subtitle_style.size, 150);
        assert_eq!(loaded.subtitle_style.position, 8);
        // Lines keep to 80 % of the picture's width, and no colour counts as picked.
        assert_eq!(loaded.subtitle_style.max_width, 80);
        assert!(!loaded.subtitle_style.text_color_chosen);
        assert!(!loaded.subtitle_style.background_color_chosen);
        assert_eq!(loaded.default_speed, 100);

        let player = |change: fn(&mut raygun_model::VideoPlayerPrefs)| {
            let mut prefs = raygun_model::VideoPlayerPrefs::default();
            change(&mut prefs);
            SettingChange::VideoPlayer(Box::new(prefs))
        };
        for bad in [
            player(|prefs| prefs.controls_seconds = 6),
            player(|prefs| prefs.default_speed = 500),
            player(|prefs| prefs.speed_presets.clear()),
            player(|prefs| prefs.hold_speed = 250),
            player(|prefs| prefs.volume_boost = 35),
            player(|prefs| prefs.skip_wait_seconds = 4),
            player(|prefs| prefs.subtitle_style.size = 20),
            player(|prefs| prefs.subtitle_style.max_width = 30),
            player(|prefs| prefs.subtitle_style.max_width = 120),
            player(|prefs| prefs.audio_language = "not a tag!".into()),
        ] {
            assert!(update(&db, bad.clone()).await.is_err(), "{bad:?}");
        }
        let saved = update(&db, player(|prefs| prefs.speed_presets = vec![100, 400]))
            .await
            .unwrap();
        assert_eq!(saved.video_player.speed_presets, vec![100, 400]);
    }

    #[tokio::test]
    async fn display_and_player_settings_keep_to_what_is_offered() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        for bad in [
            SettingChange::CoverSize(-60),
            SettingChange::CoverSize(110),
            SettingChange::WatchedPercent(42),
            SettingChange::WatchedPercent(92),
            SettingChange::SeekSeconds(7),
            SettingChange::SubtitleLanguage("not a tag!".into()),
        ] {
            assert!(update(&db, bad.clone()).await.is_err(), "{bad:?}");
        }
        update(&db, SettingChange::Accent(raygun_model::Accent::Teal))
            .await
            .unwrap();
        update(&db, SettingChange::CoverSize(25)).await.unwrap();
        update(&db, SettingChange::WatchedPercent(85))
            .await
            .unwrap();
        update(&db, SettingChange::SeekSeconds(30)).await.unwrap();
        update(&db, SettingChange::SubtitleLanguage("pt-BR".into()))
            .await
            .unwrap();
        let saved = update(&db, SettingChange::SubtitleLanguage(String::new()))
            .await
            .unwrap();
        assert_eq!(
            (
                saved.accent,
                saved.cover_size,
                saved.watched_percent,
                saved.seek_seconds
            ),
            (raygun_model::Accent::Teal, 25, 85, 30)
        );
        assert_eq!(
            saved.subtitle_language, "",
            "empty follows the app's language"
        );
        assert!(saved.resume_videos && !saved.auto_subtitles, "defaults");
    }

    #[test]
    fn the_old_indent_switch_becomes_an_amount() {
        let indent = |stored: &str| {
            let row = format!(
                r#"{{"lineSpacing":16,"font":"serif","width":"narrow","paged":false,"indent":{stored}}}"#
            );
            match parse_row("novelReader", &migrate_row("novelReader", &row)) {
                Ok(SettingChange::NovelReader(prefs)) => prefs.indent,
                other => panic!("{stored}: {other:?}"),
            }
        };
        assert_eq!(indent("true"), raygun_model::NOVEL_INDENT_SWITCHED_ON);
        assert_eq!(indent("false"), 0);
        assert_eq!(indent("null"), 0);
        assert_eq!(indent("15"), 15, "an amount stays as it is");
        // Only the novel reader's row has one.
        let other = r#"{"indent":true}"#;
        assert_eq!(migrate_row("mangaReader", other), other);
    }

    #[tokio::test]
    async fn fixed_sizes_from_before_become_sizes_from_the_screen() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        // An older version's rows: four covers per row, a cover width, 20 px novel text with the
        // paragraph indent switched on, a webtoon column at 60 % of the window and the page bar at
        // the screen's own width.
        for (key, value) in [
            ("gridColumns", "4"),
            ("coverWidth", "140"),
            (
                "novelReader",
                r#"{"fontSize":20,"lineSpacing":16,"font":"serif","width":"narrow","paged":false,"indent":true}"#,
            ),
            ("mangaReader", r#"{"webtoonWidth":60,"barWidth":0}"#),
        ] {
            settings::put(&db, key, value).await.unwrap();
        }
        let loaded = load(&db).await.unwrap();
        assert_eq!(
            loaded.cover_size, -25,
            "four across a phone: smaller covers"
        );
        assert_eq!(loaded.novel_reader.text_size, 2);
        assert_eq!(
            loaded.novel_reader.indent,
            raygun_model::NOVEL_INDENT_SWITCHED_ON,
            "the switch's indent, 2 em"
        );
        assert_eq!(loaded.manga_reader.webtoon_size, -40);
        assert_eq!(loaded.manga_reader.bar_size, 0);

        // A width alone, and what's saved now winning over both.
        assert_eq!(cover_size_from(0, 140), 35);
        assert_eq!(cover_size_from(0, 0), 0);
        assert_eq!(cover_size_from(8, 0), -50, "kept within the sizes offered");
        update(&db, SettingChange::CoverSize(10)).await.unwrap();
        assert_eq!(load(&db).await.unwrap().cover_size, 10);
    }

    #[tokio::test]
    async fn unreadable_rows_fall_back_to_defaults() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        settings::put(&db, "theme", "\"neon\"").await.unwrap();
        settings::put(&db, "removedInOldVersion", "true")
            .await
            .unwrap();
        settings::put(&db, "language", "\"ja\"").await.unwrap();

        let loaded = load(&db).await.unwrap();

        assert_eq!(loaded.theme, Theme::System);
        assert_eq!(loaded.language, "ja");
    }

    fn item(id: &str, visible: bool) -> raygun_model::LayoutItem {
        raygun_model::LayoutItem {
            id: id.to_owned(),
            visible,
        }
    }

    #[tokio::test]
    async fn layouts_keep_the_users_order_and_take_new_entries_in_place() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        // A layout saved by an older version: More first, an id since removed, Browse missing.
        settings::put(
            &db,
            "layout",
            r#"{"tabs":[{"id":"more","visible":true},{"id":"gone","visible":true},{"id":"library","visible":true},{"id":"home","visible":false}]}"#,
        )
        .await
        .unwrap();

        let layout = load(&db).await.unwrap().layout;
        let tabs: Vec<(&str, bool)> = layout
            .tabs
            .iter()
            .map(|tab| (tab.id.as_str(), tab.visible))
            .collect();
        // New tabs go after their neighbour by default (More, here, now first).
        let first_four: Vec<(&str, bool)> = tabs
            .iter()
            .copied()
            .filter(|(id, _)| ["home", "library", "browse", "more"].contains(id))
            .collect();
        assert_eq!(
            first_four,
            [
                ("more", true),
                ("library", true),
                ("browse", true),
                ("home", false)
            ],
            "the saved order, unknown ids dropped, Browse back after Library"
        );
        assert!(!tabs.iter().any(|(id, _)| *id == "gone"));
        assert_eq!(tabs.len(), raygun_model::LAYOUT_TABS.len());
        assert!(
            tabs.contains(&("watch", false)),
            "tabs that can be added are listed hidden"
        );
        // The other lists are whole.
        assert_eq!(
            layout.more_entries.len(),
            raygun_model::LAYOUT_MORE_ENTRIES.len()
        );
        // "Nothing to pick up yet" leads Home (M46.37), then the rows as before.
        assert_eq!(layout.home_sections[0].id, "getStarted");
        assert_eq!(layout.home_sections[1].id, "continueWatching");
    }

    #[tokio::test]
    async fn layouts_keep_more_settings_and_a_tab() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        let base = load(&db).await.unwrap().layout;
        let hide = |list: &[raygun_model::LayoutItem], id: &str| -> Vec<raygun_model::LayoutItem> {
            list.iter()
                .map(|entry| item(&entry.id, entry.visible && entry.id != id))
                .collect()
        };

        for layout in [
            raygun_model::LayoutPrefs {
                tabs: hide(&base.tabs, "more"),
                ..base.clone()
            },
            raygun_model::LayoutPrefs {
                more_entries: hide(&base.more_entries, "settings"),
                ..base.clone()
            },
            raygun_model::LayoutPrefs {
                browse_tabs: hide(&base.browse_tabs, "search"),
                ..base.clone()
            },
            raygun_model::LayoutPrefs {
                tabs: base
                    .tabs
                    .iter()
                    .map(|tab| item(&tab.id, tab.id == "more"))
                    .collect(),
                ..base.clone()
            },
            raygun_model::LayoutPrefs {
                library_media: base
                    .library_media
                    .iter()
                    .map(|tab| item(&tab.id, false))
                    .collect(),
                ..base.clone()
            },
        ] {
            assert!(
                update(&db, SettingChange::Layout(layout)).await.is_err(),
                "refused"
            );
        }

        // Adding Watch as a tab and hiding Home is fine, and the start screen can be Watch.
        let mut tabs = hide(&base.tabs, "home");
        tabs.iter_mut()
            .filter(|tab| tab.id == "watch")
            .for_each(|tab| tab.visible = true);
        let saved = update(
            &db,
            SettingChange::Layout(raygun_model::LayoutPrefs {
                tabs,
                ..base.clone()
            }),
        )
        .await
        .unwrap();
        assert!(
            saved
                .layout
                .tabs
                .iter()
                .any(|tab| tab.id == "watch" && tab.visible)
        );
        let saved = update(
            &db,
            SettingChange::StartScreen(raygun_model::StartScreen::Watch),
        )
        .await
        .unwrap();
        assert_eq!(saved.start_screen, raygun_model::StartScreen::Watch);
    }

    #[tokio::test]
    async fn searching_through_listings_keeps_to_what_is_offered() {
        use raygun_model::SearchListingsPrefs;
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        assert_eq!(
            load(&db).await.unwrap().search_listings,
            SearchListingsPrefs {
                enabled: true,
                entries: 20,
                pages: 5
            }
        );
        let prefs = |entries, pages| {
            SettingChange::SearchListings(SearchListingsPrefs {
                enabled: false,
                entries,
                pages,
            })
        };
        for bad in [prefs(0, 5), prefs(101, 5), prefs(20, 0), prefs(20, 21)] {
            assert!(update(&db, bad.clone()).await.is_err(), "{bad:?}");
        }
        let saved = update(&db, prefs(100, 20)).await.unwrap();
        assert_eq!(
            (
                saved.search_listings.enabled,
                saved.search_listings.entries,
                saved.search_listings.pages
            ),
            (false, 100, 20)
        );
        // A row saved without a field keeps that field's default.
        settings::put(&db, "searchListings", r#"{"enabled":false}"#)
            .await
            .unwrap();
        assert_eq!(
            load(&db).await.unwrap().search_listings,
            SearchListingsPrefs {
                enabled: false,
                ..SearchListingsPrefs::default()
            }
        );
    }

    #[tokio::test]
    async fn browse_start_tab_is_one_of_browses_tabs() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        assert_eq!(load(&db).await.unwrap().browse_start_tab, "discover");

        for (tab, _) in raygun_model::LAYOUT_BROWSE_TABS {
            let saved = update(&db, SettingChange::BrowseStartTab(tab.into()))
                .await
                .unwrap();
            assert_eq!(saved.browse_start_tab, tab);
        }
        assert!(
            update(&db, SettingChange::BrowseStartTab("charts".into()))
                .await
                .is_err()
        );
        assert!(
            update(&db, SettingChange::BrowseStartTab(String::new()))
                .await
                .is_err()
        );
        assert_eq!(load(&db).await.unwrap().browse_start_tab, "extensions");
    }

    #[tokio::test]
    async fn text_sizes_and_fonts_keep_to_what_css_can_take() {
        use raygun_model::FontPrefs;
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        let saved = load(&db).await.unwrap();
        assert_eq!((saved.text_size, &saved.fonts), (0, &FontPrefs::default()));

        // Text size: percent more or less than the screen's own, over the interface size.
        let saved = update(&db, SettingChange::TextSize(-30)).await.unwrap();
        assert_eq!(saved.text_size, -30);
        let saved = update(&db, SettingChange::TextSize(100)).await.unwrap();
        assert_eq!(saved.text_size, 100);
        for bad in [-35, 105, 400] {
            assert!(
                update(&db, SettingChange::TextSize(bad)).await.is_err(),
                "{bad}"
            );
        }

        // Fonts: family names, kept as named, without the spaces around them.
        let fonts = |interface: &str, reading: &str| {
            SettingChange::Fonts(FontPrefs {
                interface: interface.into(),
                reading: reading.into(),
            })
        };
        let saved = update(&db, fonts(" Segoe UI Variable ", "Noto Serif"))
            .await
            .unwrap();
        assert_eq!(saved.fonts.interface, "Segoe UI Variable");
        assert_eq!(saved.fonts.reading, "Noto Serif");
        assert_eq!(load(&db).await.unwrap().fonts, saved.fonts);
        for bad in [
            "Arial\"; color: red",
            "Arial}",
            "Back\\slash",
            "Line\nbreak",
            &"x".repeat(201),
        ] {
            assert!(update(&db, fonts(bad, "")).await.is_err(), "{bad}");
            assert!(update(&db, fonts("", bad)).await.is_err(), "{bad}");
        }
        let saved = update(&db, fonts("", "")).await.unwrap();
        assert_eq!(saved.fonts, FontPrefs::default(), "back to the defaults");
    }

    #[tokio::test]
    async fn custom_accents_cover_sizes_and_reader_zoom_keep_to_what_is_offered() {
        use raygun_model::{
            Accent, CustomColors, MangaReaderPrefs, PagedScale, PanelPrefs, PlayerLayout,
            PlayerViewPrefs, TapCell, TapZones, TextAlign, TrackTablePrefs, TrackTableView,
        };
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;

        let saved = update(&db, SettingChange::AccentColor("#1E90FF".into()))
            .await
            .unwrap();
        assert_eq!(saved.accent_color, "#1e90ff");

        // The desktop window (M46.9): its frame, and the interface size from 50 to 200 % of the
        // screen's (M46.48).
        assert_eq!(saved.zoom, 100);
        assert_eq!(saved.window_frame, raygun_model::WindowFrame::App);
        let saved = update(&db, SettingChange::Zoom(135)).await.unwrap();
        assert_eq!(saved.zoom, 135);
        for bad in [40, 49, 210] {
            assert!(
                update(&db, SettingChange::Zoom(bad)).await.is_err(),
                "{bad}"
            );
        }
        let saved = update(
            &db,
            SettingChange::WindowFrame(raygun_model::WindowFrame::System),
        )
        .await
        .unwrap();
        assert_eq!(saved.window_frame, raygun_model::WindowFrame::System);

        // Charts follow a country code, or the app's language when empty (M46.9).
        assert!(saved.discovery.tag_songs);
        let discovery = |region: &str| {
            SettingChange::Discovery(raygun_model::DiscoveryPrefs {
                region: region.into(),
                tag_songs: false,
            })
        };
        let saved = update(&db, discovery("GB")).await.unwrap();
        assert_eq!(saved.discovery.region, "gb");
        assert!(!saved.discovery.tag_songs);
        for bad in ["gbr", "g1", "u s"] {
            assert!(update(&db, discovery(bad)).await.is_err(), "{bad}");
        }
        let saved = update(&db, SettingChange::Accent(Accent::Custom))
            .await
            .unwrap();
        assert_eq!(saved.accent, Accent::Custom);
        for bad in ["blue", "#12345", "#12345g", "1e90ff0"] {
            assert!(
                update(&db, SettingChange::AccentColor(bad.into()))
                    .await
                    .is_err(),
                "{bad}"
            );
        }

        assert_eq!(
            update(&db, SettingChange::CoverSize(15))
                .await
                .unwrap()
                .cover_size,
            15
        );
        assert!(update(&db, SettingChange::CoverSize(-55)).await.is_err());

        let zoom = MangaReaderPrefs {
            paged_scale: PagedScale::FitWidth,
            webtoon_size: -40,
            bar_size: -50,
            bar_zoom: false,
            preload_pages: 6,
            ..MangaReaderPrefs::default()
        };
        assert_eq!(
            update(&db, SettingChange::MangaReader(zoom))
                .await
                .unwrap()
                .manga_reader,
            zoom
        );
        for width in [-75, 105] {
            let bad = MangaReaderPrefs {
                webtoon_size: width,
                ..zoom
            };
            assert!(
                update(&db, SettingChange::MangaReader(bad)).await.is_err(),
                "{width}"
            );
        }
        for bad in [
            MangaReaderPrefs {
                bar_size: -80,
                ..zoom
            },
            MangaReaderPrefs {
                preload_pages: 0,
                ..zoom
            },
            MangaReaderPrefs {
                preload_pages: 11,
                ..zoom
            },
        ] {
            assert!(update(&db, SettingChange::MangaReader(bad)).await.is_err());
        }
        // Rows from before the custom tap grids read with the default layouts.
        let older: MangaReaderPrefs = serde_json::from_str(r#"{"tapZones":"kindle"}"#).unwrap();
        assert_eq!(
            older.tap_grid_paged,
            MangaReaderPrefs::default().tap_grid_paged
        );
        assert_eq!(
            older.tap_grid_webtoon,
            MangaReaderPrefs::default().tap_grid_webtoon
        );
        assert!(older.bar_progress);
        assert!(older.full_screen);
        let mut custom = MangaReaderPrefs {
            tap_zones: TapZones::Custom,
            ..zoom
        };
        custom.tap_grid_webtoon[4] = TapCell::NextChapter;
        assert_eq!(
            update(&db, SettingChange::MangaReader(custom))
                .await
                .unwrap()
                .manga_reader,
            custom
        );

        // Song tables, panels and colours (M46.8).
        let mut tables = TrackTablePrefs::default();
        tables
            .tables
            .insert("favorites".into(), TrackTableView::default());
        assert!(
            update(&db, SettingChange::TrackTables(tables.clone()))
                .await
                .is_ok()
        );
        let mut unknown = tables.clone();
        unknown
            .tables
            .insert("somewhere".into(), TrackTableView::default());
        assert!(
            update(&db, SettingChange::TrackTables(unknown))
                .await
                .is_err()
        );
        let mut twice = TrackTableView::default();
        twice.columns.push(twice.columns[0]);
        let mut doubled = TrackTablePrefs::default();
        doubled.tables.insert("playlist".into(), twice);
        assert!(
            update(&db, SettingChange::TrackTables(doubled))
                .await
                .is_err()
        );
        let panels = PanelPrefs {
            rail_width: 72,
            side_panel_width: 400,
            player_split: 40,
            next_up_rows: 4,
            rail_nav_height: 240,
        };
        assert_eq!(
            update(&db, SettingChange::Panels(panels))
                .await
                .unwrap()
                .panels,
            panels
        );
        assert!(
            update(
                &db,
                SettingChange::Panels(PanelPrefs {
                    rail_width: 40,
                    ..panels
                })
            )
            .await
            .is_err()
        );
        assert!(
            update(
                &db,
                SettingChange::Panels(PanelPrefs {
                    rail_nav_height: 20,
                    ..panels
                })
            )
            .await
            .is_err(),
            "the rail's tabs keep a row"
        );
        let colors = CustomColors {
            background: "#101010".into(),
            surface: "".into(),
            foreground: "#FAFAFA".into(),
            muted: "".into(),
        };
        assert_eq!(
            update(&db, SettingChange::Colors(colors))
                .await
                .unwrap()
                .colors
                .foreground,
            "#fafafa"
        );
        assert!(
            update(
                &db,
                SettingChange::Colors(CustomColors {
                    background: "red".into(),
                    ..CustomColors::default()
                })
            )
            .await
            .is_err()
        );

        // The full player's layout.
        let view = PlayerViewPrefs {
            layout: PlayerLayout::SideBySide,
            cover_offset: -20,
            width_offset: 15,
            lyrics_offset: 6,
            lyrics_align: TextAlign::Center,
            background: false,
        };
        assert_eq!(
            update(&db, SettingChange::PlayerView(view))
                .await
                .unwrap()
                .player_view,
            view
        );
        for bad in [
            PlayerViewPrefs {
                cover_offset: 41,
                ..view
            },
            PlayerViewPrefs {
                width_offset: -45,
                ..view
            },
            PlayerViewPrefs {
                lyrics_offset: -6,
                ..view
            },
            PlayerViewPrefs {
                lyrics_offset: 11,
                ..view
            },
        ] {
            assert!(update(&db, SettingChange::PlayerView(bad)).await.is_err());
        }
    }

    #[tokio::test]
    async fn old_player_sizes_become_offsets() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        // Before the offsets: the whole window's width and 32 px lyrics, centred.
        settings::put(
            &db,
            "playerView",
            r#"{"layout":"stacked","width":100,"lyricsSize":32,"lyricsAlign":"center"}"#,
        )
        .await
        .unwrap();

        let view = load(&db).await.unwrap().player_view;

        assert_eq!(view.layout, raygun_model::PlayerLayout::Stacked);
        assert_eq!(view.width_offset, 33);
        assert_eq!(view.lyrics_offset, 10);
        assert_eq!(view.cover_offset, 0);
        assert_eq!(view.lyrics_align, raygun_model::TextAlign::Center);
        assert!(view.background);

        // "By screen" was 0, and stays the screen's size.
        settings::put(&db, "playerView", r#"{"width":0,"lyricsSize":0}"#)
            .await
            .unwrap();
        assert_eq!(
            load(&db).await.unwrap().player_view,
            raygun_model::PlayerViewPrefs::default()
        );
    }
}
