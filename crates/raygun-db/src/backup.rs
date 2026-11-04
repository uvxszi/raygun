//! Backups (M30): every row of the tables worth keeping, as the same JSON objects device sync
//! sends, and a restore that merges them in (last writer wins per row). Rows for an item this
//! database already has under another id (a title fetched on both devices) take our id, and
//! their children follow.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{AssertSqlSafe, Row, Sqlite, Transaction};

use crate::sync::{SyncKind, TABLES, TableSpec, is_newer, read_row, text_field, write_row};
use crate::{Db, DbError, persist_hlc};

pub const FORMAT: &str = "raygun-backup";
pub const VERSION: u32 = 1;

const STAMP: [&str; 3] = ["updated_at_hlc", "origin_device", "deleted"];

/// Tables kept besides the synced ones, in restore order (parents first).
const EXTRA: &[TableSpec] = &[
    TableSpec {
        name: "extension_repo",
        key: "id",
        columns: &["id", "url", "name", STAMP[0], STAMP[1], STAMP[2]],
        kind: SyncKind::Library,
    },
    TableSpec {
        name: "title",
        key: "id",
        columns: &[
            "id",
            "kind",
            "source_id",
            "remote_id",
            "remote_title",
            "remote_cover",
            "remote_cover_headers",
            "remote_authors",
            "remote_artists",
            "remote_description",
            "remote_genres",
            "remote_status",
            "remote_url",
            "remote_media_type",
            "remote_year",
            "user_in_library_at",
            "user_reader_mode",
            "state_refreshed_at",
            STAMP[0],
            STAMP[1],
            STAMP[2],
        ],
        kind: SyncKind::Library,
    },
    TableSpec {
        name: "unit",
        key: "id",
        columns: &[
            "id",
            "title_id",
            "kind",
            "remote_id",
            "remote_name",
            "number",
            "volume",
            "remote_scanlator",
            "remote_date",
            "source_order",
            "state_found_at",
            STAMP[0],
            STAMP[1],
            STAMP[2],
        ],
        kind: SyncKind::Library,
    },
    TableSpec {
        name: "unit_progress",
        key: "unit_id",
        columns: &[
            "unit_id",
            "position",
            "total",
            "completed_at",
            STAMP[0],
            STAMP[1],
            STAMP[2],
        ],
        kind: SyncKind::Library,
    },
];

fn specs() -> impl Iterator<Item = &'static TableSpec> {
    TABLES.iter().chain(EXTRA.iter())
}

/// A backup's contents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupData {
    pub format: String,
    pub version: u32,
    /// Unix ms.
    pub created_at: i64,
    pub device_id: String,
    pub tables: BTreeMap<String, Vec<Value>>,
}

impl BackupData {
    /// How many live (not deleted) rows a table holds.
    pub fn live_rows(&self, table: &str) -> usize {
        self.tables.get(table).map_or(0, |rows| {
            rows.iter()
                .filter(|row| row.get("deleted").and_then(Value::as_i64) != Some(1))
                .count()
        })
    }
}

/// Every row of every backed-up table, tombstones included (so deletions win on restore too).
pub async fn export(db: &Db, now_ms: i64) -> Result<BackupData, DbError> {
    let mut tables = BTreeMap::new();
    for spec in specs() {
        let pairs = spec
            .columns
            .iter()
            .map(|column| format!("'{column}', {column}"))
            .collect::<Vec<_>>()
            .join(", ");
        // Table and column names come from the constants above.
        let sql = format!(
            "SELECT json_object({pairs}) AS row FROM {} ORDER BY {}",
            spec.name, spec.key
        );
        let rows = sqlx::query(AssertSqlSafe(sql))
            .fetch_all(db.pool())
            .await?
            .iter()
            .filter_map(|row| serde_json::from_str(&row.get::<String, _>("row")).ok())
            .collect();
        tables.insert(spec.name.to_owned(), rows);
    }
    Ok(BackupData {
        format: FORMAT.into(),
        version: VERSION,
        created_at: now_ms,
        device_id: db.device_id().to_string(),
        tables,
    })
}

/// What a restore did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Restored {
    pub received: usize,
    pub changed: usize,
}

/// Merges a backup in. Unknown tables are ignored, so newer backups restore what this version
/// knows.
pub async fn restore(db: &Db, backup: &BackupData) -> Result<Restored, DbError> {
    let mut restored = Restored::default();
    let mut transaction = db.pool().begin().await?;
    let mut newest = 0u64;
    // Backup id → our id, for rows we already hold under another id.
    let mut titles: HashMap<String, String> = HashMap::new();
    let mut units: HashMap<String, String> = HashMap::new();
    for spec in specs() {
        let Some(rows) = backup.tables.get(spec.name) else {
            continue;
        };
        for incoming in rows {
            let mut row = incoming.clone();
            match spec.name {
                "extension_repo" => {
                    adopt_id(&mut transaction, &mut row, "id", "extension_repo", &["url"]).await?;
                }
                "title" => {
                    if let Some((from, to)) = adopt_id(
                        &mut transaction,
                        &mut row,
                        "id",
                        "title",
                        &["source_id", "remote_id"],
                    )
                    .await?
                    {
                        titles.insert(from, to);
                    }
                }
                "unit" => {
                    remap(&mut row, "title_id", &titles);
                    if let Some((from, to)) = adopt_id(
                        &mut transaction,
                        &mut row,
                        "id",
                        "unit",
                        &["title_id", "remote_id"],
                    )
                    .await?
                    {
                        units.insert(from, to);
                    }
                }
                "unit_progress" => remap(&mut row, "unit_id", &units),
                // One live favorite per song (M46.48): a song favorited here too keeps our row.
                "music_favorite"
                    if row
                        .get("deleted")
                        .and_then(Value::as_i64)
                        .unwrap_or_default()
                        == 0 =>
                {
                    adopt_id(
                        &mut transaction,
                        &mut row,
                        "id",
                        "music_favorite",
                        &["track_key"],
                    )
                    .await?;
                }
                _ => {}
            }
            let Some(key) = text_field(&row, spec.key) else {
                continue;
            };
            restored.received += 1;
            newest = newest.max(
                row.get("updated_at_hlc")
                    .and_then(Value::as_i64)
                    .unwrap_or_default() as u64,
            );
            let existing = read_row(&mut *transaction, spec, &key).await?;
            if let Some(existing) = existing
                .as_ref()
                .filter(|existing| !is_newer(&row, existing))
            {
                if spec.name == "title"
                    && fill_library_gap(db, &mut transaction, &row, existing).await?
                {
                    restored.changed += 1;
                }
                continue;
            }
            write_row(&mut transaction, spec, &row).await?;
            restored.changed += 1;
        }
    }
    if newest > 0 {
        let timestamp = db.observe(newest);
        persist_hlc(&mut *transaction, timestamp).await?;
    }
    transaction.commit().await?;
    Ok(restored)
}

/// Our title row is newer (say, refreshed here) but isn't in the library while the backup's is:
/// the user's library choice comes back anyway, as a new local write.
async fn fill_library_gap(
    db: &Db,
    transaction: &mut Transaction<'_, Sqlite>,
    incoming: &Value,
    existing: &Value,
) -> Result<bool, DbError> {
    let added_at = incoming.get("user_in_library_at").and_then(Value::as_i64);
    let live = |row: &Value| row.get("deleted").and_then(Value::as_i64) == Some(0);
    let missing = existing
        .get("user_in_library_at")
        .is_none_or(Value::is_null);
    let (Some(added_at), true, true, true) = (added_at, missing, live(incoming), live(existing))
    else {
        return Ok(false);
    };
    let stamp = db.tick();
    sqlx::query(
        "UPDATE title SET user_in_library_at = ?2, updated_at_hlc = ?3, origin_device = ?4
         WHERE id = ?1",
    )
    .bind(text_field(existing, "id"))
    .bind(added_at)
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .execute(&mut **transaction)
    .await?;
    persist_hlc(&mut **transaction, stamp).await?;
    Ok(true)
}

/// When a live row of `table` matches `row` on `natural` columns under another id, gives `row`
/// that id. Returns `(backup id, our id)` when it did.
async fn adopt_id(
    transaction: &mut Transaction<'_, Sqlite>,
    row: &mut Value,
    key: &str,
    table: &str,
    natural: &[&str],
) -> Result<Option<(String, String)>, DbError> {
    let Some(incoming_id) = text_field(row, key) else {
        return Ok(None);
    };
    let conditions = natural
        .iter()
        .enumerate()
        .map(|(index, column)| format!("{column} = ?{}", index + 1))
        .collect::<Vec<_>>()
        .join(" AND ");
    // Table and column names are constants from `restore`.
    let sql = format!("SELECT {key} AS id FROM {table} WHERE {conditions} AND deleted = 0 LIMIT 1");
    let mut query = sqlx::query(AssertSqlSafe(sql));
    for column in natural {
        query = query.bind(text_field(row, column).unwrap_or_default());
    }
    let ours: Option<String> = query
        .fetch_optional(&mut **transaction)
        .await?
        .map(|found| found.get("id"));
    match ours {
        Some(ours) if ours != incoming_id => {
            row[key] = Value::from(ours.clone());
            Ok(Some((incoming_id, ours)))
        }
        _ => Ok(None),
    }
}

/// Points a reference at our id when its parent was adopted.
fn remap(row: &mut Value, field: &str, ids: &HashMap<String, String>) {
    if let Some(ours) = text_field(row, field).and_then(|current| ids.get(&current)) {
        row[field] = Value::from(ours.clone());
    }
}

#[cfg(test)]
#[path = "backup_tests.rs"]
mod tests;
