//! Song lengths Raygun learned (M46.26), by track key. Device-local; these rows never sync. A row
//! without a length remembers a lookup that found none, so it isn't tried again too soon.

use sqlx::{AssertSqlSafe, Row};

use crate::{Db, DbError};

/// A stored row: the length, or `None` for a lookup that found none, and when it was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stored {
    pub duration_ms: Option<i64>,
    pub updated_at: i64,
}

/// Rows stored for each of `keys` that has one.
pub async fn get_many(db: &Db, keys: &[String]) -> Result<Vec<(String, Stored)>, DbError> {
    let mut found = Vec::new();
    // SQLite allows a limited number of bound values per statement.
    for chunk in keys.chunks(500) {
        let marks = vec!["?"; chunk.len()].join(",");
        let sql = format!(
            "SELECT track_key, duration_ms, updated_at FROM local_track_duration \
             WHERE track_key IN ({marks})"
        );
        // Only placeholders are formatted in; the keys are bound.
        let mut query = sqlx::query(AssertSqlSafe(sql));
        for key in chunk {
            query = query.bind(key);
        }
        for row in query.fetch_all(db.pool()).await? {
            found.push((
                row.get("track_key"),
                Stored {
                    duration_ms: row.get("duration_ms"),
                    updated_at: row.get("updated_at"),
                },
            ));
        }
    }
    Ok(found)
}

/// Stores lengths learned together: `(key, ms)`. A known length is replaced only by playback's
/// (`replace`), and a remembered miss always gives way.
pub async fn put_many(
    db: &Db,
    lengths: &[(String, i64)],
    source: &str,
    replace: bool,
    now: i64,
) -> Result<(), DbError> {
    let mut transaction = db.pool().begin().await?;
    for (key, ms) in lengths {
        let sql = if replace {
            "INSERT INTO local_track_duration (track_key, duration_ms, source, updated_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT (track_key) DO UPDATE SET duration_ms = excluded.duration_ms,
                 source = excluded.source, updated_at = excluded.updated_at"
        } else {
            "INSERT INTO local_track_duration (track_key, duration_ms, source, updated_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT (track_key) DO UPDATE SET duration_ms = excluded.duration_ms,
                 source = excluded.source, updated_at = excluded.updated_at
             WHERE local_track_duration.duration_ms IS NULL"
        };
        sqlx::query(sql)
            .bind(key)
            .bind(ms)
            .bind(source)
            .bind(now)
            .execute(&mut *transaction)
            .await?;
    }
    transaction.commit().await?;
    Ok(())
}

/// Remembers that a lookup found no length for `key` (a known length stays).
pub async fn put_miss(db: &Db, key: &str, now: i64) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO local_track_duration (track_key, duration_ms, source, updated_at)
         VALUES (?, NULL, 'lookup', ?)
         ON CONFLICT (track_key) DO UPDATE SET updated_at = excluded.updated_at
         WHERE local_track_duration.duration_ms IS NULL",
    )
    .bind(key)
    .bind(now)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// Fills the length of saved songs (favorites, playlists, history) that have none. These are the
/// user's own rows, so only gaps are filled; nothing a source said is changed.
pub async fn fill_saved(db: &Db, key: &str, ms: i64) -> Result<u64, DbError> {
    let mut filled = 0;
    for table in [
        "music_favorite",
        "music_playlist_item",
        "music_play_history",
    ] {
        let sql = format!(
            "UPDATE {table} SET duration_ms = ? WHERE track_key = ? AND duration_ms IS NULL"
        );
        // The table name is one of the constants above.
        filled += sqlx::query(AssertSqlSafe(sql))
            .bind(ms)
            .bind(key)
            .execute(db.pool())
            .await?
            .rows_affected();
    }
    Ok(filled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lengths_fill_gaps_and_playback_corrects() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = crate::Db::open(&temp.path().join("raygun.db"))
            .await
            .unwrap();
        put_miss(&db, "a|one", 1).await.unwrap();
        put_many(
            &db,
            &[("a|one".into(), 200_000), ("a|two".into(), 180_000)],
            "source",
            false,
            2,
        )
        .await
        .unwrap();
        // A source doesn't replace a known length; playback does.
        put_many(&db, &[("a|two".into(), 1_000)], "source", false, 3)
            .await
            .unwrap();
        put_many(&db, &[("a|one".into(), 201_000)], "playback", true, 4)
            .await
            .unwrap();
        // A miss never hides a known length.
        put_miss(&db, "a|two", 5).await.unwrap();
        let mut rows = get_many(&db, &["a|one".into(), "a|two".into(), "a|three".into()])
            .await
            .unwrap();
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            rows.iter()
                .map(|(key, stored)| (key.as_str(), stored.duration_ms))
                .collect::<Vec<_>>(),
            vec![("a|one", Some(201_000)), ("a|two", Some(180_000))]
        );
    }
}
