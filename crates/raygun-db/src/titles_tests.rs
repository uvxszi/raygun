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
