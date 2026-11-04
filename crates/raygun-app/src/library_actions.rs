//! Library depth (M46): what the manga, novel and video libraries do to many titles at once
//! (Mihon's selection mode), the user's lists, favorites and pins (CloudStream's watch status,
//! Shosetsu's pins), where a title continues, duplicates across sources, and a category's own
//! display.

use std::collections::HashSet;
use std::sync::Arc;

use futures_util::future::join_all;
use raygun_db::titles::{self, TitleMark, TitleRow, UnitRow};
use raygun_model::{
    BulkFailure, BulkResult, CategoryDisplay, LibraryDuplicate, LibraryList, RemoteTitleRef,
    TitleMarks, VideoLinkView, VideoSourcePlay,
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
/// How long one episode may take to find a link it can download.
const EPISODE_LINK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
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

/// Whether the video queue can save a link: a web address of a file or HLS stream (DASH and
/// torrents can't be downloaded).
pub fn downloadable(link: &VideoLinkView) -> bool {
    (link.url.starts_with("https://") || link.url.starts_with("http://"))
        && !matches!(
            link.format.as_deref(),
            Some("dash") | Some(raygun_ext_host::video::TORRENT)
        )
}

/// The link of a server's list to download: the preferred one when it can be, else the first
/// that can.
pub fn download_link(links: Vec<VideoLinkView>) -> Option<VideoLinkView> {
    let usable: Vec<VideoLinkView> = links.into_iter().filter(downloadable).collect();
    let preferred = usable.iter().position(|link| link.preferred).unwrap_or(0);
    usable.into_iter().nth(preferred)
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

/// Queues the next `amount` unwatched episodes of video titles (all when `None`): each episode's
/// servers are asked in order for a link the queue can save. Episodes already queued or saved
/// are skipped; each title that fails says why once.
pub async fn download_episodes(
    app: &AppHandle,
    catalog: &Catalog,
    ids: &[String],
    amount: Option<u32>,
) -> (u32, Vec<BulkFailure>) {
    let db = catalog.db();
    let sources = app.state::<VideoSourceService>();
    let downloads = app.state::<Arc<VideoDownloadService>>();
    let saved: HashSet<String> = raygun_db::video_downloads::all(&db)
        .await
        .map(|rows| rows.into_iter().map(|row| row.download.key).collect())
        .unwrap_or_default();
    let now = now_ms();
    let mut queued = 0;
    let mut failed = Vec::new();
    for id in ids {
        let Ok(Some(row)) = titles::title(&db, id).await else {
            continue;
        };
        if row.fields.source_id == STREMIO_SOURCE {
            failed.push(BulkFailure {
                title: row.fields.title.clone(),
                message: STREMIO_DOWNLOADS.into(),
            });
            continue;
        }
        if row.fields.source_id.starts_with(LOCAL_SOURCE_PREFIX) {
            continue;
        }
        let units = titles::units(&db, id).await.unwrap_or_default();
        let source = row.fields.source_id.as_str();
        let wanted = units_to_queue(
            &units,
            |unit| {
                saved.contains(&crate::video_sources::content_key(
                    source,
                    &unit.fields.remote_id,
                ))
            },
            amount,
            now,
        );
        let mut problem = None;
        for unit in wanted {
            match queue_episode(&sources, &downloads, &row, unit).await {
                Ok(()) => queued += 1,
                Err(message) => {
                    problem.get_or_insert(message);
                }
            }
        }
        if let Some(message) = problem {
            failed.push(BulkFailure {
                title: row.fields.title.clone(),
                message,
            });
        }
    }
    (queued, failed)
}

/// Finds a link of an episode the video queue can save (its servers in order) and queues it.
async fn queue_episode(
    sources: &VideoSourceService,
    downloads: &VideoDownloadService,
    row: &TitleRow,
    unit: &UnitRow,
) -> Result<(), String> {
    let source = row.fields.source_id.as_str();
    let find = async {
        let servers = sources.servers(source, &unit.fields.remote_id).await?;
        for server in servers {
            let links = match server.links {
                Some(links) => links,
                None => match sources.links(source, &server.id).await {
                    Ok(links) => links,
                    Err(_) => continue,
                },
            };
            if let Some(link) = download_link(links) {
                return Ok(link);
            }
        }
        Err::<VideoLinkView, String>("None of its servers has a link that can be downloaded".into())
    };
    let link = tokio::time::timeout(EPISODE_LINK_TIMEOUT, find)
        .await
        .map_err(|_| "The source didn't answer in time".to_owned())??;
    let video = sources
        .request(VideoSourcePlay {
            title_id: row.id.clone(),
            episode_id: unit.id.clone(),
            link,
            external: false,
        })
        .await?;
    downloads.enqueue(video, Some(row.id.clone())).await
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

    fn link(url: &str, format: Option<&str>, preferred: bool) -> VideoLinkView {
        VideoLinkView {
            url: url.into(),
            label: None,
            quality: None,
            headers: Default::default(),
            format: format.map(str::to_owned),
            subtitles: Vec::new(),
            preferred,
            chapters: Vec::new(),
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
    fn episode_downloads_pick_a_link_the_queue_can_save() {
        assert!(downloadable(&link("https://a/v.mp4", None, false)));
        assert!(downloadable(&link("http://a/v.m3u8", Some("hls"), false)));
        assert!(!downloadable(&link("https://a/v.mpd", Some("dash"), true)));
        assert!(!downloadable(&link(
            "magnet:?xt=urn",
            Some("torrent"),
            true
        )));
        assert!(!downloadable(&link("content://local", None, true)));

        let picked = download_link(vec![
            link("https://a/dash.mpd", Some("dash"), true),
            link("https://a/480.mp4", None, false),
            link("https://a/720.mp4", None, false),
        ]);
        assert_eq!(
            picked.map(|link| link.url).as_deref(),
            Some("https://a/480.mp4"),
            "the preferred link can't be saved: the first that can"
        );
        let picked = download_link(vec![
            link("https://a/480.mp4", None, false),
            link("https://a/1080.m3u8", Some("hls"), true),
        ]);
        assert_eq!(
            picked.map(|link| link.url).as_deref(),
            Some("https://a/1080.m3u8")
        );
        assert_eq!(
            download_link(vec![link("magnet:x", Some("torrent"), true)]),
            None
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
