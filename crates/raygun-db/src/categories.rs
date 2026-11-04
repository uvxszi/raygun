//! Library categories (M39): user-named shelves per kind of title, in the user's order, with
//! titles in any number of them.

use std::collections::HashMap;

use sqlx::Row;
use uuid::Uuid;

use crate::{Db, DbError, persist_hlc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub position: i64,
    /// Its own display and sort as JSON (M46); none follows the library's.
    pub display: Option<String>,
}

/// A kind's categories in the user's order.
pub async fn list(db: &Db, kind: &str) -> Result<Vec<Category>, DbError> {
    let rows = sqlx::query(
        "SELECT id, name, position, user_display FROM category WHERE kind = ?1 AND deleted = 0
         ORDER BY position, name COLLATE NOCASE",
    )
    .bind(kind)
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .iter()
        .map(|row| Category {
            id: row.get("id"),
            name: row.get("name"),
            position: row.get("position"),
            display: row.get("user_display"),
        })
        .collect())
}

/// Sets a category's own display and sort (JSON), or clears it (M46).
pub async fn set_display(db: &Db, id: &str, display: Option<&str>) -> Result<(), DbError> {
    let stamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "UPDATE category SET user_display = ?2, updated_at_hlc = ?3, origin_device = ?4
         WHERE id = ?1 AND deleted = 0",
    )
    .bind(id)
    .bind(display)
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(())
}

/// Adds titles to categories and takes them out of others, leaving the rest of their categories
/// alone (the bulk action of M46's selection mode).
pub async fn change_titles(
    db: &Db,
    title_ids: &[String],
    add: &[String],
    remove: &[String],
) -> Result<(), DbError> {
    let stamp = db.tick();
    let device = db.device_id().to_string();
    let mut transaction = db.pool().begin().await?;
    for title_id in title_ids {
        for category_id in add {
            sqlx::query(
                "INSERT INTO title_category (title_id, category_id, updated_at_hlc, origin_device, deleted)
                 VALUES (?1, ?2, ?3, ?4, 0)
                 ON CONFLICT (title_id, category_id) DO UPDATE SET
                     deleted = 0, updated_at_hlc = excluded.updated_at_hlc,
                     origin_device = excluded.origin_device",
            )
            .bind(title_id)
            .bind(category_id)
            .bind(stamp as i64)
            .bind(&device)
            .execute(&mut *transaction)
            .await?;
        }
        for category_id in remove {
            sqlx::query(
                "UPDATE title_category SET deleted = 1, updated_at_hlc = ?3, origin_device = ?4
                 WHERE title_id = ?1 AND category_id = ?2 AND deleted = 0",
            )
            .bind(title_id)
            .bind(category_id)
            .bind(stamp as i64)
            .bind(&device)
            .execute(&mut *transaction)
            .await?;
        }
    }
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(())
}

/// Adds a category at the end; returns its id.
pub async fn create(db: &Db, kind: &str, name: &str) -> Result<String, DbError> {
    let stamp = db.tick();
    let id = Uuid::now_v7().to_string();
    let mut transaction = db.pool().begin().await?;
    let next: i64 = sqlx::query(
        "SELECT COALESCE(MAX(position), -1) + 1 AS next FROM category WHERE kind = ?1 AND deleted = 0",
    )
    .bind(kind)
    .fetch_one(&mut *transaction)
    .await?
    .get("next");
    sqlx::query(
        "INSERT INTO category (id, kind, name, position, updated_at_hlc, origin_device, deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)",
    )
    .bind(&id)
    .bind(kind)
    .bind(name)
    .bind(next)
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(id)
}

pub async fn rename(db: &Db, id: &str, name: &str) -> Result<(), DbError> {
    let stamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "UPDATE category SET name = ?2, updated_at_hlc = ?3, origin_device = ?4
         WHERE id = ?1 AND deleted = 0",
    )
    .bind(id)
    .bind(name)
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(())
}

/// Removes a category; its titles stay in the library (in their other categories, if any).
pub async fn delete(db: &Db, id: &str) -> Result<(), DbError> {
    let stamp = db.tick();
    let device = db.device_id().to_string();
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "UPDATE category SET deleted = 1, updated_at_hlc = ?2, origin_device = ?3 WHERE id = ?1",
    )
    .bind(id)
    .bind(stamp as i64)
    .bind(&device)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "UPDATE title_category SET deleted = 1, updated_at_hlc = ?2, origin_device = ?3
         WHERE category_id = ?1 AND deleted = 0",
    )
    .bind(id)
    .bind(stamp as i64)
    .bind(&device)
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(())
}

/// Puts a kind's categories in the order of `ids` (unknown ids are ignored).
pub async fn reorder(db: &Db, kind: &str, ids: &[String]) -> Result<(), DbError> {
    let stamp = db.tick();
    let device = db.device_id().to_string();
    let mut transaction = db.pool().begin().await?;
    for (position, id) in ids.iter().enumerate() {
        sqlx::query(
            "UPDATE category SET position = ?3, updated_at_hlc = ?4, origin_device = ?5
             WHERE id = ?1 AND kind = ?2 AND deleted = 0",
        )
        .bind(id)
        .bind(kind)
        .bind(position as i64)
        .bind(stamp as i64)
        .bind(&device)
        .execute(&mut *transaction)
        .await?;
    }
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(())
}

/// Makes `category_ids` exactly the title's categories.
pub async fn set_title_categories(
    db: &Db,
    title_id: &str,
    category_ids: &[String],
) -> Result<(), DbError> {
    let stamp = db.tick();
    let device = db.device_id().to_string();
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "UPDATE title_category SET deleted = 1, updated_at_hlc = ?2, origin_device = ?3
         WHERE title_id = ?1 AND deleted = 0",
    )
    .bind(title_id)
    .bind(stamp as i64)
    .bind(&device)
    .execute(&mut *transaction)
    .await?;
    for category_id in category_ids {
        sqlx::query(
            "INSERT INTO title_category (title_id, category_id, updated_at_hlc, origin_device, deleted)
             VALUES (?1, ?2, ?3, ?4, 0)
             ON CONFLICT (title_id, category_id) DO UPDATE SET
                 deleted = 0, updated_at_hlc = excluded.updated_at_hlc,
                 origin_device = excluded.origin_device",
        )
        .bind(title_id)
        .bind(category_id)
        .bind(stamp as i64)
        .bind(&device)
        .execute(&mut *transaction)
        .await?;
    }
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(())
}

/// Each title's live categories, for a kind.
pub async fn memberships(db: &Db, kind: &str) -> Result<HashMap<String, Vec<String>>, DbError> {
    let rows = sqlx::query(
        "SELECT tc.title_id, tc.category_id FROM title_category tc
         JOIN category c ON c.id = tc.category_id AND c.deleted = 0
         WHERE tc.deleted = 0 AND c.kind = ?1
         ORDER BY c.position",
    )
    .bind(kind)
    .fetch_all(db.pool())
    .await?;
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for row in rows {
        map.entry(row.get("title_id"))
            .or_default()
            .push(row.get("category_id"));
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[tokio::test]
    async fn categories_order_rename_delete_and_memberships() {
        let temp = TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let reading = create(&db, "manga", "Reading").await.unwrap();
        let later = create(&db, "manga", "Later").await.unwrap();
        create(&db, "novel", "Novels only").await.unwrap();
        let names = |list: Vec<Category>| list.into_iter().map(|c| c.name).collect::<Vec<_>>();
        assert_eq!(
            names(list(&db, "manga").await.unwrap()),
            ["Reading", "Later"]
        );

        reorder(&db, "manga", &[later.clone(), reading.clone()])
            .await
            .unwrap();
        rename(&db, &later, "Plan to read").await.unwrap();
        assert_eq!(
            names(list(&db, "manga").await.unwrap()),
            ["Plan to read", "Reading"]
        );

        set_title_categories(&db, "t1", &[reading.clone(), later.clone()])
            .await
            .unwrap();
        set_title_categories(&db, "t2", std::slice::from_ref(&reading))
            .await
            .unwrap();
        set_title_categories(&db, "t1", std::slice::from_ref(&later))
            .await
            .unwrap();
        let members = memberships(&db, "manga").await.unwrap();
        assert_eq!(members["t1"], std::slice::from_ref(&later));
        assert_eq!(members["t2"], std::slice::from_ref(&reading));

        delete(&db, &reading).await.unwrap();
        let members = memberships(&db, "manga").await.unwrap();
        assert!(
            !members.contains_key("t2"),
            "a deleted category's titles are uncategorized"
        );
        assert_eq!(names(list(&db, "manga").await.unwrap()), ["Plan to read"]);
    }

    #[tokio::test]
    async fn each_medium_keeps_its_own_groups_and_folders() {
        let temp = TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let comics = create(&db, "manga", "Shonen").await.unwrap();
        let books = create(&db, "novel", "Classics").await.unwrap();
        let folder = create(&db, "video", "Anime").await.unwrap();
        let later = create(&db, "video", "Later").await.unwrap();
        let names = |list: Vec<Category>| list.into_iter().map(|c| c.name).collect::<Vec<_>>();
        assert_eq!(names(list(&db, "manga").await.unwrap()), ["Shonen"]);
        assert_eq!(names(list(&db, "novel").await.unwrap()), ["Classics"]);
        assert_eq!(names(list(&db, "video").await.unwrap()), ["Anime", "Later"]);
        // Positions count per medium: the first folder is first though comics came before.
        assert_eq!(list(&db, "video").await.unwrap()[0].position, 0);

        // A bulk add puts many titles into a folder at once; reordering one medium leaves the
        // others alone.
        change_titles(
            &db,
            &["v1".to_owned(), "v2".to_owned()],
            &[folder.clone(), later.clone()],
            &[],
        )
        .await
        .unwrap();
        change_titles(&db, &["m1".to_owned()], std::slice::from_ref(&comics), &[])
            .await
            .unwrap();
        reorder(&db, "video", &[later.clone(), folder.clone()])
            .await
            .unwrap();
        assert_eq!(names(list(&db, "video").await.unwrap()), ["Later", "Anime"]);
        assert_eq!(names(list(&db, "manga").await.unwrap()), ["Shonen"]);

        let videos = memberships(&db, "video").await.unwrap();
        assert_eq!(videos["v1"], [later.clone(), folder.clone()]);
        assert_eq!(videos["v2"], [later.clone(), folder.clone()]);
        assert!(!videos.contains_key("m1"), "comics' groups aren't folders");
        assert_eq!(memberships(&db, "manga").await.unwrap()["m1"], [comics]);
        assert!(memberships(&db, "novel").await.unwrap().is_empty());

        // Adding again changes nothing; deleting a folder tombstones its memberships only.
        change_titles(&db, &["v1".to_owned()], std::slice::from_ref(&folder), &[])
            .await
            .unwrap();
        delete(&db, &folder).await.unwrap();
        let videos = memberships(&db, "video").await.unwrap();
        assert_eq!(videos["v1"], std::slice::from_ref(&later));
        let tombstones: i64 = sqlx::query(
            "SELECT COUNT(*) AS n FROM title_category WHERE category_id = ?1 AND deleted = 1",
        )
        .bind(&folder)
        .fetch_one(db.pool())
        .await
        .unwrap()
        .get("n");
        assert_eq!(tombstones, 2, "synced rows are tombstoned, not deleted");
        assert_eq!(list(&db, "novel").await.unwrap()[0].id, books);
    }

    #[tokio::test]
    async fn bulk_changes_and_a_categorys_own_display() {
        let temp = TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let reading = create(&db, "manga", "Reading").await.unwrap();
        let later = create(&db, "manga", "Later").await.unwrap();
        set_title_categories(&db, "t1", std::slice::from_ref(&reading))
            .await
            .unwrap();
        change_titles(
            &db,
            &["t1".to_owned(), "t2".to_owned()],
            std::slice::from_ref(&later),
            std::slice::from_ref(&reading),
        )
        .await
        .unwrap();
        let members = memberships(&db, "manga").await.unwrap();
        assert_eq!(members["t1"], std::slice::from_ref(&later));
        assert_eq!(members["t2"], std::slice::from_ref(&later));

        set_display(&db, &later, Some("{\"mode\":\"list\"}"))
            .await
            .unwrap();
        let shown = list(&db, "manga").await.unwrap();
        let display = |id: &str| shown.iter().find(|c| c.id == id).unwrap().display.clone();
        assert_eq!(display(&later).as_deref(), Some("{\"mode\":\"list\"}"));
        assert_eq!(display(&reading), None);
        set_display(&db, &later, None).await.unwrap();
        assert_eq!(list(&db, "manga").await.unwrap()[1].display, None);
    }
}
