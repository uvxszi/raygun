//! Novels as files (M46.1): an EPUB 3 book (with an EPUB 2 table of contents for older readers)
//! or plain text, from the chapters' text blocks. Images in chapters are left out: they aren't
//! downloaded with the text. Formulas go in as MathML (their chapters marked `mathml` in the
//! manifest, as EPUB 3 asks), and in plain text as their text.

use std::fs::File;
use std::io::{Seek, Write};

use raygun_model::{TextBlock, TextPart};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

/// A book to write.
pub struct Book {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub description: Option<String>,
    pub language: String,
    pub genres: Vec<String>,
    /// The cover image's bytes and file extension.
    pub cover: Option<(Vec<u8>, String)>,
    pub chapters: Vec<BookChapter>,
    /// `YYYY-MM-DDThh:mm:ssZ`.
    pub modified: String,
}

pub struct BookChapter {
    pub title: String,
    pub blocks: Vec<TextBlock>,
}

fn escape(text: &str) -> String {
    quick_xml::escape::escape(text).into_owned()
}

fn image_mime(extension: &str) -> &'static str {
    match extension {
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "image/jpeg",
    }
}

/// The chapter's blocks without a first heading that only repeats its title (many sources start
/// the text with it).
fn body_blocks(chapter: &BookChapter) -> &[TextBlock] {
    match chapter.blocks.first() {
        Some(TextBlock::Heading { text })
            if text.trim().eq_ignore_ascii_case(chapter.title.trim()) =>
        {
            &chapter.blocks[1..]
        }
        _ => &chapter.blocks,
    }
}

/// Whether a chapter holds formulas (its document is then marked `mathml`).
fn has_math(chapter: &BookChapter) -> bool {
    chapter.blocks.iter().any(|block| match block {
        TextBlock::Math { .. } => true,
        TextBlock::Paragraph {
            parts: Some(parts), ..
        } => parts
            .iter()
            .any(|part| matches!(part, TextPart::Math { .. })),
        _ => false,
    })
}

/// A paragraph's parts as XHTML: formulas as their MathML (already well-formed MathML only).
fn parts_xhtml(parts: &[TextPart]) -> String {
    parts
        .iter()
        .map(|part| match part {
            TextPart::Text { text } => escape(text),
            TextPart::Sup { text } => format!("<sup>{}</sup>", escape(text)),
            TextPart::Sub { text } => format!("<sub>{}</sub>", escape(text)),
            TextPart::Math { mathml, .. } => mathml.clone(),
        })
        .collect()
}

fn chapter_xhtml(chapter: &BookChapter, language: &str) -> String {
    let mut body = format!("<h2>{}</h2>\n", escape(&chapter.title));
    for block in body_blocks(chapter) {
        match block {
            TextBlock::Paragraph {
                parts: Some(parts), ..
            } => body.push_str(&format!("<p>{}</p>\n", parts_xhtml(parts))),
            TextBlock::Paragraph { text, .. } => {
                body.push_str(&format!("<p>{}</p>\n", escape(text)))
            }
            TextBlock::Heading { text } => body.push_str(&format!("<h3>{}</h3>\n", escape(text))),
            TextBlock::Separator => body.push_str("<hr/>\n"),
            TextBlock::Image { .. } => {}
            TextBlock::Math { mathml, .. } => {
                body.push_str(&format!("<div class=\"formula\">{mathml}</div>\n"))
            }
        }
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE html>\n\
         <html xmlns=\"http://www.w3.org/1999/xhtml\" xml:lang=\"{language}\" lang=\"{language}\">\n\
         <head><title>{}</title><link rel=\"stylesheet\" type=\"text/css\" href=\"../style.css\"/></head>\n\
         <body>\n{body}</body>\n</html>\n",
        escape(&chapter.title)
    )
}

const STYLE: &str = "body { margin: 0 5%; line-height: 1.5; }\n\
    h2 { text-align: center; margin: 1.5em 0; }\n\
    p { margin: 0 0 0.8em; text-indent: 1.2em; }\n\
    hr { margin: 1.5em 25%; }\n\
    div.formula { text-align: center; margin: 0.8em 0; overflow-x: auto; }\n";

fn write_into<W: Write + Seek>(zip: &mut ZipWriter<W>, book: &Book) -> std::io::Result<()> {
    let deflated =
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    // The mimetype comes first, uncompressed, so readers can sniff the file.
    zip.start_file(
        "mimetype",
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )?;
    zip.write_all(b"application/epub+zip")?;
    zip.start_file("META-INF/container.xml", deflated)?;
    zip.write_all(
        b"<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
          <container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\">\n\
          <rootfiles><rootfile full-path=\"OEBPS/content.opf\" media-type=\"application/oebps-package+xml\"/></rootfiles>\n\
          </container>\n",
    )?;
    zip.start_file("OEBPS/style.css", deflated)?;
    zip.write_all(STYLE.as_bytes())?;

    let language = escape(&book.language);
    let mut manifest = String::from(
        "<item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n\
         <item id=\"ncx\" href=\"toc.ncx\" media-type=\"application/x-dtbncx+xml\"/>\n\
         <item id=\"style\" href=\"style.css\" media-type=\"text/css\"/>\n",
    );
    let mut spine = String::new();
    let mut nav = String::new();
    let mut ncx = String::new();
    let mut cover_meta = String::new();

    if let Some((bytes, extension)) = &book.cover {
        let name = format!("cover.{extension}");
        zip.start_file(format!("OEBPS/{name}"), deflated)?;
        zip.write_all(bytes)?;
        // The cover is in the metadata only: a cover page in the spine reads as a chapter in
        // some readers (Raygun's own import among them).
        manifest.push_str(&format!(
            "<item id=\"cover-image\" href=\"{name}\" media-type=\"{}\" properties=\"cover-image\"/>\n",
            image_mime(extension)
        ));
        cover_meta.push_str("<meta name=\"cover\" content=\"cover-image\"/>\n");
    }

    for (index, chapter) in book.chapters.iter().enumerate() {
        let number = index + 1;
        let href = format!("chapters/{number:04}.xhtml");
        zip.start_file(format!("OEBPS/{href}"), deflated)?;
        zip.write_all(chapter_xhtml(chapter, &language).as_bytes())?;
        let properties = if has_math(chapter) {
            " properties=\"mathml\""
        } else {
            ""
        };
        manifest.push_str(&format!(
            "<item id=\"c{number}\" href=\"{href}\" media-type=\"application/xhtml+xml\"{properties}/>\n"
        ));
        spine.push_str(&format!("<itemref idref=\"c{number}\"/>\n"));
        nav.push_str(&format!(
            "<li><a href=\"{href}\">{}</a></li>\n",
            escape(&chapter.title)
        ));
        ncx.push_str(&format!(
            "<navPoint id=\"n{number}\" playOrder=\"{number}\"><navLabel><text>{}</text></navLabel>\
             <content src=\"{href}\"/></navPoint>\n",
            escape(&chapter.title)
        ));
    }

    let mut metadata = format!(
        "<dc:identifier id=\"book-id\">{}</dc:identifier>\n<dc:title>{}</dc:title>\n\
         <dc:language>{language}</dc:language>\n\
         <meta property=\"dcterms:modified\">{}</meta>\n",
        escape(&book.id),
        escape(&book.title),
        escape(&book.modified)
    );
    for author in &book.authors {
        metadata.push_str(&format!("<dc:creator>{}</dc:creator>\n", escape(author)));
    }
    if let Some(description) = &book.description {
        metadata.push_str(&format!(
            "<dc:description>{}</dc:description>\n",
            escape(description)
        ));
    }
    for genre in &book.genres {
        metadata.push_str(&format!("<dc:subject>{}</dc:subject>\n", escape(genre)));
    }
    metadata.push_str(&cover_meta);

    zip.start_file("OEBPS/content.opf", deflated)?;
    zip.write_all(
        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
             <package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"book-id\" xml:lang=\"{language}\">\n\
             <metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n{metadata}</metadata>\n\
             <manifest>\n{manifest}</manifest>\n<spine toc=\"ncx\">\n{spine}</spine>\n</package>\n"
        )
        .as_bytes(),
    )?;
    zip.start_file("OEBPS/nav.xhtml", deflated)?;
    zip.write_all(
        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE html>\n\
             <html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" xml:lang=\"{language}\">\n\
             <head><title>{}</title></head>\n<body>\n<nav epub:type=\"toc\" id=\"toc\"><ol>\n{nav}</ol></nav>\n</body>\n</html>\n",
            escape(&book.title)
        )
        .as_bytes(),
    )?;
    zip.start_file("OEBPS/toc.ncx", deflated)?;
    zip.write_all(
        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
             <ncx xmlns=\"http://www.daisy.org/z3986/2005/ncx/\" version=\"2005-1\">\n\
             <head><meta name=\"dtb:uid\" content=\"{}\"/></head>\n\
             <docTitle><text>{}</text></docTitle>\n<navMap>\n{ncx}</navMap>\n</ncx>\n",
            escape(&book.id),
            escape(&book.title)
        )
        .as_bytes(),
    )?;
    Ok(())
}

/// Writes `book` as an EPUB into `file`.
pub fn write_epub(file: &mut File, book: &Book) -> std::io::Result<()> {
    if super::output::seekable(file) {
        let mut zip = ZipWriter::new(&mut *file);
        write_into(&mut zip, book)?;
        zip.finish()?;
    } else {
        let mut zip = ZipWriter::new_stream(&mut *file);
        write_into(&mut zip, book)?;
        zip.finish()?;
    }
    Ok(())
}

/// The book as plain UTF-8 text: the title, then each chapter under its name.
pub fn text(book: &Book) -> String {
    let mut out = format!("{}\n", book.title);
    if !book.authors.is_empty() {
        out.push_str(&format!("{}\n", book.authors.join(", ")));
    }
    for chapter in &book.chapters {
        out.push_str(&format!("\n\n{}\n\n", chapter.title));
        for block in body_blocks(chapter) {
            match block {
                TextBlock::Paragraph { text, .. } => out.push_str(&format!("{text}\n\n")),
                TextBlock::Heading { text } => out.push_str(&format!("{text}\n\n")),
                TextBlock::Separator => out.push_str("* * *\n\n"),
                TextBlock::Image { .. } => {}
                TextBlock::Math { text, .. } => out.push_str(&format!("{text}\n\n")),
            }
        }
    }
    out
}
