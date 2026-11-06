use tempfile::TempDir;

use super::*;
use crate::titles::{self, TitleFields, UnitFields};

async fn open(temp: &TempDir, name: &str) -> Db {
    Db::open(&temp.path().join(name)).await.unwrap()
}

fn manga(remote_id: &str, title: &str) -> TitleFields {
    TitleFields {
        kind: "manga".into(),
        source_id: "dev.raygun.archive-comics/comics".into(),
        remote_id: remote_id.into(),
        title: title.into(),
        cover: None,
        cover_headers: [("User-Agent".to_owned(), "Raygun".to_owned())].into(),
        authors: vec!["Yamada".into()],
        artists: vec![],
        description: None,
        genres: vec!["Fantasy".into()],
        status: "ongoing".into(),
        url: None,
        media_type: None,
        year: None,
    }
}

fn chapter(remote_id: &str, number: f64) -> UnitFields {
    UnitFields {
        remote_id: remote_id.into(),
        name: format!("Chapter {number}"),
        number: Some(number),
        volume: None,
        scanlator: None,
        date: None,
    }
}

/// A library title with two chapters, the first read and the second half read.
async fn reader_state(db: &Db) -> String {
    let id = titles::upsert_title(db, &manga("m1", "Frieren"), 1)
        .await
        .unwrap();
    titles::set_in_library(db, &id, Some(5)).await.unwrap();
    titles::set_reader_mode(db, &id, "pagedRtl").await.unwrap();
    titles::sync_units(
        db,
        &id,
        &[chapter("c2", 2.0), chapter("c1", 1.0)],
        "chapter",
        2,
    )
    .await
    .unwrap();
    let units = titles::units(db, &id).await.unwrap();
    titles::set_completed(db, &units[1].id, true, 6)
        .await
        .unwrap();
    titles::set_progress(db, &units[0].id, 4, Some(20), 7)
        .await
        .unwrap();
    id
}

#[tokio::test]
async fn a_backup_restores_into_an_empty_database() {
    let temp = TempDir::new().unwrap();
    let source = open(&temp, "a.db").await;
    let title_id = reader_state(&source).await;
    crate::settings::put(&source, "theme", "\"amoled\"")
        .await
        .unwrap();

    let backup = export(&source, 99, &Part::ALL).await.unwrap();
    assert_eq!((backup.format.as_str(), backup.version), (FORMAT, VERSION));
    assert_eq!(backup.live_rows("title"), 1);
    assert_eq!(backup.live_rows("unit"), 2);
    // Through text, as it travels in a file.
    let backup: BackupData =
        serde_json::from_str(&serde_json::to_string(&backup).unwrap()).unwrap();

    let target = open(&temp, "b.db").await;
    let restored = restore(&target, &backup, &Part::ALL).await.unwrap();
    assert!(restored.changed >= 5, "{restored:?}");

    let title = titles::title(&target, &title_id).await.unwrap().unwrap();
    assert_eq!(title.fields.title, "Frieren");
    assert_eq!(title.fields.cover_headers["User-Agent"], "Raygun");
    assert_eq!(
        (title.in_library_at, title.reader_mode.as_str()),
        (Some(5), "pagedRtl")
    );
    let units = titles::units(&target, &title_id).await.unwrap();
    assert_eq!(units[1].completed_at, Some(6));
    assert_eq!((units[0].position, units[0].total), (Some(4), Some(20)));
    assert_eq!(
        crate::settings::load(&target).await.unwrap(),
        [("theme".to_owned(), "\"amoled\"".to_owned())]
    );

    let again = restore(&target, &backup, &Part::ALL).await.unwrap();
    assert_eq!(again.changed, 0, "restoring twice changes nothing");
}

#[tokio::test]
async fn restore_keeps_newer_local_rows_and_adopts_local_ids() {
    let temp = TempDir::new().unwrap();
    let source = open(&temp, "a.db").await;
    let backup_title = reader_state(&source).await;
    crate::settings::put(&source, "theme", "\"light\"")
        .await
        .unwrap();
    let backup = export(&source, 99, &Part::ALL).await.unwrap();

    // The other device fetched the same title on its own (another id), read chapter 2 to the
    // end, and changed the theme after the backup was made.
    let target = open(&temp, "b.db").await;
    let local_title = titles::upsert_title(&target, &manga("m1", "Frieren"), 1)
        .await
        .unwrap();
    assert_ne!(local_title, backup_title);
    titles::sync_units(&target, &local_title, &[chapter("c2", 2.0)], "chapter", 3)
        .await
        .unwrap();
    let c2 = titles::units(&target, &local_title).await.unwrap()[0]
        .id
        .clone();
    target.observe(source.tick());
    titles::set_progress(&target, &c2, 19, Some(20), 8)
        .await
        .unwrap();
    crate::settings::put(&target, "theme", "\"dark\"")
        .await
        .unwrap();

    restore(&target, &backup, &Part::ALL).await.unwrap();

    assert!(
        titles::title(&target, &backup_title)
            .await
            .unwrap()
            .is_none(),
        "the backup's title took the local id"
    );
    let title = titles::title(&target, &local_title).await.unwrap().unwrap();
    assert_eq!(
        title.in_library_at,
        Some(5),
        "the local row is newer, but its library gap is filled"
    );
    let units = titles::units(&target, &local_title).await.unwrap();
    let names: Vec<_> = units
        .iter()
        .map(|unit| unit.fields.remote_id.as_str())
        .collect();
    assert_eq!(names, ["c2", "c1"], "chapters joined the local title");
    let c2 = units.iter().find(|unit| unit.id == c2).unwrap();
    assert_eq!(
        (c2.position, c2.completed_at),
        (Some(19), Some(8)),
        "newer local progress stays"
    );
    assert_eq!(
        crate::settings::load(&target).await.unwrap(),
        [("theme".to_owned(), "\"dark\"".to_owned())],
        "newer local setting stays"
    );
}

#[tokio::test]
async fn a_song_favorited_on_both_sides_restores_as_one_favorite() {
    use crate::collections::{TrackFields, favorites, set_favorite};
    let song = |title: &str| TrackFields {
        track_key: format!("nina simone|{}", title.to_lowercase()),
        title: title.into(),
        artist: Some("Nina Simone".into()),
        album: None,
        duration_ms: None,
        artwork_url: None,
        source: None,
    };
    let temp = TempDir::new().unwrap();
    let source = open(&temp, "a.db").await;
    set_favorite(&source, &song("Sinnerman"), true, 1)
        .await
        .unwrap();
    set_favorite(&source, &song("Feeling Good"), true, 2)
        .await
        .unwrap();
    let backup = export(&source, 9, &Part::ALL).await.unwrap();

    // Favorited here too, under another id: the unique favorite per song used to fail the restore.
    let target = open(&temp, "b.db").await;
    set_favorite(&target, &song("Sinnerman"), true, 5)
        .await
        .unwrap();
    let ours = favorites(&target).await.unwrap()[0].id.clone();
    restore(&target, &backup, &Part::ALL).await.unwrap();

    let restored = favorites(&target).await.unwrap();
    let titles: Vec<&str> = restored
        .iter()
        .map(|row| row.track.title.as_str())
        .collect();
    assert_eq!(restored.len(), 2, "{titles:?}");
    assert!(restored.iter().any(|row| row.id == ours), "our row stays");
}

pub(crate) fn show(feed: &str, kind: &str) -> crate::podcasts::PodcastFields {
    crate::podcasts::PodcastFields {
        feed_url: feed.into(),
        title: format!("Show {feed}"),
        author: None,
        description: None,
        artwork: None,
        link: None,
        kind: kind.into(),
    }
}

pub(crate) fn episode(guid: &str) -> crate::podcasts::EpisodeFields {
    crate::podcasts::EpisodeFields {
        guid: guid.into(),
        title: format!("Episode {guid}"),
        description: None,
        audio_url: format!("https://example.org/{guid}.mp3"),
        audio_type: None,
        duration_ms: None,
        published_at: None,
        artwork: None,
        number: None,
        transcript: None,
    }
}

pub(crate) fn song(title: &str) -> crate::collections::TrackFields {
    crate::collections::TrackFields {
        track_key: format!("artist|{}", title.to_lowercase()),
        title: title.into(),
        artist: Some("Artist".into()),
        album: None,
        duration_ms: None,
        artwork_url: None,
        source: None,
    }
}

/// A comic, a book, a followed podcast, a playlist with a song, a favorite, pins and a setting.
pub(crate) async fn a_bit_of_everything(db: &Db) {
    reader_state(db).await;
    let mut novel = manga("n1", "A Book");
    novel.kind = "novel".into();
    let novel = titles::upsert_title(db, &novel, 1).await.unwrap();
    titles::set_in_library(db, &novel, Some(2)).await.unwrap();
    let feed = show("https://feed.example/a", "podcast");
    let podcast = crate::podcasts::upsert_podcast(db, &feed, 1).await.unwrap();
    crate::podcasts::set_subscribed(db, &podcast, Some(3))
        .await
        .unwrap();
    crate::podcasts::sync_episodes(db, &podcast, &[episode("e1"), episode("e2")], 4)
        .await
        .unwrap();
    let playlist = crate::collections::create_playlist(db, "Mix", 5)
        .await
        .unwrap();
    crate::collections::add_to_playlist(db, &playlist, &[song("Song")], 6)
        .await
        .unwrap();
    crate::collections::set_favorite(db, &song("Other"), true, 7)
        .await
        .unwrap();
    crate::settings::put(db, "theme", "\"dark\"").await.unwrap();
    crate::settings::put(
        db,
        PINS_KEY,
        r#"{"pins":[{"kind":"playlist"},{"kind":"album"}],"played":[{"to":"/a"},{"to":"/b"},{"to":"/c"}]}"#,
    )
    .await
    .unwrap();
    crate::local_folders::add(
        db,
        crate::local_folders::NewFolder {
            id: "f1",
            location: "/storage/emulated/0/Music",
            name: "Music",
            path: Some("/storage/emulated/0/Music"),
            access: crate::local_folders::FolderAccess::Path,
        },
        8,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn parts_choose_what_a_backup_holds_and_what_a_restore_takes() {
    let temp = TempDir::new().unwrap();
    let source = open(&temp, "a.db").await;
    a_bit_of_everything(&source).await;

    let backup = export(&source, 9, &[Part::Comics, Part::Pins])
        .await
        .unwrap();
    assert_eq!(parts_in(&backup), [Part::Comics, Part::Pins]);
    let counted = index(&backup);
    assert_eq!(counted[&Part::Comics][&Item::Titles], 1);
    assert_eq!(counted[&Part::Comics][&Item::Chapters], 2);
    assert_eq!(counted[&Part::Comics][&Item::Read], 1);
    assert_eq!(counted[&Part::Pins][&Item::Pins], 2);
    assert!(!counted.contains_key(&Part::Books) && !counted.contains_key(&Part::Settings));
    assert_eq!(backup.live_rows("setting"), 1, "only the pins' setting");

    // A restore takes only what it's asked for, even when the backup holds more.
    let everything = export(&source, 9, &Part::ALL).await.unwrap();
    let target = open(&temp, "b.db").await;
    restore(&target, &everything, &[Part::Pins, Part::Podcasts])
        .await
        .unwrap();
    let settings = crate::settings::load(&target).await.unwrap();
    assert_eq!(settings.len(), 1);
    assert_eq!(settings[0].0, PINS_KEY);
    assert!(titles::library(&target, "manga").await.unwrap().is_empty());
    assert_eq!(
        crate::podcasts::subscriptions(&target).await.unwrap().len(),
        1
    );
    assert!(
        crate::collections::playlists(&target)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn the_index_counts_a_backup_as_the_database_is_counted() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp, "a.db").await;
    a_bit_of_everything(&db).await;
    let backup = export(&db, 9, &Part::ALL).await.unwrap();
    let counted = crate::data::counts(&db).await.unwrap();
    assert_eq!(index(&backup), counted);
    assert_eq!(counted[&Part::Podcasts][&Item::Shows], 1);
    assert_eq!(counted[&Part::Podcasts][&Item::Episodes], 2);
    assert_eq!(counted[&Part::Playlists][&Item::PlaylistSongs], 1);
    assert_eq!(counted[&Part::Playlists][&Item::Favorites], 1);
    assert_eq!(counted[&Part::Books][&Item::Titles], 1);
    assert_eq!(counted[&Part::Settings][&Item::Settings], 1);
    assert_eq!(counted[&Part::Pins][&Item::Played], 3);
    assert_eq!(counted[&Part::LocalFiles][&Item::Folders], 1);
    // A folder keeps how it's read.
    let folder = backup.rows("local_folder").next().expect("the folder");
    assert_eq!(folder["access"], "path");
    assert_eq!(folder["location"], "/storage/emulated/0/Music");
}

#[tokio::test]
async fn a_podcast_followed_on_both_sides_keeps_our_id_and_its_episodes_follow() {
    let temp = TempDir::new().unwrap();
    let feed = show("https://feed.example/a", "podcast");
    let source = open(&temp, "a.db").await;
    let theirs = crate::podcasts::upsert_podcast(&source, &feed, 1)
        .await
        .unwrap();
    crate::podcasts::sync_episodes(&source, &theirs, &[episode("e1")], 2)
        .await
        .unwrap();
    let backup = export(&source, 9, &Part::ALL).await.unwrap();

    let target = open(&temp, "b.db").await;
    let ours = crate::podcasts::upsert_podcast(&target, &feed, 3)
        .await
        .unwrap();
    crate::podcasts::sync_episodes(&target, &ours, &[episode("e1")], 4)
        .await
        .unwrap();
    let restored = restore(&target, &backup, &[Part::Podcasts]).await.unwrap();
    assert_eq!(restored.ours(&theirs), ours);
    assert!(
        crate::podcasts::podcast(&target, &theirs)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        crate::podcasts::episodes(&target, &ours)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn unknown_tables_are_ignored() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp, "a.db").await;
    let mut backup = export(&db, 1, &Part::ALL).await.unwrap();
    backup.tables.insert(
        "from_the_future".into(),
        vec![serde_json::json!({"id": "x"})],
    );
    restore(&db, &backup, &Part::ALL).await.unwrap();
}
