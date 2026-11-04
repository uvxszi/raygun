use tempfile::TempDir;

use super::*;

async fn open(temp: &TempDir) -> Db {
    Db::open(&temp.path().join("raygun.db")).await.unwrap()
}

fn manga(remote_id: &str, title: &str) -> TitleFields {
    TitleFields {
        kind: "manga".into(),
        source_id: "dev.raygun.mangadex/mangadex".into(),
        remote_id: remote_id.into(),
        title: title.into(),
        cover: Some("https://img/cover.jpg".into()),
        cover_headers: [("User-Agent".to_owned(), "Raygun".to_owned())].into(),
        authors: vec!["Yamada".into()],
        artists: vec![],
        description: Some("An elf mage".into()),
        genres: vec!["Fantasy".into(), "Adventure".into()],
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
        scanlator: Some("Group".into()),
        date: Some(1_700_000_000_000),
    }
}

#[tokio::test]
async fn refreshing_a_title_keeps_the_users_choices() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let id = upsert_title(&db, &manga("m1", "Frieren"), 1).await.unwrap();
    set_in_library(&db, &id, Some(5)).await.unwrap();

    let mut renamed = manga("m1", "Frieren: Beyond Journey's End");
    renamed.status = "completed".into();
    let again = upsert_title(&db, &renamed, 9).await.unwrap();
    assert_eq!(again, id, "the same source title keeps its row");

    let stored = title(&db, &id).await.unwrap().unwrap();
    assert_eq!(stored.fields.title, "Frieren: Beyond Journey's End");
    assert_eq!(stored.fields.status, "completed");
    assert_eq!(stored.fields.genres, ["Fantasy", "Adventure"]);
    assert_eq!(stored.fields.cover_headers["User-Agent"], "Raygun");
    assert_eq!(
        stored.in_library_at,
        Some(5),
        "refresh never touches user_* columns"
    );
    assert_eq!(
        stored.reader_mode, "",
        "the reader's default until one is picked"
    );
    set_reader_mode(&db, &id, "pagedRtl").await.unwrap();
    upsert_title(&db, &renamed, 10).await.unwrap();
    assert_eq!(
        title(&db, &id).await.unwrap().unwrap().reader_mode,
        "pagedRtl"
    );
    assert_eq!(stored.refreshed_at, Some(9));
    assert_eq!(
        find_title(&db, "dev.raygun.mangadex/mangadex", "m1")
            .await
            .unwrap()
            .unwrap()
            .id,
        id
    );
}

#[tokio::test]
async fn units_keep_progress_across_refreshes() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let id = upsert_title(&db, &manga("m1", "Frieren"), 1).await.unwrap();
    sync_units(
        &db,
        &id,
        &[chapter("c2", 2.0), chapter("c1", 1.0)],
        "chapter",
        1,
    )
    .await
    .unwrap();
    let first = units(&db, &id).await.unwrap();
    assert_eq!(
        first[0].fields.remote_id, "c2",
        "kept in the source's order"
    );
    let c1 = first[1].id.clone();

    set_progress(&db, &c1, 3, Some(20), 10).await.unwrap();
    assert_eq!(unit(&db, &c1).await.unwrap().unwrap().position, Some(3));
    assert_eq!(unit(&db, &c1).await.unwrap().unwrap().completed_at, None);
    set_progress(&db, &c1, 19, Some(20), 11).await.unwrap();
    set_progress(&db, &c1, 2, None, 12).await.unwrap();
    let read = unit(&db, &c1).await.unwrap().unwrap();
    assert_eq!(
        read.completed_at,
        Some(11),
        "going back doesn't un-read a chapter"
    );
    assert_eq!(read.total, Some(20));

    // A refresh adds c3, drops c2 from the source's list, and keeps c1's progress.
    sync_units(
        &db,
        &id,
        &[chapter("c3", 3.0), chapter("c1", 1.0)],
        "chapter",
        1,
    )
    .await
    .unwrap();
    let after = units(&db, &id).await.unwrap();
    let names: Vec<_> = after
        .iter()
        .map(|unit| unit.fields.remote_id.as_str())
        .collect();
    assert_eq!(
        names,
        ["c3", "c1", "c2"],
        "the vanished chapter keeps its row, last"
    );
    assert_eq!(after[1].completed_at, Some(11));
}

#[tokio::test]
async fn the_library_counts_unread_chapters() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let frieren = upsert_title(&db, &manga("m1", "Frieren"), 1).await.unwrap();
    let berserk = upsert_title(&db, &manga("m2", "berserk"), 1).await.unwrap();
    upsert_title(&db, &manga("m3", "Not added"), 1)
        .await
        .unwrap();
    set_in_library(&db, &frieren, Some(1)).await.unwrap();
    set_in_library(&db, &berserk, Some(2)).await.unwrap();
    sync_units(
        &db,
        &frieren,
        &[chapter("c2", 2.0), chapter("c1", 1.0)],
        "chapter",
        1,
    )
    .await
    .unwrap();
    let c1 = units(&db, &frieren).await.unwrap()[1].id.clone();
    set_completed(&db, &c1, true, 5).await.unwrap();

    let shelf = library(&db, "manga").await.unwrap();
    let titles: Vec<_> = shelf
        .iter()
        .map(|entry| entry.title.fields.title.as_str())
        .collect();
    assert_eq!(titles, ["berserk", "Frieren"], "alphabetical without case");
    assert_eq!((shelf[1].units, shelf[1].unread), (2, 1));

    let ids = in_library(
        &db,
        "dev.raygun.mangadex/mangadex",
        &["m1".into(), "m3".into()],
    )
    .await
    .unwrap();
    assert_eq!(ids, HashSet::from(["m1".to_owned()]));

    set_completed(&db, &c1, false, 6).await.unwrap();
    assert_eq!(library(&db, "manga").await.unwrap()[1].unread, 2);
    set_in_library(&db, &berserk, None).await.unwrap();
    assert_eq!(library(&db, "manga").await.unwrap().len(), 1);
}

#[tokio::test]
async fn chapters_are_marked_together() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let id = upsert_title(&db, &manga("m1", "Frieren"), 1).await.unwrap();
    set_in_library(&db, &id, Some(1)).await.unwrap();
    sync_units(
        &db,
        &id,
        &[chapter("c3", 3.0), chapter("c2", 2.0), chapter("c1", 1.0)],
        "chapter",
        1,
    )
    .await
    .unwrap();
    let ids: Vec<String> = units(&db, &id)
        .await
        .unwrap()
        .into_iter()
        .map(|unit| unit.id)
        .collect();
    set_progress(&db, &ids[0], 4, Some(20), 2).await.unwrap();
    set_units_completed(&db, &ids[..2], true, 7).await.unwrap();
    assert_eq!(library(&db, "manga").await.unwrap()[0].unread, 1);
    let marked = units(&db, &id).await.unwrap();
    assert_eq!(marked[0].completed_at, Some(7));
    assert_eq!(marked[0].position, Some(4), "marking read keeps the page");

    set_units_completed(&db, &ids, false, 8).await.unwrap();
    let cleared = units(&db, &id).await.unwrap();
    assert!(cleared.iter().all(|unit| unit.completed_at.is_none()));
    assert_eq!(cleared[0].position, Some(0), "marking unread starts over");
    set_units_completed(&db, &[], true, 9).await.unwrap();
}

#[tokio::test]
async fn the_feed_lists_chapters_found_after_joining_the_library() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let id = upsert_title(&db, &manga("m1", "Frieren"), 1).await.unwrap();
    // Browsed at 10 (two chapters), added at 20, updated at 30 and 40.
    let first = sync_units(
        &db,
        &id,
        &[chapter("c2", 2.0), chapter("c1", 1.0)],
        "chapter",
        10,
    )
    .await
    .unwrap();
    assert_eq!(first, 2);
    set_in_library(&db, &id, Some(20)).await.unwrap();
    assert!(updates(&db, "manga", 50).await.unwrap().is_empty());

    let found = sync_units(
        &db,
        &id,
        &[chapter("c3", 3.0), chapter("c2", 2.0), chapter("c1", 1.0)],
        "chapter",
        30,
    )
    .await
    .unwrap();
    assert_eq!(found, 1, "only c3 is new");
    sync_units(
        &db,
        &id,
        &[
            chapter("c4", 4.0),
            chapter("c3", 3.0),
            chapter("c2", 2.0),
            chapter("c1", 1.0),
        ],
        "chapter",
        40,
    )
    .await
    .unwrap();
    let c3 = units(&db, &id).await.unwrap()[1].id.clone();
    set_completed(&db, &c3, true, 41).await.unwrap();

    let feed = updates(&db, "manga", 50).await.unwrap();
    let rows: Vec<_> = feed
        .iter()
        .map(|row| {
            (
                row.unit.fields.remote_id.as_str(),
                row.found_at,
                row.unit.completed_at,
            )
        })
        .collect();
    assert_eq!(
        rows,
        [("c4", 40, None), ("c3", 30, Some(41))],
        "newest first, with read state"
    );
    assert_eq!(feed[0].title.fields.title, "Frieren");
    assert_eq!(updates(&db, "manga", 1).await.unwrap().len(), 1);

    set_in_library(&db, &id, None).await.unwrap();
    assert!(
        updates(&db, "manga", 50).await.unwrap().is_empty(),
        "library titles only"
    );
}

fn series(remote_id: &str) -> TitleFields {
    TitleFields {
        kind: "video".into(),
        source_id: "stremio".into(),
        remote_id: remote_id.into(),
        title: "Breaking Bad".into(),
        media_type: Some("series".into()),
        year: Some(2008),
        ..manga(remote_id, "Breaking Bad")
    }
}

fn episode(remote_id: &str, number: f64, date: Option<i64>) -> UnitFields {
    UnitFields {
        remote_id: remote_id.into(),
        name: format!("Episode {number}"),
        number: Some(number),
        volume: Some("1".into()),
        scanlator: None,
        date,
    }
}

#[tokio::test]
async fn episodes_are_found_by_source_and_id_and_count_as_watched() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let id = upsert_title(&db, &series("series:tt1"), 1).await.unwrap();
    sync_units(
        &db,
        &id,
        &[
            episode("tt1:1:1", 1.0, Some(5)),
            episode("tt1:1:2", 2.0, Some(6)),
        ],
        "episode",
        1,
    )
    .await
    .unwrap();
    set_in_library(&db, &id, Some(2)).await.unwrap();
    assert!(
        find_units(&db, "other", "tt1:1:1")
            .await
            .unwrap()
            .is_empty()
    );
    let found = find_units(&db, "stremio", "tt1:1:1").await.unwrap();
    assert_eq!(found.len(), 1);
    let unit_id = found[0].id.clone();

    set_watch_progress(&db, &unit_id, 60_000, Some(2_400_000), false, 3)
        .await
        .unwrap();
    let library_row = &library(&db, "video").await.unwrap()[0];
    assert_eq!(
        (library_row.unread, library_row.started),
        (2, 1),
        "started, not watched"
    );
    let half = unit(&db, &unit_id).await.unwrap().unwrap();
    assert_eq!((half.position, half.total), (Some(60_000), Some(2_400_000)));

    set_watch_progress(&db, &unit_id, 2_300_000, None, true, 4)
        .await
        .unwrap();
    set_watch_progress(&db, &unit_id, 1_000, None, false, 5)
        .await
        .unwrap();
    let watched = unit(&db, &unit_id).await.unwrap().unwrap();
    assert_eq!(
        watched.completed_at,
        Some(4),
        "watching again keeps it watched"
    );
    assert_eq!(library(&db, "video").await.unwrap()[0].unread, 1);
}

#[tokio::test]
async fn upcoming_lists_future_episodes_of_library_series_soonest_first() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let id = upsert_title(&db, &series("series:tt2"), 1).await.unwrap();
    sync_units(
        &db,
        &id,
        &[
            episode("e1", 1.0, Some(100)),
            episode("e3", 3.0, Some(300)),
            episode("e2", 2.0, Some(200)),
            episode("e4", 4.0, None),
        ],
        "episode",
        1,
    )
    .await
    .unwrap();
    assert!(
        upcoming(&db, "video", 150, 10).await.unwrap().is_empty(),
        "library only"
    );
    set_in_library(&db, &id, Some(2)).await.unwrap();
    let soon: Vec<_> = upcoming(&db, "video", 150, 10)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.unit.fields.remote_id)
        .collect();
    assert_eq!(soon, ["e2", "e3"]);
    assert!(upcoming(&db, "manga", 150, 10).await.unwrap().is_empty());

    // An episode announced for a later date (in 2096) isn't unwatched yet.
    assert_eq!(library(&db, "video").await.unwrap()[0].unread, 4);
    sync_units(
        &db,
        &id,
        &[episode("e5", 5.0, Some(4_000_000_000_000))],
        "episode",
        3,
    )
    .await
    .unwrap();
    let entry = &library(&db, "video").await.unwrap()[0];
    assert_eq!((entry.units, entry.unread), (5, 4));
}

#[tokio::test]
async fn lists_favorites_pins_and_bulk_read() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let first = upsert_title(&db, &manga("m1", "Frieren"), 1).await.unwrap();
    let second = upsert_title(&db, &manga("m2", "Dungeon Meshi"), 1)
        .await
        .unwrap();
    let both = [first.clone(), second.clone()];
    set_mark(&db, &both, &TitleMark::List(Some("reading".into())))
        .await
        .unwrap();
    set_mark(
        &db,
        std::slice::from_ref(&first),
        &TitleMark::Favorite(Some(7)),
    )
    .await
    .unwrap();
    set_mark(
        &db,
        std::slice::from_ref(&second),
        &TitleMark::Pinned(Some(8)),
    )
    .await
    .unwrap();
    let stored = title(&db, &first).await.unwrap().unwrap();
    assert_eq!(stored.list.as_deref(), Some("reading"));
    assert_eq!((stored.favorite_at, stored.pinned_at), (Some(7), None));
    set_mark(&db, std::slice::from_ref(&first), &TitleMark::List(None))
        .await
        .unwrap();
    assert_eq!(title(&db, &first).await.unwrap().unwrap().list, None);
    assert_eq!(
        title(&db, &second).await.unwrap().unwrap().pinned_at,
        Some(8)
    );

    // Chapter 3 isn't out yet: marking read leaves it unread.
    let mut future = chapter("c3", 3.0);
    future.date = Some(4_000_000_000_000);
    sync_units(
        &db,
        &first,
        &[chapter("c1", 1.0), chapter("c2", 2.0), future],
        "chapter",
        1,
    )
    .await
    .unwrap();
    set_in_library(&db, &first, Some(2)).await.unwrap();
    set_titles_completed(&db, std::slice::from_ref(&first), true, 1_800_000_000_000)
        .await
        .unwrap();
    let stored_units = units(&db, &first).await.unwrap();
    let read: Vec<&str> = stored_units
        .iter()
        .filter(|unit| unit.completed_at.is_some())
        .map(|unit| unit.fields.remote_id.as_str())
        .collect();
    assert_eq!(read.len(), 2);
    assert!(!read.contains(&"c3"));
    assert_eq!(
        library(&db, "manga").await.unwrap()[0].unread,
        0,
        "what isn't out yet doesn't count as unread"
    );

    set_titles_completed(&db, std::slice::from_ref(&first), false, 1_800_000_000_001)
        .await
        .unwrap();
    assert!(
        units(&db, &first)
            .await
            .unwrap()
            .iter()
            .all(|unit| unit.completed_at.is_none())
    );
}
