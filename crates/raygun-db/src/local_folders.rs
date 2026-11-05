//! Local Files (M46.27): the folders the user added and the comics, books and videos found in
//! them. Device-local; these rows never sync.

use std::collections::HashMap;

use sqlx::Row;

use crate::{Db, DbError};

/// How a folder is read (M46.51).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderAccess {
    /// By its path: a desktop folder, or one chosen in Raygun's own folder browser (Android).
    Path,
    /// Through the grant of Android's system folder picker; `location` is its `content://` tree.
    Tree,
    /// Raygun's own folder of files added one by one: copies it made, deleted with the folder.
    Added,
}

impl FolderAccess {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Tree => "tree",
            Self::Added => "added",
        }
    }

    /// A stored value; one this version doesn't know is read the way its location says.
    pub fn parse(value: &str, location: &str) -> Self {
        match value {
            "tree" => Self::Tree,
            "added" => Self::Added,
            "path" => Self::Path,
            _ if location.starts_with("content://") => Self::Tree,
            _ => Self::Path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderRow {
    pub id: String,
    pub location: String,
    pub name: String,
    pub path: Option<String>,
    pub access: FolderAccess,
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
    let location: String = row.get("location");
    FolderRow {
        id: row.get("id"),
        access: FolderAccess::parse(row.get("access"), &location),
        location,
        name: row.get("name"),
        path: row.get("path"),
        added_at: row.get("added_at"),
        scanned_at: row.get("scanned_at"),
        counts: row.get("counts"),
    }
}

pub async fn folders(db: &Db) -> Result<Vec<FolderRow>, DbError> {
    let rows = sqlx::query(
        "SELECT id, location, name, path, access, added_at, scanned_at, counts FROM local_folder
         ORDER BY added_at",
    )
    .fetch_all(db.pool())
    .await?;
    Ok(rows.iter().map(folder_row).collect())
}

pub async fn folder(db: &Db, id: &str) -> Result<Option<FolderRow>, DbError> {
    let row = sqlx::query(
        "SELECT id, location, name, path, access, added_at, scanned_at, counts FROM local_folder
         WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(db.pool())
    .await?;
    Ok(row.as_ref().map(folder_row))
}

/// A folder to add.
#[derive(Debug, Clone, Copy)]
pub struct NewFolder<'a> {
    pub id: &'a str,
    /// Its path, or (a tree) the picker's address.
    pub location: &'a str,
    pub name: &'a str,
    /// The folder as a path, when there is one.
    pub path: Option<&'a str>,
    pub access: FolderAccess,
}

/// Adds a folder (a location added again keeps its row, read the new way); returns its id.
pub async fn add(db: &Db, folder: NewFolder<'_>, now: i64) -> Result<String, DbError> {
    let NewFolder {
        id,
        location,
        name,
        path,
        access,
    } = folder;
    sqlx::query(
        "INSERT INTO local_folder (id, location, name, path, access, added_at)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT (location) DO UPDATE SET name = excluded.name, path = excluded.path,
             access = excluded.access",
    )
    .bind(id)
    .bind(location)
    .bind(name)
    .bind(path)
    .bind(access.as_str())
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

    fn by_path<'a>(id: &'a str, path: &'a str) -> NewFolder<'a> {
        NewFolder {
            id,
            location: path,
            name: "Media",
            path: Some(path),
            access: FolderAccess::Path,
        }
    }

    #[tokio::test]
    async fn folders_and_their_files() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let id = add(&db, by_path("f1", "C:/Media"), 1).await.unwrap();
        // The same place again keeps its row.
        assert_eq!(add(&db, by_path("f2", "C:/Media"), 2).await.unwrap(), id);
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

    #[tokio::test]
    async fn folders_say_how_they_are_read() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let tree = "content://com.android.externalstorage.documents/tree/1A2B-3C4D%3AMusic";
        let picked = add(
            &db,
            NewFolder {
                id: "t1",
                location: tree,
                name: "Music",
                path: Some("/storage/1A2B-3C4D/Music"),
                access: FolderAccess::Tree,
            },
            1,
        )
        .await
        .unwrap();
        let browsed = add(&db, by_path("p1", "/storage/emulated/0/Download"), 2)
            .await
            .unwrap();
        let added = add(
            &db,
            NewFolder {
                id: "a1",
                location: "/data/raygun/downloads/added",
                name: "Added files",
                path: Some("/data/raygun/downloads/added"),
                access: FolderAccess::Added,
            },
            3,
        )
        .await
        .unwrap();
        let read = |id: String| {
            let db = &db;
            async move { folder(db, &id).await.unwrap().unwrap().access }
        };
        assert_eq!(read(picked.clone()).await, FolderAccess::Tree);
        assert_eq!(read(browsed.clone()).await, FolderAccess::Path);
        assert_eq!(read(added).await, FolderAccess::Added);

        // A folder chosen again another way is read the new way, under the same row.
        let again = add(
            &db,
            NewFolder {
                access: FolderAccess::Path,
                ..by_path("p2", tree)
            },
            4,
        )
        .await
        .unwrap();
        assert_eq!(again, picked);
        assert_eq!(read(picked).await, FolderAccess::Path);
        assert_eq!(FolderAccess::parse("later", tree), FolderAccess::Tree);
        assert_eq!(FolderAccess::parse("later", "C:/Media"), FolderAccess::Path);
    }

    /// Folders added before they said how they're read: picked trees are trees, the rest paths.
    #[tokio::test]
    async fn folders_from_before_are_read_as_they_were() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("raygun.db");
        {
            let options = sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true);
            let pool = sqlx::SqlitePool::connect_with(options).await.unwrap();
            let mut earlier = sqlx::migrate!("./migrations");
            earlier.migrations = earlier
                .migrations
                .iter()
                .filter(|migration| migration.version < 39)
                .cloned()
                .collect::<Vec<_>>()
                .into();
            earlier.run(&pool).await.unwrap();
            for (id, location) in [
                (
                    "t",
                    "content://com.android.externalstorage.documents/tree/primary%3AMusic",
                ),
                ("p", "C:/Users/me/Media"),
            ] {
                sqlx::query(
                    "INSERT INTO local_folder (id, location, name, added_at) VALUES (?, ?, 'x', 1)",
                )
                .bind(id)
                .bind(location)
                .execute(&pool)
                .await
                .unwrap();
            }
            pool.close().await;
        }
        let db = Db::open(&path).await.unwrap();
        assert_eq!(
            folder(&db, "t").await.unwrap().unwrap().access,
            FolderAccess::Tree
        );
        assert_eq!(
            folder(&db, "p").await.unwrap().unwrap().access,
            FolderAccess::Path
        );
    }
}
