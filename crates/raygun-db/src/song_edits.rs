//! A song's own sound and lyrics (M46.12), by track key, and the loudness songs measured while
//! they played. Edits are stamped like the synced tables; a reset is a tombstone. Loudness is
//! device-local.

use sqlx::Row;

use crate::{Db, DbError, persist_hlc};

/// A stored edit. `lyrics` is JSON written by the app (`{synced, plain}`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SongEditRow {
    pub track_key: String,
    pub start_ms: i64,
    pub end_ms: Option<i64>,
    pub gain: i64,
    pub lyrics_offset_ms: i64,
    pub lyrics: Option<String>,
}

fn edit(row: &sqlx::sqlite::SqliteRow) -> SongEditRow {
    SongEditRow {
        track_key: row.get("track_key"),
        start_ms: row.get("start_ms"),
        end_ms: row.get("end_ms"),
        gain: row.get("gain"),
        lyrics_offset_ms: row.get("lyrics_offset_ms"),
        lyrics: row.get("lyrics"),
    }
}

pub async fn get(db: &Db, track_key: &str) -> Result<Option<SongEditRow>, DbError> {
    let row = sqlx::query(
        "SELECT track_key, start_ms, end_ms, gain, lyrics_offset_ms, lyrics
         FROM song_edit WHERE track_key = ?1 AND deleted = 0",
    )
    .bind(track_key)
    .fetch_optional(db.pool())
    .await?;
    Ok(row.as_ref().map(edit))
}

/// Every song with an edit, for the playback service's cache.
pub async fn all(db: &Db) -> Result<Vec<SongEditRow>, DbError> {
    let rows = sqlx::query(
        "SELECT track_key, start_ms, end_ms, gain, lyrics_offset_ms, lyrics
         FROM song_edit WHERE deleted = 0",
    )
    .fetch_all(db.pool())
    .await?;
    Ok(rows.iter().map(edit).collect())
}

/// Stores a song's edit, replacing (or bringing back) its row.
pub async fn put(db: &Db, row: &SongEditRow) -> Result<(), DbError> {
    let stamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "INSERT INTO song_edit (track_key, start_ms, end_ms, gain, lyrics_offset_ms, lyrics,
                                updated_at_hlc, origin_device, deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)
         ON CONFLICT(track_key) DO UPDATE SET
            start_ms = excluded.start_ms, end_ms = excluded.end_ms, gain = excluded.gain,
            lyrics_offset_ms = excluded.lyrics_offset_ms, lyrics = excluded.lyrics,
            updated_at_hlc = excluded.updated_at_hlc, origin_device = excluded.origin_device,
            deleted = 0",
    )
    .bind(&row.track_key)
    .bind(row.start_ms)
    .bind(row.end_ms)
    .bind(row.gain)
    .bind(row.lyrics_offset_ms)
    .bind(&row.lyrics)
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(())
}

/// Forgets a song's edit (a tombstone, so other devices forget it too).
pub async fn remove(db: &Db, track_key: &str) -> Result<(), DbError> {
    let stamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "UPDATE song_edit SET deleted = 1, lyrics = NULL, updated_at_hlc = ?2, origin_device = ?3
         WHERE track_key = ?1 AND deleted = 0",
    )
    .bind(track_key)
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(())
}

/// Every measured loudness: `(track key, LUFS)`.
pub async fn loudness_all(db: &Db) -> Result<Vec<(String, f64)>, DbError> {
    let rows = sqlx::query("SELECT track_key, lufs FROM local_track_loudness")
        .fetch_all(db.pool())
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.get("track_key"), row.get("lufs")))
        .collect())
}

/// Remembers how loud a song measured.
pub async fn put_loudness(
    db: &Db,
    track_key: &str,
    lufs: f64,
    measured_at: i64,
) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO local_track_loudness (track_key, lufs, measured_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(track_key) DO UPDATE SET lufs = excluded.lufs, measured_at = excluded.measured_at",
    )
    .bind(track_key)
    .bind(lufs)
    .bind(measured_at)
    .execute(db.pool())
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn edits_are_stored_replaced_and_reset() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let row = SongEditRow {
            track_key: "bts|swim".into(),
            start_ms: 1_500,
            end_ms: Some(150_000),
            gain: -30,
            lyrics_offset_ms: 400,
            lyrics: Some(r#"{"synced":[],"plain":"Swim"}"#.into()),
        };
        put(&db, &row).await.unwrap();
        assert_eq!(get(&db, "bts|swim").await.unwrap(), Some(row.clone()));
        let louder = SongEditRow { gain: 20, ..row };
        put(&db, &louder).await.unwrap();
        assert_eq!(all(&db).await.unwrap(), vec![louder]);
        remove(&db, "bts|swim").await.unwrap();
        assert_eq!(get(&db, "bts|swim").await.unwrap(), None);
        assert!(all(&db).await.unwrap().is_empty());

        put_loudness(&db, "bts|swim", -9.5, 1).await.unwrap();
        put_loudness(&db, "bts|swim", -10.25, 2).await.unwrap();
        assert_eq!(
            loudness_all(&db).await.unwrap(),
            vec![("bts|swim".to_owned(), -10.25)]
        );
    }
}
