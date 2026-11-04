//! EPUB books from OPDS catalogues, read as a novel source reads chapters: the spine's documents
//! in reading order, named by the table of contents (EPUB 3's `nav`, else EPUB 2's NCX), each a
//! passage of HTML. Pictures stay out: they live inside the book, where no address reaches.

use std::collections::HashMap;
use std::io::{Cursor, Read};

use quick_xml::Reader;
use quick_xml::events::Event;

/// Largest document read out of a book.
const MAX_DOCUMENT_BYTES: u64 = 16 * 1024 * 1024;
/// A spine document with less text than this and no entry in the contents is a cover or a
/// title page, not a chapter.
const MIN_UNLISTED_TEXT: usize = 200;

/// One chapter: a spine document and its title in the contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Chapter {
    /// The document's path in the book.
    pub path: String,
    pub title: Option<String>,
}

/// A book's chapters, in reading order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Book {
    pub title: Option<String>,
    pub chapters: Vec<Chapter>,
}

struct Archive {
    zip: zip::ZipArchive<Cursor<Vec<u8>>>,
}

impl Archive {
    fn open(bytes: Vec<u8>) -> Result<Self, String> {
        zip::ZipArchive::new(Cursor::new(bytes))
            .map(|zip| Self { zip })
            .map_err(|error| format!("the book isn't a readable EPUB: {error}"))
    }

    fn text(&mut self, path: &str) -> Result<String, String> {
        let mut file = self
            .zip
            .by_name(path)
            .map_err(|_| format!("the book has no {path}"))?;
        let mut bytes = Vec::new();
        (&mut file)
            .take(MAX_DOCUMENT_BYTES)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("couldn't read {path} in the book: {error}"))?;
        Ok(String::from_utf8_lossy(&bytes)
            .trim_start_matches('\u{feff}')
            .to_owned())
    }
}

/// `href` (percent-encoded, relative to the document at `from`) as a path in the book.
fn join(from: &str, href: &str) -> String {
    let href = href.split('#').next().unwrap_or_default();
    let decoded: String =
        url::form_urlencoded::parse(format!("x={}", href.replace('+', "%2B")).as_bytes())
            .next()
            .map(|(_, value)| value.into_owned())
            .unwrap_or_else(|| href.to_owned());
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    for part in decoded.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    parts.retain(|part| !part.is_empty());
    parts.join("/")
}

/// Every element's attributes, as `(local name, attributes)` in document order.
fn elements(xml: &str) -> Vec<(String, HashMap<String, String>, String)> {
    let mut reader = Reader::from_str(xml);
    let mut buffer = Vec::new();
    let mut found: Vec<(String, HashMap<String, String>, String)> = Vec::new();
    let mut open: Vec<usize> = Vec::new();
    while let Ok(event) = reader.read_event_into(&mut buffer) {
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(element) | Event::Empty(element) => {
                let name = element.name().as_ref().to_ascii_lowercase();
                let local = name.rsplit(':').next().unwrap_or_default().to_owned();
                let attributes = element
                    .attributes()
                    .flatten()
                    .filter_map(|attribute| {
                        let key = attribute.key.as_ref().to_ascii_lowercase();
                        let value = attribute
                            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                            .ok()?
                            .into_owned();
                        Some((key, value))
                    })
                    .collect();
                found.push((local, attributes, String::new()));
                if !empty {
                    open.push(found.len() - 1);
                }
            }
            Event::Text(content) => {
                if let Some(&index) = open.last() {
                    found[index].2.push_str(&content);
                }
            }
            Event::CData(content) => {
                if let Some(&index) = open.last() {
                    found[index].2.push_str(&content);
                }
            }
            Event::End(_) => {
                open.pop();
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    found
}

/// The OPF package's path, from `META-INF/container.xml`.
fn package_path(archive: &mut Archive) -> Result<String, String> {
    let container = archive.text("META-INF/container.xml")?;
    elements(&container)
        .into_iter()
        .find(|(name, _, _)| name == "rootfile")
        .and_then(|(_, attributes, _)| attributes.get("full-path").cloned())
        .ok_or_else(|| "the book names no package document".to_owned())
}

/// Titles by document path, from the contents: EPUB 3's `nav`, else the NCX.
fn contents(
    archive: &mut Archive,
    nav: Option<String>,
    ncx: Option<String>,
) -> HashMap<String, String> {
    let mut titles = HashMap::new();
    if let Some(path) = nav
        && let Ok(text) = archive.text(&path)
    {
        let document = scraper::Html::parse_document(&text);
        let navs = scraper::Selector::parse("nav").expect("valid selector");
        let links = scraper::Selector::parse("a[href]").expect("valid selector");
        let mut found: Vec<_> = document.select(&navs).collect();
        // The table of contents, not the landmarks or the page list.
        found.sort_by_key(|nav| nav.value().attr("epub:type") != Some("toc"));
        if let Some(toc) = found.first() {
            for link in toc.select(&links) {
                let title = link.text().collect::<Vec<_>>().join(" ");
                let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
                if let Some(href) = link.value().attr("href")
                    && !title.is_empty()
                {
                    titles.entry(join(&path, href)).or_insert(title);
                }
            }
        }
    }
    if titles.is_empty()
        && let Some(path) = ncx
        && let Ok(text) = archive.text(&path)
    {
        let mut label: Option<String> = None;
        for (name, attributes, text) in elements(&text) {
            match name.as_str() {
                "text" => label = Some(text.split_whitespace().collect::<Vec<_>>().join(" ")),
                "content" => {
                    if let (Some(title), Some(src)) = (label.take(), attributes.get("src"))
                        && !title.is_empty()
                    {
                        titles.entry(join(&path, src)).or_insert(title);
                    }
                }
                _ => {}
            }
        }
    }
    titles
}

/// The visible text of a document, for telling chapters from covers.
fn text_length(html: &str) -> usize {
    let document = scraper::Html::parse_document(html);
    let body = scraper::Selector::parse("body").expect("valid selector");
    document
        .select(&body)
        .next()
        .map(|body| body.text().map(|text| text.trim().len()).sum())
        .unwrap_or_default()
}

/// A book's chapters.
pub(crate) fn read(bytes: Vec<u8>) -> Result<Book, String> {
    let mut archive = Archive::open(bytes)?;
    let opf_path = package_path(&mut archive)?;
    let opf = archive.text(&opf_path)?;
    let parsed = elements(&opf);
    let mut items: HashMap<String, (String, String, String)> = HashMap::new();
    let mut spine: Vec<(String, bool)> = Vec::new();
    let mut toc_id = None;
    let mut title = None;
    for (name, attributes, text) in &parsed {
        match name.as_str() {
            "item" => {
                if let (Some(id), Some(href)) = (attributes.get("id"), attributes.get("href")) {
                    items.insert(
                        id.clone(),
                        (
                            join(&opf_path, href),
                            attributes.get("media-type").cloned().unwrap_or_default(),
                            attributes.get("properties").cloned().unwrap_or_default(),
                        ),
                    );
                }
            }
            "spine" => toc_id = attributes.get("toc").cloned(),
            "itemref" => {
                if let Some(id) = attributes.get("idref") {
                    let linear = attributes.get("linear").map(String::as_str) != Some("no");
                    spine.push((id.clone(), linear));
                }
            }
            "title" if title.is_none() && !text.trim().is_empty() => {
                title = Some(text.trim().to_owned())
            }
            _ => {}
        }
    }
    let nav = items
        .values()
        .find(|(_, _, properties)| {
            properties
                .split_whitespace()
                .any(|property| property == "nav")
        })
        .map(|(path, _, _)| path.clone());
    let ncx = toc_id
        .and_then(|id| items.get(&id).map(|(path, _, _)| path.clone()))
        .or_else(|| {
            items
                .values()
                .find(|(_, kind, _)| kind == "application/x-dtbncx+xml")
                .map(|(path, _, _)| path.clone())
        });
    let titles = contents(&mut archive, nav.clone(), ncx);
    let mut chapters = Vec::new();
    for (id, linear) in spine {
        let Some((path, kind, _)) = items.get(&id) else {
            continue;
        };
        let document = kind.contains("html") || kind.is_empty();
        if !linear || !document || Some(path) == nav.as_ref() {
            continue;
        }
        let title = titles.get(path).cloned();
        if title.is_none() {
            let text = archive.text(path).unwrap_or_default();
            if text_length(&text) < MIN_UNLISTED_TEXT {
                continue;
            }
        }
        chapters.push(Chapter {
            path: path.clone(),
            title,
        });
    }
    if chapters.is_empty() {
        return Err("the book has no chapters Raygun can read".into());
    }
    Ok(Book { title, chapters })
}

/// A chapter's document, as the HTML inside its body.
pub(crate) fn passage(bytes: Vec<u8>, path: &str) -> Result<String, String> {
    let mut archive = Archive::open(bytes)?;
    let text = archive.text(path)?;
    let document = scraper::Html::parse_document(&text);
    let body = scraper::Selector::parse("body").expect("valid selector");
    Ok(document
        .select(&body)
        .next()
        .map_or(text.clone(), |body| body.inner_html()))
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::Write;

    use super::*;

    /// A small EPUB 3 book: a cover page, two chapters in the contents, and one unlisted
    /// document long enough to be a chapter.
    pub(crate) fn sample_book() -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut bytes));
            let options = zip::write::SimpleFileOptions::default();
            let mut add = |path: &str, text: &str| {
                zip.start_file(path, options).unwrap();
                zip.write_all(text.as_bytes()).unwrap();
            };
            add("mimetype", "application/epub+zip");
            add(
                "META-INF/container.xml",
                r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#,
            );
            add(
                "OEBPS/content.opf",
                r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>The Time Machine</dc:title></metadata>
<manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="cover" href="text/cover.xhtml" media-type="application/xhtml+xml"/><item id="c1" href="text/chapter%201.xhtml" media-type="application/xhtml+xml"/><item id="c2" href="text/c2.xhtml" media-type="application/xhtml+xml"/><item id="c3" href="text/c3.xhtml" media-type="application/xhtml+xml"/><item id="css" href="style.css" media-type="text/css"/></manifest>
<spine><itemref idref="cover"/><itemref idref="c1"/><itemref idref="c2"/><itemref idref="c3"/></spine></package>"#,
            );
            add(
                "OEBPS/nav.xhtml",
                r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="landmarks"><ol><li><a href="text/cover.xhtml">Cover</a></li></ol></nav><nav epub:type="toc"><ol><li><a href="text/chapter%201.xhtml">I. Introduction</a></li><li><a href="text/c2.xhtml#start">II.
                The Machine</a></li></ol></nav></body></html>"#,
            );
            add(
                "OEBPS/text/cover.xhtml",
                r#"<html><body><img src="../images/cover.jpg"/></body></html>"#,
            );
            add(
                "OEBPS/text/chapter 1.xhtml",
                r#"<html><head><title>I</title></head><body><h2>I</h2><p>The Time Traveller was expounding a recondite matter to us.</p><img src="../images/fig.png"/></body></html>"#,
            );
            add(
                "OEBPS/text/c2.xhtml",
                r#"<html><body><h2>II</h2><p>I think that at that time none of us quite believed in the Time Machine.</p></body></html>"#,
            );
            add(
                "OEBPS/text/c3.xhtml",
                &format!(
                    "<html><body><p>{}</p></body></html>",
                    "The next Thursday I went again to Richmond. ".repeat(10)
                ),
            );
            zip.finish().unwrap();
        }
        bytes
    }

    #[test]
    fn chapters_follow_the_spine_named_by_the_contents() {
        let book = read(sample_book()).unwrap();
        assert_eq!(book.title.as_deref(), Some("The Time Machine"));
        let chapters: Vec<(&str, Option<&str>)> = book
            .chapters
            .iter()
            .map(|chapter| (chapter.path.as_str(), chapter.title.as_deref()))
            .collect();
        assert_eq!(
            chapters,
            [
                ("OEBPS/text/chapter 1.xhtml", Some("I. Introduction")),
                ("OEBPS/text/c2.xhtml", Some("II. The Machine")),
                ("OEBPS/text/c3.xhtml", None),
            ],
            "the cover page isn't a chapter; an unlisted long document is"
        );
        let html = passage(sample_book(), "OEBPS/text/chapter 1.xhtml").unwrap();
        assert!(html.contains("<p>The Time Traveller was expounding"));
        assert!(!html.contains("<title>"));
        let blocks = crate::novel::html_blocks(&html, None);
        assert!(
            !blocks
                .iter()
                .any(|block| matches!(block, crate::novel::PassageBlock::Image(_))),
            "pictures inside the book stay out"
        );
    }

    #[test]
    fn paths_resolve_inside_the_book() {
        assert_eq!(
            join("OEBPS/content.opf", "text/a%20b.xhtml#x"),
            "OEBPS/text/a b.xhtml"
        );
        assert_eq!(join("OEBPS/text/nav.xhtml", "../c.xhtml"), "OEBPS/c.xhtml");
        assert_eq!(join("content.opf", "c.xhtml"), "c.xhtml");
        assert!(read(b"not a zip".to_vec()).is_err());
    }
}
