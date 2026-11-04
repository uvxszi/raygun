//! Library depth (M46): what the manga, novel and video libraries do to many titles at once
//! (Mihon's selection mode), the user's lists, favorites and pins (CloudStream's watch status,
//! Shosetsu's pins), where a title continues, duplicates across sources, and a category's own
//! display.

use std::collections::HashSet;
use std::sync::Arc;

use raygun_db::titles::{self, TitleMark};
use raygun_model::{CategoryDisplay, LibraryDuplicate, LibraryList, TitleMarks};
use raygun_resolver::video_match;
use tauri::{AppHandle, Manager};

use crate::catalog::{
    Catalog, LOCAL_SOURCE_PREFIX, continue_from, now_ms, reading_order, to_message,
};
use crate::manga_downloads::MangaDownloadService;
use crate::video_downloads::VideoDownloadService;

/// A title's list, favorite and pin.
pub async fn marks(catalog: &Catalog, title_id: &str) -> Result<TitleMarks, String> {
    let row = titles::title(&catalog.db(), title_id)
        .await
        .map_err(to_message)?;
    Ok(row.map_or_else(TitleMarks::default, |row| TitleMarks {
        in_library: row.in_library_at.is_some(),
        list: row.list.as_deref().and_then(LibraryList::from_key),
        favorite: row.favorite_at.is_some(),
        pinned: row.pinned_at.is_some(),
    }))
}

/// What can be set on many titles at once.
pub enum Mark {
    List(Option<LibraryList>),
    Favorite(bool),
    Pinned(bool),
}

/// Sets a list, favorite or pin. Putting a title on a list or among favorites adds it to the
/// library (CloudStream's bookmarks are its library).
pub async fn set_mark(catalog: &Catalog, ids: &[String], mark: Mark) -> Result<(), String> {
    let db = catalog.db();
    let now = now_ms();
    let (stored, adds) = match mark {
        Mark::List(list) => (
            TitleMark::List(list.map(|list| list.key().to_owned())),
            list.is_some(),
        ),
        Mark::Favorite(on) => (TitleMark::Favorite(on.then_some(now)), on),
        Mark::Pinned(on) => (TitleMark::Pinned(on.then_some(now)), false),
    };
    titles::set_mark(&db, ids, &stored)
        .await
        .map_err(to_message)?;
    if adds {
        for id in ids {
            let row = titles::title(&db, id).await.map_err(to_message)?;
            if let Some(row) = row.filter(|row| row.in_library_at.is_none()) {
                titles::set_in_library(&db, id, Some(now))
                    .await
                    .map_err(to_message)?;
                crate::quick_access::add(catalog.app(), crate::quick_access::title_pin(&row)).await;
            }
        }
    }
    Ok(())
}

/// Marks every released chapter or episode of titles read, or all of them unread.
pub async fn set_read(catalog: &Catalog, ids: &[String], read: bool) -> Result<(), String> {
    titles::set_titles_completed(&catalog.db(), ids, read, now_ms())
        .await
        .map_err(to_message)
}

/// Takes titles out of the library (with their list, favorite and pin) and/or deletes what was
/// saved of them (Mihon's Remove dialog: either or both).
pub async fn remove(
    app: &AppHandle,
    catalog: &Catalog,
    ids: &[String],
    from_library: bool,
    delete_downloads: bool,
) -> Result<(), String> {
    let db = catalog.db();
    if from_library {
        for mark in [
            TitleMark::List(None),
            TitleMark::Pinned(None),
            TitleMark::Favorite(None),
        ] {
            titles::set_mark(&db, ids, &mark)
                .await
                .map_err(to_message)?;
        }
        for id in ids {
            if let Ok(Some(row)) = titles::title(&db, id).await {
                let pin = crate::quick_access::title_pin(&row);
                crate::quick_access::remove(app, &pin.to, &pin.search).await;
            }
            titles::set_in_library(&db, id, None)
                .await
                .map_err(to_message)?;
        }
    }
    if !delete_downloads {
        return Ok(());
    }
    if catalog.kind() == crate::video_library::KIND {
        let wanted: HashSet<&String> = ids.iter().collect();
        let downloads = app.state::<Arc<VideoDownloadService>>();
        for row in raygun_db::video_downloads::all(&db)
            .await
            .map_err(to_message)?
        {
            if row
                .download
                .title_id
                .as_ref()
                .is_some_and(|title| wanted.contains(title))
            {
                downloads.remove(&row.id).await?;
            }
        }
    } else {
        let downloads = app.state::<Arc<MangaDownloadService>>();
        for id in ids {
            let units: Vec<String> = titles::units(&db, id)
                .await
                .map_err(to_message)?
                .into_iter()
                .map(|unit| unit.id)
                .collect();
            downloads.remove(&units).await?;
        }
    }
    Ok(())
}

/// Puts titles into `add` categories and out of `remove` ones.
pub async fn change_categories(
    catalog: &Catalog,
    ids: &[String],
    add: &[String],
    remove: &[String],
) -> Result<(), String> {
    raygun_db::categories::change_titles(&catalog.db(), ids, add, remove)
        .await
        .map_err(to_message)
}

/// Queues the next `amount` unread chapters of each title (all unread ones when `None`), in
/// reading order, skipping ones already saved. Imported titles are on the device already.
pub async fn download_next(
    app: &AppHandle,
    catalog: &Catalog,
    ids: &[String],
    amount: Option<u32>,
) -> Result<u32, String> {
    let db = catalog.db();
    let downloads = app.state::<Arc<MangaDownloadService>>();
    let now = now_ms();
    let mut queued = Vec::new();
    for id in ids {
        let Some(row) = titles::title(&db, id).await.map_err(to_message)? else {
            continue;
        };
        if row.fields.source_id.starts_with(LOCAL_SOURCE_PREFIX) {
            continue;
        }
        let units = titles::units(&db, id).await.map_err(to_message)?;
        let saved = downloads.for_title(id).await;
        let unread = reading_order(&units).into_iter().filter(|unit| {
            unit.completed_at.is_none()
                && unit.fields.date.is_none_or(|date| date <= now)
                && !saved.contains_key(&unit.id)
        });
        let take = amount.map_or(usize::MAX, |amount| amount as usize);
        queued.extend(unread.take(take).map(|unit| unit.id.clone()));
    }
    if !queued.is_empty() {
        downloads.enqueue(&queued).await?;
    }
    Ok(queued.len() as u32)
}

/// [`download_next`] for videos: the next `amount` unwatched episodes of each title that are out
/// (all of them when `None`), in watching order, queued to find their links when their turn
/// comes. Local Files videos are on the device already.
pub async fn download_next_episodes(
    app: &AppHandle,
    catalog: &Catalog,
    ids: &[String],
    amount: Option<u32>,
) -> Result<u32, String> {
    let db = catalog.db();
    let downloads = app.state::<Arc<VideoDownloadService>>();
    let now = now_ms();
    let mut queued = 0;
    let mut failure = None;
    for id in ids {
        let Some(row) = titles::title(&db, id).await.map_err(to_message)? else {
            continue;
        };
        if row.fields.source_id == crate::local_files::LOCAL_VIDEO {
            continue;
        }
        let group = match crate::video_library::stremio_id(&row) {
            Some((kind, stremio_id)) => format!("stremio:{kind}:{stremio_id}"),
            None => format!("title:{id}"),
        };
        let units = titles::units(&db, id).await.map_err(to_message)?;
        let keys: Vec<String> = units
            .iter()
            .map(|unit| crate::video_library::unit_key(&row, &unit.fields.remote_id))
            .collect();
        let saved = raygun_db::video_downloads::states_for_keys(&db, &keys)
            .await
            .map_err(to_message)?;
        let take = amount.map_or(usize::MAX, |amount| amount as usize);
        let next: Vec<String> = crate::video_sources::watching_order(&units)
            .into_iter()
            .filter(|unit| {
                let key = crate::video_library::unit_key(&row, &unit.fields.remote_id);
                unit.completed_at.is_none()
                    && unit.fields.date.is_none_or(|date| date <= now)
                    && saved
                        .get(&key)
                        .is_none_or(|state| state == raygun_db::video_downloads::FAILED)
            })
            .take(take)
            .map(|unit| unit.fields.remote_id.clone())
            .collect();
        if next.is_empty() {
            continue;
        }
        // One title that can't be read doesn't stop the others.
        match downloads.enqueue_episodes(&group, &next).await {
            Ok(count) => queued += count,
            Err(error) => failure = Some(format!("{}: {error}", row.fields.title)),
        }
    }
    match failure {
        Some(error) if queued == 0 => Err(error),
        _ => Ok(queued),
    }
}

/// Where a title continues: the chapter or episode its Continue opens.
pub async fn continue_unit(catalog: &Catalog, title_id: &str) -> Result<Option<String>, String> {
    let units = titles::units(&catalog.db(), title_id)
        .await
        .map_err(to_message)?;
    Ok(continue_from(&units).map(|unit| unit.id.clone()))
}

/// A title's name reduced to what decides a duplicate: letters and digits, lowercase.
pub fn duplicate_key(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Whether two names may be the same title: one contains the other (Mihon matches by containment;
/// both ways here, so "Naruto" and "Naruto Shippuden" find each other). Very short names must be
/// equal, or "Up" would match everything.
pub fn may_duplicate(a: &str, b: &str) -> bool {
    let (a, b) = (duplicate_key(a), duplicate_key(b));
    if a.is_empty() || b.is_empty() {
        return false;
    }
    let (short, long) = if a.chars().count() <= b.chars().count() {
        (&a, &b)
    } else {
        (&b, &a)
    };
    if short.chars().count() < 4 {
        return short == long;
    }
    long.contains(short.as_str())
}

/// [`may_duplicate`] for videos: another season of a show isn't the same title ("Overlord" and
/// "Overlord II"), and the same season spelled another way is ("Mushoku Tensei II: …" and
/// "Mushoku Tensei: … 2nd Season").
pub fn may_duplicate_video(a: &str, b: &str) -> bool {
    let (first, second) = (video_match::parse_title(a), video_match::parse_title(b));
    first.same_title(&second) || (may_duplicate(a, b) && first.same_season(&second))
}

/// Library titles whose names may be the same as `name` (Mihon asks before adding one).
pub async fn duplicates(
    catalog: &Catalog,
    title_id: &str,
    name: &str,
) -> Result<Vec<LibraryDuplicate>, String> {
    let names = catalog.source_names().await;
    let video = catalog.kind() == crate::video_library::KIND;
    Ok(titles::library(&catalog.db(), catalog.kind())
        .await
        .map_err(to_message)?
        .into_iter()
        .filter(|entry| {
            let other = &entry.title.fields.title;
            entry.title.id != title_id
                && if video {
                    may_duplicate_video(other, name)
                } else {
                    may_duplicate(other, name)
                }
        })
        .map(|entry| LibraryDuplicate {
            source_name: names
                .get(&entry.title.fields.source_id)
                .cloned()
                .unwrap_or_else(|| entry.title.fields.source_id.clone()),
            cover_url: entry
                .title
                .fields
                .cover
                .as_deref()
                .map(|url| catalog.cover(url, &entry.title.fields.cover_headers)),
            title_id: entry.title.id,
            source_id: entry.title.fields.source_id,
            remote_id: entry.title.fields.remote_id,
            title: entry.title.fields.title,
        })
        .collect())
}

/// Sets or clears a category's own display and sort.
pub async fn set_category_display(
    catalog: &Catalog,
    category_id: &str,
    display: Option<CategoryDisplay>,
) -> Result<(), String> {
    let json = display
        .map(|display| serde_json::to_string(&display))
        .transpose()
        .map_err(to_message)?;
    raygun_db::categories::set_display(&catalog.db(), category_id, json.as_deref())
        .await
        .map_err(to_message)
}

/// The library titles of a category, for updating it alone.
pub async fn category_titles(
    catalog: &Catalog,
    category_id: &str,
) -> Result<HashSet<String>, String> {
    let memberships = raygun_db::categories::memberships(&catalog.db(), catalog.kind())
        .await
        .map_err(to_message)?;
    Ok(memberships
        .into_iter()
        .filter(|(_, categories)| categories.iter().any(|id| id == category_id))
        .map(|(title_id, _)| title_id)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicates_match_on_letters_and_digits_either_way() {
        assert_eq!(
            duplicate_key("Frieren: Beyond Journey's End"),
            "frierenbeyondjourneysend"
        );
        assert!(may_duplicate(
            "FRIEREN - beyond journey’s end",
            "Frieren: Beyond Journey's End"
        ));
        assert!(may_duplicate("Naruto", "Naruto Shippuden"));
        assert!(may_duplicate("Naruto Shippuden", "Naruto"));
        assert!(!may_duplicate("One Piece", "One Punch-Man"));
        assert!(
            !may_duplicate("Up", "Upside Down"),
            "short names must be equal"
        );
        assert!(may_duplicate("Up", "UP!"));
        assert!(!may_duplicate("!!!", "!!!"));
    }

    #[test]
    fn another_season_of_a_video_is_no_duplicate() {
        assert!(!may_duplicate_video("Overlord", "Overlord II"));
        assert!(!may_duplicate_video(
            "Attack on Titan",
            "Attack on Titan Season 2"
        ));
        assert!(may_duplicate_video(
            "Mushoku Tensei II: Isekai Ittara Honki Dasu",
            "Mushoku Tensei: Isekai Ittara Honki Dasu 2nd Season"
        ));
        assert!(may_duplicate_video("Frieren (Dub)", "Frieren"));
        assert!(
            may_duplicate_video("Naruto", "Naruto Shippuden"),
            "names that contain each other still ask"
        );
    }
}
