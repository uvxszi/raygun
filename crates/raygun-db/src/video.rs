//! Watching progress (M38): where each movie, episode or link was left, for "continue watching"
//! and resuming.

use sqlx::Row;
use sqlx::sqlite::SqliteRow;
use uuid::Uuid;

use crate::{Db, DbError, persist_hlc};

/// What is being watched, as the player was asked to play it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoEntry {
    pub key: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub poster: Option<String>,
    /// JSON the app uses to play it again.
    pub request: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoProgress {
    pub entry: VideoEntry,
    pub position_ms: i64,
    pub duration_ms: Option<i64>,
    pub watched_at: Option<i64>,
    pub updated_at: i64,
}

fn progress_row(row: &SqliteRow) -> VideoProgress {
    VideoProgress {
        entry: VideoEntry {
            key: row.get("key"),
            title: row.get("title"),
            subtitle: row.get("subtitle"),
            poster: row.get("poster"),
            request: row.get("request"),
        },
        position_ms: row.get("position_ms"),
        duration_ms: row.get("duration_ms"),
        watched_at: row.get("user_watched_at"),
        updated_at: row.get("updated_at"),
    }
}

const COLUMNS: &str = "key, title, subtitle, poster, request, position_ms, duration_ms, \
     user_watched_at, updated_at";

/// Records that `entry` started playing, keeping any progress already saved for it.
pub async fn started(db: &Db, entry: &VideoEntry, now_ms: i64) -> Result<(), DbError> {
    let stamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    let existing: Option<String> =
        sqlx::query("SELECT id FROM video_progress WHERE key = ?1 AND deleted = 0")
            .bind(&entry.key)
            .fetch_optional(&mut *transaction)
            .await?
            .map(|row| row.get("id"));
    let id = existing.unwrap_or_else(|| Uuid::now_v7().to_string());
    sqlx::query(
        "INSERT INTO video_progress (id, key, title, subtitle, poster, request, updated_at,
             updated_at_hlc, origin_device, deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0)
         ON CONFLICT (id) DO UPDATE SET
             title = excluded.title, subtitle = excluded.subtitle, poster = excluded.poster,
             request = excluded.request, updated_at = excluded.updated_at,
             updated_at_hlc = excluded.updated_at_hlc, origin_device = excluded.origin_device",
    )
    .bind(&id)
    .bind(&entry.key)
    .bind(&entry.title)
    .bind(&entry.subtitle)
    .bind(&entry.poster)
    .bind(&entry.request)
    .bind(now_ms)
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(())
}

/// Saves where watching is; `watched` marks it finished (and rewinds it for next time).
pub async fn set_progress(
    db: &Db,
    key: &str,
    position_ms: i64,
    duration_ms: Option<i64>,
    watched: bool,
    now_ms: i64,
) -> Result<(), DbError> {
    let stamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "UPDATE video_progress SET
             position_ms = CASE WHEN ?4 THEN 0 ELSE ?2 END,
             duration_ms = COALESCE(?3, duration_ms),
             user_watched_at = CASE WHEN ?4 THEN ?5 ELSE user_watched_at END,
             updated_at = ?5, updated_at_hlc = ?6, origin_device = ?7
         WHERE key = ?1 AND deleted = 0",
    )
    .bind(key)
    .bind(position_ms.max(0))
    .bind(duration_ms)
    .bind(watched)
    .bind(now_ms)
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(())
}

/// Marks something watched (`Some(now)`) or unwatched (`None`), rewinding it either way.
pub async fn set_watched(db: &Db, key: &str, at: Option<i64>) -> Result<(), DbError> {
    let stamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "UPDATE video_progress SET user_watched_at = ?2, position_ms = 0,
             updated_at_hlc = ?3, origin_device = ?4
         WHERE key = ?1 AND deleted = 0",
    )
    .bind(key)
    .bind(at)
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(())
}

pub async fn progress(db: &Db, key: &str) -> Result<Option<VideoProgress>, DbError> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM video_progress WHERE key = ?1 AND deleted = 0"
    )))
    .bind(key)
    .fetch_optional(db.pool())
    .await?;
    Ok(row.as_ref().map(progress_row))
}

/// Started but unfinished, most recent first.
pub async fn continue_watching(db: &Db, limit: u32) -> Result<Vec<VideoProgress>, DbError> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM video_progress
         WHERE deleted = 0 AND user_watched_at IS NULL AND position_ms > 0
         ORDER BY updated_at DESC LIMIT ?1"
    )))
    .bind(i64::from(limit))
    .fetch_all(db.pool())
    .await?;
    Ok(rows.iter().map(progress_row).collect())
}

/// Forgets an entry (a tombstone, so sync can carry the removal later).
pub async fn remove(db: &Db, key: &str) -> Result<(), DbError> {
    let stamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "UPDATE video_progress SET deleted = 1, updated_at_hlc = ?2, origin_device = ?3
         WHERE key = ?1 AND deleted = 0",
    )
    .bind(key)
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn entry(key: &str) -> VideoEntry {
        VideoEntry {
            key: key.into(),
            title: "Big Buck Bunny".into(),
            subtitle: None,
            poster: None,
            request: "{}".into(),
        }
    }

    #[tokio::test]
    async fn progress_resumes_and_finishes() {
        let temp = TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        started(&db, &entry("url:a"), 1).await.unwrap();
        assert!(
            continue_watching(&db, 10).await.unwrap().is_empty(),
            "not started yet"
        );

        set_progress(&db, "url:a", 60_000, Some(600_000), false, 2)
            .await
            .unwrap();
        started(&db, &entry("url:a"), 3).await.unwrap();
        let saved = progress(&db, "url:a").await.unwrap().unwrap();
        assert_eq!(
            (saved.position_ms, saved.duration_ms),
            (60_000, Some(600_000))
        );
        assert_eq!(continue_watching(&db, 10).await.unwrap().len(), 1);

        set_progress(&db, "url:a", 590_000, None, true, 4)
            .await
            .unwrap();
        let done = progress(&db, "url:a").await.unwrap().unwrap();
        assert_eq!((done.position_ms, done.watched_at), (0, Some(4)));
        assert!(continue_watching(&db, 10).await.unwrap().is_empty());

        set_watched(&db, "url:a", None).await.unwrap();
        remove(&db, "url:a").await.unwrap();
        assert!(progress(&db, "url:a").await.unwrap().is_none());
    }
}
