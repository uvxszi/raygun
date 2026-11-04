//! Storage for podcasts (M37): feeds, their episodes, listening progress and downloads.
//! Refreshing a feed rewrites only `remote_*` columns; subscriptions and progress stay.

use sqlx::sqlite::SqliteRow;
use sqlx::{AssertSqlSafe, Row};
use uuid::Uuid;

use crate::{Db, DbError, persist_hlc};

/// What a feed says about a podcast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodcastFields {
    pub feed_url: String,
    pub title: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub artwork: Option<String>,
    pub link: Option<String>,
    /// `podcast` or `audiobook` (M46.22).
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PodcastRow {
    pub id: String,
    pub fields: PodcastFields,
    pub subscribed_at: Option<i64>,
    pub refreshed_at: Option<i64>,
    /// How its episodes list, as JSON (M46.48); `None` for the kind's own order.
    pub episode_order: Option<String>,
    /// The speed its episodes play at (M46.48); `None` leaves the player's.
    pub speed: Option<f64>,
}

/// A subscription with its episode counts.
#[derive(Debug, Clone, PartialEq)]
pub struct Subscription {
    pub podcast: PodcastRow,
    pub episodes: i64,
    /// Unplayed episodes found after subscribing.
    pub unplayed: i64,
    pub latest_published_at: Option<i64>,
}

/// What a feed says about an episode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpisodeFields {
    pub guid: String,
    pub title: String,
    pub description: Option<String>,
    pub audio_url: String,
    pub audio_type: Option<String>,
    pub duration_ms: Option<i64>,
    pub published_at: Option<i64>,
    pub artwork: Option<String>,
    /// Its number in a serial feed (an audiobook's chapter), M46.22.
    pub number: Option<i64>,
    /// Its transcript as the feed links it, `<type>|<url>` (M46.32).
    pub transcript: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EpisodeRow {
    pub id: String,
    pub podcast_id: String,
    pub fields: EpisodeFields,
    pub position_ms: i64,
    pub played_at: Option<i64>,
    pub found_at: i64,
    /// When listening last moved it on (M46.48).
    pub listened_at: Option<i64>,
    /// From the podcast, for lists that mix podcasts.
    pub podcast_title: String,
    pub podcast_author: Option<String>,
    pub podcast_artwork: Option<String>,
    /// `podcast` or `audiobook`.
    pub podcast_kind: String,
    /// The show's own speed (M46.48).
    pub podcast_speed: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpisodeDownload {
    pub episode_id: String,
    /// `downloading`, `done` or `failed`.
    pub state: String,
    pub path: Option<String>,
    pub received_bytes: i64,
    pub total_bytes: Option<i64>,
    pub error: Option<String>,
}

const PODCAST_COLUMNS: &str = "p.id, p.feed_url, p.remote_title, p.remote_author, \
     p.remote_description, p.remote_artwork, p.remote_link, p.remote_kind, p.user_subscribed_at, \
     p.state_refreshed_at, p.user_episode_order, p.user_speed";

const EPISODE_COLUMNS: &str = "e.id, e.podcast_id, e.guid, e.remote_title, e.remote_description, \
     e.remote_audio_url, e.remote_audio_type, e.remote_duration_ms, e.remote_published_at, \
     e.remote_artwork, e.remote_number, e.remote_transcript, e.user_position_ms, e.user_played_at, \
     e.state_found_at, e.user_listened_at, p.remote_title AS podcast_title, \
     p.remote_author AS podcast_author, p.remote_artwork AS podcast_artwork, \
     p.remote_kind AS podcast_kind, p.user_speed AS podcast_speed";

fn podcast_row(row: &SqliteRow) -> PodcastRow {
    PodcastRow {
        id: row.get("id"),
        fields: PodcastFields {
            feed_url: row.get("feed_url"),
            title: row.get("remote_title"),
            author: row.get("remote_author"),
            description: row.get("remote_description"),
            artwork: row.get("remote_artwork"),
            link: row.get("remote_link"),
            kind: row.get("remote_kind"),
        },
        subscribed_at: row.get("user_subscribed_at"),
        refreshed_at: row.get("state_refreshed_at"),
        episode_order: row.get("user_episode_order"),
        speed: row.get("user_speed"),
    }
}

fn episode_row(row: &SqliteRow) -> EpisodeRow {
    EpisodeRow {
        id: row.get("id"),
        podcast_id: row.get("podcast_id"),
        fields: EpisodeFields {
            guid: row.get("guid"),
            title: row.get("remote_title"),
            description: row.get("remote_description"),
            audio_url: row.get("remote_audio_url"),
            audio_type: row.get("remote_audio_type"),
            duration_ms: row.get("remote_duration_ms"),
            published_at: row.get("remote_published_at"),
            artwork: row.get("remote_artwork"),
            number: row.get("remote_number"),
            transcript: row.get("remote_transcript"),
        },
        position_ms: row.get("user_position_ms"),
        played_at: row.get("user_played_at"),
        found_at: row.get("state_found_at"),
        listened_at: row.get("user_listened_at"),
        podcast_title: row.get("podcast_title"),
        podcast_author: row.get("podcast_author"),
        podcast_artwork: row.get("podcast_artwork"),
        podcast_kind: row.get("podcast_kind"),
        podcast_speed: row.get("podcast_speed"),
    }
}

/// Stores what a feed said about a podcast (inserting it on first sight) and returns its id.
pub async fn upsert_podcast(
    db: &Db,
    fields: &PodcastFields,
    now_ms: i64,
) -> Result<String, DbError> {
    let stamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    let existing: Option<String> =
        sqlx::query("SELECT id FROM podcast WHERE feed_url = ?1 AND deleted = 0")
            .bind(&fields.feed_url)
            .fetch_optional(&mut *transaction)
            .await?
            .map(|row| row.get("id"));
    let id = existing.unwrap_or_else(|| Uuid::now_v7().to_string());
    sqlx::query(
        "INSERT INTO podcast (id, feed_url, remote_title, remote_author, remote_description,
             remote_artwork, remote_link, state_refreshed_at, updated_at_hlc, origin_device, deleted,
             remote_kind)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, ?11)
         ON CONFLICT (id) DO UPDATE SET
             remote_title = excluded.remote_title, remote_author = excluded.remote_author,
             remote_description = excluded.remote_description,
             remote_artwork = excluded.remote_artwork, remote_link = excluded.remote_link,
             remote_kind = excluded.remote_kind,
             state_refreshed_at = excluded.state_refreshed_at,
             updated_at_hlc = excluded.updated_at_hlc, origin_device = excluded.origin_device",
    )
    .bind(&id)
    .bind(&fields.feed_url)
    .bind(&fields.title)
    .bind(&fields.author)
    .bind(&fields.description)
    .bind(&fields.artwork)
    .bind(&fields.link)
    .bind(now_ms)
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .bind(&fields.kind)
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(id)
}

/// Adds new episodes and refreshes known ones (matched by guid). Returns how many were new.
pub async fn sync_episodes(
    db: &Db,
    podcast_id: &str,
    episodes: &[EpisodeFields],
    now_ms: i64,
) -> Result<usize, DbError> {
    let stamp = db.tick();
    let device = db.device_id().to_string();
    let mut transaction = db.pool().begin().await?;
    let mut added = 0;
    for episode in episodes {
        let existing: Option<String> = sqlx::query(
            "SELECT id FROM podcast_episode WHERE podcast_id = ?1 AND guid = ?2 AND deleted = 0",
        )
        .bind(podcast_id)
        .bind(&episode.guid)
        .fetch_optional(&mut *transaction)
        .await?
        .map(|row| row.get("id"));
        added += usize::from(existing.is_none());
        let id = existing.unwrap_or_else(|| Uuid::now_v7().to_string());
        // `state_found_at` is set on insert only.
        sqlx::query(
            "INSERT INTO podcast_episode (id, podcast_id, guid, remote_title, remote_description,
                 remote_audio_url, remote_audio_type, remote_duration_ms, remote_published_at,
                 remote_artwork, state_found_at, updated_at_hlc, origin_device, deleted,
                 remote_number, remote_transcript)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 0, ?14, ?15)
             ON CONFLICT (id) DO UPDATE SET
                 remote_number = excluded.remote_number,
                 remote_transcript = excluded.remote_transcript,
                 remote_title = excluded.remote_title,
                 remote_description = excluded.remote_description,
                 remote_audio_url = excluded.remote_audio_url,
                 remote_audio_type = excluded.remote_audio_type,
                 remote_duration_ms = excluded.remote_duration_ms,
                 remote_published_at = excluded.remote_published_at,
                 remote_artwork = excluded.remote_artwork,
                 updated_at_hlc = excluded.updated_at_hlc, origin_device = excluded.origin_device",
        )
        .bind(&id)
        .bind(podcast_id)
        .bind(&episode.guid)
        .bind(&episode.title)
        .bind(&episode.description)
        .bind(&episode.audio_url)
        .bind(&episode.audio_type)
        .bind(episode.duration_ms)
        .bind(episode.published_at)
        .bind(&episode.artwork)
        .bind(now_ms)
        .bind(stamp as i64)
        .bind(&device)
        .bind(episode.number)
        .bind(&episode.transcript)
        .execute(&mut *transaction)
        .await?;
    }
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(added)
}

/// Subscribes (`Some(now)`) or unsubscribes (`None`).
pub async fn set_subscribed(db: &Db, podcast_id: &str, at: Option<i64>) -> Result<(), DbError> {
    update_podcast(db, podcast_id, "user_subscribed_at", Value::Integer(at)).await
}

/// How a show's or book's episodes list (M46.48): JSON, or `None` for the kind's own order.
pub async fn set_episode_order(
    db: &Db,
    podcast_id: &str,
    order: Option<&str>,
) -> Result<(), DbError> {
    update_podcast(
        db,
        podcast_id,
        "user_episode_order",
        Value::Text(order.map(str::to_owned)),
    )
    .await
}

/// The speed a show's or book's episodes play at (M46.48); `None` leaves the player's.
pub async fn set_speed(db: &Db, podcast_id: &str, speed: Option<f64>) -> Result<(), DbError> {
    update_podcast(db, podcast_id, "user_speed", Value::Real(speed)).await
}

/// A value for one of the podcast's own columns.
enum Value {
    Integer(Option<i64>),
    Text(Option<String>),
    Real(Option<f64>),
}

async fn update_podcast(
    db: &Db,
    podcast_id: &str,
    column: &'static str,
    value: Value,
) -> Result<(), DbError> {
    let stamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    let sql = format!(
        "UPDATE podcast SET {column} = ?2, updated_at_hlc = ?3, origin_device = ?4
         WHERE id = ?1 AND deleted = 0"
    );
    let query = sqlx::query(AssertSqlSafe(sql)).bind(podcast_id);
    let query = match value {
        Value::Integer(value) => query.bind(value),
        Value::Text(value) => query.bind(value),
        Value::Real(value) => query.bind(value),
    };
    query
        .bind(stamp as i64)
        .bind(db.device_id().to_string())
        .execute(&mut *transaction)
        .await?;
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(())
}

pub async fn podcast(db: &Db, id: &str) -> Result<Option<PodcastRow>, DbError> {
    let row = sqlx::query(AssertSqlSafe(format!(
        "SELECT {PODCAST_COLUMNS} FROM podcast p WHERE p.id = ?1 AND p.deleted = 0"
    )))
    .bind(id)
    .fetch_optional(db.pool())
    .await?;
    Ok(row.as_ref().map(podcast_row))
}

pub async fn podcast_by_feed(db: &Db, feed_url: &str) -> Result<Option<PodcastRow>, DbError> {
    let row = sqlx::query(AssertSqlSafe(format!(
        "SELECT {PODCAST_COLUMNS} FROM podcast p WHERE p.feed_url = ?1 AND p.deleted = 0"
    )))
    .bind(feed_url)
    .fetch_optional(db.pool())
    .await?;
    Ok(row.as_ref().map(podcast_row))
}

/// Subscribed podcasts, most recently updated first.
pub async fn subscriptions(db: &Db) -> Result<Vec<Subscription>, DbError> {
    let rows = sqlx::query(AssertSqlSafe(format!(
        "SELECT {PODCAST_COLUMNS},
             COUNT(e.id) AS episodes,
             -- New: unplayed episodes that arrived after subscribing (not the back catalogue).
             COALESCE(SUM(e.user_played_at IS NULL AND e.state_found_at > p.user_subscribed_at), 0)
                 AS unplayed,
             MAX(e.remote_published_at) AS latest
         FROM podcast p
         LEFT JOIN podcast_episode e ON e.podcast_id = p.id AND e.deleted = 0
         WHERE p.user_subscribed_at IS NOT NULL AND p.deleted = 0 AND p.remote_kind = 'podcast'
         GROUP BY p.id
         ORDER BY latest IS NULL, latest DESC, p.remote_title"
    )))
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .iter()
        .map(|row| Subscription {
            podcast: podcast_row(row),
            episodes: row.get("episodes"),
            unplayed: row.get("unplayed"),
            latest_published_at: row.get("latest"),
        })
        .collect())
}

/// A podcast's episodes, newest first.
pub async fn episodes(db: &Db, podcast_id: &str) -> Result<Vec<EpisodeRow>, DbError> {
    let rows = sqlx::query(AssertSqlSafe(format!(
        "SELECT {EPISODE_COLUMNS} FROM podcast_episode e JOIN podcast p ON p.id = e.podcast_id
         WHERE e.podcast_id = ?1 AND e.deleted = 0
         ORDER BY
             -- A book's chapters in order (M46.22); a podcast's newest first.
             CASE WHEN p.remote_kind = 'audiobook' THEN COALESCE(e.remote_number, 0) END,
             e.remote_published_at IS NULL, e.remote_published_at DESC, e.state_found_at DESC"
    )))
    .bind(podcast_id)
    .fetch_all(db.pool())
    .await?;
    Ok(rows.iter().map(episode_row).collect())
}

/// A book with how far it was listened (M46.22).
#[derive(Debug, Clone, PartialEq)]
pub struct BookRow {
    pub podcast: PodcastRow,
    pub chapters: i64,
    pub finished: i64,
    /// When listening last moved it on (unix ms).
    pub listened_at: Option<i64>,
    /// Its length, and how much of it was listened (M46.48).
    pub total_ms: i64,
    pub listened_ms: i64,
}

/// What a book's chapters add up to, for `BookRow`.
const BOOK_PROGRESS: &str = "COUNT(e.id) AS chapters,
     COALESCE(SUM(e.user_played_at IS NOT NULL), 0) AS finished,
     MAX(COALESCE(e.user_listened_at, e.user_played_at)) AS listened,
     COALESCE(SUM(e.remote_duration_ms), 0) AS total_ms,
     COALESCE(SUM(CASE WHEN e.user_played_at IS NOT NULL THEN COALESCE(e.remote_duration_ms, 0)
         ELSE MIN(e.user_position_ms, COALESCE(e.remote_duration_ms, e.user_position_ms)) END), 0)
         AS listened_ms";

fn book_row(row: &SqliteRow) -> BookRow {
    BookRow {
        podcast: podcast_row(row),
        chapters: row.get("chapters"),
        finished: row.get("finished"),
        listened_at: row.get("listened"),
        total_ms: row.get("total_ms"),
        listened_ms: row.get("listened_ms"),
    }
}

/// The audiobooks in the library, the last listened first.
pub async fn audiobooks(db: &Db) -> Result<Vec<BookRow>, DbError> {
    let rows = sqlx::query(AssertSqlSafe(format!(
        "SELECT {PODCAST_COLUMNS}, {BOOK_PROGRESS}
         FROM podcast p
         LEFT JOIN podcast_episode e ON e.podcast_id = p.id AND e.deleted = 0
         WHERE p.user_subscribed_at IS NOT NULL AND p.deleted = 0 AND p.remote_kind = 'audiobook'
         GROUP BY p.id
         ORDER BY listened IS NULL, listened DESC, p.user_subscribed_at DESC"
    )))
    .fetch_all(db.pool())
    .await?;
    Ok(rows.iter().map(book_row).collect())
}

/// Books started and not finished, in the library or not, the last listened first (M46.48).
pub async fn books_in_progress(db: &Db, limit: u32) -> Result<Vec<BookRow>, DbError> {
    let rows = sqlx::query(AssertSqlSafe(format!(
        "SELECT {PODCAST_COLUMNS}, {BOOK_PROGRESS}
         FROM podcast p
         JOIN podcast_episode e ON e.podcast_id = p.id AND e.deleted = 0
         WHERE p.deleted = 0 AND p.remote_kind = 'audiobook'
         GROUP BY p.id
         HAVING finished < chapters AND (finished > 0 OR SUM(e.user_position_ms > 0) > 0)
         ORDER BY listened IS NULL, listened DESC
         LIMIT ?1"
    )))
    .bind(i64::from(limit))
    .fetch_all(db.pool())
    .await?;
    Ok(rows.iter().map(book_row).collect())
}

/// Podcast episodes started and not finished, the last listened first (M46.48).
pub async fn episodes_in_progress(db: &Db, limit: u32) -> Result<Vec<EpisodeRow>, DbError> {
    let rows = sqlx::query(AssertSqlSafe(format!(
        "SELECT {EPISODE_COLUMNS} FROM podcast_episode e JOIN podcast p ON p.id = e.podcast_id
         WHERE e.deleted = 0 AND p.deleted = 0 AND p.remote_kind = 'podcast'
             AND e.user_played_at IS NULL AND e.user_position_ms > 0
         ORDER BY e.user_listened_at IS NULL, e.user_listened_at DESC
         LIMIT ?1"
    )))
    .bind(i64::from(limit))
    .fetch_all(db.pool())
    .await?;
    Ok(rows.iter().map(episode_row).collect())
}

/// The artwork of every followed show and every book in the library (M46.2: kept covers).
pub async fn library_artwork(db: &Db) -> Result<Vec<String>, DbError> {
    Ok(sqlx::query(
        "SELECT remote_artwork FROM podcast
         WHERE user_subscribed_at IS NOT NULL AND deleted = 0 AND remote_artwork IS NOT NULL",
    )
    .fetch_all(db.pool())
    .await?
    .iter()
    .map(|row| row.get("remote_artwork"))
    .collect())
}

/// The newest episodes across subscriptions.
pub async fn latest_episodes(db: &Db, limit: u32) -> Result<Vec<EpisodeRow>, DbError> {
    let rows = sqlx::query(AssertSqlSafe(format!(
        "SELECT {EPISODE_COLUMNS} FROM podcast_episode e JOIN podcast p ON p.id = e.podcast_id
         WHERE p.user_subscribed_at IS NOT NULL AND p.deleted = 0 AND e.deleted = 0
             AND p.remote_kind = 'podcast'
         ORDER BY e.remote_published_at IS NULL, e.remote_published_at DESC
         LIMIT ?1"
    )))
    .bind(i64::from(limit))
    .fetch_all(db.pool())
    .await?;
    Ok(rows.iter().map(episode_row).collect())
}

pub async fn episode(db: &Db, id: &str) -> Result<Option<EpisodeRow>, DbError> {
    let row = sqlx::query(AssertSqlSafe(format!(
        "SELECT {EPISODE_COLUMNS} FROM podcast_episode e JOIN podcast p ON p.id = e.podcast_id
         WHERE e.id = ?1 AND e.deleted = 0"
    )))
    .bind(id)
    .fetch_optional(db.pool())
    .await?;
    Ok(row.as_ref().map(episode_row))
}

/// The episode playing from `audio_url` (its feed URL), for following playback.
pub async fn episode_by_audio(db: &Db, audio_url: &str) -> Result<Option<String>, DbError> {
    Ok(sqlx::query(
        "SELECT id FROM podcast_episode WHERE remote_audio_url = ?1 AND deleted = 0 LIMIT 1",
    )
    .bind(audio_url)
    .fetch_optional(db.pool())
    .await?
    .map(|row| row.get("id")))
}

/// Where listening stopped, at `now_ms`. Unchanged positions write nothing.
pub async fn set_position(
    db: &Db,
    episode_id: &str,
    position_ms: i64,
    now_ms: i64,
) -> Result<(), DbError> {
    let stamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    let changed = sqlx::query(
        "UPDATE podcast_episode SET user_position_ms = ?2, user_listened_at = ?5,
             updated_at_hlc = ?3, origin_device = ?4
         WHERE id = ?1 AND deleted = 0 AND user_position_ms != ?2",
    )
    .bind(episode_id)
    .bind(position_ms.max(0))
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .bind(now_ms)
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    if changed > 0 {
        persist_hlc(&mut *transaction, stamp).await?;
    }
    transaction.commit().await?;
    Ok(())
}

/// Marks an episode played (`Some(now)`, which also rewinds it) or unplayed (`None`).
pub async fn set_played(db: &Db, episode_id: &str, at: Option<i64>) -> Result<(), DbError> {
    set_played_many(db, &[episode_id.to_owned()], at).await
}

/// Marks episodes played (`Some(now)`, which also rewinds them) or unplayed (`None`) at once.
pub async fn set_played_many(
    db: &Db,
    episode_ids: &[String],
    at: Option<i64>,
) -> Result<(), DbError> {
    if episode_ids.is_empty() {
        return Ok(());
    }
    let stamp = db.tick();
    let device = db.device_id().to_string();
    let mut transaction = db.pool().begin().await?;
    for episode_id in episode_ids {
        sqlx::query(
            "UPDATE podcast_episode SET user_played_at = ?2,
                 user_position_ms = CASE WHEN ?2 IS NULL THEN user_position_ms ELSE 0 END,
                 user_listened_at = COALESCE(?2, user_listened_at),
                 updated_at_hlc = ?3, origin_device = ?4
             WHERE id = ?1 AND deleted = 0",
        )
        .bind(episode_id)
        .bind(at)
        .bind(stamp as i64)
        .bind(&device)
        .execute(&mut *transaction)
        .await?;
    }
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(())
}

/// Forgets where listening stopped in an episode, without marking it played (M46.48).
pub async fn clear_position(db: &Db, episode_id: &str) -> Result<(), DbError> {
    let stamp = db.tick();
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "UPDATE podcast_episode SET user_position_ms = 0, updated_at_hlc = ?2, origin_device = ?3
         WHERE id = ?1 AND deleted = 0",
    )
    .bind(episode_id)
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .execute(&mut *transaction)
    .await?;
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(())
}

// ---- Downloads (device-local) ----

fn download_row(row: &SqliteRow) -> EpisodeDownload {
    EpisodeDownload {
        episode_id: row.get("episode_id"),
        state: row.get("state"),
        path: row.get("path"),
        received_bytes: row.get("received_bytes"),
        total_bytes: row.get("total_bytes"),
        error: row.get("error"),
    }
}

pub async fn downloads(db: &Db) -> Result<Vec<EpisodeDownload>, DbError> {
    let rows = sqlx::query("SELECT * FROM local_podcast_download")
        .fetch_all(db.pool())
        .await?;
    Ok(rows.iter().map(download_row).collect())
}

pub async fn download(db: &Db, episode_id: &str) -> Result<Option<EpisodeDownload>, DbError> {
    let row = sqlx::query("SELECT * FROM local_podcast_download WHERE episode_id = ?1")
        .bind(episode_id)
        .fetch_optional(db.pool())
        .await?;
    Ok(row.as_ref().map(download_row))
}

pub async fn put_download(db: &Db, download: &EpisodeDownload, now_ms: i64) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO local_podcast_download
             (episode_id, state, path, received_bytes, total_bytes, error, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT (episode_id) DO UPDATE SET
             state = excluded.state, path = excluded.path,
             received_bytes = excluded.received_bytes, total_bytes = excluded.total_bytes,
             error = excluded.error, updated_at = excluded.updated_at",
    )
    .bind(&download.episode_id)
    .bind(&download.state)
    .bind(&download.path)
    .bind(download.received_bytes)
    .bind(download.total_bytes)
    .bind(&download.error)
    .bind(now_ms)
    .execute(db.pool())
    .await?;
    Ok(())
}

pub async fn remove_download(db: &Db, episode_id: &str) -> Result<(), DbError> {
    sqlx::query("DELETE FROM local_podcast_download WHERE episode_id = ?1")
        .bind(episode_id)
        .execute(db.pool())
        .await?;
    Ok(())
}

/// The episode playing from a downloaded file at `path`.
pub async fn episode_by_download(db: &Db, path: &str) -> Result<Option<String>, DbError> {
    Ok(sqlx::query(
        "SELECT episode_id FROM local_podcast_download WHERE path = ?1 AND state = 'done'",
    )
    .bind(path)
    .fetch_optional(db.pool())
    .await?
    .map(|row| row.get("episode_id")))
}

#[cfg(test)]
#[path = "podcasts_tests.rs"]
mod tests;
