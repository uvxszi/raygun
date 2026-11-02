//! Local comics (M46, Mihon's local source): CBZ and ZIP archives picked on the device become
//! chapters of a library manga with source `local.comics`. An archive is unpacked once into
//! `downloads/comics/<series>/<archive sha-256>/` as numbered images served by the proxy's
//! `/local/` route; ComicInfo.xml names the series, chapter and authors when it's there, the file
//! name otherwise. Archives of the same series join one title.

use std::cmp::Ordering;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use crate::catalog::to_message;

pub const LOCAL_COMICS: &str = "local.comics";
/// The folder under the downloads folder that holds unpacked comics.
pub const COMICS: &str = "comics";
/// Most entries an archive may have, and the most pages taken from it.
const MAX_ENTRIES: usize = 10_000;
const MAX_PAGES: usize = 2_000;
/// The largest single page unpacked.
const MAX_PAGE_BYTES: u64 = 64 * 1024 * 1024;
const IMAGE_EXTENSIONS: [&str; 6] = ["jpg", "jpeg", "png", "webp", "gif", "avif"];

/// What ComicInfo.xml says (the ComicRack schema), all optional.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ComicInfo {
    pub series: Option<String>,
    pub number: Option<f64>,
    pub title: Option<String>,
    pub writer: Option<String>,
    pub penciller: Option<String>,
    pub summary: Option<String>,
    pub genre: Option<String>,
}

impl ComicInfo {
    /// Reads the flat elements of a ComicInfo.xml.
    pub fn parse(xml: &str) -> Self {
        let field = |name: &str| -> Option<String> {
            let open = format!("<{name}>");
            let start = xml.find(&open)? + open.len();
            let end = start + xml[start..].find(&format!("</{name}>"))?;
            let value = unescape(xml[start..end].trim());
            (!value.is_empty()).then_some(value)
        };
        Self {
            series: field("Series"),
            number: field("Number").and_then(|number| number.parse().ok()),
            title: field("Title"),
            writer: field("Writer"),
            penciller: field("Penciller"),
            summary: field("Summary"),
            genre: field("Genre"),
        }
    }
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// A comic file's series name and chapter number from its name alone: "Berserk v01.cbz" is
/// volume 1 of "Berserk", "One Piece - Chapter 1001.5 [Group].cbz" chapter 1001.5 of "One Piece".
pub fn series_and_number(file_name: &str) -> (String, Option<f64>) {
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(file_name);
    // Drop bracketed groups: "[Scanlator]", "(2004)", "{digital}".
    let mut cleaned = String::new();
    let mut depth = 0usize;
    for c in stem.chars() {
        match c {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => cleaned.push(if c == '_' { ' ' } else { c }),
            _ => {}
        }
    }
    let lower = cleaned.to_lowercase();
    // Where the numbering starts: a chapter or volume marker, else the last number.
    let markers = [
        " chapter", " chap", " ch.", " ch", " c", " vol.", " vol", " v", " #", " - ",
    ];
    let mut cut = None;
    for marker in markers {
        let mut from = 0;
        while let Some(found) = lower[from..].find(marker) {
            let at = from + found;
            let rest = lower[at + marker.len()..].trim_start_matches([' ', '.', '#']);
            if rest.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                cut = Some(cut.map_or(at, |known: usize| known.min(at)));
                break;
            }
            from = at + marker.len();
        }
    }
    let (name, numbers) = match cut {
        Some(at) => (cleaned[..at].to_owned(), cleaned[at..].to_owned()),
        None => {
            // "Series 12": the trailing number.
            let trimmed = cleaned.trim_end();
            let digits = trimmed
                .trim_end_matches(|c: char| c.is_ascii_digit() || c == '.')
                .len();
            if digits < trimmed.len() && digits > 0 {
                (trimmed[..digits].to_owned(), trimmed[digits..].to_owned())
            } else {
                (cleaned.clone(), String::new())
            }
        }
    };
    let number = first_number(&numbers);
    let name = name
        .trim()
        .trim_end_matches(['-', ':', ','])
        .trim()
        .to_owned();
    if name.is_empty() {
        (stem.trim().to_owned(), number)
    } else {
        (name, number)
    }
}

/// The first number in a text, decimals included ("v01 c003.5" gives 1).
fn first_number(text: &str) -> Option<f64> {
    let start = text.find(|c: char| c.is_ascii_digit())?;
    let digits: String = text[start..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    digits.trim_end_matches('.').parse().ok()
}

/// Orders names as people do: "page2" before "page10".
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut left = a.chars().peekable();
    let mut right = b.chars().peekable();
    loop {
        match (left.peek().copied(), right.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let take = |chars: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut number = String::new();
                    while let Some(c) = chars.peek().copied().filter(char::is_ascii_digit) {
                        number.push(c);
                        chars.next();
                    }
                    number
                };
                let (x, y) = (take(&mut left), take(&mut right));
                let (x_trim, y_trim) = (x.trim_start_matches('0'), y.trim_start_matches('0'));
                let order = x_trim
                    .len()
                    .cmp(&y_trim.len())
                    .then_with(|| x_trim.cmp(y_trim));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(x), Some(y)) => {
                let order = x.to_ascii_lowercase().cmp(&y.to_ascii_lowercase());
                if order != Ordering::Equal {
                    return order;
                }
                left.next();
                right.next();
            }
        }
    }
}

/// What an archive holds once unpacked.
#[derive(Debug, Clone, PartialEq)]
pub struct Unpacked {
    /// Its SHA-256, the chapter's id.
    pub hash: String,
    pub series: String,
    pub number: Option<f64>,
    /// The chapter's name.
    pub name: String,
    pub info: ComicInfo,
    pub pages: usize,
}

/// Unpacks an archive's images, in natural order, into `<comics dir>/<series key>/<hash>/` as
/// `0001.jpg`, `0002.png`… (through a `.part` folder, so a failed unpack leaves nothing).
/// `series_key` picks the folder once the series is known.
pub fn unpack(
    archive_path: &Path,
    file_name: &str,
    comics_dir: &Path,
    series_key: impl Fn(&str) -> String,
) -> Result<(Unpacked, PathBuf), String> {
    let bytes = std::fs::read(archive_path).map_err(to_message)?;
    let hash = raygun_ext_host::package::sha256_hex(&bytes);
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| "This isn't a CBZ or ZIP file".to_owned())?;
    if archive.len() > MAX_ENTRIES {
        return Err("This archive has too many files".into());
    }
    let mut images = Vec::new();
    let mut info = ComicInfo::default();
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|_| "This archive is damaged".to_owned())?;
        if file.is_dir() || file.is_symlink() {
            continue;
        }
        let Ok(name) = file.name().map(|name| name.into_owned()) else {
            continue;
        };
        let base = name.rsplit('/').next().unwrap_or(&name);
        if name.starts_with("__MACOSX/") || base.starts_with('.') {
            continue;
        }
        if base.eq_ignore_ascii_case("ComicInfo.xml") {
            let mut xml = String::new();
            if file
                .by_ref()
                .take(1024 * 1024)
                .read_to_string(&mut xml)
                .is_ok()
            {
                info = ComicInfo::parse(&xml);
            }
            continue;
        }
        let extension = base
            .rsplit('.')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if IMAGE_EXTENSIONS.contains(&extension.as_str()) && file.size() <= MAX_PAGE_BYTES {
            images.push((name, index, extension));
        }
    }
    if images.is_empty() {
        return Err("There are no pages in this archive".into());
    }
    images.sort_by(|a, b| natural_cmp(&a.0, &b.0));
    images.truncate(MAX_PAGES);

    let (file_series, file_number) = series_and_number(file_name);
    let series = info.series.clone().unwrap_or(file_series);
    let number = info.number.or(file_number);
    let name = info.title.clone().unwrap_or_else(|| {
        Path::new(file_name)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(file_name)
            .to_owned()
    });

    let dir = comics_dir.join(series_key(&series));
    let target = dir.join(&hash);
    let partial = dir.join(format!("{hash}.part"));
    let _ = std::fs::remove_dir_all(&partial);
    std::fs::create_dir_all(&partial).map_err(to_message)?;
    for (page, (_, index, extension)) in images.iter().enumerate() {
        let mut file = archive
            .by_index(*index)
            .map_err(|_| "This archive is damaged".to_owned())?;
        let mut out = std::fs::File::create(partial.join(format!("{:04}.{extension}", page + 1)))
            .map_err(to_message)?;
        std::io::copy(&mut file.by_ref().take(MAX_PAGE_BYTES), &mut out).map_err(to_message)?;
    }
    let _ = std::fs::remove_dir_all(&target);
    std::fs::rename(&partial, &target).map_err(to_message)?;
    Ok((
        Unpacked {
            hash,
            series,
            number,
            name,
            info,
            pages: images.len(),
        },
        target,
    ))
}

/// An unpacked chapter's page files, in order.
pub fn pages(dir: &Path) -> std::io::Result<Vec<String>> {
    let mut names: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    names.sort_by(|a, b| natural_cmp(a, b));
    Ok(names)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn file_names_give_series_and_number() {
        let cases = [
            ("Berserk v01.cbz", "Berserk", Some(1.0)),
            (
                "One Piece - Chapter 1001.5 [Group].cbz",
                "One Piece",
                Some(1001.5),
            ),
            ("Akira_Vol.3.zip", "Akira", Some(3.0)),
            ("Saga #12 (2013) (Digital).cbz", "Saga", Some(12.0)),
            ("Monster c004.cbz", "Monster", Some(4.0)),
            ("Blame 07.cbz", "Blame", Some(7.0)),
            ("Oneshot.cbz", "Oneshot", None),
            ("1984.cbz", "1984", None),
        ];
        for (file, series, number) in cases {
            assert_eq!(
                series_and_number(file),
                (series.to_owned(), number),
                "{file}"
            );
        }
    }

    #[test]
    fn names_sort_naturally() {
        let mut names = vec!["p10.jpg", "p2.jpg", "P1.jpg", "p02b.jpg", "cover.jpg"];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            names,
            vec!["cover.jpg", "P1.jpg", "p2.jpg", "p02b.jpg", "p10.jpg"]
        );
    }

    #[test]
    fn comic_info_is_read() {
        let info = ComicInfo::parse(
            "<?xml version=\"1.0\"?><ComicInfo><Series>Tom &amp; Jerry</Series>\
             <Number>3.5</Number><Title>The Chase</Title><Writer>Hanna</Writer>\
             <Genre>Comedy, Kids</Genre><Summary> A cat. </Summary></ComicInfo>",
        );
        assert_eq!(info.series.as_deref(), Some("Tom & Jerry"));
        assert_eq!(info.number, Some(3.5));
        assert_eq!(info.title.as_deref(), Some("The Chase"));
        assert_eq!(info.writer.as_deref(), Some("Hanna"));
        assert_eq!(info.summary.as_deref(), Some("A cat."));
        assert_eq!(ComicInfo::parse("<ComicInfo/>"), ComicInfo::default());
    }

    fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, contents) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(contents).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn archives_unpack_their_pages_in_order() {
        let temp = tempfile::TempDir::new().unwrap();
        let file = temp.path().join("upload.part");
        std::fs::write(
            &file,
            archive(&[
                ("Vol 1/page10.png", b"ten"),
                ("Vol 1/page2.jpg", b"two"),
                ("__MACOSX/Vol 1/._page2.jpg", b"junk"),
                ("Vol 1/notes.txt", b"skip"),
                (
                    "ComicInfo.xml",
                    b"<ComicInfo><Series>Test Comic</Series><Number>2</Number></ComicInfo>",
                ),
            ]),
        )
        .unwrap();
        let comics = temp.path().join("comics");
        let (unpacked, dir) = unpack(&file, "whatever v9.cbz", &comics, |series| {
            series.replace(' ', "-")
        })
        .unwrap();
        assert_eq!(
            unpacked.series, "Test Comic",
            "ComicInfo wins over the file name"
        );
        assert_eq!(unpacked.number, Some(2.0));
        assert_eq!(unpacked.pages, 2);
        assert_eq!(dir, comics.join("Test-Comic").join(&unpacked.hash));
        assert_eq!(pages(&dir).unwrap(), vec!["0001.jpg", "0002.png"]);
        assert_eq!(std::fs::read(dir.join("0001.jpg")).unwrap(), b"two");

        std::fs::write(&file, archive(&[("readme.txt", b"no pages")])).unwrap();
        assert!(unpack(&file, "x.cbz", &comics, |s| s.to_owned()).is_err());
    }
}
