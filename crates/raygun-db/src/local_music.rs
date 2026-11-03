//! Storage for the local music library (M07). Device-local: these rows never sync.

use std::collections::HashMap;

use sqlx::sqlite::SqliteRow;
use sqlx::{AssertSqlSafe, Row};

use crate::{Db, DbError};

/// A scanned file to store. Grouping columns are computed by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackRecord {
    pub path: String,
    pub file_size: i64,
    pub file_mtime: i64,
    pub title: String,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub track_no: Option<i64>,
    pub disc_no: Option<i64>,
    pub year: Option<i64>,
    pub genre: Option<String>,
    pub duration_ms: Option<i64>,
    pub art: Option<String>,
    pub group_artist: String,
    pub group_album: String,
    /// The resolver's `artist|title` key, which saved tracks (M18) match on.
    pub track_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackRow {
    pub id: String,
    pub path: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub track_no: Option<i64>,
    pub disc_no: Option<i64>,
    pub year: Option<i64>,
    pub duration_ms: Option<i64>,
    pub art: Option<String>,
    pub group_artist: String,
    pub group_album: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlbumRow {
    pub artist: String,
    pub album: String,
    pub year: Option<i64>,
    pub track_count: i64,
    pub art: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtistRow {
    pub name: String,
    pub album_count: i64,
    pub track_count: i64,
    pub art: Option<String>,
}

// Dynamic SQL in this module splices only these two constants, placeholder lists and numbers,
// never user input, which is why the formatted queries below are marked `AssertSqlSafe`.
const TRACK_COLUMNS: &str = "id, path, title, artist, album, track_no, disc_no, year, duration_ms, \
     art, group_artist, group_album";
const TRACK_ORDER: &str = "group_artist COLLATE NOCASE, group_album COLLATE NOCASE, \
     COALESCE(disc_no, 1), COALESCE(track_no, 0), title COLLATE NOCASE";

pub async fn roots(db: &Db) -> Result<Vec<String>, DbError> {
    let rows = sqlx::query("SELECT path FROM local_library_root ORDER BY path")
        .fetch_all(db.pool())
        .await?;
    Ok(rows.into_iter().map(|row| row.get("path")).collect())
}

pub async fn add_root(db: &Db, path: &str) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO local_library_root (path, added_at) VALUES (?1, unixepoch())
         ON CONFLICT (path) DO NOTHING",
    )
    .bind(path)
    .execute(db.pool())
    .await?;
    Ok(())
}

pub async fn remove_root(db: &Db, path: &str) -> Result<(), DbError> {
    sqlx::query("DELETE FROM local_library_root WHERE path = ?1")
        .bind(path)
        .execute(db.pool())
        .await?;
    Ok(())
}

/// Size and modification time of every stored file, keyed by path, for incremental rescans.
pub async fn known_files(db: &Db) -> Result<HashMap<String, (i64, i64)>, DbError> {
    let rows = sqlx::query("SELECT path, file_size, file_mtime FROM local_track")
        .fetch_all(db.pool())
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.get("path"),
                (row.get("file_size"), row.get("file_mtime")),
            )
        })
        .collect())
}

/// Inserts or updates tracks by path. A rescanned path keeps its id.
pub async fn upsert_tracks(db: &Db, tracks: &[TrackRecord]) -> Result<(), DbError> {
    let mut transaction = db.pool().begin().await?;
    for track in tracks {
        sqlx::query(
            "INSERT INTO local_track (id, path, file_size, file_mtime, title, artist, album_artist,
                 album, track_no, disc_no, year, genre, duration_ms, art, group_artist, group_album,
                 track_key, scanned_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
                 unixepoch())
             ON CONFLICT (path) DO UPDATE SET
                 file_size = excluded.file_size, file_mtime = excluded.file_mtime,
                 title = excluded.title, artist = excluded.artist,
                 album_artist = excluded.album_artist, album = excluded.album,
                 track_no = excluded.track_no, disc_no = excluded.disc_no, year = excluded.year,
                 genre = excluded.genre, duration_ms = excluded.duration_ms, art = excluded.art,
                 group_artist = excluded.group_artist, group_album = excluded.group_album,
                 track_key = excluded.track_key, scanned_at = excluded.scanned_at",
        )
        .bind(uuid::Uuid::now_v7().to_string())
        .bind(&track.path)
        .bind(track.file_size)
        .bind(track.file_mtime)
        .bind(&track.title)
        .bind(&track.artist)
        .bind(&track.album_artist)
        .bind(&track.album)
        .bind(track.track_no)
        .bind(track.disc_no)
        .bind(track.year)
        .bind(&track.genre)
        .bind(track.duration_ms)
        .bind(&track.art)
        .bind(&track.group_artist)
        .bind(&track.group_album)
        .bind(&track.track_key)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(())
}

pub async fn delete_paths(db: &Db, paths: &[String]) -> Result<(), DbError> {
    let mut transaction = db.pool().begin().await?;
    for path in paths {
        sqlx::query("DELETE FROM local_track WHERE path = ?1")
            .bind(path)
            .execute(&mut *transaction)
            .await?;
    }
    transaction.commit().await?;
    Ok(())
}

pub async fn track_count(db: &Db) -> Result<i64, DbError> {
    Ok(sqlx::query("SELECT COUNT(*) AS count FROM local_track")
        .fetch_one(db.pool())
        .await?
        .get("count"))
}

pub async fn albums(db: &Db, artist: Option<&str>) -> Result<Vec<AlbumRow>, DbError> {
    let rows = sqlx::query(
        "SELECT group_artist, group_album, MIN(year) AS year, COUNT(*) AS track_count,
                MAX(art) AS art
         FROM local_track
         WHERE ?1 IS NULL OR group_artist = ?1
         GROUP BY group_artist COLLATE NOCASE, group_album COLLATE NOCASE
         ORDER BY group_album COLLATE NOCASE, group_artist COLLATE NOCASE",
    )
    .bind(artist)
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| AlbumRow {
            artist: row.get("group_artist"),
            album: row.get("group_album"),
            year: row.get("year"),
            track_count: row.get("track_count"),
            art: row.get("art"),
        })
        .collect())
}

pub async fn artists(db: &Db) -> Result<Vec<ArtistRow>, DbError> {
    let rows = sqlx::query(
        "SELECT group_artist, COUNT(DISTINCT group_album) AS album_count,
                COUNT(*) AS track_count, MAX(art) AS art
         FROM local_track
         GROUP BY group_artist COLLATE NOCASE
         ORDER BY group_artist COLLATE NOCASE",
    )
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| ArtistRow {
            name: row.get("group_artist"),
            album_count: row.get("album_count"),
            track_count: row.get("track_count"),
            art: row.get("art"),
        })
        .collect())
}

pub async fn album_tracks(db: &Db, artist: &str, album: &str) -> Result<Vec<TrackRow>, DbError> {
    let rows = sqlx::query(AssertSqlSafe(format!(
        "SELECT {TRACK_COLUMNS} FROM local_track
         WHERE group_artist = ?1 COLLATE NOCASE AND group_album = ?2 COLLATE NOCASE
         ORDER BY {TRACK_ORDER}"
    )))
    .bind(artist)
    .bind(album)
    .fetch_all(db.pool())
    .await?;
    Ok(rows.iter().map(track_row).collect())
}

pub async fn artist_tracks(db: &Db, artist: &str) -> Result<Vec<TrackRow>, DbError> {
    let rows = sqlx::query(AssertSqlSafe(format!(
        "SELECT {TRACK_COLUMNS} FROM local_track
         WHERE group_artist = ?1 COLLATE NOCASE
         ORDER BY {TRACK_ORDER}"
    )))
    .bind(artist)
    .fetch_all(db.pool())
    .await?;
    Ok(rows.iter().map(track_row).collect())
}

/// Every track in library order, a page at a time.
pub async fn tracks(db: &Db, offset: i64, limit: i64) -> Result<Vec<TrackRow>, DbError> {
    let rows = sqlx::query(AssertSqlSafe(format!(
        "SELECT {TRACK_COLUMNS} FROM local_track ORDER BY {TRACK_ORDER} LIMIT ?1 OFFSET ?2"
    )))
    .bind(limit)
    .bind(offset)
    .fetch_all(db.pool())
    .await?;
    Ok(rows.iter().map(track_row).collect())
}

/// Tracks stored before keys existed: `(id, title, artist or album artist)`.
pub async fn keyless_tracks(db: &Db) -> Result<Vec<(String, String, Option<String>)>, DbError> {
    let rows = sqlx::query(
        "SELECT id, title, COALESCE(artist, album_artist) AS artist FROM local_track
         WHERE track_key IS NULL",
    )
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .iter()
        .map(|row| (row.get("id"), row.get("title"), row.get("artist")))
        .collect())
}

/// Every local track's song key (M46.46: their kept lyrics stay).
pub async fn track_keys(db: &Db) -> Result<Vec<String>, DbError> {
    let rows =
        sqlx::query("SELECT DISTINCT track_key FROM local_track WHERE track_key IS NOT NULL")
            .fetch_all(db.pool())
            .await?;
    Ok(rows.iter().map(|row| row.get("track_key")).collect())
}

/// Stores computed keys as `(id, track_key)` pairs.
pub async fn set_track_keys(db: &Db, keys: &[(String, String)]) -> Result<(), DbError> {
    let mut transaction = db.pool().begin().await?;
    for (id, key) in keys {
        sqlx::query("UPDATE local_track SET track_key = ?2 WHERE id = ?1")
            .bind(id)
            .bind(key)
            .execute(&mut *transaction)
            .await?;
    }
    transaction.commit().await?;
    Ok(())
}

/// Local tracks with any of these keys, as `(key, track)`; the first stored track wins per key.
pub async fn tracks_by_keys(
    db: &Db,
    keys: &[String],
) -> Result<HashMap<String, TrackRow>, DbError> {
    let mut found = HashMap::new();
    // SQLite caps bound parameters per statement; 500 stays well under every build's limit.
    for chunk in keys.chunks(500) {
        let placeholders = vec!["?"; chunk.len()].join(", ");
        let sql = format!(
            "SELECT {TRACK_COLUMNS}, track_key FROM local_track WHERE track_key IN ({placeholders})
             ORDER BY id"
        );
        let mut query = sqlx::query(AssertSqlSafe(sql));
        for key in chunk {
            query = query.bind(key);
        }
        for row in query.fetch_all(db.pool()).await? {
            found
                .entry(row.get::<String, _>("track_key"))
                .or_insert_with(|| track_row(&row));
        }
    }
    Ok(found)
}

/// Tracks whose title, artist or album contains every word, in library order.
pub async fn search(db: &Db, words: &[String], limit: i64) -> Result<Vec<TrackRow>, DbError> {
    if words.is_empty() {
        return Ok(Vec::new());
    }
    let conditions = (1..=words.len())
        .map(|index| {
            format!(
                "(title LIKE ?{index} ESCAPE '\\' OR artist LIKE ?{index} ESCAPE '\\' \
                 OR album LIKE ?{index} ESCAPE '\\')"
            )
        })
        .collect::<Vec<_>>()
        .join(" AND ");
    let sql = format!(
        "SELECT {TRACK_COLUMNS} FROM local_track WHERE {conditions} ORDER BY {TRACK_ORDER} \
         LIMIT {limit}"
    );
    let mut query = sqlx::query(AssertSqlSafe(sql));
    for word in words {
        let escaped = word
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        query = query.bind(format!("%{escaped}%"));
    }
    Ok(query
        .fetch_all(db.pool())
        .await?
        .iter()
        .map(track_row)
        .collect())
}

fn track_row(row: &SqliteRow) -> TrackRow {
    TrackRow {
        id: row.get("id"),
        path: row.get("path"),
        title: row.get("title"),
        artist: row.get("artist"),
        album: row.get("album"),
        track_no: row.get("track_no"),
        disc_no: row.get("disc_no"),
        year: row.get("year"),
        duration_ms: row.get("duration_ms"),
        art: row.get("art"),
        group_artist: row.get("group_artist"),
        group_album: row.get("group_album"),
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn record(path: &str, artist: &str, album: &str, track_no: i64) -> TrackRecord {
        TrackRecord {
            path: path.into(),
            file_size: 100,
            file_mtime: 1,
            title: format!("Track {track_no}"),
            artist: Some(artist.into()),
            album_artist: None,
            album: Some(album.into()),
            track_no: Some(track_no),
            disc_no: None,
            year: Some(2020),
            genre: None,
            duration_ms: Some(1000),
            art: (track_no == 2).then(|| "cover.png".into()),
            group_artist: artist.into(),
            group_album: album.into(),
            track_key: format!("{}|track {track_no}", artist.to_lowercase()),
        }
    }

    async fn open(temp: &TempDir) -> Db {
        Db::open(&temp.path().join("raygun.db")).await.unwrap()
    }

    #[tokio::test]
    async fn roots_are_unique() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;

        add_root(&db, "/sdcard/Music").await.unwrap();
        add_root(&db, "/sdcard/Music").await.unwrap();
        add_root(&db, "/sdcard/Download").await.unwrap();
        remove_root(&db, "/sdcard/Download").await.unwrap();

        assert_eq!(roots(&db).await.unwrap(), vec!["/sdcard/Music".to_owned()]);
    }

    #[tokio::test]
    async fn groups_tracks_into_albums_and_artists() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        upsert_tracks(
            &db,
            &[
                record("/m/a/2.mp3", "Zeta", "Second", 2),
                record("/m/a/1.mp3", "Zeta", "Second", 1),
                record("/m/b/1.mp3", "Alpha", "First", 1),
                record("/m/c/1.mp3", "Zeta", "Another", 1),
            ],
        )
        .await
        .unwrap();

        let albums = albums(&db, None).await.unwrap();
        assert_eq!(
            albums
                .iter()
                .map(|album| album.album.as_str())
                .collect::<Vec<_>>(),
            vec!["Another", "First", "Second"]
        );
        let second = albums.iter().find(|album| album.album == "Second").unwrap();
        assert_eq!(second.track_count, 2);
        assert_eq!(second.art.as_deref(), Some("cover.png"));

        let artists = artists(&db).await.unwrap();
        assert_eq!(artists[0].name, "Alpha");
        assert_eq!(artists[1].name, "Zeta");
        assert_eq!(artists[1].album_count, 2);
        assert_eq!(artists[1].track_count, 3);

        assert_eq!(albums_of(&db, "Zeta").await, vec!["Another", "Second"]);
    }

    async fn albums_of(db: &Db, artist: &str) -> Vec<String> {
        albums(db, Some(artist))
            .await
            .unwrap()
            .into_iter()
            .map(|album| album.album)
            .collect()
    }

    #[tokio::test]
    async fn album_tracks_follow_track_numbers() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        upsert_tracks(
            &db,
            &[
                record("/m/a/3.mp3", "Zeta", "Second", 3),
                record("/m/a/1.mp3", "Zeta", "Second", 1),
                record("/m/a/2.mp3", "Zeta", "Second", 2),
            ],
        )
        .await
        .unwrap();

        let tracks = album_tracks(&db, "zeta", "second").await.unwrap();

        assert_eq!(
            tracks
                .iter()
                .map(|track| track.track_no)
                .collect::<Vec<_>>(),
            vec![Some(1), Some(2), Some(3)]
        );
    }

    #[tokio::test]
    async fn rescanning_a_path_updates_it_and_keeps_its_id() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        upsert_tracks(&db, &[record("/m/a/1.mp3", "Zeta", "Second", 1)])
            .await
            .unwrap();
        let id = tracks(&db, 0, 10).await.unwrap()[0].id.clone();

        let mut changed = record("/m/a/1.mp3", "Zeta", "Second", 1);
        changed.title = "Renamed".into();
        changed.file_mtime = 2;
        upsert_tracks(&db, &[changed]).await.unwrap();

        let stored = tracks(&db, 0, 10).await.unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].id, id);
        assert_eq!(stored[0].title, "Renamed");
        assert_eq!(known_files(&db).await.unwrap()["/m/a/1.mp3"], (100, 2));
    }

    #[tokio::test]
    async fn deleting_paths_and_paging() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        let records: Vec<_> = (1..=5)
            .map(|index| record(&format!("/m/{index}.mp3"), "A", "B", index))
            .collect();
        upsert_tracks(&db, &records).await.unwrap();

        delete_paths(&db, &["/m/1.mp3".into(), "/m/2.mp3".into()])
            .await
            .unwrap();

        assert_eq!(track_count(&db).await.unwrap(), 3);
        let page = tracks(&db, 1, 1).await.unwrap();
        assert_eq!(page[0].track_no, Some(4));
    }

    #[tokio::test]
    async fn search_matches_every_word_and_keys_find_tracks() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        let mut odd = record("/m/c.flac", "Bob", "100% Pure_Gold", 3);
        odd.title = "Under_score".into();
        upsert_tracks(
            &db,
            &[
                record("/m/a.flac", "Ann", "Blue", 1),
                record("/m/b.flac", "Ann", "Red", 2),
                odd,
            ],
        )
        .await
        .unwrap();

        let words = |text: &str| text.split(' ').map(String::from).collect::<Vec<_>>();
        let found = |rows: Vec<TrackRow>| rows.into_iter().map(|row| row.path).collect::<Vec<_>>();
        assert_eq!(
            found(search(&db, &words("ann red"), 10).await.unwrap()),
            ["/m/b.flac"]
        );
        assert_eq!(found(search(&db, &words("ANN"), 1).await.unwrap()).len(), 1);
        assert_eq!(
            found(search(&db, &words("100%"), 10).await.unwrap()),
            ["/m/c.flac"]
        );
        assert!(
            search(&db, &words("ann_"), 10).await.unwrap().is_empty(),
            "_ is literal"
        );
        // As a wildcard, "d_r" would match "Under"; escaped, only a literal "r_s" does.
        assert!(search(&db, &words("d_r"), 10).await.unwrap().is_empty());
        assert_eq!(
            found(search(&db, &words("r_s"), 10).await.unwrap()),
            ["/m/c.flac"]
        );
        assert!(search(&db, &[], 10).await.unwrap().is_empty());

        let by_key = tracks_by_keys(&db, &["ann|track 2".into(), "nobody|x".into()])
            .await
            .unwrap();
        assert_eq!(by_key.len(), 1);
        assert_eq!(by_key["ann|track 2"].path, "/m/b.flac");
    }

    #[tokio::test]
    async fn keyless_tracks_get_keys() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        upsert_tracks(&db, &[record("/m/a.flac", "Ann", "Blue", 1)])
            .await
            .unwrap();
        sqlx::query("UPDATE local_track SET track_key = NULL")
            .execute(db.pool())
            .await
            .unwrap();
        let keyless = keyless_tracks(&db).await.unwrap();
        assert_eq!(keyless.len(), 1);
        assert_eq!(keyless[0].1, "Track 1");
        set_track_keys(&db, &[(keyless[0].0.clone(), "ann|track 1".into())])
            .await
            .unwrap();
        assert!(keyless_tracks(&db).await.unwrap().is_empty());
    }
}
