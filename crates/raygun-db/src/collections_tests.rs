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
