//! Stremio addons (M41): their transport URLs, cached manifests, order and whether they're on.

use sqlx::Row;
use uuid::Uuid;

use crate::{Db, DbError, persist_hlc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddonRow {
    pub id: String,
    pub transport_url: String,
    pub manifest: Option<String>,
    pub fetched_at: Option<i64>,
    /// Off: kept in the list, asked for nothing.
    pub enabled: bool,
}

/// Addons in their order, which is also the order details are asked in.
pub async fn addons(db: &Db) -> Result<Vec<AddonRow>, DbError> {
    let rows = sqlx::query(
        "SELECT id, transport_url, manifest, fetched_at, enabled FROM stremio_addon
         WHERE deleted = 0 ORDER BY position, transport_url",
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
            enabled: row.get::<i64, _>("enabled") != 0,
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
             updated_at_hlc, origin_device, deleted, enabled)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 1)
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

/// Points an addon at a new address (the same addon set up again), keeping its place.
pub async fn replace_url(
    db: &Db,
    id: &str,
    transport_url: &str,
    manifest: &str,
    now_ms: i64,
) -> Result<(), DbError> {
    let stamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    // Another row with the new address would be a duplicate: it goes.
    sqlx::query(
        "UPDATE stremio_addon SET deleted = 1, updated_at_hlc = ?3, origin_device = ?4
         WHERE transport_url = ?1 AND id != ?2 AND deleted = 0",
    )
    .bind(transport_url)
    .bind(id)
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "UPDATE stremio_addon SET transport_url = ?2, manifest = ?3, fetched_at = ?4,
             updated_at_hlc = ?5, origin_device = ?6
         WHERE id = ?1",
    )
    .bind(id)
    .bind(transport_url)
    .bind(manifest)
    .bind(now_ms)
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(())
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

pub async fn set_enabled(db: &Db, id: &str, enabled: bool) -> Result<(), DbError> {
    let stamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "UPDATE stremio_addon SET enabled = ?2, updated_at_hlc = ?3, origin_device = ?4
         WHERE id = ?1 AND deleted = 0",
    )
    .bind(id)
    .bind(enabled)
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(())
}

/// Puts the addons in the order of `ids`; addons not named keep their order after them.
pub async fn set_order(db: &Db, ids: &[String]) -> Result<(), DbError> {
    let current = addons(db).await?;
    let mut ordered: Vec<&str> = ids
        .iter()
        .map(String::as_str)
        .filter(|id| current.iter().any(|row| row.id == *id))
        .collect();
    for row in &current {
        if !ordered.contains(&row.id.as_str()) {
            ordered.push(&row.id);
        }
    }
    let stamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    for (position, id) in ordered.iter().enumerate() {
        sqlx::query(
            "UPDATE stremio_addon SET position = ?2, updated_at_hlc = ?3, origin_device = ?4
             WHERE id = ?1 AND position != ?2",
        )
        .bind(id)
        .bind(position as i64)
        .bind(stamp as i64)
        .bind(db.device_id().to_string())
        .execute(&mut *transaction)
        .await?;
    }
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
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

    const CINEMETA: &str = "https://v3-cinemeta.strem.io/manifest.json";
    const SUBTITLES: &str = "https://opensubtitles-v3.strem.io/manifest.json";

    #[tokio::test]
    async fn seeded_added_cached_and_removed() {
        let temp = TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        // A new install starts with the subtitles addon only.
        let seeded = addons(&db).await.unwrap();
        assert_eq!(
            seeded
                .iter()
                .map(|row| row.transport_url.as_str())
                .collect::<Vec<_>>(),
            [SUBTITLES]
        );
        assert!(seeded[0].manifest.is_none());
        assert!(seeded.iter().all(|row| row.enabled), "addons start on");

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
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].fetched_at, Some(7));
        assert_eq!(list[1].manifest.as_deref(), Some("{\"v\":2}"));

        remove(&db, &id).await.unwrap();
        assert_eq!(addons(&db).await.unwrap().len(), 1);
    }

    /// An install from before Cinemeta stopped being seeded keeps it, wherever it was moved.
    #[tokio::test]
    async fn an_install_that_has_cinemeta_keeps_it() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("raygun.db");
        {
            // The database as the last version left it: every earlier migration, then a start.
            let options = sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true);
            let pool = sqlx::SqlitePool::connect_with(options).await.unwrap();
            let mut earlier = sqlx::migrate!("./migrations");
            earlier.migrations = earlier
                .migrations
                .iter()
                .filter(|migration| migration.version < 38)
                .cloned()
                .collect::<Vec<_>>()
                .into();
            earlier.run(&pool).await.unwrap();
            sqlx::query("INSERT INTO local_meta (key, value) VALUES ('device_id', ?1)")
                .bind(uuid::Uuid::now_v7().to_string())
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("UPDATE stremio_addon SET position = 5 WHERE transport_url = ?1")
                .bind(CINEMETA)
                .execute(&pool)
                .await
                .unwrap();
            pool.close().await;
        }
        let db = Db::open(&path).await.unwrap();
        let kept: Vec<String> = addons(&db)
            .await
            .unwrap()
            .into_iter()
            .map(|row| row.transport_url)
            .collect();
        assert_eq!(kept, [SUBTITLES, CINEMETA]);
    }

    #[tokio::test]
    async fn turned_off_reordered_and_set_up_again() {
        let temp = TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let tmdb = add(&db, "https://tmdb.example/manifest.json", "{}", 1)
            .await
            .unwrap();
        let catalog = add(&db, "https://catalog.example/manifest.json", "{}", 1)
            .await
            .unwrap();
        let subtitles = addons(&db).await.unwrap()[0].id.clone();

        set_enabled(&db, &subtitles, false).await.unwrap();
        // TMDB first: its details win from now on.
        set_order(&db, &[tmdb.clone(), catalog.clone()])
            .await
            .unwrap();
        let list = addons(&db).await.unwrap();
        let order: Vec<&str> = list.iter().map(|row| row.id.as_str()).collect();
        assert_eq!(order, [tmdb.as_str(), catalog.as_str(), subtitles.as_str()]);
        assert!(!list[2].enabled);

        // Set up again on its website: a new address, the same place.
        add(&db, "https://tmdb.example/cfg/manifest.json", "{}", 2)
            .await
            .unwrap();
        replace_url(
            &db,
            &tmdb,
            "https://tmdb.example/cfg/manifest.json",
            "{\"v\":3}",
            3,
        )
        .await
        .unwrap();
        let list = addons(&db).await.unwrap();
        assert_eq!(list.len(), 3, "the duplicate row is gone");
        assert_eq!(list[0].id, tmdb);
        assert_eq!(
            list[0].transport_url,
            "https://tmdb.example/cfg/manifest.json"
        );
        assert_eq!(list[0].manifest.as_deref(), Some("{\"v\":3}"));
    }
}
