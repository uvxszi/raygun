//! Stremio addons (M41): their transport URLs, cached manifests and order.

use sqlx::Row;
use uuid::Uuid;

use crate::{Db, DbError, persist_hlc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddonRow {
    pub id: String,
    pub transport_url: String,
    pub manifest: Option<String>,
    pub fetched_at: Option<i64>,
}

/// Addons in the user's order.
pub async fn addons(db: &Db) -> Result<Vec<AddonRow>, DbError> {
    let rows = sqlx::query(
        "SELECT id, transport_url, manifest, fetched_at FROM stremio_addon WHERE deleted = 0
         ORDER BY position, transport_url",
    )
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .iter()
        .map(|row| AddonRow {
            id: row.get("id"),
            transport_url: row.get("transport_url"),
            manifest: row.get("manifest"),
            fetched_at: row.get("fetched_at"),
        })
        .collect())
}

/// Adds an addon at the end (or brings a removed one back); returns its id.
pub async fn add(
    db: &Db,
    transport_url: &str,
    manifest: &str,
    now_ms: i64,
) -> Result<String, DbError> {
    let stamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    let existing: Option<String> =
        sqlx::query("SELECT id FROM stremio_addon WHERE transport_url = ?1 AND deleted = 0")
            .bind(transport_url)
            .fetch_optional(&mut *transaction)
            .await?
            .map(|row| row.get("id"));
    let id = existing.unwrap_or_else(|| Uuid::now_v7().to_string());
    let next: i64 = sqlx::query(
        "SELECT COALESCE(MAX(position), -1) + 1 AS next FROM stremio_addon WHERE deleted = 0",
    )
    .fetch_one(&mut *transaction)
    .await?
    .get("next");
    sqlx::query(
        "INSERT INTO stremio_addon (id, transport_url, manifest, fetched_at, position,
             updated_at_hlc, origin_device, deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)
         ON CONFLICT (id) DO UPDATE SET manifest = excluded.manifest,
             fetched_at = excluded.fetched_at, updated_at_hlc = excluded.updated_at_hlc,
             origin_device = excluded.origin_device",
    )
    .bind(&id)
    .bind(transport_url)
    .bind(manifest)
    .bind(now_ms)
    .bind(next)
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(id)
}

/// Caches a freshly fetched manifest. Device-local bookkeeping: no HLC change.
pub async fn set_manifest(db: &Db, id: &str, manifest: &str, now_ms: i64) -> Result<(), DbError> {
    sqlx::query("UPDATE stremio_addon SET manifest = ?2, fetched_at = ?3 WHERE id = ?1")
        .bind(id)
        .bind(manifest)
        .bind(now_ms)
        .execute(db.pool())
        .await?;
    Ok(())
}

pub async fn remove(db: &Db, id: &str) -> Result<(), DbError> {
    let stamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "UPDATE stremio_addon SET deleted = 1, updated_at_hlc = ?2, origin_device = ?3 WHERE id = ?1",
    )
    .bind(id)
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

    #[tokio::test]
    async fn seeded_added_cached_and_removed() {
        let temp = TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let seeded = addons(&db).await.unwrap();
        assert_eq!(
            seeded
                .iter()
                .map(|row| row.transport_url.as_str())
                .collect::<Vec<_>>(),
            [
                "https://v3-cinemeta.strem.io/manifest.json",
                "https://opensubtitles-v3.strem.io/manifest.json"
            ]
        );
        assert!(seeded[0].manifest.is_none());

        let id = add(&db, "https://addon.example/manifest.json", "{}", 5)
            .await
            .unwrap();
        let again = add(&db, "https://addon.example/manifest.json", "{\"v\":2}", 6)
            .await
            .unwrap();
        assert_eq!(id, again, "adding twice keeps one row");
        set_manifest(&db, &seeded[0].id, "{\"id\":\"c\"}", 7)
            .await
            .unwrap();
        let list = addons(&db).await.unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].fetched_at, Some(7));
        assert_eq!(list[2].manifest.as_deref(), Some("{\"v\":2}"));

        remove(&db, &id).await.unwrap();
        assert_eq!(addons(&db).await.unwrap().len(), 2);
    }
}
