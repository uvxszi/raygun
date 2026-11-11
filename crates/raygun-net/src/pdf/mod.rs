//! PDF documents for the readers: books imported from the device and chapters sources answer as
//! PDF files. Read with `hayro` (pure Rust, the same rasterizer converted Mihon sources draw their
//! PDF pages with), so it works alike on Android and desktop without a native library:
//! - pages are drawn as pictures at the width the reader asks for ([`Document::render_png`]),
//!   served by the stream proxy's `/pdf/` route and kept in the page cache ([`PdfStore`]);
//! - each page's text is read back into paragraphs and headings ([`text`]) for the text view,
//!   searching and reading aloud;
//! - the outline (bookmarks) gives a book its chapters ([`Document::outline`]).
//!
//! Documents open from a file, mapped rather than read, so a large textbook isn't held in memory.

mod outline;
mod render;
mod store;
#[cfg(test)]
mod tests;
pub mod text;

use std::collections::BTreeMap;
use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use hayro::hayro_syntax::Pdf;
use serde::{Deserialize, Serialize};

pub use outline::OutlineEntry;
pub use store::{PdfError, PdfStore, SearchHit};
pub use text::PageText;

/// The mark a chapter's page list carries a PDF with (`<address>#raygun-pdf` or
/// `<address>#raygun-pdf=3-40`): the readers draw its pages instead of fetching one picture.
pub const PDF_MARK: &str = "#raygun-pdf";

/// A chapter that is a PDF document on the web, or a run of its pages, as a source answers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfRef {
    pub url: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    /// The chapter's first page, counted from 1; the document's first when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_page: Option<u32>,
    /// The chapter's last page, included; the document's last when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_page: Option<u32>,
}

impl PdfRef {
    /// The address a page list carries it as, with [`PDF_MARK`].
    pub fn marked(&self) -> String {
        match (self.first_page, self.last_page) {
            (None, None) => format!("{}{PDF_MARK}", self.url),
            (first, last) => format!(
                "{}{PDF_MARK}={}-{}",
                self.url,
                first.unwrap_or(1),
                last.map(|last| last.to_string()).unwrap_or_default()
            ),
        }
    }

    /// The document a marked page address names, with the headers its page carries.
    pub fn from_marked(url: &str, headers: &BTreeMap<String, String>) -> Option<Self> {
        let at = url.rfind(PDF_MARK)?;
        let rest = &url[at + PDF_MARK.len()..];
        let (first_page, last_page) = match rest.strip_prefix('=') {
            None if rest.is_empty() => (None, None),
            None => return None,
            Some(range) => {
                let (first, last) = range.split_once('-').unwrap_or((range, range));
                (
                    first.trim().parse().ok().filter(|first| *first > 0),
                    last.trim().parse().ok().filter(|last| *last > 0),
                )
            }
        };
        Some(Self {
            url: url[..at].to_owned(),
            headers: headers.clone(),
            first_page,
            last_page,
        })
    }

    /// The pages it covers in a document of `count` pages, as indexes from 0; a range past the
    /// document's end is cut to it, and a backwards one is the first page alone.
    pub fn pages(&self, count: usize) -> Range<usize> {
        page_range(self.first_page, self.last_page, count)
    }
}

/// Pages `first..=last` (counted from 1) of a document of `count` pages, as indexes from 0.
pub fn page_range(first: Option<u32>, last: Option<u32>, count: usize) -> Range<usize> {
    if count == 0 {
        return 0..0;
    }
    let start = (first.unwrap_or(1).max(1) as usize - 1).min(count - 1);
    let end = last.map_or(count, |last| (last as usize).min(count));
    start..end.max(start + 1)
}

/// What a document says about itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Info {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
}

/// An open PDF document.
pub struct Document {
    pdf: Pdf,
}

/// The largest document read, a long scanned book.
pub const MAX_PDF_BYTES: u64 = 1024 * 1024 * 1024;

impl Document {
    /// Opens the file at `path`, mapped into memory. Blocking.
    pub fn open(path: &Path) -> Result<Self, String> {
        let file = std::fs::File::open(path).map_err(|_| "The PDF file isn't there".to_owned())?;
        let size = file.metadata().map(|meta| meta.len()).unwrap_or_default();
        if size > MAX_PDF_BYTES {
            return Err("This PDF is larger than 1 GB".into());
        }
        // SAFETY: the files mapped are Raygun's own copies (imports, downloads, the web copies'
        // folder), written whole before they're opened and never changed while open.
        let map = unsafe { memmap2::Mmap::map(&file) }
            .map_err(|error| format!("The PDF couldn't be read ({error})"))?;
        Self::from_data(Arc::new(map))
    }

    /// Reads a document held in memory.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, String> {
        Self::from_data(Arc::new(bytes))
    }

    fn from_data<T: AsRef<[u8]> + Send + Sync + 'static>(data: Arc<T>) -> Result<Self, String> {
        let bytes: &[u8] = (*data).as_ref();
        if !is_pdf(bytes) {
            return Err("This file isn't a PDF".into());
        }
        let pdf = Pdf::new(data).map_err(|error| match error {
            hayro::hayro_syntax::LoadPdfError::Decryption(_) => {
                "This PDF is locked with a password".to_owned()
            }
            hayro::hayro_syntax::LoadPdfError::Invalid => "This PDF couldn't be read".to_owned(),
        })?;
        if pdf.pages().is_empty() {
            return Err("This PDF has no pages".into());
        }
        Ok(Self { pdf })
    }

    pub fn page_count(&self) -> usize {
        self.pdf.pages().len()
    }

    /// A page's width and height in points, as it's shown (rotation applied).
    pub fn page_size(&self, index: usize) -> Option<(f32, f32)> {
        self.pdf
            .pages()
            .get(index)
            .map(|page| page.render_dimensions())
    }

    pub fn info(&self) -> Info {
        let metadata = self.pdf.metadata();
        let field = |value: &Option<Box<[u8]>>| {
            value
                .as_deref()
                .map(text_string)
                .map(|text| text.trim().to_owned())
                .filter(|text| !text.is_empty())
        };
        Info {
            title: field(&metadata.title),
            author: field(&metadata.author),
            subject: field(&metadata.subject),
        }
    }

    /// The bookmarks, in order, each with the page it leads to.
    pub fn outline(&self) -> Vec<OutlineEntry> {
        outline::read(&self.pdf)
    }

    /// Page `index` drawn `width` pixels wide on white, as PNG. Blocking.
    pub fn render_png(&self, index: usize, width: u32) -> Result<Vec<u8>, String> {
        let page = self
            .pdf
            .pages()
            .get(index)
            .ok_or_else(|| format!("The PDF has no page {}", index + 1))?;
        render::png(page, width)
    }

    /// Page `index`'s text, as paragraphs and headings. Blocking.
    pub fn page_text(&self, index: usize) -> PageText {
        match self.pdf.pages().get(index) {
            Some(page) => text::read(page),
            None => PageText::default(),
        }
    }
}

/// Whether `bytes` start as a PDF does (the header may follow a little junk).
pub fn is_pdf(bytes: &[u8]) -> bool {
    bytes[..bytes.len().min(1024)]
        .windows(5)
        .any(|window| window == b"%PDF-")
}

/// A text string from a PDF: UTF-16 or UTF-8 with their byte order marks, else PDFDocEncoding.
pub(crate) fn text_string(bytes: &[u8]) -> String {
    if let Some(utf16) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        let units: Vec<u16> = utf16
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_be_bytes(*pair))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    if let Some(utf8) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(utf8).into_owned();
    }
    bytes.iter().map(|byte| pdf_doc_char(*byte)).collect()
}

/// PDFDocEncoding: Latin-1 but for 0x18–0x1F and 0x80–0x9F (PDF 32000-1, Annex D).
fn pdf_doc_char(byte: u8) -> char {
    const HIGH: [char; 32] = [
        '•', '†', '‡', '…', '—', '–', 'ƒ', '⁄', '‹', '›', '−', '‰', '„', '“', '”', '‘', '’', '‚',
        '™', 'ﬁ', 'ﬂ', 'Ł', 'Œ', 'Š', 'Ÿ', 'Ž', 'ı', 'ł', 'œ', 'š', 'ž', '\u{FFFD}',
    ];
    const LOW: [char; 8] = ['˘', 'ˇ', 'ˆ', '˙', '˝', '˛', '˚', '˜'];
    match byte {
        0x18..=0x1F => LOW[usize::from(byte - 0x18)],
        0x80..=0x9F => HIGH[usize::from(byte - 0x80)],
        0xA0 => '€',
        _ => char::from(byte),
    }
}
