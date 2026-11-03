//! The user's own details for songs, albums and artists (M46.30): titles, names and covers shown
//! over what sources said. Device-local; these rows never sync.

use sqlx::Row;

use crate::{Db, DbError};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DetailEditRow {
    /// `song`, `album` or `artist`.
    pub kind: String,
    pub key: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub artwork: Option<String>,
}

impl DetailEditRow {
    /// Nothing is changed: the row can go.
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.artist.is_none()
            && self.album.is_none()
            && self.artwork.is_none()
    }
}

pub async fn all(db: &Db) -> Result<Vec<DetailEditRow>, DbError> {
    let rows =
        sqlx::query("SELECT kind, key, title, artist, album, artwork FROM local_detail_edit")
            .fetch_all(db.pool())
            .await?;
    Ok(rows
        .into_iter()
        .map(|row| DetailEditRow {
            kind: row.get("kind"),
            key: row.get("key"),
            title: row.get("title"),
            artist: row.get("artist"),
            album: row.get("album"),
            artwork: row.get("artwork"),
        })
        .collect())
}

/// Stores an edit, or removes it when it changes nothing.
pub async fn put(db: &Db, row: &DetailEditRow, now: i64) -> Result<(), DbError> {
    if row.is_empty() {
        sqlx::query("DELETE FROM local_detail_edit WHERE kind = ? AND key = ?")
            .bind(&row.kind)
            .bind(&row.key)
            .execute(db.pool())
            .await?;
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO local_detail_edit (kind, key, title, artist, album, artwork, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (kind, key) DO UPDATE SET title = excluded.title, artist = excluded.artist,
             album = excluded.album, artwork = excluded.artwork, updated_at = excluded.updated_at",
    )
    .bind(&row.kind)
    .bind(&row.key)
    .bind(&row.title)
    .bind(&row.artist)
    .bind(&row.album)
    .bind(&row.artwork)
    .bind(now)
    .execute(db.pool())
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn edits_are_kept_and_an_empty_one_goes() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let row = DetailEditRow {
            kind: "song".into(),
            key: "daft punk|get lucky".into(),
            title: Some("Get Lucky (Radio Edit)".into()),
            ..Default::default()
        };
        put(&db, &row, 1).await.unwrap();
        assert_eq!(all(&db).await.unwrap(), vec![row.clone()]);
        put(&db, &DetailEditRow { title: None, ..row }, 2)
            .await
            .unwrap();
        assert!(all(&db).await.unwrap().is_empty());
    }
}
