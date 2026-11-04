//! Local Files (M46.27): the folders the user added and the comics, books and videos found in
//! them. Device-local; these rows never sync.

use std::collections::HashMap;

use sqlx::Row;

use crate::{Db, DbError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderRow {
    pub id: String,
    pub location: String,
    pub name: String,
    pub path: Option<String>,
    pub added_at: i64,
    pub scanned_at: Option<i64>,
    /// JSON `{"songs":…,"comics":…,"books":…,"videos":…}`.
    pub counts: String,
}

/// A file found before: its size and time, and what it became.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRow {
    pub size: i64,
    pub modified: i64,
    pub kind: String,
    pub item: Option<String>,
}

fn folder_row(row: &sqlx::sqlite::SqliteRow) -> FolderRow {
    FolderRow {
        id: row.get("id"),
        location: row.get("location"),
        name: row.get("name"),
        path: row.get("path"),
        added_at: row.get("added_at"),
        scanned_at: row.get("scanned_at"),
        counts: row.get("counts"),
    }
}

pub async fn folders(db: &Db) -> Result<Vec<FolderRow>, DbError> {
    let rows = sqlx::query(
        "SELECT id, location, name, path, added_at, scanned_at, counts FROM local_folder
         ORDER BY added_at",
    )
    .fetch_all(db.pool())
    .await?;
    Ok(rows.iter().map(folder_row).collect())
}

pub async fn folder(db: &Db, id: &str) -> Result<Option<FolderRow>, DbError> {
    let row = sqlx::query(
        "SELECT id, location, name, path, added_at, scanned_at, counts FROM local_folder
         WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(db.pool())
    .await?;
    Ok(row.as_ref().map(folder_row))
}

/// Adds a folder (a location added again keeps its row); returns its id.
pub async fn add(
    db: &Db,
    id: &str,
    location: &str,
    name: &str,
    path: Option<&str>,
    now: i64,
) -> Result<String, DbError> {
    sqlx::query(
        "INSERT INTO local_folder (id, location, name, path, added_at) VALUES (?, ?, ?, ?, ?)
         ON CONFLICT (location) DO UPDATE SET name = excluded.name, path = excluded.path",
    )
    .bind(id)
    .bind(location)
    .bind(name)
    .bind(path)
    .bind(now)
    .execute(db.pool())
    .await?;
    let row = sqlx::query("SELECT id FROM local_folder WHERE location = ?")
        .bind(location)
        .fetch_one(db.pool())
        .await?;
    Ok(row.get("id"))
}

/// Forgets a folder and what was found in it.
pub async fn remove(db: &Db, id: &str) -> Result<(), DbError> {
    let mut transaction = db.pool().begin().await?;
    sqlx::query("DELETE FROM local_folder_file WHERE folder_id = ?")
        .bind(id)
        .execute(&mut *transaction)
        .await?;
    sqlx::query("DELETE FROM local_folder WHERE id = ?")
        .bind(id)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(())
}

pub async fn set_scanned(db: &Db, id: &str, counts: &str, now: i64) -> Result<(), DbError> {
    sqlx::query("UPDATE local_folder SET scanned_at = ?, counts = ? WHERE id = ?")
        .bind(now)
        .bind(counts)
        .bind(id)
        .execute(db.pool())
        .await?;
    Ok(())
}

/// Changes what a folder's line says without counting it as scanned (it couldn't be opened).
pub async fn set_counts(db: &Db, id: &str, counts: &str) -> Result<(), DbError> {
    sqlx::query("UPDATE local_folder SET counts = ? WHERE id = ?")
        .bind(counts)
        .bind(id)
        .execute(db.pool())
        .await?;
    Ok(())
}

/// The files found in a folder before, by their path inside it.
pub async fn files(db: &Db, folder_id: &str) -> Result<HashMap<String, FileRow>, DbError> {
    let rows = sqlx::query(
        "SELECT rel_path, size, modified, kind, item FROM local_folder_file WHERE folder_id = ?",
    )
    .bind(folder_id)
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.get("rel_path"),
                FileRow {
                    size: row.get("size"),
                    modified: row.get("modified"),
                    kind: row.get("kind"),
                    item: row.get("item"),
                },
            )
        })
        .collect())
}

pub async fn put_file(
    db: &Db,
    folder_id: &str,
    rel_path: &str,
    file: &FileRow,
) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO local_folder_file (folder_id, rel_path, size, modified, kind, item)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT (folder_id, rel_path) DO UPDATE SET size = excluded.size,
             modified = excluded.modified, kind = excluded.kind, item = excluded.item",
    )
    .bind(folder_id)
    .bind(rel_path)
    .bind(file.size)
    .bind(file.modified)
    .bind(&file.kind)
    .bind(&file.item)
    .execute(db.pool())
    .await?;
    Ok(())
}

pub async fn delete_file(db: &Db, folder_id: &str, rel_path: &str) -> Result<(), DbError> {
    sqlx::query("DELETE FROM local_folder_file WHERE folder_id = ? AND rel_path = ?")
        .bind(folder_id)
        .bind(rel_path)
        .execute(db.pool())
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn folders_and_their_files() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let id = add(&db, "f1", "C:/Media", "Media", Some("C:/Media"), 1)
            .await
            .unwrap();
        // The same place again keeps its row.
        assert_eq!(
            add(&db, "f2", "C:/Media", "Media", Some("C:/Media"), 2)
                .await
                .unwrap(),
            id
        );
        let file = FileRow {
            size: 10,
            modified: 5,
            kind: "comic".into(),
            item: Some("local.comics:saga".into()),
        };
        put_file(&db, &id, "Saga/1.cbz", &file).await.unwrap();
        set_scanned(&db, &id, r#"{"comics":1}"#, 3).await.unwrap();
        assert_eq!(
            files(&db, &id).await.unwrap().get("Saga/1.cbz"),
            Some(&file)
        );
        assert_eq!(folders(&db).await.unwrap()[0].scanned_at, Some(3));
        // A folder that couldn't be opened keeps its last scan's time.
        set_counts(&db, &id, r#"{"comics":1,"missing":true}"#)
            .await
            .unwrap();
        let row = folder(&db, &id).await.unwrap().unwrap();
        assert_eq!(
            (row.scanned_at, row.counts.as_str()),
            (Some(3), r#"{"comics":1,"missing":true}"#)
        );
        remove(&db, &id).await.unwrap();
        assert!(folders(&db).await.unwrap().is_empty());
        assert!(files(&db, &id).await.unwrap().is_empty());
    }
}
