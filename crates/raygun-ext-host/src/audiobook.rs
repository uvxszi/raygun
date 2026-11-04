//! `audiobook.directory` providers (M46.22): finding audiobooks by title or author, and listing
//! popular, new or a subject's books, a page at a time. Each book comes with its chapter feed,
//! which the app reads like a podcast's; sources whose books have no feed list a book's chapters
//! themselves (`chapters`, M46.48), and such a book's feed address is a key naming the provider
//! and the book (`audiobook:<provider>#<id>`).

use serde_json::Value;

use crate::ExtensionHost;

pub const AUDIOBOOK_DIRECTORY: &str = "audiobook.directory";
/// Most books kept from one page.
const MAX_BOOKS: usize = 100;
/// Most chapters kept from one book.
const MAX_CHAPTERS: usize = 2_000;
/// How the feed address of a book without a feed starts.
const LISTED: &str = "audiobook:";

/// A book a directory found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookResult {
    pub title: String,
    pub author: Option<String>,
    /// The book's chapter feed (http(s)), or a `listed_key` when the source lists its chapters.
    pub feed_url: String,
    pub artwork_url: Option<String>,
    /// "13:06:44", as the directory gives it.
    pub runtime: Option<String>,
    pub language: Option<String>,
    pub description: Option<String>,
}

/// One page of books, and whether there are more.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BookPage {
    pub books: Vec<BookResult>,
    pub has_more: bool,
}

/// A book as a source lists it, with its chapters in order (M46.48).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookDetails {
    pub title: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub artwork_url: Option<String>,
    /// The book's page on the source's site.
    pub link: Option<String>,
    pub chapters: Vec<BookChapter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookChapter {
    /// Stable within the book; the audio address when the source gives none.
    pub id: String,
    pub title: String,
    pub audio_url: String,
    pub audio_type: Option<String>,
    pub duration_ms: Option<i64>,
    pub description: Option<String>,
}

/// The feed address of a book whose source lists its chapters.
pub fn listed_key(provider: &str, id: &str) -> String {
    format!("{LISTED}{provider}#{id}")
}

/// The provider and book a `listed_key` names.
pub fn parse_listed_key(key: &str) -> Option<(&str, &str)> {
    let (provider, id) = key.strip_prefix(LISTED)?.split_once('#')?;
    (!provider.is_empty() && !id.is_empty()).then_some((provider, id))
}

impl ExtensionHost {
    /// A page of `list` (`search`, `popular`, `latest` or `subject`) from one directory.
    pub async fn audiobooks(
        &self,
        provider: &str,
        list: &str,
        query: &str,
        page: u32,
    ) -> Result<BookPage, String> {
        let (method, args) = match list {
            "search" => (
                "search",
                serde_json::json!({ "query": query, "page": page }),
            ),
            "subject" => (
                "subject",
                serde_json::json!({ "subject": query, "page": page }),
            ),
            "latest" => ("latest", serde_json::json!({ "page": page })),
            _ => ("popular", serde_json::json!({ "page": page })),
        };
        let value = self
            .call_interactive(provider, method, args)
            .await
            .map_err(|error| error.to_string())?;
        Ok(parse_page(&value, provider))
    }

    /// A book's details and chapters from the source that listed it (M46.48).
    pub async fn audiobook_details(&self, provider: &str, id: &str) -> Result<BookDetails, String> {
        let value = self
            .call_interactive(provider, "chapters", serde_json::json!({ "id": id }))
            .await
            .map_err(|error| error.to_string())?;
        parse_details(&value)
            .ok_or_else(|| "The source didn't list this book's chapters".to_owned())
    }
}

fn text(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn url(value: Option<&Value>) -> Option<String> {
    text(value).filter(|url| url.starts_with("https://") || url.starts_with("http://"))
}

/// `{ books: [...], hasMore }` as a provider returned it; books without a title, or without
/// either an http(s) feed or an id, are dropped, and so are repeats.
pub fn parse_page(value: &Value, provider: &str) -> BookPage {
    let mut seen = std::collections::HashSet::new();
    let books = value
        .get("books")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|book| {
            let feed_url = url(book.get("feedUrl"))
                .or_else(|| text(book.get("id")).map(|id| listed_key(provider, &id)))?;
            Some(BookResult {
                title: text(book.get("title"))?,
                author: text(book.get("author")),
                feed_url,
                artwork_url: url(book.get("artworkUrl")),
                runtime: text(book.get("runtime")),
                language: text(book.get("language")),
                description: text(book.get("description")),
            })
        })
        .filter(|book| seen.insert(book.feed_url.clone()))
        .take(MAX_BOOKS)
        .collect();
    BookPage {
        books,
        has_more: value
            .get("hasMore")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

/// `{ title, author, description, artworkUrl, link, chapters: [{ id, title, audioUrl,
/// durationMs, description }] }`; chapters without an http(s) audio address are dropped.
pub fn parse_details(value: &Value) -> Option<BookDetails> {
    let mut seen = std::collections::HashSet::new();
    let chapters: Vec<BookChapter> = value
        .get("chapters")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|chapter| {
            let audio_url = url(chapter.get("audioUrl"))?;
            Some(BookChapter {
                id: text(chapter.get("id")).unwrap_or_else(|| audio_url.clone()),
                title: text(chapter.get("title")).unwrap_or_default(),
                audio_type: text(chapter.get("audioType")),
                duration_ms: chapter
                    .get("durationMs")
                    .and_then(Value::as_f64)
                    .filter(|ms| ms.is_finite() && *ms > 0.0)
                    .map(|ms| ms.round() as i64),
                description: text(chapter.get("description")),
                audio_url,
            })
        })
        .filter(|chapter| seen.insert(chapter.id.clone()))
        .take(MAX_CHAPTERS)
        .collect();
    if chapters.is_empty() {
        return None;
    }
    Some(BookDetails {
        title: text(value.get("title"))?,
        author: text(value.get("author")),
        description: text(value.get("description")),
        artwork_url: url(value.get("artworkUrl")),
        link: url(value.get("link")),
        chapters,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_keep_books_with_a_feed_or_an_id() {
        let page = parse_page(
            &serde_json::json!({
                "books": [
                    { "title": " Pride and Prejudice ", "author": "Jane Austen",
                      "feedUrl": "https://librivox.org/rss/253", "runtime": "13:06:44" },
                    { "title": "No feed" },
                    { "title": "Twice", "feedUrl": "https://librivox.org/rss/253" },
                    { "title": "Bad feed", "feedUrl": "ftp://x" },
                    { "title": "Listed", "id": "32/sherlock" },
                ],
                "hasMore": true,
            }),
            "dev.raygun.lit2go/books",
        );
        assert_eq!(page.books.len(), 2);
        assert_eq!(page.books[0].title, "Pride and Prejudice");
        assert_eq!(page.books[0].runtime.as_deref(), Some("13:06:44"));
        assert_eq!(
            page.books[1].feed_url,
            "audiobook:dev.raygun.lit2go/books#32/sherlock"
        );
        assert_eq!(
            parse_listed_key(&page.books[1].feed_url),
            Some(("dev.raygun.lit2go/books", "32/sherlock"))
        );
        assert!(page.has_more);
        assert_eq!(parse_page(&serde_json::json!({}), "p"), BookPage::default());
        assert_eq!(parse_listed_key("https://librivox.org/rss/1"), None);
        assert_eq!(parse_listed_key("audiobook:#x"), None);
    }

    #[test]
    fn details_keep_chapters_with_audio() {
        let details = parse_details(&serde_json::json!({
            "title": "The Adventures of Sherlock Holmes",
            "author": "Sir Arthur Conan Doyle",
            "link": "https://etc.usf.edu/lit2go/32/",
            "chapters": [
                { "id": "345", "title": "A Scandal in Bohemia",
                  "audioUrl": "https://etc.usf.edu/lit2go/audio/mp3/a.345.mp3", "durationMs": 1.5e6 },
                { "title": "No audio" },
                { "id": "345", "title": "Twice", "audioUrl": "https://x/b.mp3" },
                { "title": "No id", "audioUrl": "https://x/c.mp3", "durationMs": -1 },
            ],
        }))
        .unwrap();
        assert_eq!(details.chapters.len(), 2);
        assert_eq!(details.chapters[0].duration_ms, Some(1_500_000));
        assert_eq!(details.chapters[1].id, "https://x/c.mp3");
        assert_eq!(details.chapters[1].duration_ms, None);
        assert_eq!(
            details.link.as_deref(),
            Some("https://etc.usf.edu/lit2go/32/")
        );
        assert!(parse_details(&serde_json::json!({ "title": "Empty", "chapters": [] })).is_none());
    }
}
