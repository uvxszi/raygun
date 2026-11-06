//! Storage for extension repositories, extension settings and install records (M13).

use sqlx::Row;

use crate::{Db, DbError, persist_hlc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRow {
    pub id: String,
    pub url: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRow {
    pub extension_id: String,
    pub repo_url: String,
    pub version: String,
    pub sha256: String,
    pub installed_at: i64,
}

/// Live repositories, oldest first (the official one leads).
pub async fn repos(db: &Db) -> Result<Vec<RepoRow>, DbError> {
    let rows = sqlx::query(
        "SELECT id, url, name FROM extension_repo WHERE deleted = 0 ORDER BY updated_at_hlc = 0 DESC, id",
    )
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| RepoRow {
            id: row.get("id"),
            url: row.get("url"),
            name: row.get("name"),
        })
        .collect())
}

/// Adds a repository, or revives a removed one with the same URL.
pub async fn add_repo(db: &Db, url: &str, name: &str) -> Result<RepoRow, DbError> {
    let timestamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    let id: String = sqlx::query(
        "INSERT INTO extension_repo (id, url, name, updated_at_hlc, origin_device, deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, 0)
         ON CONFLICT (url) DO UPDATE SET
             name = excluded.name,
             updated_at_hlc = excluded.updated_at_hlc,
             origin_device = excluded.origin_device,
             deleted = 0
         RETURNING id",
    )
    .bind(uuid::Uuid::now_v7().to_string())
    .bind(url)
    .bind(name)
    .bind(timestamp as i64)
    .bind(db.device_id().to_string())
    .fetch_one(&mut *transaction)
    .await?
    .get("id");
    persist_hlc(&mut *transaction, timestamp).await?;
    transaction.commit().await?;
    Ok(RepoRow {
        id,
        url: url.to_owned(),
        name: name.to_owned(),
    })
}

/// Records the name a repository's index gives itself.
pub async fn rename_repo(db: &Db, id: &str, name: &str) -> Result<(), DbError> {
    let timestamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "UPDATE extension_repo SET name = ?1, updated_at_hlc = ?2, origin_device = ?3
         WHERE id = ?4 AND deleted = 0 AND name <> ?1",
    )
    .bind(name)
    .bind(timestamp as i64)
    .bind(db.device_id().to_string())
    .bind(id)
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, timestamp).await?;
    transaction.commit().await?;
    Ok(())
}

/// Folds repositories that another one replaced into it (Paraline took over Raygun
/// Extensions and both Paraline Raygun indexes). When this device still lists one of `retired`,
/// `replacement` is added (under its id) or revived, and the retired rows are removed; either
/// way, extensions installed from a retired address count as installed from the replacement.
/// URLs compare without case or a trailing slash. Returns whether anything changed; safe to run
/// on every start.
pub async fn replace_repos(
    db: &Db,
    retired: &[&str],
    replacement: &RepoRow,
) -> Result<bool, DbError> {
    // `?1` is the JSON array of retired URLs, each compared like `same_url`.
    let retired = serde_json::Value::from(retired).to_string();
    let listed: i64 = sqlx::query(
        "SELECT (SELECT COUNT(*) FROM extension_repo WHERE deleted = 0
                 AND lower(rtrim(trim(url), '/')) IN
                     (SELECT lower(rtrim(trim(value), '/')) FROM json_each(?1)))
              + (SELECT COUNT(*) FROM local_extension_install
                 WHERE lower(rtrim(trim(repo_url), '/')) IN
                     (SELECT lower(rtrim(trim(value), '/')) FROM json_each(?1))) AS n",
    )
    .bind(&retired)
    .fetch_one(db.pool())
    .await?
    .get("n");
    if listed == 0 {
        return Ok(false);
    }
    let timestamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    let live = sqlx::query(
        "UPDATE extension_repo SET deleted = 1, updated_at_hlc = ?2, origin_device = ?3
         WHERE deleted = 0 AND lower(rtrim(trim(url), '/')) IN
             (SELECT lower(rtrim(trim(value), '/')) FROM json_each(?1))",
    )
    .bind(&retired)
    .bind(timestamp as i64)
    .bind(db.device_id().to_string())
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    if live > 0 {
        sqlx::query(
            "INSERT INTO extension_repo (id, url, name, updated_at_hlc, origin_device, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, 0)
             ON CONFLICT (url) DO UPDATE SET
                 name = excluded.name,
                 updated_at_hlc = excluded.updated_at_hlc,
                 origin_device = excluded.origin_device,
                 deleted = 0",
        )
        .bind(&replacement.id)
        .bind(&replacement.url)
        .bind(&replacement.name)
        .bind(timestamp as i64)
        .bind(db.device_id().to_string())
        .execute(&mut *transaction)
        .await?;
    }
    sqlx::query(
        "UPDATE local_extension_install SET repo_url = ?2
         WHERE lower(rtrim(trim(repo_url), '/')) IN
             (SELECT lower(rtrim(trim(value), '/')) FROM json_each(?1))",
    )
    .bind(&retired)
    .bind(&replacement.url)
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, timestamp).await?;
    transaction.commit().await?;
    Ok(true)
}

/// Tombstones a repository.
pub async fn remove_repo(db: &Db, id: &str) -> Result<(), DbError> {
    let timestamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "UPDATE extension_repo SET deleted = 1, updated_at_hlc = ?1, origin_device = ?2
         WHERE id = ?3",
    )
    .bind(timestamp as i64)
    .bind(db.device_id().to_string())
    .bind(id)
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, timestamp).await?;
    transaction.commit().await?;
    Ok(())
}

/// Every live setting value as `(extension id, key, json)`.
pub async fn settings(db: &Db) -> Result<Vec<(String, String, String)>, DbError> {
    let rows = sqlx::query(
        "SELECT extension_id, key, value FROM extension_setting WHERE deleted = 0
         ORDER BY extension_id, key",
    )
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.get("extension_id"), row.get("key"), row.get("value")))
        .collect())
}

pub async fn put_setting(
    db: &Db,
    extension_id: &str,
    key: &str,
    value_json: &str,
) -> Result<(), DbError> {
    let timestamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "INSERT INTO extension_setting (extension_id, key, value, updated_at_hlc, origin_device, deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, 0)
         ON CONFLICT (extension_id, key) DO UPDATE SET
             value = excluded.value,
             updated_at_hlc = excluded.updated_at_hlc,
             origin_device = excluded.origin_device,
             deleted = 0",
    )
    .bind(extension_id)
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

pub async fn installs(db: &Db) -> Result<Vec<InstallRow>, DbError> {
    let rows = sqlx::query(
        "SELECT extension_id, repo_url, version, sha256, installed_at FROM local_extension_install
         ORDER BY extension_id",
    )
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| InstallRow {
            extension_id: row.get("extension_id"),
            repo_url: row.get("repo_url"),
            version: row.get("version"),
            sha256: row.get("sha256"),
            installed_at: row.get("installed_at"),
        })
        .collect())
}

pub async fn record_install(db: &Db, install: &InstallRow) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO local_extension_install (extension_id, repo_url, version, sha256, installed_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (extension_id) DO UPDATE SET
             repo_url = excluded.repo_url,
             version = excluded.version,
             sha256 = excluded.sha256,
             installed_at = excluded.installed_at",
    )
    .bind(&install.extension_id)
    .bind(&install.repo_url)
    .bind(&install.version)
    .bind(&install.sha256)
    .bind(install.installed_at)
    .execute(db.pool())
    .await?;
    Ok(())
}

pub async fn remove_install(db: &Db, extension_id: &str) -> Result<(), DbError> {
    sqlx::query("DELETE FROM local_extension_install WHERE extension_id = ?1")
        .bind(extension_id)
        .execute(db.pool())
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Opens a database migrated up to (not including) `before`, lets `fill` add rows, then opens
    /// it with every migration.
    async fn upgraded<F, Fut>(before: i64, fill: F) -> (tempfile::TempDir, Db)
    where
        F: FnOnce(sqlx::SqlitePool) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("raygun.db");
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let pool = sqlx::SqlitePool::connect_with(options).await.unwrap();
        let mut earlier = sqlx::migrate!("./migrations");
        earlier.migrations = earlier
            .migrations
            .iter()
            .filter(|migration| migration.version < before)
            .cloned()
            .collect::<Vec<_>>()
            .into();
        earlier.run(&pool).await.unwrap();
        fill(pool.clone()).await;
        pool.close().await;
        let db = Db::open(&path).await.unwrap();
        (temp, db)
    }

    /// A new install starts with no repository and isn't marked for the former built-in sources.
    #[tokio::test]
    async fn a_new_install_starts_with_no_repository() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        assert!(repos(&db).await.unwrap().is_empty());
        assert_eq!(
            crate::local_value(&db, "extensions.formerBuiltIns")
                .await
                .unwrap(),
            None
        );
    }

    /// A device that used Raygun before keeps its repositories and is marked so the sources the
    /// app used to carry get installed.
    #[tokio::test]
    async fn an_upgraded_device_keeps_its_repositories_and_is_marked() {
        let (_temp, db) = upgraded(41, |pool| async move {
            sqlx::query(
                "INSERT INTO extension_repo (id, url, name, updated_at_hlc, origin_device, deleted)
                 VALUES ('p', 'https://example.org/index.json', 'Paraline', 5, 'device', 0)",
            )
            .execute(&pool)
            .await
            .unwrap();
        })
        .await;
        let urls: Vec<String> = repos(&db)
            .await
            .unwrap()
            .into_iter()
            .map(|repo| repo.url)
            .collect();
        assert_eq!(urls, ["https://example.org/index.json"]);
        assert_eq!(
            crate::local_value(&db, "extensions.formerBuiltIns")
                .await
                .unwrap()
                .as_deref(),
            Some("pending")
        );
    }

    /// A device that only ever had the untouched seed loses it, unless it installed something.
    #[tokio::test]
    async fn an_untouched_seed_goes_but_installs_still_mark_the_device() {
        let (_temp, db) = upgraded(41, |pool| async move {
            sqlx::query(
                "INSERT INTO local_extension_install
                     (extension_id, repo_url, version, sha256, installed_at)
                 VALUES ('dev.example.a', 'https://example.org/index.json', '1.0.0', 'ab', 1)",
            )
            .execute(&pool)
            .await
            .unwrap();
        })
        .await;
        assert!(repos(&db).await.unwrap().is_empty());
        assert_eq!(
            crate::local_value(&db, "extensions.formerBuiltIns")
                .await
                .unwrap()
                .as_deref(),
            Some("pending")
        );
    }
}
