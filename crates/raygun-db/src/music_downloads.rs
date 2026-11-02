//! Songs saved into the app (M46.10): one row per song key, worked one at a time by the app's
//! download service. Device-local; these rows never sync, as the files stay on this device.

use sqlx::Row;
use sqlx::sqlite::SqliteRow;
use uuid::Uuid;

use crate::{Db, DbError};

pub const QUEUED: &str = "queued";
pub const DOWNLOADING: &str = "downloading";
pub const RECORDING: &str = "recording";
pub const DONE: &str = "done";
pub const FAILED: &str = "failed";
/// Only YouTube has the song: it can be recorded as it plays.
pub const NEEDS_RECORDING: &str = "needsRecording";

/// What a new download is asked with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSong {
    pub track_key: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
    pub artwork_url: Option<String>,
    /// A MediaSource as JSON.
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongDownloadRow {
    pub id: String,
    pub song: NewSong,
    pub state: String,
    pub kind: Option<String>,
    pub provider: Option<String>,
    pub file: Option<String>,
    pub done_bytes: i64,
    pub total_bytes: Option<i64>,
    pub error: Option<String>,
    pub created_at: i64,
    pub finished_at: Option<i64>,
}

const COLUMNS: &str =
    "id, track_key, title, artist, album, duration_ms, artwork_url, source, state,
    kind, provider, file, done_bytes, total_bytes, error, created_at, finished_at";

fn from_row(row: &SqliteRow) -> SongDownloadRow {
    SongDownloadRow {
        id: row.get("id"),
        song: NewSong {
            track_key: row.get("track_key"),
            title: row.get("title"),
            artist: row.get("artist"),
            album: row.get("album"),
            duration_ms: row.get("duration_ms"),
            artwork_url: row.get("artwork_url"),
            source: row.get("source"),
        },
        state: row.get("state"),
        kind: row.get("kind"),
        provider: row.get("provider"),
        file: row.get("file"),
        done_bytes: row.get("done_bytes"),
        total_bytes: row.get("total_bytes"),
        error: row.get("error"),
        created_at: row.get("created_at"),
        finished_at: row.get("finished_at"),
    }
}

/// Queues a song. One already queued, running or done is left alone (its id comes back); a failed
/// one goes back to the queue.
pub async fn enqueue(db: &Db, song: &NewSong, now_ms: i64) -> Result<String, DbError> {
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO local_music_download
             (id, track_key, title, artist, album, duration_ms, artwork_url, source, state, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'queued', ?9)
         ON CONFLICT(track_key) DO UPDATE SET state = 'queued', error = NULL, created_at = ?9
             WHERE local_music_download.state = 'failed'",
    )
    .bind(&id)
    .bind(&song.track_key)
    .bind(&song.title)
    .bind(&song.artist)
    .bind(&song.album)
    .bind(song.duration_ms)
    .bind(&song.artwork_url)
    .bind(&song.source)
    .bind(now_ms)
    .execute(db.pool())
    .await?;
    let row = sqlx::query("SELECT id FROM local_music_download WHERE track_key = ?1")
        .bind(&song.track_key)
        .fetch_one(db.pool())
        .await?;
    Ok(row.get("id"))
}

/// The oldest queued song.
pub async fn next_queued(db: &Db) -> Result<Option<SongDownloadRow>, DbError> {
    let sql = format!(
        "SELECT {COLUMNS} FROM local_music_download WHERE state = 'queued'
         ORDER BY created_at, id LIMIT 1"
    );
    let row = sqlx::query(sqlx::AssertSqlSafe(sql))
        .fetch_optional(db.pool())
        .await?;
    Ok(row.as_ref().map(from_row))
}

pub async fn get(db: &Db, id: &str) -> Result<Option<SongDownloadRow>, DbError> {
    let sql = format!("SELECT {COLUMNS} FROM local_music_download WHERE id = ?1");
    let row = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(db.pool())
        .await?;
    Ok(row.as_ref().map(from_row))
}

/// Every download, newest first.
pub async fn all(db: &Db) -> Result<Vec<SongDownloadRow>, DbError> {
    let sql =
        format!("SELECT {COLUMNS} FROM local_music_download ORDER BY created_at DESC, id DESC");
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .fetch_all(db.pool())
        .await?;
    Ok(rows.iter().map(from_row).collect())
}

/// Finished downloads: `(id, track_key, file)`.
pub async fn done(db: &Db) -> Result<Vec<(String, String, String)>, DbError> {
    let rows = sqlx::query(
        "SELECT id, track_key, file FROM local_music_download
         WHERE state = 'done' AND file IS NOT NULL",
    )
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .iter()
        .map(|row| (row.get("id"), row.get("track_key"), row.get("file")))
        .collect())
}

/// Sets a download's state (and clears its error unless it failed).
pub async fn set_state(db: &Db, id: &str, state: &str) -> Result<(), DbError> {
    sqlx::query("UPDATE local_music_download SET state = ?2, error = NULL WHERE id = ?1")
        .bind(id)
        .bind(state)
        .execute(db.pool())
        .await?;
    Ok(())
}

pub async fn set_progress(
    db: &Db,
    id: &str,
    done_bytes: i64,
    total_bytes: Option<i64>,
) -> Result<(), DbError> {
    sqlx::query("UPDATE local_music_download SET done_bytes = ?2, total_bytes = ?3 WHERE id = ?1")
        .bind(id)
        .bind(done_bytes)
        .bind(total_bytes)
        .execute(db.pool())
        .await?;
    Ok(())
}

/// A finished song: its file, how it was made and where it came from.
pub async fn set_done(
    db: &Db,
    id: &str,
    file: &str,
    kind: &str,
    provider: Option<&str>,
    now_ms: i64,
) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE local_music_download SET state = 'done', file = ?2, kind = ?3, provider = ?4,
             error = NULL, finished_at = ?5 WHERE id = ?1",
    )
    .bind(id)
    .bind(file)
    .bind(kind)
    .bind(provider)
    .bind(now_ms)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// A download that stopped with `error`; `state` is `failed` or `needsRecording`.
pub async fn set_failed(db: &Db, id: &str, state: &str, error: &str) -> Result<(), DbError> {
    sqlx::query("UPDATE local_music_download SET state = ?2, error = ?3 WHERE id = ?1")
        .bind(id)
        .bind(state)
        .bind(error)
        .execute(db.pool())
        .await?;
    Ok(())
}

/// Remembers the source a recording plays (a YouTube video, as JSON).
pub async fn set_source(db: &Db, id: &str, source: &str) -> Result<(), DbError> {
    sqlx::query("UPDATE local_music_download SET source = ?2 WHERE id = ?1")
        .bind(id)
        .bind(source)
        .execute(db.pool())
        .await?;
    Ok(())
}

/// Downloads and recordings cut off by the last exit go back to the queue (recordings to wait
/// for another go). Returns how many.
pub async fn reset_interrupted(db: &Db) -> Result<u64, DbError> {
    let downloads =
        sqlx::query("UPDATE local_music_download SET state = 'queued' WHERE state = 'downloading'")
            .execute(db.pool())
            .await?
            .rows_affected();
    let recordings = sqlx::query(
        "UPDATE local_music_download SET state = 'needsRecording' WHERE state = 'recording'",
    )
    .execute(db.pool())
    .await?
    .rows_affected();
    Ok(downloads + recordings)
}

pub async fn remove(db: &Db, id: &str) -> Result<(), DbError> {
    sqlx::query("DELETE FROM local_music_download WHERE id = ?1")
        .bind(id)
        .execute(db.pool())
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(key: &str) -> NewSong {
        NewSong {
            track_key: key.into(),
            title: key.into(),
            artist: Some("A".into()),
            album: None,
            duration_ms: Some(200_000),
            artwork_url: None,
            source: None,
        }
    }

    #[tokio::test]
    async fn queues_works_and_finishes_songs_once_each() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let first = enqueue(&db, &song("a|one"), 1).await.unwrap();
        let second = enqueue(&db, &song("a|two"), 2).await.unwrap();
        assert_eq!(
            enqueue(&db, &song("a|one"), 3).await.unwrap(),
            first,
            "one per song"
        );
        assert_eq!(next_queued(&db).await.unwrap().unwrap().id, first);

        set_state(&db, &first, DOWNLOADING).await.unwrap();
        assert_eq!(next_queued(&db).await.unwrap().unwrap().id, second);
        assert_eq!(reset_interrupted(&db).await.unwrap(), 1);
        assert_eq!(get(&db, &first).await.unwrap().unwrap().state, QUEUED);

        set_progress(&db, &first, 10, Some(100)).await.unwrap();
        set_done(&db, &first, "track.mp3", "file", Some("Archive"), 9)
            .await
            .unwrap();
        assert_eq!(
            done(&db).await.unwrap(),
            vec![(first.clone(), "a|one".to_owned(), "track.mp3".to_owned())]
        );

        set_failed(&db, &second, NEEDS_RECORDING, "Only YouTube has it")
            .await
            .unwrap();
        set_state(&db, &second, RECORDING).await.unwrap();
        reset_interrupted(&db).await.unwrap();
        assert_eq!(
            get(&db, &second).await.unwrap().unwrap().state,
            NEEDS_RECORDING
        );
        set_failed(&db, &second, FAILED, "gone").await.unwrap();
        // A failed song asked again goes back to the queue.
        enqueue(&db, &song("a|two"), 10).await.unwrap();
        assert_eq!(get(&db, &second).await.unwrap().unwrap().state, QUEUED);
        assert_eq!(all(&db).await.unwrap().len(), 2);
        remove(&db, &second).await.unwrap();
        assert_eq!(all(&db).await.unwrap().len(), 1);
    }
}
