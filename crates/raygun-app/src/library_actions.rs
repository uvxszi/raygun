//! Library depth (M46): what the manga, novel and video libraries do to many titles at once
//! (Mihon's selection mode), the user's lists, favorites and pins (CloudStream's watch status,
//! Shosetsu's pins), where a title continues, duplicates across sources, and a category's own
//! display.

use std::collections::HashSet;
use std::sync::Arc;

use futures_util::future::join_all;
use raygun_db::titles::{self, ProgressCopy, TitleMark, TitleRow, UnitRow};
use raygun_model::{
    BulkFailure, BulkResult, CategoryDisplay, LibraryDuplicate, LibraryList, RemoteTitleRef,
    TitleMarks,
};
use raygun_resolver::video_match;
use tauri::{AppHandle, Manager};

use crate::catalog::{
    Catalog, LOCAL_SOURCE_PREFIX, continue_from, now_ms, reading_order, to_message,
};
use crate::manga_downloads::MangaDownloadService;
use crate::stremio::StremioService;
use crate::video_downloads::VideoDownloadService;
use crate::video_library::STREMIO_SOURCE;
use crate::video_sources::VideoSourceService;

/// Titles of a selection fetched at once: sources are slow and many, but one search's picks
/// shouldn't hit one source with dozens of requests together.
const BULK_CONCURRENCY: usize = 4;
/// Why a Stremio title isn't downloaded in bulk: its streams come from addons, picked per video.
const STREMIO_DOWNLOADS: &str = "Videos from addons are downloaded from their title page";

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
        queued.extend(
            units_to_queue(&units, |unit| saved.contains_key(&unit.id), amount, now)
                .into_iter()
                .map(|unit| unit.id.clone()),
        );
    }
    if !queued.is_empty() {
        downloads.enqueue(&queued).await?;
    }
    Ok(queued.len() as u32)
}

/// The chapters or episodes a download of the next `amount` (all when `None`) takes, in reading
/// order: not read or watched, released, and not saved already.
pub fn units_to_queue(
    units: &[UnitRow],
    saved: impl Fn(&UnitRow) -> bool,
    amount: Option<u32>,
    now: i64,
) -> Vec<&UnitRow> {
    let take = amount.map_or(usize::MAX, |amount| amount as usize);
    reading_order(units)
        .into_iter()
        .filter(|unit| {
            unit.completed_at.is_none()
                && unit.fields.date.is_none_or(|date| date <= now)
                && !saved(unit)
        })
        .take(take)
        .collect()
}

/// Splits bulk outcomes into the titles done and the ones that failed.
fn gather(outcomes: Vec<Result<String, BulkFailure>>) -> BulkResult {
    let mut result = BulkResult::default();
    for outcome in outcomes {
        match outcome {
            Ok(id) => result.title_ids.push(id),
            Err(failure) => result.failed.push(failure),
        }
    }
    result
}

/// One picked title added to the library; returns its id.
async fn add_one(
    app: &AppHandle,
    catalog: &Catalog,
    title: &RemoteTitleRef,
) -> Result<String, BulkFailure> {
    let video = catalog.kind() == crate::video_library::KIND;
    let added = async {
        if video && title.source_id == STREMIO_SOURCE {
            let (kind, id) = title
                .remote_id
                .split_once(':')
                .ok_or_else(|| "This title's address is unknown".to_owned())?;
            app.state::<Arc<StremioService>>()
                .set_in_library(kind, id, true)
                .await?;
            return titles::find_title(&catalog.db(), STREMIO_SOURCE, &title.remote_id)
                .await
                .map_err(to_message)?
                .map(|row| row.id)
                .ok_or_else(|| "The title went missing".to_owned());
        }
        let row = catalog
            .title_row(&title.source_id, &title.remote_id, false)
            .await?;
        if row.in_library_at.is_none() {
            if video {
                app.state::<VideoSourceService>()
                    .set_in_library(&row.id, true)
                    .await?;
            } else {
                catalog.set_in_library(&row.id, true).await?;
            }
        }
        Ok::<_, String>(row.id)
    };
    added.await.map_err(|message| BulkFailure {
        title: title.title.clone(),
        message,
    })
}

/// One picked title stored (fetched when it never was), for downloading; returns its id.
async fn fetch_one(catalog: &Catalog, title: &RemoteTitleRef) -> Result<String, BulkFailure> {
    if title.source_id == STREMIO_SOURCE {
        return Err(BulkFailure {
            title: title.title.clone(),
            message: STREMIO_DOWNLOADS.into(),
        });
    }
    catalog
        .title_row(&title.source_id, &title.remote_id, false)
        .await
        .map(|row| row.id)
        .map_err(|message| BulkFailure {
            title: title.title.clone(),
            message,
        })
}

/// Adds titles picked from search or a source to the library (fetching the ones never opened)
/// and puts them into `category_ids`. Titles that fail are reported; the others are added.
pub async fn add_titles(
    app: &AppHandle,
    catalog: &Catalog,
    picked: &[RemoteTitleRef],
    category_ids: &[String],
) -> BulkResult {
    let db = catalog.db();
    let mut outcomes = Vec::with_capacity(picked.len());
    for chunk in picked.chunks(BULK_CONCURRENCY) {
        outcomes.extend(join_all(chunk.iter().map(|title| add_one(app, catalog, title))).await);
    }
    let mut result = gather(outcomes);
    if !category_ids.is_empty()
        && !result.title_ids.is_empty()
        && let Err(error) =
            raygun_db::categories::change_titles(&db, &result.title_ids, category_ids, &[]).await
    {
        result.failed.push(BulkFailure {
            title: String::new(),
            message: to_message(error),
        });
    }
    result
}

/// Downloads the next `amount` chapters or episodes (all unread or unwatched ones when `None`)
/// of titles picked from search or a source, fetching the ones never opened. They join the
/// normal download queues; nothing is added to the library.
pub async fn download_titles(
    app: &AppHandle,
    catalog: &Catalog,
    picked: &[RemoteTitleRef],
    amount: Option<u32>,
) -> BulkResult {
    let video = catalog.kind() == crate::video_library::KIND;
    let mut outcomes = Vec::with_capacity(picked.len());
    for chunk in picked.chunks(BULK_CONCURRENCY) {
        outcomes.extend(join_all(chunk.iter().map(|title| fetch_one(catalog, title))).await);
    }
    let mut result = gather(outcomes);
    if video {
        let (queued, failed) = download_episodes(app, catalog, &result.title_ids, amount).await;
        result.queued = queued;
        result.failed.extend(failed);
    } else {
        match download_next(app, catalog, &result.title_ids, amount).await {
            Ok(queued) => result.queued = queued,
            Err(message) => result.failed.push(BulkFailure {
                title: String::new(),
                message,
            }),
        }
    }
    result
}

/// Queues the next `amount` unwatched episodes of video titles that are out (all when `None`), in
/// watching order, without links: each finds its best link when its turn comes (as a playback
/// session ranks them), so nothing waits on sources here. Episodes already queued or saved are
/// skipped; each title that fails says why once. Local Files videos are on the device already.
pub async fn download_episodes(
    app: &AppHandle,
    catalog: &Catalog,
    ids: &[String],
    amount: Option<u32>,
) -> (u32, Vec<BulkFailure>) {
    let db = catalog.db();
    let downloads = app.state::<Arc<VideoDownloadService>>();
    let now = now_ms();
    let mut queued = 0;
    let mut failed = Vec::new();
    for id in ids {
        let Ok(Some(row)) = titles::title(&db, id).await else {
            continue;
        };
        if row.fields.source_id.starts_with(LOCAL_SOURCE_PREFIX) {
            continue;
        }
        let units = titles::units(&db, id).await.unwrap_or_default();
        let keys: Vec<String> = units
            .iter()
            .map(|unit| crate::video_library::unit_key(&row, &unit.fields.remote_id))
            .collect();
        let states = raygun_db::video_downloads::states_for_keys(&db, &keys)
            .await
            .unwrap_or_default();
        let next: Vec<String> = episodes_to_queue(
            &units,
            |unit| {
                let key = crate::video_library::unit_key(&row, &unit.fields.remote_id);
                states
                    .get(&key)
                    .is_some_and(|state| state != raygun_db::video_downloads::FAILED)
            },
            amount,
            now,
        )
        .into_iter()
        .map(|unit| unit.fields.remote_id.clone())
        .collect();
        if next.is_empty() {
            continue;
        }
        // One title that can't be read doesn't stop the others.
        match downloads.enqueue_episodes(&video_group(&row), &next).await {
            Ok(count) => queued += count,
            Err(message) => failed.push(BulkFailure {
                title: row.fields.title.clone(),
                message,
            }),
        }
    }
    (queued, failed)
}

/// [`units_to_queue`] for episodes: in watching order (seasons, then numbers).
pub fn episodes_to_queue(
    units: &[UnitRow],
    saved: impl Fn(&UnitRow) -> bool,
    amount: Option<u32>,
    now: i64,
) -> Vec<&UnitRow> {
    let take = amount.map_or(usize::MAX, |amount| amount as usize);
    crate::video_sources::watching_order(units)
        .into_iter()
        .filter(|unit| {
            unit.completed_at.is_none()
                && unit.fields.date.is_none_or(|date| date <= now)
                && !saved(unit)
        })
        .take(take)
        .collect()
}

/// A stored video title's player group: `stremio:<type>:<id>` or `title:<id>`.
fn video_group(row: &TitleRow) -> String {
    match crate::video_library::stremio_id(row) {
        Some((kind, id)) => format!("stremio:{kind}:{id}"),
        None => format!("title:{}", row.id),
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
            authors: entry.title.fields.authors,
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

/// Whether a unit was started or finished.
fn has_progress(unit: &UnitRow) -> bool {
    unit.completed_at.is_some() || unit.position.is_some_and(|position| position > 0)
}

/// What a migration carries onto the new title's units (Mihon's migrate): a unit with the same
/// number (and season, for videos) takes its progress; one numbered at or below the furthest
/// finished one of its season counts as read; unnumbered units match by name. Units with
/// progress of their own keep it. Each copy comes with the unit it was taken from, if one.
pub fn migrated_progress<'a>(
    from: &'a [UnitRow],
    to: &[UnitRow],
    video: bool,
    now: i64,
) -> Vec<(ProgressCopy, Option<&'a UnitRow>)> {
    let season_of = |unit: &UnitRow| {
        if video {
            crate::video_sources::season(unit)
        } else {
            None
        }
    };
    let same = |a: Option<f64>, b: Option<f64>| match (a, b) {
        (Some(a), Some(b)) => (a - b).abs() < 1e-6,
        (None, None) => true,
        _ => false,
    };
    let started: Vec<&UnitRow> = from.iter().filter(|unit| has_progress(unit)).collect();
    to.iter()
        .filter(|unit| !has_progress(unit))
        .filter_map(|unit| {
            let season = season_of(unit);
            let matched = started.iter().find(|old| {
                same(season_of(old), season)
                    && match (old.fields.number, unit.fields.number) {
                        (Some(_), Some(_)) => same(old.fields.number, unit.fields.number),
                        (None, None) => {
                            duplicate_key(&old.fields.name) == duplicate_key(&unit.fields.name)
                        }
                        _ => false,
                    }
            });
            if let Some(old) = matched {
                return Some((
                    ProgressCopy {
                        unit_id: unit.id.clone(),
                        position: old.position.unwrap_or(0),
                        total: old.total,
                        completed_at: old.completed_at,
                    },
                    Some(*old),
                ));
            }
            let number = unit.fields.number.filter(|number| *number > 0.0)?;
            let furthest = started
                .iter()
                .filter(|old| old.completed_at.is_some() && same(season_of(old), season))
                .filter_map(|old| old.fields.number)
                .fold(f64::NEG_INFINITY, f64::max);
            (number <= furthest).then(|| {
                (
                    ProgressCopy {
                        unit_id: unit.id.clone(),
                        position: 0,
                        total: None,
                        completed_at: Some(now),
                    },
                    None,
                )
            })
        })
        .collect()
}

/// Moves a library title onto another stored title (Mihon's migrate): progress by chapter or
/// episode number, list, favorite, pin, categories and reader mode go over, the new title joins
/// the library (keeping the date added) and the old one leaves it. Downloads stay.
pub async fn migrate_rows(
    db: &raygun_db::Db,
    kind: &str,
    from_id: &str,
    to_id: &str,
    now: i64,
) -> Result<(), String> {
    if from_id == to_id {
        return Ok(());
    }
    let from = titles::title(db, from_id)
        .await
        .map_err(to_message)?
        .ok_or_else(|| "The title to move is gone from the library".to_owned())?;
    let to = titles::title(db, to_id)
        .await
        .map_err(to_message)?
        .ok_or_else(|| "The other source's title couldn't be stored".to_owned())?;
    let video = kind == crate::video_library::KIND;
    let from_units = titles::units(db, from_id).await.map_err(to_message)?;
    let to_units = titles::units(db, to_id).await.map_err(to_message)?;
    let carried = migrated_progress(&from_units, &to_units, video, now);
    let copies: Vec<ProgressCopy> = carried.iter().map(|(copy, _)| copy.clone()).collect();
    titles::merge_progress(db, &copies)
        .await
        .map_err(to_message)?;
    if video {
        move_watch_positions(db, &from, &to, &to_units, &carried).await?;
    }
    let target = [to_id.to_owned()];
    for mark in [
        from.list.clone().map(|list| TitleMark::List(Some(list))),
        from.favorite_at.map(|at| TitleMark::Favorite(Some(at))),
        from.pinned_at.map(|at| TitleMark::Pinned(Some(at))),
    ]
    .into_iter()
    .flatten()
    {
        titles::set_mark(db, &target, &mark)
            .await
            .map_err(to_message)?;
    }
    if to.in_library_at.is_none() {
        titles::set_in_library(db, to_id, Some(from.in_library_at.unwrap_or(now)))
            .await
            .map_err(to_message)?;
    }
    let memberships = raygun_db::categories::memberships(db, kind)
        .await
        .map_err(to_message)?;
    if let Some(old) = memberships.get(from_id) {
        let mut ids = memberships.get(to_id).cloned().unwrap_or_default();
        ids.extend(
            old.iter()
                .filter(|id| !ids.contains(id))
                .cloned()
                .collect::<Vec<_>>(),
        );
        raygun_db::categories::set_title_categories(db, to_id, &ids)
            .await
            .map_err(to_message)?;
    }
    if !from.reader_mode.is_empty() && to.reader_mode.is_empty() {
        titles::set_reader_mode(db, to_id, &from.reader_mode)
            .await
            .map_err(to_message)?;
    }
    let source = [from_id.to_owned()];
    for mark in [
        TitleMark::List(None),
        TitleMark::Pinned(None),
        TitleMark::Favorite(None),
    ] {
        titles::set_mark(db, &source, &mark)
            .await
            .map_err(to_message)?;
    }
    titles::set_in_library(db, from_id, None)
        .await
        .map_err(to_message)
}

/// Moves partly watched episodes' saved positions (the video progress store, which Continue
/// watching and the title page read) onto the new title's episodes, as session references so
/// resuming opens the new source. The old entries go, keeping their time, so Continue watching
/// shows the new one where the old one was.
async fn move_watch_positions(
    db: &raygun_db::Db,
    from: &TitleRow,
    to: &TitleRow,
    to_units: &[UnitRow],
    carried: &[(ProgressCopy, Option<&UnitRow>)],
) -> Result<(), String> {
    use raygun_db::video as progress_store;
    for (copy, old) in carried {
        let Some(old) = old.filter(|_| copy.completed_at.is_none()) else {
            continue;
        };
        let old_key = crate::video_library::unit_key(from, &old.fields.remote_id);
        let Some(saved) = progress_store::progress(db, &old_key)
            .await
            .map_err(to_message)?
            .filter(|saved| saved.watched_at.is_none() && saved.position_ms > 0)
        else {
            continue;
        };
        let Some(unit) = to_units.iter().find(|unit| unit.id == copy.unit_id) else {
            continue;
        };
        let request = serde_json::to_string(&crate::video_session::EpisodeRef {
            origin: crate::video_session::Origin::Source {
                title_id: to.id.clone(),
            },
            episode: unit.fields.remote_id.clone(),
        })
        .map_err(to_message)?;
        let entry = progress_store::VideoEntry {
            key: crate::video_library::unit_key(to, &unit.fields.remote_id),
            title: to.fields.title.clone(),
            subtitle: saved.entry.subtitle.clone(),
            poster: to.fields.cover.clone(),
            request,
        };
        progress_store::started(db, &entry, saved.updated_at)
            .await
            .map_err(to_message)?;
        progress_store::set_progress(
            db,
            &entry.key,
            saved.position_ms,
            saved.duration_ms,
            false,
            saved.updated_at,
        )
        .await
        .map_err(to_message)?;
        progress_store::remove(db, &old_key)
            .await
            .map_err(to_message)?;
    }
    Ok(())
}

/// "Use this source instead": stores the other source's title (fetching it when new), moves the
/// library entry onto it with [`migrate_rows`] and swaps its quick access pin. Returns the new
/// title's id.
pub async fn migrate(
    app: &AppHandle,
    catalog: &Catalog,
    from_id: &str,
    to_source_id: &str,
    to_remote_id: &str,
) -> Result<String, String> {
    let db = catalog.db();
    let old = titles::title(&db, from_id).await.map_err(to_message)?;
    let to = catalog.title_row(to_source_id, to_remote_id, false).await?;
    if catalog.kind() == crate::video_library::KIND {
        crate::video_library::backfill(&db, &to).await?;
    }
    migrate_rows(&db, catalog.kind(), from_id, &to.id, now_ms()).await?;
    if let Some(old) = old.filter(|old| old.id != to.id) {
        let pin = crate::quick_access::title_pin(&old);
        crate::quick_access::remove(app, &pin.to, &pin.search).await;
    }
    if let Ok(Some(row)) = titles::title(&db, &to.id).await {
        crate::quick_access::add(app, crate::quick_access::title_pin(&row)).await;
    }
    Ok(to.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use raygun_db::titles::{TitleFields, UnitFields};
    use tempfile::TempDir;

    fn fields(source: &str, remote: &str) -> TitleFields {
        TitleFields {
            kind: "manga".into(),
            source_id: source.into(),
            remote_id: remote.into(),
            title: "Frieren".into(),
            cover: None,
            cover_headers: Default::default(),
            authors: vec![],
            artists: vec![],
            description: None,
            genres: vec![],
            status: "ongoing".into(),
            url: None,
            media_type: None,
            year: None,
        }
    }

    fn chapter(remote: &str, number: Option<f64>) -> UnitFields {
        UnitFields {
            remote_id: remote.into(),
            name: number.map_or_else(|| "Extra".to_owned(), |number| format!("Ch. {number}")),
            number,
            volume: None,
            scanlator: None,
            date: None,
        }
    }

    #[tokio::test]
    async fn migrating_keeps_progress_marks_and_categories() {
        let temp = TempDir::new().unwrap();
        let db = raygun_db::Db::open(&temp.path().join("raygun.db"))
            .await
            .unwrap();
        let old = titles::upsert_title(&db, &fields("a", "1"), 1)
            .await
            .unwrap();
        titles::sync_units(
            &db,
            &old,
            &[
                chapter("a1", Some(1.0)),
                chapter("a2", Some(2.0)),
                chapter("a3", Some(3.0)),
                chapter("ax", None),
            ],
            "manga",
            1,
        )
        .await
        .unwrap();
        let new = titles::upsert_title(&db, &fields("b", "9"), 1)
            .await
            .unwrap();
        titles::sync_units(
            &db,
            &new,
            &[
                chapter("b1", Some(1.0)),
                chapter("b1.5", Some(1.5)),
                chapter("b2", Some(2.0)),
                chapter("b3", Some(3.0)),
                chapter("b4", Some(4.0)),
                chapter("bx", None),
            ],
            "manga",
            1,
        )
        .await
        .unwrap();
        let old_units = titles::units(&db, &old).await.unwrap();
        let id_of = |units: &[UnitRow], remote: &str| {
            units
                .iter()
                .find(|unit| unit.fields.remote_id == remote)
                .unwrap()
                .id
                .clone()
        };
        titles::set_units_completed(
            &db,
            &[id_of(&old_units, "a1"), id_of(&old_units, "a2")],
            true,
            50,
        )
        .await
        .unwrap();
        titles::set_progress(&db, &id_of(&old_units, "a3"), 7, Some(20), 60)
            .await
            .unwrap();
        titles::set_units_completed(&db, &[id_of(&old_units, "ax")], true, 55)
            .await
            .unwrap();
        titles::set_in_library(&db, &old, Some(10)).await.unwrap();
        titles::set_mark(
            &db,
            std::slice::from_ref(&old),
            &TitleMark::Favorite(Some(20)),
        )
        .await
        .unwrap();
        let category = raygun_db::categories::create(&db, "manga", "Weekly")
            .await
            .unwrap();
        raygun_db::categories::set_title_categories(&db, &old, std::slice::from_ref(&category))
            .await
            .unwrap();

        migrate_rows(&db, "manga", &old, &new, 100).await.unwrap();

        let units = titles::units(&db, &new).await.unwrap();
        let unit = |remote: &str| {
            units
                .iter()
                .find(|unit| unit.fields.remote_id == remote)
                .unwrap()
        };
        assert_eq!(
            unit("b1").completed_at,
            Some(50),
            "same number: same progress"
        );
        assert_eq!(unit("b2").completed_at, Some(50));
        assert_eq!(
            unit("b1.5").completed_at,
            Some(100),
            "below the furthest finished chapter counts as read"
        );
        assert_eq!(unit("b3").completed_at, None);
        assert_eq!(
            unit("b3").position,
            Some(7),
            "a partly read chapter keeps its page"
        );
        assert_eq!(unit("b3").total, Some(20));
        assert_eq!(unit("b4").completed_at, None);
        assert_eq!(
            unit("bx").completed_at,
            Some(55),
            "unnumbered ones match by name"
        );

        let moved = titles::title(&db, &new).await.unwrap().unwrap();
        assert_eq!(moved.in_library_at, Some(10), "keeps the date added");
        assert_eq!(moved.favorite_at, Some(20));
        let memberships = raygun_db::categories::memberships(&db, "manga")
            .await
            .unwrap();
        assert_eq!(memberships.get(&new), Some(&vec![category]));
        let left = titles::title(&db, &old).await.unwrap().unwrap();
        assert_eq!(left.in_library_at, None, "the old title leaves the library");
        assert_eq!(left.favorite_at, None);
    }

    #[tokio::test]
    async fn migrating_a_video_moves_a_partly_watched_position() {
        use raygun_db::video as progress_store;
        let temp = TempDir::new().unwrap();
        let db = raygun_db::Db::open(&temp.path().join("raygun.db"))
            .await
            .unwrap();
        let video = |source: &str, remote: &str| TitleFields {
            kind: "video".into(),
            ..fields(source, remote)
        };
        let episode = |remote: &str, number: f64| UnitFields {
            volume: Some("1".into()),
            ..chapter(remote, Some(number))
        };
        let old = titles::upsert_title(&db, &video("a", "1"), 1)
            .await
            .unwrap();
        titles::sync_units(
            &db,
            &old,
            &[episode("a1", 1.0), episode("a2", 2.0)],
            "video",
            1,
        )
        .await
        .unwrap();
        let new = titles::upsert_title(&db, &video("b", "9"), 1)
            .await
            .unwrap();
        titles::sync_units(
            &db,
            &new,
            &[episode("b1", 1.0), episode("b2", 2.0)],
            "video",
            1,
        )
        .await
        .unwrap();
        let old_units = titles::units(&db, &old).await.unwrap();
        let second = old_units
            .iter()
            .find(|unit| unit.fields.remote_id == "a2")
            .unwrap();
        titles::set_watch_progress(&db, &second.id, 600_000, Some(1_400_000), false, 70)
            .await
            .unwrap();
        let old_key = crate::video_sources::content_key("a", "a2");
        let entry = progress_store::VideoEntry {
            key: old_key.clone(),
            title: "Frieren".into(),
            subtitle: Some("E2".into()),
            poster: None,
            request: "{}".into(),
        };
        progress_store::started(&db, &entry, 70).await.unwrap();
        progress_store::set_progress(&db, &old_key, 600_000, Some(1_400_000), false, 70)
            .await
            .unwrap();
        titles::set_in_library(&db, &old, Some(10)).await.unwrap();

        migrate_rows(&db, "video", &old, &new, 100).await.unwrap();

        let moved = progress_store::progress(&db, &crate::video_sources::content_key("b", "b2"))
            .await
            .unwrap()
            .expect("the position follows to the new source");
        assert_eq!(moved.position_ms, 600_000);
        assert_eq!(moved.duration_ms, Some(1_400_000));
        assert_eq!(moved.updated_at, 70, "keeps its place in Continue watching");
        let resume: crate::video_session::EpisodeRef =
            serde_json::from_str(&moved.entry.request).unwrap();
        assert_eq!(resume.episode, "b2");
        assert_eq!(
            resume.origin,
            crate::video_session::Origin::Source {
                title_id: new.clone()
            }
        );
        assert!(
            progress_store::progress(&db, &old_key)
                .await
                .unwrap()
                .is_none(),
            "the old entry leaves Continue watching"
        );
    }

    #[test]
    fn video_progress_moves_within_its_season() {
        let unit = |id: &str, season: Option<&str>, number: f64, done: bool| UnitRow {
            id: id.into(),
            title_id: "t".into(),
            fields: UnitFields {
                remote_id: id.into(),
                name: format!("Episode {number}"),
                number: Some(number),
                volume: season.map(str::to_owned),
                scanlator: None,
                date: None,
            },
            source_order: 0,
            position: None,
            total: None,
            completed_at: done.then_some(5),
            progress_at: done.then_some(5),
        };
        let from = [
            unit("o1", Some("1"), 1.0, true),
            unit("o2", Some("1"), 2.0, true),
        ];
        let to = [
            unit("n1", Some("1"), 1.0, false),
            unit("n2", Some("2"), 1.0, false),
            unit("n3", Some("1"), 3.0, false),
        ];
        let copies = migrated_progress(&from, &to, true, 9);
        assert_eq!(
            copies
                .iter()
                .map(|(copy, _)| copy.unit_id.as_str())
                .collect::<Vec<_>>(),
            ["n1"],
            "another season's episode 1 isn't this one"
        );
    }

    fn unit(id: &str, number: f64, completed: bool, date: Option<i64>) -> UnitRow {
        UnitRow {
            id: id.into(),
            title_id: "t".into(),
            fields: raygun_db::titles::UnitFields {
                remote_id: format!("r{id}"),
                name: id.into(),
                number: Some(number),
                volume: None,
                scanlator: None,
                date,
            },
            source_order: 0,
            position: None,
            total: None,
            completed_at: completed.then_some(1),
            progress_at: None,
        }
    }

    #[test]
    fn bulk_downloads_take_the_next_unread_released_unsaved_units_in_order() {
        // Listed newest first, as sources do.
        let units = vec![
            unit("e5", 5.0, false, Some(2_000)),
            unit("e4", 4.0, false, None),
            unit("e3", 3.0, false, None),
            unit("e2", 2.0, false, None),
            unit("e1", 1.0, true, None),
        ];
        let ids = |picked: Vec<&UnitRow>| picked.iter().map(|u| u.id.clone()).collect::<Vec<_>>();
        let saved = |unit: &UnitRow| unit.id == "e3";
        assert_eq!(
            ids(units_to_queue(&units, saved, Some(2), 1_000)),
            ["e2", "e4"]
        );
        assert_eq!(
            ids(units_to_queue(&units, saved, None, 1_000)),
            ["e2", "e4"],
            "read, saved and unreleased ones stay out"
        );
        assert_eq!(
            ids(units_to_queue(&units, |_| false, None, 3_000)),
            ["e2", "e3", "e4", "e5"]
        );
        assert!(units_to_queue(&units, |_| false, Some(0), 3_000).is_empty());
    }

    #[test]
    fn bulk_episode_downloads_take_the_next_unwatched_ones_in_watching_order() {
        // Listed newest first, as sources do.
        let units = vec![
            unit("e4", 4.0, false, Some(2_000)),
            unit("e3", 3.0, false, None),
            unit("e2", 2.0, false, None),
            unit("e1", 1.0, true, None),
        ];
        let ids = |picked: Vec<&UnitRow>| picked.iter().map(|u| u.id.clone()).collect::<Vec<_>>();
        assert_eq!(
            ids(episodes_to_queue(&units, |_| false, Some(1), 1_000)),
            ["e2"]
        );
        assert_eq!(
            ids(episodes_to_queue(
                &units,
                |unit| unit.id == "e2",
                None,
                3_000
            )),
            ["e3", "e4"],
            "watched and queued ones stay out; released ones join"
        );
    }

    #[test]
    fn bulk_outcomes_split_into_done_and_failed() {
        let result = gather(vec![
            Ok("a".into()),
            Err(BulkFailure {
                title: "B".into(),
                message: "The source didn't answer in time".into(),
            }),
            Ok("c".into()),
        ]);
        assert_eq!(result.title_ids, ["a", "c"]);
        assert_eq!(result.failed.len(), 1);
        assert_eq!(result.queued, 0);
    }

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
