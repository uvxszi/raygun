//! What Raygun keeps of a provider's answer, for the people who write extensions (`raygun-ext
//! test` and the `call` example): the answer read by the parsers the app reads it with, entry by
//! entry, so the entries the app would drop show before the extension ships.

use std::time::Duration;

use raygun_model::{MediaSource, ProviderInfo};
use serde_json::{Value, json};

use crate::answers::{self, ANSWERS};
use crate::audiobook::{self, AUDIOBOOK_DIRECTORY};
use crate::dictionary::{self, DICTIONARY};
use crate::host::{
    LYRICS, MUSIC_CHARTS, MUSIC_INFO, MUSIC_METADATA, MUSIC_RECOMMEND, MUSIC_STREAM, MUSIC_TAGS,
    albums_in, allowed_source, artist_names, artists_in, events_in, parse_album, parse_artist_info,
    parse_candidates, parse_charts, parse_credits, parse_lyrics, parse_playlist, parse_tags,
    parse_tracks, playlists_in,
};
use crate::manga::{self, MANGA_SOURCE};
use crate::novel::{NOVEL_SOURCE, PassageBlock, parse_passage};
use crate::podcast::{PODCAST_DIRECTORY, parse_results};
use crate::video::{self, VIDEO_SOURCE};
use crate::{CATALOG_SEARCH_TIMEOUT, INTERACTIVE_TIMEOUT};

/// How long the app gives a call that no tighter limit covers (`HostConfig::call_timeout`).
pub const CALL_TIMEOUT: Duration = Duration::from_secs(30);

/// What Raygun makes of one answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Review {
    /// What the answer holds: "tracks", "items", "chapters"; for an answer that is one object,
    /// what that is ("details", "passage").
    pub what: &'static str,
    /// The answer is one object, not a list.
    pub one: bool,
    /// Entries the answer gave; 1 for one object, 0 for none (`null`).
    pub given: usize,
    /// Entries Raygun keeps.
    pub kept: usize,
    /// Places (from 0) of the entries Raygun drops as they are; repeats dropped for being
    /// repeats aren't counted here.
    pub dropped: Vec<usize>,
    /// What an entry needs to be kept.
    pub needs: &'static str,
    /// What else the app reads in it, or leaves out: "12 synced lines", "no album".
    pub notes: Vec<String>,
}

impl Review {
    /// "Raygun keeps 28 of 30 tracks", with the dropped entries and what they lack; for one
    /// object, whether Raygun can use it.
    pub fn summary(&self) -> String {
        let mut text = match (self.one, self.given, self.kept) {
            (true, 0, _) => format!("Raygun has no {} to show", self.what),
            (true, _, 0) => format!(
                "Raygun can't use the {}: it needs {}",
                self.what, self.needs
            ),
            (true, _, _) => format!("Raygun reads the {}", self.what),
            (false, given, kept) => format!("Raygun keeps {kept} of {given} {}", self.what),
        };
        if !self.one {
            if !self.dropped.is_empty() {
                let places: Vec<String> =
                    self.dropped.iter().take(20).map(usize::to_string).collect();
                let more = if self.dropped.len() > 20 { ", …" } else { "" };
                text.push_str(&format!(
                    "; it drops entries {}{more} (each needs {})",
                    places.join(", "),
                    self.needs
                ));
            } else if self.kept < self.given {
                text.push_str(" (the rest repeat an entry)");
            }
        }
        for note in &self.notes {
            text.push_str(&format!("\n  {note}"));
        }
        text
    }
}

const TRACK: &str = "a title; a url source needs headers (use {} for none)";
const ARTIST: &str = "an id and a name";
const RELEASE: &str = "an id and a title";
const EVENT: &str = "a name and a date";
const CANDIDATE: &str = "an id and a title; an inline url source must be an http(s) address \
                         with headers";
const CHART: &str = "an id and a name";
const TAG: &str = "a name of at most 60 characters";
const ITEM: &str = "an id and a title";
const CHAPTER: &str = "an id, and a name or a number";
const PAGE: &str = "an http(s) address";
const FILTER: &str = "a kind (select, tags or toggle), a key of letters, digits, - and _ (at \
                      most 40), and options for select and tags";
const SERVER: &str = "an id";
const VIDEO: &str = "an http(s) url (or youtube:<11-character id>)";
const PODCAST: &str = "a title and an http(s) feedUrl";
const BOOK: &str = "a title, and an http(s) feedUrl or an id";
const BOOK_CHAPTER: &str = "an http(s) audioUrl";
const COMMENT: &str = "an author and a text";
const ENTRY: &str = "a word, and a meaning with a definition";
const ANSWER: &str = "a result (text, or { text, tex })";

/// How the app reads `method`'s answer from a provider of `kind`; `None` for a call the app never
/// makes.
pub fn review(kind: &str, method: &str, answer: &Value) -> Option<Review> {
    let provider = ProviderInfo {
        id: "review/provider".to_owned(),
        extension_id: "review".to_owned(),
        kind: kind.to_owned(),
        name: "The source".to_owned(),
        search_pill: None,
        adult: false,
    };
    let tracks = || {
        let review = listing("tracks", TRACK, answer, Some("tracks"), |value| {
            parse_tracks(value.clone()).len()
        });
        with_links(review, answer, "tracks", "artworkUrl")
    };
    let catalog = matches!(kind, MANGA_SOURCE | NOVEL_SOURCE | VIDEO_SOURCE);
    let review = match (kind, method) {
        (MUSIC_METADATA, "search")
        | (MUSIC_CHARTS, "chart" | "tagSongs")
        | (MUSIC_RECOMMEND, "topTracks" | "newReleases" | "radio") => tracks(),
        (MUSIC_METADATA, "album") => {
            let found = answer
                .get("album")
                .and_then(|album| parse_album(&provider, album));
            with_note(
                tracks(),
                found.is_none(),
                "no album: it needs an id and a title, or the album page fails",
            )
        }
        (MUSIC_METADATA, "playlist") => {
            let found = answer
                .get("playlist")
                .and_then(|playlist| parse_playlist(&provider, playlist));
            with_note(
                tracks(),
                found.is_none(),
                "no playlist: it needs an id and a title, or the playlist page fails",
            )
        }
        (MUSIC_METADATA, "searchArtists") => {
            let review = listing("artists", ARTIST, answer, Some("artists"), |value| {
                artists_in(&provider, value).len()
            });
            with_links(review, answer, "artists", "imageUrl")
        }
        (MUSIC_METADATA, "searchAlbums") => {
            let review = listing("albums", RELEASE, answer, Some("albums"), |value| {
                albums_in(&provider, value, "albums").len()
            });
            with_links(review, answer, "albums", "artworkUrl")
        }
        (MUSIC_METADATA, "artistReleases") => {
            listing("releases", RELEASE, answer, Some("releases"), |value| {
                albums_in(&provider, value, "releases").len()
            })
        }
        (MUSIC_METADATA, "searchPlaylists") => {
            listing("playlists", RELEASE, answer, Some("playlists"), |value| {
                playlists_in(&provider, value, false).len()
            })
        }
        (MUSIC_METADATA, "artistEvents") => {
            listing("events", EVENT, answer, Some("events"), |value| {
                events_in(&provider, value).len()
            })
        }
        (MUSIC_STREAM, "candidates") => listing(
            "candidates",
            CANDIDATE,
            answer,
            Some("candidates"),
            |value| parse_candidates(&provider, value.clone()).len(),
        ),
        (MUSIC_STREAM, "resolve") => {
            let source = answer
                .get("source")
                .cloned()
                .and_then(|source| serde_json::from_value::<MediaSource>(source).ok())
                .filter(allowed_source);
            single(
                "stream",
                "a source: an http(s) url with headers, or a YouTube videoId",
                source.is_some(),
            )
        }
        (LYRICS, "lyrics") => lyrics(&provider, answer),
        (MUSIC_CHARTS, "charts") => listing("charts", CHART, answer, Some("charts"), |value| {
            parse_charts(&provider, value).len()
        }),
        (MUSIC_TAGS, "tags") => listing("tags", TAG, answer, Some("tags"), |value| {
            parse_tags(value).len()
        }),
        (MUSIC_INFO, "artist") if answer.get("artist").is_some_and(Value::is_null) => nothing(
            "artist info",
            "a name",
            "the source doesn't know the artist (null)",
        ),
        (MUSIC_INFO, "artist") => single(
            "artist info",
            "a name",
            parse_artist_info(answer, "The source").is_some(),
        ),
        (MUSIC_INFO, "credits") => {
            let groups = answer
                .get("credits")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            let kept = parse_credits(answer, "The source").map_or(0, |found| found.credits.len());
            Review {
                what: "credit groups",
                one: false,
                given: groups,
                kept,
                dropped: Vec::new(),
                needs: "a role and names",
                notes: Vec::new(),
            }
        }
        (MUSIC_RECOMMEND, "similarArtists") => {
            listing("artists", "a name", answer, Some("artists"), |value| {
                artist_names(value).len()
            })
        }
        (_, "popular" | "latest" | "search") if catalog => {
            let review = listing("items", ITEM, answer, Some("items"), |value| {
                manga::parse_listing(value).items.len()
            });
            let more = answer.get("hasNextPage").and_then(Value::as_bool);
            let review = with_note(
                review,
                more.is_none(),
                "no hasNextPage: Raygun reads it as the last page",
            );
            with_links(review, answer, "items", "coverUrl")
        }
        (_, "filters") if catalog => listing("filters", FILTER, answer, None, |value| {
            manga::parse_filters(value).len()
        }),
        (_, "details") if catalog => single(
            "details",
            "a title",
            manga::parse_details(answer, "requested").is_some(),
        ),
        (MANGA_SOURCE | NOVEL_SOURCE, "chapters") => {
            listing("chapters", CHAPTER, answer, Some("chapters"), |value| {
                manga::parse_units(value, "chapters", "Chapter").len()
            })
        }
        (MANGA_SOURCE, "pages") if answer.get("pdf").is_some() => pdf_answer("pages", answer),
        (MANGA_SOURCE, "pages") => listing("pages", PAGE, answer, Some("pages"), |value| {
            manga::parse_pages(value).len()
        }),
        (NOVEL_SOURCE, "passage") => passage(answer),
        (VIDEO_SOURCE, "episodes") => {
            listing("episodes", CHAPTER, answer, Some("episodes"), |value| {
                let mut episodes = manga::parse_units(value, "episodes", "Episode");
                video::fill_from_names(&mut episodes);
                episodes.len()
            })
        }
        (VIDEO_SOURCE, "servers") => {
            let resolved = video::parse_servers(answer)
                .iter()
                .filter(|server| server.videos.is_some())
                .count();
            let review = listing("servers", SERVER, answer, Some("servers"), |value| {
                video::parse_servers(value).len()
            });
            with_note(
                review,
                resolved > 0,
                &format!("{resolved} came with their videos: Raygun won't call videos for them"),
            )
        }
        (VIDEO_SOURCE, "videos") => listing("videos", VIDEO, answer, Some("videos"), |value| {
            video::parse_videos(value).len()
        }),
        (VIDEO_SOURCE, "comments") => comments(answer),
        (PODCAST_DIRECTORY, "search" | "top") => {
            let review = listing("podcasts", PODCAST, answer, None, |value| {
                parse_results(value).len()
            });
            with_links(review, answer, "", "artworkUrl")
        }
        (AUDIOBOOK_DIRECTORY, "search" | "popular" | "latest" | "subject") => {
            let review = listing("books", BOOK, answer, Some("books"), |value| {
                audiobook::parse_page(value, "review/provider").books.len()
            });
            with_links(review, answer, "books", "artworkUrl")
        }
        (AUDIOBOOK_DIRECTORY, "chapters") => {
            let review = listing(
                "chapters",
                BOOK_CHAPTER,
                answer,
                Some("chapters"),
                |value| {
                    let chapters = value.get("chapters").cloned().unwrap_or_default();
                    audiobook::parse_details(&json!({ "title": "x", "chapters": chapters }))
                        .map_or(0, |details| details.chapters.len())
                },
            );
            let titled = audiobook::parse_details(answer).is_some();
            with_note(
                review,
                !titled && answer.get("chapters").is_some(),
                "no book: it needs a title (and chapters with audio)",
            )
        }
        (DICTIONARY, "define") => listing("entries", ENTRY, answer, Some("entries"), |value| {
            dictionary::parse_entries(value, "en").len()
        }),
        (ANSWERS, "answer") => listing("answers", ANSWER, answer, Some("answers"), |value| {
            answers::parse_answers(value, "review/provider", "Review").len()
        }),
        _ => return None,
    };
    Some(review)
}

/// How long the app waits for `method` of a provider of `kind` before it gives up on it.
pub fn time_limit(kind: &str, method: &str) -> Duration {
    match (kind, method) {
        (MUSIC_METADATA, "album" | "playlist") => CALL_TIMEOUT,
        (
            MUSIC_METADATA | MUSIC_STREAM | LYRICS | MUSIC_INFO | AUDIOBOOK_DIRECTORY | DICTIONARY
            | ANSWERS,
            _,
        )
        | (MUSIC_CHARTS, "tagSongs")
        | (VIDEO_SOURCE, "comments") => INTERACTIVE_TIMEOUT,
        // A search row, or the popular titles it shows before anything is typed.
        (MANGA_SOURCE | NOVEL_SOURCE | VIDEO_SOURCE, "search" | "popular") => {
            CATALOG_SEARCH_TIMEOUT
        }
        _ => CALL_TIMEOUT,
    }
}

/// The entries under `key` (the answer itself for an empty key or `None`), when they're a list.
fn entries<'a>(answer: &'a Value, key: Option<&str>) -> Option<&'a Value> {
    match key {
        Some(key) if !key.is_empty() => answer.get(key),
        _ => Some(answer),
    }
}

/// A list read entry by entry: `count` says how many entries Raygun keeps of an answer, and each
/// entry is tried on its own in an answer of one.
fn listing(
    what: &'static str,
    needs: &'static str,
    answer: &Value,
    key: Option<&str>,
    count: impl Fn(&Value) -> usize,
) -> Review {
    let list = entries(answer, key);
    let given = list
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    let alone = |entry: &Value| match key {
        Some(key) => json!({ key: [entry] }),
        None => json!([entry]),
    };
    let dropped = given
        .iter()
        .enumerate()
        .filter(|(_, entry)| count(&alone(entry)) == 0)
        .map(|(place, _)| place)
        .collect();
    let mut notes = Vec::new();
    if list.is_some_and(|list| !list.is_array()) || (list.is_none() && !answer.is_null()) {
        notes.push(match key {
            Some(key) => format!("the answer has no `{key}` list"),
            None => "the answer isn't a list".to_owned(),
        });
    }
    Review {
        what,
        one: false,
        given: given.len(),
        kept: count(answer),
        dropped,
        needs,
        notes,
    }
}

/// Notes the entries (under `key`) whose `field` holds an address Raygun can't use: it keeps
/// them without it.
fn with_links(mut review: Review, answer: &Value, key: &str, field: &str) -> Review {
    let unusable = entries(answer, Some(key))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get(field)?.as_str())
        .filter(|link| {
            let link = link.trim();
            !link.is_empty() && !link.starts_with("https://") && !link.starts_with("http://")
        })
        .count();
    if unusable > 0 {
        review.notes.push(format!(
            "{unusable} without their {field}: it must be an http(s) address"
        ));
    }
    review
}

/// An answer that is one object, kept or not.
fn single(what: &'static str, needs: &'static str, kept: bool) -> Review {
    Review {
        what,
        one: true,
        given: 1,
        kept: usize::from(kept),
        dropped: if kept { Vec::new() } else { vec![0] },
        needs,
        notes: Vec::new(),
    }
}

/// An answer that says there's nothing (`null`).
fn nothing(what: &'static str, needs: &'static str, note: &str) -> Review {
    Review {
        what,
        one: true,
        given: 0,
        kept: 0,
        dropped: Vec::new(),
        needs,
        notes: vec![note.to_owned()],
    }
}

fn with_note(mut review: Review, when: bool, note: &str) -> Review {
    if when {
        review.notes.push(note.to_owned());
    }
    review
}

fn lyrics(provider: &ProviderInfo, answer: &Value) -> Review {
    const NEEDS: &str = "synced lines or plain text";
    if answer.is_null() {
        return nothing("lyrics", NEEDS, "no lyrics for this song (null)");
    }
    let found = parse_lyrics(provider, answer);
    let mut review = single("lyrics", NEEDS, found.is_some());
    if let Some(lyrics) = found {
        let given = answer
            .get("synced")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        if given > 0 {
            review.notes.push(format!(
                "{} of {given} synced lines kept (each needs a timeMs of 0 or more and a text)",
                lyrics.synced.len()
            ));
        }
        if lyrics.plain.is_some() {
            review.notes.push("plain text kept".to_owned());
        }
        let translations = answer
            .get("translations")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        if translations > 0 {
            review.notes.push(format!(
                "{} of {translations} translations kept (each needs a language tag and lines)",
                lyrics.translations.len()
            ));
        }
    }
    review
}

/// A chapter answered as a PDF document.
fn pdf_answer(what: &'static str, answer: &Value) -> Review {
    let pdf = manga::parse_pdf(answer);
    let mut review = single(
        what,
        "a pdf: an http(s) address, or { url, headers, firstPage, lastPage }",
        pdf.is_some(),
    );
    if let Some(pdf) = pdf {
        review.notes.push(match (pdf.first_page, pdf.last_page) {
            (None, None) => "a PDF, read as its pages".to_owned(),
            (first, last) => format!(
                "pages {} to {} of a PDF, read as pages",
                first.unwrap_or(1),
                last.map_or_else(|| "its end".to_owned(), |last| last.to_string())
            ),
        });
    }
    review
}

fn passage(answer: &Value) -> Review {
    if answer.get("pdf").is_some() {
        return pdf_answer("passage", answer);
    }
    let blocks = parse_passage(answer);
    let count =
        |wanted: fn(&PassageBlock) -> bool| blocks.iter().filter(|block| wanted(block)).count();
    let mut review = single(
        "passage",
        "html or text with something to read",
        !blocks.is_empty(),
    );
    if !blocks.is_empty() {
        review.notes.push(format!(
            "{} paragraphs, {} headings, {} pictures, {} separators",
            count(|block| matches!(
                block,
                PassageBlock::Paragraph(_) | PassageBlock::Rich(_) | PassageBlock::Item(_)
            )),
            count(|block| matches!(block, PassageBlock::Heading(_))),
            count(|block| matches!(block, PassageBlock::Image(_))),
            count(|block| matches!(block, PassageBlock::Separator)),
        ));
        let math = |parts: &[crate::novel::Part]| {
            parts
                .iter()
                .filter(|part| matches!(part, crate::novel::Part::Math(_)))
                .count()
        };
        let formulas = blocks
            .iter()
            .map(|block| match block {
                PassageBlock::Math(_) => 1,
                PassageBlock::Rich(parts) => math(parts),
                PassageBlock::Item(item) => math(&item.parts),
                PassageBlock::Table(table) => table
                    .rows
                    .iter()
                    .flatten()
                    .map(|cell| math(&cell.parts))
                    .sum(),
                _ => 0,
            })
            .sum::<usize>();
        let items = count(|block| matches!(block, PassageBlock::Item(_)));
        if items > 0 {
            review
                .notes
                .push(format!("{items} of them list items or indented"));
        }
        let tables: Vec<String> = blocks
            .iter()
            .filter_map(|block| match block {
                PassageBlock::Table(table) => Some(format!(
                    "{}×{}",
                    table.rows.len(),
                    table.rows.iter().map(Vec::len).max().unwrap_or(0)
                )),
                _ => None,
            })
            .collect();
        if !tables.is_empty() {
            review.notes.push(format!(
                "{} tables of data (rows × cells: {})",
                tables.len(),
                tables.join(", ")
            ));
        }
        if formulas > 0 {
            review.notes.push(format!(
                "{formulas} formulas (MathML or TeX), drawn as MathML"
            ));
        }
        let code = count(|block| matches!(block, PassageBlock::Code(_)));
        if code > 0 {
            review
                .notes
                .push(format!("{code} code blocks (<pre>), kept line by line"));
        }
    }
    review
}

fn comments(answer: &Value) -> Review {
    let given = answer
        .get("comments")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let (kept, notes) = match serde_json::from_value::<raygun_model::VideoComments>(answer.clone())
    {
        Ok(page) => (
            page.comments
                .iter()
                .filter(|comment| !comment.text.trim().is_empty())
                .count(),
            Vec::new(),
        ),
        Err(error) => (
            0,
            vec![format!(
                "Raygun can't read the page, so it shows none: {error}"
            )],
        ),
    };
    Review {
        what: "comments",
        one: false,
        given,
        kept,
        dropped: Vec::new(),
        needs: COMMENT,
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_are_read_one_by_one() {
        let review = review(
            MUSIC_METADATA,
            "search",
            &json!({ "tracks": [
                { "title": "Kept", "source": { "kind": "url", "url": "https://a.example/1.mp3", "headers": {} } },
                { "title": "No headers", "source": { "kind": "url", "url": "https://a.example/2.mp3" } },
                { "artist": "No title" },
                { "title": "Local", "source": { "kind": "localFile", "path": "/etc/passwd" }, "artworkUrl": "file:///x.png" },
            ]}),
        )
        .unwrap();
        assert_eq!((review.given, review.kept), (4, 2));
        // A source without headers sinks the whole track; a local file is only left out.
        assert_eq!(review.dropped, [1, 2]);
        let summary = review.summary();
        assert!(
            summary.starts_with("Raygun keeps 2 of 4 tracks; it drops entries 1, 2"),
            "{summary}"
        );
        assert!(summary.contains("1 without their artworkUrl"), "{summary}");
    }

    #[test]
    fn catalog_answers_count_items_chapters_and_details() {
        let items = review(
            MANGA_SOURCE,
            "popular",
            &json!({ "items": [{ "id": 1, "title": "A" }, { "id": "", "title": "B" }] }),
        )
        .unwrap();
        assert_eq!((items.kept, items.dropped.clone()), (1, vec![1]));
        assert_eq!(
            items.notes,
            ["no hasNextPage: Raygun reads it as the last page"]
        );
        let chapters = review(
            NOVEL_SOURCE,
            "chapters",
            &json!({ "chapters": [{ "id": "1", "number": 1 }, { "id": "2" }] }),
        )
        .unwrap();
        assert_eq!(chapters.dropped, [1], "neither a name nor a number");
        let details = review(VIDEO_SOURCE, "details", &json!({ "id": "x" })).unwrap();
        assert_eq!(
            details.summary(),
            "Raygun can't use the details: it needs a title"
        );
        let missing = review(MANGA_SOURCE, "chapters", &json!({ "chapter": [] })).unwrap();
        assert_eq!(missing.notes, ["the answer has no `chapters` list"]);
    }

    #[test]
    fn repeated_episodes_count_once() {
        let episodes = review(
            VIDEO_SOURCE,
            "episodes",
            &json!({ "episodes": [{ "id": "a", "name": "Film" }, { "id": "a", "name": "Film" }] }),
        )
        .unwrap();
        assert_eq!((episodes.given, episodes.kept), (2, 1));
        assert!(episodes.dropped.is_empty());
        assert!(episodes.summary().contains("the rest repeat an entry"));
    }

    #[test]
    fn single_answers_and_unknown_calls() {
        let lyrics = review(LYRICS, "lyrics", &Value::Null).unwrap();
        assert_eq!(
            lyrics.summary(),
            "Raygun has no lyrics to show\n  no lyrics for this song (null)"
        );
        let synced = review(
            LYRICS,
            "lyrics",
            &json!({ "synced": [{ "timeMs": 0, "text": "One" }, { "timeMs": -1, "text": "x" }] }),
        )
        .unwrap();
        assert_eq!(synced.kept, 1);
        assert_eq!(
            synced.notes[0],
            "1 of 2 synced lines kept (each needs a timeMs of 0 or more and a text)"
        );
        let stream = review(
            MUSIC_STREAM,
            "resolve",
            &json!({ "source": { "kind": "youTube", "videoId": "abc" } }),
        )
        .unwrap();
        assert_eq!(stream.summary(), "Raygun reads the stream");
        let passage = review(
            NOVEL_SOURCE,
            "passage",
            &json!({ "html": "<h2>One</h2><p>Text</p>" }),
        )
        .unwrap();
        assert_eq!(
            passage.notes,
            ["1 paragraphs, 1 headings, 0 pictures, 0 separators"]
        );
        let comments = review(
            VIDEO_SOURCE,
            "comments",
            &json!({ "comments": [{ "text": "no author" }] }),
        )
        .unwrap();
        assert!(comments.notes[0].starts_with("Raygun can't read the page"));
        let unknown = review(MUSIC_INFO, "artist", &json!({ "artist": null })).unwrap();
        assert!(
            unknown
                .summary()
                .starts_with("Raygun has no artist info to show")
        );
        assert!(review(MANGA_SOURCE, "passage", &json!({})).is_none());
        assert!(review("music.unknown", "search", &json!({})).is_none());
    }

    #[test]
    fn podcasts_and_books_use_their_own_lists() {
        let podcasts = review(
            PODCAST_DIRECTORY,
            "top",
            &json!([{ "title": "A", "feedUrl": "https://a.example/feed" }, { "title": "B" }]),
        )
        .unwrap();
        assert_eq!((podcasts.kept, podcasts.dropped.clone()), (1, vec![1]));
        let books = review(
            AUDIOBOOK_DIRECTORY,
            "chapters",
            &json!({ "chapters": [{ "title": "One", "audioUrl": "https://a.example/1.mp3" }] }),
        )
        .unwrap();
        assert_eq!(books.kept, 1);
        assert_eq!(
            books.notes,
            ["no book: it needs a title (and chapters with audio)"]
        );
    }

    #[test]
    fn every_call_the_sdk_lists_is_reviewed() {
        // `raygun-ext` reads the calls of each kind from the SDK's table; every one of them
        // should be a call the host makes.
        let kinds: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/sdk/lib/kinds.json"
        )))
        .unwrap();
        let kinds = kinds.as_object().unwrap();
        // Fourteen kinds of providers, and site scripts (`crate::site`), which have no calls.
        assert_eq!(kinds.len(), 15);
        assert!(
            kinds["site.scripts"]["required"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        for (kind, info) in kinds {
            for list in ["required", "optional"] {
                for method in info[list].as_array().unwrap() {
                    let method = method.as_str().unwrap();
                    assert!(
                        review(kind, method, &json!({})).is_some(),
                        "{kind} {method} isn't reviewed"
                    );
                    assert!(
                        info["args"].get(method).is_some(),
                        "{kind} {method} has no arguments in kinds.json"
                    );
                }
            }
        }
    }

    #[test]
    fn limits_follow_the_app() {
        assert_eq!(time_limit(MUSIC_METADATA, "search"), INTERACTIVE_TIMEOUT);
        assert_eq!(time_limit(MUSIC_METADATA, "album"), CALL_TIMEOUT);
        assert_eq!(time_limit(MANGA_SOURCE, "search"), CATALOG_SEARCH_TIMEOUT);
        assert_eq!(time_limit(MANGA_SOURCE, "pages"), CALL_TIMEOUT);
        assert_eq!(
            time_limit(AUDIOBOOK_DIRECTORY, "chapters"),
            INTERACTIVE_TIMEOUT
        );
    }
}
