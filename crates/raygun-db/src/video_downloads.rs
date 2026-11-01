//! The video download queue (M43), in the device-local `local_video_download` table. Rows move
//! queued → downloading → done, or → failed; the files live under `downloads/video/<id>/`.

use sqlx::Row;
use sqlx::sqlite::SqliteRow;
use uuid::Uuid;

use crate::{Db, DbError};

pub const QUEUED: &str = "queued";
pub const DOWNLOADING: &str = "downloading";
pub const DONE: &str = "done";
pub const FAILED: &str = "failed";

/// What to download: a link of an episode or film, with what the player shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewDownload {
    pub key: String,
    pub title_id: Option<String>,
    pub title: String,
    pub subtitle: Option<String>,
    pub poster: Option<String>,
    pub url: String,
    /// JSON object.
    pub headers: String,
    pub format: Option<String>,
    /// JSON array of the remote subtitle tracks.
    pub subtitles: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoDownloadRow {
    pub id: String,
    pub download: NewDownload,
    pub state: String,
    pub file: Option<String>,
    /// JSON array of the saved tracks.
    pub local_subtitles: String,
    pub done_bytes: i64,
    pub total_bytes: Option<i64>,
    pub done_parts: i64,
    pub total_parts: Option<i64>,
    pub error: Option<String>,
    pub queued_at: i64,
    pub finished_at: Option<i64>,
}

const COLUMNS: &str = "id, key, title_id, title, subtitle, poster, url, headers, format, subtitles, \
     state, file, local_subtitles, done_bytes, total_bytes, done_parts, total_parts, error, \
     queued_at, finished_at";

fn download_row(row: &SqliteRow) -> VideoDownloadRow {
    VideoDownloadRow {
        id: row.get("id"),
        download: NewDownload {
            key: row.get("key"),
            title_id: row.get("title_id"),
            title: row.get("title"),
            subtitle: row.get("subtitle"),
            poster: row.get("poster"),
            url: row.get("url"),
            headers: row.get("headers"),
            format: row.get("format"),
            subtitles: row.get("subtitles"),
        },
        state: row.get("state"),
        file: row.get("file"),
        local_subtitles: row.get("local_subtitles"),
        done_bytes: row.get("done_bytes"),
        total_bytes: row.get("total_bytes"),
        done_parts: row.get("done_parts"),
        total_parts: row.get("total_parts"),
        error: row.get("error"),
        queued_at: row.get("queued_at"),
        finished_at: row.get("finished_at"),
    }
}

/// Queues a download, returning its id. A key already queued, running or done keeps its row
/// (and id); a failed one is queued again with the new link.
pub async fn enqueue(db: &Db, new: &NewDownload, now_ms: i64) -> Result<String, DbError> {
    let mut transaction = db.pool().begin().await?;
    let existing: Option<(String, String)> =
        sqlx::query("SELECT id, state FROM local_video_download WHERE key = ?1")
            .bind(&new.key)
            .fetch_optional(&mut *transaction)
            .await?
            .map(|row| (row.get("id"), row.get("state")));
    let id = match existing {
        Some((id, state)) if state != FAILED => {
            transaction.commit().await?;
            return Ok(id);
        }
        Some((id, _)) => id,
        None => Uuid::now_v7().to_string(),
    };
    sqlx::query(
        "INSERT INTO local_video_download (id, key, title_id, title, subtitle, poster, url,
             headers, format, subtitles, state, queued_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'queued', ?11)
         ON CONFLICT (id) DO UPDATE SET
             url = excluded.url, headers = excluded.headers, format = excluded.format,
             subtitles = excluded.subtitles, state = 'queued', done_bytes = 0, total_bytes = NULL,
             done_parts = 0, total_parts = NULL, error = NULL, file = NULL,
             queued_at = excluded.queued_at, finished_at = NULL",
    )
    .bind(&id)
    .bind(&new.key)
    .bind(&new.title_id)
    .bind(&new.title)
    .bind(&new.subtitle)
    .bind(&new.poster)
    .bind(&new.url)
    .bind(&new.headers)
    .bind(&new.format)
    .bind(&new.subtitles)
    .bind(now_ms)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(id)
}

/// The oldest queued download.
pub async fn next_queued(db: &Db) -> Result<Option<VideoDownloadRow>, DbError> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM local_video_download
         WHERE state = 'queued' ORDER BY queued_at, id LIMIT 1"
    )))
    .fetch_optional(db.pool())
    .await?;
    Ok(row.as_ref().map(download_row))
}

pub async fn get(db: &Db, id: &str) -> Result<Option<VideoDownloadRow>, DbError> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM local_video_download WHERE id = ?1"
    )))
    .bind(id)
    .fetch_optional(db.pool())
    .await?;
    Ok(row.as_ref().map(download_row))
}

/// The finished download playing under `key`, if there is one.
pub async fn done_for_key(db: &Db, key: &str) -> Result<Option<VideoDownloadRow>, DbError> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM local_video_download WHERE key = ?1 AND state = 'done'"
    )))
    .bind(key)
    .fetch_optional(db.pool())
    .await?;
    Ok(row.as_ref().map(download_row))
}

/// Every download, newest first.
pub async fn all(db: &Db) -> Result<Vec<VideoDownloadRow>, DbError> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM local_video_download ORDER BY queued_at DESC, id"
    )))
    .fetch_all(db.pool())
    .await?;
    Ok(rows.iter().map(download_row).collect())
}

/// The states of downloads under `keys` (for marking episodes), by key.
pub async fn states_for_keys(
    db: &Db,
    keys: &[String],
) -> Result<std::collections::HashMap<String, String>, DbError> {
    if keys.is_empty() {
        return Ok(Default::default());
    }
    let placeholders = (1..=keys.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!("SELECT key, state FROM local_video_download WHERE key IN ({placeholders})");
    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
    for key in keys {
        query = query.bind(key);
    }
    let rows = query.fetch_all(db.pool()).await?;
    Ok(rows
        .iter()
        .map(|row| (row.get("key"), row.get("state")))
        .collect())
}

pub async fn set_downloading(db: &Db, id: &str) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE local_video_download SET state = 'downloading', error = NULL WHERE id = ?1",
    )
    .bind(id)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// Records how far a download got: bytes for a file, segments for HLS.
pub async fn set_progress(
    db: &Db,
    id: &str,
    done_bytes: i64,
    total_bytes: Option<i64>,
    done_parts: i64,
    total_parts: Option<i64>,
) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE local_video_download SET done_bytes = ?2, total_bytes = ?3, done_parts = ?4,
             total_parts = ?5
         WHERE id = ?1",
    )
    .bind(id)
    .bind(done_bytes)
    .bind(total_bytes)
    .bind(done_parts)
    .bind(total_parts)
    .execute(db.pool())
    .await?;
    Ok(())
}

pub async fn set_done(
    db: &Db,
    id: &str,
    file: &str,
    local_subtitles: &str,
    now_ms: i64,
) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE local_video_download SET state = 'done', file = ?2, local_subtitles = ?3,
             error = NULL, finished_at = ?4
         WHERE id = ?1",
    )
    .bind(id)
    .bind(file)
    .bind(local_subtitles)
    .bind(now_ms)
    .execute(db.pool())
    .await?;
    Ok(())
}

pub async fn set_failed(db: &Db, id: &str, error: &str) -> Result<(), DbError> {
    sqlx::query("UPDATE local_video_download SET state = 'failed', error = ?2 WHERE id = ?1")
        .bind(id)
        .bind(error)
        .execute(db.pool())
        .await?;
    Ok(())
}

/// Queues a failed download again, keeping what it fetched (files are checked on resume).
pub async fn retry(db: &Db, id: &str, now_ms: i64) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE local_video_download SET state = 'queued', error = NULL, queued_at = ?2
         WHERE id = ?1 AND state = 'failed'",
    )
    .bind(id)
    .bind(now_ms)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// Puts a paused download back in the queue, in its place.
pub async fn requeue(db: &Db, id: &str) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE local_video_download SET state = 'queued' WHERE id = ?1 AND state = 'downloading'",
    )
    .bind(id)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// Puts downloads cut off by the last exit back in the queue; they resume where they stopped.
pub async fn reset_interrupted(db: &Db) -> Result<u64, DbError> {
    let result =
        sqlx::query("UPDATE local_video_download SET state = 'queued' WHERE state = 'downloading'")
            .execute(db.pool())
            .await?;
    Ok(result.rows_affected())
}

pub async fn remove(db: &Db, id: &str) -> Result<(), DbError> {
    sqlx::query("DELETE FROM local_video_download WHERE id = ?1")
        .bind(id)
        .execute(db.pool())
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn film(key: &str) -> NewDownload {
        NewDownload {
            key: key.into(),
            title_id: None,
            title: "Big Buck Bunny".into(),
            subtitle: None,
            poster: None,
            url: "https://cdn.example/film.mp4".into(),
            headers: "{}".into(),
            format: None,
            subtitles: "[]".into(),
        }
    }

    #[tokio::test]
    async fn queue_runs_finishes_fails_and_resumes() {
        let temp = TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let first = enqueue(&db, &film("url:a"), 1).await.unwrap();
        let second = enqueue(&db, &film("url:b"), 2).await.unwrap();
        assert_eq!(
            enqueue(&db, &film("url:a"), 3).await.unwrap(),
            first,
            "queued twice keeps one row"
        );
        assert_eq!(next_queued(&db).await.unwrap().unwrap().id, first);

        set_downloading(&db, &first).await.unwrap();
        set_progress(&db, &first, 50, Some(100), 0, None)
            .await
            .unwrap();
        assert_eq!(next_queued(&db).await.unwrap().unwrap().id, second);
        assert_eq!(reset_interrupted(&db).await.unwrap(), 1);
        let resumed = get(&db, &first).await.unwrap().unwrap();
        assert_eq!((resumed.state.as_str(), resumed.done_bytes), (QUEUED, 50));

        set_done(&db, &first, "video.mp4", "[]", 9).await.unwrap();
        assert_eq!(
            done_for_key(&db, "url:a")
                .await
                .unwrap()
                .unwrap()
                .file
                .as_deref(),
            Some("video.mp4")
        );
        set_failed(&db, &second, "HTTP 404").await.unwrap();
        let mut moved = film("url:b");
        moved.url = "https://cdn.example/other.mp4".into();
        assert_eq!(
            enqueue(&db, &moved, 10).await.unwrap(),
            second,
            "a failed one is retried"
        );
        let retried = get(&db, &second).await.unwrap().unwrap();
        assert_eq!(
            (retried.state.as_str(), retried.download.url.as_str()),
            (QUEUED, "https://cdn.example/other.mp4")
        );

        let states = states_for_keys(&db, &["url:a".into(), "url:b".into(), "url:c".into()])
            .await
            .unwrap();
        assert_eq!(states.len(), 2);
        assert_eq!(states["url:a"], DONE);
        remove(&db, &first).await.unwrap();
        assert_eq!(all(&db).await.unwrap().len(), 1);
    }
}
