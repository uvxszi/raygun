//! Songs saved into the app (M46.10): one row per song key, worked one at a time by the app's
//! download service. Device-local; these rows never sync, as the files stay on this device.
//!
//! The queue runs by `queued_at`, oldest first: a song tried again or given another source goes
//! first, and the queue can be put in any order. A saved copy remembers where it came from
//! (M46.49), and a song downloading again keeps its copy (`replacing`) until the new one is done.

use sqlx::Row;
use sqlx::sqlite::SqliteRow;
use uuid::Uuid;

use crate::{Db, DbError};

pub const QUEUED: &str = "queued";
pub const DOWNLOADING: &str = "downloading";
pub const DONE: &str = "done";
pub const FAILED: &str = "failed";

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

/// Where a saved copy came from (M46.49).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Origin {
    /// The stream's provider: an extension's id, `own` for the source the song was asked with,
    /// `picked` for one picked for it, `ytdlp`.
    pub provider: Option<String>,
    /// The stream's id at its provider.
    pub stream: Option<String>,
    /// The stream (MediaSource JSON).
    pub source: Option<String>,
    /// The stream's own names.
    pub title: Option<String>,
    pub artist: Option<String>,
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
    pub origin: Origin,
    /// The saved file's length in ms, when known.
    pub file_ms: Option<i64>,
    /// The file still played while the song downloads again.
    pub replacing: Option<String>,
    /// What a check found (JSON).
    pub problem: Option<String>,
}

const COLUMNS: &str =
    "id, track_key, title, artist, album, duration_ms, artwork_url, source, state,
    kind, provider, file, done_bytes, total_bytes, error, error_detail, created_at,
    COALESCE(queued_at, created_at) AS queued_at, finished_at, origin_provider, origin_stream,
    origin_source, origin_title, origin_artist, file_ms, replacing, problem";

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
        origin: Origin {
            provider: row.get("origin_provider"),
            stream: row.get("origin_stream"),
            source: row.get("origin_source"),
            title: row.get("origin_title"),
            artist: row.get("origin_artist"),
        },
        file_ms: row.get("file_ms"),
        replacing: row.get("replacing"),
        problem: row.get("problem"),
    }
}

/// Queues a song. One already queued, running or done is left alone; a failed one goes back to
/// the end of the queue, with the source it's asked with now. Returns its id and whether anything
/// changed.
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
    .await?
    .rows_affected()
        > 0;
    let row = sqlx::query("SELECT id FROM local_music_download WHERE track_key = ?1")
        .bind(&song.track_key)
        .fetch_one(db.pool())
        .await?;
    Ok((row.get("id"), changed))
}

/// The song first in the queue: of the songs to record (M46.52: [`RECORDING`], a queue of their
/// own beside the files), or of the others.
pub async fn next_queued(db: &Db, recordings: bool) -> Result<Option<SongDownloadRow>, DbError> {
    let sql = format!(
        "SELECT {COLUMNS} FROM local_music_download
         WHERE state = 'queued' AND (COALESCE(kind, '') = 'recording') = ?1
         ORDER BY COALESCE(queued_at, created_at), created_at, id LIMIT 1"
    );
    let row = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(recordings)
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

/// Songs with a file that plays. A song downloading again plays the copy it keeps meanwhile.
pub async fn done(db: &Db) -> Result<Vec<SongDownloadRow>, DbError> {
    let sql = format!(
        "SELECT {COLUMNS} FROM local_music_download
         WHERE file IS NOT NULL AND (state = 'done' OR replacing IS NOT NULL)"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .fetch_all(db.pool())
        .await?;
    Ok(rows.iter().map(from_row).collect())
}

/// Ids of the downloads that stopped with an error, in queue order.
pub async fn failed(db: &Db) -> Result<Vec<String>, DbError> {
    let rows = sqlx::query(
        "SELECT id FROM local_music_download WHERE state = 'failed'
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
             finished_at = CASE WHEN replacing IS NULL THEN NULL ELSE finished_at END,
             queued_at = COALESCE(?2, queued_at, created_at)
         WHERE id = ?1 AND state <> 'done'",
    )
    .bind(id)
    .bind(queued_at)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// Downloads a saved song again (M46.49), at `queued_at`: with `keep`, its file keeps playing
/// until the new one is done (else it's gone or damaged, and simply replaced). `source` (JSON) is
/// what it downloads from now, when it was given one. False when it isn't a saved song (anymore).
pub async fn start_replacing(
    db: &Db,
    id: &str,
    source: Option<&str>,
    queued_at: i64,
    keep: bool,
) -> Result<bool, DbError> {
    let updated = sqlx::query(
        "UPDATE local_music_download SET state = 'queued',
             replacing = CASE WHEN ?4 THEN file ELSE NULL END,
             source = COALESCE(?2, source), error = NULL, error_detail = NULL, problem = NULL,
             done_bytes = 0, total_bytes = NULL, queued_at = ?3,
             finished_at = CASE WHEN ?4 THEN finished_at ELSE NULL END
         WHERE id = ?1 AND state = 'done' AND file IS NOT NULL",
    )
    .bind(id)
    .bind(source)
    .bind(queued_at)
    .bind(keep)
    .execute(db.pool())
    .await?
    .rows_affected();
    Ok(updated > 0)
}

/// Stops downloading a song again: it stays saved as it was. Returns the file it keeps (its size
/// is the caller's to set again, as what downloaded meanwhile counted over it).
pub async fn keep_replaced(db: &Db, id: &str) -> Result<Option<String>, DbError> {
    let row = sqlx::query(
        "UPDATE local_music_download SET state = 'done', file = replacing, replacing = NULL,
             error = NULL, error_detail = NULL
         WHERE id = ?1 AND replacing IS NOT NULL RETURNING file",
    )
    .bind(id)
    .fetch_optional(db.pool())
    .await?;
    Ok(row.map(|row| row.get("file")))
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

/// A saved song's file: its name, how it was made (`file`, `capture`), the name of where it came
/// from, its size and length, and its origin.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Saved {
    pub file: String,
    pub kind: String,
    pub provider: Option<String>,
    pub bytes: Option<i64>,
    pub file_ms: Option<i64>,
    pub origin: Origin,
}

/// A finished song (a copy it kept while downloading again is replaced). False when the download
/// was removed meanwhile.
pub async fn set_done(db: &Db, id: &str, saved: &Saved, now_ms: i64) -> Result<bool, DbError> {
    let updated = sqlx::query(
        "UPDATE local_music_download SET state = 'done', file = ?2, kind = ?3, provider = ?4,
             done_bytes = COALESCE(?5, done_bytes), total_bytes = COALESCE(?5, total_bytes),
             error = NULL, error_detail = NULL, finished_at = ?6, file_ms = ?7,
             origin_provider = ?8, origin_stream = ?9, origin_source = ?10, origin_title = ?11,
             origin_artist = ?12, replacing = NULL, problem = NULL
         WHERE id = ?1",
    )
    .bind(id)
    .bind(&saved.file)
    .bind(&saved.kind)
    .bind(&saved.provider)
    .bind(saved.bytes)
    .bind(now_ms)
    .bind(saved.file_ms)
    .bind(&saved.origin.provider)
    .bind(&saved.origin.stream)
    .bind(&saved.origin.source)
    .bind(&saved.origin.title)
    .bind(&saved.origin.artist)
    .execute(db.pool())
    .await?
    .rows_affected();
    Ok(updated > 0)
}

/// A download that stopped with `error` (and the sources' own errors in `detail`). A song that
/// was downloading again stays saved as it was, the error saying why it wasn't replaced.
pub async fn set_failed(
    db: &Db,
    id: &str,
    error: &str,
    detail: Option<&str>,
    now_ms: i64,
) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE local_music_download SET
             state = CASE WHEN replacing IS NULL THEN 'failed' ELSE 'done' END,
             file = COALESCE(replacing, file),
             finished_at = CASE WHEN replacing IS NULL THEN ?4 ELSE finished_at END,
             replacing = NULL, error = ?2, error_detail = ?3
         WHERE id = ?1",
    )
    .bind(id)
    .bind(error)
    .bind(detail)
    .bind(now_ms)
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

/// The source a download is asked with (JSON), or none: the resolver finds one by its names.
pub async fn set_source(db: &Db, id: &str, source: Option<&str>) -> Result<(), DbError> {
    sqlx::query("UPDATE local_music_download SET source = ?2 WHERE id = ?1")
        .bind(id)
        .bind(source)
        .execute(db.pool())
        .await?;
    Ok(())
}

/// Forgets how a queued download is made (a capture first), for one tried again from scratch.
pub async fn clear_kind(db: &Db, id: &str) -> Result<(), DbError> {
    sqlx::query("UPDATE local_music_download SET kind = NULL WHERE id = ?1 AND state <> 'done'")
        .bind(id)
        .execute(db.pool())
        .await?;
    Ok(())
}

/// How a download is made: [`RECORDING`] for a song that waits to be recorded.
pub async fn set_kind(db: &Db, id: &str, kind: Option<&str>) -> Result<(), DbError> {
    sqlx::query("UPDATE local_music_download SET kind = ?2 WHERE id = ?1 AND state <> 'done'")
        .bind(id)
        .bind(kind)
        .execute(db.pool())
        .await?;
    Ok(())
}

/// The kind of a song recorded from YouTube's player (M46.52), or waiting to be: these wait in a
/// queue of their own ([`next_queued`]).
pub const RECORDING: &str = "recording";

/// Songs that stopped waiting to be recorded go back to the queue, at the back, except those
/// saying `except` (the person stopped them; they wait for Retry). Returns how many.
pub async fn requeue_recordings_except(db: &Db, except: &str, now_ms: i64) -> Result<u64, DbError> {
    Ok(sqlx::query(
        "UPDATE local_music_download
            SET state = 'queued', error = NULL, error_detail = NULL, finished_at = NULL,
                queued_at = ?2
          WHERE state = 'failed' AND kind = 'recording' AND COALESCE(error, '') <> ?1",
    )
    .bind(except)
    .bind(now_ms)
    .execute(db.pool())
    .await?
    .rows_affected())
}

/// Stops songs waiting to be recorded (`id`'s, else all): queued ones, and failed ones whose
/// error starts with `partial` (they'd try again by themselves). They say `error` and wait for
/// Retry. Returns how many.
pub async fn stop_recordings(
    db: &Db,
    id: Option<&str>,
    error: &str,
    partial: &str,
    now_ms: i64,
) -> Result<u64, DbError> {
    Ok(sqlx::query(
        "UPDATE local_music_download
            SET state = 'failed', error = ?2, error_detail = NULL, finished_at = ?4
          WHERE kind = 'recording' AND (?1 IS NULL OR id = ?1)
            AND (state = 'queued' OR (state = 'failed' AND substr(error, 1, length(?3)) = ?3))",
    )
    .bind(id)
    .bind(error)
    .bind(partial)
    .bind(now_ms)
    .execute(db.pool())
    .await?
    .rows_affected())
}

/// A note on a queued song saying why it waits (M46.52: a recording waiting for Raygun's window),
/// or none. It goes once the song starts.
pub async fn set_note(db: &Db, id: &str, note: Option<&str>) -> Result<(), DbError> {
    sqlx::query("UPDATE local_music_download SET error = ?2 WHERE id = ?1 AND state = 'queued'")
        .bind(id)
        .bind(note)
        .execute(db.pool())
        .await?;
    Ok(())
}

/// How many songs wait to be recorded (`queued`: those queued or being recorded instead).
pub async fn recordings(db: &Db, queued: bool) -> Result<u32, DbError> {
    let states = if queued {
        "('queued', 'downloading')"
    } else {
        "('failed')"
    };
    let sql = format!(
        "SELECT COUNT(*) FROM local_music_download WHERE kind = 'recording' AND state IN {states}"
    );
    let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .fetch_one(db.pool())
        .await?;
    Ok(u32::try_from(count).unwrap_or(u32::MAX))
}

/// What a check found wrong with a saved song (JSON), or nothing.
pub async fn set_problem(db: &Db, id: &str, problem: Option<&str>) -> Result<bool, DbError> {
    let updated = sqlx::query(
        "UPDATE local_music_download SET problem = ?2
         WHERE id = ?1 AND state = 'done' AND problem IS NOT ?2",
    )
    .bind(id)
    .bind(problem)
    .execute(db.pool())
    .await?
    .rows_affected();
    Ok(updated > 0)
}

/// Downloads cut off by the last exit go back to the queue. Returns how many.
pub async fn reset_interrupted(db: &Db) -> Result<u64, DbError> {
    Ok(
        sqlx::query("UPDATE local_music_download SET state = 'queued' WHERE state = 'downloading'")
            .execute(db.pool())
            .await?
            .rows_affected(),
    )
}

/// Songs earlier versions gave up on because only YouTube had them go back to the queue, at the
/// back (M46.51: YouTube's own audio files download now). Returns how many.
pub async fn requeue_youtube_failures(db: &Db, now_ms: i64) -> Result<u64, DbError> {
    Ok(sqlx::query(
        "UPDATE local_music_download
            SET state = 'queued', error = NULL, error_detail = NULL, kind = NULL,
                finished_at = NULL, done_bytes = 0, total_bytes = NULL, queued_at = ?1
          WHERE state = 'failed' AND error LIKE 'Only YouTube has this song%'",
    )
    .bind(now_ms)
    .execute(db.pool())
    .await?
    .rows_affected())
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

    fn saved(file: &str, bytes: i64) -> Saved {
        Saved {
            file: file.into(),
            kind: "file".into(),
            provider: Some("Archive".into()),
            bytes: Some(bytes),
            file_ms: Some(200_000),
            origin: Origin {
                provider: Some("archive".into()),
                stream: Some("item-1".into()),
                source: Some("{\"kind\":\"url\"}".into()),
                title: Some("One".into()),
                artist: Some("A".into()),
            },
        }
    }

    /// The songs with a file that plays: `(id, key, file)`.
    async fn playable(db: &Db) -> Vec<(String, String, String)> {
        done(db)
            .await
            .unwrap()
            .into_iter()
            .map(|row| (row.id, row.song.track_key, row.file.unwrap_or_default()))
            .collect()
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
        assert_eq!(next_queued(&db, false).await.unwrap().unwrap().id, first);

        set_state(&db, &first, DOWNLOADING).await.unwrap();
        assert_eq!(next_queued(&db, false).await.unwrap().unwrap().id, second);
        assert_eq!(reset_interrupted(&db).await.unwrap(), 1);
        assert_eq!(get(&db, &first).await.unwrap().unwrap().state, QUEUED);

        set_progress(&db, &first, 10, Some(100)).await.unwrap();
        set_done(&db, &first, &saved("track.mp3", 4_000), 9)
            .await
            .unwrap();
        assert_eq!(
            playable(&db).await,
            vec![(first.clone(), "a|one".to_owned(), "track.mp3".to_owned())]
        );
        let finished = get(&db, &first).await.unwrap().unwrap();
        assert_eq!(
            (
                finished.done_bytes,
                finished.total_bytes,
                finished.finished_at,
                finished.file_ms
            ),
            (4_000, Some(4_000), Some(9), Some(200_000)),
            "a finished song keeps its size and length"
        );
        assert_eq!(
            finished.origin,
            saved("", 0).origin,
            "and where it came from"
        );

        set_failed(&db, &second, "No source has it", Some("Audius: 403"), 10)
            .await
            .unwrap();
        let failed_row = get(&db, &second).await.unwrap().unwrap();
        assert_eq!(failed_row.state, FAILED);
        assert_eq!(failed_row.error_detail.as_deref(), Some("Audius: 403"));
        assert_eq!(failed_row.finished_at, Some(10));
        assert_eq!(failed(&db).await.unwrap(), vec![second.clone()]);
        // A failed song asked again with a source goes back to the queue with it.
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

    #[tokio::test]
    async fn a_song_downloading_again_keeps_its_copy_until_the_new_one_is_done() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let (id, _) = enqueue(&db, &song("a|one"), 1).await.unwrap();
        set_done(&db, &id, &saved("track.mp3", 4_000), 5)
            .await
            .unwrap();
        assert!(
            set_problem(&db, &id, Some("{\"kind\":\"missing\"}"))
                .await
                .unwrap()
        );
        assert!(
            !set_problem(&db, &id, Some("{\"kind\":\"missing\"}"))
                .await
                .unwrap(),
            "unchanged"
        );

        assert!(
            start_replacing(&db, &id, Some("{\"kind\":\"url\",\"url\":\"b\"}"), 7, true)
                .await
                .unwrap()
        );
        let row = get(&db, &id).await.unwrap().unwrap();
        assert_eq!(row.state, QUEUED);
        assert_eq!(row.replacing.as_deref(), Some("track.mp3"));
        assert_eq!(row.problem, None, "the check's finding is answered");
        assert_eq!(row.queued_at, 7);
        assert_eq!(
            playable(&db).await,
            vec![(id.clone(), "a|one".to_owned(), "track.mp3".to_owned())],
            "the copy still plays"
        );
        assert!(
            !start_replacing(&db, &id, None, 8, true).await.unwrap(),
            "only saved songs"
        );

        // It fails: the copy stays, saying why it wasn't replaced.
        set_state(&db, &id, DOWNLOADING).await.unwrap();
        set_failed(&db, &id, "Audius didn't answer", None, 20)
            .await
            .unwrap();
        let row = get(&db, &id).await.unwrap().unwrap();
        assert_eq!(
            (row.state.as_str(), row.file.as_deref(), row.replacing),
            (DONE, Some("track.mp3"), None)
        );
        assert_eq!(row.error.as_deref(), Some("Audius didn't answer"));
        assert_eq!(row.finished_at, Some(5), "still saved when it was");

        // Again, and it's done: the new file and where it came from.
        assert!(start_replacing(&db, &id, None, 30, true).await.unwrap());
        let mut flac = saved("track.flac", 9_000);
        flac.kind = "capture".into();
        flac.origin.provider = Some("audius".into());
        set_done(&db, &id, &flac, 40).await.unwrap();
        let row = get(&db, &id).await.unwrap().unwrap();
        assert_eq!(
            (row.file.as_deref(), row.kind.as_deref(), row.replacing),
            (Some("track.flac"), Some("capture"), None)
        );
        assert_eq!(row.origin.provider.as_deref(), Some("audius"));
        assert_eq!(row.error, None);

        // Downloading again can be called off: it stays as it was.
        assert!(start_replacing(&db, &id, None, 50, true).await.unwrap());
        assert_eq!(
            keep_replaced(&db, &id).await.unwrap().as_deref(),
            Some("track.flac")
        );
        assert_eq!(get(&db, &id).await.unwrap().unwrap().state, DONE);
        assert_eq!(keep_replaced(&db, &id).await.unwrap(), None);
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
        set_failed(&db, &ids[3], "The server answered 503", None, 20)
            .await
            .unwrap();
        assert_eq!(failed(&db).await.unwrap(), vec![ids[3].clone()]);
        let front = queue_front(&db).await.unwrap().unwrap();
        set_queued(&db, &ids[3], Some(front - 1)).await.unwrap();
        assert_eq!(next_queued(&db, false).await.unwrap().unwrap().id, ids[3]);
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
        set_done(&db, &ids[1], &saved("track.mp3", 1), 30)
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

    #[tokio::test]
    async fn songs_that_waited_to_be_recorded_are_captured_or_failed() {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

        let temp = tempfile::TempDir::new().unwrap();
        let options = SqliteConnectOptions::new()
            .filename(temp.path().join("old.db"))
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        for script in [
            include_str!("../migrations/0020_music_downloads.sql"),
            include_str!("../migrations/0033_song_download_queue.sql"),
        ] {
            sqlx::raw_sql(script).execute(&pool).await.unwrap();
        }
        let hls = r#"{"kind":"url","url":"https://radio.example/a.m3u8","headers":{}}"#;
        let youtube = r#"{"kind":"youTube","videoId":"abc"}"#;
        for (id, state, kind, source) in [
            ("hls", "needsRecording", None, Some(hls)),
            ("youtube", "needsRecording", None, Some(youtube)),
            ("cut", "recording", Some("recording"), Some(hls)),
            ("recorded", "done", Some("recording"), Some(youtube)),
            ("plain", "failed", None, None),
        ] {
            sqlx::query(
                "INSERT INTO local_music_download (id, track_key, title, source, state, kind,
                     error, created_at, queued_at, finished_at)
                 VALUES (?1, ?1, ?1, ?2, ?3, ?4, 'Record it as it plays', 1, 1, 2)",
            )
            .bind(id)
            .bind(source)
            .bind(state)
            .bind(kind)
            .execute(&pool)
            .await
            .unwrap();
        }
        sqlx::raw_sql(include_str!("../migrations/0037_song_download_capture.sql"))
            .execute(&pool)
            .await
            .unwrap();
        let rows: Vec<(String, String, Option<String>, Option<String>)> = sqlx::query(
            "SELECT id, state, kind, error FROM local_music_download ORDER BY created_at, id",
        )
        .fetch_all(&pool)
        .await
        .unwrap()
        .iter()
        .map(|row| (row.get(0), row.get(1), row.get(2), row.get(3)))
        .collect();
        let find = |id: &str| rows.iter().find(|row| row.0 == id).unwrap().clone();
        assert_eq!(
            find("hls"),
            ("hls".into(), "queued".into(), Some("capture".into()), None),
            "a stream that plays is captured"
        );
        assert_eq!(
            find("cut"),
            ("cut".into(), "queued".into(), Some("capture".into()), None)
        );
        let youtube = find("youtube");
        assert_eq!((youtube.1.as_str(), youtube.2), ("failed", None));
        assert!(youtube.3.unwrap().starts_with("Only YouTube has this song"));
        assert_eq!(
            find("recorded").2.as_deref(),
            Some("capture"),
            "a recorded copy counts as captured"
        );
        assert_eq!(find("plain").1, "failed", "others stay as they were");
    }

    #[tokio::test]
    async fn recordings_wait_in_a_queue_of_their_own() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let (file, _) = enqueue(&db, &song("a|file"), 1).await.unwrap();
        let (recording, _) = enqueue(&db, &song("a|embed"), 2).await.unwrap();
        set_kind(&db, &recording, Some(RECORDING)).await.unwrap();
        assert_eq!(next_queued(&db, false).await.unwrap().unwrap().id, file);
        assert_eq!(next_queued(&db, true).await.unwrap().unwrap().id, recording);
        assert_eq!(recordings(&db, true).await.unwrap(), 1);

        // Why it waits shows while it's queued, and goes once it starts.
        set_note(&db, &recording, Some("Waits until Raygun is open."))
            .await
            .unwrap();
        assert_eq!(
            get(&db, &recording)
                .await
                .unwrap()
                .unwrap()
                .error
                .as_deref(),
            Some("Waits until Raygun is open.")
        );
        set_state(&db, &recording, DOWNLOADING).await.unwrap();
        assert_eq!(get(&db, &recording).await.unwrap().unwrap().error, None);
        assert!(next_queued(&db, true).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn stopped_recordings_wait_for_retry_and_the_rest_go_on_at_start() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let (partial, _) = enqueue(&db, &song("a|partial"), 1).await.unwrap();
        let (queued, _) = enqueue(&db, &song("a|queued"), 2).await.unwrap();
        let (waited, _) = enqueue(&db, &song("a|window"), 3).await.unwrap();
        let (other, _) = enqueue(&db, &song("a|other"), 4).await.unwrap();
        for id in [&partial, &queued, &waited] {
            set_kind(&db, id, Some(RECORDING)).await.unwrap();
        }
        set_failed(&db, &partial, "Partly recorded: 1:10 of 3:20.", None, 5)
            .await
            .unwrap();
        // M46.50's wait for Raygun's window.
        set_failed(
            &db,
            &waited,
            "Waits to be recorded until Raygun is open.",
            None,
            5,
        )
        .await
        .unwrap();
        set_failed(&db, &other, "No source has this song.", None, 5)
            .await
            .unwrap();

        // Stop one: only it.
        assert_eq!(
            stop_recordings(&db, Some(&queued), "Stopped", "Partly recorded", 6)
                .await
                .unwrap(),
            1
        );
        assert_eq!(get(&db, &queued).await.unwrap().unwrap().state, FAILED);
        assert_eq!(
            get(&db, &partial).await.unwrap().unwrap().error.as_deref(),
            Some("Partly recorded: 1:10 of 3:20.")
        );
        // Stop all: the one that would try again by itself too, not the one waiting for a window.
        set_queued(&db, &queued, Some(7)).await.unwrap();
        assert_eq!(
            stop_recordings(&db, None, "Stopped", "Partly recorded", 8)
                .await
                .unwrap(),
            2
        );
        assert_eq!(
            get(&db, &partial).await.unwrap().unwrap().error.as_deref(),
            Some("Stopped")
        );

        // Raygun starts: recordings go on, except stopped ones; other failures stay.
        set_failed(&db, &partial, "Partly recorded: 1:10 of 3:20.", None, 9)
            .await
            .unwrap();
        assert_eq!(
            requeue_recordings_except(&db, "Stopped", 10).await.unwrap(),
            2
        );
        for id in [&partial, &waited] {
            let row = get(&db, id).await.unwrap().unwrap();
            assert_eq!(
                (row.state.as_str(), row.kind.as_deref(), row.error),
                (QUEUED, Some(RECORDING), None)
            );
        }
        assert_eq!(get(&db, &queued).await.unwrap().unwrap().state, FAILED);
        assert_eq!(get(&db, &other).await.unwrap().unwrap().state, FAILED);
    }
    #[tokio::test]
    async fn songs_given_up_on_for_youtube_are_queued_again() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let (youtube, _) = enqueue(&db, &song("a|youtube"), 1).await.unwrap();
        let (other, _) = enqueue(&db, &song("a|other"), 2).await.unwrap();
        set_failed(
            &db,
            &youtube,
            "Only YouTube has this song, and it couldn't be saved from there. Choose another \
             source.",
            None,
            3,
        )
        .await
        .unwrap();
        set_failed(&db, &other, "No source has this song.", None, 3)
            .await
            .unwrap();

        assert_eq!(requeue_youtube_failures(&db, 10).await.unwrap(), 1);
        let row = get(&db, &youtube).await.unwrap().unwrap();
        assert_eq!(row.state, QUEUED);
        assert_eq!((row.error, row.queued_at), (None, 10));
        assert_eq!(get(&db, &other).await.unwrap().unwrap().state, FAILED);
        assert_eq!(requeue_youtube_failures(&db, 11).await.unwrap(), 0, "once");
    }
}
