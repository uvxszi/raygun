//! EPUB books (M33): reading the package (metadata, cover, spine, table of contents) and
//! unpacking the files where the stream proxy's `/local` route can serve them. Parsing is lenient
//! (an HTML parser reads the XML), as real-world books often aren't well-formed.

use std::collections::{BTreeMap, HashMap};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use ego_tree::NodeRef;
use scraper::{Html, Node};

/// Largest file accepted, and most bytes and entries unpacked from one.
pub const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;
const MAX_UNPACKED_BYTES: u64 = 100 * 1024 * 1024;
const MAX_ENTRIES: usize = 5_000;

/// A book's files, by their path inside it.
pub struct Epub {
    files: BTreeMap<String, Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Book {
    pub title: String,
    pub authors: Vec<String>,
    pub description: Option<String>,
    pub genres: Vec<String>,
    /// Path of the cover image in the book.
    pub cover: Option<String>,
    pub chapters: Vec<Chapter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chapter {
    /// Path of the chapter's XHTML in the book, or `path#start#end` for one of several chapters
    /// in a file (between two anchors; either may be empty). See [`split_id`].
    pub path: String,
    pub name: String,
}

impl Epub {
    pub fn open(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_FILE_BYTES {
            return Err("This book is larger than 64 MB".into());
        }
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
            .map_err(|_| "This isn't an EPUB file".to_owned())?;
        if archive.len() > MAX_ENTRIES {
            return Err("This book has too many files".into());
        }
        let mut files = BTreeMap::new();
        let mut total = 0u64;
        for index in 0..archive.len() {
            let mut file = archive
                .by_index(index)
                .map_err(|_| "This EPUB is damaged".to_owned())?;
            if file.is_dir() || file.is_symlink() {
                continue;
            }
            let Some(path) = file.name().ok().and_then(|name| normalize(&name)) else {
                continue;
            };
            let mut contents = Vec::new();
            (&mut file)
                .take(MAX_UNPACKED_BYTES - total + 1)
                .read_to_end(&mut contents)
                .map_err(|_| "This EPUB is damaged".to_owned())?;
            total += contents.len() as u64;
            if total > MAX_UNPACKED_BYTES {
                return Err("This book unpacks to more than 100 MB".into());
            }
            files.insert(path, contents);
        }
        Ok(Self { files })
    }

    fn text(&self, path: &str) -> Option<String> {
        self.files
            .get(path)
            .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
    }

    /// The package's metadata, cover and chapters in reading order.
    pub fn book(&self) -> Result<Book, String> {
        let container = self
            .text("META-INF/container.xml")
            .ok_or("This EPUB has no container.xml")?;
        let container = Html::parse_document(&container);
        let opf_path = elements(&container, "rootfile")
            .find_map(|element| attr(element, "full-path"))
            .and_then(|path| normalize(&path))
            .ok_or("This EPUB names no package file")?;
        let opf_text = self
            .text(&opf_path)
            .ok_or("This EPUB's package file is missing")?;
        let opf = Html::parse_document(&opf_text);
        let base = parent(&opf_path);

        let first_text = |name: &str| {
            elements(&opf, name)
                .map(|element| text(element))
                .find(|text| !text.is_empty())
        };
        let title = first_text("dc:title").unwrap_or_else(|| "Untitled".into());
        let authors: Vec<String> = elements(&opf, "dc:creator")
            .map(text)
            .filter(|author| !author.is_empty())
            .collect();
        let description = first_text("dc:description").map(|html| plain(&html));
        let mut genres: Vec<String> = Vec::new();
        for subject in elements(&opf, "dc:subject").map(text) {
            let subject = subject
                .split(" -- ")
                .next()
                .unwrap_or_default()
                .trim()
                .to_owned();
            if !subject.is_empty() && !genres.contains(&subject) {
                genres.push(subject);
            }
        }

        // Manifest: id → (path, media type, properties).
        let mut manifest: HashMap<String, (String, String, String)> = HashMap::new();
        for item in elements(&opf, "item") {
            let (Some(id), Some(href)) = (attr(item, "id"), attr(item, "href")) else {
                continue;
            };
            let Some(path) = resolve(&base, &href) else {
                continue;
            };
            manifest.insert(
                id,
                (
                    path,
                    attr(item, "media-type").unwrap_or_default(),
                    attr(item, "properties").unwrap_or_default(),
                ),
            );
        }
        let cover = manifest
            .values()
            .find(|(_, _, properties)| properties.split_whitespace().any(|p| p == "cover-image"))
            .map(|(path, _, _)| path.clone())
            .or_else(|| {
                let id = elements(&opf, "meta")
                    .find(|meta| attr(*meta, "name").as_deref() == Some("cover"))
                    .and_then(|meta| attr(meta, "content"))?;
                manifest.get(&id).map(|(path, _, _)| path.clone())
            })
            .filter(|path| self.files.contains_key(path));

        let toc = self.table_of_contents(&manifest, &opf);
        let mut chapters = Vec::new();
        for itemref in elements(&opf, "itemref") {
            let Some((path, media, _)) = attr(itemref, "idref").and_then(|id| manifest.get(&id))
            else {
                continue;
            };
            if !media.contains("html") || !self.files.contains_key(path) {
                continue;
            }
            // Entries of this file, in table-of-contents order, one per anchor.
            let mut entries: Vec<&TocEntry> = Vec::new();
            for entry in toc.iter().filter(|entry| entry.path == *path) {
                if !entries.iter().any(|known| known.fragment == entry.fragment) {
                    entries.push(entry);
                }
            }
            let html = self.text(path).unwrap_or_default();
            let splits: Vec<&TocEntry> = entries
                .iter()
                .skip(1)
                .copied()
                .filter(|entry| {
                    entry
                        .fragment
                        .as_deref()
                        .is_some_and(|fragment| anchor(&html, fragment).is_some())
                })
                .collect();
            if splits.is_empty() {
                let name = entries
                    .first()
                    .map(|entry| entry.label.clone())
                    .or_else(|| first_heading(&html))
                    .unwrap_or_else(|| {
                        if has_text(&html) {
                            format!("Chapter {}", chapters.len() + 1)
                        } else if chapters.is_empty() {
                            "Cover".to_owned()
                        } else {
                            "Illustration".to_owned()
                        }
                    });
                chapters.push(Chapter {
                    path: path.clone(),
                    name,
                });
                continue;
            }
            // A file holding several chapters is split at their anchors: `path#start#end`.
            let first_name = entries[0].label.clone();
            let mut starts: Vec<(Option<String>, String)> = vec![(None, first_name)];
            for entry in &splits {
                starts.push((entry.fragment.clone(), entry.label.clone()));
            }
            for (index, (start, name)) in starts.iter().enumerate() {
                let end = starts
                    .get(index + 1)
                    .and_then(|(fragment, _)| fragment.clone());
                chapters.push(Chapter {
                    path: format!(
                        "{path}#{}#{}",
                        start.as_deref().unwrap_or_default(),
                        end.as_deref().unwrap_or_default()
                    ),
                    name: name.clone(),
                });
            }
        }
        if chapters.is_empty() {
            return Err("This book has no chapters".into());
        }
        Ok(Book {
            title,
            authors,
            description,
            genres,
            cover,
            chapters,
        })
    }

    /// The table of contents in order, from the EPUB 3 nav document or else the EPUB 2 NCX.
    fn table_of_contents(
        &self,
        manifest: &HashMap<String, (String, String, String)>,
        opf: &Html,
    ) -> Vec<TocEntry> {
        let mut toc = Vec::new();
        let add = |toc: &mut Vec<TocEntry>, href: &str, label: String, base: &str| {
            let (href, fragment) = match href.split_once('#') {
                Some((href, fragment)) => (href, Some(fragment.to_owned())),
                None => (href, None),
            };
            if let Some(path) = resolve(base, href)
                && !label.is_empty()
            {
                toc.push(TocEntry {
                    path,
                    fragment: fragment.filter(|fragment| !fragment.is_empty()),
                    label,
                });
            }
        };
        let nav = manifest
            .values()
            .find(|(_, _, properties)| properties.split_whitespace().any(|p| p == "nav"));
        if let Some((path, _, _)) = nav
            && let Some(text) = self.text(path)
        {
            let document = Html::parse_document(&text);
            let nav_base = parent(path);
            // The `toc` nav only: the document may also list pages and landmarks.
            let navs: Vec<_> = elements(&document, "nav").collect();
            let chosen = navs
                .iter()
                .find(|nav| {
                    nav.value().as_element().is_some_and(|element| {
                        element.attrs().any(|(name, value)| {
                            name.ends_with("type")
                                && value.split_whitespace().any(|kind| kind == "toc")
                        })
                    })
                })
                .or(navs.first())
                .copied();
            let links = chosen
                .into_iter()
                .flat_map(|nav| nav.descendants())
                .filter(|node| {
                    node.value()
                        .as_element()
                        .is_some_and(|element| element.name() == "a")
                });
            for link in links {
                if let Some(href) = attr(link, "href") {
                    add(&mut toc, &href, text_of(link), &nav_base);
                }
            }
        }
        if toc.is_empty() {
            let ncx = elements(opf, "spine")
                .find_map(|spine| attr(spine, "toc"))
                .and_then(|id| manifest.get(&id))
                .map(|(path, _, _)| path.clone())
                .or_else(|| {
                    manifest
                        .values()
                        .find(|(_, media, _)| media.contains("ncx"))
                        .map(|(path, _, _)| path.clone())
                });
            if let Some(path) = ncx
                && let Some(text) = self.text(&path)
            {
                let document = Html::parse_document(&text);
                let ncx_base = parent(&path);
                // Labels and sources alternate in document order.
                let mut label = None;
                for node in document.tree.root().descendants() {
                    let Some(element) = node.value().as_element() else {
                        continue;
                    };
                    match element.name() {
                        "navlabel" => label = Some(text_of(node)),
                        "content" => {
                            if let (Some(name), Some(src)) = (label.take(), element.attr("src")) {
                                add(&mut toc, src, name, &ncx_base);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        toc
    }

    /// Writes every file under `dir` at its [`safe_path`].
    pub fn unpack(&self, dir: &Path) -> std::io::Result<()> {
        for (path, contents) in &self.files {
            let target = dir.join(safe_path(path));
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(target, contents)?;
        }
        Ok(())
    }
}

struct TocEntry {
    path: String,
    fragment: Option<String>,
    label: String,
}

/// A chapter id as its file and the anchors it starts and ends at.
pub fn split_id(id: &str) -> (&str, Option<&str>, Option<&str>) {
    let mut parts = id.splitn(3, '#');
    let path = parts.next().unwrap_or_default();
    let start = parts.next().filter(|part| !part.is_empty());
    let end = parts.next().filter(|part| !part.is_empty());
    (path, start, end)
}

/// Where the element with `id` starts in `html`.
fn anchor(html: &str, id: &str) -> Option<usize> {
    let at = html
        .find(&format!("id=\"{id}\""))
        .or_else(|| html.find(&format!("id='{id}'")))?;
    html[..at].rfind('<')
}

/// The part of a chapter file between two anchors (the whole file without them).
pub fn slice<'a>(html: &'a str, start: Option<&str>, end: Option<&str>) -> &'a str {
    let from = start.and_then(|id| anchor(html, id)).unwrap_or(0);
    let to = end
        .and_then(|id| anchor(html, id))
        .filter(|to| *to > from)
        .unwrap_or(html.len());
    &html[from..to]
}

/// Images an SVG wrapper page shows (`<image xlink:href>`), which HTML conversion drops.
pub fn svg_images(html: &str) -> Vec<String> {
    let document = Html::parse_document(html);
    // `xlink:href` sits in the XLink namespace, which `attr` doesn't look in.
    elements(&document, "image")
        .filter_map(|image| {
            image
                .value()
                .as_element()?
                .attrs()
                .find(|(name, _)| *name == "href")
                .map(|(_, value)| value.to_owned())
        })
        .collect()
}

fn first_heading(html: &str) -> Option<String> {
    raygun_ext_host::novel::html_blocks(html, None)
        .into_iter()
        .take(8)
        .find_map(|block| match block {
            raygun_ext_host::novel::PassageBlock::Heading(heading) => Some(heading),
            _ => None,
        })
}

fn has_text(html: &str) -> bool {
    raygun_ext_host::novel::html_blocks(html, None)
        .iter()
        .any(|block| matches!(block, raygun_ext_host::novel::PassageBlock::Paragraph(_)))
}

/// Where a book path is unpacked: each segment limited to `[A-Za-z0-9._-]` (other characters
/// become `_`, a leading dot gets an `x` before it), so the proxy can serve it.
pub fn safe_path(path: &str) -> PathBuf {
    path.split('/').map(safe_segment).collect()
}

/// [`safe_path`] as a `/`-joined string, for `/local/` URLs.
pub fn safe_url_path(path: &str) -> String {
    path.split('/')
        .map(safe_segment)
        .collect::<Vec<_>>()
        .join("/")
}

fn safe_segment(segment: &str) -> String {
    let mut safe: String = segment
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if safe.starts_with('.') || safe.is_empty() {
        safe.insert(0, 'x');
    }
    safe
}

/// A zip entry name as a clean relative path; `None` when it would leave the book.
fn normalize(name: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    let name = name.replace('\\', "/");
    for part in name.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn parent(path: &str) -> String {
    path.rsplit_once('/')
        .map(|(dir, _)| format!("{dir}/"))
        .unwrap_or_default()
}

/// A (percent-encoded) href relative to `base`, as a book path.
pub fn resolve(base: &str, href: &str) -> Option<String> {
    if href.contains("://") || href.starts_with('/') && href.starts_with("//") {
        return None;
    }
    let decoded = url::form_urlencoded::parse(format!("x={}", href.replace('+', "%2B")).as_bytes())
        .next()
        .map(|(_, value)| value.into_owned())?;
    normalize(&format!("{base}{decoded}"))
}

fn elements<'a>(document: &'a Html, name: &'a str) -> impl Iterator<Item = NodeRef<'a, Node>> + 'a {
    document.tree.root().descendants().filter(move |node| {
        node.value()
            .as_element()
            .is_some_and(|element| element.name() == name)
    })
}

fn attr(node: NodeRef<'_, Node>, name: &str) -> Option<String> {
    node.value()
        .as_element()
        .and_then(|element| element.attr(name))
        .map(str::to_owned)
}

fn text(node: NodeRef<'_, Node>) -> String {
    text_of(node)
}

fn text_of(node: NodeRef<'_, Node>) -> String {
    let raw: String = node
        .descendants()
        .filter_map(|child| child.value().as_text().map(|text| text.to_string()))
        .collect();
    collapse(&raw)
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Descriptions are sometimes HTML.
fn plain(text: &str) -> String {
    raygun_ext_host::novel::html_blocks(text, None)
        .into_iter()
        .filter_map(|block| match block {
            raygun_ext_host::novel::PassageBlock::Paragraph(text)
            | raygun_ext_host::novel::PassageBlock::Heading(text) => Some(text),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    /// A small EPUB built in memory.
    pub fn sample(nav: bool) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        let mut add = |name: &str, contents: &[u8]| {
            zip.start_file(name, options).unwrap();
            zip.write_all(contents).unwrap();
        };
        add("mimetype", b"application/epub+zip");
        add(
            "META-INF/container.xml",
            br#"<?xml version="1.0"?><container><rootfiles><rootfile full-path="OEBPS/content.opf"/></rootfiles></container>"#,
        );
        let nav_item = if nav {
            r#"<item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>"#
        } else {
            ""
        };
        add(
            "OEBPS/content.opf",
            format!(
                r#"<?xml version="1.0"?>
                <package xmlns:dc="http://purl.org/dc/elements/1.1/" version="3.0">
                  <metadata>
                    <dc:title>The Test &amp; Book</dc:title>
                    <dc:creator>Ada Writer</dc:creator><dc:creator>Bo Helper</dc:creator>
                    <dc:description>&lt;p&gt;A short book.&lt;/p&gt;</dc:description>
                    <dc:subject>Fantasy -- Fiction</dc:subject><dc:subject>Fantasy</dc:subject>
                    <meta name="cover" content="cover"/>
                  </metadata>
                  <manifest>
                    <item id="cover" href="images/cover%20art.jpg" media-type="image/jpeg"/>
                    <item id="c1" href="text/one.xhtml" media-type="application/xhtml+xml"/>
                    <item id="c2" href="text/two.xhtml" media-type="application/xhtml+xml"/>
                    <item id="c3" href="text/three.xhtml" media-type="application/xhtml+xml"/>
                    <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
                    {nav_item}
                  </manifest>
                  <spine toc="ncx"><itemref idref="c2"/><itemref idref="c1"/><itemref idref="c3"/></spine>
                </package>"#
            )
            .as_bytes(),
        );
        add(
            "OEBPS/nav.xhtml",
            br#"<html><body><nav epub:type="toc"><ol>
                <li><a href="text/two.xhtml#start">Prologue (nav)</a></li>
                <li><a href="text/one.xhtml">One (nav)</a></li></ol></nav></body></html>"#,
        );
        add(
            "OEBPS/toc.ncx",
            br#"<ncx><navMap>
                <navPoint><navLabel><text>Prologue</text></navLabel><content src="text/two.xhtml"/></navPoint>
                <navPoint><navLabel><text>Chapter One</text></navLabel><content src="text/one.xhtml#a"/></navPoint>
                <navPoint><navLabel><text>Again</text></navLabel><content src="text/one.xhtml#b"/></navPoint>
            </navMap></ncx>"#,
        );
        add(
            "OEBPS/text/one.xhtml",
            br#"<html><body><h1 id="a">One</h1><p>First text.</p><img src="../images/cover%20art.jpg"/><h2 id="b">Two</h2><p>Second.</p></body></html>"#,
        );
        add(
            "OEBPS/text/two.xhtml",
            b"<html><body><p>Before it all.</p></body></html>",
        );
        add(
            "OEBPS/text/three.xhtml",
            b"<html><body><h2>The End</h2><p>Done.</p></body></html>",
        );
        add("OEBPS/images/cover art.jpg", b"\xFF\xD8\xFFjpeg");
        add("../escape.txt", b"nope");
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn reads_metadata_spine_and_ncx_names() {
        let epub = Epub::open(&sample(false)).unwrap();
        let book = epub.book().unwrap();
        assert_eq!(book.title, "The Test & Book");
        assert_eq!(book.authors, ["Ada Writer", "Bo Helper"]);
        assert_eq!(book.description.as_deref(), Some("A short book."));
        assert_eq!(book.genres, ["Fantasy"]);
        assert_eq!(book.cover.as_deref(), Some("OEBPS/images/cover art.jpg"));
        let chapters: Vec<_> = book
            .chapters
            .iter()
            .map(|chapter| (chapter.path.as_str(), chapter.name.as_str()))
            .collect();
        assert_eq!(
            chapters,
            [
                ("OEBPS/text/two.xhtml", "Prologue"),
                ("OEBPS/text/one.xhtml##b", "Chapter One"),
                ("OEBPS/text/one.xhtml#b#", "Again"),
                ("OEBPS/text/three.xhtml", "The End"),
            ],
            "spine order; NCX names; a file with two entries is split at its anchors"
        );
        let one = epub.text("OEBPS/text/one.xhtml").unwrap();
        let (path, start, end) = split_id("OEBPS/text/one.xhtml#b#");
        assert_eq!(
            (path, start, end),
            ("OEBPS/text/one.xhtml", Some("b"), None)
        );
        assert!(slice(&one, start, end).starts_with("<h2 id=\"b\">Two"));
        assert!(!slice(&one, None, Some("b")).contains("Second."));
        assert_eq!(split_id("a.xhtml"), ("a.xhtml", None, None));
        assert_eq!(
            svg_images(r#"<svg><image xlink:href="cover.jpg"/></svg>"#),
            ["cover.jpg"]
        );
        assert!(
            !epub.files.keys().any(|path| path.contains("escape")),
            "paths stay inside"
        );
    }

    #[test]
    fn prefers_the_epub3_nav() {
        let book = Epub::open(&sample(true)).unwrap().book().unwrap();
        assert_eq!(book.chapters[0].name, "Prologue (nav)");
        assert_eq!(book.chapters[1].name, "One (nav)");
        assert_eq!(book.chapters.len(), 3, "one entry per file: no split");
    }

    #[test]
    fn unpacks_to_servable_paths() {
        let epub = Epub::open(&sample(false)).unwrap();
        let temp = tempfile::TempDir::new().unwrap();
        epub.unpack(temp.path()).unwrap();
        assert!(temp.path().join("OEBPS/images/cover_art.jpg").exists());
        assert_eq!(
            safe_url_path("OEBPS/images/cover art.jpg"),
            "OEBPS/images/cover_art.jpg"
        );
        assert_eq!(safe_url_path(".hidden/x y"), "x.hidden/x_y");
        assert_eq!(
            resolve("OEBPS/text/", "../images/cover%20art.jpg").as_deref(),
            Some("OEBPS/images/cover art.jpg")
        );
        assert_eq!(resolve("OEBPS/", "../../etc/passwd"), None);
        assert_eq!(resolve("OEBPS/", "https://example.com/x.jpg"), None);
    }

    #[test]
    fn rejects_what_isnt_a_book() {
        assert!(Epub::open(b"not a zip").is_err());
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file("hello.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"hi").unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        assert!(Epub::open(&bytes).unwrap().book().is_err());
    }
}

/// `RAYGUN_EPUB=<file> cargo test -p raygun-app -- --ignored real_epub --nocapture`.
#[cfg(test)]
#[test]
#[ignore = "needs an EPUB file named by RAYGUN_EPUB"]
fn real_epub() {
    let path = std::env::var("RAYGUN_EPUB").unwrap();
    let epub = Epub::open(&std::fs::read(path).unwrap()).unwrap();
    let book = epub.book().unwrap();
    eprintln!(
        "{} by {:?}; cover {:?}; genres {:?}",
        book.title, book.authors, book.cover, book.genres
    );
    for chapter in book.chapters.iter().take(6) {
        eprintln!("  {} — {}", chapter.name, chapter.path);
    }
    eprintln!("  … {} chapters", book.chapters.len());
}
