//! Genres and moods found for songs (M46.9), by track key. Device-local; these rows never sync.
//! The tags are JSON written by the app (`[{ name, kind }]`); an empty list remembers that the
//! sources had none.

use sqlx::{AssertSqlSafe, Row};

use crate::{Db, DbError};

/// Tags stored for each of `keys` that has any row: `(key, tags JSON)`.
pub async fn get_many(db: &Db, keys: &[String]) -> Result<Vec<(String, String)>, DbError> {
    let mut found = Vec::new();
    // SQLite allows a limited number of bound values per statement.
    for chunk in keys.chunks(500) {
        let marks = vec!["?"; chunk.len()].join(",");
        let sql =
            format!("SELECT track_key, tags FROM local_track_tags WHERE track_key IN ({marks})");
        // Only placeholders are formatted in; the keys are bound.
        let mut query = sqlx::query(AssertSqlSafe(sql));
        for key in chunk {
            query = query.bind(key);
        }
        for row in query.fetch_all(db.pool()).await? {
            found.push((row.get("track_key"), row.get("tags")));
        }
    }
    Ok(found)
}

/// Every stored row with tags: `(key, tags JSON)`.
pub async fn all(db: &Db) -> Result<Vec<(String, String)>, DbError> {
    let rows = sqlx::query("SELECT track_key, tags FROM local_track_tags WHERE tags != '[]'")
        .fetch_all(db.pool())
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.get("track_key"), row.get("tags")))
        .collect())
}

/// Stores a song's tags (or that it has none), replacing what was there.
pub async fn put(db: &Db, key: &str, tags_json: &str, fetched_at: i64) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO local_track_tags (track_key, tags, fetched_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(track_key) DO UPDATE SET tags = excluded.tags, fetched_at = excluded.fetched_at",
    )
    .bind(key)
    .bind(tags_json)
    .bind(fetched_at)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// Songs the user keeps: favorites first, then playlist items, then the device's library.
const SAVED_SONGS: &str = "
    SELECT track_key, title, artist, album, duration_ms, artwork_url, source, NULL AS genre, 0 AS rank
      FROM music_favorite WHERE deleted = 0
    UNION ALL
    SELECT track_key, title, artist, album, duration_ms, artwork_url, source, NULL, 1
      FROM music_playlist_item WHERE deleted = 0
    UNION ALL
    SELECT track_key, title, artist, album, duration_ms, NULL, NULL, genre, 2
      FROM local_track WHERE track_key IS NOT NULL";

/// A kept song without tags yet, for the background tagger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Untagged {
    pub track_key: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
    /// The genre in a library file's own tags.
    pub file_genre: Option<String>,
}

/// Up to `limit` kept songs with no tags row, favorites first.
pub async fn untagged(db: &Db, limit: i64) -> Result<Vec<Untagged>, DbError> {
    let sql = format!(
        "SELECT track_key, title, artist, album, duration_ms, MAX(genre) AS genre FROM ({SAVED_SONGS}) s
         WHERE NOT EXISTS (SELECT 1 FROM local_track_tags g WHERE g.track_key = s.track_key)
         GROUP BY track_key ORDER BY MIN(rank) LIMIT ?1"
    );
    let rows = sqlx::query(AssertSqlSafe(sql))
        .bind(limit)
        .fetch_all(db.pool())
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| Untagged {
            track_key: row.get("track_key"),
            title: row.get("title"),
            artist: row.get("artist"),
            album: row.get("album"),
            duration_ms: row.get("duration_ms"),
            file_genre: row.get("genre"),
        })
        .collect())
}

/// The tags of every kept song that has some: `(key, tags JSON)`, one row per song.
pub async fn kept(db: &Db) -> Result<Vec<(String, String)>, DbError> {
    let sql = format!(
        "SELECT g.track_key, g.tags FROM local_track_tags g
         WHERE g.tags != '[]' AND g.track_key IN (SELECT track_key FROM ({SAVED_SONGS}))"
    );
    let rows = sqlx::query(AssertSqlSafe(sql)).fetch_all(db.pool()).await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.get("track_key"), row.get("tags")))
        .collect())
}

/// A saved song (a favorite or a playlist item) with a given tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaggedSong {
    pub track_key: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
    pub artwork_url: Option<String>,
    /// A MediaSource as JSON.
    pub source: Option<String>,
}

/// Favorites and playlist items tagged `name`, once each, favorites first.
pub async fn saved_with_tag(db: &Db, name: &str) -> Result<Vec<TaggedSong>, DbError> {
    let sql = format!(
        "SELECT s.track_key, s.title, s.artist, s.album, s.duration_ms, s.artwork_url, s.source
           FROM ({SAVED_SONGS}) s JOIN local_track_tags g ON g.track_key = s.track_key
          WHERE s.rank < 2
            AND EXISTS (SELECT 1 FROM json_each(g.tags) WHERE json_extract(value, '$.name') = ?1)
          GROUP BY s.track_key ORDER BY MIN(s.rank), s.title"
    );
    let rows = sqlx::query(AssertSqlSafe(sql))
        .bind(name)
        .fetch_all(db.pool())
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| TaggedSong {
            track_key: row.get("track_key"),
            title: row.get("title"),
            artist: row.get("artist"),
            album: row.get("album"),
            duration_ms: row.get("duration_ms"),
            artwork_url: row.get("artwork_url"),
            source: row.get("source"),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stores_finds_and_replaces_tags() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        put(
            &db,
            "daft punk|get lucky",
            r#"[{"name":"Funk","kind":"genre"}]"#,
            1,
        )
        .await
        .unwrap();
        put(&db, "x|none", "[]", 1).await.unwrap();
        let keys = vec![
            "daft punk|get lucky".to_owned(),
            "x|none".into(),
            "y|missing".into(),
        ];
        let found = get_many(&db, &keys).await.unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(
            all(&db).await.unwrap().len(),
            1,
            "songs without tags aren't listed"
        );
        put(&db, "daft punk|get lucky", "[]", 2).await.unwrap();
        assert!(all(&db).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn kept_songs_wait_for_tags_and_are_found_by_them() {
        use crate::collections::{TrackFields, set_favorite};
        let temp = tempfile::TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let track = |key: &str, title: &str| TrackFields {
            track_key: key.into(),
            title: title.into(),
            artist: Some("Daft Punk".into()),
            album: None,
            duration_ms: Some(248_000),
            artwork_url: None,
            source: None,
        };
        set_favorite(&db, &track("daft punk|get lucky", "Get Lucky"), true, 1)
            .await
            .unwrap();
        set_favorite(
            &db,
            &track("daft punk|one more time", "One More Time"),
            true,
            2,
        )
        .await
        .unwrap();
        let waiting = untagged(&db, 10).await.unwrap();
        assert_eq!(waiting.len(), 2);

        put(
            &db,
            "daft punk|get lucky",
            r#"[{"name":"Funk","kind":"genre"},{"name":"Happy","kind":"mood"}]"#,
            3,
        )
        .await
        .unwrap();
        put(
            &db,
            "someone|else",
            r#"[{"name":"Funk","kind":"genre"}]"#,
            3,
        )
        .await
        .unwrap();
        assert_eq!(untagged(&db, 10).await.unwrap().len(), 1);
        assert_eq!(kept(&db).await.unwrap().len(), 1, "only kept songs count");
        let funk = saved_with_tag(&db, "Funk").await.unwrap();
        assert_eq!(funk.len(), 1);
        assert_eq!(funk[0].title, "Get Lucky");
        assert!(saved_with_tag(&db, "Rock").await.unwrap().is_empty());
    }
}
