//! Storage for favorites, playlists and play history (M18). Synced tables: every write is stamped
//! with an HLC timestamp and this device's id, and removals are tombstones (ARCHITECTURE §5).
//! What a track is (portable sources, local matches) is decided by the app; this layer stores the
//! fields it is given.

use sqlx::sqlite::SqliteRow;
use sqlx::{Row, Sqlite, Transaction};
use uuid::Uuid;

use crate::{Db, DbError, persist_hlc};

/// A track as saved on a row. `source` is a MediaSource as JSON.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackFields {
    pub track_key: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
    pub artwork_url: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FavoriteRow {
    pub id: String,
    pub track: TrackFields,
    pub added_at: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaylistRow {
    pub id: String,
    pub name: String,
    pub created_at: i64,
    pub item_count: i64,
    /// The first items, for a cover made of their artwork.
    pub first_items: Vec<TrackFields>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaylistItemRow {
    pub id: String,
    pub playlist_id: String,
    pub position: f64,
    pub track: TrackFields,
    pub added_at: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HistoryRow {
    pub id: String,
    pub track: TrackFields,
    pub played_at: i64,
}

/// Items read per playlist for its cover.
const COVER_ITEMS: i64 = 12;
/// Below this gap between neighbours, a move renumbers the playlist first.
const MIN_POSITION_GAP: f64 = 1e-6;

const TRACK_COLUMNS: &str = "track_key, title, artist, album, duration_ms, artwork_url, source";

// ---- Favorites ----

/// Live favorites, most recently added first.
pub async fn favorites(db: &Db) -> Result<Vec<FavoriteRow>, DbError> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT id, added_at, {TRACK_COLUMNS} FROM music_favorite WHERE deleted = 0
         ORDER BY added_at DESC, id DESC"
    )))
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .iter()
        .map(|row| FavoriteRow {
            id: row.get("id"),
            track: track_fields(row),
            added_at: row.get("added_at"),
        })
        .collect())
}

pub async fn is_favorite(db: &Db, track_key: &str) -> Result<bool, DbError> {
    let row = sqlx::query("SELECT 1 FROM music_favorite WHERE track_key = ?1 AND deleted = 0")
        .bind(track_key)
        .fetch_optional(db.pool())
        .await?;
    Ok(row.is_some())
}

/// Favorites or unfavorites a track. Favoriting a track again revives its tombstoned row, so a
/// track keeps one id across devices.
pub async fn set_favorite(
    db: &Db,
    track: &TrackFields,
    favorite: bool,
    now_ms: i64,
) -> Result<(), DbError> {
    let (mut transaction, stamp) = begin(db).await?;
    if !favorite {
        sqlx::query(
            "UPDATE music_favorite SET deleted = 1, updated_at_hlc = ?2, origin_device = ?3
             WHERE track_key = ?1 AND deleted = 0",
        )
        .bind(&track.track_key)
        .bind(stamp.hlc)
        .bind(&stamp.device)
        .execute(&mut *transaction)
        .await?;
        return finish(transaction, stamp).await;
    }

    let live: Option<String> =
        sqlx::query("SELECT id FROM music_favorite WHERE track_key = ?1 AND deleted = 0")
            .bind(&track.track_key)
            .fetch_optional(&mut *transaction)
            .await?
            .map(|row| row.get("id"));
    if live.is_some() {
        return Ok(());
    }
    let dead: Option<String> = sqlx::query(
        "SELECT id FROM music_favorite WHERE track_key = ?1 ORDER BY updated_at_hlc DESC LIMIT 1",
    )
    .bind(&track.track_key)
    .fetch_optional(&mut *transaction)
    .await?
    .map(|row| row.get("id"));
    let id = dead.unwrap_or_else(|| Uuid::now_v7().to_string());
    sqlx::query(
        "INSERT INTO music_favorite (id, track_key, title, artist, album, duration_ms, artwork_url,
             source, added_at, updated_at_hlc, origin_device, deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0)
         ON CONFLICT (id) DO UPDATE SET
             title = excluded.title, artist = excluded.artist, album = excluded.album,
             duration_ms = excluded.duration_ms, artwork_url = excluded.artwork_url,
             source = excluded.source, added_at = excluded.added_at,
             updated_at_hlc = excluded.updated_at_hlc, origin_device = excluded.origin_device,
             deleted = 0",
    )
    .bind(&id)
    .bind(&track.track_key)
    .bind(&track.title)
    .bind(&track.artist)
    .bind(&track.album)
    .bind(track.duration_ms)
    .bind(&track.artwork_url)
    .bind(&track.source)
    .bind(now_ms)
    .bind(stamp.hlc)
    .bind(&stamp.device)
    .execute(&mut *transaction)
    .await?;
    finish(transaction, stamp).await
}

// ---- Playlists ----

/// Live playlists, alphabetically.
pub async fn playlists(db: &Db) -> Result<Vec<PlaylistRow>, DbError> {
    let rows = sqlx::query(
        "SELECT p.id, p.name, p.created_at,
             (SELECT COUNT(*) FROM music_playlist_item i
              WHERE i.playlist_id = p.id AND i.deleted = 0) AS item_count
         FROM music_playlist p WHERE p.deleted = 0
         ORDER BY p.name COLLATE NOCASE, p.id",
    )
    .fetch_all(db.pool())
    .await?;
    let mut playlists = Vec::with_capacity(rows.len());
    for row in rows {
        let id: String = row.get("id");
        let first_items = items_query(db, &id, Some(COVER_ITEMS))
            .await?
            .into_iter()
            .map(|item| item.track)
            .collect();
        playlists.push(PlaylistRow {
            name: row.get("name"),
            created_at: row.get("created_at"),
            item_count: row.get("item_count"),
            first_items,
            id,
        });
    }
    Ok(playlists)
}

/// A live playlist by id.
pub async fn playlist(db: &Db, id: &str) -> Result<Option<PlaylistRow>, DbError> {
    Ok(playlists(db)
        .await?
        .into_iter()
        .find(|playlist| playlist.id == id))
}

/// Creates an empty playlist and returns its id.
pub async fn create_playlist(db: &Db, name: &str, now_ms: i64) -> Result<String, DbError> {
    let id = Uuid::now_v7().to_string();
    let (mut transaction, stamp) = begin(db).await?;
    sqlx::query(
        "INSERT INTO music_playlist (id, name, created_at, updated_at_hlc, origin_device, deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, 0)",
    )
    .bind(&id)
    .bind(name)
    .bind(now_ms)
    .bind(stamp.hlc)
    .bind(&stamp.device)
    .execute(&mut *transaction)
    .await?;
    finish(transaction, stamp).await?;
    Ok(id)
}

/// Renames a live playlist; false when there is none with this id.
pub async fn rename_playlist(db: &Db, id: &str, name: &str) -> Result<bool, DbError> {
    let (mut transaction, stamp) = begin(db).await?;
    let changed = sqlx::query(
        "UPDATE music_playlist SET name = ?2, updated_at_hlc = ?3, origin_device = ?4
         WHERE id = ?1 AND deleted = 0",
    )
    .bind(id)
    .bind(name)
    .bind(stamp.hlc)
    .bind(&stamp.device)
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    finish(transaction, stamp).await?;
    Ok(changed > 0)
}

/// Tombstones a playlist and its items.
pub async fn delete_playlist(db: &Db, id: &str) -> Result<(), DbError> {
    let (mut transaction, stamp) = begin(db).await?;
    for sql in [
        "UPDATE music_playlist SET deleted = 1, updated_at_hlc = ?2, origin_device = ?3
         WHERE id = ?1 AND deleted = 0",
        "UPDATE music_playlist_item SET deleted = 1, updated_at_hlc = ?2, origin_device = ?3
         WHERE playlist_id = ?1 AND deleted = 0",
    ] {
        sqlx::query(sql)
            .bind(id)
            .bind(stamp.hlc)
            .bind(&stamp.device)
            .execute(&mut *transaction)
            .await?;
    }
    finish(transaction, stamp).await
}

/// A playlist's live items in order.
pub async fn playlist_items(db: &Db, playlist_id: &str) -> Result<Vec<PlaylistItemRow>, DbError> {
    items_query(db, playlist_id, None).await
}

/// Appends tracks to a playlist, in the order given.
pub async fn add_to_playlist(
    db: &Db,
    playlist_id: &str,
    tracks: &[TrackFields],
    now_ms: i64,
) -> Result<(), DbError> {
    let (mut transaction, stamp) = begin(db).await?;
    let last: Option<f64> = sqlx::query(
        "SELECT MAX(position) AS last FROM music_playlist_item
         WHERE playlist_id = ?1 AND deleted = 0",
    )
    .bind(playlist_id)
    .fetch_one(&mut *transaction)
    .await?
    .get("last");
    let mut position = last.unwrap_or(0.0);
    for track in tracks {
        position += 1.0;
        sqlx::query(
            "INSERT INTO music_playlist_item (id, playlist_id, position, track_key, title, artist,
                 album, duration_ms, artwork_url, source, added_at, updated_at_hlc, origin_device,
                 deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 0)",
        )
        .bind(Uuid::now_v7().to_string())
        .bind(playlist_id)
        .bind(position)
        .bind(&track.track_key)
        .bind(&track.title)
        .bind(&track.artist)
        .bind(&track.album)
        .bind(track.duration_ms)
        .bind(&track.artwork_url)
        .bind(&track.source)
        .bind(now_ms)
        .bind(stamp.hlc)
        .bind(&stamp.device)
        .execute(&mut *transaction)
        .await?;
    }
    finish(transaction, stamp).await
}

pub async fn remove_playlist_item(db: &Db, item_id: &str) -> Result<(), DbError> {
    let (mut transaction, stamp) = begin(db).await?;
    sqlx::query(
        "UPDATE music_playlist_item SET deleted = 1, updated_at_hlc = ?2, origin_device = ?3
         WHERE id = ?1 AND deleted = 0",
    )
    .bind(item_id)
    .bind(stamp.hlc)
    .bind(&stamp.device)
    .execute(&mut *transaction)
    .await?;
    finish(transaction, stamp).await
}

/// The live playlists holding a song (M46.8).
pub async fn playlists_with_track(db: &Db, track_key: &str) -> Result<Vec<String>, DbError> {
    let rows = sqlx::query(
        "SELECT DISTINCT item.playlist_id AS id FROM music_playlist_item AS item
         JOIN music_playlist AS playlist ON playlist.id = item.playlist_id AND playlist.deleted = 0
         WHERE item.track_key = ?1 AND item.deleted = 0",
    )
    .bind(track_key)
    .fetch_all(db.pool())
    .await?;
    Ok(rows.iter().map(|row| row.get("id")).collect())
}

/// Tombstones every item of a song in a playlist (M46.8).
pub async fn remove_track_from_playlist(
    db: &Db,
    playlist_id: &str,
    track_key: &str,
) -> Result<(), DbError> {
    let (mut transaction, stamp) = begin(db).await?;
    sqlx::query(
        "UPDATE music_playlist_item SET deleted = 1, updated_at_hlc = ?3, origin_device = ?4
         WHERE playlist_id = ?1 AND track_key = ?2 AND deleted = 0",
    )
    .bind(playlist_id)
    .bind(track_key)
    .bind(stamp.hlc)
    .bind(&stamp.device)
    .execute(&mut *transaction)
    .await?;
    finish(transaction, stamp).await
}

/// Moves an item before another item of its playlist, or to the end when `before` is None.
/// Only the moved row changes, unless the gap is too small and the playlist is renumbered.
pub async fn move_playlist_item(
    db: &Db,
    item_id: &str,
    before: Option<&str>,
) -> Result<(), DbError> {
    let Some(playlist_id) =
        sqlx::query("SELECT playlist_id FROM music_playlist_item WHERE id = ?1 AND deleted = 0")
            .bind(item_id)
            .fetch_optional(db.pool())
            .await?
            .map(|row| row.get::<String, _>("playlist_id"))
    else {
        return Ok(());
    };
    let mut others: Vec<(String, f64)> = playlist_items(db, &playlist_id)
        .await?
        .into_iter()
        .filter(|item| item.id != item_id)
        .map(|item| (item.id, item.position))
        .collect();
    let (mut transaction, stamp) = begin(db).await?;
    let mut position = new_position(&others, before);
    if position.is_none() {
        // Neighbours too close to split: space every item out again, then place this one.
        for (index, (id, position)) in others.iter_mut().enumerate() {
            *position = index as f64 + 1.0;
            set_position(&mut transaction, &stamp, id, *position).await?;
        }
        position = new_position(&others, before);
    }
    if let Some(position) = position {
        set_position(&mut transaction, &stamp, item_id, position).await?;
    }
    finish(transaction, stamp).await
}

/// Where an item goes so it lands before `before` (or last), or None when the neighbours are too
/// close together to fit it between them.
fn new_position(others: &[(String, f64)], before: Option<&str>) -> Option<f64> {
    let index = before
        .and_then(|before| others.iter().position(|(id, _)| id == before))
        .unwrap_or(others.len());
    let next = others.get(index).map(|(_, position)| *position);
    let previous = index
        .checked_sub(1)
        .and_then(|previous| others.get(previous))
        .map(|(_, position)| *position);
    match (previous, next) {
        (None, None) => Some(1.0),
        (Some(previous), None) => Some(previous + 1.0),
        (None, Some(next)) => Some(next - 1.0),
        (Some(previous), Some(next)) => {
            (next - previous > MIN_POSITION_GAP).then_some((previous + next) / 2.0)
        }
    }
}

async fn set_position(
    transaction: &mut Transaction<'_, Sqlite>,
    stamp: &Stamp,
    id: &str,
    position: f64,
) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE music_playlist_item SET position = ?2, updated_at_hlc = ?3, origin_device = ?4
         WHERE id = ?1",
    )
    .bind(id)
    .bind(position)
    .bind(stamp.hlc)
    .bind(&stamp.device)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn items_query(
    db: &Db,
    playlist_id: &str,
    limit: Option<i64>,
) -> Result<Vec<PlaylistItemRow>, DbError> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT id, playlist_id, position, added_at, {TRACK_COLUMNS} FROM music_playlist_item
         WHERE playlist_id = ?1 AND deleted = 0 ORDER BY position, id LIMIT ?2"
    )))
    .bind(playlist_id)
    .bind(limit.unwrap_or(-1))
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .iter()
        .map(|row| PlaylistItemRow {
            id: row.get("id"),
            playlist_id: row.get("playlist_id"),
            position: row.get("position"),
            track: track_fields(row),
            added_at: row.get("added_at"),
        })
        .collect())
}

// ---- History ----

/// Appends a play.
pub async fn record_play(db: &Db, track: &TrackFields, played_at: i64) -> Result<(), DbError> {
    let (mut transaction, stamp) = begin(db).await?;
    sqlx::query(
        "INSERT INTO music_play_history (id, track_key, title, artist, album, duration_ms,
             artwork_url, source, played_at, updated_at_hlc, origin_device, deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0)",
    )
    .bind(Uuid::now_v7().to_string())
    .bind(&track.track_key)
    .bind(&track.title)
    .bind(&track.artist)
    .bind(&track.album)
    .bind(track.duration_ms)
    .bind(&track.artwork_url)
    .bind(&track.source)
    .bind(played_at)
    .bind(stamp.hlc)
    .bind(&stamp.device)
    .execute(&mut *transaction)
    .await?;
    finish(transaction, stamp).await
}

/// The latest play of each track, newest first.
pub async fn recent_history(db: &Db, limit: i64) -> Result<Vec<HistoryRow>, DbError> {
    // SQLite takes the other columns from the row holding MAX(played_at).
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT id, MAX(played_at) AS played_at, {TRACK_COLUMNS} FROM music_play_history
         WHERE deleted = 0 GROUP BY track_key ORDER BY played_at DESC, id DESC LIMIT ?1"
    )))
    .bind(limit)
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .iter()
        .map(|row| HistoryRow {
            id: row.get("id"),
            track: track_fields(row),
            played_at: row.get("played_at"),
        })
        .collect())
}

/// Every play since `since_ms`, newest first, at most `limit` (M46.20: Made for You).
pub async fn plays_since(db: &Db, since_ms: i64, limit: i64) -> Result<Vec<HistoryRow>, DbError> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT id, played_at, {TRACK_COLUMNS} FROM music_play_history
         WHERE deleted = 0 AND played_at >= ?1 ORDER BY played_at DESC, id DESC LIMIT ?2"
    )))
    .bind(since_ms)
    .bind(limit)
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .iter()
        .map(|row| HistoryRow {
            id: row.get("id"),
            track: track_fields(row),
            played_at: row.get("played_at"),
        })
        .collect())
}

/// How many times each of `keys` was played (M46.8); keys never played are missing.
pub async fn play_counts(
    db: &Db,
    keys: &[String],
) -> Result<std::collections::HashMap<String, i64>, DbError> {
    let mut counts = std::collections::HashMap::new();
    // SQLite allows a limited number of bound values per statement.
    for chunk in keys.chunks(500) {
        let marks = vec!["?"; chunk.len()].join(", ");
        let mut query = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT track_key, COUNT(*) AS plays FROM music_play_history
             WHERE deleted = 0 AND track_key IN ({marks}) GROUP BY track_key"
        )));
        for key in chunk {
            query = query.bind(key);
        }
        for row in query.fetch_all(db.pool()).await? {
            counts.insert(row.get("track_key"), row.get("plays"));
        }
    }
    Ok(counts)
}

/// Tombstones every play.
pub async fn clear_history(db: &Db) -> Result<(), DbError> {
    let (mut transaction, stamp) = begin(db).await?;
    sqlx::query(
        "UPDATE music_play_history SET deleted = 1, updated_at_hlc = ?1, origin_device = ?2
         WHERE deleted = 0",
    )
    .bind(stamp.hlc)
    .bind(&stamp.device)
    .execute(&mut *transaction)
    .await?;
    finish(transaction, stamp).await
}

// ---- Helpers ----

fn track_fields(row: &SqliteRow) -> TrackFields {
    TrackFields {
        track_key: row.get("track_key"),
        title: row.get("title"),
        artist: row.get("artist"),
        album: row.get("album"),
        duration_ms: row.get("duration_ms"),
        artwork_url: row.get("artwork_url"),
        source: row.get("source"),
    }
}

/// The HLC timestamp and device id a write is stamped with.
struct Stamp {
    hlc: i64,
    device: String,
}

async fn begin(db: &Db) -> Result<(Transaction<'static, Sqlite>, Stamp), DbError> {
    let hlc = db.tick();
    let transaction = db.pool().begin().await?;
    Ok((
        transaction,
        Stamp {
            hlc: hlc as i64,
            device: db.device_id().to_string(),
        },
    ))
}

async fn finish(mut transaction: Transaction<'_, Sqlite>, stamp: Stamp) -> Result<(), DbError> {
    persist_hlc(&mut *transaction, stamp.hlc as u64).await?;
    transaction.commit().await?;
    Ok(())
}

#[cfg(test)]
#[path = "collections_tests.rs"]
mod tests;
