//! Websites the user added (M46.3), in their order, with where each was left and whether its
//! ad blocker is on.

use sqlx::{AssertSqlSafe, Row};
use uuid::Uuid;

use crate::{Db, DbError, persist_hlc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Website {
    pub id: String,
    pub name: String,
    pub url: String,
    pub last_url: Option<String>,
    pub block_ads: bool,
    pub position: i64,
}

fn website(row: &sqlx::sqlite::SqliteRow) -> Website {
    Website {
        id: row.get("id"),
        name: row.get("name"),
        url: row.get("url"),
        last_url: row.get("user_last_url"),
        block_ads: row.get::<i64, _>("user_block_ads") != 0,
        position: row.get("position"),
    }
}

const COLUMNS: &str = "id, name, url, user_last_url, user_block_ads, position";

pub async fn list(db: &Db) -> Result<Vec<Website>, DbError> {
    let rows = sqlx::query(AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM website WHERE deleted = 0 ORDER BY position, name COLLATE NOCASE"
    )))
    .fetch_all(db.pool())
    .await?;
    Ok(rows.iter().map(website).collect())
}

pub async fn get(db: &Db, id: &str) -> Result<Option<Website>, DbError> {
    let row = sqlx::query(AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM website WHERE id = ?1 AND deleted = 0"
    )))
    .bind(id)
    .fetch_optional(db.pool())
    .await?;
    Ok(row.as_ref().map(website))
}

/// Adds a site at the end; returns its id.
pub async fn add(db: &Db, name: &str, url: &str) -> Result<String, DbError> {
    let stamp = db.tick();
    let id = Uuid::now_v7().to_string();
    let mut transaction = db.pool().begin().await?;
    let next: i64 = sqlx::query(
        "SELECT COALESCE(MAX(position), -1) + 1 AS next FROM website WHERE deleted = 0",
    )
    .fetch_one(&mut *transaction)
    .await?
    .get("next");
    sqlx::query(
        "INSERT INTO website (id, name, url, user_last_url, user_block_ads, position,
                              updated_at_hlc, origin_device, deleted)
         VALUES (?1, ?2, ?3, NULL, 1, ?4, ?5, ?6, 0)",
    )
    .bind(&id)
    .bind(name)
    .bind(url)
    .bind(next)
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(id)
}

/// Renames a site or changes its address (which forgets where it was left).
pub async fn edit(db: &Db, id: &str, name: &str, url: &str) -> Result<(), DbError> {
    let stamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "UPDATE website SET name = ?2,
                user_last_url = CASE WHEN url = ?3 THEN user_last_url ELSE NULL END,
                url = ?3, updated_at_hlc = ?4, origin_device = ?5
         WHERE id = ?1 AND deleted = 0",
    )
    .bind(id)
    .bind(name)
    .bind(url)
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(())
}

/// Where the site was left (or none, to start over), and whether its ad blocker is on.
pub async fn set_state(
    db: &Db,
    id: &str,
    last_url: Option<&str>,
    block_ads: bool,
) -> Result<(), DbError> {
    let stamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "UPDATE website SET user_last_url = ?2, user_block_ads = ?3, updated_at_hlc = ?4,
                origin_device = ?5
         WHERE id = ?1 AND deleted = 0",
    )
    .bind(id)
    .bind(last_url)
    .bind(i64::from(block_ads))
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(())
}

pub async fn remove(db: &Db, id: &str) -> Result<(), DbError> {
    let stamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "UPDATE website SET deleted = 1, updated_at_hlc = ?2, origin_device = ?3 WHERE id = ?1",
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
    async fn sites_keep_their_order_and_state() {
        let temp = TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let first = add(&db, "Archive", "https://archive.org/").await.unwrap();
        let second = add(&db, "Wiki", "https://en.wikipedia.org/").await.unwrap();
        let names: Vec<String> = list(&db)
            .await
            .unwrap()
            .into_iter()
            .map(|site| site.name)
            .collect();
        assert_eq!(names, ["Archive", "Wiki"]);

        set_state(&db, &first, Some("https://archive.org/details/x"), false)
            .await
            .unwrap();
        let site = get(&db, &first).await.unwrap().unwrap();
        assert_eq!(
            site.last_url.as_deref(),
            Some("https://archive.org/details/x")
        );
        assert!(!site.block_ads);

        // Renaming keeps where it was left; a new address forgets it.
        edit(&db, &first, "Internet Archive", "https://archive.org/")
            .await
            .unwrap();
        assert!(get(&db, &first).await.unwrap().unwrap().last_url.is_some());
        edit(&db, &first, "Internet Archive", "https://web.archive.org/")
            .await
            .unwrap();
        let site = get(&db, &first).await.unwrap().unwrap();
        assert_eq!(
            (site.name.as_str(), site.last_url),
            ("Internet Archive", None)
        );

        remove(&db, &second).await.unwrap();
        assert_eq!(list(&db).await.unwrap().len(), 1);
        assert!(get(&db, &second).await.unwrap().is_none());
    }
}
