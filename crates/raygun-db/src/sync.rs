//! Device sync storage (M21): which rows changed since a peer last heard from us, merging rows
//! from a peer (last writer wins per row), undoing a sync, and the peer and session bookkeeping.
//! Rows travel as JSON objects built and read by SQLite (`json_object` / `json_extract`), so one
//! code path serves every synced table.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{AssertSqlSafe, Row, Sqlite, Transaction};

use crate::{Db, DbError, persist_hlc};

/// What a user can choose to sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncKind {
    Settings,
    Library,
    History,
}

impl SyncKind {
    pub const ALL: [SyncKind; 3] = [SyncKind::Settings, SyncKind::Library, SyncKind::History];
}

/// A synced table: its key column and every column that travels.
#[derive(Debug)]
pub struct TableSpec {
    pub name: &'static str,
    pub key: &'static str,
    pub columns: &'static [&'static str],
    pub kind: SyncKind,
}

/// Setting keys kept to the device they were set on: switches each device turns on for itself,
/// and fonts, which name what's installed there.
pub const DEVICE_ONLY_SETTINGS: &[&str] = &[
    "manualBrowserChecks",
    "runSourceCode",
    "fonts",
    "checkUpdates",
    "network",
    "torrents",
    // Voices are downloaded per device.
    "readAloud",
];

const STAMP: [&str; 3] = ["updated_at_hlc", "origin_device", "deleted"];

pub const TABLES: &[TableSpec] = &[
    TableSpec {
        name: "setting",
        key: "key",
        columns: &["key", "value", STAMP[0], STAMP[1], STAMP[2]],
        kind: SyncKind::Settings,
    },
    TableSpec {
        name: "music_favorite",
        key: "id",
        columns: &[
            "id",
            "track_key",
            "title",
            "artist",
            "album",
            "duration_ms",
            "artwork_url",
            "source",
            "added_at",
            "position",
            STAMP[0],
            STAMP[1],
            STAMP[2],
        ],
        kind: SyncKind::Library,
    },
    TableSpec {
        name: "music_playlist",
        key: "id",
        columns: &["id", "name", "created_at", STAMP[0], STAMP[1], STAMP[2]],
        kind: SyncKind::Library,
    },
    TableSpec {
        name: "music_playlist_item",
        key: "id",
        columns: &[
            "id",
            "playlist_id",
            "position",
            "track_key",
            "title",
            "artist",
            "album",
            "duration_ms",
            "artwork_url",
            "source",
            "added_at",
            STAMP[0],
            STAMP[1],
            STAMP[2],
        ],
        kind: SyncKind::Library,
    },
    TableSpec {
        name: "music_play_history",
        key: "id",
        columns: &[
            "id",
            "track_key",
            "title",
            "artist",
            "album",
            "duration_ms",
            "artwork_url",
            "source",
            "played_at",
            STAMP[0],
            STAMP[1],
            STAMP[2],
        ],
        kind: SyncKind::History,
    },
];

pub fn table(name: &str) -> Option<&'static TableSpec> {
    TABLES.iter().find(|spec| spec.name == name)
}

/// One row on the wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncRow {
    pub table: String,
    pub row: Value,
}

/// Rows that changed after `after_seq`, oldest first, and the last log seq looked at. Rows of
/// kinds not in `kinds`, and rows that came from `for_peer` itself, are skipped but still move
/// the seq on.
pub async fn changes_since(
    db: &Db,
    after_seq: i64,
    kinds: &[SyncKind],
    limit: i64,
    for_peer: Option<&str>,
) -> Result<(Vec<SyncRow>, i64), DbError> {
    let entries = sqlx::query(
        "SELECT seq, tbl, row_key, from_peer FROM local_sync_log WHERE seq > ?1
         ORDER BY seq LIMIT ?2",
    )
    .bind(after_seq)
    .bind(limit)
    .fetch_all(db.pool())
    .await?;
    let mut last = after_seq;
    let mut rows = Vec::with_capacity(entries.len());
    for entry in entries {
        last = entry.get("seq");
        let from: Option<String> = entry.get("from_peer");
        if from.is_some() && from.as_deref() == for_peer {
            continue;
        }
        let tbl: String = entry.get("tbl");
        let Some(spec) = table(&tbl).filter(|spec| kinds.contains(&spec.kind)) else {
            continue;
        };
        let key: String = entry.get("row_key");
        // Settings each device turns on for itself, after its own warning, never travel.
        if spec.kind == SyncKind::Settings && DEVICE_ONLY_SETTINGS.contains(&key.as_str()) {
            continue;
        }
        if let Some(row) = read_row(db.pool(), spec, &key).await? {
            rows.push(SyncRow { table: tbl, row });
        }
    }
    Ok((rows, last))
}

/// The newest change log seq.
pub async fn latest_seq(db: &Db) -> Result<i64, DbError> {
    Ok(
        sqlx::query("SELECT COALESCE(MAX(seq), 0) AS seq FROM local_sync_log")
            .fetch_one(db.pool())
            .await?
            .get("seq"),
    )
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Applied {
    pub received: usize,
    pub changed: usize,
}

/// Merges rows from `from_peer`: a row replaces ours only when its `(updated_at_hlc,
/// origin_device)` is newer. What it replaces is saved for undo under `session_id`.
pub async fn apply(
    db: &Db,
    session_id: i64,
    rows: &[SyncRow],
    kinds: &[SyncKind],
    from_peer: &str,
) -> Result<Applied, DbError> {
    let mut applied = Applied::default();
    let mut transaction = db.pool().begin().await?;
    let mut newest = 0u64;
    for incoming in rows {
        let Some(spec) = table(&incoming.table).filter(|spec| kinds.contains(&spec.kind)) else {
            continue;
        };
        let Some(key) = text_field(&incoming.row, spec.key) else {
            continue;
        };
        if spec.kind == SyncKind::Settings && DEVICE_ONLY_SETTINGS.contains(&key.as_str()) {
            continue;
        }
        applied.received += 1;
        let hlc = incoming
            .row
            .get("updated_at_hlc")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        newest = newest.max(hlc as u64);
        let existing = read_row(&mut *transaction, spec, &key).await?;
        if let Some(existing) = &existing
            && !is_newer(&incoming.row, existing)
        {
            continue;
        }
        let mut row = incoming.row.clone();
        if spec.name == "music_favorite" && int_field(&row, "deleted") == 0 {
            row = settle_favorite(db, &mut transaction, session_id, row, &key).await?;
        }
        save_undo(&mut transaction, session_id, spec, &key, existing.as_ref()).await?;
        write_row(&mut transaction, spec, &row).await?;
        // Taken as it came: the sender has it already. A row changed here (a settled favorite)
        // is a new version and goes back.
        if row == incoming.row {
            sqlx::query("UPDATE local_sync_log SET from_peer = ?1 WHERE tbl = ?2 AND row_key = ?3")
                .bind(from_peer)
                .bind(spec.name)
                .bind(&key)
                .execute(&mut *transaction)
                .await?;
        }
        applied.changed += 1;
    }
    if newest > 0 {
        let timestamp = db.observe(newest);
        persist_hlc(&mut *transaction, timestamp).await?;
    }
    transaction.commit().await?;
    Ok(applied)
}

/// Two devices that favorite the same song make two live rows for one `track_key`. The smaller
/// id stays; the other becomes a tombstone stamped here, so every device settles on the same row.
async fn settle_favorite(
    db: &Db,
    transaction: &mut Transaction<'_, Sqlite>,
    session_id: i64,
    mut row: Value,
    key: &str,
) -> Result<Value, DbError> {
    let spec = table("music_favorite").expect("favorites are synced");
    let track_key = text_field(&row, "track_key").unwrap_or_default();
    let other: Option<String> = sqlx::query(
        "SELECT id FROM music_favorite WHERE track_key = ?1 AND deleted = 0 AND id != ?2",
    )
    .bind(&track_key)
    .bind(key)
    .fetch_optional(&mut **transaction)
    .await?
    .map(|found| found.get("id"));
    let Some(other) = other else {
        return Ok(row);
    };
    let stamp = db.tick();
    let device = db.device_id().to_string();
    if key < other.as_str() {
        let before = read_row(&mut **transaction, spec, &other).await?;
        save_undo(transaction, session_id, spec, &other, before.as_ref()).await?;
        sqlx::query(
            "UPDATE music_favorite SET deleted = 1, updated_at_hlc = ?2, origin_device = ?3
             WHERE id = ?1",
        )
        .bind(&other)
        .bind(stamp as i64)
        .bind(&device)
        .execute(&mut **transaction)
        .await?;
    } else {
        row["deleted"] = Value::from(1);
        row["updated_at_hlc"] = Value::from(stamp as i64);
        row["origin_device"] = Value::from(device);
    }
    persist_hlc(&mut **transaction, stamp).await?;
    Ok(row)
}

/// Restores what a session changed, as new local writes (so the undo reaches other devices too).
/// Returns how many rows were restored.
pub async fn undo(db: &Db, session_id: i64) -> Result<usize, DbError> {
    let entries =
        sqlx::query("SELECT tbl, row_key, old_row FROM local_sync_undo WHERE session_id = ?1")
            .bind(session_id)
            .fetch_all(db.pool())
            .await?;
    let mut transaction = db.pool().begin().await?;
    let mut restored = 0;
    for entry in entries {
        let tbl: String = entry.get("tbl");
        let Some(spec) = table(&tbl) else { continue };
        let key: String = entry.get("row_key");
        let stamp = db.tick();
        let device = db.device_id().to_string();
        let old: Option<String> = entry.get("old_row");
        let row = match old.and_then(|json| serde_json::from_str::<Value>(&json).ok()) {
            Some(mut row) => {
                row["updated_at_hlc"] = Value::from(stamp as i64);
                row["origin_device"] = Value::from(device);
                row
            }
            // The sync created this row: undoing it is a tombstone.
            None => match read_row(&mut *transaction, spec, &key).await? {
                Some(mut row) => {
                    row["deleted"] = Value::from(1);
                    row["updated_at_hlc"] = Value::from(stamp as i64);
                    row["origin_device"] = Value::from(device);
                    row
                }
                None => continue,
            },
        };
        write_row(&mut transaction, spec, &row).await?;
        persist_hlc(&mut *transaction, stamp).await?;
        restored += 1;
    }
    sqlx::query("UPDATE local_sync_session SET undone = 1 WHERE id = ?1")
        .bind(session_id)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(restored)
}

pub(crate) fn is_newer(incoming: &Value, existing: &Value) -> bool {
    let stamp = |row: &Value| {
        (
            row.get("updated_at_hlc")
                .and_then(Value::as_i64)
                .unwrap_or_default(),
            text_field(row, "origin_device").unwrap_or_default(),
        )
    };
    stamp(incoming) > stamp(existing)
}

pub(crate) async fn read_row(
    executor: impl sqlx::SqliteExecutor<'_>,
    spec: &TableSpec,
    key: &str,
) -> Result<Option<Value>, DbError> {
    let pairs = spec
        .columns
        .iter()
        .map(|column| format!("'{column}', {column}"))
        .collect::<Vec<_>>()
        .join(", ");
    // Table and column names come from the TABLES constant, never from the network.
    let sql = format!(
        "SELECT json_object({pairs}) AS row FROM {} WHERE {} = ?1",
        spec.name, spec.key
    );
    let found = sqlx::query(AssertSqlSafe(sql))
        .bind(key)
        .fetch_optional(executor)
        .await?;
    Ok(found
        .map(|row| row.get::<String, _>("row"))
        .and_then(|json| serde_json::from_str(&json).ok()))
}

pub(crate) async fn write_row(
    transaction: &mut Transaction<'_, Sqlite>,
    spec: &TableSpec,
    row: &Value,
) -> Result<(), DbError> {
    let columns = spec.columns.join(", ");
    let values = spec
        .columns
        .iter()
        .map(|column| format!("json_extract(?1, '$.{column}')"))
        .collect::<Vec<_>>()
        .join(", ");
    let updates = spec
        .columns
        .iter()
        .filter(|column| **column != spec.key)
        .map(|column| format!("{column} = excluded.{column}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO {} ({columns}) VALUES ({values})
         ON CONFLICT ({}) DO UPDATE SET {updates}",
        spec.name, spec.key
    );
    sqlx::query(AssertSqlSafe(sql))
        .bind(row.to_string())
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

async fn save_undo(
    transaction: &mut Transaction<'_, Sqlite>,
    session_id: i64,
    spec: &TableSpec,
    key: &str,
    before: Option<&Value>,
) -> Result<(), DbError> {
    // The first change in a session records the state before the session.
    sqlx::query(
        "INSERT OR IGNORE INTO local_sync_undo (session_id, tbl, row_key, old_row)
         VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(session_id)
    .bind(spec.name)
    .bind(key)
    .bind(before.map(Value::to_string))
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

pub(crate) fn text_field(row: &Value, field: &str) -> Option<String> {
    match row.get(field)? {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn int_field(row: &Value, field: &str) -> i64 {
    row.get(field).and_then(Value::as_i64).unwrap_or_default()
}

// ---- Peers ----

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    pub device_id: String,
    pub name: String,
    pub public_key: String,
    pub address: Option<String>,
    pub paired_at: i64,
    pub last_sync_at: Option<i64>,
    pub sent_seq: i64,
    /// The kinds last sent to this peer.
    pub kinds: Vec<SyncKind>,
}

pub async fn peers(db: &Db) -> Result<Vec<Peer>, DbError> {
    let rows = sqlx::query(
        "SELECT device_id, name, public_key, address, paired_at, last_sync_at, sent_seq, kinds
         FROM local_sync_peer ORDER BY paired_at",
    )
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .iter()
        .map(|row| Peer {
            device_id: row.get("device_id"),
            name: row.get("name"),
            public_key: row.get("public_key"),
            address: row.get("address"),
            paired_at: row.get("paired_at"),
            last_sync_at: row.get("last_sync_at"),
            sent_seq: row.get("sent_seq"),
            kinds: serde_json::from_str(&row.get::<String, _>("kinds")).unwrap_or_default(),
        })
        .collect())
}

pub async fn peer(db: &Db, device_id: &str) -> Result<Option<Peer>, DbError> {
    Ok(peers(db)
        .await?
        .into_iter()
        .find(|peer| peer.device_id == device_id))
}

/// Adds or re-pairs a device. Re-pairing keeps nothing of the old watermark: the peer may have
/// lost its data, so everything is sent again.
pub async fn save_peer(db: &Db, peer: &Peer) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO local_sync_peer (device_id, name, public_key, address, paired_at, sent_seq)
         VALUES (?1, ?2, ?3, ?4, ?5, 0)
         ON CONFLICT (device_id) DO UPDATE SET name = excluded.name,
             public_key = excluded.public_key, address = excluded.address,
             paired_at = excluded.paired_at, sent_seq = 0, kinds = ''",
    )
    .bind(&peer.device_id)
    .bind(&peer.name)
    .bind(&peer.public_key)
    .bind(&peer.address)
    .bind(peer.paired_at)
    .execute(db.pool())
    .await?;
    Ok(())
}

pub async fn remove_peer(db: &Db, device_id: &str) -> Result<(), DbError> {
    sqlx::query("DELETE FROM local_sync_peer WHERE device_id = ?1")
        .bind(device_id)
        .execute(db.pool())
        .await?;
    Ok(())
}

/// The peer acknowledged our changes of `kinds` up to `seq`.
pub async fn set_sent_seq(
    db: &Db,
    device_id: &str,
    seq: i64,
    kinds: &[SyncKind],
) -> Result<(), DbError> {
    let kinds = serde_json::to_string(kinds).unwrap_or_default();
    sqlx::query(
        "UPDATE local_sync_peer SET sent_seq = MAX(sent_seq, ?2), kinds = ?3 WHERE device_id = ?1",
    )
    .bind(device_id)
    .bind(seq)
    .bind(kinds)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// Records where the peer was reached (or found) and when it last synced.
pub async fn peer_seen(
    db: &Db,
    device_id: &str,
    address: Option<&str>,
    synced_at: Option<i64>,
) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE local_sync_peer SET address = COALESCE(?2, address),
             last_sync_at = COALESCE(?3, last_sync_at)
         WHERE device_id = ?1",
    )
    .bind(device_id)
    .bind(address)
    .bind(synced_at)
    .execute(db.pool())
    .await?;
    Ok(())
}

// ---- Sessions ----

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub id: i64,
    pub peer_id: String,
    pub peer_name: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub sent: i64,
    pub received: i64,
    pub changed: i64,
    pub error: Option<String>,
    pub undone: bool,
}

pub async fn start_session(
    db: &Db,
    peer_id: &str,
    peer_name: &str,
    now_ms: i64,
) -> Result<i64, DbError> {
    Ok(sqlx::query(
        "INSERT INTO local_sync_session (peer_id, peer_name, started_at) VALUES (?1, ?2, ?3)",
    )
    .bind(peer_id)
    .bind(peer_name)
    .bind(now_ms)
    .execute(db.pool())
    .await?
    .last_insert_rowid())
}

#[allow(clippy::too_many_arguments)]
pub async fn finish_session(
    db: &Db,
    id: i64,
    sent: usize,
    received: usize,
    changed: usize,
    error: Option<&str>,
    now_ms: i64,
) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE local_sync_session SET finished_at = ?2, sent = ?3, received = ?4, changed = ?5,
             error = ?6
         WHERE id = ?1",
    )
    .bind(id)
    .bind(now_ms)
    .bind(sent as i64)
    .bind(received as i64)
    .bind(changed as i64)
    .bind(error)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// The latest sessions, newest first.
pub async fn sessions(db: &Db, limit: i64) -> Result<Vec<Session>, DbError> {
    let rows = sqlx::query(
        "SELECT id, peer_id, peer_name, started_at, finished_at, sent, received, changed, error,
             undone
         FROM local_sync_session ORDER BY id DESC LIMIT ?1",
    )
    .bind(limit)
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .iter()
        .map(|row| Session {
            id: row.get("id"),
            peer_id: row.get("peer_id"),
            peer_name: row.get("peer_name"),
            started_at: row.get("started_at"),
            finished_at: row.get("finished_at"),
            sent: row.get("sent"),
            received: row.get("received"),
            changed: row.get("changed"),
            error: row.get("error"),
            undone: row.get::<i64, _>("undone") != 0,
        })
        .collect())
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;
