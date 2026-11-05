//! Settings → Data (M46.51): what each part of the database holds here, counted as a backup's
//! index counts it, and clearing a part.
//!
//! Clearing writes, never deletes: rows become tombstones (or leave the library) stamped with a
//! fresh HLC timestamp from this device, so device sync carries a clear of a synced part
//! (playlists, settings, played songs) to linked devices like any other change, and a restore
//! or a sync never brings back rows a newer clear removed. Parts that only live in services
//! (pins, sources, downloads) are cleared by the app.

use sqlx::{AssertSqlSafe, Row, Sqlite, Transaction};

use crate::backup::{Counts, Item, Part, finished_item, followed_item, units_item};
use crate::{Db, DbError, persist_hlc};

/// What the database holds, part by part, counted as [`crate::backup::index`] counts a backup.
pub async fn counts(db: &Db) -> Result<Counts, DbError> {
    let mut counts = Counts::new();
    let mut add = |part: Part, item: Item, count: i64| {
        if count > 0 {
            *counts.entry(part).or_default().entry(item).or_default() += count as u32;
        }
    };
    let grouped = |sql: &'static str| async move {
        Ok::<_, DbError>(
            sqlx::query(sql)
                .fetch_all(db.pool())
                .await?
                .iter()
                .map(|row| (row.get::<String, _>(0), row.get::<i64, _>(1)))
                .collect::<Vec<_>>(),
        )
    };
    for (kind, count) in grouped(
        "SELECT kind, COUNT(*) FROM title
         WHERE deleted = 0 AND user_in_library_at IS NOT NULL GROUP BY kind",
    )
    .await?
    {
        if let Some(part) = title_part(&kind) {
            add(part, Item::Titles, count);
        }
    }
    for (kind, count) in grouped(
        "SELECT t.kind, COUNT(*) FROM unit u JOIN title t ON t.id = u.title_id
         WHERE u.deleted = 0 AND t.deleted = 0 AND t.user_in_library_at IS NOT NULL
         GROUP BY t.kind",
    )
    .await?
    {
        if let Some(part) = title_part(&kind) {
            add(part, units_item(part), count);
        }
    }
    for (kind, count) in grouped(
        "SELECT t.kind, COUNT(*) FROM unit_progress p
         JOIN unit u ON u.id = p.unit_id JOIN title t ON t.id = u.title_id
         WHERE p.deleted = 0 AND p.completed_at IS NOT NULL AND u.deleted = 0
             AND t.deleted = 0 AND t.user_in_library_at IS NOT NULL
         GROUP BY t.kind",
    )
    .await?
    {
        if let Some(part) = title_part(&kind) {
            add(part, finished_item(part), count);
        }
    }
    for (kind, count) in
        grouped("SELECT kind, COUNT(*) FROM category WHERE deleted = 0 GROUP BY kind").await?
    {
        if let Some(part) = title_part(&kind) {
            add(part, Item::Categories, count);
        }
    }
    for (kind, count) in grouped(
        "SELECT remote_kind, COUNT(*) FROM podcast
         WHERE deleted = 0 AND user_subscribed_at IS NOT NULL GROUP BY remote_kind",
    )
    .await?
    {
        let part = podcast_part(&kind);
        add(part, followed_item(part), count);
    }
    for (kind, count) in grouped(
        "SELECT p.remote_kind, COUNT(*) FROM podcast_episode e JOIN podcast p ON p.id = e.podcast_id
         WHERE e.deleted = 0 AND p.deleted = 0 AND p.user_subscribed_at IS NOT NULL
         GROUP BY p.remote_kind",
    )
    .await?
    {
        add(podcast_part(&kind), Item::Episodes, count);
    }
    let single = |sql: &'static str| async move {
        Ok::<_, DbError>(
            sqlx::query(sql)
                .fetch_one(db.pool())
                .await?
                .get::<i64, _>(0),
        )
    };
    let rows: [(&'static str, Part, Item); 11] = [
        (
            "SELECT COUNT(*) FROM website WHERE deleted = 0",
            Part::Websites,
            Item::Sites,
        ),
        (
            "SELECT COUNT(*) FROM music_playlist WHERE deleted = 0",
            Part::Playlists,
            Item::Playlists,
        ),
        (
            "SELECT COUNT(*) FROM music_playlist_item i JOIN music_playlist p ON p.id = i.playlist_id
             WHERE i.deleted = 0 AND p.deleted = 0",
            Part::Playlists,
            Item::PlaylistSongs,
        ),
        (
            "SELECT COUNT(*) FROM music_favorite WHERE deleted = 0",
            Part::Playlists,
            Item::Favorites,
        ),
        (
            "SELECT COALESCE(SUM(json_array_length(value, '$.pins')), 0) FROM setting
             WHERE key = 'rail' AND deleted = 0 AND json_valid(value)",
            Part::Pins,
            Item::Pins,
        ),
        (
            "SELECT COUNT(*) FROM setting WHERE deleted = 0 AND key != 'rail'",
            Part::Settings,
            Item::Settings,
        ),
        (
            "SELECT COUNT(*) FROM music_play_history WHERE deleted = 0",
            Part::History,
            Item::Plays,
        ),
        (
            "SELECT COUNT(*) FROM video_progress WHERE deleted = 0",
            Part::History,
            Item::Watched,
        ),
        (
            "SELECT COUNT(*) FROM extension_repo WHERE deleted = 0",
            Part::Sources,
            Item::Repositories,
        ),
        (
            "SELECT COUNT(*) FROM stremio_addon WHERE deleted = 0",
            Part::Sources,
            Item::Addons,
        ),
        (
            "SELECT COUNT(*) FROM local_extension_install",
            Part::Sources,
            Item::Extensions,
        ),
    ];
    for (sql, part, item) in rows {
        add(part, item, single(sql).await?);
    }
    for (table, item) in [
        ("local_music_download", Item::Songs),
        ("local_manga_download", Item::Chapters),
        ("local_podcast_download", Item::Episodes),
        ("local_video_download", Item::Videos),
    ] {
        // Names come from the list above.
        let sql = format!(
            "SELECT COALESCE(SUM(state = 'done'), 0), COALESCE(SUM(state != 'done'), 0)
             FROM {table}"
        );
        let row = sqlx::query(AssertSqlSafe(sql)).fetch_one(db.pool()).await?;
        add(Part::Downloads, item, row.get::<i64, _>(0));
        add(Part::Downloads, Item::Waiting, row.get::<i64, _>(1));
    }
    Ok(counts)
}

/// Every download kept here, by kind, with whether each has its file: what clearing downloads
/// goes through.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Downloads {
    /// `local_music_download.id`.
    pub songs: Vec<(String, bool)>,
    /// `local_manga_download.unit_id`.
    pub chapters: Vec<(String, bool)>,
    /// `local_podcast_download.episode_id`.
    pub episodes: Vec<(String, bool)>,
    /// `local_video_download.id`.
    pub videos: Vec<(String, bool)>,
}

pub async fn downloads(db: &Db) -> Result<Downloads, DbError> {
    let read = |sql: &'static str| async move {
        Ok::<_, DbError>(
            sqlx::query(sql)
                .fetch_all(db.pool())
                .await?
                .iter()
                .map(|row| (row.get::<String, _>(0), row.get::<String, _>(1) == "done"))
                .collect::<Vec<_>>(),
        )
    };
    Ok(Downloads {
        songs: read("SELECT id, state FROM local_music_download ORDER BY id").await?,
        chapters: read("SELECT unit_id, state FROM local_manga_download ORDER BY unit_id").await?,
        episodes: read("SELECT episode_id, state FROM local_podcast_download ORDER BY episode_id")
            .await?,
        videos: read("SELECT id, state FROM local_video_download ORDER BY id").await?,
    })
}

fn title_part(kind: &str) -> Option<Part> {
    Part::ALL
        .into_iter()
        .find(|part| part.title_kind() == Some(kind))
}

fn podcast_part(kind: &str) -> Part {
    if kind == "audiobook" {
        Part::Audiobooks
    } else {
        Part::Podcasts
    }
}

/// Clears a part kept in the database; returns how many rows changed. Library parts leave the
/// library (titles stay, with what was read, as after removing them one by one) and lose their
/// categories; the rest become tombstones. Pins, sources and downloads are the app's to clear:
/// nothing changes here for them.
pub async fn clear(db: &Db, part: Part) -> Result<u64, DbError> {
    let statements: Vec<&'static str> = match part {
        Part::Comics | Part::Books | Part::Videos => vec![
            "UPDATE title SET user_in_library_at = NULL, updated_at_hlc = ?1, origin_device = ?2
             WHERE kind = ?3 AND deleted = 0 AND user_in_library_at IS NOT NULL",
            "UPDATE title_category SET deleted = 1, updated_at_hlc = ?1, origin_device = ?2
             WHERE deleted = 0 AND title_id IN (SELECT id FROM title WHERE kind = ?3)",
            "UPDATE category SET deleted = 1, updated_at_hlc = ?1, origin_device = ?2
             WHERE deleted = 0 AND kind = ?3",
        ],
        Part::Podcasts | Part::Audiobooks => vec![
            "UPDATE podcast SET user_subscribed_at = NULL, updated_at_hlc = ?1, origin_device = ?2
             WHERE remote_kind = ?3 AND deleted = 0 AND user_subscribed_at IS NOT NULL",
        ],
        Part::Websites => vec![
            "UPDATE website SET deleted = 1, updated_at_hlc = ?1, origin_device = ?2
             WHERE deleted = 0",
        ],
        Part::Playlists => vec![
            "UPDATE music_playlist_item SET deleted = 1, updated_at_hlc = ?1, origin_device = ?2
             WHERE deleted = 0",
            "UPDATE music_playlist SET deleted = 1, updated_at_hlc = ?1, origin_device = ?2
             WHERE deleted = 0",
            "UPDATE music_favorite SET deleted = 1, updated_at_hlc = ?1, origin_device = ?2
             WHERE deleted = 0",
        ],
        Part::History => vec![
            "UPDATE music_play_history SET deleted = 1, updated_at_hlc = ?1, origin_device = ?2
             WHERE deleted = 0",
            "UPDATE video_progress SET deleted = 1, updated_at_hlc = ?1, origin_device = ?2
             WHERE deleted = 0",
        ],
        Part::Settings => vec![
            "UPDATE setting SET deleted = 1, updated_at_hlc = ?1, origin_device = ?2
             WHERE deleted = 0 AND key != 'rail'",
        ],
        Part::Pins | Part::Sources | Part::Downloads => return Ok(0),
    };
    let kind = part.title_kind().or(part.podcast_kind());
    let stamp = db.tick();
    let device = db.device_id().to_string();
    let mut transaction = db.pool().begin().await?;
    let mut changed = 0;
    for sql in statements {
        changed += run(&mut transaction, sql, stamp, &device, kind).await?;
    }
    persist_hlc(&mut *transaction, stamp).await?;
    transaction.commit().await?;
    Ok(changed)
}

async fn run(
    transaction: &mut Transaction<'_, Sqlite>,
    sql: &'static str,
    stamp: u64,
    device: &str,
    kind: Option<&str>,
) -> Result<u64, DbError> {
    let mut query = sqlx::query(sql).bind(stamp as i64).bind(device);
    if sql.contains("?3") {
        query = query.bind(kind.unwrap_or_default());
    }
    Ok(query.execute(&mut **transaction).await?.rows_affected())
}

#[cfg(test)]
#[path = "data_tests.rs"]
mod tests;
