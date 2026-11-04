//! OPDS catalogues (OPDS 1.2, specs.opds.io/opds-1.2): the Atom feeds Komga, Kavita, Calibre and
//! many libraries serve. Comics stream page by page through the Page Streaming Extension
//! (OPDS-PSE 1.1, vaemendis.net/opds-pse): each publication's `pse/stream` link names its pages
//! by number. A catalogue becomes a `manga.source`:
//!
//! - lists are the root's sections: the one for popular titles (`opds-spec.org/sort/popular`,
//!   else one named "All series" and the like), the one for new ones (`sort/new`, "Latest"...),
//!   and every section as the `section` filter;
//! - a navigation entry (a series, a folder) is a title whose publications are its chapters, and
//!   a publication listed on its own is a title of one chapter;
//! - search is the catalogue's OpenSearch template.
//!
//! The same catalogue is a `novel.source` for its EPUB books: a book's chapters are its own
//! (`super::epub`), read from the file, which is kept on this device once fetched.
//!
//! Requests sign in with HTTP Basic when the catalogue needs it.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use reqwest::Url;
use serde_json::{Value, json};
use sha2::Digest;

use super::http::{Answer, Http, MAX_ANSWER_BYTES, date_ms, id_arg, number_arg, text_arg};
use crate::failure::Failure;

/// Pages of one feed followed for a title's chapters (a long series), or to find an entry.
const MAX_FEED_PAGES: usize = 50;
/// The width asked for where a page link takes one (`{maxWidth}`).
const PAGE_WIDTH: &str = "1600";

const REL_PSE: &str = "http://vaemendis.net/opds-pse/stream";
const REL_POPULAR: &str = "http://opds-spec.org/sort/popular";
const REL_NEW: &str = "http://opds-spec.org/sort/new";

/// A link of a feed or an entry, its address resolved against the feed's.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Link {
    pub rel: String,
    pub href: String,
    pub kind: String,
    pub title: Option<String>,
    /// `pse:count`: the publication's pages.
    pub count: Option<u32>,
    /// The file's size, when the catalogue says (`length`).
    pub length: Option<u64>,
}

impl Link {
    /// A link to another catalogue feed (not an entry document).
    fn is_feed(&self) -> bool {
        let kind = self.kind.to_ascii_lowercase();
        kind.contains("application/atom+xml")
            && !kind.contains("type=entry")
            && !self.rel.starts_with("http://opds-spec.org/acquisition")
            && !matches!(
                self.rel.as_str(),
                "self" | "start" | "up" | "search" | "next" | "previous" | "first" | "last"
            )
    }

    fn is_image(&self) -> bool {
        matches!(
            self.rel.as_str(),
            "http://opds-spec.org/image"
                | "http://opds-spec.org/cover"
                | "x-stanza-cover-image"
                | "http://opds-spec.org/image/thumbnail"
                | "http://opds-spec.org/thumbnail"
                | "x-stanza-cover-image-thumbnail"
        )
    }

    fn is_thumbnail(&self) -> bool {
        self.rel.contains("thumbnail")
    }
}

/// One entry of a feed.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Entry {
    pub id: String,
    pub title: String,
    pub updated: Option<String>,
    pub authors: Vec<String>,
    pub summary: Option<String>,
    pub categories: Vec<String>,
    pub links: Vec<Link>,
}

impl Entry {
    /// Where the entry leads, when it's a section, a series or a folder.
    pub fn feed(&self) -> Option<&Link> {
        if self.pages().is_some() || self.acquisitions().next().is_some() {
            return None;
        }
        self.links.iter().find(|link| link.is_feed())
    }

    /// The page stream of a comic: its address template and page count.
    pub fn pages(&self) -> Option<(&str, u32)> {
        self.links
            .iter()
            .find(|link| link.rel == REL_PSE)
            .and_then(|link| Some((link.href.as_str(), link.count.filter(|count| *count > 0)?)))
    }

    pub fn acquisitions(&self) -> impl Iterator<Item = &Link> {
        self.links
            .iter()
            .filter(|link| link.rel.starts_with("http://opds-spec.org/acquisition"))
    }

    /// The book's EPUB file: the smallest, when the catalogue offers several (Gutenberg's
    /// versions with and without pictures).
    pub fn epub(&self) -> Option<&str> {
        self.acquisitions()
            .filter(|link| {
                link.kind
                    .to_ascii_lowercase()
                    .starts_with("application/epub+zip")
            })
            .min_by_key(|link| link.length.unwrap_or(u64::MAX))
            .map(|link| link.href.as_str())
    }

    /// Whether `medium` can read the publication.
    fn readable_as(&self, medium: Medium) -> bool {
        match medium {
            Medium::Comics => self.pages().is_some(),
            Medium::Books => self.epub().is_some(),
        }
    }

    /// The entry's cover: the full image first, for title pages; `thumbnail` for lists.
    pub fn image(&self, thumbnail: bool) -> Option<&str> {
        let mut images: Vec<&Link> = self.links.iter().filter(|link| link.is_image()).collect();
        images.sort_by_key(|link| link.is_thumbnail() != thumbnail);
        images.first().map(|link| link.href.as_str())
    }
}

/// A feed, read.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Feed {
    pub title: String,
    pub subtitle: Option<String>,
    pub links: Vec<Link>,
    pub entries: Vec<Entry>,
}

impl Feed {
    pub fn link(&self, rel: &str) -> Option<&Link> {
        self.links.iter().find(|link| link.rel == rel)
    }
}

/// Reads an OPDS 1.x (Atom) feed fetched from `url`; `None` when the text isn't a feed.
pub(crate) fn parse_feed(text: &str, url: &Url) -> Option<Feed> {
    let mut reader = Reader::from_str(text);
    let mut buffer = Vec::new();
    let mut path: Vec<String> = Vec::new();
    let mut feed = Feed::default();
    let mut entry: Option<Entry> = None;
    let mut text = String::new();
    let mut is_feed = false;
    // Inside a summary or a content element: its XHTML's elements add to its text.
    let mut inside_text: usize = 0;
    loop {
        let event = reader.read_event_into(&mut buffer).ok()?;
        match event {
            Event::Start(element) => {
                let name = local_name(&element);
                if inside_text > 0 {
                    inside_text += 1;
                    text.push(' ');
                    path.push(name);
                    continue;
                }
                if name == "feed" && path.is_empty() {
                    is_feed = true;
                }
                if name == "entry" {
                    entry = Some(Entry::default());
                }
                on_element(&element, &name, url, &mut feed, entry.as_mut());
                if matches!(name.as_str(), "summary" | "content") {
                    inside_text = 1;
                }
                path.push(name);
                text.clear();
            }
            Event::Empty(element) => {
                let name = local_name(&element);
                on_element(&element, &name, url, &mut feed, entry.as_mut());
            }
            Event::Text(content) => text.push_str(&content),
            Event::CData(content) => text.push_str(&content),
            Event::GeneralRef(reference) => {
                let escaped = format!("&{};", &*reference);
                text.push_str(
                    &quick_xml::escape::unescape(&escaped)
                        .map(std::borrow::Cow::into_owned)
                        .unwrap_or(escaped),
                );
            }
            Event::End(_) => {
                let name = path.pop().unwrap_or_default();
                if inside_text > 1 {
                    inside_text -= 1;
                    continue;
                }
                inside_text = 0;
                let value = std::mem::take(&mut text).trim().to_owned();
                let parent = path.last().map(String::as_str).unwrap_or_default();
                if name == "entry" {
                    if let Some(done) = entry.take() {
                        feed.entries.push(done);
                    }
                    continue;
                }
                if value.is_empty() {
                    continue;
                }
                match (entry.as_mut(), name.as_str(), parent) {
                    (Some(entry), "title", "entry") => entry.title = value,
                    (Some(entry), "id", "entry") => entry.id = value,
                    (Some(entry), "updated" | "published" | "issued", "entry") => {
                        entry.updated.get_or_insert(value);
                    }
                    (Some(entry), "name", "author") => entry.authors.push(value),
                    (Some(entry), "summary" | "content", "entry") => {
                        if entry
                            .summary
                            .as_ref()
                            .is_none_or(|old| old.len() < value.len())
                        {
                            entry.summary = Some(plain_text(&value));
                        }
                    }
                    (None, "title", "feed") => feed.title = value,
                    (None, "subtitle", "feed") => feed.subtitle = Some(value),
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    is_feed.then_some(feed)
}

/// An element's local name, lower case (`atom:link` → `link`).
fn local_name(element: &BytesStart<'_>) -> String {
    let name = element.name().as_ref().to_ascii_lowercase();
    match name.rsplit_once(':') {
        Some((_, local)) => local.to_owned(),
        None => name,
    }
}

/// An attribute by local name (`pse:count` is `count`).
fn attribute(element: &BytesStart<'_>, key: &str) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attribute| {
            let name = attribute.key.as_ref().to_ascii_lowercase();
            name.rsplit(':').next() == Some(key)
        })
        .and_then(|attribute| {
            attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .ok()
                .map(|value| value.trim().to_owned())
        })
        .filter(|value| !value.is_empty())
}

fn on_element(
    element: &BytesStart<'_>,
    name: &str,
    url: &Url,
    feed: &mut Feed,
    entry: Option<&mut Entry>,
) {
    match name {
        "link" => {
            let Some(href) = attribute(element, "href") else {
                return;
            };
            // Templates keep their braces: `{pageNumber}` must survive the join.
            let Some(href) = resolve(url, &href) else {
                return;
            };
            let link = Link {
                rel: attribute(element, "rel").unwrap_or_default(),
                href,
                kind: attribute(element, "type").unwrap_or_default(),
                title: attribute(element, "title"),
                count: attribute(element, "count").and_then(|count| count.parse().ok()),
                length: attribute(element, "length").and_then(|length| length.parse().ok()),
            };
            match entry {
                Some(entry) => entry.links.push(link),
                None => feed.links.push(link),
            }
        }
        "category" => {
            if let Some(entry) = entry
                && let Some(label) =
                    attribute(element, "label").or_else(|| attribute(element, "term"))
            {
                entry.categories.push(label);
            }
        }
        _ => {}
    }
}

/// `href` against the feed's address, keeping template braces as they are; http(s) only.
fn resolve(base: &Url, href: &str) -> Option<String> {
    let joined = base.join(href).ok()?;
    if !matches!(joined.scheme(), "http" | "https") {
        return None;
    }
    // `Url` escapes `{` and `}`; templates need them back.
    Some(joined.to_string().replace("%7B", "{").replace("%7D", "}"))
}

/// HTML summaries as plain text (Calibre writes HTML in `content`).
fn plain_text(text: &str) -> String {
    if !text.contains('<') {
        return text.to_owned();
    }
    let fragment = scraper::Html::parse_fragment(text);
    let words: Vec<&str> = fragment.root_element().text().collect();
    words
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Where a catalogue's lists are, found once from its root.
#[derive(Debug, Clone, Default)]
struct Sections {
    popular: Option<String>,
    latest: Option<String>,
    /// The OpenSearch template, with `{searchTerms}`.
    search: Option<String>,
    /// Every section of the root, for the `section` filter.
    all: Vec<(String, String)>,
}

/// What a provider of the catalogue reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Medium {
    Comics,
    Books,
}

/// Books fetched lately, kept in memory for their next chapter.
const BOOKS_IN_MEMORY: usize = 3;
/// Books kept on this device.
const BOOKS_ON_DISK: usize = 40;
/// Largest book fetched.
const MAX_BOOK_BYTES: usize = 64 * 1024 * 1024;
/// Books of one series read for its chapters.
const MAX_SERIES_BOOKS: usize = 12;

/// An OPDS catalogue.
pub(crate) struct Opds {
    http: Http,
    root: Url,
    /// `Authorization: Basic …`, when the catalogue needs a sign-in.
    basic: Option<String>,
    sections: RefCell<Option<Sections>>,
    /// The address of a feed's later pages, by feed and page, as `next` links gave them.
    pages: RefCell<HashMap<(String, u32), String>>,
    /// Where fetched books are kept (the connection's data folder); `None` keeps them in memory
    /// only.
    books_dir: Option<PathBuf>,
    /// Books fetched lately, newest last.
    books: RefCell<Vec<(String, Rc<Vec<u8>>)>>,
}

/// `Authorization` for HTTP Basic.
pub(crate) fn basic_auth(username: &str, password: &str) -> String {
    use base64::Engine;
    let encoded =
        base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
    format!("Basic {encoded}")
}

/// Fetches and reads a feed; a page that isn't one says so.
pub(crate) async fn fetch_feed(
    http: &Http,
    url: &Url,
    basic: Option<&str>,
) -> Result<(Feed, Answer), Failure> {
    let mut request = http.client().get(url.clone()).header(
        reqwest::header::ACCEPT,
        "application/atom+xml, application/xml;q=0.9, */*;q=0.5",
    );
    if let Some(basic) = basic {
        request = request.header(reqwest::header::AUTHORIZATION, basic);
    }
    let answer = http.send(request, MAX_ANSWER_BYTES).await?;
    let text = answer.text();
    let feed = parse_feed(&text, &answer.url).ok_or_else(|| {
        let what = if answer.content_type.contains("json") {
            "it's an OPDS 2 catalogue; Raygun reads OPDS 1 feeds, which most servers also serve"
        } else {
            "it isn't an OPDS feed"
        };
        super::http::unreadable(url.host_str().unwrap_or_default(), &what)
    })?;
    Ok((feed, answer))
}

/// Text for OpenSearch's `{searchTerms}`; the template's optional parameters (`{startPage?}`)
/// are dropped.
fn fill_template(template: &str, query: &str) -> String {
    let terms: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    let mut filled = template.replace("{searchTerms}", &terms);
    while let (Some(start), Some(end)) = (filled.find('{'), filled.find('}')) {
        if end < start {
            break;
        }
        filled.replace_range(start..=end, "");
    }
    filled
}

/// Words that name a catalogue's list of every title, and of the newest.
const ALL_WORDS: [&str; 7] = [
    "all series",
    "all comics",
    "all books",
    "all titles",
    "all publications",
    "series",
    "all",
];
const NEW_WORDS: [&str; 5] = ["latest", "recently added", "recent", "newest", "new"];

impl Opds {
    pub fn new(http: Http, root: Url, basic: Option<String>, data_dir: Option<PathBuf>) -> Self {
        Self {
            http,
            root,
            basic,
            sections: RefCell::new(None),
            pages: RefCell::new(HashMap::new()),
            books_dir: data_dir.map(|dir| dir.join("books")),
            books: RefCell::new(Vec::new()),
        }
    }

    /// The sign-in for a request to `url`: only the catalogue's own host gets it. Other public
    /// sites (a catalogue's mirrors) are reached without; anything else isn't reached.
    fn sign_for(&self, url: &Url) -> Result<Option<&str>, Failure> {
        let host = url.host_str().unwrap_or_default();
        if url.host_str() == self.root.host_str() {
            Ok(self.basic.as_deref())
        } else if matches!(url.scheme(), "http" | "https")
            && crate::permissions::is_public_host(host)
        {
            Ok(None)
        } else {
            Err(Failure::said(format!(
                "the catalogue named an address Raygun won't follow ({host})"
            )))
        }
    }

    async fn feed(&self, url: &str) -> Result<Feed, Failure> {
        let url = Url::parse(url)
            .map_err(|error| Failure::said(format!("invalid feed address: {error}")))?;
        let sign = self.sign_for(&url)?;
        fetch_feed(&self.http, &url, sign)
            .await
            .map(|(feed, _)| feed)
    }

    async fn sections(&self) -> Result<Sections, Failure> {
        if let Some(sections) = self.sections.borrow().clone() {
            return Ok(sections);
        }
        let root = self.feed(self.root.as_str()).await?;
        let mut sections = Sections::default();
        let navigation: Vec<(String, &Link)> = root
            .entries
            .iter()
            .filter_map(|entry| Some((entry.title.clone(), entry.feed()?)))
            .collect();
        sections.all = navigation
            .iter()
            .map(|(title, link)| (link.href.clone(), title.clone()))
            .collect();
        let by_rel = |rel: &str| {
            root.link(rel).map(|link| link.href.clone()).or_else(|| {
                navigation
                    .iter()
                    .find(|(_, link)| link.rel == rel)
                    .map(|(_, link)| link.href.clone())
            })
        };
        let by_words = |words: &[&str]| {
            words.iter().find_map(|word| {
                navigation
                    .iter()
                    .find(|(title, _)| {
                        let title = title.to_lowercase();
                        title == *word
                            || title.starts_with(&format!("{word} "))
                            || title.contains(&format!(" {word}"))
                    })
                    .map(|(_, link)| link.href.clone())
            })
        };
        sections.popular = by_rel(REL_POPULAR)
            .or_else(|| by_words(&["popular", "most read"]))
            .or_else(|| by_words(&ALL_WORDS));
        sections.latest = by_rel(REL_NEW).or_else(|| by_words(&NEW_WORDS));
        sections.search = match root.link("search") {
            Some(link) if link.href.contains("{searchTerms}") => Some(link.href.clone()),
            Some(link) if link.kind.contains("opensearchdescription") => {
                self.search_template(&link.href).await
            }
            _ => None,
        };
        *self.sections.borrow_mut() = Some(sections.clone());
        Ok(sections)
    }

    /// The Atom search template an OpenSearch description gives.
    async fn search_template(&self, description: &str) -> Option<String> {
        let url = Url::parse(description).ok()?;
        let sign = self.sign_for(&url).ok()?;
        let mut request = self.http.client().get(url.clone());
        if let Some(basic) = sign {
            request = request.header(reqwest::header::AUTHORIZATION, basic);
        }
        let answer = self.http.send(request, MAX_ANSWER_BYTES).await.ok()?;
        let text = answer.text();
        let mut reader = Reader::from_str(&text);
        let mut buffer = Vec::new();
        let mut templates: Vec<(String, String)> = Vec::new();
        loop {
            match reader.read_event_into(&mut buffer).ok()? {
                Event::Start(element) | Event::Empty(element) if local_name(&element) == "url" => {
                    if let Some(template) = attribute(&element, "template") {
                        templates.push((attribute(&element, "type").unwrap_or_default(), template));
                    }
                }
                Event::Eof => break,
                _ => {}
            }
            buffer.clear();
        }
        let (_, template) = templates
            .iter()
            .find(|(kind, _)| kind.contains("atom"))
            .or_else(|| templates.first())?;
        resolve(&answer.url, template)
    }

    /// Page `page` of the feed at `url`, following `next` links from the first.
    async fn feed_page(&self, url: &str, page: u32) -> Result<Feed, Failure> {
        let mut known = (1u32, url.to_owned());
        for earlier in (2..=page).rev() {
            if let Some(found) = self.pages.borrow().get(&(url.to_owned(), earlier)) {
                known = (earlier, found.clone());
                break;
            }
        }
        let (mut at, mut address) = known;
        loop {
            let feed = self.feed(&address).await?;
            if at >= page {
                return Ok(feed);
            }
            let Some(next) = feed.link("next").map(|link| link.href.clone()) else {
                return Ok(Feed::default());
            };
            at += 1;
            self.pages
                .borrow_mut()
                .insert((url.to_owned(), at), next.clone());
            address = next;
        }
    }

    /// Every entry of the feed at `url`, over its pages (at most [`MAX_FEED_PAGES`]).
    async fn all_entries(&self, url: &str) -> Result<(Feed, Vec<Entry>), Failure> {
        let first = self.feed(url).await?;
        let mut entries = first.entries.clone();
        let mut next = first.link("next").map(|link| link.href.clone());
        let mut pages = 1;
        while let Some(address) = next.take() {
            if pages >= MAX_FEED_PAGES {
                break;
            }
            let feed = self.feed(&address).await?;
            entries.extend(feed.entries.iter().cloned());
            next = feed.link("next").map(|link| link.href.clone());
            pages += 1;
        }
        Ok((first, entries))
    }

    /// A feed's entries as titles: sections and series lead to more; publications `medium`
    /// reads are titles of their own.
    fn listing(&self, feed: &Feed, feed_url: &str, medium: Medium) -> Value {
        let items: Vec<Value> = feed
            .entries
            .iter()
            .filter_map(|entry| {
                let id = if let Some(link) = entry.feed() {
                    format!("feed:{}", link.href)
                } else if entry.readable_as(medium) && !entry.id.is_empty() {
                    format!("entry:{feed_url}#{}", entry.id)
                } else {
                    return None;
                };
                Some(json!({ "id": id, "title": entry.title, "coverUrl": entry.image(true) }))
            })
            .collect();
        json!({ "items": items, "hasNextPage": feed.link("next").is_some() })
    }

    pub async fn comics_call(&self, method: &str, args: &Value) -> Result<Value, Failure> {
        match method {
            "chapters" => self.chapters(&id_arg(args, "id")?).await,
            "pages" => {
                let chapter = id_arg(args, "chapterId")?;
                let (count, template) = chapter
                    .strip_prefix("pse:")
                    .and_then(|rest| rest.split_once(':'))
                    .and_then(|(count, template)| Some((count.parse::<u32>().ok()?, template)))
                    .ok_or_else(|| Failure::said("this chapter isn't a comic Raygun can stream"))?;
                let pages: Vec<String> = (0..count.min(5000))
                    .map(|number| {
                        template
                            .replace("{pageNumber}", &number.to_string())
                            .replace("{maxWidth}", PAGE_WIDTH)
                    })
                    .collect();
                Ok(json!({ "pages": pages }))
            }
            other => self.catalog_call(other, args, Medium::Comics).await,
        }
    }

    pub async fn books_call(&self, method: &str, args: &Value) -> Result<Value, Failure> {
        match method {
            "chapters" => self.book_chapters(&id_arg(args, "id")?).await,
            "passage" => {
                let chapter = id_arg(args, "chapterId")?;
                let (path, href) = chapter
                    .strip_prefix("epub:")
                    .and_then(|rest| rest.split_once('|'))
                    .ok_or_else(|| Failure::said("this chapter isn't a book Raygun can read"))?;
                let bytes = self.book(href).await?;
                let (path, owned) = (path.to_owned(), (*bytes).clone());
                let html = run_blocking(move || super::epub::passage(owned, &path)).await?;
                Ok(json!({ "html": html }))
            }
            other => self.catalog_call(other, args, Medium::Books).await,
        }
    }

    /// The calls both kinds share: lists, search, filters and details.
    async fn catalog_call(
        &self,
        method: &str,
        args: &Value,
        medium: Medium,
    ) -> Result<Value, Failure> {
        let page = number_arg(args, "page", 1, 1..=10_000);
        match method {
            "popular" | "latest" => {
                let sections = self.sections().await?;
                let wanted = if method == "latest" {
                    sections.latest.clone()
                } else {
                    sections.popular.clone()
                };
                let url = wanted.unwrap_or_else(|| self.root.to_string());
                let feed = self.feed_page(&url, page).await?;
                Ok(self.listing(&feed, &url, medium))
            }
            "search" => {
                let query = text_arg(args, "query");
                let section = args
                    .get("filters")
                    .and_then(|filters| filters.get("section"))
                    .and_then(Value::as_str)
                    .filter(|section| !section.is_empty() && *section != "default")
                    .map(str::to_owned);
                let sections = self.sections().await?;
                let url = match (query, section) {
                    (Some(query), _) => {
                        // A catalogue without search finds nothing, quietly: it's asked with
                        // every source for each search.
                        let Some(template) = &sections.search else {
                            return Ok(json!({ "items": [], "hasNextPage": false }));
                        };
                        fill_template(template, &query)
                    }
                    (None, Some(section))
                        if sections.all.iter().any(|(href, _)| *href == section) =>
                    {
                        section
                    }
                    _ => sections
                        .popular
                        .clone()
                        .unwrap_or_else(|| self.root.to_string()),
                };
                let feed = self.feed_page(&url, page).await?;
                Ok(self.listing(&feed, &url, medium))
            }
            "filters" => {
                let sections = self.sections().await?;
                if sections.all.is_empty() {
                    return Ok(json!([]));
                }
                let mut options = vec![json!({ "value": "default", "label": "Everything" })];
                options.extend(
                    sections
                        .all
                        .iter()
                        .map(|(href, title)| json!({ "value": href, "label": title })),
                );
                Ok(
                    json!([{ "kind": "select", "key": "section", "label": "Section", "options": options, "default": "default" }]),
                )
            }
            "details" => self.details(&id_arg(args, "id")?).await,
            other => Err(Failure::said(format!(
                "this catalogue has no method {other}"
            ))),
        }
    }

    /// The entry a title id names (`entry:<feed>#<entry id>`), looked for over the feed's pages.
    async fn find_entry(&self, id: &str) -> Result<(Entry, String), Failure> {
        let rest = id.strip_prefix("entry:").unwrap_or(id);
        let (feed_url, entry_id) = rest
            .rsplit_once('#')
            .ok_or_else(|| Failure::said("unknown title"))?;
        let (_, entries) = self.all_entries(feed_url).await?;
        entries
            .into_iter()
            .find(|entry| entry.id == entry_id)
            .map(|entry| (entry, feed_url.to_owned()))
            .ok_or_else(|| Failure::said("This title is no longer in the catalogue"))
    }

    async fn details(&self, id: &str) -> Result<Value, Failure> {
        if let Some(url) = id.strip_prefix("feed:") {
            let feed = self.feed(url).await?;
            let first = feed
                .entries
                .iter()
                .find(|entry| entry.pages().is_some() || entry.acquisitions().next().is_some());
            let mut genres: Vec<String> = Vec::new();
            for category in feed.entries.iter().flat_map(|entry| &entry.categories) {
                if !genres.contains(category) {
                    genres.push(category.clone());
                }
            }
            // A book's summary says more than the catalogue's own tagline.
            let description = first
                .and_then(|entry| entry.summary.clone())
                .filter(|summary| !is_file_note(summary))
                .or_else(|| feed.subtitle.clone());
            return Ok(json!({
                "id": id,
                "title": feed.title,
                "coverUrl": first.and_then(|entry| entry.image(false)),
                "authors": first.map(|entry| entry.authors.clone()).unwrap_or_default(),
                "description": description,
                "genres": genres,
                "status": "unknown",
            }));
        }
        let (entry, _) = self.find_entry(id).await?;
        Ok(json!({
            "id": id,
            "title": entry.title,
            "coverUrl": entry.image(false),
            "authors": entry.authors,
            "description": entry.summary.filter(|summary| !is_file_note(summary)),
            "genres": entry.categories,
            "status": "unknown",
        }))
    }

    /// The publications a title stands for: a series' (over its feed's pages), or itself.
    async fn publications(&self, id: &str) -> Result<Vec<Entry>, Failure> {
        if let Some(url) = id.strip_prefix("feed:") {
            Ok(self.all_entries(url).await?.1)
        } else {
            Ok(vec![self.find_entry(id).await?.0])
        }
    }

    /// A book's file, from memory, this device, or the catalogue.
    async fn book(&self, href: &str) -> Result<Rc<Vec<u8>>, Failure> {
        if let Some((_, bytes)) = self.books.borrow().iter().find(|(url, _)| url == href) {
            return Ok(bytes.clone());
        }
        let file = self.books_dir.as_ref().map(|dir| {
            let digest = sha2::Sha256::digest(href.as_bytes());
            let name: String = digest
                .iter()
                .take(10)
                .map(|byte| format!("{byte:02x}"))
                .collect();
            dir.join(format!("{name}.epub"))
        });
        let kept = match &file {
            Some(path) => tokio::fs::read(path).await.ok(),
            None => None,
        };
        let bytes = match kept {
            Some(bytes) => bytes,
            None => {
                let url = Url::parse(href)
                    .map_err(|error| Failure::said(format!("invalid book address: {error}")))?;
                let sign = self.sign_for(&url)?;
                let mut request = self.http.client().get(url);
                if let Some(basic) = sign {
                    request = request.header(reqwest::header::AUTHORIZATION, basic);
                }
                let bytes = self.http.send(request, MAX_BOOK_BYTES).await?.body;
                if let Some(path) = &file {
                    keep_book(path, &bytes).await;
                }
                bytes
            }
        };
        let bytes = Rc::new(bytes);
        let mut books = self.books.borrow_mut();
        books.push((href.to_owned(), bytes.clone()));
        if books.len() > BOOKS_IN_MEMORY {
            books.remove(0);
        }
        Ok(bytes)
    }

    /// A title's chapters as a novel source lists them: each book's own, in order (a series of
    /// several books names each chapter after its book).
    async fn book_chapters(&self, id: &str) -> Result<Value, Failure> {
        let books: Vec<Entry> = self
            .publications(id)
            .await?
            .into_iter()
            .filter(|entry| entry.epub().is_some())
            .take(MAX_SERIES_BOOKS)
            .collect();
        let several = books.len() > 1;
        let mut chapters = Vec::new();
        let mut failure = None;
        for entry in &books {
            let href = entry.epub().unwrap_or_default().to_owned();
            let read = match self.book(&href).await {
                Ok(bytes) => {
                    let owned = (*bytes).clone();
                    run_blocking(move || super::epub::read(owned)).await
                }
                Err(error) => Err(error),
            };
            let book = match read {
                Ok(book) => book,
                Err(error) => {
                    failure = Some(error);
                    continue;
                }
            };
            for (index, chapter) in book.chapters.iter().enumerate() {
                let title = chapter
                    .title
                    .clone()
                    .unwrap_or_else(|| format!("Part {}", index + 1));
                let name = if several {
                    format!("{} · {title}", entry.title)
                } else {
                    title
                };
                chapters.push(json!({
                    "id": format!("epub:{}|{href}", chapter.path),
                    "name": name,
                    "number": chapters.len() + 1,
                    "volume": several.then(|| entry.title.clone()),
                }));
            }
        }
        match failure {
            Some(failure) if chapters.is_empty() => Err(failure),
            _ => Ok(json!({ "chapters": chapters })),
        }
    }

    async fn chapters(&self, id: &str) -> Result<Value, Failure> {
        let entries = self.publications(id).await?;
        let chapters: Vec<Value> = entries
            .iter()
            .filter_map(|entry| Some((entry, entry.pages()?)))
            .enumerate()
            .map(|(index, (entry, (template, count)))| {
                json!({
                    "id": format!("pse:{count}:{template}"),
                    "name": entry.title,
                    "number": index + 1,
                    "dateUpload": entry.updated.as_deref().and_then(date_ms),
                })
            })
            .collect();
        Ok(json!({ "chapters": chapters }))
    }
}

/// Whether a summary only describes the file ("cbz - 20.9 MiB", as Komga writes).
fn is_file_note(summary: &str) -> bool {
    let words: Vec<&str> = summary.split_whitespace().collect();
    matches!(words.as_slice(), [format, "-", size, unit]
        if format.chars().all(|c| c.is_ascii_alphanumeric())
            && size.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',')
            && unit.ends_with('B'))
}

/// Keeps a fetched book on this device, making way for it among the oldest kept.
async fn keep_book(path: &std::path::Path, bytes: &[u8]) {
    let Some(dir) = path.parent() else {
        return;
    };
    if tokio::fs::create_dir_all(dir).await.is_err() {
        return;
    }
    if let Ok(mut entries) = tokio::fs::read_dir(dir).await {
        let mut kept = Vec::new();
        while let Ok(Some(entry)) = entries.next_entry().await {
            if let Ok(meta) = entry.metadata().await {
                kept.push((meta.modified().ok(), entry.path()));
            }
        }
        if kept.len() >= BOOKS_ON_DISK {
            kept.sort();
            for (_, old) in kept.iter().take(kept.len() + 1 - BOOKS_ON_DISK) {
                let _ = tokio::fs::remove_file(old).await;
            }
        }
    }
    let partial = path.with_extension("part");
    if tokio::fs::write(&partial, bytes).await.is_ok() {
        let _ = tokio::fs::rename(&partial, path).await;
    }
}

/// Runs reading a book's zip off the extension's lane.
async fn run_blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, Failure> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| Failure::said("reading the book stopped"))?
        .map_err(|message| Failure {
            message,
            trouble: None,
            answered: true,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_notes_are_not_descriptions() {
        assert!(is_file_note("cbz - 20.9 MiB"));
        assert!(is_file_note("epub - 590.7 KiB"));
        assert!(!is_file_note("A scientist travels to the year 802,701."));
    }

    const KOMGA_SERIES: &str = r#"<feed xmlns="http://www.w3.org/2005/Atom"><id>63</id><title>Space Adventures</title>
<link type="application/atom+xml;profile=opds-catalog;kind=navigation" rel="self" href="https://comics.example.org/opds/v1.2/series/63"/>
<entry><title>Space Adventures 01</title><updated>2025-12-15T11:36:12.257+08:00</updated><id>64</id><content>cbz - 20.9 MiB</content>
<link type="image/jpeg" rel="http://opds-spec.org/image/thumbnail" href="/opds/v1.2/books/64/thumbnail/small"/>
<link type="image/jpeg" rel="http://opds-spec.org/image" href="/opds/v1.2/books/64/thumbnail"/>
<link type="application/zip" rel="http://opds-spec.org/acquisition" href="/opds/v1.2/books/64/file/Space_Adventures_01.cbz"/>
<link href="/opds/v1.2/books/64/pages/{pageNumber}" xmlns:pse="http://vaemendis.net/opds-pse/ns" pse:count="35" type="image/jpeg" rel="http://vaemendis.net/opds-pse/stream"/></entry>
<entry><title>Dracula &amp; friends</title><id>65</id><author><name>Bram Stoker</name></author>
<link type="application/epub+zip" rel="http://opds-spec.org/acquisition" href="/opds/v1.2/books/65/file/Dracula.epub"/></entry>
</feed>"#;

    #[test]
    fn reads_publications_and_their_page_streams() {
        let url: Url = "https://comics.example.org/opds/v1.2/series/63"
            .parse()
            .unwrap();
        let feed = parse_feed(KOMGA_SERIES, &url).unwrap();
        assert_eq!(feed.title, "Space Adventures");
        assert_eq!(feed.entries.len(), 2);
        let comic = &feed.entries[0];
        assert_eq!(
            comic.pages(),
            Some((
                "https://comics.example.org/opds/v1.2/books/64/pages/{pageNumber}",
                35
            ))
        );
        assert_eq!(
            comic.image(true),
            Some("https://comics.example.org/opds/v1.2/books/64/thumbnail/small")
        );
        assert_eq!(
            comic.image(false),
            Some("https://comics.example.org/opds/v1.2/books/64/thumbnail")
        );
        assert!(comic.feed().is_none(), "a publication isn't a section");
        let book = &feed.entries[1];
        assert_eq!(book.title, "Dracula & friends");
        assert_eq!(book.authors, ["Bram Stoker"]);
        assert!(book.pages().is_none());
        assert_eq!(book.acquisitions().count(), 1);
    }

    #[test]
    fn sections_are_feeds_and_templates_fill() {
        let root = r#"<feed xmlns="http://www.w3.org/2005/Atom"><title>Catalog</title>
<link type="application/opensearchdescription+xml" rel="search" href="/opds/search"/>
<entry><title>All series</title><id>a</id><link type="application/atom+xml;profile=opds-catalog;kind=navigation" rel="subsection" href="series"/></entry>
<entry><title>Book</title><id>b</id><link type="application/atom+xml;type=entry;profile=opds-catalog" rel="alternate" href="b.xml"/></entry>
</feed>"#;
        let url: Url = "https://books.example.org/opds/".parse().unwrap();
        let feed = parse_feed(root, &url).unwrap();
        assert_eq!(
            feed.entries[0].feed().map(|link| link.href.as_str()),
            Some("https://books.example.org/opds/series")
        );
        assert!(
            feed.entries[1].feed().is_none(),
            "an entry document isn't a feed"
        );
        assert_eq!(
            feed.link("search").unwrap().href,
            "https://books.example.org/opds/search"
        );
        assert_eq!(
            fill_template(
                "https://x.example/s?q={searchTerms}&p={startPage?}&n={count?}",
                "moby dick"
            ),
            "https://x.example/s?q=moby+dick&p=&n="
        );
        assert!(parse_feed("<html><body>Sign in</body></html>", &url).is_none());
        assert!(basic_auth("demo", "pw").starts_with("Basic ZGVtbzpwdw"));
    }
}
