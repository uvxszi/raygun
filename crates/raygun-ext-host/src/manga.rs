//! `manga.source` providers (M24): Mihon-shaped catalogue calls, with every result checked before
//! it reaches the app (ids as strings, http(s) URLs only, known statuses).

use std::collections::BTreeMap;

use raygun_model::{FilterOption, FilterValue, SourceFilter};
use serde_json::Value;

use crate::ExtensionHost;

pub const MANGA_SOURCE: &str = "manga.source";

/// Statuses a source may report; anything else becomes `unknown`.
const STATUSES: [&str; 6] = [
    "ongoing",
    "completed",
    "hiatus",
    "cancelled",
    "publishingFinished",
    "unknown",
];
/// Kinds of video a source may report (M42); anything else is left out.
const MEDIA_TYPES: [&str; 4] = ["movie", "series", "anime", "other"];
/// Headers a source may not set on image requests.
const FORBIDDEN_HEADERS: [&str; 4] = ["host", "content-length", "connection", "transfer-encoding"];

#[derive(Debug, Clone, PartialEq)]
pub struct MangaSummary {
    pub id: String,
    pub title: String,
    pub cover_url: Option<String>,
    /// Sent when the app fetches the cover (some hosts refuse bare requests).
    pub cover_headers: BTreeMap<String, String>,
    /// Marked adult by the source; hidden while NSFW content is off (M35).
    pub nsfw: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MangaListing {
    pub items: Vec<MangaSummary>,
    pub has_next_page: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MangaDetails {
    pub id: String,
    pub title: String,
    pub cover_url: Option<String>,
    pub cover_headers: BTreeMap<String, String>,
    pub authors: Vec<String>,
    pub artists: Vec<String>,
    pub description: Option<String>,
    pub genres: Vec<String>,
    pub status: String,
    /// The title's page on the source's website.
    pub url: Option<String>,
    /// Video sources (M42): `movie`, `series`, `anime` or `other`.
    pub media_type: Option<String>,
    /// Video sources (M42): the release year.
    pub year: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MangaChapter {
    pub id: String,
    pub name: String,
    pub number: Option<f64>,
    pub volume: Option<String>,
    pub scanlator: Option<String>,
    /// Unix ms.
    pub date_upload: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageImage {
    pub url: String,
    pub headers: BTreeMap<String, String>,
}

/// Which catalogue list to ask a source for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MangaList {
    Popular,
    Latest,
}

impl ExtensionHost {
    /// A page of a source's popular or latest titles.
    pub async fn manga_list(
        &self,
        provider: &str,
        list: MangaList,
        page: u32,
    ) -> Result<MangaListing, String> {
        let method = match list {
            MangaList::Popular => "popular",
            MangaList::Latest => "latest",
        };
        let value = self
            .call(provider, method, serde_json::json!({ "page": page }))
            .await
            .map_err(|error| error.to_string())?;
        Ok(parse_listing(&value))
    }

    pub async fn manga_search(
        &self,
        provider: &str,
        query: &str,
        page: u32,
    ) -> Result<MangaListing, String> {
        let value = self
            .call(
                provider,
                "search",
                serde_json::json!({ "query": query, "page": page }),
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(parse_listing(&value))
    }

    /// A search with the source's own filters (M40); an empty query is fine when filters are set.
    pub async fn manga_search_filtered(
        &self,
        provider: &str,
        query: &str,
        page: u32,
        filters: &BTreeMap<String, FilterValue>,
    ) -> Result<MangaListing, String> {
        let value = self
            .call(
                provider,
                "search",
                serde_json::json!({ "query": query, "page": page, "filters": filters }),
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(parse_listing(&value))
    }

    /// A source's first page of results for a search row (M46.48): its popular titles when
    /// `query` is blank. Rows take turns among catalogue searches (good sources first; a slow
    /// one makes way after a moment) and give up after [`crate::CATALOG_SEARCH_TIMEOUT`].
    /// Dropping the future stops the call.
    pub async fn catalog_search_row(
        &self,
        provider: &str,
        query: &str,
    ) -> Result<MangaListing, crate::CallError> {
        let standing = self.health().standing(provider);
        let mut turn = Some(self.catalog_turns().wait(standing).await);
        let (method, args) = if query.is_empty() {
            ("popular", serde_json::json!({ "page": 1 }))
        } else {
            ("search", serde_json::json!({ "query": query, "page": 1 }))
        };
        let started = std::time::Instant::now();
        let mut call =
            std::pin::pin!(self.call_within(provider, method, args, crate::CATALOG_SEARCH_TIMEOUT));
        let answer = tokio::select! {
            answer = &mut call => answer,
            () = tokio::time::sleep(crate::fanout::HOLD) => {
                turn.take();
                call.await
            }
        };
        drop(turn);
        self.health()
            .record(provider, answer.as_ref().map(|_| ()), started.elapsed());
        Ok(parse_listing(&answer?))
    }

    /// The filters a source offers for its search; none when it declares none. A source that has
    /// filters but couldn't list them (a network error) is an error, so the app can ask again.
    pub async fn manga_filters(&self, provider: &str) -> Result<Vec<SourceFilter>, String> {
        match self.call(provider, "filters", serde_json::json!({})).await {
            Ok(value) => Ok(parse_filters(&value)),
            Err(error) if error.is_missing() => Ok(Vec::new()),
            Err(error) => {
                tracing::warn!(%provider, %error, "source filters failed");
                Err(error.to_string())
            }
        }
    }

    pub async fn manga_details(&self, provider: &str, id: &str) -> Result<MangaDetails, String> {
        let value = self
            .call(provider, "details", serde_json::json!({ "id": id }))
            .await
            .map_err(|error| error.to_string())?;
        parse_details(&value, id).ok_or_else(|| {
            raygun_net::Issue::new(
                raygun_net::IssueKind::SiteChanged,
                self.provider_name(provider),
                "its answer had no title",
            )
            .to_string()
        })
    }

    pub async fn manga_chapters(
        &self,
        provider: &str,
        id: &str,
    ) -> Result<Vec<MangaChapter>, String> {
        let value = self
            .call(provider, "chapters", serde_json::json!({ "id": id }))
            .await
            .map_err(|error| error.to_string())?;
        Ok(parse_units(&value, "chapters", "Chapter"))
    }

    pub async fn manga_pages(
        &self,
        provider: &str,
        chapter_id: &str,
    ) -> Result<Vec<PageImage>, String> {
        let value = self
            .call(
                provider,
                "pages",
                serde_json::json!({ "chapterId": chapter_id }),
            )
            .await
            .map_err(|error| error.to_string())?;
        let pages = parse_pages(&value);
        if pages.is_empty() {
            return Err(format!(
                "{} has no pages for this chapter",
                self.provider_name(provider)
            ));
        }
        Ok(pages)
    }
}

fn parse_listing(value: &Value) -> MangaListing {
    let items = value
        .get("items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some(MangaSummary {
                        id: id(item.get("id")?)?,
                        title: text(item.get("title"))?,
                        cover_url: url(item.get("coverUrl")),
                        cover_headers: headers(item.get("coverHeaders")),
                        nsfw: item.get("nsfw").and_then(Value::as_bool).unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    MangaListing {
        items,
        has_next_page: value
            .get("hasNextPage")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

fn parse_details(value: &Value, requested: &str) -> Option<MangaDetails> {
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .filter(|status| STATUSES.contains(status))
        .unwrap_or("unknown");
    Some(MangaDetails {
        id: value
            .get("id")
            .and_then(id)
            .unwrap_or_else(|| requested.to_owned()),
        title: text(value.get("title"))?,
        cover_url: url(value.get("coverUrl")),
        cover_headers: headers(value.get("coverHeaders")),
        // A video source's cast and directors take the authors' and artists' places.
        authors: or_else(texts(value.get("authors")), || texts(value.get("cast"))),
        artists: or_else(texts(value.get("artists")), || {
            texts(value.get("directors"))
        }),
        description: text(value.get("description")),
        genres: texts(value.get("genres")),
        status: status.to_owned(),
        url: url(value.get("url")),
        media_type: value
            .get("type")
            .and_then(Value::as_str)
            .filter(|kind| MEDIA_TYPES.contains(kind))
            .map(str::to_owned),
        year: value
            .get("year")
            .and_then(Value::as_i64)
            .filter(|year| (1800..=2200).contains(year))
            .map(|year| year as i32),
    })
}

fn or_else(first: Vec<String>, second: impl FnOnce() -> Vec<String>) -> Vec<String> {
    if first.is_empty() { second() } else { first }
}

/// A source's chapters or episodes (under `key`); unnamed ones are called "<noun> <number>". A
/// video episode's `season` is kept as its volume.
pub(crate) fn parse_units(value: &Value, key: &str, noun: &str) -> Vec<MangaChapter> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|chapters| {
            chapters
                .iter()
                .filter_map(|chapter| {
                    let number = chapter
                        .get("number")
                        .and_then(Value::as_f64)
                        .filter(|n| n.is_finite());
                    let name = text(chapter.get("name")).or_else(|| {
                        number.map(|number| format!("{noun} {}", format_number(number)))
                    })?;
                    Some(MangaChapter {
                        id: id(chapter.get("id")?)?,
                        name,
                        number,
                        volume: chapter.get("volume").and_then(id).or_else(|| {
                            chapter
                                .get("season")
                                .and_then(Value::as_f64)
                                .filter(|season| season.is_finite() && *season >= 0.0)
                                .map(format_number)
                        }),
                        scanlator: text(chapter.get("scanlator")),
                        date_upload: chapter
                            .get("dateUpload")
                            .and_then(Value::as_f64)
                            .filter(|ms| ms.is_finite() && *ms > 0.0)
                            .map(|ms| ms as i64),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_pages(value: &Value) -> Vec<PageImage> {
    value
        .get("pages")
        .and_then(Value::as_array)
        .map(|pages| {
            pages
                .iter()
                .filter_map(|page| {
                    let (url, headers) = match page {
                        Value::String(_) => (url(Some(page))?, BTreeMap::new()),
                        _ => (url(page.get("url"))?, headers(page.get("headers"))),
                    };
                    Some(PageImage { url, headers })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// "12" for 12.0, "12.5" otherwise.
pub fn format_number(number: f64) -> String {
    if number.fract() == 0.0 {
        format!("{number:.0}")
    } else {
        number.to_string()
    }
}

/// Most filters a source may declare, and most options in one.
const MAX_FILTERS: usize = 20;
const MAX_OPTIONS: usize = 500;
const MAX_LABEL: usize = 80;

fn label(value: Option<&Value>) -> Option<String> {
    text(value).map(|label| label.chars().take(MAX_LABEL).collect())
}

/// Filters as a source declared them, checked: known kinds, keys, labels, bounded option lists.
pub fn parse_filters(value: &Value) -> Vec<SourceFilter> {
    let options = |item: &Value| -> Vec<FilterOption> {
        item.get("options")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|option| {
                Some(FilterOption {
                    value: text(option.get("value"))?,
                    label: label(option.get("label")).or_else(|| text(option.get("value")))?,
                })
            })
            .take(MAX_OPTIONS)
            .collect()
    };
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let key = text(item.get("key")).filter(|key| {
                key.len() <= 40
                    && key
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            })?;
            let label = label(item.get("label")).unwrap_or_else(|| key.clone());
            match item.get("kind").and_then(Value::as_str)? {
                "select" => {
                    let options = options(item);
                    let default = text(item.get("default"))
                        .filter(|default| options.iter().any(|option| &option.value == default));
                    (!options.is_empty()).then_some(SourceFilter::Select {
                        key,
                        label,
                        options,
                        default,
                    })
                }
                "tags" => {
                    let options = options(item);
                    (!options.is_empty()).then_some(SourceFilter::Tags {
                        key,
                        label,
                        options,
                    })
                }
                "toggle" => Some(SourceFilter::Toggle {
                    key,
                    label,
                    default: item
                        .get("default")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                }),
                _ => None,
            }
        })
        .take(MAX_FILTERS)
        .collect()
}

pub(crate) fn id(value: &Value) -> Option<String> {
    match value {
        Value::String(text) if !text.trim().is_empty() => Some(text.trim().to_owned()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

pub(crate) fn text(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn texts(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(|item| text(Some(item))).collect())
        .unwrap_or_default()
}

pub(crate) fn url(value: Option<&Value>) -> Option<String> {
    text(value).filter(|url| url.starts_with("https://") || url.starts_with("http://"))
}

pub(crate) fn headers(value: Option<&Value>) -> BTreeMap<String, String> {
    value
        .and_then(Value::as_object)
        .map(|map| {
            map.iter()
                .filter(|(name, _)| {
                    !FORBIDDEN_HEADERS.contains(&name.to_ascii_lowercase().as_str())
                })
                .filter_map(|(name, value)| Some((name.clone(), value.as_str()?.to_owned())))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn listings_keep_valid_items_only() {
        let listing = parse_listing(&json!({
            "items": [
                { "id": "a1", "title": " One Piece ", "coverUrl": "https://img/x.jpg" },
                { "id": 42, "title": "Numeric id", "coverUrl": "javascript:alert(1)" },
                { "id": "", "title": "No id" },
                { "id": "b2" },
            ],
            "hasNextPage": true,
        }));
        assert!(listing.has_next_page);
        assert_eq!(listing.items.len(), 2);
        assert_eq!(listing.items[0].title, "One Piece");
        assert_eq!(listing.items[1].id, "42");
        assert_eq!(listing.items[1].cover_url, None, "only http(s) covers");
        assert_eq!(parse_listing(&json!(null)).items, vec![]);
    }

    #[test]
    fn details_default_unknown_fields() {
        let details = parse_details(
            &json!({
                "title": "Frieren", "authors": ["Yamada", 3, ""], "status": "weird",
                "genres": ["Fantasy"], "url": "https://comics.example/title/x", "description": "  "
            }),
            "requested-id",
        )
        .unwrap();
        assert_eq!(details.id, "requested-id");
        assert_eq!(details.authors, ["Yamada"]);
        assert_eq!(details.status, "unknown");
        assert_eq!(details.description, None);
        assert!(
            parse_details(&json!({ "id": "x" }), "x").is_none(),
            "a title is required"
        );
    }

    #[test]
    fn chapters_get_names_from_numbers_when_missing() {
        let chapters = parse_units(
            &json!({
                "chapters": [
                    { "id": "c1", "number": 12, "dateUpload": 1700000000000.0, "volume": 2 },
                    { "id": "c2", "name": "Extra", "number": null, "scanlator": "Group" },
                    { "id": "c3" },
                ]
            }),
            "chapters",
            "Chapter",
        );
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].name, "Chapter 12");
        assert_eq!(chapters[0].volume.as_deref(), Some("2"));
        assert_eq!(chapters[0].date_upload, Some(1_700_000_000_000));
        assert_eq!(chapters[1].scanlator.as_deref(), Some("Group"));
        assert_eq!(format_number(12.5), "12.5");
    }

    #[test]
    fn pages_accept_strings_or_objects_and_drop_unsafe_headers() {
        let pages = parse_pages(&json!({
            "pages": [
                "https://cdn/1.png",
                { "url": "https://cdn/2.png", "headers": { "Referer": "https://site/", "Host": "evil", "X": 1 } },
                { "url": "ftp://cdn/3.png" },
            ]
        }));
        assert_eq!(pages.len(), 2);
        assert!(pages[0].headers.is_empty());
        assert_eq!(
            pages[1].headers,
            BTreeMap::from([("Referer".to_owned(), "https://site/".to_owned())])
        );
    }

    #[test]
    fn filters_are_checked() {
        let filters = parse_filters(&json!([
            { "kind": "select", "key": "status", "label": "Status", "default": "any",
              "options": [{ "value": "any", "label": "Any" }, { "value": "ongoing", "label": "Ongoing" }] },
            { "kind": "tags", "key": "genres", "label": "Genres",
              "options": [{ "value": "t1", "label": "Action" }, { "label": "No value" }] },
            { "kind": "toggle", "key": "hasChapters", "label": "Has chapters", "default": true },
            { "kind": "select", "key": "empty", "options": [] },
            { "kind": "slider", "key": "unknown" },
            { "kind": "toggle", "key": "bad key!" },
        ]));
        assert_eq!(filters.len(), 3);
        assert!(matches!(
            &filters[0],
            SourceFilter::Select { default: Some(default), options, .. }
                if default == "any" && options.len() == 2
        ));
        assert!(matches!(&filters[1], SourceFilter::Tags { options, .. } if options.len() == 1));
        assert!(matches!(
            &filters[2],
            SourceFilter::Toggle { default: true, .. }
        ));
        assert!(parse_filters(&json!({ "not": "a list" })).is_empty());
    }
}
