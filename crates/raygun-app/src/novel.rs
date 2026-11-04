//! Novels (M31): `novel.source` catalogues through the shared `Catalog`, and chapters as text
//! blocks for the novel reader. Progress is a character offset into the chapter's text.
//! M33 added EPUB imports (unpacked under `downloads/books/`) and downloaded chapters.

use std::path::PathBuf;
use std::sync::Arc;

use base64::Engine;
use raygun_db::titles::{self, TitleFields, UnitFields};
use raygun_ext_host::novel::{NOVEL_SOURCE, PassageBlock, html_blocks};
use raygun_model::{
    MangaBrowseMode, MangaLibraryEntry, MangaListingPage, MangaSource, MangaTitleView,
    NovelChapter, TextBlock,
};
use raygun_net::ImageLinks;
use tauri::{AppHandle, Manager};

use crate::catalog::{Catalog, LOCAL_PREFIX, chapter_info, now_ms, to_message};
use crate::epub::{self, Epub};
use crate::manga_downloads::MangaDownloadService;

const KIND: &str = "novel";
/// The source id of imported EPUB books.
pub const LOCAL_EPUB: &str = "local.epub";
/// Folder under the downloads root holding unpacked books.
const BOOKS: &str = "books";
/// Host of the placeholder base URL that chapter images resolve against.
const BOOK_HOST: &str = "book.invalid";

pub struct NovelService {
    app: AppHandle,
    catalog: Catalog,
    downloads_dir: PathBuf,
}

impl NovelService {
    /// The shared catalog behind this library (categories, M39).
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    pub fn new(app: AppHandle, images: ImageLinks, downloads_dir: PathBuf) -> Self {
        Self {
            catalog: Catalog::new(app.clone(), images, KIND, NOVEL_SOURCE),
            app,
            downloads_dir,
        }
    }

    pub async fn sources(&self) -> Vec<MangaSource> {
        self.catalog.sources().await
    }

    pub async fn browse(
        &self,
        source_id: &str,
        mode: MangaBrowseMode,
        query: Option<&str>,
        page: u32,
    ) -> Result<MangaListingPage, String> {
        self.catalog.browse(source_id, mode, query, page).await
    }

    pub async fn title(
        &self,
        source_id: &str,
        remote_id: &str,
        refresh: bool,
    ) -> Result<MangaTitleView, String> {
        let row = if source_id == LOCAL_EPUB {
            // An imported book has nothing to refresh from.
            titles::find_title(&self.catalog.db(), source_id, remote_id)
                .await
                .map_err(to_message)?
                .ok_or_else(|| "This book was removed".to_owned())?
        } else {
            self.catalog
                .title_row(source_id, remote_id, refresh)
                .await?
        };
        let downloads = self.downloads().for_title(&row.id).await;
        let mut view = self.catalog.title_view(row, &downloads).await?;
        if view.source_id == LOCAL_EPUB {
            view.source_name = "EPUB file".into();
        }
        Ok(view)
    }

    /// Imports an EPUB (its contents as base64): unpacks it and stores it as a library novel.
    /// The same file imported again keeps its title and progress. Returns the source's id
    /// (`local.epub`) and the book's id, as the title page wants them.
    pub async fn import_epub(&self, contents: &str) -> Result<(String, String), String> {
        if contents.len() > epub::MAX_FILE_BYTES / 3 * 4 + 4 {
            return Err("This book is larger than 64 MB".into());
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(contents.trim())
            .map_err(|_| "The file couldn't be read".to_owned())?;
        self.import_epub_bytes(bytes).await
    }

    /// Imports an uploaded EPUB file (sent in slices, so a big book never crosses IPC whole); the
    /// file is deleted afterwards.
    pub async fn import_epub_file(&self, file: PathBuf) -> Result<(String, String), String> {
        let bytes = tauri::async_runtime::spawn_blocking(move || {
            let result = match std::fs::metadata(&file) {
                Ok(meta) if meta.len() > epub::MAX_FILE_BYTES as u64 => {
                    Err("This book is larger than 64 MB".to_owned())
                }
                Ok(_) => std::fs::read(&file).map_err(|_| "The file couldn't be read".to_owned()),
                Err(_) => Err("The file couldn't be read".to_owned()),
            };
            let _ = std::fs::remove_file(&file);
            result
        })
        .await
        .map_err(to_message)??;
        self.import_epub_bytes(bytes).await
    }

    /// Imports an EPUB's bytes (an upload, or a file in a Local Files folder, M46.27).
    pub async fn import_epub_bytes(&self, bytes: Vec<u8>) -> Result<(String, String), String> {
        if bytes.len() > epub::MAX_FILE_BYTES {
            return Err("This book is larger than 64 MB".into());
        }
        let dir = self.downloads_dir.join(BOOKS);
        let (hash, book) = tauri::async_runtime::spawn_blocking(move || -> Result<_, String> {
            let hash = raygun_ext_host::package::sha256_hex(&bytes);
            let epub = Epub::open(&bytes)?;
            let book = epub.book()?;
            let target = dir.join(&hash);
            let partial = dir.join(format!("{hash}.part"));
            let _ = std::fs::remove_dir_all(&partial);
            epub.unpack(&partial).map_err(to_message)?;
            let _ = std::fs::remove_dir_all(&target);
            std::fs::rename(&partial, &target).map_err(to_message)?;
            Ok((hash, book))
        })
        .await
        .map_err(to_message)??;

        let db = self.catalog.db();
        let now = now_ms();
        let id = titles::upsert_title(
            &db,
            &TitleFields {
                kind: KIND.into(),
                source_id: LOCAL_EPUB.into(),
                remote_id: hash.clone(),
                title: book.title.clone(),
                cover: book.cover.as_deref().map(|path| {
                    format!("{LOCAL_PREFIX}{BOOKS}/{hash}/{}", epub::safe_url_path(path))
                }),
                cover_headers: Default::default(),
                authors: book.authors.clone(),
                artists: Vec::new(),
                description: book.description.clone(),
                genres: book.genres.clone(),
                status: "completed".into(),
                url: None,
                media_type: None,
                year: None,
            },
            now,
        )
        .await
        .map_err(to_message)?;
        let units: Vec<UnitFields> = book
            .chapters
            .iter()
            .enumerate()
            .map(|(index, chapter)| UnitFields {
                remote_id: chapter.path.clone(),
                name: chapter.name.clone(),
                number: Some((index + 1) as f64),
                volume: None,
                scanlator: None,
                date: None,
            })
            .collect();
        titles::sync_units(&db, &id, &units, "chapter", now)
            .await
            .map_err(to_message)?;
        let stored = titles::title(&db, &id).await.map_err(to_message)?;
        if stored.is_some_and(|title| title.in_library_at.is_none()) {
            titles::set_in_library(&db, &id, Some(now))
                .await
                .map_err(to_message)?;
        }
        tracing::info!(book = %book.title, chapters = units.len(), "EPUB imported");
        Ok((LOCAL_EPUB.to_owned(), hash))
    }

    /// A chapter of an imported book, from its unpacked files; images become `/local` links.
    async fn book_chapter(&self, hash: &str, id: &str) -> Result<Vec<TextBlock>, String> {
        let (path, start, end) = epub::split_id(id);
        let file = self
            .downloads_dir
            .join(BOOKS)
            .join(hash)
            .join(epub::safe_path(path));
        let html = tokio::fs::read(&file)
            .await
            .map_err(|_| "This chapter is missing from the imported book".to_owned())?;
        let html = String::from_utf8_lossy(&html).into_owned();
        let base = url::Url::parse(&format!("http://{BOOK_HOST}/"))
            .ok()
            .and_then(|root| root.join(path).ok());
        let mut blocks = html_blocks(epub::slice(&html, start, end), base.as_ref());
        if blocks.is_empty() {
            // A cover or illustration page drawn with SVG.
            blocks = epub::svg_images(&html)
                .into_iter()
                .filter_map(|href| base.as_ref()?.join(&href).ok())
                .map(|url| PassageBlock::Image(url.to_string()))
                .collect();
        }
        Ok(blocks
            .into_iter()
            .filter_map(|block| match block {
                PassageBlock::Image(url) => {
                    let url = url::Url::parse(&url).ok()?;
                    if url.host_str() != Some(BOOK_HOST) {
                        return Some(self.block(PassageBlock::Image(url.to_string())));
                    }
                    let inside = epub::resolve("", url.path().trim_start_matches('/'))?;
                    Some(TextBlock::Image {
                        url: self
                            .catalog
                            .images
                            .local(&format!("{BOOKS}/{hash}/{}", epub::safe_url_path(&inside))),
                    })
                }
                block => Some(self.block(block)),
            })
            .collect())
    }

    pub async fn set_in_library(&self, title_id: &str, in_library: bool) -> Result<(), String> {
        self.catalog.set_in_library(title_id, in_library).await
    }

    pub async fn library(&self) -> Result<Vec<MangaLibraryEntry>, String> {
        self.catalog.library().await
    }

    /// One chapter's text, with its neighbours in reading order.
    pub async fn chapter(&self, title_id: &str, unit_id: &str) -> Result<NovelChapter, String> {
        // Another translation of the chapter just read isn't the next one.
        let context = self
            .catalog
            .chapter_context(title_id, unit_id, true)
            .await?;
        let downloads = self.downloads();
        let blocks = if context.title.fields.source_id == LOCAL_EPUB {
            self.book_chapter(
                &context.title.fields.remote_id,
                &context.unit.fields.remote_id,
            )
            .await?
        } else {
            // A downloaded chapter reads from disk, without asking the source.
            let passage = match downloads.local_passage(title_id, unit_id).await {
                Some(passage) => passage,
                None => {
                    let extensions = self.catalog.extensions();
                    let (_ready, host) = extensions.ready().await;
                    host.novel_passage(
                        &context.title.fields.source_id,
                        &context.unit.fields.remote_id,
                    )
                    .await?
                }
            };
            passage.into_iter().map(|block| self.block(block)).collect()
        };
        let download = downloads
            .for_chapters(std::slice::from_ref(&context.unit.id))
            .await;
        Ok(NovelChapter {
            chapter: chapter_info(&context.unit, &download),
            title_id: context.title.id,
            title: context.title.fields.title,
            blocks,
            previous_chapter_id: context.previous_id,
            next_chapter_id: context.next_id,
        })
    }

    fn block(&self, block: PassageBlock) -> TextBlock {
        match block {
            PassageBlock::Paragraph(text) => TextBlock::Paragraph { text },
            PassageBlock::Heading(text) => TextBlock::Heading { text },
            PassageBlock::Separator => TextBlock::Separator,
            PassageBlock::Image(url) => TextBlock::Image {
                url: self.catalog.images.link(&url, &Default::default()),
            },
        }
    }

    /// Stores the character offset reached; reaching the end (`offset >= total - 1`) completes
    /// the chapter.
    pub async fn set_progress(&self, unit_id: &str, offset: u32, total: u32) -> Result<(), String> {
        self.catalog.set_progress(unit_id, offset, total).await
    }

    pub async fn set_read(&self, unit_id: &str, read: bool) -> Result<(), String> {
        self.catalog.set_read(unit_id, read).await
    }

    fn downloads(&self) -> tauri::State<'_, Arc<MangaDownloadService>> {
        self.app.state::<Arc<MangaDownloadService>>()
    }
}
