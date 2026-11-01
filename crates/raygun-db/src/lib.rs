//! Raygun's SQLite database (ARCHITECTURE §5).
//!
//! Owns the connection pool, applies the embedded migrations and keeps the bookkeeping that device
//! sync needs: a stable per-device id and a persisted hybrid logical clock. Queries are checked at
//! runtime (`sqlx::query`), so building never needs a live database or offline query data.

pub mod backup;
pub mod collections;
pub mod extensions;
pub mod hlc;
pub mod local_music;
pub mod manga_downloads;
pub mod podcasts;
pub mod settings;
pub mod streams;
pub mod sync;
pub mod titles;

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
pub async fn local_value(db: &Db, key: &str) -> Result<Option<String>, DbError> {
    read_meta(db.pool(), key).await
}

/// Stores a device-local value, replacing any earlier one.
pub async fn set_local_value(db: &Db, key: &str, value: &str) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO local_meta (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(value)
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
