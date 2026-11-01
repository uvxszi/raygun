//! The chapter download queue (M28), in the device-local `local_manga_download` table. Rows move
//! queued → downloading → done, or → failed; the files themselves live under the downloads folder.

use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::{Db, DbError};

pub const QUEUED: &str = "queued";
pub const DOWNLOADING: &str = "downloading";
pub const DONE: &str = "done";
pub const FAILED: &str = "failed";

#[derive(Debug, Clone, PartialEq)]
pub struct DownloadRow {
    pub unit_id: String,
    pub title_id: String,
    pub state: String,
    pub pages_done: i64,
    pub pages_total: Option<i64>,
    pub error: Option<String>,
    pub queued_at: i64,
    pub finished_at: Option<i64>,
}

/// A queue row with the names the downloads screen shows.
#[derive(Debug, Clone, PartialEq)]
pub struct DownloadEntry {
    pub download: DownloadRow,
    pub title: String,
    pub chapter: String,
}

const COLUMNS: &str = "d.unit_id, d.title_id, d.state, d.pages_done, d.pages_total, d.error, \
     d.queued_at, d.finished_at";

/// Queues a unit. A failed one is queued again; queued, running and finished ones are left as
/// they are. Returns whether anything changed.
pub async fn enqueue(db: &Db, unit_id: &str, title_id: &str, now_ms: i64) -> Result<bool, DbError> {
    let result = sqlx::query(
        "INSERT INTO local_manga_download (unit_id, title_id, state, queued_at)
         VALUES (?1, ?2, 'queued', ?3)
         ON CONFLICT (unit_id) DO UPDATE SET
             state = 'queued', pages_done = 0, error = NULL, queued_at = excluded.queued_at,
             finished_at = NULL
         WHERE local_manga_download.state = 'failed'",
    )
    .bind(unit_id)
    .bind(title_id)
    .bind(now_ms)
    .execute(db.pool())
    .await?;
    Ok(result.rows_affected() > 0)
}

/// The oldest queued unit.
pub async fn next_queued(db: &Db) -> Result<Option<DownloadRow>, DbError> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM local_manga_download d
         WHERE d.state = 'queued' ORDER BY d.queued_at, d.unit_id LIMIT 1"
    )))
    .fetch_optional(db.pool())
    .await?;
    Ok(row.as_ref().map(download_row))
}

pub async fn get(db: &Db, unit_id: &str) -> Result<Option<DownloadRow>, DbError> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM local_manga_download d WHERE d.unit_id = ?1"
    )))
    .bind(unit_id)
    .fetch_optional(db.pool())
    .await?;
    Ok(row.as_ref().map(download_row))
}

/// Every row of one title.
pub async fn for_title(db: &Db, title_id: &str) -> Result<Vec<DownloadRow>, DbError> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM local_manga_download d WHERE d.title_id = ?1"
    )))
    .bind(title_id)
    .fetch_all(db.pool())
    .await?;
    Ok(rows.iter().map(download_row).collect())
}

/// Rows of the given units (any that have one).
pub async fn for_units(db: &Db, unit_ids: &[String]) -> Result<Vec<DownloadRow>, DbError> {
    if unit_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; unit_ids.len()].join(", ");
    let sql =
        format!("SELECT {COLUMNS} FROM local_manga_download d WHERE d.unit_id IN ({placeholders})");
    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
    for id in unit_ids {
        query = query.bind(id);
    }
    Ok(query
        .fetch_all(db.pool())
        .await?
        .iter()
        .map(download_row)
        .collect())
}

/// The whole queue with names, in queue order.
pub async fn entries(db: &Db) -> Result<Vec<DownloadEntry>, DbError> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS}, t.remote_title, u.remote_name
         FROM local_manga_download d
         JOIN unit u ON u.id = d.unit_id
         JOIN title t ON t.id = d.title_id
         ORDER BY d.queued_at, d.unit_id"
    )))
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .iter()
        .map(|row| DownloadEntry {
            download: download_row(row),
            title: row.get("remote_title"),
            chapter: row.get("remote_name"),
        })
        .collect())
}

pub async fn start(db: &Db, unit_id: &str) -> Result<(), DbError> {
    set_state(db, unit_id, DOWNLOADING, None).await
}

pub async fn set_progress(db: &Db, unit_id: &str, done: i64, total: i64) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE local_manga_download SET pages_done = ?2, pages_total = ?3 WHERE unit_id = ?1",
    )
    .bind(unit_id)
    .bind(done)
    .bind(total)
    .execute(db.pool())
    .await?;
    Ok(())
}

pub async fn finish(db: &Db, unit_id: &str, now_ms: i64) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE local_manga_download SET state = 'done', error = NULL, finished_at = ?2,
             pages_done = COALESCE(pages_total, pages_done)
         WHERE unit_id = ?1",
    )
    .bind(unit_id)
    .bind(now_ms)
    .execute(db.pool())
    .await?;
    Ok(())
}

pub async fn fail(db: &Db, unit_id: &str, error: &str) -> Result<(), DbError> {
    set_state(db, unit_id, FAILED, Some(error)).await
}

/// Puts a running download back in the queue (paused, or interrupted).
pub async fn requeue(db: &Db, unit_id: &str) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE local_manga_download SET state = 'queued', pages_done = 0
         WHERE unit_id = ?1 AND state = 'downloading'",
    )
    .bind(unit_id)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// After a restart nothing is running: downloads cut off mid-way go back to the queue.
pub async fn reset_interrupted(db: &Db) -> Result<u64, DbError> {
    let result = sqlx::query(
        "UPDATE local_manga_download SET state = 'queued', pages_done = 0
         WHERE state = 'downloading'",
    )
    .execute(db.pool())
    .await?;
    Ok(result.rows_affected())
}

pub async fn remove(db: &Db, unit_id: &str) -> Result<(), DbError> {
    sqlx::query("DELETE FROM local_manga_download WHERE unit_id = ?1")
        .bind(unit_id)
        .execute(db.pool())
        .await?;
    Ok(())
}

async fn set_state(
    db: &Db,
    unit_id: &str,
    state: &str,
    error: Option<&str>,
) -> Result<(), DbError> {
    sqlx::query("UPDATE local_manga_download SET state = ?2, error = ?3 WHERE unit_id = ?1")
        .bind(unit_id)
        .bind(state)
        .bind(error)
        .execute(db.pool())
        .await?;
    Ok(())
}

fn download_row(row: &SqliteRow) -> DownloadRow {
    DownloadRow {
        unit_id: row.get("unit_id"),
        title_id: row.get("title_id"),
        state: row.get("state"),
        pages_done: row.get("pages_done"),
        pages_total: row.get("pages_total"),
        error: row.get("error"),
        queued_at: row.get("queued_at"),
        finished_at: row.get("finished_at"),
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::titles::{TitleFields, UnitFields, sync_units, units, upsert_title};

    async fn setup(temp: &TempDir) -> (Db, String, Vec<String>) {
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let title = upsert_title(
            &db,
            &TitleFields {
                kind: "manga".into(),
                source_id: "s".into(),
                remote_id: "m1".into(),
                title: "Frieren".into(),
                cover: None,
                cover_headers: Default::default(),
                authors: vec![],
                artists: vec![],
                description: None,
                genres: vec![],
                status: "ongoing".into(),
                url: None,
            },
            1,
        )
        .await
        .unwrap();
        let chapter = |id: &str| UnitFields {
            remote_id: id.into(),
            name: format!("Chapter {id}"),
            number: None,
            volume: None,
            scanlator: None,
            date: None,
        };
        sync_units(&db, &title, &[chapter("2"), chapter("1")], "chapter", 1)
            .await
            .unwrap();
        let ids = units(&db, &title)
            .await
            .unwrap()
            .into_iter()
            .map(|unit| unit.id)
            .collect();
        (db, title, ids)
    }

    #[tokio::test]
    async fn moves_through_the_queue_in_order() {
        let temp = TempDir::new().unwrap();
        let (db, title, ids) = setup(&temp).await;
        assert!(enqueue(&db, &ids[1], &title, 10).await.unwrap());
        assert!(enqueue(&db, &ids[0], &title, 11).await.unwrap());
        assert!(
            !enqueue(&db, &ids[1], &title, 12).await.unwrap(),
            "already queued"
        );

        let next = next_queued(&db).await.unwrap().unwrap();
        assert_eq!(next.unit_id, ids[1], "oldest first");
        start(&db, &next.unit_id).await.unwrap();
        set_progress(&db, &next.unit_id, 3, 10).await.unwrap();
        assert_eq!(next_queued(&db).await.unwrap().unwrap().unit_id, ids[0]);
        finish(&db, &next.unit_id, 20).await.unwrap();
        let done = get(&db, &ids[1]).await.unwrap().unwrap();
        assert_eq!(
            (done.state.as_str(), done.pages_done, done.finished_at),
            (DONE, 10, Some(20))
        );
        assert!(
            !enqueue(&db, &ids[1], &title, 21).await.unwrap(),
            "done stays done"
        );

        start(&db, &ids[0]).await.unwrap();
        fail(&db, &ids[0], "HTTP 404").await.unwrap();
        assert_eq!(next_queued(&db).await.unwrap(), None);
        let entries = entries(&db).await.unwrap();
        assert_eq!(entries[1].download.error.as_deref(), Some("HTTP 404"));
        assert_eq!(
            (entries[0].title.as_str(), entries[0].chapter.as_str()),
            ("Frieren", "Chapter 1")
        );
        assert!(
            enqueue(&db, &ids[0], &title, 30).await.unwrap(),
            "failed ones retry"
        );
        assert_eq!(get(&db, &ids[0]).await.unwrap().unwrap().error, None);
        assert_eq!(for_title(&db, &title).await.unwrap().len(), 2);
        assert_eq!(for_units(&db, &ids[..1]).await.unwrap().len(), 1);

        remove(&db, &ids[0]).await.unwrap();
        assert_eq!(get(&db, &ids[0]).await.unwrap(), None);
    }

    #[tokio::test]
    async fn interrupted_downloads_go_back_to_the_queue() {
        let temp = TempDir::new().unwrap();
        let (db, title, ids) = setup(&temp).await;
        enqueue(&db, &ids[0], &title, 10).await.unwrap();
        start(&db, &ids[0]).await.unwrap();
        set_progress(&db, &ids[0], 4, 9).await.unwrap();
        assert_eq!(reset_interrupted(&db).await.unwrap(), 1);
        let row = get(&db, &ids[0]).await.unwrap().unwrap();
        assert_eq!((row.state.as_str(), row.pages_done), (QUEUED, 0));

        start(&db, &ids[0]).await.unwrap();
        requeue(&db, &ids[0]).await.unwrap();
        assert_eq!(next_queued(&db).await.unwrap().unwrap().unit_id, ids[0]);
    }
}
