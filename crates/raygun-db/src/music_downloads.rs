//! Songs saved into the app (M46.10): one row per song key, worked one at a time by the app's
//! download service. Device-local; these rows never sync, as the files stay on this device.
//!
//! The queue runs by `queued_at`, oldest first: a song tried again or given another source goes
//! first, and the queue can be put in any order.

use sqlx::Row;
use sqlx::sqlite::SqliteRow;
use uuid::Uuid;

use crate::{Db, DbError};

pub const QUEUED: &str = "queued";
pub const DOWNLOADING: &str = "downloading";
pub const RECORDING: &str = "recording";
pub const DONE: &str = "done";
pub const FAILED: &str = "failed";
/// No source gave a file, but one plays it: it can be recorded as it plays.
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
    /// Where the audio came from once done; before that, the source picked for it, if any.
    pub provider: Option<String>,
    pub file: Option<String>,
    pub done_bytes: i64,
    pub total_bytes: Option<i64>,
    /// What went wrong, for the listener.
    pub error: Option<String>,
    /// Each source's own error, for More info.
    pub error_detail: Option<String>,
    pub created_at: i64,
    /// Its place in the queue (unix ms; earlier goes first).
    pub queued_at: i64,
    /// When it finished, or stopped with an error.
    pub finished_at: Option<i64>,
}

const COLUMNS: &str =
    "id, track_key, title, artist, album, duration_ms, artwork_url, source, state,
    kind, provider, file, done_bytes, total_bytes, error, error_detail, created_at,
    COALESCE(queued_at, created_at) AS queued_at, finished_at";

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
        error_detail: row.get("error_detail"),
        created_at: row.get("created_at"),
        queued_at: row.get("queued_at"),
        finished_at: row.get("finished_at"),
    }
}

/// Queues a song. One already queued, running or done is left alone; a failed one (or one waiting
/// to be recorded) goes back to the end of the queue, with the source it's asked with now. Returns
/// its id and whether anything changed.
pub async fn enqueue(db: &Db, song: &NewSong, now_ms: i64) -> Result<(String, bool), DbError> {
    let id = Uuid::now_v7().to_string();
    let changed = sqlx::query(
        "INSERT INTO local_music_download
             (id, track_key, title, artist, album, duration_ms, artwork_url, source, state,
              created_at, queued_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'queued', ?9, ?9)
         ON CONFLICT(track_key) DO UPDATE SET state = 'queued', error = NULL, error_detail = NULL,
             source = COALESCE(excluded.source, source), provider = NULL, done_bytes = 0,
             total_bytes = NULL, created_at = ?9, queued_at = ?9, finished_at = NULL
             WHERE local_music_download.state IN ('failed', 'needsRecording')",
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
    .await?
    .rows_affected()
        > 0;
    let row = sqlx::query("SELECT id FROM local_music_download WHERE track_key = ?1")
        .bind(&song.track_key)
        .fetch_one(db.pool())
        .await?;
    Ok((row.get("id"), changed))
}

/// The song first in the queue.
pub async fn next_queued(db: &Db) -> Result<Option<SongDownloadRow>, DbError> {
    let sql = format!(
        "SELECT {COLUMNS} FROM local_music_download WHERE state = 'queued'
         ORDER BY COALESCE(queued_at, created_at), created_at, id LIMIT 1"
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

/// The download of the song with this key, whatever its state.
pub async fn by_key(db: &Db, track_key: &str) -> Result<Option<SongDownloadRow>, DbError> {
    let sql = format!("SELECT {COLUMNS} FROM local_music_download WHERE track_key = ?1");
    let row = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(track_key)
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

/// Ids of the downloads that stopped with an error or wait to be recorded, in queue order.
pub async fn failed(db: &Db) -> Result<Vec<String>, DbError> {
    let rows = sqlx::query(
        "SELECT id FROM local_music_download WHERE state IN ('failed', 'needsRecording')
         ORDER BY COALESCE(queued_at, created_at), id",
    )
    .fetch_all(db.pool())
    .await?;
    Ok(rows.iter().map(|row| row.get("id")).collect())
}

/// Where the front of the queue is: the earliest place of a queued song.
pub async fn queue_front(db: &Db) -> Result<Option<i64>, DbError> {
    let row = sqlx::query(
        "SELECT MIN(COALESCE(queued_at, created_at)) AS front FROM local_music_download
         WHERE state = 'queued'",
    )
    .fetch_one(db.pool())
    .await?;
    Ok(row.get("front"))
}

/// Sets a download's state (and clears its error unless it failed).
pub async fn set_state(db: &Db, id: &str, state: &str) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE local_music_download SET state = ?2, error = NULL, error_detail = NULL
         WHERE id = ?1",
    )
    .bind(id)
    .bind(state)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// Puts a download (back) in the queue, at `queued_at` when given, else where it was.
pub async fn set_queued(db: &Db, id: &str, queued_at: Option<i64>) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE local_music_download SET state = 'queued', error = NULL, error_detail = NULL,
             finished_at = NULL, queued_at = COALESCE(?2, queued_at, created_at)
         WHERE id = ?1 AND state <> 'done'",
    )
    .bind(id)
    .bind(queued_at)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// Puts the queued songs among `ids` in that order, from `front` on; others keep their places.
pub async fn reorder(db: &Db, ids: &[String], front: i64) -> Result<(), DbError> {
    let mut tx = db.pool().begin().await?;
    for (index, id) in ids.iter().enumerate() {
        sqlx::query(
            "UPDATE local_music_download SET queued_at = ?2 WHERE id = ?1 AND state = 'queued'",
        )
        .bind(id)
        .bind(front + index as i64)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
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

/// A finished song: its file and size, how it was made and where it came from. False when the
/// download was removed meanwhile.
pub async fn set_done(
    db: &Db,
    id: &str,
    file: &str,
    kind: &str,
    provider: Option<&str>,
    bytes: Option<i64>,
    now_ms: i64,
) -> Result<bool, DbError> {
    let updated = sqlx::query(
        "UPDATE local_music_download SET state = 'done', file = ?2, kind = ?3, provider = ?4,
             done_bytes = COALESCE(?5, done_bytes), total_bytes = COALESCE(?5, total_bytes),
             error = NULL, error_detail = NULL, finished_at = ?6 WHERE id = ?1",
    )
    .bind(id)
    .bind(file)
    .bind(kind)
    .bind(provider)
    .bind(bytes)
    .bind(now_ms)
    .execute(db.pool())
    .await?
    .rows_affected();
    Ok(updated > 0)
}

/// A download that stopped with `error` (and the sources' own errors in `detail`); `state` is
/// `failed` or `needsRecording`.
pub async fn set_failed(
    db: &Db,
    id: &str,
    state: &str,
    error: &str,
    detail: Option<&str>,
    now_ms: i64,
) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE local_music_download SET state = ?2, error = ?3, error_detail = ?4,
             finished_at = ?5 WHERE id = ?1",
    )
    .bind(id)
    .bind(state)
    .bind(error)
    .bind(detail)
    .bind(now_ms)
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

/// The source picked for a download (JSON) and its name. What was saved from another stream
/// doesn't count anymore.
pub async fn set_picked(
    db: &Db,
    id: &str,
    source: &str,
    provider: Option<&str>,
) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE local_music_download SET source = ?2, provider = ?3, done_bytes = 0,
             total_bytes = NULL WHERE id = ?1 AND state <> 'done'",
    )
    .bind(id)
    .bind(source)
    .bind(provider)
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
        let (first, added) = enqueue(&db, &song("a|one"), 1).await.unwrap();
        assert!(added);
        let (second, _) = enqueue(&db, &song("a|two"), 2).await.unwrap();
        assert_eq!(
            enqueue(&db, &song("a|one"), 3).await.unwrap(),
            (first.clone(), false),
            "one per song"
        );
        assert_eq!(next_queued(&db).await.unwrap().unwrap().id, first);

        set_state(&db, &first, DOWNLOADING).await.unwrap();
        assert_eq!(next_queued(&db).await.unwrap().unwrap().id, second);
        assert_eq!(reset_interrupted(&db).await.unwrap(), 1);
        assert_eq!(get(&db, &first).await.unwrap().unwrap().state, QUEUED);

        set_progress(&db, &first, 10, Some(100)).await.unwrap();
        set_done(
            &db,
            &first,
            "track.mp3",
            "file",
            Some("Archive"),
            Some(4_000),
            9,
        )
        .await
        .unwrap();
        assert_eq!(
            done(&db).await.unwrap(),
            vec![(first.clone(), "a|one".to_owned(), "track.mp3".to_owned())]
        );
        let finished = get(&db, &first).await.unwrap().unwrap();
        assert_eq!(
            (
                finished.done_bytes,
                finished.total_bytes,
                finished.finished_at
            ),
            (4_000, Some(4_000), Some(9)),
            "a finished song keeps its size"
        );

        set_failed(
            &db,
            &second,
            NEEDS_RECORDING,
            "Only YouTube has it",
            Some("Audius: 403"),
            10,
        )
        .await
        .unwrap();
        let waiting = get(&db, &second).await.unwrap().unwrap();
        assert_eq!(waiting.error_detail.as_deref(), Some("Audius: 403"));
        assert_eq!(waiting.finished_at, Some(10));
        set_state(&db, &second, RECORDING).await.unwrap();
        reset_interrupted(&db).await.unwrap();
        assert_eq!(
            get(&db, &second).await.unwrap().unwrap().state,
            NEEDS_RECORDING
        );
        // A song waiting to be recorded, asked again with a source, goes back to the queue with it.
        let asked = NewSong {
            source: Some("{\"kind\":\"url\"}".into()),
            ..song("a|two")
        };
        assert!(enqueue(&db, &asked, 11).await.unwrap().1);
        let again = get(&db, &second).await.unwrap().unwrap();
        assert_eq!(again.state, QUEUED);
        assert_eq!(again.error, None);
        assert_eq!(again.error_detail, None);
        assert_eq!(again.song.source.as_deref(), Some("{\"kind\":\"url\"}"));
        assert_eq!(all(&db).await.unwrap().len(), 2);
        assert_eq!(
            by_key(&db, "a|two").await.unwrap().map(|row| row.id),
            Some(second.clone())
        );
        remove(&db, &second).await.unwrap();
        assert_eq!(all(&db).await.unwrap().len(), 1);
    }

    /// The queued songs in the order they'll download.
    async fn order(db: &Db) -> Vec<String> {
        let mut queued: Vec<SongDownloadRow> = all(db)
            .await
            .unwrap()
            .into_iter()
            .filter(|row| row.state == QUEUED)
            .collect();
        queued.sort_by_key(|row| (row.queued_at, row.created_at));
        queued.into_iter().map(|row| row.id).collect()
    }

    #[tokio::test]
    async fn songs_tried_again_go_first_and_the_queue_takes_any_order() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let mut ids = Vec::new();
        for (index, key) in ["a|one", "a|two", "a|three", "a|four"].iter().enumerate() {
            ids.push(enqueue(&db, &song(key), 10 + index as i64).await.unwrap().0);
        }

        // The last one fails, then is tried again: it goes first.
        set_failed(&db, &ids[3], FAILED, "The server answered 503", None, 20)
            .await
            .unwrap();
        assert_eq!(failed(&db).await.unwrap(), vec![ids[3].clone()]);
        let front = queue_front(&db).await.unwrap().unwrap();
        set_queued(&db, &ids[3], Some(front - 1)).await.unwrap();
        assert_eq!(next_queued(&db).await.unwrap().unwrap().id, ids[3]);
        let retried = get(&db, &ids[3]).await.unwrap().unwrap();
        assert_eq!((retried.error, retried.finished_at), (None, None));

        // Any order, ahead of the front; songs not mentioned keep their places after them.
        let front = queue_front(&db).await.unwrap().unwrap();
        reorder(&db, &[ids[2].clone(), ids[0].clone()], front - 2)
            .await
            .unwrap();
        assert_eq!(
            order(&db).await,
            vec![
                ids[2].clone(),
                ids[0].clone(),
                ids[3].clone(),
                ids[1].clone()
            ]
        );

        // A finished song can't be queued again by mistake.
        set_done(&db, &ids[1], "track.mp3", "file", None, None, 30)
            .await
            .unwrap();
        set_queued(&db, &ids[1], Some(0)).await.unwrap();
        assert_eq!(get(&db, &ids[1]).await.unwrap().unwrap().state, DONE);

        // A picked source starts over.
        set_progress(&db, &ids[0], 500, Some(1_000)).await.unwrap();
        set_picked(&db, &ids[0], "{\"kind\":\"url\"}", Some("Audius"))
            .await
            .unwrap();
        let picked = get(&db, &ids[0]).await.unwrap().unwrap();
        assert_eq!(
            (
                picked.done_bytes,
                picked.total_bytes,
                picked.provider.as_deref()
            ),
            (0, None, Some("Audius"))
        );
    }
}
