//! Library depth (M46): what the manga, novel and video libraries do to many titles at once
//! (Mihon's selection mode), the user's lists, favorites and pins (CloudStream's watch status,
//! Shosetsu's pins), where a title continues, duplicates across sources, and a category's own
//! display.

use std::collections::HashSet;
use std::sync::Arc;

use raygun_db::titles::{self, ProgressCopy, TitleMark, UnitRow};
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

/// Whether a unit was started or finished.
fn has_progress(unit: &UnitRow) -> bool {
    unit.completed_at.is_some() || unit.position.is_some_and(|position| position > 0)
}

/// What a migration carries onto the new title's units (Mihon's migrate): a unit with the same
/// number (and season, for videos) takes its progress; one numbered at or below the furthest
/// finished one of its season counts as read; unnumbered units match by name. Units with
/// progress of their own keep it.
pub fn migrated_progress(
    from: &[UnitRow],
    to: &[UnitRow],
    video: bool,
    now: i64,
) -> Vec<ProgressCopy> {
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
                return Some(ProgressCopy {
                    unit_id: unit.id.clone(),
                    position: old.position.unwrap_or(0),
                    total: old.total,
                    completed_at: old.completed_at,
                });
            }
            let number = unit.fields.number.filter(|number| *number > 0.0)?;
            let furthest = started
                .iter()
                .filter(|old| old.completed_at.is_some() && same(season_of(old), season))
                .filter_map(|old| old.fields.number)
                .fold(f64::NEG_INFINITY, f64::max);
            (number <= furthest).then(|| ProgressCopy {
                unit_id: unit.id.clone(),
                position: 0,
                total: None,
                completed_at: Some(now),
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
    let copies = migrated_progress(
        &titles::units(db, from_id).await.map_err(to_message)?,
        &titles::units(db, to_id).await.map_err(to_message)?,
        kind == crate::video_library::KIND,
        now,
    );
    titles::merge_progress(db, &copies)
        .await
        .map_err(to_message)?;
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
                .map(|copy| copy.unit_id.as_str())
                .collect::<Vec<_>>(),
            ["n1"],
            "another season's episode 1 isn't this one"
        );
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
