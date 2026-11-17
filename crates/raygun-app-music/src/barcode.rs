//! What a scanned barcode is (v2.0.0): an ISBN (EAN-13 from 978 or 979) is looked up on Open
//! Library; any other EAN-13 or UPC-A is looked for as a CD's or record's barcode on MusicBrainz,
//! whose release then finds the music sources' albums. Both are keyless public APIs; MusicBrainz
//! asks for one request a second at most, and a scan makes one.

use std::sync::Arc;

use raygun_model::{BarcodeFind, MusicAlbum};
use serde_json::Value;
use tauri::Manager;

use crate::extensions::ExtensionService;

const MUSICBRAINZ: &str = "https://musicbrainz.org/ws/2/release";
const OPEN_LIBRARY: &str = "https://openlibrary.org/api/books";

/// The digits of a scanned code when it's a valid EAN-13, UPC-A (as EAN-13 with a leading 0) or
/// EAN-8: spaces and dashes dropped, the check digit checked.
pub fn normalize(code: &str) -> Option<String> {
    let digits: String = code
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect();
    if !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let digits = match digits.len() {
        12 => format!("0{digits}"),
        8 | 13 => digits,
        _ => return None,
    };
    check_digit_ok(&digits).then_some(digits)
}

/// The GS1 check: digits weighted 3 and 1 from the right (the check digit itself weighs 1).
fn check_digit_ok(digits: &str) -> bool {
    let sum: u32 = digits
        .chars()
        .rev()
        .enumerate()
        .map(|(at, c)| c.to_digit(10).unwrap_or(0) * if at % 2 == 1 { 3 } else { 1 })
        .sum();
    sum.is_multiple_of(10)
}

/// Whether a normalized code is a book's ISBN-13.
pub fn is_isbn(code: &str) -> bool {
    code.len() == 13 && (code.starts_with("978") || code.starts_with("979"))
}

/// What the code is: a book, an album with the sources' matches, or unknown.
pub async fn lookup(
    app: &tauri::AppHandle,
    client: &reqwest::Client,
    code: &str,
) -> Result<BarcodeFind, String> {
    let code = normalize(code).ok_or("That isn't a barcode Raygun can read (EAN, UPC or ISBN)")?;
    if is_isbn(&code) {
        return book(client, &code).await;
    }
    let Some((title, artist, year)) = release(client, &code).await? else {
        return Ok(BarcodeFind::Unknown { code });
    };
    let query = [artist.as_deref().unwrap_or_default(), &title].join(" ");
    let extensions = app.state::<Arc<ExtensionService>>().inner().clone();
    let found = extensions.search_albums(query.trim(), |_, _, _| {}).await;
    let matches = best_albums(&title, artist.as_deref(), found);
    Ok(BarcodeFind::Album {
        code,
        title,
        artist,
        year,
        matches,
    })
}

/// A release with this barcode on MusicBrainz: its title, credited artist and year.
async fn release(
    client: &reqwest::Client,
    code: &str,
) -> Result<Option<(String, Option<String>, Option<u32>)>, String> {
    // A UPC is kept on MusicBrainz as 12 digits, an EAN as 13: ask for both spellings.
    let short = code.strip_prefix('0').filter(|short| short.len() == 12);
    let query = match short {
        Some(short) => format!("barcode:{code} OR barcode:{short}"),
        None => format!("barcode:{code}"),
    };
    let url = reqwest::Url::parse_with_params(
        MUSICBRAINZ,
        &[("query", query.as_str()), ("fmt", "json"), ("limit", "5")],
    )
    .map_err(|error| error.to_string())?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("MusicBrainz couldn't be reached ({error})"))?;
    if !response.status().is_success() {
        return Err(format!(
            "MusicBrainz answered HTTP {}",
            response.status().as_u16()
        ));
    }
    let answer: Value = response
        .json()
        .await
        .map_err(|_| "MusicBrainz's answer couldn't be read".to_owned())?;
    Ok(parse_release(&answer, code))
}

/// The first release in a MusicBrainz search answer whose barcode is the code.
pub(crate) fn parse_release(
    answer: &Value,
    code: &str,
) -> Option<(String, Option<String>, Option<u32>)> {
    let releases = answer.get("releases")?.as_array()?;
    let release = releases.iter().find(|release| {
        release
            .get("barcode")
            .and_then(Value::as_str)
            .and_then(normalize)
            .is_some_and(|barcode| barcode == code)
    })?;
    let title = release.get("title")?.as_str()?.to_owned();
    let artist: String = release
        .get("artist-credit")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|credit| {
            let name = credit
                .get("name")
                .or_else(|| credit.pointer("/artist/name"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            let join = credit
                .get("joinphrase")
                .and_then(Value::as_str)
                .unwrap_or_default();
            format!("{name}{join}")
        })
        .collect();
    let year = release
        .get("date")
        .and_then(Value::as_str)
        .and_then(|date| date.get(..4))
        .and_then(|year| year.parse().ok());
    Some((
        title,
        Some(artist.trim().to_owned()).filter(|artist| !artist.is_empty()),
        year,
    ))
}

/// The sources' albums that are this release: the title alike, and the artist when both name one.
fn best_albums(title: &str, artist: Option<&str>, found: Vec<MusicAlbum>) -> Vec<MusicAlbum> {
    let title = raygun_resolver::normalize(title);
    let artist = artist.map(raygun_resolver::normalize);
    let mut matches: Vec<(u8, MusicAlbum)> = found
        .into_iter()
        .filter_map(|album| {
            let same_title = raygun_resolver::normalize(&album.title) == title;
            let near_title = !same_title && {
                let other = raygun_resolver::normalize(&album.title);
                other.starts_with(&title) || title.starts_with(&other)
            };
            if !same_title && !near_title {
                return None;
            }
            let artist_agrees = match (&artist, album.artist.as_deref()) {
                (Some(wanted), Some(theirs)) => {
                    let theirs = raygun_resolver::normalize(theirs);
                    theirs.contains(wanted.as_str()) || wanted.contains(theirs.as_str())
                }
                _ => true,
            };
            artist_agrees.then_some((
                u8::from(same_title) * 2 + u8::from(album.artwork_url.is_some()),
                album,
            ))
        })
        .collect();
    matches.sort_by_key(|(points, _)| std::cmp::Reverse(*points));
    matches
        .into_iter()
        .map(|(_, album)| album)
        .take(5)
        .collect()
}

/// A book with this ISBN on Open Library.
async fn book(client: &reqwest::Client, code: &str) -> Result<BarcodeFind, String> {
    let key = format!("ISBN:{code}");
    let url = reqwest::Url::parse_with_params(
        OPEN_LIBRARY,
        &[
            ("bibkeys", key.as_str()),
            ("format", "json"),
            ("jscmd", "data"),
        ],
    )
    .map_err(|error| error.to_string())?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("Open Library couldn't be reached ({error})"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Open Library answered HTTP {}",
            response.status().as_u16()
        ));
    }
    let answer: Value = response
        .json()
        .await
        .map_err(|_| "Open Library's answer couldn't be read".to_owned())?;
    Ok(parse_book(&answer, code).unwrap_or(BarcodeFind::Unknown {
        code: code.to_owned(),
    }))
}

pub(crate) fn parse_book(answer: &Value, code: &str) -> Option<BarcodeFind> {
    let book = answer.get(format!("ISBN:{code}"))?;
    let title = book.get("title")?.as_str()?.trim();
    if title.is_empty() {
        return None;
    }
    let authors = book
        .get("authors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|author| author.get("name").and_then(Value::as_str))
        .map(str::to_owned)
        .collect();
    let cover_url = book
        .get("cover")
        .and_then(|cover| cover.get("medium").or_else(|| cover.get("large")))
        .and_then(Value::as_str)
        .map(str::to_owned);
    Some(BarcodeFind::Book {
        code: code.to_owned(),
        title: title.to_owned(),
        authors,
        cover_url,
    })
}

/// What a scanned barcode is (v2.0.0): an album with the music sources' matches, a book, or
/// unknown.
#[tauri::command]
#[specta::specta]
pub async fn barcode_lookup(
    import: tauri::State<'_, crate::import::ImportService>,
    code: String,
) -> Result<BarcodeFind, String> {
    lookup(import.app(), import.client(), &code).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn codes_are_checked_and_upc_becomes_ean() {
        // A UPC-A (Nevermind) and an ISBN-13.
        assert_eq!(normalize("720642442517").as_deref(), Some("0720642442517"));
        assert_eq!(
            normalize("978-0-306-40615-7").as_deref(),
            Some("9780306406157")
        );
        assert!(is_isbn("9780306406157"));
        assert!(!is_isbn("0720642442517"));
        assert_eq!(normalize("9780306406158"), None, "a wrong check digit");
        assert_eq!(normalize("abc"), None);
        assert_eq!(normalize("96385074").as_deref(), Some("96385074"));
    }

    #[test]
    fn a_musicbrainz_release_is_read() {
        let answer = json!({ "releases": [
            { "title": "Other", "barcode": "5099902894225" },
            { "title": "Nevermind", "barcode": "720642442517", "date": "1991-09-24",
              "artist-credit": [{ "name": "Nirvana", "joinphrase": "" }] },
        ]});
        assert_eq!(
            parse_release(&answer, "0720642442517"),
            Some(("Nevermind".into(), Some("Nirvana".into()), Some(1991)))
        );
        assert_eq!(
            parse_release(&json!({ "releases": [] }), "0720642442517"),
            None
        );
    }

    #[test]
    fn an_open_library_book_is_read() {
        let answer = json!({ "ISBN:9780306406157": {
            "title": "Programming",
            "authors": [{ "name": "Ada" }],
            "cover": { "medium": "https://covers.openlibrary.org/b/id/1-M.jpg" }
        }});
        assert_eq!(
            parse_book(&answer, "9780306406157"),
            Some(BarcodeFind::Book {
                code: "9780306406157".into(),
                title: "Programming".into(),
                authors: vec!["Ada".into()],
                cover_url: Some("https://covers.openlibrary.org/b/id/1-M.jpg".into()),
            })
        );
        assert_eq!(parse_book(&json!({}), "9780306406157"), None);
    }
}
