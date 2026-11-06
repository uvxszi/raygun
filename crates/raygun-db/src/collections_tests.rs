use tempfile::TempDir;

use super::*;

async fn open(temp: &TempDir) -> Db {
    Db::open(&temp.path().join("raygun.db")).await.unwrap()
}

fn track(key: &str) -> TrackFields {
    TrackFields {
        track_key: key.into(),
        title: key.rsplit('|').next().unwrap().into(),
        artist: Some(key.split('|').next().unwrap().into()),
        album: None,
        duration_ms: Some(180_000),
        artwork_url: None,
        source: None,
    }
}

fn titles(items: &[PlaylistItemRow]) -> Vec<&str> {
    items.iter().map(|item| item.track.title.as_str()).collect()
}

#[tokio::test]
async fn favorites_toggle_and_revive_the_same_row() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let song = track("nina simone|feeling good");

    set_favorite(&db, &song, true, 10).await.unwrap();
    set_favorite(&db, &song, true, 20).await.unwrap();
    let first = favorites(&db).await.unwrap();
    assert_eq!(first.len(), 1, "favoriting twice keeps one row");
    assert_eq!(first[0].added_at, 10);

    set_favorite(&db, &song, false, 30).await.unwrap();
    assert!(favorites(&db).await.unwrap().is_empty());
    assert!(!is_favorite(&db, &song.track_key).await.unwrap());

    set_favorite(&db, &song, true, 40).await.unwrap();
    let revived = favorites(&db).await.unwrap();
    assert_eq!(revived[0].id, first[0].id, "the tombstoned row comes back");
    assert_eq!(revived[0].added_at, 40);

    set_favorite(&db, &track("a|older"), true, 5).await.unwrap();
    let keys: Vec<_> = favorites(&db)
        .await
        .unwrap()
        .into_iter()
        .map(|favorite| favorite.track.track_key)
        .collect();
    assert_eq!(keys, ["nina simone|feeling good", "a|older"]);
}

#[tokio::test]
async fn playlists_append_move_and_remove_items() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let id = create_playlist(&db, "Road trip", 1).await.unwrap();
    add_to_playlist(&db, &id, &[track("a|one"), track("a|two")], 2)
        .await
        .unwrap();
    add_to_playlist(&db, &id, &[track("a|three")], 3)
        .await
        .unwrap();
    let items = playlist_items(&db, &id).await.unwrap();
    assert_eq!(titles(&items), ["one", "two", "three"]);

    // three before one, then one to the end.
    move_playlist_item(&db, &items[2].id, Some(&items[0].id))
        .await
        .unwrap();
    assert_eq!(
        titles(&playlist_items(&db, &id).await.unwrap()),
        ["three", "one", "two"]
    );
    move_playlist_item(&db, &items[0].id, None).await.unwrap();
    assert_eq!(
        titles(&playlist_items(&db, &id).await.unwrap()),
        ["three", "two", "one"]
    );

    remove_playlist_item(&db, &items[1].id).await.unwrap();
    assert_eq!(
        titles(&playlist_items(&db, &id).await.unwrap()),
        ["three", "one"]
    );

    let listed = playlist(&db, &id).await.unwrap().unwrap();
    assert_eq!(listed.name, "Road trip");
    assert_eq!(listed.item_count, 2);
    // The live items' lengths, summed.
    assert_eq!((listed.known_ms, listed.known_count), (360_000, 2));
    assert_eq!(listed.first_items.len(), 2);
}

#[tokio::test]
async fn moves_renumber_when_positions_run_out_of_room() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let id = create_playlist(&db, "Tight", 1).await.unwrap();
    add_to_playlist(&db, &id, &[track("a|x"), track("a|y"), track("a|z")], 2)
        .await
        .unwrap();
    let items = playlist_items(&db, &id).await.unwrap();
    // Halving the gap between x and y each time runs out of room after ~20 moves.
    for round in 0..40 {
        let moving = if round % 2 == 0 { &items[2] } else { &items[1] };
        let current = playlist_items(&db, &id).await.unwrap();
        move_playlist_item(&db, &moving.id, Some(&current[1].id))
            .await
            .unwrap();
    }
    let after = playlist_items(&db, &id).await.unwrap();
    assert_eq!(after.len(), 3);
    assert_eq!(after[0].track.title, "x", "x never moved");
    let positions: Vec<f64> = after.iter().map(|item| item.position).collect();
    // 40 halvings alone would leave gaps near 1e-12; renumbering keeps them far wider.
    assert!(positions.windows(2).all(|pair| pair[1] - pair[0] > 1e-9));
}

#[tokio::test]
async fn renaming_and_deleting_playlists() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let keep = create_playlist(&db, "b side", 1).await.unwrap();
    let gone = create_playlist(&db, "A side", 2).await.unwrap();
    add_to_playlist(&db, &gone, &[track("a|one")], 3)
        .await
        .unwrap();

    let names: Vec<_> = playlists(&db)
        .await
        .unwrap()
        .into_iter()
        .map(|playlist| playlist.name)
        .collect();
    assert_eq!(names, ["A side", "b side"], "sorted without case");

    assert!(rename_playlist(&db, &keep, "Keepers").await.unwrap());
    delete_playlist(&db, &gone).await.unwrap();
    assert!(!rename_playlist(&db, &gone, "Ghost").await.unwrap());
    let left = playlists(&db).await.unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].name, "Keepers");
    assert!(playlist_items(&db, &gone).await.unwrap().is_empty());
    assert!(playlist(&db, &gone).await.unwrap().is_none());
}

#[tokio::test]
async fn history_lists_each_track_once_by_its_latest_play() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    record_play(&db, &track("a|one"), 100).await.unwrap();
    record_play(&db, &track("a|two"), 200).await.unwrap();
    record_play(&db, &track("a|one"), 300).await.unwrap();

    let recent = recent_history(&db, 10).await.unwrap();
    let listed: Vec<_> = recent
        .iter()
        .map(|row| (row.track.title.as_str(), row.played_at))
        .collect();
    assert_eq!(listed, [("one", 300), ("two", 200)]);
    assert_eq!(recent_history(&db, 1).await.unwrap().len(), 1);

    // Every play, for Made for You (M46.20): newest first, from a time, at most a number.
    let plays: Vec<i64> = plays_since(&db, 150, 10)
        .await
        .unwrap()
        .iter()
        .map(|row| row.played_at)
        .collect();
    assert_eq!(plays, [300, 200]);
    assert_eq!(plays_since(&db, 0, 2).await.unwrap().len(), 2);

    clear_history(&db).await.unwrap();
    assert!(recent_history(&db, 10).await.unwrap().is_empty());
}

#[tokio::test]
async fn writes_are_stamped_for_sync() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    set_favorite(&db, &track("a|one"), true, 1).await.unwrap();
    let first: (i64, String) =
        sqlx::query_as("SELECT updated_at_hlc, origin_device FROM music_favorite")
            .fetch_one(db.pool())
            .await
            .unwrap();
    set_favorite(&db, &track("a|one"), false, 2).await.unwrap();
    let second: (i64, i64) = sqlx::query_as("SELECT updated_at_hlc, deleted FROM music_favorite")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(first.1, db.device_id().to_string());
    assert!(second.0 > first.0);
    assert_eq!(second.1, 1, "unfavoriting leaves a tombstone");
}

#[tokio::test]
async fn songs_know_their_playlists_and_play_counts() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let song = track("daft punk|get lucky");
    let other = track("daft punk|one more time");
    let road = create_playlist(&db, "Road", 1).await.unwrap();
    let gym = create_playlist(&db, "Gym", 2).await.unwrap();
    add_to_playlist(&db, &road, &[song.clone(), other.clone(), song.clone()], 3)
        .await
        .unwrap();
    add_to_playlist(&db, &gym, std::slice::from_ref(&song), 4)
        .await
        .unwrap();

    let mut holding = playlists_with_track(&db, &song.track_key).await.unwrap();
    holding.sort();
    let mut both = vec![road.clone(), gym.clone()];
    both.sort();
    assert_eq!(holding, both);

    // Taking a song out of a playlist takes every copy of it.
    remove_track_from_playlist(&db, &road, &song.track_key)
        .await
        .unwrap();
    assert_eq!(
        titles(&playlist_items(&db, &road).await.unwrap()),
        ["one more time"]
    );
    assert_eq!(
        playlists_with_track(&db, &song.track_key).await.unwrap(),
        std::slice::from_ref(&gym)
    );
    delete_playlist(&db, &gym).await.unwrap();
    assert!(
        playlists_with_track(&db, &song.track_key)
            .await
            .unwrap()
            .is_empty()
    );

    record_play(&db, &song, 10).await.unwrap();
    record_play(&db, &song, 20).await.unwrap();
    record_play(&db, &other, 30).await.unwrap();
    let counts = play_counts(&db, &[song.track_key.clone(), "never|played".into()])
        .await
        .unwrap();
    assert_eq!(counts.get(&song.track_key), Some(&2));
    assert_eq!(counts.get("never|played"), None);
}

#[tokio::test]
async fn a_found_source_fills_every_saved_copy_and_clears_the_mark() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let id = create_playlist(&db, "Imported", 1).await.unwrap();
    let mut song = track("a|lost");
    song.source = None;
    song.album = None;
    add_to_playlist(&db, &id, &[song.clone()], 2).await.unwrap();
    mark_unavailable(&db, &["a|lost".to_owned(), "b|other".to_owned()], 3)
        .await
        .unwrap();
    let found = TrackFields {
        album: Some("Found Album".into()),
        source: Some(r#"{"kind":"url","url":"https://x.example/a.mp3","headers":{}}"#.into()),
        ..song
    };
    assert_eq!(
        set_track_source(&db, "a|lost", &found, false)
            .await
            .unwrap(),
        1
    );
    clear_unavailable(&db, "a|lost").await.unwrap();
    let items = playlist_items(&db, &id).await.unwrap();
    assert_eq!(items[0].track.album.as_deref(), Some("Found Album"));
    assert!(items[0].track.source.is_some());
    assert_eq!(
        unavailable(&db, &["a|lost".to_owned(), "b|other".to_owned()])
            .await
            .unwrap(),
        vec!["b|other".to_owned()]
    );
}

fn favorite_titles(rows: &[FavoriteRow]) -> Vec<&str> {
    rows.iter().map(|row| row.track.title.as_str()).collect()
}

#[tokio::test]
async fn favorites_keep_the_order_they_are_dragged_into() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    for (at, key) in [(1, "x|a"), (2, "x|b"), (3, "x|c")] {
        set_favorite(&db, &track(key), true, at).await.unwrap();
    }
    assert_eq!(
        favorite_titles(&favorites(&db).await.unwrap()),
        ["c", "b", "a"]
    );

    // One song moved: only it gets a position.
    move_favorite(&db, "x|a", Some("x|c")).await.unwrap();
    assert_eq!(
        favorite_titles(&favorites(&db).await.unwrap()),
        ["a", "c", "b"]
    );
    move_favorite(&db, "x|c", None).await.unwrap();
    assert_eq!(
        favorite_titles(&favorites(&db).await.unwrap()),
        ["a", "b", "c"]
    );

    // A whole order, from a sorted table; unknown keys are ignored, missing ones follow.
    set_favorites_order(&db, &["x|b".into(), "x|gone".into(), "x|c".into()])
        .await
        .unwrap();
    assert_eq!(
        favorite_titles(&favorites(&db).await.unwrap()),
        ["b", "c", "a"]
    );

    // New favorites still arrive on top, and a revived one starts there again.
    set_favorite(&db, &track("x|d"), true, 4).await.unwrap();
    set_favorite(&db, &track("x|c"), false, 5).await.unwrap();
    set_favorite(&db, &track("x|c"), true, 6).await.unwrap();
    assert_eq!(
        favorite_titles(&favorites(&db).await.unwrap()),
        ["c", "d", "b", "a"]
    );
}

#[tokio::test]
async fn saved_copies_are_listed_with_their_names_and_places() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let road = create_playlist(&db, "Road", 1).await.unwrap();
    let gone = create_playlist(&db, "Gone", 2).await.unwrap();
    add_to_playlist(&db, &road, &[track("a|one")], 30)
        .await
        .unwrap();
    add_to_playlist(&db, &gone, &[track("a|two")], 20)
        .await
        .unwrap();
    set_favorite(&db, &track("a|three"), true, 10)
        .await
        .unwrap();
    delete_playlist(&db, &gone).await.unwrap();

    let refs = saved_refs(&db).await.unwrap();
    let listed: Vec<(&str, Option<&str>, i64)> = refs
        .iter()
        .map(|row| (row.title.as_str(), row.playlist_id.as_deref(), row.added_at))
        .collect();
    // Oldest first; a deleted playlist's items are gone with it.
    assert_eq!(
        listed,
        [("three", None, 10), ("one", Some(road.as_str()), 30)]
    );
    assert!(refs[0].is_favorite());
    assert_eq!(refs[1].artist.as_deref(), Some("a"));

    // Tombstoned by id, both kinds in one write.
    remove_saved(&db, &[refs[0].id.clone()], &[refs[1].id.clone()])
        .await
        .unwrap();
    assert!(saved_refs(&db).await.unwrap().is_empty());
    assert!(favorites(&db).await.unwrap().is_empty());
    let dead: i64 = sqlx::query_scalar("SELECT deleted FROM music_playlist_item WHERE id = ?1")
        .bind(&refs[1].id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(dead, 1, "removing leaves a tombstone for sync");
}

#[tokio::test]
async fn sources_and_keys_are_set_by_row() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let id = create_playlist(&db, "Imported", 1).await.unwrap();
    add_to_playlist(&db, &id, &[track("a b|song"), track("a|other")], 2)
        .await
        .unwrap();
    set_favorite(&db, &track("a|song"), true, 3).await.unwrap();
    let refs = saved_refs(&db).await.unwrap();
    let favorite = refs.iter().find(|row| row.is_favorite()).unwrap();
    let item = refs.iter().find(|row| row.track_key == "a b|song").unwrap();
    let found = TrackFields {
        album: Some("Found".into()),
        source: Some(r#"{"kind":"url","url":"https://x.example/a.mp3","headers":{}}"#.into()),
        ..track("a|song")
    };
    let changed = set_track_source_of(
        &db,
        std::slice::from_ref(&favorite.id),
        std::slice::from_ref(&item.id),
        &found,
        true,
    )
    .await
    .unwrap();
    assert_eq!(changed, 2, "both copies, whatever their keys");
    let items = playlist_items(&db, &id).await.unwrap();
    assert_eq!(items[0].track.album.as_deref(), Some("Found"));
    assert!(items[1].track.source.is_none(), "other songs stay");
    assert!(favorites(&db).await.unwrap()[0].track.source.is_some());

    set_keys(&db, &[], &[(item.id.clone(), "a|song".into())])
        .await
        .unwrap();
    assert_eq!(
        playlists_with_track(&db, "a|song").await.unwrap(),
        vec![id.clone()]
    );
}

#[tokio::test]
async fn the_repair_migration_keeps_the_first_copy_of_a_song() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let road = create_playlist(&db, "Road", 1).await.unwrap();
    let gym = create_playlist(&db, "Gym", 2).await.unwrap();
    // The bug: the same song added again, to the same playlist.
    add_to_playlist(&db, &road, &[track("a|one"), track("a|two")], 10)
        .await
        .unwrap();
    add_to_playlist(&db, &road, &[track("a|one")], 20)
        .await
        .unwrap();
    add_to_playlist(&db, &road, &[track("a|one")], 30)
        .await
        .unwrap();
    add_to_playlist(&db, &gym, &[track("a|one")], 40)
        .await
        .unwrap();
    let before: i64 = sqlx::query_scalar("SELECT MAX(updated_at_hlc) FROM music_playlist_item")
        .fetch_one(db.pool())
        .await
        .unwrap();

    sqlx::raw_sql(include_str!("../migrations/0031_song_identity.sql"))
        .execute(db.pool())
        .await
        .unwrap();

    let items = playlist_items(&db, &road).await.unwrap();
    assert_eq!(titles(&items), ["one", "two"], "the first copy stays");
    assert_eq!(items[0].added_at, 10);
    assert_eq!(
        titles(&playlist_items(&db, &gym).await.unwrap()),
        ["one"],
        "another playlist's copy isn't a duplicate"
    );
    let stamps: Vec<(i64, String)> = sqlx::query_as(
        "SELECT updated_at_hlc, origin_device FROM music_playlist_item WHERE deleted = 1",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(stamps.len(), 2);
    for (hlc, device) in &stamps {
        assert!(*hlc > before, "a tombstone is a newer write");
        assert_eq!(device, &db.device_id().to_string());
    }
    // The clock goes on past the repair's stamps, after a restart too.
    let tombstone = stamps[0].0;
    db.close().await;
    let db = open(&temp).await;
    set_favorite(&db, &track("a|new"), true, 50).await.unwrap();
    let next: i64 = sqlx::query_scalar("SELECT updated_at_hlc FROM music_favorite")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert!(next > tombstone);
}

#[tokio::test]
async fn the_artist_migration_drops_quotes_left_around_saved_names() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let saved = |artist: &str, title: &str| TrackFields {
        artist: Some(artist.into()),
        ..track(&format!("{}|{title}", artist.to_lowercase()))
    };
    for (at, (artist, title)) in [
        ("\"System of a Down\"", "chop suey"),
        ("\"Weird Al\" Yankovic", "white & nerdy"),
        ("Guns N' Roses", "patience"),
        ("\"", "untitled"),
    ]
    .into_iter()
    .enumerate()
    {
        set_favorite(&db, &saved(artist, title), true, at as i64)
            .await
            .unwrap();
    }
    let road = create_playlist(&db, "Road", 1).await.unwrap();
    add_to_playlist(
        &db,
        &road,
        &[
            saved("“Daft Punk”", "one more time"),
            saved(" «Kino»", "gruppa krovi"),
            saved("Nirvana\" ", "lithium"),
            saved("'Til Tuesday", "voices carry"),
        ],
        20,
    )
    .await
    .unwrap();
    crate::music_downloads::enqueue(
        &db,
        &crate::music_downloads::NewSong {
            track_key: "a|swim".into(),
            title: "Swim".into(),
            artist: Some("\"BTS".into()),
            album: None,
            duration_ms: None,
            artwork_url: None,
            source: None,
        },
        30,
    )
    .await
    .unwrap();
    async fn stamp_of(db: &Db, title: &str) -> i64 {
        sqlx::query_scalar("SELECT updated_at_hlc FROM music_favorite WHERE title = ?")
            .bind(title)
            .fetch_one(db.pool())
            .await
            .unwrap()
    }
    let before = sqlx::query_scalar::<_, i64>(
        "SELECT MAX(updated_at_hlc) FROM (SELECT updated_at_hlc FROM music_favorite
         UNION ALL SELECT updated_at_hlc FROM music_playlist_item)",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let kept = stamp_of(&db, "white & nerdy").await;

    sqlx::raw_sql(include_str!("../migrations/0040_tidy_saved_artists.sql"))
        .execute(db.pool())
        .await
        .unwrap();

    let mut favorites: Vec<(String, Option<String>)> = favorites(&db)
        .await
        .unwrap()
        .into_iter()
        .map(|row| (row.track.title, row.track.artist))
        .collect();
    favorites.sort();
    assert_eq!(
        favorites,
        [
            ("chop suey".to_owned(), Some("System of a Down".to_owned())),
            ("patience".to_owned(), Some("Guns N' Roses".to_owned())),
            ("untitled".to_owned(), None),
            (
                "white & nerdy".to_owned(),
                Some("\"Weird Al\" Yankovic".to_owned())
            ),
        ]
    );
    let items: Vec<Option<String>> = playlist_items(&db, &road)
        .await
        .unwrap()
        .into_iter()
        .map(|item| item.track.artist)
        .collect();
    assert_eq!(
        items,
        [
            Some("Daft Punk".to_owned()),
            Some("Kino".to_owned()),
            Some("Nirvana".to_owned()),
            Some("'Til Tuesday".to_owned()),
        ]
    );
    let download = crate::music_downloads::by_key(&db, "a|swim")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(download.song.artist.as_deref(), Some("BTS"));

    // The tidied rows are this device's newer writes; the others keep their stamps.
    assert!(stamp_of(&db, "chop suey").await > before);
    assert_eq!(stamp_of(&db, "white & nerdy").await, kept);
    let device: String =
        sqlx::query_scalar("SELECT origin_device FROM music_favorite WHERE title = ?")
            .bind("chop suey")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(device, db.device_id().to_string());
    let tidied = stamp_of(&db, "chop suey").await;
    db.close().await;
    let db = open(&temp).await;
    set_favorite(&db, &track("a|new"), true, 50).await.unwrap();
    let next: i64 =
        sqlx::query_scalar("SELECT updated_at_hlc FROM music_favorite WHERE title = 'new'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(
        next > tidied,
        "the clock goes on past the migration's stamps"
    );
}

#[tokio::test]
async fn playlists_bring_their_first_items_for_covers() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let long = create_playlist(&db, "Long", 1).await.unwrap();
    let short = create_playlist(&db, "Short", 2).await.unwrap();
    let many: Vec<TrackFields> = (0..20).map(|n| track(&format!("a|song {n:02}"))).collect();
    add_to_playlist(&db, &long, &many, 3).await.unwrap();
    add_to_playlist(&db, &short, &[track("b|only")], 4)
        .await
        .unwrap();
    let items = playlist_items(&db, &long).await.unwrap();
    // The last song moved first: covers follow the playlist's order.
    move_playlist_item(&db, &items[19].id, Some(&items[0].id))
        .await
        .unwrap();

    let listed = playlists(&db).await.unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].item_count, 20);
    assert_eq!(listed[0].first_items.len(), COVER_ITEMS as usize);
    assert_eq!(listed[0].first_items[0].title, "song 19");
    assert_eq!(listed[0].first_items[1].title, "song 00");
    assert_eq!(listed[1].first_items.len(), 1);

    let one = playlist(&db, &short).await.unwrap().unwrap();
    assert_eq!((one.name.as_str(), one.item_count), ("Short", 1));
    assert_eq!(one.first_items[0].title, "only");
    delete_playlist(&db, &short).await.unwrap();
    assert!(playlist(&db, &short).await.unwrap().is_none());
}

#[tokio::test]
async fn a_playlist_takes_a_whole_new_order() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let id = create_playlist(&db, "Mix", 1).await.unwrap();
    add_to_playlist(&db, &id, &[track("x|a"), track("x|b"), track("x|c")], 2)
        .await
        .unwrap();
    let items = playlist_items(&db, &id).await.unwrap();
    let ids: Vec<String> = items.iter().map(|item| item.id.clone()).collect();
    set_playlist_order(&db, &id, &[ids[2].clone(), ids[0].clone()])
        .await
        .unwrap();
    assert_eq!(
        titles(&playlist_items(&db, &id).await.unwrap()),
        ["c", "a", "b"]
    );
}

#[tokio::test]
async fn imported_songs_are_marked() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    mark_imported(&db, &["a|one".to_owned(), "a|two".to_owned()], 1)
        .await
        .unwrap();
    mark_imported(&db, &["a|one".to_owned()], 2).await.unwrap();
    let found = imported(&db, &["a|one".to_owned(), "a|three".to_owned()])
        .await
        .unwrap();
    assert_eq!(found, ["a|one"]);
}
