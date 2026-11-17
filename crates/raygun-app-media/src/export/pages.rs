//! A reader page saved to Pictures, shared or copied (M46.1; Mihon's page actions, Apache-2.0:
//! `ReaderViewModel.saveImage`/`shareImage`/`copyImage`). The page is fetched back through the
//! image proxy, which serves the download, the page cache or the source with its headers.

use std::sync::Arc;

use raygun_db::titles;
use raygun_model::PageRef;
use raygun_net::ImageLinks;
use tauri::{AppHandle, Manager};

use super::{ExportService, Place, image_mime, names, write_one};
use crate::catalog::to_message;
use crate::startup::AppDb;

/// What the page action does with the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageAction {
    Save,
    Share,
    Copy,
}

struct Page {
    bytes: Vec<u8>,
    extension: &'static str,
    /// "Frieren - Chapter 3 - 12"
    stem: String,
    title: String,
}

async fn fetch(app: &AppHandle, service: &ExportService, page: &PageRef) -> Result<Page, String> {
    if !app.state::<ImageLinks>().is_page(&page.url) {
        return Err("That isn't one of the reader's pages".into());
    }
    let response = service
        .client()
        .get(&page.url)
        .send()
        .await
        .map_err(to_message)?;
    if !response.status().is_success() {
        return Err(format!(
            "The page couldn't be loaded (HTTP {})",
            response.status().as_u16()
        ));
    }
    let bytes = response.bytes().await.map_err(to_message)?.to_vec();
    let extension = match raygun_net::page_cache::sniff_content_type(&bytes) {
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/avif" => "avif",
        "image/jpeg" => "jpg",
        _ => return Err("The page isn't an image".into()),
    };
    let db = app.state::<AppDb>().inner().0.clone();
    let title = titles::title(&db, &page.title_id)
        .await
        .map_err(to_message)?
        .map(|title| title.fields.title)
        .unwrap_or_default();
    let chapter = titles::unit(&db, &page.chapter_id)
        .await
        .map_err(to_message)?
        .map(|unit| unit.fields.name)
        .unwrap_or_default();
    Ok(Page {
        bytes,
        extension,
        stem: format!("{title} - {chapter} - {}", page.index + 1),
        title,
    })
}

/// Saves, shares or copies a page. Returns where a saved page went.
pub async fn act(app: &AppHandle, page: &PageRef, action: PageAction) -> Result<String, String> {
    let service: Arc<ExportService> = app.state::<Arc<ExportService>>().inner().clone();
    let fetched = fetch(app, &service, page).await?;
    let prefs = service.save_prefs().await;
    let name = names::file_name(&fetched.stem, fetched.extension);
    let mime = image_mime(fetched.extension);
    // Desktop has no share sheet: "Show in folder" saves the page like Save, then shows it (M46.6).
    let save_like = matches!(action, PageAction::Save)
        || (matches!(action, PageAction::Share) && cfg!(not(target_os = "android")));
    let (place, folder) = match action {
        _ if save_like => {
            let folder = if prefs.pages_per_title && !fetched.title.trim().is_empty() {
                vec![names::safe_name(&fetched.title)]
            } else {
                Vec::new()
            };
            (Place::Pictures, folder)
        }
        _ => (Place::Share, Vec::new()),
    };
    let outputs = service.outputs().clone();
    let bytes = fetched.bytes;
    let saved = {
        let outputs = outputs.clone();
        let name = name.clone();
        tauri::async_runtime::spawn_blocking(move || {
            if place == Place::Share {
                outputs.clear_shared();
            }
            write_one(&outputs, &place, &folder, &name, mime, |file| {
                std::io::Write::write_all(file, &bytes).map(|()| 0)
            })
        })
        .await
        .map_err(to_message)??
        .0
    };
    let shared = if matches!(action, PageAction::Save) {
        None
    } else if save_like {
        saved.local.clone()
    } else {
        saved.path.clone()
    };
    if let Some(path) = shared {
        tauri::async_runtime::spawn_blocking(move || match action {
            PageAction::Copy => outputs.copy_image(&path, &name),
            _ => outputs.share(&path, mime, &name),
        })
        .await
        .map_err(to_message)??;
    }
    Ok(saved.shown)
}
