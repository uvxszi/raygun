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

pub async fn load(db: &Db) -> Result<Settings, SettingsError> {
    let mut settings = Settings::default();
    for (key, value) in settings::load(db).await? {
        match parse_row(&key, &value) {
            Ok(change) => settings.apply(change),
            Err(error) => tracing::warn!(%key, %value, %error, "ignoring unreadable setting"),
        }
    }
    Ok(settings)
}

pub async fn update(db: &Db, change: SettingChange) -> Result<Settings, SettingsError> {
    change.validate().map_err(SettingsError::Invalid)?;
    let value = match serde_json::to_value(&change)
        .map_err(|error| SettingsError::Invalid(error.to_string()))?
    {
        serde_json::Value::Object(mut fields) => fields.remove("value").unwrap_or_default(),
        _ => unreachable!("SettingChange serializes as an object"),
    };
    settings::put(db, change.key(), &value.to_string()).await?;
    load(db).await
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
    async fn video_quality_is_one_offered_or_the_highest() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        let quality = |height| SettingChange::PreferredVideoQuality(height);

        assert!(update(&db, quality(Some(900))).await.is_err());
        let capped = update(&db, quality(Some(720))).await.unwrap();
        assert_eq!(capped.preferred_video_quality, Some(720));
        let highest = update(&db, quality(None)).await.unwrap();
        assert_eq!(highest.preferred_video_quality, None, "back to the highest");
        assert_eq!(load(&db).await.unwrap().preferred_video_quality, None);
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
            SettingChange::GridColumns(1),
            SettingChange::GridColumns(9),
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
        update(&db, SettingChange::GridColumns(4)).await.unwrap();
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
                saved.grid_columns,
                saved.watched_percent,
                saved.seek_seconds
            ),
            (raygun_model::Accent::Teal, 4, 85, 30)
        );
        assert_eq!(
            saved.subtitle_language, "",
            "empty follows the app's language"
        );
        assert!(saved.resume_videos && !saved.auto_subtitles, "defaults");
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
}
