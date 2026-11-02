//! CBZ files: a chapter's pages, stored uncompressed (they're compressed images already), with a
//! ComicInfo.xml in the Anansi v2.0 schema that Mihon, Komga and Kavita read (fields as Mihon's
//! `getComicInfo` fills them, Apache-2.0). Raygun's own import (`comics.rs`) reads them back.

use std::fs::File;
use std::io::{Seek, Write};
use std::path::PathBuf;

use zip::ZipWriter;
use zip::write::SimpleFileOptions;

/// What a chapter's ComicInfo.xml says.
#[derive(Debug, Clone, Default)]
pub struct ComicInfo {
    pub series: String,
    /// The chapter's name.
    pub title: String,
    pub number: Option<f64>,
    pub summary: Option<String>,
    pub writers: Vec<String>,
    pub pencillers: Vec<String>,
    /// The scanlation group.
    pub translator: Option<String>,
    pub genres: Vec<String>,
    pub web: Option<String>,
    pub language: Option<String>,
    pub page_count: usize,
}

fn escape(text: &str) -> String {
    quick_xml::escape::escape(text).into_owned()
}

/// A chapter number as ComicInfo writes it: "12", "12.5".
pub fn number_text(number: f64) -> String {
    if number.fract() == 0.0 {
        format!("{}", number as i64)
    } else {
        let text = format!("{number:.3}");
        text.trim_end_matches('0').to_owned()
    }
}

impl ComicInfo {
    pub fn xml(&self) -> String {
        let mut xml = String::from(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<ComicInfo \
             xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\" \
             xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\n",
        );
        let mut field = |name: &str, value: &str| {
            if !value.trim().is_empty() {
                xml.push_str(&format!("  <{name}>{}</{name}>\n", escape(value.trim())));
            }
        };
        field("Title", &self.title);
        field("Series", &self.series);
        if let Some(number) = self.number.filter(|number| *number >= 0.0) {
            field("Number", &number_text(number));
        }
        field("Summary", self.summary.as_deref().unwrap_or_default());
        field("Writer", &self.writers.join(", "));
        field("Penciller", &self.pencillers.join(", "));
        field("Translator", self.translator.as_deref().unwrap_or_default());
        field("Genre", &self.genres.join(", "));
        field("Web", self.web.as_deref().unwrap_or_default());
        field("PageCount", &self.page_count.to_string());
        field("LanguageISO", self.language.as_deref().unwrap_or_default());
        xml.push_str("</ComicInfo>\n");
        xml
    }
}

fn stored() -> SimpleFileOptions {
    SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .large_file(false)
}

/// Writes the pages (`<0001.ext>` in order) and ComicInfo.xml.
fn write_into<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    pages: &[PathBuf],
    info: &ComicInfo,
) -> std::io::Result<()> {
    let width = pages.len().to_string().len().max(4);
    for (index, page) in pages.iter().enumerate() {
        let extension = page
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("jpg");
        zip.start_file(format!("{:0width$}.{extension}", index + 1), stored())?;
        std::io::copy(&mut File::open(page)?, zip)?;
    }
    let info = ComicInfo {
        page_count: pages.len(),
        ..info.clone()
    };
    zip.start_file(
        "ComicInfo.xml",
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
    )?;
    zip.write_all(info.xml().as_bytes())?;
    Ok(())
}

/// Writes a CBZ of `pages` into `file`, in streaming form when the file can't seek.
pub fn write(file: &mut File, pages: &[PathBuf], info: &ComicInfo) -> std::io::Result<()> {
    if super::output::seekable(file) {
        let mut zip = ZipWriter::new(&mut *file);
        write_into(&mut zip, pages, info)?;
        zip.finish()?;
    } else {
        let mut zip = ZipWriter::new_stream(&mut *file);
        write_into(&mut zip, pages, info)?;
        zip.finish()?;
    }
    Ok(())
}
