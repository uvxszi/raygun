//! The video library (M43): movies and series saved from video sources and Stremio addons live in
//! the title store (kind `video`). Watching progress, kept by progress key, is mirrored onto their
//! episodes so library counts and sorting work as for manga; the feeds list the episodes library
//! updates found and the ones announced for later.

use std::collections::HashMap;

use raygun_db::Db;
use raygun_db::titles::{self, TitleRow, UnitRow, UpdateRow};
use raygun_db::video as progress_store;
use raygun_db::video_downloads;
use raygun_model::VideoUpdateItem;
use raygun_net::ImageLinks;

use crate::catalog::to_message;

/// `title.kind` of video titles.
pub const KIND: &str = "video";
/// `title.source_id` of titles saved from Stremio addons; their `remote_id` is `<type>:<id>`.
pub const STREMIO_SOURCE: &str = "stremio";
/// Most episodes a feed lists.
const FEED_LIMIT: u32 = 300;

/// The source and the source's episode id a progress key names: `source:<sourceId>:<episodeId>`
/// or `stremio:<type>:<videoId>`. Links (`url:...`) belong to no title.
pub fn key_unit(key: &str) -> Option<(&str, &str)> {
    if let Some(rest) = key.strip_prefix("source:") {
        return rest.split_once(':');
    }
    let (_kind, video_id) = key.strip_prefix("stremio:")?.split_once(':')?;
    Some((STREMIO_SOURCE, video_id))
}

/// The progress key of a stored title's episode: the inverse of [`key_unit`].
pub fn unit_key(title: &TitleRow, unit_remote_id: &str) -> String {
    match stremio_id(title) {
        Some((kind, _)) => crate::stremio::content_key(kind, unit_remote_id),
        None => crate::video_sources::content_key(&title.fields.source_id, unit_remote_id),
    }
}

/// A Stremio title's type and id.
pub fn stremio_id(title: &TitleRow) -> Option<(&str, &str)> {
    (title.fields.source_id == STREMIO_SOURCE)
        .then(|| title.fields.remote_id.split_once(':'))
        .flatten()
}

/// Every progress key of a title's episodes starts with this.
fn key_prefix(title: &TitleRow) -> String {
    match stremio_id(title) {
        // A movie's key is its own id; a series' episodes extend it (`tt0903747:1:2`).
        Some((kind, id)) => crate::stremio::content_key(kind, id),
        None => crate::video_sources::content_key(&title.fields.source_id, ""),
    }
}

/// Mirrors a progress write onto the stored episodes the key names.
pub async fn mirror_progress(
    db: &Db,
    key: &str,
    position_ms: i64,
    duration_ms: Option<i64>,
    watched: bool,
    now_ms: i64,
) -> Result<(), String> {
    for unit in key_units(db, key).await? {
        titles::set_watch_progress(db, &unit.id, position_ms, duration_ms, watched, now_ms)
            .await
            .map_err(to_message)?;
    }
    Ok(())
}

/// Mirrors marking something watched or not.
pub async fn mirror_mark(db: &Db, key: &str, watched: bool, now_ms: i64) -> Result<(), String> {
    for unit in key_units(db, key).await? {
        titles::set_completed(db, &unit.id, watched, now_ms)
            .await
            .map_err(to_message)?;
    }
    Ok(())
}

async fn key_units(db: &Db, key: &str) -> Result<Vec<UnitRow>, String> {
    let Some((source_id, remote_id)) = key_unit(key) else {
        return Ok(Vec::new());
    };
    titles::find_units(db, source_id, remote_id)
        .await
        .map_err(to_message)
}

/// Copies progress saved before a title's episodes were stored onto the episodes that have none
/// (a series saved to the library after watching some of it).
pub async fn backfill(db: &Db, title: &TitleRow) -> Result<(), String> {
    let saved: HashMap<String, progress_store::VideoProgress> =
        progress_store::progress_with_prefix(db, &key_prefix(title))
            .await
            .map_err(to_message)?
            .into_iter()
            .map(|row| (row.entry.key.clone(), row))
            .collect();
    if saved.is_empty() {
        return Ok(());
    }
    let units = titles::units(db, &title.id).await.map_err(to_message)?;
    for unit in units
        .iter()
        .filter(|unit| unit.position.is_none() && unit.completed_at.is_none())
    {
        let Some(row) = saved.get(&unit_key(title, &unit.fields.remote_id)) else {
            continue;
        };
        titles::set_watch_progress(
            db,
            &unit.id,
            row.position_ms,
            row.duration_ms,
            row.watched_at.is_some(),
            row.watched_at.unwrap_or(row.updated_at),
        )
        .await
        .map_err(to_message)?;
    }
    Ok(())
}

/// Download states of `keys` that are finished, as a set of keys.
pub async fn downloaded_keys(
    db: &Db,
    keys: &[String],
) -> Result<std::collections::HashSet<String>, String> {
    Ok(video_downloads::states_for_keys(db, keys)
        .await
        .map_err(to_message)?
        .into_iter()
        .filter(|(_, state)| state == video_downloads::DONE)
        .map(|(key, _)| key)
        .collect())
}

/// Episodes library updates found, newest first.
pub async fn updates(db: &Db, images: &ImageLinks) -> Result<Vec<VideoUpdateItem>, String> {
    let rows = titles::updates(db, KIND, FEED_LIMIT)
        .await
        .map_err(to_message)?;
    feed(db, images, rows).await
}

/// Episodes of library series released after `now_ms`, soonest first.
pub async fn upcoming(
    db: &Db,
    images: &ImageLinks,
    now_ms: i64,
) -> Result<Vec<VideoUpdateItem>, String> {
    let rows = titles::upcoming(db, KIND, now_ms, FEED_LIMIT)
        .await
        .map_err(to_message)?;
    feed(db, images, rows).await
}

async fn feed(
    db: &Db,
    images: &ImageLinks,
    rows: Vec<UpdateRow>,
) -> Result<Vec<VideoUpdateItem>, String> {
    let keys: Vec<String> = rows
        .iter()
        .map(|row| unit_key(&row.title, &row.unit.fields.remote_id))
        .collect();
    let downloaded = downloaded_keys(db, &keys).await?;
    Ok(rows
        .into_iter()
        .zip(keys)
        .map(|(row, key)| VideoUpdateItem {
            cover_url: row
                .title
                .fields
                .cover
                .as_deref()
                .map(|url| images.kept_cover(url, &row.title.fields.cover_headers)),
            season: crate::video_sources::season(&row.unit),
            title_id: row.title.id,
            source_id: row.title.fields.source_id,
            remote_id: row.title.fields.remote_id,
            title: row.title.fields.title,
            episode_id: row.unit.id,
            episode_remote_id: row.unit.fields.remote_id,
            name: row.unit.fields.name,
            number: row.unit.fields.number,
            date: row.unit.fields.date.map(|ms| ms as f64),
            found_at: row.found_at as f64,
            watched: row.unit.completed_at.is_some(),
            downloaded: downloaded.contains(&key),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_keys_name_their_episodes() {
        assert_eq!(
            key_unit("source:dev.raygun.ia/films:ep:1"),
            Some(("dev.raygun.ia/films", "ep:1"))
        );
        assert_eq!(
            key_unit("stremio:series:tt0903747:1:2"),
            Some(("stremio", "tt0903747:1:2"))
        );
        assert_eq!(
            key_unit("stremio:movie:tt0111161"),
            Some(("stremio", "tt0111161"))
        );
        assert_eq!(key_unit("url:https://cdn.example/a.mp4"), None);
    }
}
