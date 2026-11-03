//! Lyrics kept on this device (M46.46): what a source gave for a song, by its track key, with when
//! it was fetched and last shown. Device-local.

use sqlx::Row;

use crate::{Db, DbError};

/// A song's kept row: its lyrics (JSON, or none found) and when they were fetched.
pub struct CachedLyrics {
    pub lyrics: Option<String>,
    pub fetched_at: i64,
}

/// The song's kept row, if any; reading it marks the song's lyrics as shown now.
pub async fn get(db: &Db, track_key: &str, now_ms: i64) -> Result<Option<CachedLyrics>, DbError> {
    let row = sqlx::query(
        "UPDATE local_lyrics_cache SET used_at = ?2 WHERE track_key = ?1
         RETURNING lyrics, fetched_at",
    )
    .bind(track_key)
    .bind(now_ms)
    .fetch_optional(db.pool())
    .await?;
    Ok(row.map(|row| CachedLyrics {
        lyrics: row.get("lyrics"),
        fetched_at: row.get("fetched_at"),
    }))
}

/// Keeps what a source gave (or that none had any), shown now.
pub async fn put(
    db: &Db,
    track_key: &str,
    lyrics: Option<&str>,
    now_ms: i64,
) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO local_lyrics_cache (track_key, lyrics, fetched_at, used_at)
         VALUES (?1, ?2, ?3, ?3)
         ON CONFLICT (track_key) DO UPDATE SET lyrics = excluded.lyrics,
             fetched_at = excluded.fetched_at, used_at = excluded.used_at",
    )
    .bind(track_key)
    .bind(lyrics)
    .bind(now_ms)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// Marks a song's lyrics as shown now (its own lyrics show, so nothing is fetched).
pub async fn touch(db: &Db, track_key: &str, now_ms: i64) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO local_lyrics_cache (track_key, lyrics, fetched_at, used_at)
         VALUES (?1, NULL, 0, ?2)
         ON CONFLICT (track_key) DO UPDATE SET used_at = excluded.used_at",
    )
    .bind(track_key)
    .bind(now_ms)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// Forgets these songs' rows.
pub async fn remove(db: &Db, track_keys: &[String]) -> Result<(), DbError> {
    let mut tx = db.pool().begin().await?;
    for key in track_keys {
        sqlx::query("DELETE FROM local_lyrics_cache WHERE track_key = ?1")
            .bind(key)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Keys of songs with kept lyrics.
pub async fn keys(db: &Db) -> Result<Vec<String>, DbError> {
    let rows = sqlx::query("SELECT track_key FROM local_lyrics_cache WHERE lyrics IS NOT NULL")
        .fetch_all(db.pool())
        .await?;
    Ok(rows.iter().map(|row| row.get("track_key")).collect())
}

/// Keys of songs whose lyrics weren't shown since `before_ms`.
pub async fn stale(db: &Db, before_ms: i64) -> Result<Vec<String>, DbError> {
    let rows = sqlx::query("SELECT track_key FROM local_lyrics_cache WHERE used_at < ?1")
        .bind(before_ms)
        .fetch_all(db.pool())
        .await?;
    Ok(rows.iter().map(|row| row.get("track_key")).collect())
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[tokio::test]
    async fn kept_lyrics_are_read_back_and_go_stale_when_not_shown() {
        let temp = TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        put(&db, "a|one", Some("{\"x\":1}"), 10).await.unwrap();
        touch(&db, "a|two", 20).await.unwrap();
        let read = get(&db, "a|one", 30).await.unwrap().unwrap();
        assert_eq!(read.lyrics.as_deref(), Some("{\"x\":1}"));
        assert_eq!(read.fetched_at, 10);
        assert_eq!(keys(&db).await.unwrap(), ["a|one"]);
        // "a|one" was shown at 30, "a|two" last at 20.
        assert_eq!(stale(&db, 25).await.unwrap(), ["a|two"]);
        remove(&db, &["a|one".into()]).await.unwrap();
        assert!(get(&db, "a|one", 40).await.unwrap().is_none());
    }
}
