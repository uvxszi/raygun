//! Storage for the episodic library (M24): titles (manga so far), their units (chapters) and
//! reading progress. Refreshing a title rewrites only its `remote_*` columns; the user's choices
//! and progress stay.

use std::collections::HashSet;

use sqlx::sqlite::SqliteRow;
use sqlx::{AssertSqlSafe, Row};
use uuid::Uuid;

use crate::{Db, DbError, persist_hlc};

/// What a source says about a title.
#[derive(Debug, Clone, PartialEq)]
pub struct TitleFields {
    pub kind: String,
    pub source_id: String,
    pub remote_id: String,
    pub title: String,
    pub cover: Option<String>,
    pub cover_headers: std::collections::BTreeMap<String, String>,
    pub authors: Vec<String>,
    pub artists: Vec<String>,
    pub description: Option<String>,
    pub genres: Vec<String>,
    pub status: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TitleRow {
    pub id: String,
    pub fields: TitleFields,
    pub in_library_at: Option<i64>,
    pub refreshed_at: Option<i64>,
    /// `webtoon`, `pagedLtr` or `pagedRtl` (M27).
    pub reader_mode: String,
}

/// A library title with its reading state.
#[derive(Debug, Clone, PartialEq)]
pub struct LibraryTitle {
    pub title: TitleRow,
    pub units: i64,
    pub unread: i64,
}

/// What a source says about a unit.
#[derive(Debug, Clone, PartialEq)]
pub struct UnitFields {
    pub remote_id: String,
    pub name: String,
    pub number: Option<f64>,
    pub volume: Option<String>,
    pub scanlator: Option<String>,
    pub date: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnitRow {
    pub id: String,
    pub title_id: String,
    pub fields: UnitFields,
    pub source_order: i64,
    pub position: Option<i64>,
    pub total: Option<i64>,
    pub completed_at: Option<i64>,
}

const TITLE_COLUMNS: &str = "t.id, t.kind, t.source_id, t.remote_id, t.remote_title, t.remote_cover, \
     t.remote_cover_headers, \
     t.remote_authors, t.remote_artists, t.remote_description, t.remote_genres, t.remote_status, \
     t.remote_url, t.user_in_library_at, t.state_refreshed_at, t.user_reader_mode";

/// Stores what a source said about a title (inserting it on first sight) and returns its id.
pub async fn upsert_title(db: &Db, fields: &TitleFields, now_ms: i64) -> Result<String, DbError> {
    let stamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    let existing: Option<String> =
        sqlx::query("SELECT id FROM title WHERE source_id = ?1 AND remote_id = ?2 AND deleted = 0")
            .bind(&fields.source_id)
            .bind(&fields.remote_id)
            .fetch_optional(&mut *transaction)
            .await?
            .map(|row| row.get("id"));
    let id = existing.unwrap_or_else(|| Uuid::now_v7().to_string());
    sqlx::query(
        "INSERT INTO title (id, kind, source_id, remote_id, remote_title, remote_cover,
             remote_authors, remote_artists, remote_description, remote_genres, remote_status,
             remote_url, state_refreshed_at, updated_at_hlc, origin_device, deleted,
             remote_cover_headers)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 0, ?16)
         ON CONFLICT (id) DO UPDATE SET
             remote_title = excluded.remote_title, remote_cover = excluded.remote_cover,
             remote_cover_headers = excluded.remote_cover_headers,
             remote_authors = excluded.remote_authors, remote_artists = excluded.remote_artists,
             remote_description = excluded.remote_description,
             remote_genres = excluded.remote_genres, remote_status = excluded.remote_status,
             remote_url = excluded.remote_url, state_refreshed_at = excluded.state_refreshed_at,
             updated_at_hlc = excluded.updated_at_hlc, origin_device = excluded.origin_device",
    )
    .bind(&id)
    .bind(&fields.kind)
    .bind(&fields.source_id)
    .bind(&fields.remote_id)
    .bind(&fields.title)
    .bind(&fields.cover)
    .bind(json(&fields.authors))
    .bind(json(&fields.artists))
    .bind(&fields.description)
    .bind(json(&fields.genres))
    .bind(&fields.status)
    .bind(&fields.url)
    .bind(now_ms)
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .bind(serde_json::to_string(&fields.cover_headers).unwrap_or_else(|_| "{}".into()))
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(id)
}

pub async fn title(db: &Db, id: &str) -> Result<Option<TitleRow>, DbError> {
    let row = sqlx::query(AssertSqlSafe(format!(
        "SELECT {TITLE_COLUMNS} FROM title t WHERE t.id = ?1 AND t.deleted = 0"
    )))
    .bind(id)
    .fetch_optional(db.pool())
    .await?;
    Ok(row.as_ref().map(title_row))
}

pub async fn find_title(
    db: &Db,
    source_id: &str,
    remote_id: &str,
) -> Result<Option<TitleRow>, DbError> {
    let row = sqlx::query(AssertSqlSafe(format!(
        "SELECT {TITLE_COLUMNS} FROM title t
         WHERE t.source_id = ?1 AND t.remote_id = ?2 AND t.deleted = 0"
    )))
    .bind(source_id)
    .bind(remote_id)
    .fetch_optional(db.pool())
    .await?;
    Ok(row.as_ref().map(title_row))
}

/// Adds a title to the library (with the time it was added) or takes it out.
pub async fn set_in_library(db: &Db, id: &str, added_at: Option<i64>) -> Result<(), DbError> {
    let stamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "UPDATE title SET user_in_library_at = ?2, updated_at_hlc = ?3, origin_device = ?4
         WHERE id = ?1 AND deleted = 0",
    )
    .bind(id)
    .bind(added_at)
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(())
}

/// Remembers how the user reads a title.
pub async fn set_reader_mode(db: &Db, id: &str, mode: &str) -> Result<(), DbError> {
    let stamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "UPDATE title SET user_reader_mode = ?2, updated_at_hlc = ?3, origin_device = ?4
         WHERE id = ?1 AND deleted = 0",
    )
    .bind(id)
    .bind(mode)
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(())
}

/// Library titles of `kind`, alphabetically, with chapter and unread counts.
pub async fn library(db: &Db, kind: &str) -> Result<Vec<LibraryTitle>, DbError> {
    let rows = sqlx::query(AssertSqlSafe(format!(
        "SELECT {TITLE_COLUMNS},
             (SELECT COUNT(*) FROM unit u WHERE u.title_id = t.id AND u.deleted = 0) AS units,
             (SELECT COUNT(*) FROM unit u
              LEFT JOIN unit_progress p ON p.unit_id = u.id AND p.deleted = 0
              WHERE u.title_id = t.id AND u.deleted = 0 AND p.completed_at IS NULL) AS unread
         FROM title t
         WHERE t.kind = ?1 AND t.user_in_library_at IS NOT NULL AND t.deleted = 0
         ORDER BY t.remote_title COLLATE NOCASE"
    )))
    .bind(kind)
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .iter()
        .map(|row| LibraryTitle {
            title: title_row(row),
            units: row.get("units"),
            unread: row.get("unread"),
        })
        .collect())
}

/// Which of a source's titles are in the library, by the source's ids.
pub async fn in_library(
    db: &Db,
    source_id: &str,
    remote_ids: &[String],
) -> Result<HashSet<String>, DbError> {
    if remote_ids.is_empty() {
        return Ok(HashSet::new());
    }
    let placeholders = vec!["?"; remote_ids.len()].join(", ");
    let sql = format!(
        "SELECT remote_id FROM title WHERE source_id = ?1 AND deleted = 0
             AND user_in_library_at IS NOT NULL AND remote_id IN ({placeholders})"
    );
    let mut query = sqlx::query(AssertSqlSafe(sql)).bind(source_id);
    for id in remote_ids {
        query = query.bind(id);
    }
    Ok(query
        .fetch_all(db.pool())
        .await?
        .iter()
        .map(|row| row.get("remote_id"))
        .collect())
}

/// A unit found by a library update, with its title (M28).
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateRow {
    pub title: TitleRow,
    pub unit: UnitRow,
    pub found_at: i64,
}

/// Stores a source's unit list for a title: new units are added (stamped `found_at = now_ms`),
/// known ones updated and reordered. Units the source no longer lists keep their rows and
/// progress. Returns how many units were new.
pub async fn sync_units(
    db: &Db,
    title_id: &str,
    units: &[UnitFields],
    kind: &str,
    now_ms: i64,
) -> Result<usize, DbError> {
    let stamp = db.tick();
    let device = db.device_id().to_string();
    let mut transaction = db.pool().begin().await?;
    // Units the source stopped listing end up after the listed ones, in their old order.
    sqlx::query(
        "UPDATE unit SET source_order = source_order + ?2 WHERE title_id = ?1 AND deleted = 0",
    )
    .bind(title_id)
    .bind(units.len() as i64)
    .execute(&mut *transaction)
    .await?;
    let mut added = 0;
    for (order, unit) in units.iter().enumerate() {
        let existing: Option<String> = sqlx::query(
            "SELECT id FROM unit WHERE title_id = ?1 AND remote_id = ?2 AND deleted = 0",
        )
        .bind(title_id)
        .bind(&unit.remote_id)
        .fetch_optional(&mut *transaction)
        .await?
        .map(|row| row.get("id"));
        added += usize::from(existing.is_none());
        let id = existing.unwrap_or_else(|| Uuid::now_v7().to_string());
        // `state_found_at` is set on insert only.
        sqlx::query(
            "INSERT INTO unit (id, title_id, kind, remote_id, remote_name, number, volume,
                 remote_scanlator, remote_date, source_order, updated_at_hlc, origin_device,
                 deleted, state_found_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0, ?13)
             ON CONFLICT (id) DO UPDATE SET
                 remote_name = excluded.remote_name, number = excluded.number,
                 volume = excluded.volume, remote_scanlator = excluded.remote_scanlator,
                 remote_date = excluded.remote_date, source_order = excluded.source_order,
                 updated_at_hlc = excluded.updated_at_hlc, origin_device = excluded.origin_device",
        )
        .bind(&id)
        .bind(title_id)
        .bind(kind)
        .bind(&unit.remote_id)
        .bind(&unit.name)
        .bind(unit.number)
        .bind(&unit.volume)
        .bind(&unit.scanlator)
        .bind(unit.date)
        .bind(order as i64)
        .bind(stamp as i64)
        .bind(&device)
        .bind(now_ms)
        .execute(&mut *transaction)
        .await?;
    }
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(added)
}

/// Units of library titles of `kind` found after their title joined the library, newest first.
pub async fn updates(db: &Db, kind: &str, limit: u32) -> Result<Vec<UpdateRow>, DbError> {
    let rows = sqlx::query(AssertSqlSafe(format!(
        "SELECT {TITLE_COLUMNS}, u.id AS unit_id, u.remote_id AS unit_remote_id, u.remote_name,
             u.number, u.volume, u.remote_scanlator, u.remote_date, u.source_order,
             u.state_found_at, p.position, p.total, p.completed_at
         FROM unit u
         JOIN title t ON t.id = u.title_id AND t.deleted = 0
         LEFT JOIN unit_progress p ON p.unit_id = u.id AND p.deleted = 0
         WHERE t.kind = ?1 AND t.user_in_library_at IS NOT NULL AND u.deleted = 0
             AND u.state_found_at > t.user_in_library_at
         ORDER BY u.state_found_at DESC, u.source_order
         LIMIT ?2"
    )))
    .bind(kind)
    .bind(limit)
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .iter()
        .map(|row| UpdateRow {
            title: title_row(row),
            unit: UnitRow {
                id: row.get("unit_id"),
                title_id: row.get("id"),
                fields: UnitFields {
                    remote_id: row.get("unit_remote_id"),
                    name: row.get("remote_name"),
                    number: row.get("number"),
                    volume: row.get("volume"),
                    scanlator: row.get("remote_scanlator"),
                    date: row.get("remote_date"),
                },
                source_order: row.get("source_order"),
                position: row.get("position"),
                total: row.get("total"),
                completed_at: row.get("completed_at"),
            },
            found_at: row.get("state_found_at"),
        })
        .collect())
}

const UNIT_SELECT: &str = "SELECT u.id, u.title_id, u.remote_id, u.remote_name, u.number, u.volume,
         u.remote_scanlator, u.remote_date, u.source_order, p.position, p.total, p.completed_at
     FROM unit u LEFT JOIN unit_progress p ON p.unit_id = u.id AND p.deleted = 0";

/// A title's units in the source's order.
pub async fn units(db: &Db, title_id: &str) -> Result<Vec<UnitRow>, DbError> {
    let rows = sqlx::query(AssertSqlSafe(format!(
        "{UNIT_SELECT} WHERE u.title_id = ?1 AND u.deleted = 0 ORDER BY u.source_order, u.id"
    )))
    .bind(title_id)
    .fetch_all(db.pool())
    .await?;
    Ok(rows.iter().map(unit_row).collect())
}

pub async fn unit(db: &Db, id: &str) -> Result<Option<UnitRow>, DbError> {
    let row = sqlx::query(AssertSqlSafe(format!(
        "{UNIT_SELECT} WHERE u.id = ?1 AND u.deleted = 0"
    )))
    .bind(id)
    .fetch_optional(db.pool())
    .await?;
    Ok(row.as_ref().map(unit_row))
}

/// Records how far a unit was read. Reaching the last page completes it; going back later
/// doesn't un-complete it.
pub async fn set_progress(
    db: &Db,
    unit_id: &str,
    position: i64,
    total: Option<i64>,
    now_ms: i64,
) -> Result<(), DbError> {
    let completed = total.is_some_and(|total| total > 0 && position >= total - 1);
    let stamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "INSERT INTO unit_progress (unit_id, position, total, completed_at, updated_at_hlc,
             origin_device, deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)
         ON CONFLICT (unit_id) DO UPDATE SET
             position = excluded.position, total = COALESCE(excluded.total, total),
             completed_at = COALESCE(completed_at, excluded.completed_at),
             updated_at_hlc = excluded.updated_at_hlc, origin_device = excluded.origin_device,
             deleted = 0",
    )
    .bind(unit_id)
    .bind(position)
    .bind(total)
    .bind(completed.then_some(now_ms))
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(())
}

/// Marks a unit read or unread outright (from the chapter list).
pub async fn set_completed(
    db: &Db,
    unit_id: &str,
    completed: bool,
    now_ms: i64,
) -> Result<(), DbError> {
    let stamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "INSERT INTO unit_progress (unit_id, position, total, completed_at, updated_at_hlc,
             origin_device, deleted)
         VALUES (?1, 0, NULL, ?2, ?3, ?4, 0)
         ON CONFLICT (unit_id) DO UPDATE SET
             completed_at = excluded.completed_at,
             position = CASE WHEN excluded.completed_at IS NULL THEN 0 ELSE position END,
             updated_at_hlc = excluded.updated_at_hlc, origin_device = excluded.origin_device,
             deleted = 0",
    )
    .bind(unit_id)
    .bind(completed.then_some(now_ms))
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(())
}

fn title_row(row: &SqliteRow) -> TitleRow {
    let list = |column: &str| -> Vec<String> {
        serde_json::from_str(&row.get::<String, _>(column)).unwrap_or_default()
    };
    TitleRow {
        id: row.get("id"),
        fields: TitleFields {
            kind: row.get("kind"),
            source_id: row.get("source_id"),
            remote_id: row.get("remote_id"),
            title: row.get("remote_title"),
            cover: row.get("remote_cover"),
            cover_headers: serde_json::from_str(&row.get::<String, _>("remote_cover_headers"))
                .unwrap_or_default(),
            authors: list("remote_authors"),
            artists: list("remote_artists"),
            description: row.get("remote_description"),
            genres: list("remote_genres"),
            status: row.get("remote_status"),
            url: row.get("remote_url"),
        },
        in_library_at: row.get("user_in_library_at"),
        refreshed_at: row.get("state_refreshed_at"),
        reader_mode: row.get("user_reader_mode"),
    }
}

fn unit_row(row: &SqliteRow) -> UnitRow {
    UnitRow {
        id: row.get("id"),
        title_id: row.get("title_id"),
        fields: UnitFields {
            remote_id: row.get("remote_id"),
            name: row.get("remote_name"),
            number: row.get("number"),
            volume: row.get("volume"),
            scanlator: row.get("remote_scanlator"),
            date: row.get("remote_date"),
        },
        source_order: row.get("source_order"),
        position: row.get("position"),
        total: row.get("total"),
        completed_at: row.get("completed_at"),
    }
}

fn json(values: &[String]) -> String {
    serde_json::to_string(values).unwrap_or_else(|_| "[]".into())
}

#[cfg(test)]
#[path = "titles_tests.rs"]
mod tests;
