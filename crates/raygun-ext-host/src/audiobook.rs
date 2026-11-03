//! `audiobook.directory` providers (M46.22): finding audiobooks by title or author, and listing
//! popular, new or a subject's books, a page at a time. Each book comes with its chapter feed,
//! which the app reads like a podcast's.

use serde_json::Value;

use crate::ExtensionHost;

pub const AUDIOBOOK_DIRECTORY: &str = "audiobook.directory";
/// Most books kept from one page.
const MAX_BOOKS: usize = 100;

/// A book a directory found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookResult {
    pub title: String,
    pub author: Option<String>,
    /// http(s) only: the book's chapter feed.
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
        Ok(parse_page(&value))
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

/// `{ books: [...], hasMore }` as a provider returned it; books without a title or an http(s)
/// feed are dropped, and so are repeats of a feed.
pub fn parse_page(value: &Value) -> BookPage {
    let mut seen = std::collections::HashSet::new();
    let books = value
        .get("books")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|book| {
            Some(BookResult {
                title: text(book.get("title"))?,
                author: text(book.get("author")),
                feed_url: url(book.get("feedUrl"))?,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_keep_books_with_a_feed() {
        let page = parse_page(&serde_json::json!({
            "books": [
                { "title": " Pride and Prejudice ", "author": "Jane Austen",
                  "feedUrl": "https://librivox.org/rss/253", "runtime": "13:06:44" },
                { "title": "No feed" },
                { "title": "Twice", "feedUrl": "https://librivox.org/rss/253" },
                { "title": "Bad feed", "feedUrl": "ftp://x" },
            ],
            "hasMore": true,
        }));
        assert_eq!(page.books.len(), 1);
        assert_eq!(page.books[0].title, "Pride and Prejudice");
        assert_eq!(page.books[0].runtime.as_deref(), Some("13:06:44"));
        assert!(page.has_more);
        assert_eq!(parse_page(&serde_json::json!({})), BookPage::default());
    }
}
