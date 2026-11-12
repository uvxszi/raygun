//! Novels (M31): `novel.source` catalogues through the shared `Catalog`, and chapters as text
//! blocks for the novel reader. Progress is a character offset into the chapter's text.
//! M33 added EPUB imports (unpacked under `downloads/books/`) and downloaded chapters.
//!
//! PDF books and chapters: an imported PDF is a book of the source `local.pdf`, copied to
//! `books/<sha256>/document.pdf`, its chapters the outline's top entries as runs of pages
//! (`pages:<first>-<last>`, counted from 1). A source's chapter may be a PDF too (`passage`
//! answers `{ pdf }`). Either way the chapter comes with [`PdfChapter`]: its pages drawn by the
//! stream proxy (`raygun_net::pdf`), read like a comic's, and its text on request
//! ([`NovelService::pdf_text`]) for the text view, reading aloud and search. Progress is then the
//! page reached. Its contents are a book's whole outline, each bookmark with the chapter and page
//! it leads to ([`pdf_contents`]), which the page and text views both list.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use base64::Engine;
use raygun_db::titles::{self, TitleFields, TitleRow, UnitFields};
use raygun_ext_host::novel::{NOVEL_SOURCE, Part, PassageBlock, html_blocks};
use raygun_model::{
    MangaBrowseMode, MangaLibraryEntry, MangaListingPage, MangaSource, MangaTitleView,
    NovelChapter, PdfChapter, PdfChapterText, PdfContentsEntry, PdfSearchHit, ReaderMode,
    TextBlock, TextPart,
};
use raygun_net::pdf::{Document, OutlineEntry, PdfRef, page_range};
use raygun_net::{ImageLinks, PdfStore};
use tauri::{AppHandle, Manager};

use crate::catalog::{Catalog, LOCAL_PREFIX, chapter_info, now_ms, to_message};
use crate::epub::{self, Epub};
use crate::manga_downloads::MangaDownloadService;

const KIND: &str = "novel";
/// The source id of imported EPUB books.
pub const LOCAL_EPUB: &str = "local.epub";
/// The source id of imported PDF books.
pub const LOCAL_PDF: &str = "local.pdf";
/// Folder under the downloads root holding unpacked books.
pub(crate) const BOOKS: &str = "books";
/// An imported PDF's file, and its cover, in its folder under [`BOOKS`].
const PDF_FILE: &str = "document.pdf";
const PDF_COVER: &str = "cover.png";
/// How wide an imported PDF's cover is drawn.
const COVER_WIDTH: u32 = 480;
/// A PDF without an outline is read in parts of this many pages.
const PART_PAGES: usize = 50;
/// Chapters of a PDF book at most.
const MAX_PDF_CHAPTERS: usize = 1_000;
/// Host of the placeholder base URL that chapter images resolve against.
const BOOK_HOST: &str = "book.invalid";

pub struct NovelService {
    app: AppHandle,
    catalog: Catalog,
    downloads_dir: PathBuf,
    /// Source chapters that turned out to be PDF documents, by chapter id, so their text and
    /// search needn't ask the source again.
    pdf_chapters: Mutex<HashMap<String, PdfRef>>,
}

/// Where a PDF chapter's pages are: its document and its pages in it (from 0).
struct PdfPlace {
    token: String,
    range: std::ops::Range<usize>,
}

/// The pages a `pages:<first>-<last>` chapter id names, counted from 1.
fn pages_of(id: &str) -> Option<(u32, u32)> {
    let (first, last) = id.strip_prefix("pages:")?.split_once('-')?;
    Some((first.parse().ok()?, last.parse().ok()?))
}

/// A PDF book's chapters from its outline: the top entries (or those under a lone top entry, the
/// book's own title), each running to the next; pages before the first are the front matter. A
/// document without an outline is read in parts. Each chapter is (name, first page, last page),
/// pages from 0.
pub(crate) fn pdf_chapters(outline: &[OutlineEntry], count: usize) -> Vec<(String, usize, usize)> {
    if count == 0 {
        return Vec::new();
    }
    let top = outline.iter().filter(|entry| entry.level == 0).count();
    let level = u32::from(top == 1 && outline.iter().any(|entry| entry.level == 1));
    let mut starts: Vec<(usize, String)> = Vec::new();
    let mut entries: Vec<&OutlineEntry> = outline
        .iter()
        .filter(|entry| entry.level == level && entry.page < count)
        .collect();
    entries.sort_by_key(|entry| entry.page);
    for entry in entries {
        match starts.last_mut() {
            // Two entries on one page are one chapter.
            Some((page, name)) if *page == entry.page => {
                name.push_str(" · ");
                name.push_str(&entry.title);
            }
            _ => starts.push((entry.page, entry.title.clone())),
        }
    }
    if starts.is_empty() {
        return (0..count)
            .step_by(PART_PAGES)
            .map(|first| {
                let last = (first + PART_PAGES).min(count) - 1;
                (format!("Pages {}–{}", first + 1, last + 1), first, last)
            })
            .collect();
    }
    match starts.first_mut() {
        // A cover page or two belong to the first chapter.
        Some((page, _)) if *page <= 2 => *page = 0,
        _ => starts.insert(0, (0, "Front matter".to_owned())),
    }
    starts.truncate(MAX_PDF_CHAPTERS);
    let ends: Vec<usize> = starts
        .iter()
        .skip(1)
        .map(|(page, _)| page - 1)
        .chain(std::iter::once(count - 1))
        .collect();
    starts
        .into_iter()
        .zip(ends)
        .map(|((first, name), last)| (name, first, last.max(first)))
        .collect()
}

/// A PDF's bookmarks as the reader's contents, in the outline's order: each with the chapter
/// holding its page and the page within it. `chapters` are page runs (from 0) that don't overlap;
/// bookmarks on pages none of them holds are left out.
pub(crate) fn pdf_contents(
    outline: Vec<OutlineEntry>,
    chapters: &[(String, std::ops::Range<usize>)],
) -> Vec<PdfContentsEntry> {
    let mut sorted: Vec<&(String, std::ops::Range<usize>)> = chapters.iter().collect();
    sorted.sort_by_key(|(_, range)| range.start);
    outline
        .into_iter()
        .filter_map(|entry| {
            // The last chapter starting at or before the page, if it reaches that far.
            let after = sorted.partition_point(|(_, range)| range.start <= entry.page);
            let (chapter_id, range) = sorted.get(after.checked_sub(1)?)?;
            range.contains(&entry.page).then(|| PdfContentsEntry {
                title: entry.title,
                chapter_id: chapter_id.clone(),
                page: (entry.page - range.start) as u32,
                page_number: entry.page as u32 + 1,
                level: entry.level,
            })
        })
        .collect()
}

/// A PDF's own title, unless it's a file name or a word processor's ("Microsoft Word - x.docx").
fn pdf_title(info: Option<&str>, name: Option<&str>) -> String {
    let own = info
        .map(|title| {
            title
                .trim()
                .trim_start_matches("Microsoft Word - ")
                .trim()
                .to_owned()
        })
        .filter(|title| {
            let lower = title.to_ascii_lowercase();
            !title.is_empty()
                && !lower.starts_with("untitled")
                && ![".doc", ".docx", ".pdf", ".tex", ".odt", ".indd"]
                    .iter()
                    .any(|extension| lower.ends_with(extension))
        });
    own.or_else(|| {
        let name = name?.trim();
        let stem = name
            .rsplit_once('.')
            .filter(|(_, extension)| extension.eq_ignore_ascii_case("pdf"))
            .map_or(name, |(stem, _)| stem);
        Some(stem.replace('_', " ").trim().to_owned()).filter(|stem| !stem.is_empty())
    })
    .unwrap_or_else(|| "PDF document".to_owned())
}

/// The authors a PDF names: one field, split where several are listed.
fn pdf_authors(author: Option<&str>) -> Vec<String> {
    let Some(author) = author else {
        return Vec::new();
    };
    author
        .split([';', '&'])
        .flat_map(|part| part.split(" and "))
        .flat_map(|part| {
            // "Last, First" stays one name; "A, B, C" (more than two) are several.
            if part.matches(',').count() >= 2 {
                part.split(',').collect::<Vec<_>>()
            } else {
                vec![part]
            }
        })
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The SHA-256 of a file, read in pieces. Blocking.
fn file_hash(path: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 256 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// A PDF page's text as the reader's blocks.
fn text_blocks(texts: &[raygun_net::pdf::PageText]) -> PdfChapterText {
    let mut blocks = Vec::new();
    let mut page_starts = Vec::new();
    for page in texts {
        page_starts.push(blocks.len() as u32);
        for block in &page.blocks {
            blocks.push(if block.heading {
                TextBlock::Heading {
                    text: block.text.clone(),
                }
            } else {
                TextBlock::paragraph(block.text.clone())
            });
        }
    }
    PdfChapterText {
        blocks,
        page_starts,
    }
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
            pdf_chapters: Mutex::default(),
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
        let row = if source_id == LOCAL_EPUB || source_id == LOCAL_PDF {
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
        } else if view.source_id == LOCAL_PDF {
            view.source_name = "PDF file".into();
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

    /// Imports an uploaded book (sent in slices, so a big book never crosses IPC whole): a PDF, or
    /// an EPUB. The file is deleted afterwards; `name` is what it was called, for a PDF's title.
    pub async fn import_book_upload(
        &self,
        file: PathBuf,
        name: Option<String>,
    ) -> Result<(String, String), String> {
        let probe = file.clone();
        let pdf = tauri::async_runtime::spawn_blocking(move || is_pdf_file(&probe))
            .await
            .unwrap_or(false);
        if pdf {
            let result = self.import_pdf_file(&file, name.as_deref()).await;
            let _ = tokio::fs::remove_file(&file).await;
            return result;
        }
        self.import_epub_file(file).await
    }

    /// Imports an uploaded EPUB file; the file is deleted afterwards.
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
                        return self.block(PassageBlock::Image(url.to_string()));
                    }
                    let inside = epub::resolve("", url.path().trim_start_matches('/'))?;
                    Some(TextBlock::Image {
                        url: self
                            .catalog
                            .images
                            .local(&format!("{BOOKS}/{hash}/{}", epub::safe_url_path(&inside))),
                    })
                }
                block => self.block(block),
            })
            .collect())
    }

    pub async fn set_in_library(&self, title_id: &str, in_library: bool) -> Result<(), String> {
        self.catalog.set_in_library(title_id, in_library).await
    }

    pub async fn library(&self) -> Result<Vec<MangaLibraryEntry>, String> {
        self.catalog.library().await
    }

    /// One chapter's text, with its neighbours in reading order; a PDF chapter's pages instead.
    pub async fn chapter(&self, title_id: &str, unit_id: &str) -> Result<NovelChapter, String> {
        // Another translation of the chapter just read isn't the next one.
        let context = self
            .catalog
            .chapter_context(title_id, unit_id, true)
            .await?;
        let downloads = self.downloads();
        let mut pdf = None;
        let blocks = if context.title.fields.source_id == LOCAL_EPUB {
            self.book_chapter(
                &context.title.fields.remote_id,
                &context.unit.fields.remote_id,
            )
            .await?
        } else if context.title.fields.source_id == LOCAL_PDF {
            let place = self.local_pdf_place(&context.title, &context.unit.fields.remote_id)?;
            // The book's chapters share its document: its whole outline is the contents.
            let chapters = self.local_pdf_chapters(title_id).await?;
            pdf = Some(
                self.pdf_chapter(place, &context.title, unit_id, Some(chapters))
                    .await?,
            );
            Vec::new()
        } else {
            let passage = self
                .passage(
                    title_id,
                    unit_id,
                    &context.title,
                    &context.unit.fields.remote_id,
                )
                .await?;
            if let [PassageBlock::Pdf(reference)] = passage.as_slice() {
                let place = self.web_pdf_place(title_id, unit_id, reference).await;
                pdf = Some(
                    self.pdf_chapter(place, &context.title, unit_id, None)
                        .await?,
                );
                Vec::new()
            } else {
                passage
                    .into_iter()
                    .filter_map(|block| self.block(block))
                    .collect()
            }
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
            pdf,
        })
    }

    /// A source chapter's passage: from its download, else the source. A PDF one is remembered,
    /// so its text and search needn't ask again.
    async fn passage(
        &self,
        title_id: &str,
        unit_id: &str,
        title: &TitleRow,
        remote_id: &str,
    ) -> Result<Vec<PassageBlock>, String> {
        // A downloaded chapter reads from disk, without asking the source.
        let passage = match self.downloads().local_passage(title_id, unit_id).await {
            Some(passage) => passage,
            None => {
                let extensions = self.catalog.extensions();
                let (_ready, host) = extensions.ready().await;
                host.novel_passage(&title.fields.source_id, remote_id)
                    .await?
            }
        };
        if let [PassageBlock::Pdf(reference)] = passage.as_slice() {
            self.pdf_chapters
                .lock()
                .expect("pdf chapters poisoned")
                .insert(unit_id.to_owned(), reference.clone());
        }
        Ok(passage)
    }

    fn block(&self, block: PassageBlock) -> Option<TextBlock> {
        Some(match block {
            PassageBlock::Paragraph(text) => TextBlock::paragraph(text),
            PassageBlock::Heading(text) => TextBlock::Heading { text },
            PassageBlock::Separator => TextBlock::Separator,
            PassageBlock::Image(url) => TextBlock::Image {
                url: self.catalog.images.link(&url, &Default::default()),
            },
            PassageBlock::Rich(parts) => TextBlock::Paragraph {
                text: raygun_ext_host::novel::plain_text(&parts),
                parts: Some(parts.into_iter().map(text_part).collect()),
            },
            PassageBlock::Math(formula) => TextBlock::Math {
                mathml: formula.mathml,
                text: formula.text,
                speech: formula.speech,
            },
            PassageBlock::Code(code) => TextBlock::Code {
                text: code.text,
                language: code.language,
            },
            // A PDF passage is read as pages (`chapter`).
            PassageBlock::Pdf(_) => return None,
        })
    }

    /// An imported PDF's chapters: each with its pages in the document (from 0).
    async fn local_pdf_chapters(
        &self,
        title_id: &str,
    ) -> Result<Vec<(String, std::ops::Range<usize>)>, String> {
        Ok(titles::units(&self.catalog.db(), title_id)
            .await
            .map_err(to_message)?
            .into_iter()
            .filter_map(|unit| {
                let (first, last) = pages_of(&unit.fields.remote_id)?;
                Some((unit.id, first.saturating_sub(1) as usize..last as usize))
            })
            .collect())
    }

    /// An imported PDF's chapter: its document and the pages its id names.
    fn local_pdf_place(&self, title: &TitleRow, remote_id: &str) -> Result<PdfPlace, String> {
        let (first, last) = pages_of(remote_id)
            .ok_or_else(|| "This chapter is missing from the book".to_owned())?;
        Ok(PdfPlace {
            token: self
                .pdfs()
                .local_token(&format!("{BOOKS}/{}/{PDF_FILE}", title.fields.remote_id)),
            range: (first.saturating_sub(1) as usize)..(last as usize),
        })
    }

    /// A source's PDF chapter: its downloaded copy, else the document on the web.
    async fn web_pdf_place(&self, title_id: &str, unit_id: &str, reference: &PdfRef) -> PdfPlace {
        let pdfs = self.pdfs();
        let copy = crate::manga_downloads::novel_pdf_path(title_id, unit_id);
        let token = if tokio::fs::try_exists(self.downloads_dir.join(&copy))
            .await
            .unwrap_or(false)
        {
            pdfs.local_token(&copy)
        } else {
            pdfs.web_token(&reference.url, &reference.headers)
        };
        PdfPlace {
            token,
            // Applied to the document's length once it's open.
            range: reference
                .first_page
                .map_or(0, |first| first.max(1) as usize - 1)
                ..reference.last_page.map_or(usize::MAX, |last| last as usize),
        }
    }

    /// Where a PDF chapter is, asking the source only when it isn't known yet.
    async fn pdf_place(&self, title_id: &str, unit_id: &str) -> Result<PdfPlace, String> {
        let db = self.catalog.db();
        let title = titles::title(&db, title_id)
            .await
            .map_err(to_message)?
            .ok_or_else(|| "This title isn't stored".to_owned())?;
        let unit = titles::unit(&db, unit_id)
            .await
            .map_err(to_message)?
            .ok_or_else(|| "This chapter isn't stored".to_owned())?;
        if title.fields.source_id == LOCAL_PDF {
            return self.local_pdf_place(&title, &unit.fields.remote_id);
        }
        let known = self
            .pdf_chapters
            .lock()
            .expect("pdf chapters poisoned")
            .get(unit_id)
            .cloned();
        let reference = match known {
            Some(reference) => reference,
            None => match self
                .passage(title_id, unit_id, &title, &unit.fields.remote_id)
                .await?
                .as_slice()
            {
                [PassageBlock::Pdf(reference)] => reference.clone(),
                _ => return Err("This chapter isn't a PDF".into()),
            },
        };
        Ok(self.web_pdf_place(title_id, unit_id, &reference).await)
    }

    /// What the reader needs of a PDF chapter: its pages (the document opened, a web one fetched
    /// first), and the bookmarks: those of every chapter of `chapters` (a book's, which share its
    /// document), else this chapter's own.
    async fn pdf_chapter(
        &self,
        place: PdfPlace,
        title: &TitleRow,
        unit_id: &str,
        chapters: Option<Vec<(String, std::ops::Range<usize>)>>,
    ) -> Result<PdfChapter, String> {
        let pdfs = self.pdfs();
        let document = pdfs.document(&place.token).await.map_err(String::from)?;
        let count = document.page_count();
        let range = clamp(&place.range, count);
        let pages = range
            .clone()
            .map(|index| pdfs.page_url(&place.token, index))
            .collect();
        let outline = tauri::async_runtime::spawn_blocking(move || document.outline())
            .await
            .map_err(to_message)?;
        let chapters = chapters.unwrap_or_else(|| vec![(unit_id.to_owned(), range.clone())]);
        let contents = pdf_contents(outline, &chapters);
        let default_mode = match crate::settings::load(&self.catalog.db()).await {
            Ok(settings) => settings.manga_reader.default_mode,
            Err(_) => raygun_model::MangaReaderPrefs::default().default_mode,
        };
        let own_reader_mode = ReaderMode::chosen(&title.reader_mode);
        Ok(PdfChapter {
            pages,
            first_page: range.start as u32 + 1,
            reader_mode: own_reader_mode.unwrap_or(default_mode),
            own_reader_mode,
            contents,
        })
    }

    /// A PDF chapter's text, page by page, for the text view and reading aloud.
    pub async fn pdf_text(&self, title_id: &str, unit_id: &str) -> Result<PdfChapterText, String> {
        let place = self.pdf_place(title_id, unit_id).await?;
        let pdfs = self.pdfs();
        let count = pdfs
            .document(&place.token)
            .await
            .map_err(String::from)?
            .page_count();
        let texts = pdfs
            .texts(&place.token, clamp(&place.range, count))
            .await
            .map_err(String::from)?;
        Ok(text_blocks(&texts))
    }

    /// Where `query` appears in a PDF: an imported book's whole document (each hit with its
    /// chapter), or a source chapter's pages.
    pub async fn pdf_search(
        &self,
        title_id: &str,
        unit_id: &str,
        query: &str,
    ) -> Result<Vec<PdfSearchHit>, String> {
        let place = self.pdf_place(title_id, unit_id).await?;
        let pdfs = self.pdfs();
        let count = pdfs
            .document(&place.token)
            .await
            .map_err(String::from)?
            .page_count();
        let db = self.catalog.db();
        let local = titles::title(&db, title_id)
            .await
            .map_err(to_message)?
            .is_some_and(|title| title.fields.source_id == LOCAL_PDF);
        // A book's chapters share its document: each hit goes to the chapter holding its page.
        let chapters: Vec<(String, std::ops::Range<usize>)> = if local {
            self.local_pdf_chapters(title_id).await?
        } else {
            vec![(unit_id.to_owned(), clamp(&place.range, count))]
        };
        let searched = if local {
            0..count
        } else {
            clamp(&place.range, count)
        };
        let hits = pdfs
            .search(&place.token, query, searched)
            .await
            .map_err(String::from)?;
        Ok(hits
            .into_iter()
            .filter_map(|hit| {
                let (chapter_id, range) = chapters
                    .iter()
                    .find(|(_, range)| range.contains(&hit.page))?;
                Some(PdfSearchHit {
                    chapter_id: chapter_id.clone(),
                    page: (hit.page - range.start) as u32,
                    page_number: hit.page as u32 + 1,
                    before: hit.before,
                    matched: hit.matched,
                    after: hit.after,
                })
            })
            .collect())
    }

    /// Imports a PDF file (an upload, a Local Files folder's, one another app handed over):
    /// copied to `books/<sha256>/document.pdf` with its first page as the cover, its chapters
    /// from its outline. The same file imported again keeps its title and progress. `name` is the
    /// file's name, the title when the document gives none. Returns the source's id
    /// (`local.pdf`) and the book's id.
    pub async fn import_pdf_file(
        &self,
        file: &Path,
        name: Option<&str>,
    ) -> Result<(String, String), String> {
        let books = self.downloads_dir.join(BOOKS);
        let source = file.to_path_buf();
        let (hash, info, chapters) =
            tauri::async_runtime::spawn_blocking(move || -> Result<_, String> {
                let size = std::fs::metadata(&source)
                    .map_err(|_| "The file couldn't be read".to_owned())?
                    .len();
                if size > raygun_net::pdf::MAX_PDF_BYTES {
                    return Err("This PDF is larger than 1 GB".into());
                }
                let hash =
                    file_hash(&source).map_err(|_| "The file couldn't be read".to_owned())?;
                let dir = books.join(&hash);
                let target = dir.join(PDF_FILE);
                if !target.is_file() {
                    std::fs::create_dir_all(&dir).map_err(to_message)?;
                    let partial = dir.join(format!("{PDF_FILE}.part"));
                    std::fs::copy(&source, &partial).map_err(to_message)?;
                    std::fs::rename(&partial, &target).map_err(to_message)?;
                }
                let document = match Document::open(&target) {
                    Ok(document) => document,
                    Err(error) => {
                        let _ = std::fs::remove_dir_all(&dir);
                        return Err(error);
                    }
                };
                let cover = dir.join(PDF_COVER);
                if !cover.is_file()
                    && let Ok(png) = document.render_png(0, COVER_WIDTH)
                {
                    let _ = std::fs::write(&cover, png);
                }
                let chapters = pdf_chapters(&document.outline(), document.page_count());
                Ok((hash, document.info(), chapters))
            })
            .await
            .map_err(to_message)??;

        let db = self.catalog.db();
        let now = now_ms();
        let cover = self
            .downloads_dir
            .join(BOOKS)
            .join(&hash)
            .join(PDF_COVER)
            .is_file()
            .then(|| format!("{LOCAL_PREFIX}{BOOKS}/{hash}/{PDF_COVER}"));
        let title = pdf_title(info.title.as_deref(), name);
        let id = titles::upsert_title(
            &db,
            &TitleFields {
                kind: KIND.into(),
                source_id: LOCAL_PDF.into(),
                remote_id: hash.clone(),
                title: title.clone(),
                cover,
                cover_headers: Default::default(),
                authors: pdf_authors(info.author.as_deref()),
                artists: Vec::new(),
                description: info.subject.clone(),
                genres: Vec::new(),
                status: "completed".into(),
                url: None,
                media_type: None,
                year: None,
            },
            now,
        )
        .await
        .map_err(to_message)?;
        let units: Vec<UnitFields> = chapters
            .iter()
            .enumerate()
            .map(|(index, (name, first, last))| UnitFields {
                remote_id: format!("pages:{}-{}", first + 1, last + 1),
                name: name.clone(),
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
        tracing::info!(book = %title, chapters = units.len(), "PDF imported");
        Ok((LOCAL_PDF.to_owned(), hash))
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

    fn pdfs(&self) -> PdfStore {
        self.app.state::<PdfStore>().inner().clone()
    }
}

/// `range` within a document of `count` pages, never empty.
fn clamp(range: &std::ops::Range<usize>, count: usize) -> std::ops::Range<usize> {
    let first = u32::try_from(range.start + 1).ok();
    let last = u32::try_from(range.end.min(count)).ok();
    page_range(first, last, count)
}

/// Whether a file starts as a PDF does. Blocking.
fn is_pdf_file(path: &Path) -> bool {
    use std::io::Read;
    let mut head = [0u8; 1024];
    let read = std::fs::File::open(path)
        .and_then(|mut file| file.read(&mut head))
        .unwrap_or(0);
    raygun_net::pdf::is_pdf(&head[..read])
}

/// A paragraph's part as the reader gets it.
fn text_part(part: Part) -> TextPart {
    match part {
        Part::Text(text) => TextPart::Text { text },
        Part::Sup(text) => TextPart::Sup { text },
        Part::Sub(text) => TextPart::Sub { text },
        Part::Math(formula) => TextPart::Math {
            mathml: formula.mathml,
            text: formula.text,
            speech: formula.speech,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(title: &str, page: usize, level: u32) -> OutlineEntry {
        OutlineEntry {
            title: title.into(),
            page,
            level,
        }
    }

    fn named(chapters: &[(String, usize, usize)]) -> Vec<(&str, usize, usize)> {
        chapters
            .iter()
            .map(|(name, first, last)| (name.as_str(), *first, *last))
            .collect()
    }

    #[test]
    fn a_pdfs_outline_makes_its_chapters() {
        let outline = [
            entry("Preface", 4, 0),
            entry("Chapter 1", 10, 0),
            entry("1.1 Sets", 12, 1),
            entry("Chapter 2", 30, 0),
            entry("Part II", 30, 0),
            entry("Index", 99, 0),
        ];
        assert_eq!(
            named(&pdf_chapters(&outline, 60)),
            [
                ("Front matter", 0, 3),
                ("Preface", 4, 9),
                ("Chapter 1", 10, 29),
                ("Chapter 2 · Part II", 30, 59),
            ],
            "the index lies past the end, and two entries on one page are one chapter"
        );
        // A cover before the first entry belongs to it.
        assert_eq!(
            named(&pdf_chapters(&[entry("One", 1, 0), entry("Two", 5, 0)], 8)),
            [("One", 0, 4), ("Two", 5, 7)]
        );
        // One top entry (the book's title): the entries under it are the chapters.
        let book = [
            entry("A Book", 0, 0),
            entry("First", 0, 1),
            entry("Second", 3, 1),
        ];
        assert_eq!(
            named(&pdf_chapters(&book, 6)),
            [("First", 0, 2), ("Second", 3, 5)]
        );
    }

    /// The contents both of a book's views list: the whole outline, each bookmark with the chapter
    /// holding its page, as `pdf_chapters` made the chapters.
    #[test]
    fn a_pdf_books_contents_lead_into_every_chapter() {
        let outline = vec![
            entry("Preface", 4, 0),
            entry("Chapter 1", 10, 0),
            entry("1.1 Sets", 12, 1),
            entry("1.2 Maps", 10, 1),
            entry("Chapter 2", 30, 0),
            entry("Index", 99, 0),
        ];
        let chapters: Vec<(String, std::ops::Range<usize>)> = pdf_chapters(&outline, 60)
            .into_iter()
            .enumerate()
            .map(|(index, (_, first, last))| (format!("u{index}"), first..last + 1))
            .rev()
            .collect();
        let shown: Vec<(String, String, u32, u32, u32)> = pdf_contents(outline, &chapters)
            .into_iter()
            .map(|entry| {
                (
                    entry.title,
                    entry.chapter_id,
                    entry.page,
                    entry.page_number,
                    entry.level,
                )
            })
            .collect();
        let row = |title: &str, chapter: &str, page, number, level| {
            (title.to_owned(), chapter.to_owned(), page, number, level)
        };
        assert_eq!(
            shown,
            [
                row("Preface", "u1", 0, 5, 0),
                row("Chapter 1", "u2", 0, 11, 0),
                row("1.1 Sets", "u2", 2, 13, 1),
                row("1.2 Maps", "u2", 0, 11, 1),
                row("Chapter 2", "u3", 0, 31, 0),
            ],
            "in the outline's order; the index lies past the last page"
        );
        // A source's chapter: only the bookmarks among its own pages.
        let own = pdf_contents(
            vec![entry("Before", 1, 0), entry("Inside", 6, 0)],
            &[("c".to_owned(), 5..9)],
        );
        assert_eq!(own.len(), 1);
        assert_eq!((own[0].page, own[0].page_number), (1, 7));
    }

    /// A real book's chapters and contents, as both of its views list them (Think Python 2e,
    /// greenteapress.com/thinkpython2/thinkpython2.pdf, has 21 chapters of sections):
    /// `PDF_FILE=<a PDF> cargo test -p raygun-app --lib -- --ignored pdf_book_contents_live --nocapture`.
    #[test]
    #[ignore = "reads the PDF that PDF_FILE names"]
    fn pdf_book_contents_live() {
        let path = std::env::var("PDF_FILE").expect("PDF_FILE names a PDF with an outline");
        let document = Document::open(Path::new(&path)).unwrap();
        let count = document.page_count();
        let outline = document.outline();
        assert!(!outline.is_empty(), "the book has an outline");
        let chapters: Vec<(String, std::ops::Range<usize>)> = pdf_chapters(&outline, count)
            .into_iter()
            .enumerate()
            .map(|(index, (name, first, last))| {
                println!("chapter u{index}: {name}, pages {}–{}", first + 1, last + 1);
                (format!("u{index}"), first..last + 1)
            })
            .collect();
        let contents = pdf_contents(outline.clone(), &chapters);
        assert_eq!(contents.len(), outline.len(), "every bookmark is listed");
        for entry in &contents {
            let (_, range) = chapters
                .iter()
                .find(|(id, _)| *id == entry.chapter_id)
                .unwrap();
            let page = range.start + entry.page as usize;
            assert!(range.contains(&page), "{entry:?}");
            assert_eq!(page + 1, entry.page_number as usize);
        }
        println!(
            "{} bookmarks over {} chapters; first {:?}",
            contents.len(),
            chapters.len(),
            contents.first()
        );
    }

    #[test]
    fn a_pdf_without_an_outline_is_read_in_parts() {
        assert_eq!(
            named(&pdf_chapters(&[], 120)),
            [
                ("Pages 1–50", 0, 49),
                ("Pages 51–100", 50, 99),
                ("Pages 101–120", 100, 119)
            ]
        );
        assert_eq!(named(&pdf_chapters(&[], 3)), [("Pages 1–3", 0, 2)]);
        assert!(pdf_chapters(&[], 0).is_empty());
    }

    #[test]
    fn pdf_chapter_ids_name_their_pages() {
        assert_eq!(pages_of("pages:3-40"), Some((3, 40)));
        assert_eq!(pages_of("chapter-1"), None);
        assert_eq!(clamp(&(2..5), 10), 2..5);
        assert_eq!(clamp(&(0..usize::MAX), 7), 0..7);
        assert_eq!(clamp(&(9..12), 4), 3..4);
    }

    #[test]
    fn pdfs_are_named_by_their_title_else_their_file() {
        assert_eq!(
            pdf_title(Some("Calculus Volume 1"), Some("x.pdf")),
            "Calculus Volume 1"
        );
        assert_eq!(
            pdf_title(
                Some("Microsoft Word - notes.docx"),
                Some("Lecture_Notes.pdf")
            ),
            "Lecture Notes"
        );
        assert_eq!(pdf_title(Some("  "), None), "PDF document");
        assert_eq!(
            pdf_authors(Some("Gilbert Strang; Edwin Herman")),
            ["Gilbert Strang", "Edwin Herman"]
        );
        assert_eq!(pdf_authors(Some("Doe, Jane")), ["Doe, Jane"]);
        assert_eq!(pdf_authors(Some("A and B")), ["A", "B"]);
    }
}
