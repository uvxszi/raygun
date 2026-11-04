use tempfile::TempDir;

use super::*;

async fn open(temp: &TempDir) -> Db {
    Db::open(&temp.path().join("raygun.db")).await.unwrap()
}

fn show(title: &str) -> PodcastFields {
    PodcastFields {
        feed_url: "https://feeds.example/show.xml".into(),
        title: title.into(),
        author: Some("Host".into()),
        description: Some("A show".into()),
        artwork: Some("https://img.example/show.jpg".into()),
        link: None,
        kind: "podcast".into(),
    }
}

fn item(guid: &str, published_at: i64) -> EpisodeFields {
    EpisodeFields {
        guid: guid.into(),
        title: format!("Episode {guid}"),
        description: None,
        audio_url: format!("https://cdn.example/{guid}.mp3"),
        audio_type: Some("audio/mpeg".into()),
        duration_ms: Some(1_800_000),
        published_at: Some(published_at),
        artwork: None,
        number: None,
        transcript: None,
    }
}

#[tokio::test]
async fn refreshing_keeps_subscription_and_progress() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let id = upsert_podcast(&db, &show("Show"), 1).await.unwrap();
    assert_eq!(
        sync_episodes(&db, &id, &[item("a", 100), item("b", 200)], 1)
            .await
            .unwrap(),
        2
    );
    set_subscribed(&db, &id, Some(5)).await.unwrap();
    let a = episodes(&db, &id).await.unwrap()[1].id.clone();
    set_position(&db, &a, 60_000, 7).await.unwrap();

    // The feed renames the show and one episode, and adds a third.
    let again = upsert_podcast(&db, &show("Show (renamed)"), 2)
        .await
        .unwrap();
    assert_eq!(again, id);
    let mut renamed = item("a", 100);
    renamed.title = "Better title".into();
    let added = sync_episodes(&db, &id, &[renamed, item("b", 200), item("c", 300)], 2)
        .await
        .unwrap();
    assert_eq!(added, 1);

    let row = podcast(&db, &id).await.unwrap().unwrap();
    assert_eq!(row.fields.title, "Show (renamed)");
    assert_eq!(row.subscribed_at, Some(5));
    let list = episodes(&db, &id).await.unwrap();
    assert_eq!(
        list.iter()
            .map(|e| e.fields.guid.as_str())
            .collect::<Vec<_>>(),
        ["c", "b", "a"],
        "newest first"
    );
    assert_eq!(list[2].fields.title, "Better title");
    assert_eq!(list[2].position_ms, 60_000);
    assert_eq!(list[2].podcast_title, "Show (renamed)");
}

#[tokio::test]
async fn subscriptions_count_unplayed_and_list_latest() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let id = upsert_podcast(&db, &show("Show"), 1).await.unwrap();
    sync_episodes(&db, &id, &[item("a", 100), item("b", 200)], 1)
        .await
        .unwrap();
    assert!(
        subscriptions(&db).await.unwrap().is_empty(),
        "not subscribed yet"
    );
    set_subscribed(&db, &id, Some(5)).await.unwrap();

    let b = episodes(&db, &id).await.unwrap()[0].id.clone();
    set_position(&db, &b, 1_000, 8).await.unwrap();
    set_played(&db, &b, Some(9)).await.unwrap();
    let played = episode(&db, &b).await.unwrap().unwrap();
    assert_eq!(
        (played.played_at, played.position_ms),
        (Some(9), 0),
        "played rewinds"
    );

    let subs = subscriptions(&db).await.unwrap();
    assert_eq!(subs.len(), 1);
    assert_eq!(
        (subs[0].episodes, subs[0].unplayed),
        (2, 0),
        "the back catalogue isn't new"
    );
    sync_episodes(&db, &id, &[item("c", 50)], 10).await.unwrap();
    assert_eq!(
        subscriptions(&db).await.unwrap()[0].unplayed,
        1,
        "found after subscribing"
    );
    assert_eq!(subs[0].latest_published_at, Some(200));
    assert_eq!(latest_episodes(&db, 1).await.unwrap()[0].fields.guid, "b");
    assert!(
        episode_by_audio(&db, "https://cdn.example/a.mp3")
            .await
            .unwrap()
            .is_some()
    );

    set_subscribed(&db, &id, None).await.unwrap();
    assert!(latest_episodes(&db, 10).await.unwrap().is_empty());
}

#[tokio::test]
async fn downloads_are_stored_per_episode() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let mut download = EpisodeDownload {
        episode_id: "e1".into(),
        state: "downloading".into(),
        path: None,
        received_bytes: 10,
        total_bytes: Some(100),
        error: None,
    };
    put_download(&db, &download, 1).await.unwrap();
    download.state = "done".into();
    download.path = Some("podcasts/p/e1.mp3".into());
    put_download(&db, &download, 2).await.unwrap();
    assert_eq!(downloads(&db).await.unwrap(), [download.clone()]);
    assert_eq!(
        episode_by_download(&db, "podcasts/p/e1.mp3").await.unwrap(),
        Some("e1".into())
    );
    remove_download(&db, "e1").await.unwrap();
    assert_eq!(super::download(&db, "e1").await.unwrap(), None);
}

#[tokio::test]
async fn books_keep_their_chapter_order_and_stay_out_of_podcast_lists() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let book = PodcastFields {
        feed_url: "https://librivox.org/rss/253".into(),
        kind: "audiobook".into(),
        ..show("Pride and Prejudice")
    };
    let id = upsert_podcast(&db, &book, 1).await.unwrap();
    let chapter = |guid: &str, number: i64| EpisodeFields {
        published_at: None,
        number: Some(number),
        transcript: None,
        ..item(guid, 0)
    };
    // The feed lists them out of order; the book reads them by number.
    sync_episodes(
        &db,
        &id,
        &[chapter("c2", 2), chapter("c1", 1), chapter("c3", 3)],
        2,
    )
    .await
    .unwrap();
    set_subscribed(&db, &id, Some(3)).await.unwrap();
    let show_id = upsert_podcast(&db, &show("A show"), 1).await.unwrap();
    set_subscribed(&db, &show_id, Some(3)).await.unwrap();

    let order: Vec<String> = episodes(&db, &id)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.fields.guid)
        .collect();
    assert_eq!(order, ["c1", "c2", "c3"]);
    let books = audiobooks(&db).await.unwrap();
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].chapters, 3);
    assert_eq!(books[0].podcast.fields.kind, "audiobook");
    let shows = subscriptions(&db).await.unwrap();
    assert_eq!(shows.len(), 1, "books aren't podcasts");
    assert_eq!(shows[0].podcast.fields.title, "A show");
    assert!(
        latest_episodes(&db, 10).await.unwrap().is_empty() || {
            latest_episodes(&db, 10)
                .await
                .unwrap()
                .iter()
                .all(|row| row.podcast_id != id)
        }
    );
}

#[tokio::test]
async fn each_show_keeps_its_order_and_speed() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let id = upsert_podcast(&db, &show("Show"), 1).await.unwrap();
    let row = podcast(&db, &id).await.unwrap().unwrap();
    assert_eq!((row.episode_order, row.speed), (None, None));

    set_episode_order(&db, &id, Some(r#"{"sort":"title"}"#))
        .await
        .unwrap();
    set_speed(&db, &id, Some(1.5)).await.unwrap();
    sync_episodes(&db, &id, &[item("a", 100)], 1).await.unwrap();
    let row = podcast(&db, &id).await.unwrap().unwrap();
    assert_eq!(row.episode_order.as_deref(), Some(r#"{"sort":"title"}"#));
    assert_eq!(row.speed, Some(1.5));
    assert_eq!(
        episodes(&db, &id).await.unwrap()[0].podcast_speed,
        Some(1.5),
        "episodes carry their show's speed"
    );

    // A refresh keeps both.
    upsert_podcast(&db, &show("Show"), 2).await.unwrap();
    let row = podcast(&db, &id).await.unwrap().unwrap();
    assert_eq!(row.speed, Some(1.5));
    set_episode_order(&db, &id, None).await.unwrap();
    set_speed(&db, &id, None).await.unwrap();
    let row = podcast(&db, &id).await.unwrap().unwrap();
    assert_eq!((row.episode_order, row.speed), (None, None));
}

#[tokio::test]
async fn continue_listening_lists_what_was_started_latest_first() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp).await;
    let id = upsert_podcast(&db, &show("Show"), 1).await.unwrap();
    sync_episodes(
        &db,
        &id,
        &[item("a", 100), item("b", 200), item("c", 300)],
        1,
    )
    .await
    .unwrap();
    let list = episodes(&db, &id).await.unwrap();
    let (c, b, a) = (&list[0].id, &list[1].id, &list[2].id);
    set_position(&db, a, 5_000, 10).await.unwrap();
    set_position(&db, b, 7_000, 20).await.unwrap();
    set_position(&db, c, 9_000, 30).await.unwrap();
    set_played(&db, c, Some(40)).await.unwrap();
    let started: Vec<String> = episodes_in_progress(&db, 10)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.fields.guid)
        .collect();
    assert_eq!(started, ["b", "a"], "played ones leave; the latest first");
    assert_eq!(
        episode(&db, c).await.unwrap().unwrap().listened_at,
        Some(40)
    );
    clear_position(&db, b).await.unwrap();
    assert_eq!(episodes_in_progress(&db, 10).await.unwrap().len(), 1);

    let book = upsert_podcast(
        &db,
        &PodcastFields {
            feed_url: "https://librivox.org/rss/1".into(),
            kind: "audiobook".into(),
            ..show("Book")
        },
        1,
    )
    .await
    .unwrap();
    sync_episodes(&db, &book, &[item("c1", 0), item("c2", 0)], 1)
        .await
        .unwrap();
    assert!(books_in_progress(&db, 10).await.unwrap().is_empty());
    let chapters = episodes(&db, &book).await.unwrap();
    set_played_many(&db, &[chapters[0].id.clone()], Some(50))
        .await
        .unwrap();
    let books = books_in_progress(&db, 10).await.unwrap();
    assert_eq!(books.len(), 1);
    assert_eq!((books[0].chapters, books[0].finished), (2, 1));
    assert_eq!(books[0].total_ms, 3_600_000);
    assert_eq!(books[0].listened_ms, 1_800_000);
    assert_eq!(books[0].listened_at, Some(50));
    set_played_many(
        &db,
        &chapters
            .iter()
            .map(|row| row.id.clone())
            .collect::<Vec<_>>(),
        Some(60),
    )
    .await
    .unwrap();
    assert!(
        books_in_progress(&db, 10).await.unwrap().is_empty(),
        "finished books leave"
    );

    set_subscribed(&db, &book, Some(1)).await.unwrap();
    assert_eq!(
        library_artwork(&db).await.unwrap(),
        ["https://img.example/show.jpg"],
        "books in the library keep their covers; shows not followed don't"
    );
}
