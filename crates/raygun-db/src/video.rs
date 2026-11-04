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
///
/// Saying again that something finished keeps when it was last watched: the player reports the
/// end, then the switch to the next episode or its closing, and those later reports must not make
/// the finished episode newer than the next one Continue watching moved on to.
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
             updated_at = CASE WHEN ?4 AND user_watched_at IS NOT NULL AND position_ms = 0
                 THEN updated_at ELSE ?5 END,
             user_watched_at = CASE WHEN ?4 THEN ?5 ELSE user_watched_at END,
             updated_at_hlc = ?6, origin_device = ?7
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

/// Progress for exactly these keys (a title's episodes), in no particular order.
pub async fn progress_for_keys(db: &Db, keys: &[String]) -> Result<Vec<VideoProgress>, DbError> {
    let mut found = Vec::new();
    // Under SQLite's limit on bound values, for series with thousands of episodes.
    for chunk in keys.chunks(500) {
        let marks = vec!["?"; chunk.len()].join(", ");
        let sql =
            format!("SELECT {COLUMNS} FROM video_progress WHERE deleted = 0 AND key IN ({marks})");
        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
        for key in chunk {
            query = query.bind(key);
        }
        found.extend(query.fetch_all(db.pool()).await?.iter().map(progress_row));
    }
    Ok(found)
}

/// Progress for every key starting with `prefix` (a series' episodes, M41).
pub async fn progress_with_prefix(db: &Db, prefix: &str) -> Result<Vec<VideoProgress>, DbError> {
    // `!` escapes LIKE's wildcards in the prefix.
    let escaped = prefix
        .replace('!', "!!")
        .replace('%', "!%")
        .replace('_', "!_");
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM video_progress
         WHERE deleted = 0 AND key LIKE ?1 ESCAPE '!'"
    )))
    .bind(format!("{escaped}%"))
    .fetch_all(db.pool())
    .await?;
    Ok(rows.iter().map(progress_row).collect())
}

/// Marks something watched or not, recording it first if it was never played.
pub async fn mark(db: &Db, entry: &VideoEntry, watched: bool, now_ms: i64) -> Result<(), DbError> {
    started(db, entry, now_ms).await?;
    set_watched(db, &entry.key, watched.then_some(now_ms)).await
}

/// Everything watched or started, most recent first (the Videos library, M39); of two at the
/// same moment, the unfinished one (the episode a finished one moved on to) first.
pub async fn history(db: &Db, limit: u32) -> Result<Vec<VideoProgress>, DbError> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM video_progress
         WHERE deleted = 0 AND (user_watched_at IS NOT NULL OR position_ms > 0)
         ORDER BY updated_at DESC, user_watched_at IS NOT NULL LIMIT ?1"
    )))
    .bind(i64::from(limit))
    .fetch_all(db.pool())
    .await?;
    Ok(rows.iter().map(progress_row).collect())
}

/// What Continue watching picks from, newest first: everything watched or started, and what was
/// removed (`true`), which keeps its place so a removed episode doesn't hand its title over to an
/// older one.
pub async fn continue_candidates(
    db: &Db,
    limit: u32,
) -> Result<Vec<(VideoProgress, bool)>, DbError> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS}, deleted FROM video_progress
         WHERE deleted = 1 OR user_watched_at IS NOT NULL OR position_ms > 0
         ORDER BY updated_at DESC, user_watched_at IS NOT NULL LIMIT ?1"
    )))
    .bind(i64::from(limit))
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .iter()
        .map(|row| (progress_row(row), row.get::<i64, _>("deleted") != 0))
        .collect())
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

        mark(&db, &entry("stremio:series:tt1:1:1"), true, 9)
            .await
            .unwrap();
        mark(&db, &entry("stremio:series:tt1:1:2"), false, 9)
            .await
            .unwrap();
        mark(&db, &entry("stremio:series:tt10:1:1"), true, 9)
            .await
            .unwrap();
        let episodes = progress_with_prefix(&db, "stremio:series:tt1:")
            .await
            .unwrap();
        assert_eq!(episodes.len(), 2, "tt10 doesn't match tt1:");
        assert!(
            episodes
                .iter()
                .any(|row| row.entry.key.ends_with(":1:1") && row.watched_at == Some(9))
        );

        set_watched(&db, "url:a", None).await.unwrap();
        remove(&db, "url:a").await.unwrap();
        assert!(progress(&db, "url:a").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn reporting_the_end_again_keeps_when_it_was_watched() {
        let temp = TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        started(&db, &entry("stremio:series:tt1:1:7"), 1)
            .await
            .unwrap();
        set_progress(
            &db,
            "stremio:series:tt1:1:7",
            500_000,
            Some(600_000),
            false,
            5,
        )
        .await
        .unwrap();
        set_progress(&db, "stremio:series:tt1:1:7", 600_000, None, true, 10)
            .await
            .unwrap();
        // The player's "switched" or "closed" after "ended".
        set_progress(&db, "stremio:series:tt1:1:7", 600_000, None, true, 20)
            .await
            .unwrap();
        let done = progress(&db, "stremio:series:tt1:1:7")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(done.updated_at, 10, "still watched at the end");
        assert_eq!(done.watched_at, Some(20));

        // Watching it again moves it on, and so does finishing it again.
        set_progress(&db, "stremio:series:tt1:1:7", 30_000, None, false, 30)
            .await
            .unwrap();
        set_progress(&db, "stremio:series:tt1:1:7", 600_000, None, true, 40)
            .await
            .unwrap();
        let again = progress(&db, "stremio:series:tt1:1:7")
            .await
            .unwrap()
            .unwrap();
        assert_eq!((again.updated_at, again.position_ms), (40, 0));
    }

    #[tokio::test]
    async fn progress_for_exact_keys_only() {
        let temp = TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        for key in ["source:s:e1", "source:s:e10", "source:s:e2"] {
            started(&db, &entry(key), 1).await.unwrap();
        }
        let keys: Vec<String> = ["source:s:e1", "source:s:e2", "source:s:missing"]
            .map(str::to_owned)
            .into();
        let mut found: Vec<String> = progress_for_keys(&db, &keys)
            .await
            .unwrap()
            .into_iter()
            .map(|row| row.entry.key)
            .collect();
        found.sort();
        assert_eq!(found, ["source:s:e1", "source:s:e2"]);
        let many: Vec<String> = (0..1_200)
            .map(|index| format!("source:s:x{index}"))
            .collect();
        assert!(progress_for_keys(&db, &many).await.unwrap().is_empty());
    }
}
