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

pub async fn load(db: &Db) -> Result<Settings, SettingsError> {
    let mut settings = Settings::default();
    for (key, value) in settings::load(db).await? {
        // Settings that no longer exist (M46.36: Discord presence) are left alone.
        if RETIRED.contains(&key.as_str()) {
            continue;
        }
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
    async fn custom_accents_cover_sizes_and_reader_zoom_keep_to_what_is_offered() {
        use raygun_model::{
            Accent, CustomColors, MangaReaderPrefs, PagedScale, PanelPrefs, PlayerLayout,
            PlayerViewPrefs, TextAlign, TrackTablePrefs, TrackTableView,
        };
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;

        let saved = update(&db, SettingChange::AccentColor("#1E90FF".into()))
            .await
            .unwrap();
        assert_eq!(saved.accent_color, "#1e90ff");

        // The desktop window (M46.9): its frame, and zoom in steps of 10 % from 50 to 200.
        assert_eq!(saved.zoom, 100);
        assert_eq!(saved.window_frame, raygun_model::WindowFrame::App);
        let saved = update(&db, SettingChange::Zoom(130)).await.unwrap();
        assert_eq!(saved.zoom, 130);
        for bad in [40, 125, 210] {
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
            update(&db, SettingChange::CoverWidth(120))
                .await
                .unwrap()
                .cover_width,
            120
        );
        assert!(update(&db, SettingChange::CoverWidth(125)).await.is_err());

        let zoom = MangaReaderPrefs {
            paged_scale: PagedScale::FitWidth,
            webtoon_width: 60,
            bar_width: 50,
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
        for width in [20, 62, 105] {
            let bad = MangaReaderPrefs {
                webtoon_width: width,
                ..zoom
            };
            assert!(
                update(&db, SettingChange::MangaReader(bad)).await.is_err(),
                "{width}"
            );
        }
        for bad in [
            MangaReaderPrefs {
                bar_width: 35,
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
