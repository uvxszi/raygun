//! The audiobook sources asked at once (M46.22, M46.48). Each source's page is kept a while, so
//! paging back and opening a list again is instant and a source that answers late isn't lost. A
//! search ranks books by how well their title and author answer it; lists let the sources take
//! turns. A book two sources share (the same title, author and recording) shows once.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use raygun_ext_host::audiobook::{BookPage, BookResult};

use super::titles::{self, BookName};

/// How long a source's page is kept.
const KEEP: Duration = Duration::from_secs(10 * 60);
/// Most pages kept.
const MAX_PAGES: usize = 300;

/// What one source was asked.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ask {
    pub provider: String,
    pub list: String,
    /// Trimmed, lower case.
    pub query: String,
    pub page: u32,
}

/// Sources' pages kept for `KEEP`.
#[derive(Default)]
pub struct Pages(Mutex<HashMap<Ask, (Instant, BookPage)>>);

impl Pages {
    pub fn get(&self, ask: &Ask) -> Option<BookPage> {
        let pages = self.0.lock().unwrap_or_else(|p| p.into_inner());
        pages
            .get(ask)
            .filter(|(at, _)| at.elapsed() < KEEP)
            .map(|(_, page)| page.clone())
    }

    pub fn put(&self, ask: Ask, page: BookPage) {
        let mut pages = self.0.lock().unwrap_or_else(|p| p.into_inner());
        pages.retain(|_, (at, _)| at.elapsed() < KEEP);
        if pages.len() >= MAX_PAGES
            && let Some(oldest) = pages
                .iter()
                .min_by_key(|(_, (at, _))| *at)
                .map(|(ask, _)| ask.clone())
        {
            pages.remove(&oldest);
        }
        pages.insert(ask, (Instant::now(), page));
    }
}

/// A book ready to list: its tidied name, what the source said, and which source.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub name: BookName,
    pub book: BookResult,
    pub source: String,
}

/// The sources' pages (in the sources' order) as one list, and whether any has more. `search` is
/// the query when the list is a search.
pub fn merge(pages: Vec<(String, BookPage)>, search: Option<&str>) -> (Vec<Found>, bool) {
    let has_more = pages.iter().any(|(_, page)| page.has_more);
    let words = search.map(words).unwrap_or_default();
    let mut ranked: Vec<(u32, usize, usize, Found)> = Vec::new();
    for (source_index, (source, page)) in pages.into_iter().enumerate() {
        for (place, book) in page.books.into_iter().enumerate() {
            let name = titles::book(&book.title, book.author.as_deref());
            let score = if words.is_empty() {
                0
            } else {
                score(&name, &words)
            };
            ranked.push((
                score,
                place,
                source_index,
                Found {
                    name,
                    book,
                    source: source.clone(),
                },
            ));
        }
    }
    // Best answers first; then each source's own order, the sources taking turns.
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut found: Vec<(usize, Found)> = Vec::with_capacity(ranked.len());
    for (_, _, source_index, book) in ranked {
        match found.iter_mut().find(|(_, known)| same(known, &book)) {
            // The first source's copy stands for the book (LibriVox's own feed before a copy of
            // it elsewhere), where the better ranked of the two was.
            Some((known_index, known)) if source_index < *known_index => {
                let other = std::mem::replace(known, book);
                *known_index = source_index;
                fill(known, other);
            }
            Some((_, known)) => fill(known, book),
            None => found.push((source_index, book)),
        }
    }
    (found.into_iter().map(|(_, book)| book).collect(), has_more)
}

/// A later page without the books an earlier page listed already (`earlier`, as `merge` gave
/// them): sources page apart, so one source's second page can hold another's first.
pub fn after(found: Vec<Found>, earlier: &[Found]) -> Vec<Found> {
    found
        .into_iter()
        .filter(|book| !earlier.iter().any(|known| same(known, book)))
        .collect()
}

/// The words of a query or title, for matching: letters and digits, lower case.
fn words(text: &str) -> Vec<String> {
    text.split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// How well a book answers a search: its whole title, the start of it, every word in the title
/// or the author, then some of them. The first recording of a book comes before later ones.
fn score(name: &BookName, query: &[String]) -> u32 {
    let title = words(&name.title);
    let author = name.author.as_deref().map(words).unwrap_or_default();
    let matches = |within: &[String], word: &String| {
        within
            .iter()
            .any(|part| part == word || (word.len() >= 3 && part.starts_with(word.as_str())))
    };
    let in_title = query.iter().filter(|word| matches(&title, word)).count();
    let in_author = query.iter().filter(|word| matches(&author, word)).count();
    let in_either = query
        .iter()
        .filter(|word| matches(&title, word) || matches(&author, word))
        .count();
    let all = query.len();
    let mut score = if title == query {
        100
    } else if title.starts_with(query) {
        70
    } else if in_title == all {
        50
    } else if in_author == all {
        40
    } else if in_either == all {
        35
    } else {
        (in_either * 20 / all.max(1)) as u32
    };
    if name.edition.is_none() {
        score += 1;
    }
    score
}

/// The same book from two sources (or twice from one): the same feed, or the same title and
/// recording by the same author (or one of them not saying).
fn same(known: &Found, other: &Found) -> bool {
    if known.book.feed_url == other.book.feed_url {
        return true;
    }
    let edition = |found: &Found| titles::simple(found.name.edition.as_deref().unwrap_or_default());
    titles::simple(&known.name.title) == titles::simple(&other.name.title)
        && edition(known) == edition(other)
        && match (&known.name.author, &other.name.author) {
            (Some(a), Some(b)) => titles::same_person(a, b),
            _ => true,
        }
}

/// What the first copy of a book lacks, from another source's.
fn fill(known: &mut Found, other: Found) {
    let book = &mut known.book;
    book.artwork_url = book.artwork_url.take().or(other.book.artwork_url);
    book.runtime = book.runtime.take().or(other.book.runtime);
    book.description = book.description.take().or(other.book.description);
    book.language = book.language.take().or(other.book.language);
    known.name.author = known.name.author.take().or(other.name.author);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book(title: &str, author: &str, feed: &str) -> BookResult {
        BookResult {
            title: title.into(),
            author: Some(author.into()),
            feed_url: feed.into(),
            artwork_url: None,
            runtime: None,
            language: None,
            description: None,
        }
    }

    fn page(books: Vec<BookResult>, has_more: bool) -> BookPage {
        BookPage { books, has_more }
    }

    fn titles_of(found: &[Found]) -> Vec<(String, String)> {
        found
            .iter()
            .map(|found| (found.name.title.clone(), found.source.clone()))
            .collect()
    }

    #[test]
    fn a_search_ranks_by_title_and_author_and_shows_a_shared_book_once() {
        let librivox = page(
            vec![
                // Popular first, as the Internet Archive sorts them.
                book(
                    "Pride and Prejudice",
                    "Jane Austen",
                    "https://librivox.org/rss/253",
                ),
                book(
                    "Emma (version 3)",
                    "Jane Austen",
                    "https://librivox.org/rss/3359",
                ),
                book("Emma", "Jane Austen", "https://librivox.org/rss/86"),
                book(
                    "The Watsons",
                    "Jane Austen",
                    "https://librivox.org/rss/2666",
                ),
            ],
            true,
        );
        let mut loyal_emma = book(
            "Emma",
            "Jane Austen",
            "https://www.loyalbooks.com/book/emma-by-jane-austen/feed",
        );
        loyal_emma.artwork_url = Some("https://www.loyalbooks.com/image/emma.jpg".into());
        let loyal = page(vec![loyal_emma], false);
        let (found, has_more) = merge(
            vec![("LibriVox".into(), librivox), ("Loyal Books".into(), loyal)],
            Some("emma"),
        );
        assert!(has_more);
        assert_eq!(
            titles_of(&found),
            [
                ("Emma".to_owned(), "LibriVox".to_owned()),
                ("Emma".to_owned(), "LibriVox".to_owned()),
                ("Pride and Prejudice".to_owned(), "LibriVox".to_owned()),
                ("The Watsons".to_owned(), "LibriVox".to_owned()),
            ],
            "the exact title first (the first recording before version 3); Loyal Books' Emma is \
             LibriVox's"
        );
        assert_eq!(found[1].name.edition.as_deref(), Some("Version 3"));
        assert_eq!(
            found[0].book.artwork_url.as_deref(),
            Some("https://www.loyalbooks.com/image/emma.jpg"),
            "the copy kept takes what it lacked from the other"
        );

        let (by_author, _) = merge(
            vec![(
                "LibriVox".into(),
                page(
                    vec![
                        book(
                            "Jane Eyre",
                            "Charlotte Brontë",
                            "https://librivox.org/rss/133",
                        ),
                        book("Persuasion", "Jane Austen", "https://librivox.org/rss/6"),
                    ],
                    false,
                ),
            )],
            Some("jane austen"),
        );
        assert_eq!(by_author[0].name.title, "Persuasion");
    }

    #[test]
    fn lists_let_the_sources_take_turns_and_tidy_every_title() {
        let (found, has_more) = merge(
            vec![
                (
                    "LibriVox".into(),
                    page(
                        vec![
                            book(
                                "Alice's Adventures in Wonderland, by Lewis Carroll",
                                "Lewis Carroll",
                                "https://librivox.org/rss/200",
                            ),
                            book("The Art of War", "Sun Tzu", "https://librivox.org/rss/119"),
                        ],
                        false,
                    ),
                ),
                (
                    "Loyal Books".into(),
                    page(
                        vec![
                            book("Moby Dick", "Herman Melville", "https://lb/moby/feed"),
                            book(
                                "Alice's Adventures in Wonderland",
                                "Lewis Carroll",
                                "https://lb/alice/feed",
                            ),
                        ],
                        false,
                    ),
                ),
            ],
            None,
        );
        assert!(!has_more);
        assert_eq!(
            titles_of(&found),
            [
                (
                    "Alice's Adventures in Wonderland".to_owned(),
                    "LibriVox".to_owned()
                ),
                ("Moby Dick".to_owned(), "Loyal Books".to_owned()),
                ("The Art of War".to_owned(), "LibriVox".to_owned()),
            ]
        );
    }

    #[test]
    fn different_recordings_and_authors_stay_apart() {
        let (found, _) = merge(
            vec![(
                "LibriVox".into(),
                page(
                    vec![
                        book("Poems", "Emily Dickinson", "https://librivox.org/rss/1"),
                        book("Poems", "Wilfred Owen", "https://librivox.org/rss/2"),
                        book("Dracula", "Bram Stoker", "https://librivox.org/rss/271"),
                        book(
                            "Dracula (version 2)",
                            "Bram Stoker",
                            "https://librivox.org/rss/4091",
                        ),
                    ],
                    false,
                ),
            )],
            Some("poems"),
        );
        assert_eq!(found.len(), 4);
    }

    #[test]
    fn later_pages_leave_out_books_shown_before() {
        let (first, _) = merge(
            vec![(
                "LibriVox".into(),
                page(
                    vec![book("Emma", "Jane Austen", "https://librivox.org/rss/86")],
                    true,
                ),
            )],
            None,
        );
        let (second, _) = merge(
            vec![(
                "Loyal Books".into(),
                page(
                    vec![
                        book("Emma", "Jane Austen", "https://lb/emma/feed"),
                        book("Persuasion", "Jane Austen", "https://lb/persuasion/feed"),
                    ],
                    false,
                ),
            )],
            None,
        );
        assert_eq!(
            titles_of(&after(second, &first)),
            [("Persuasion".to_owned(), "Loyal Books".to_owned())]
        );
    }

    #[test]
    fn pages_are_kept_a_while() {
        let pages = Pages::default();
        let ask = Ask {
            provider: "p".into(),
            list: "search".into(),
            query: "emma".into(),
            page: 1,
        };
        assert!(pages.get(&ask).is_none());
        pages.put(ask.clone(), page(Vec::new(), true));
        assert!(pages.get(&ask).is_some_and(|page| page.has_more));
    }
}
