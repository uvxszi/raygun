//! A book searched for a word (the lookup's "Search in book"): the chapters on this device, read
//! through the app's services; the matching is `raygun_app_reading::dictionary::book_search`.

use raygun_app_reading::dictionary::book_search::search_blocks;
use raygun_model::{BookSearch, DownloadState};
use tauri::{AppHandle, Manager};

pub async fn search(
    app: &AppHandle,
    title_id: &str,
    open_chapter: Option<&str>,
    query: &str,
) -> Result<BookSearch, String> {
    let query = query.split_whitespace().collect::<Vec<_>>().join(" ");
    if query.is_empty() {
        return Err("Type something to search for".to_owned());
    }
    let db = app.state::<crate::startup::AppDb>().inner().0.clone();
    let title = raygun_db::titles::title(&db, title_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or("This book was removed")?;
    let mut units = raygun_db::titles::units(&db, title_id)
        .await
        .map_err(|error| error.to_string())?;
    // In reading order: by number where chapters have one, else as the source lists them.
    units.sort_by(|a, b| {
        let number = |unit: &raygun_db::titles::UnitRow| unit.fields.number.unwrap_or(f64::MAX);
        number(a)
            .total_cmp(&number(b))
            .then(a.source_order.cmp(&b.source_order))
    });
    let imported = title.fields.source_id == crate::novel::LOCAL_EPUB;
    let downloaded = if imported {
        Default::default()
    } else {
        app.state::<std::sync::Arc<crate::manga_downloads::MangaDownloadService>>()
            .for_title(title_id)
            .await
    };
    let novels = app.state::<crate::novel::NovelService>();
    let mut hits = Vec::new();
    let mut searched = 0u32;
    let mut more = false;
    for unit in &units {
        let on_device = imported
            || open_chapter == Some(unit.id.as_str())
            || downloaded
                .get(&unit.id)
                .is_some_and(|download| download.state == DownloadState::Done);
        if !on_device || more {
            continue;
        }
        let Ok(chapter) = novels.chapter(title_id, &unit.id).await else {
            continue;
        };
        searched += 1;
        more = search_blocks(
            &unit.id,
            &unit.fields.name,
            &chapter.blocks,
            &query,
            &mut hits,
        );
    }
    Ok(BookSearch {
        hits,
        searched,
        chapters: units.len() as u32,
        more,
    })
}

/// Where words appear in a book: every chapter of an imported book, a source's downloaded
/// chapters and the one open (`chapter_id`).
#[tauri::command]
#[specta::specta]
pub async fn novel_search_book(
    app: AppHandle,
    title_id: String,
    chapter_id: Option<String>,
    query: String,
) -> Result<BookSearch, String> {
    search(&app, &title_id, chapter_id.as_deref(), &query).await
}
