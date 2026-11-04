//! Storage for favorites, playlists and play history (M18). Synced tables: every write is stamped
//! with an HLC timestamp and this device's id, and removals are tombstones (ARCHITECTURE §5).
//! What a track is (portable sources, local matches) is decided by the app; this layer stores the
//! fields it is given.

use std::collections::HashSet;

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
    /// The summed length of the items with a known length, and how many those are.
    pub known_ms: i64,
    pub known_count: i64,
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

/// A live favorite or playlist item as the app compares songs (M46.48): where it is, its key and
/// the names it was saved with.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedRef {
    pub id: String,
    /// The playlist holding it; `None` for a favorite.
    pub playlist_id: Option<String>,
    pub track_key: String,
    pub title: String,
    pub artist: Option<String>,
    pub added_at: i64,
    /// Its stream, a MediaSource as JSON, when it has one.
    pub source: Option<String>,
}

impl SavedRef {
    pub fn is_favorite(&self) -> bool {
        self.playlist_id.is_none()
    }
}

/// Items read per playlist for its cover.
const COVER_ITEMS: i64 = 12;
/// Below this gap between neighbours, a move renumbers the playlist first.
const MIN_POSITION_GAP: f64 = 1e-6;

const TRACK_COLUMNS: &str = "track_key, title, artist, album, duration_ms, artwork_url, source";

/// A favorite's place in the user's order (M46.44): its position once moved, else its added time
/// reversed, so favorites nobody moved stay newest first and new ones arrive on top.
const FAVORITE_PLACE: &str = "COALESCE(position, -CAST(added_at AS REAL))";

/// The tables whose rows have a `position` (M46.44).
#[derive(Clone, Copy)]
enum Ordered {
    PlaylistItems,
    Favorites,
}

impl Ordered {
    fn table(self) -> &'static str {
        match self {
            Ordered::PlaylistItems => "music_playlist_item",
            Ordered::Favorites => "music_favorite",
        }
    }
}

// ---- Favorites ----

/// Live favorites in the user's order (newest first until some are moved).
pub async fn favorites(db: &Db) -> Result<Vec<FavoriteRow>, DbError> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT id, added_at, {TRACK_COLUMNS} FROM music_favorite WHERE deleted = 0
         ORDER BY {FAVORITE_PLACE}, id DESC"
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
             source = excluded.source, added_at = excluded.added_at, position = NULL,
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

/// Every saved song (Favorites and playlists) as `(track key, artist, album, artwork)`, once each
/// (M46.46: their covers stay cached).
pub async fn saved_songs(
    db: &Db,
) -> Result<Vec<(String, Option<String>, Option<String>, Option<String>)>, DbError> {
    let rows = sqlx::query(
        "SELECT track_key, artist, album, artwork_url FROM music_favorite WHERE deleted = 0
         UNION SELECT track_key, artist, album, artwork_url FROM music_playlist_item
         WHERE deleted = 0",
    )
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .iter()
        .map(|row| {
            (
                row.get("track_key"),
                row.get("artist"),
                row.get("album"),
                row.get("artwork_url"),
            )
        })
        .collect())
}

/// Keys of every song in Favorites or a playlist (M46.45: the saved mark in lists).
pub async fn saved_keys(db: &Db) -> Result<Vec<String>, DbError> {
    let rows = sqlx::query(
        "SELECT track_key FROM music_favorite WHERE deleted = 0
         UNION SELECT track_key FROM music_playlist_item WHERE deleted = 0",
    )
    .fetch_all(db.pool())
    .await?;
    Ok(rows.iter().map(|row| row.get("track_key")).collect())
}

/// Every live favorite and every live item of a live playlist, oldest first (M46.48: the app
/// finds a song's saved copies among them, whatever names they were saved with).
pub async fn saved_refs(db: &Db) -> Result<Vec<SavedRef>, DbError> {
    let rows = sqlx::query(
        "SELECT id, NULL AS playlist_id, track_key, title, artist, added_at, source
         FROM music_favorite WHERE deleted = 0
         UNION ALL
         SELECT item.id, item.playlist_id, item.track_key, item.title, item.artist, item.added_at,
             item.source
         FROM music_playlist_item AS item
         JOIN music_playlist AS playlist ON playlist.id = item.playlist_id AND playlist.deleted = 0
         WHERE item.deleted = 0
         ORDER BY added_at, id",
    )
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .iter()
        .map(|row| SavedRef {
            id: row.get("id"),
            playlist_id: row.get("playlist_id"),
            track_key: row.get("track_key"),
            title: row.get("title"),
            artist: row.get("artist"),
            added_at: row.get("added_at"),
            source: row.get("source"),
        })
        .collect())
}

/// Tombstones favorites and playlist items by id, as one write (M46.48: every saved copy of a
/// song, whatever key it was saved under).
pub async fn remove_saved(
    db: &Db,
    favorite_ids: &[String],
    item_ids: &[String],
) -> Result<(), DbError> {
    if favorite_ids.is_empty() && item_ids.is_empty() {
        return Ok(());
    }
    let (mut transaction, stamp) = begin(db).await?;
    for (table, ids) in [
        ("music_favorite", favorite_ids),
        ("music_playlist_item", item_ids),
    ] {
        for id in ids {
            // The table name is one of the constants above.
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "UPDATE {table} SET deleted = 1, updated_at_hlc = ?2, origin_device = ?3
                 WHERE id = ?1 AND deleted = 0"
            )))
            .bind(id)
            .bind(stamp.hlc)
            .bind(&stamp.device)
            .execute(&mut *transaction)
            .await?;
        }
    }
    finish(transaction, stamp).await
}

/// Gives rows their key again, by id (M46.48: a key that no longer matched the row's names).
pub async fn set_keys(
    db: &Db,
    favorites: &[(String, String)],
    items: &[(String, String)],
) -> Result<(), DbError> {
    if favorites.is_empty() && items.is_empty() {
        return Ok(());
    }
    let (mut transaction, stamp) = begin(db).await?;
    for (table, changes) in [
        ("music_favorite", favorites),
        ("music_playlist_item", items),
    ] {
        for (id, key) in changes {
            // The table name is one of the constants above.
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "UPDATE {table} SET track_key = ?2, updated_at_hlc = ?3, origin_device = ?4
                 WHERE id = ?1 AND deleted = 0"
            )))
            .bind(id)
            .bind(key)
            .bind(stamp.hlc)
            .bind(&stamp.device)
            .execute(&mut *transaction)
            .await?;
        }
    }
    finish(transaction, stamp).await
}

/// Live favorites as `(id, track key, place)`, in the user's order.
async fn favorite_places(db: &Db) -> Result<Vec<(String, String, f64)>, DbError> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT id, track_key, {FAVORITE_PLACE} AS place FROM music_favorite WHERE deleted = 0
         ORDER BY place, id DESC"
    )))
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .iter()
        .map(|row| (row.get("id"), row.get("track_key"), row.get("place")))
        .collect())
}

/// Moves a favorite (by track key) before another, or last when `before` is None (M46.44).
pub async fn move_favorite(db: &Db, track_key: &str, before: Option<&str>) -> Result<(), DbError> {
    let places = favorite_places(db).await?;
    let id_of = |key: &str| {
        places
            .iter()
            .find(|(_, other, _)| other == key)
            .map(|(id, _, _)| id.clone())
    };
    let Some(id) = id_of(track_key) else {
        return Ok(());
    };
    let before = before.and_then(id_of);
    let others = places
        .iter()
        .filter(|(other, _, _)| *other != id)
        .map(|(id, _, place)| (id.clone(), *place))
        .collect();
    place_row(db, Ordered::Favorites, &id, before.as_deref(), others).await
}

/// Saves Favorites in the order of `track_keys` (M46.44: a sorted list dragged into shape).
pub async fn set_favorites_order(db: &Db, track_keys: &[String]) -> Result<(), DbError> {
    let places = favorite_places(db).await?;
    let ids: Vec<String> = track_keys
        .iter()
        .filter_map(|key| {
            places
                .iter()
                .find(|(_, other, _)| other == key)
                .map(|(id, _, _)| id.clone())
        })
        .collect();
    let current = places
        .into_iter()
        .map(|(id, _, place)| (id, place))
        .collect();
    set_order(db, Ordered::Favorites, current, &ids).await
}

// ---- Playlists ----

const PLAYLIST_COLUMNS: &str = "p.id, p.name, p.created_at,
     (SELECT COUNT(*) FROM music_playlist_item i
      WHERE i.playlist_id = p.id AND i.deleted = 0) AS item_count,
     (SELECT COALESCE(SUM(i.duration_ms), 0) FROM music_playlist_item i
      WHERE i.playlist_id = p.id AND i.deleted = 0) AS known_ms,
     (SELECT COUNT(i.duration_ms) FROM music_playlist_item i
      WHERE i.playlist_id = p.id AND i.deleted = 0) AS known_count";

/// Live playlists, alphabetically. Their cover items come in one query for all of them.
pub async fn playlists(db: &Db) -> Result<Vec<PlaylistRow>, DbError> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {PLAYLIST_COLUMNS} FROM music_playlist p WHERE p.deleted = 0
         ORDER BY p.name COLLATE NOCASE, p.id"
    )))
    .fetch_all(db.pool())
    .await?;
    let mut first: std::collections::HashMap<String, Vec<TrackFields>> =
        std::collections::HashMap::new();
    for item in sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT playlist_id, {TRACK_COLUMNS} FROM (
             SELECT *, ROW_NUMBER() OVER (PARTITION BY playlist_id ORDER BY position, id) AS place
             FROM music_playlist_item WHERE deleted = 0
         ) WHERE place <= ?1 ORDER BY playlist_id, place"
    )))
    .bind(COVER_ITEMS)
    .fetch_all(db.pool())
    .await?
    {
        first
            .entry(item.get("playlist_id"))
            .or_default()
            .push(track_fields(&item));
    }
    Ok(rows
        .iter()
        .map(|row| {
            let id: String = row.get("id");
            PlaylistRow {
                name: row.get("name"),
                created_at: row.get("created_at"),
                item_count: row.get("item_count"),
                known_ms: row.get("known_ms"),
                known_count: row.get("known_count"),
                first_items: first.remove(&id).unwrap_or_default(),
                id,
            }
        })
        .collect())
}

/// A live playlist by id.
pub async fn playlist(db: &Db, id: &str) -> Result<Option<PlaylistRow>, DbError> {
    let Some(row) = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {PLAYLIST_COLUMNS} FROM music_playlist p WHERE p.id = ?1 AND p.deleted = 0"
    )))
    .bind(id)
    .fetch_optional(db.pool())
    .await?
    else {
        return Ok(None);
    };
    let first_items = items_query(db, id, Some(COVER_ITEMS))
        .await?
        .into_iter()
        .map(|item| item.track)
        .collect();
    Ok(Some(PlaylistRow {
        id: row.get("id"),
        name: row.get("name"),
        created_at: row.get("created_at"),
        item_count: row.get("item_count"),
        known_ms: row.get("known_ms"),
        known_count: row.get("known_count"),
        first_items,
    }))
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
    let others: Vec<(String, f64)> = playlist_items(db, &playlist_id)
        .await?
        .into_iter()
        .filter(|item| item.id != item_id)
        .map(|item| (item.id, item.position))
        .collect();
    place_row(db, Ordered::PlaylistItems, item_id, before, others).await
}

/// Saves a playlist in the order of `item_ids` (M46.44: a sorted list dragged into shape).
pub async fn set_playlist_order(
    db: &Db,
    playlist_id: &str,
    item_ids: &[String],
) -> Result<(), DbError> {
    let current = playlist_items(db, playlist_id)
        .await?
        .into_iter()
        .map(|item| (item.id, item.position))
        .collect();
    set_order(db, Ordered::PlaylistItems, current, item_ids).await
}

/// Puts row `id` before `before` among `others` (in order, without it). Only its row changes,
/// unless the neighbours are too close to split and every row is spaced out first.
async fn place_row(
    db: &Db,
    table: Ordered,
    id: &str,
    before: Option<&str>,
    mut others: Vec<(String, f64)>,
) -> Result<(), DbError> {
    let (mut transaction, stamp) = begin(db).await?;
    let mut position = new_position(&others, before);
    if position.is_none() {
        for (index, (other, position)) in others.iter_mut().enumerate() {
            *position = index as f64 + 1.0;
            set_position(&mut transaction, &stamp, table, other, *position).await?;
        }
        position = new_position(&others, before);
    }
    if let Some(position) = position {
        set_position(&mut transaction, &stamp, table, id, position).await?;
    }
    finish(transaction, stamp).await
}

/// Numbers rows 1, 2, … in the order of `ids`; live rows not named follow in their own order.
/// Rows already at their number aren't written, so sync sends only what moved.
async fn set_order(
    db: &Db,
    table: Ordered,
    current: Vec<(String, f64)>,
    ids: &[String],
) -> Result<(), DbError> {
    let live: HashSet<&str> = current.iter().map(|(id, _)| id.as_str()).collect();
    let mut placed: HashSet<&str> = HashSet::new();
    let mut order: Vec<&str> = Vec::with_capacity(current.len());
    for id in ids
        .iter()
        .map(String::as_str)
        .chain(current.iter().map(|(id, _)| id.as_str()))
    {
        if live.contains(id) && placed.insert(id) {
            order.push(id);
        }
    }
    let (mut transaction, stamp) = begin(db).await?;
    for (index, id) in order.iter().enumerate() {
        let position = index as f64 + 1.0;
        let was = current
            .iter()
            .find(|(other, _)| other == id)
            .map(|(_, place)| *place);
        if was != Some(position) {
            set_position(&mut transaction, &stamp, table, id, position).await?;
        }
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
    table: Ordered,
    id: &str,
    position: f64,
) -> Result<(), DbError> {
    // The table name is one of `Ordered`'s, never from input.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "UPDATE {} SET position = ?2, updated_at_hlc = ?3, origin_device = ?4 WHERE id = ?1",
        table.table()
    )))
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

/// Gives every saved copy of a song (playlists and favorites) a source and fills what it lacked
/// (M46.31: a source found after importing, or one the user picked). A pick (`picked`) also
/// takes the stream's length, since that's what plays now. Returns how many changed.
pub async fn set_track_source(
    db: &Db,
    track_key: &str,
    found: &TrackFields,
    picked: bool,
) -> Result<u64, DbError> {
    let length = if picked {
        "COALESCE(?3, duration_ms)"
    } else {
        "COALESCE(duration_ms, ?3)"
    };
    let (mut transaction, stamp) = begin(db).await?;
    let mut changed = 0;
    for table in ["music_playlist_item", "music_favorite"] {
        let sql = format!(
            "UPDATE {table} SET source = ?1, album = COALESCE(album, ?2),
                 duration_ms = {length}, artwork_url = COALESCE(artwork_url, ?4),
                 updated_at_hlc = ?5, origin_device = ?6
             WHERE track_key = ?7 AND deleted = 0"
        );
        // The table name is one of the constants above.
        changed += sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(&found.source)
            .bind(&found.album)
            .bind(found.duration_ms)
            .bind(&found.artwork_url)
            .bind(stamp.hlc)
            .bind(&stamp.device)
            .bind(track_key)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
    }
    finish(transaction, stamp).await?;
    Ok(changed)
}

/// [`set_track_source`] for rows named by id (M46.48: every saved copy of a song, also those
/// saved under another artist credit or shown with the user's names). Returns how many changed.
pub async fn set_track_source_of(
    db: &Db,
    favorite_ids: &[String],
    item_ids: &[String],
    found: &TrackFields,
    picked: bool,
) -> Result<u64, DbError> {
    if favorite_ids.is_empty() && item_ids.is_empty() {
        return Ok(0);
    }
    let length = if picked {
        "COALESCE(?3, duration_ms)"
    } else {
        "COALESCE(duration_ms, ?3)"
    };
    let (mut transaction, stamp) = begin(db).await?;
    let mut changed = 0;
    for (table, ids) in [
        ("music_favorite", favorite_ids),
        ("music_playlist_item", item_ids),
    ] {
        for id in ids {
            // The table name is one of the constants above.
            changed += sqlx::query(sqlx::AssertSqlSafe(format!(
                "UPDATE {table} SET source = ?1, album = COALESCE(album, ?2),
                     duration_ms = {length}, artwork_url = COALESCE(artwork_url, ?4),
                     updated_at_hlc = ?5, origin_device = ?6
                 WHERE id = ?7 AND deleted = 0"
            )))
            .bind(&found.source)
            .bind(&found.album)
            .bind(found.duration_ms)
            .bind(&found.artwork_url)
            .bind(stamp.hlc)
            .bind(&stamp.device)
            .bind(id)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
        }
    }
    finish(transaction, stamp).await?;
    Ok(changed)
}

/// Marks songs that no source had (M46.31).
pub async fn mark_unavailable(db: &Db, keys: &[String], now_ms: i64) -> Result<(), DbError> {
    for key in keys {
        sqlx::query(
            "INSERT INTO local_track_unavailable (track_key, marked_at) VALUES (?, ?)
             ON CONFLICT (track_key) DO UPDATE SET marked_at = excluded.marked_at",
        )
        .bind(key)
        .bind(now_ms)
        .execute(db.pool())
        .await?;
    }
    Ok(())
}

pub async fn clear_unavailable(db: &Db, key: &str) -> Result<(), DbError> {
    sqlx::query("DELETE FROM local_track_unavailable WHERE track_key = ?")
        .bind(key)
        .execute(db.pool())
        .await?;
    Ok(())
}

/// Marks songs that came in through Import songs (M46.48: picking a source for one offers the
/// source's names by default).
pub async fn mark_imported(db: &Db, keys: &[String], now_ms: i64) -> Result<(), DbError> {
    for key in keys {
        sqlx::query(
            "INSERT INTO local_track_imported (track_key, imported_at) VALUES (?, ?)
             ON CONFLICT (track_key) DO NOTHING",
        )
        .bind(key)
        .bind(now_ms)
        .execute(db.pool())
        .await?;
    }
    Ok(())
}

/// Which of `keys` came in through Import songs.
pub async fn imported(db: &Db, keys: &[String]) -> Result<Vec<String>, DbError> {
    marked(db, "local_track_imported", keys).await
}

/// Which of `keys` are marked unavailable.
pub async fn unavailable(db: &Db, keys: &[String]) -> Result<Vec<String>, DbError> {
    marked(db, "local_track_unavailable", keys).await
}

/// Which of `keys` a device-local mark table holds; `table` is one of the constants above.
async fn marked(db: &Db, table: &'static str, keys: &[String]) -> Result<Vec<String>, DbError> {
    let mut found = Vec::new();
    for chunk in keys.chunks(500) {
        let marks = vec!["?"; chunk.len()].join(",");
        let sql = format!("SELECT track_key FROM {table} WHERE track_key IN ({marks})");
        // Only the table constant and placeholders are formatted in; the keys are bound.
        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
        for key in chunk {
            query = query.bind(key);
        }
        for row in query.fetch_all(db.pool()).await? {
            found.push(row.get("track_key"));
        }
    }
    Ok(found)
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
