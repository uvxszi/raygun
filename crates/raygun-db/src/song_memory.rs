//! What Raygun keeps about songs, and how recently each was played or seen: covers found at other
//! sources (`local_cover_found`, M46.40), play stamps (`local_track_seen`), and what the app's
//! sweep reads and forgets of the facts kept elsewhere: the streams that played
//! (`local_stream_candidate`) and lengths (`local_track_duration`); lyrics go through
//! [`crate::lyrics_cache`]. Device-local; these rows never sync.

use std::collections::HashMap;

use sqlx::{AssertSqlSafe, Row};

use crate::{Db, DbError};

/// What covers of songs are kept under: `song:<track key>`.
pub const SONG: &str = "song:";

/// SQLite allows a limited number of bound values per statement.
const CHUNK: usize = 500;

/// What a cover lookup found: the address (`None`: no source had one), the source that gave it,
/// and when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundCover {
    pub url: Option<String>,
    pub source: Option<String>,
    pub found_at: i64,
}

/// The kept lookups among `keys` (`song:…`, `album:…`, `artist:…`), by key.
pub async fn found(db: &Db, keys: &[String]) -> Result<HashMap<String, FoundCover>, DbError> {
    let mut found = HashMap::new();
    for chunk in keys.chunks(CHUNK) {
        let marks = vec!["?"; chunk.len()].join(",");
        // Only placeholders are formatted in; the keys are bound.
        let mut query = sqlx::query(AssertSqlSafe(format!(
            "SELECT key, url, source, found_at FROM local_cover_found WHERE key IN ({marks})"
        )));
        for key in chunk {
            query = query.bind(key);
        }
        for row in query.fetch_all(db.pool()).await? {
            found.insert(
                row.get("key"),
                FoundCover {
                    url: row.get("url"),
                    source: row.get("source"),
                    found_at: row.get("found_at"),
                },
            );
        }
    }
    Ok(found)
}

/// Keeps what a lookup found for `key`, replacing what was kept.
pub async fn put_found(
    db: &Db,
    key: &str,
    url: Option<&str>,
    source: Option<&str>,
    now_ms: i64,
) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO local_cover_found (key, url, source, found_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (key) DO UPDATE SET url = excluded.url, source = excluded.source,
             found_at = excluded.found_at",
    )
    .bind(key)
    .bind(url)
    .bind(source)
    .bind(now_ms)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// Every kept lookup's key and time, for the sweep.
pub async fn found_times(db: &Db) -> Result<Vec<(String, i64)>, DbError> {
    let rows = sqlx::query("SELECT key, found_at FROM local_cover_found")
        .fetch_all(db.pool())
        .await?;
    Ok(rows
        .iter()
        .map(|row| (row.get("key"), row.get("found_at")))
        .collect())
}

/// How many covers found elsewhere are kept (lookups that found none left out).
pub async fn found_count(db: &Db) -> Result<u32, DbError> {
    let row = sqlx::query("SELECT COUNT(*) AS kept FROM local_cover_found WHERE url IS NOT NULL")
        .fetch_one(db.pool())
        .await?;
    Ok(u32::try_from(row.get::<i64, _>("kept")).unwrap_or(u32::MAX))
}

/// Forgets these lookups.
pub async fn remove_found(db: &Db, keys: &[String]) -> Result<(), DbError> {
    let mut transaction = db.pool().begin().await?;
    for key in keys {
        sqlx::query("DELETE FROM local_cover_found WHERE key = ?1")
            .bind(key)
            .execute(&mut *transaction)
            .await?;
    }
    transaction.commit().await?;
    Ok(())
}

/// Moves lookups kept the old way (a `local_meta` value, JSON `{ key: [url or null, unix ms] }`)
/// into the table, once; returns how many came. A value that can't be read is dropped: it only
/// remembered lookups, which are made again.
pub async fn import_legacy(db: &Db, meta_key: &str) -> Result<usize, DbError> {
    let stored = match crate::local_value(db, meta_key).await {
        Ok(Some(json)) => json,
        Ok(None) => return Ok(0),
        Err(DbError::Damaged { .. }) => String::new(),
        Err(error) => return Err(error),
    };
    let entries: HashMap<String, (Option<String>, i64)> =
        serde_json::from_str(&stored).unwrap_or_default();
    let mut transaction = db.pool().begin().await?;
    for (key, (url, found_at)) in &entries {
        sqlx::query(
            "INSERT OR IGNORE INTO local_cover_found (key, url, source, found_at)
             VALUES (?1, ?2, NULL, ?3)",
        )
        .bind(key)
        .bind(url)
        .bind(found_at)
        .execute(&mut *transaction)
        .await?;
    }
    sqlx::query("DELETE FROM local_meta WHERE key = ?1")
        .bind(meta_key)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(entries.len())
}

/// Stamps a song as played now.
pub async fn stamp(db: &Db, track_key: &str, now_ms: i64) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO local_track_seen (track_key, seen_at) VALUES (?1, ?2)
         ON CONFLICT (track_key) DO UPDATE SET seen_at = MAX(seen_at, excluded.seen_at)",
    )
    .bind(track_key)
    .bind(now_ms)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// Forgets every play stamp (history cleared).
pub async fn clear_stamps(db: &Db) -> Result<(), DbError> {
    sqlx::query("DELETE FROM local_track_seen")
        .execute(db.pool())
        .await?;
    Ok(())
}

/// Every song Raygun keeps anything about or played, with when it was last played or seen, the
/// most recent first: the newest of its play stamp, its plays in history, and when its kept facts
/// were made or used (a stream that played or failed, its lyrics shown or translated, its length
/// learned, its cover looked up).
pub async fn recency(db: &Db) -> Result<Vec<(String, i64)>, DbError> {
    let rows = sqlx::query(
        "SELECT track_key, MAX(at) AS at FROM (
             SELECT track_key, seen_at AS at FROM local_track_seen
             UNION ALL
             SELECT track_key, played_at FROM music_play_history WHERE deleted = 0
             UNION ALL
             SELECT track_key, MAX(COALESCE(verified_at, 0), COALESCE(failed_at, 0))
             FROM local_stream_candidate
             UNION ALL
             SELECT track_key, used_at FROM local_lyrics_cache
             UNION ALL
             SELECT track_key, made_at FROM local_lyrics_translation
             UNION ALL
             SELECT track_key, updated_at FROM local_track_duration
             UNION ALL
             SELECT substr(key, 6), found_at FROM local_cover_found
             WHERE substr(key, 1, 5) = 'song:'
         )
         GROUP BY track_key
         ORDER BY at DESC, track_key",
    )
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .iter()
        .map(|row| (row.get("track_key"), row.get("at")))
        .collect())
}

/// Forgets what's kept of these songs here: the streams that played or failed for them, their
/// lengths and their play stamps. Their covers found elsewhere go with [`remove_found`].
pub async fn forget(db: &Db, track_keys: &[String]) -> Result<(), DbError> {
    let mut transaction = db.pool().begin().await?;
    for key in track_keys {
        for sql in [
            "DELETE FROM local_stream_candidate WHERE track_key = ?1",
            "DELETE FROM local_track_duration WHERE track_key = ?1",
            "DELETE FROM local_track_seen WHERE track_key = ?1",
        ] {
            sqlx::query(sql)
                .bind(key)
                .execute(&mut *transaction)
                .await?;
        }
    }
    transaction.commit().await?;
    Ok(())
}

/// A song in the library without a cover of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uncovered {
    pub track_key: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
    /// In Favorites, a playlist or downloads; else only in history.
    pub saved: bool,
}

/// [`uncovered`]'s order: a playlist's first songs, Favorites, the rest of the playlists,
/// downloads, history.
const HISTORY_TIER: i64 = 4;

/// Songs in playlists, Favorites, downloads and history that no copy has a cover for, and that
/// have none from elsewhere: playlists' first `cover_items` songs first (the covers made of
/// them), then Favorites, the rest of the playlists, downloads and history, the latest first
/// within each. Left out: songs a lookup found nothing for since `missed_before` (unix ms), songs
/// the user gave a cover, and the device's own files with artwork.
pub async fn uncovered(
    db: &Db,
    missed_before: i64,
    cover_items: i64,
    limit: i64,
) -> Result<Vec<Uncovered>, DbError> {
    let rows = sqlx::query(
        "WITH placed AS (
             SELECT track_key, title, artist, album, duration_ms, artwork_url, added_at,
                    ROW_NUMBER() OVER (PARTITION BY playlist_id ORDER BY position, id) AS place
             FROM music_playlist_item WHERE deleted = 0
         ),
         songs AS (
             SELECT track_key, title, artist, album, duration_ms, artwork_url,
                    CASE WHEN place <= ?2 THEN 0 ELSE 2 END AS tier, added_at AS at
             FROM placed
             UNION ALL
             SELECT track_key, title, artist, album, duration_ms, artwork_url, 1, added_at
             FROM music_favorite WHERE deleted = 0
             UNION ALL
             SELECT track_key, title, artist, album, duration_ms, artwork_url, 3, created_at
             FROM local_music_download
             UNION ALL
             SELECT track_key, title, artist, album, duration_ms, artwork_url, ?4, played_at
             FROM music_play_history WHERE deleted = 0
         ),
         grouped AS (
             SELECT track_key, title, artist, album, duration_ms, MIN(tier) AS tier,
                    MAX(at) AS at,
                    MAX(artwork_url IS NOT NULL AND artwork_url <> '') AS covered
             FROM songs GROUP BY track_key
         )
         SELECT track_key, title, artist, album, duration_ms, tier FROM grouped
         WHERE covered = 0
           AND track_key NOT IN (
               SELECT substr(key, 6) FROM local_cover_found
               WHERE substr(key, 1, 5) = 'song:' AND (url IS NOT NULL OR found_at >= ?1))
           AND track_key NOT IN (
               SELECT key FROM local_detail_edit WHERE kind = 'song' AND artwork IS NOT NULL)
           AND track_key NOT IN (
               SELECT track_key FROM local_track WHERE track_key IS NOT NULL AND art IS NOT NULL)
         ORDER BY tier, at DESC, track_key
         LIMIT ?3",
    )
    .bind(missed_before)
    .bind(cover_items)
    .bind(limit)
    .bind(HISTORY_TIER)
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .iter()
        .map(|row| Uncovered {
            track_key: row.get("track_key"),
            title: row.get("title"),
            artist: row.get("artist"),
            album: row.get("album"),
            duration_ms: row.get("duration_ms"),
            saved: row.get::<i64, _>("tier") < HISTORY_TIER,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::collections::{self, TrackFields};

    async fn open() -> (TempDir, Db) {
        let temp = TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        (temp, db)
    }

    fn song(key: &str, artwork: Option<&str>) -> TrackFields {
        let (artist, title) = key.split_once('|').unwrap();
        TrackFields {
            track_key: key.to_owned(),
            title: title.to_owned(),
            artist: Some(artist.to_owned()),
            album: None,
            duration_ms: None,
            artwork_url: artwork.map(str::to_owned),
            source: None,
        }
    }

    #[tokio::test]
    async fn found_covers_are_kept_with_their_source_and_the_old_value_moves_in() {
        let (_temp, db) = open().await;
        crate::set_local_value(
            &db,
            "covers.fallback",
            r#"{"song:a|one":["https://img/1.jpg",5],"artist:b":[null,6]}"#,
        )
        .await
        .unwrap();
        assert_eq!(import_legacy(&db, "covers.fallback").await.unwrap(), 2);
        // Once: the value is gone.
        assert_eq!(import_legacy(&db, "covers.fallback").await.unwrap(), 0);
        assert!(
            crate::local_value(&db, "covers.fallback")
                .await
                .unwrap()
                .is_none()
        );
        put_found(
            &db,
            "song:c|three",
            Some("https://img/3.jpg"),
            Some("Deezer"),
            9,
        )
        .await
        .unwrap();
        let kept = found(
            &db,
            &[
                "song:a|one".into(),
                "artist:b".into(),
                "song:c|three".into(),
            ],
        )
        .await
        .unwrap();
        assert_eq!(kept["song:a|one"].url.as_deref(), Some("https://img/1.jpg"));
        assert_eq!(kept["artist:b"].url, None);
        assert_eq!(kept["song:c|three"].source.as_deref(), Some("Deezer"));
        assert_eq!(found_count(&db).await.unwrap(), 2);
        remove_found(&db, &["song:a|one".into()]).await.unwrap();
        assert_eq!(found_times(&db).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn recency_is_the_newest_play_or_kept_fact_of_each_song() {
        let (_temp, db) = open().await;
        stamp(&db, "a|one", 100).await.unwrap();
        // A stamp never goes back.
        stamp(&db, "a|one", 50).await.unwrap();
        collections::record_play(&db, &song("b|two", None), 300)
            .await
            .unwrap();
        crate::track_durations::put_many(&db, &[("c|three".into(), 1000)], "source", false, 200)
            .await
            .unwrap();
        crate::lyrics_cache::put(&db, "a|one", None, 150)
            .await
            .unwrap();
        put_found(&db, "song:d|four", None, None, 250)
            .await
            .unwrap();
        put_found(&db, "artist:e", None, None, 400).await.unwrap();
        assert_eq!(
            recency(&db).await.unwrap(),
            [
                ("b|two".to_owned(), 300),
                ("d|four".to_owned(), 250),
                ("c|three".to_owned(), 200),
                ("a|one".to_owned(), 150),
            ]
        );

        forget(&db, &["a|one".into(), "c|three".into()])
            .await
            .unwrap();
        // Lyrics go through the lyrics store; the stamp and the length are gone.
        assert_eq!(
            recency(&db).await.unwrap().last(),
            Some(&("a|one".to_owned(), 150))
        );
        assert!(
            crate::track_durations::lengths(&db, &["c|three".into()])
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn songs_without_covers_come_mosaics_first_and_skip_recent_misses() {
        let (_temp, db) = open().await;
        let list = collections::create_playlist(&db, "Road", 1).await.unwrap();
        collections::add_to_playlist(
            &db,
            &list,
            &[
                song("a|first", None),
                song("b|covered", Some("https://img/b.jpg")),
                song("c|third", None),
            ],
            10,
        )
        .await
        .unwrap();
        collections::set_favorite(&db, &song("d|fav", None), true, 20)
            .await
            .unwrap();
        collections::record_play(&db, &song("e|heard", None), 30)
            .await
            .unwrap();
        // A play without artwork of a song whose playlist copy has some: it has a cover.
        collections::record_play(&db, &song("b|covered", None), 40)
            .await
            .unwrap();
        let keys = |rows: Vec<Uncovered>| -> Vec<String> {
            rows.into_iter().map(|row| row.track_key).collect()
        };
        // Only the first song counts as the mosaic's here.
        let all = uncovered(&db, 0, 1, 10).await.unwrap();
        // Only history's song isn't saved.
        assert!(
            all.iter()
                .all(|song| song.saved == (song.track_key != "e|heard"))
        );
        assert_eq!(keys(all), ["a|first", "d|fav", "c|third", "e|heard"]);
        // A miss at 50 holds the song back until a lookup after it is due; a found cover always.
        put_found(&db, "song:a|first", None, None, 50)
            .await
            .unwrap();
        put_found(&db, "song:d|fav", Some("https://img/d.jpg"), Some("X"), 1)
            .await
            .unwrap();
        assert_eq!(
            keys(uncovered(&db, 40, 1, 10).await.unwrap()),
            ["c|third", "e|heard"]
        );
        assert_eq!(keys(uncovered(&db, 60, 1, 1).await.unwrap()), ["a|first"]);
    }
}
