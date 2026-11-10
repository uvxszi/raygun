//! Raygun's SQLite database (ARCHITECTURE §5).
//!
//! Owns the connection pool, applies the embedded migrations and keeps the bookkeeping that device
//! sync needs: a stable per-device id and a persisted hybrid logical clock. Queries are checked at
//! runtime (`sqlx::query`), so building never needs a live database or offline query data.

pub mod backup;
pub mod captions;
pub mod categories;
pub mod collections;
pub mod data;
pub mod detail_edits;
pub mod extensions;
pub mod hlc;
pub mod local_folders;
pub mod local_music;
pub mod lyrics_cache;
pub mod manga_downloads;
pub mod music_downloads;
pub mod packed;
pub mod podcasts;
pub mod settings;
pub mod song_edits;
pub mod streams;
pub mod stremio;
pub mod sync;
pub mod titles;
pub mod track_durations;
pub mod track_tags;
pub mod video;
pub mod video_downloads;
pub mod websites;

use std::path::Path;
use std::str::FromStr;
use std::sync::Mutex;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::hlc::Clock;

const META_DEVICE_ID: &str = "device_id";
const META_HLC_LAST: &str = "hlc_last";

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("migration failed: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("stored {key} is corrupt: {value}")]
    CorruptMeta { key: &'static str, value: String },
    /// A value kept compressed ([`packed`]) that doesn't unpack.
    #[error("stored {key} is damaged")]
    Damaged { key: String },
}

pub struct Db {
    pool: SqlitePool,
    device_id: Uuid,
    clock: Mutex<Clock>,
}

impl Db {
    /// Opens (creating if needed) the database file and brings its schema up to date.
    pub async fn open(path: &Path) -> Result<Self, DbError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;

        let device_id = load_or_create_device_id(&pool).await?;
        let hlc_last = match read_meta(&pool, META_HLC_LAST).await? {
            Some(value) => value.parse().map_err(|_| DbError::CorruptMeta {
                key: META_HLC_LAST,
                value,
            })?,
            None => 0,
        };

        Ok(Self {
            pool,
            device_id,
            clock: Mutex::new(Clock::new(hlc_last)),
        })
    }

    /// Stable id of this device, used as `origin_device` on every synced row.
    pub fn device_id(&self) -> Uuid {
        self.device_id
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Closes all connections. SQLite checkpoints the WAL into the main file when the last
    /// connection closes, so the data folder can then be moved as a whole.
    pub async fn close(self) {
        self.pool.close().await;
    }

    /// Issues the HLC timestamp for a local write. Callers persist it with [`persist_hlc`] in the
    /// same transaction as the write it stamps.
    pub(crate) fn tick(&self) -> u64 {
        self.clock
            .lock()
            .expect("hlc mutex poisoned")
            .tick(hlc::now_ms())
    }

    /// Folds in a timestamp from another device, so later local writes order after it. Returns
    /// the clock's new value for the caller to persist with [`persist_hlc`].
    pub(crate) fn observe(&self, remote: u64) -> u64 {
        self.clock
            .lock()
            .expect("hlc mutex poisoned")
            .observe(remote, hlc::now_ms())
    }
}

/// Records the latest issued timestamp so the clock resumes past it after a restart.
pub(crate) async fn persist_hlc(
    executor: impl sqlx::SqliteExecutor<'_>,
    timestamp: u64,
) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO local_meta (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value
         WHERE CAST(excluded.value AS INTEGER) > CAST(local_meta.value AS INTEGER)",
    )
    .bind(META_HLC_LAST)
    .bind(timestamp.to_string())
    .execute(executor)
    .await?;
    Ok(())
}

async fn load_or_create_device_id(pool: &SqlitePool) -> Result<Uuid, DbError> {
    if let Some(value) = read_meta(pool, META_DEVICE_ID).await? {
        return Uuid::from_str(&value).map_err(|_| DbError::CorruptMeta {
            key: META_DEVICE_ID,
            value,
        });
    }
    let device_id = Uuid::now_v7();
    sqlx::query("INSERT INTO local_meta (key, value) VALUES (?1, ?2)")
        .bind(META_DEVICE_ID)
        .bind(device_id.to_string())
        .execute(pool)
        .await?;
    Ok(device_id)
}

/// A device-local value (`local_meta`): never synced, for secrets and per-device switches.
/// Long ones are kept compressed ([`packed`]) and come back as they were stored.
pub async fn local_value(db: &Db, key: &str) -> Result<Option<String>, DbError> {
    let Some(stored) = read_meta(db.pool(), key).await? else {
        return Ok(None);
    };
    match packed::unpack(&stored) {
        Some(std::borrow::Cow::Borrowed(_)) => Ok(Some(stored)),
        Some(std::borrow::Cow::Owned(value)) => Ok(Some(value)),
        None => Err(DbError::Damaged {
            key: key.to_owned(),
        }),
    }
}

/// Every device-local value whose key starts with `prefix`, by key, in one query (lists that keep
/// one value per item, such as installed extensions' icons, read them all at once). Damaged
/// values are left out.
pub async fn local_values_with_prefix(
    db: &Db,
    prefix: &str,
) -> Result<std::collections::HashMap<String, String>, DbError> {
    // Keys between the prefix and the prefix with its last character's successor: the primary
    // key's index answers it without reading the whole table.
    let mut end: Vec<char> = prefix.chars().collect();
    let rows = match end.pop().and_then(|last| char::from_u32(last as u32 + 1)) {
        Some(next) => {
            end.push(next);
            sqlx::query("SELECT key, value FROM local_meta WHERE key >= ?1 AND key < ?2")
                .bind(prefix)
                .bind(end.into_iter().collect::<String>())
                .fetch_all(db.pool())
                .await?
        }
        None => {
            sqlx::query("SELECT key, value FROM local_meta WHERE substr(key, 1, ?2) = ?1")
                .bind(prefix)
                .bind(prefix.chars().count() as i64)
                .fetch_all(db.pool())
                .await?
        }
    };
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let key: String = row.get("key");
            let stored: String = row.get("value");
            let value = packed::unpack(&stored)?.into_owned();
            key.starts_with(prefix).then_some((key, value))
        })
        .collect())
}

/// Stores a device-local value, replacing any earlier one.
pub async fn set_local_value(db: &Db, key: &str, value: &str) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO local_meta (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(packed::pack(value).as_ref())
    .execute(db.pool())
    .await?;
    Ok(())
}

async fn read_meta(pool: &SqlitePool, key: &str) -> Result<Option<String>, DbError> {
    let row = sqlx::query("SELECT value FROM local_meta WHERE key = ?1")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|row| row.get("value")))
}

#[cfg(test)]
mod tests;
