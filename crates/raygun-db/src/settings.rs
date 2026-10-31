//! Storage for user settings: one row per key, value as JSON text. Typing and validation live in
//! the settings service; this layer only stores and stamps rows.

use sqlx::Row;

use crate::{Db, DbError, persist_hlc};

/// All live (non-deleted) settings as `(key, json)` pairs.
pub async fn load(db: &Db) -> Result<Vec<(String, String)>, DbError> {
    let rows = sqlx::query("SELECT key, value FROM setting WHERE deleted = 0 ORDER BY key")
        .fetch_all(db.pool())
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.get("key"), row.get("value")))
        .collect())
}

/// Writes one setting, stamped with a fresh HLC timestamp and this device's id.
pub async fn put(db: &Db, key: &str, value_json: &str) -> Result<(), DbError> {
    let timestamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "INSERT INTO setting (key, value, updated_at_hlc, origin_device, deleted)
         VALUES (?1, ?2, ?3, ?4, 0)
         ON CONFLICT (key) DO UPDATE SET
             value = excluded.value,
             updated_at_hlc = excluded.updated_at_hlc,
             origin_device = excluded.origin_device,
             deleted = 0",
    )
    .bind(key)
    .bind(value_json)
    .bind(timestamp as i64)
    .bind(db.device_id().to_string())
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, timestamp).await?;
    transaction.commit().await?;
    Ok(())
}
