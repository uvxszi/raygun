//! The Stremio addon protocol over HTTP (M41): manifests, resource URLs and the catalog, meta,
//! stream and subtitle answers. The shapes follow stremio-core's types
//! (github.com/Stremio/stremio-core, MIT); parsing is lenient, as addons vary: unknown fields are
//! ignored and entries missing what Raygun needs are dropped.

use std::collections::BTreeMap;

use serde_json::Value;

/// Most items kept from one catalog page, and most streams or subtitles from one addon.
const MAX_ITEMS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub logo: Option<String>,
    pub types: Vec<String>,
    pub id_prefixes: Option<Vec<String>>,
    pub resources: Vec<Resource>,
    pub catalogs: Vec<Catalog>,
    pub adult: bool,
    pub configuration_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    pub name: String,
    /// Overrides the manifest's types for this resource.
    pub types: Option<Vec<String>>,
    pub id_prefixes: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    pub kind: String,
    pub id: String,
    pub name: String,
    pub extras: Vec<Extra>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extra {
    pub name: String,
    pub required: bool,
    pub options: Vec<String>,
}

impl Catalog {
    pub fn extra(&self, name: &str) -> Option<&Extra> {
        self.extras.iter().find(|extra| extra.name == name)
    }

    /// Whether the catalog lists anything without being asked something (search, a genre...).
    pub fn browsable(&self) -> bool {
        !self.extras.iter().any(|extra| extra.required)
    }

    pub fn searchable(&self) -> bool {
        self.extra("search").is_some()
    }

    /// Whether a search-only catalog can serve a search: every required extra is `search`.
    pub fn search_only_ok(&self) -> bool {
        self.extras
            .iter()
            .all(|extra| !extra.required || extra.name == "search")
    }
}

impl Manifest {
    /// Whether the addon answers `resource` for content of `kind` with this `id`.
    pub fn serves(&self, resource: &str, kind: &str, id: &str) -> bool {
        self.resources.iter().any(|candidate| {
            if candidate.name != resource {
                return false;
            }
            let types = candidate.types.as_ref().unwrap_or(&self.types);
            let prefixes = candidate.id_prefixes.as_ref().or(self.id_prefixes.as_ref());
            types.iter().any(|known| known == kind)
                && prefixes.is_none_or(|prefixes| {
                    prefixes.is_empty() || prefixes.iter().any(|prefix| id.starts_with(prefix))
                })
        })
    }
}

fn text(value: Option<&Value>) -> Option<String> {
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

fn http(value: Option<&Value>) -> Option<String> {
    text(value).filter(|url| url.starts_with("https://") || url.starts_with("http://"))
}

/// A manifest as an addon serves it; `Err` when it lacks an id, a name or anything to offer.
pub fn parse_manifest(value: &Value) -> Result<Manifest, String> {
    let id = text(value.get("id")).ok_or("The addon's manifest has no id")?;
    let name = text(value.get("name")).unwrap_or_else(|| id.clone());
    let resources: Vec<Resource> = value
        .get("resources")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|resource| match resource {
            Value::String(name) => Some(Resource {
                name: name.clone(),
                types: None,
                id_prefixes: None,
            }),
            Value::Object(_) => Some(Resource {
                name: text(resource.get("name"))?,
                types: resource.get("types").map(|types| texts(Some(types))),
                id_prefixes: resource
                    .get("idPrefixes")
                    .map(|prefixes| texts(Some(prefixes))),
            }),
            _ => None,
        })
        .collect();
    let catalogs: Vec<Catalog> = value
        .get("catalogs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|catalog| {
            let required_names = texts(catalog.get("extraRequired"));
            let mut extras: Vec<Extra> = catalog
                .get("extra")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|extra| {
                    let name = text(extra.get("name"))?;
                    Some(Extra {
                        required: extra
                            .get("isRequired")
                            .and_then(Value::as_bool)
                            .unwrap_or(false)
                            || required_names.contains(&name),
                        options: texts(extra.get("options")),
                        name,
                    })
                })
                .collect();
            // Older manifests list extras by name only.
            for name in texts(catalog.get("extraSupported")) {
                if !extras.iter().any(|extra| extra.name == name) {
                    extras.push(Extra {
                        required: required_names.contains(&name),
                        options: if name == "genre" {
                            texts(catalog.get("genres"))
                        } else {
                            Vec::new()
                        },
                        name,
                    });
                }
            }
            let id = text(catalog.get("id"))?;
            Some(Catalog {
                kind: text(catalog.get("type"))?,
                name: text(catalog.get("name")).unwrap_or_else(|| id.clone()),
                id,
                extras,
            })
        })
        .collect();
    if resources.is_empty() {
        return Err(format!("{name} offers nothing Raygun can use"));
    }
    let hints = value.get("behaviorHints");
    let hint = |key: &str| {
        hints
            .and_then(|hints| hints.get(key))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    Ok(Manifest {
        id,
        name,
        version: text(value.get("version")).unwrap_or_else(|| "0.0.0".into()),
        description: text(value.get("description")),
        logo: http(value.get("logo")),
        types: texts(value.get("types")),
        id_prefixes: value
            .get("idPrefixes")
            .map(|prefixes| texts(Some(prefixes))),
        resources,
        catalogs,
        adult: hint("adult"),
        configuration_required: hint("configurationRequired"),
    })
}

/// The addon's base URL: its manifest URL without `/manifest.json`.
pub fn base_url(transport_url: &str) -> String {
    transport_url
        .trim_end_matches('/')
        .trim_end_matches("/manifest.json")
        .to_owned()
}

/// `<base>/<resource>/<type>/<id>[/<extra>].json`, every part URI-encoded like Stremio's clients.
pub fn resource_url(
    transport_url: &str,
    resource: &str,
    kind: &str,
    id: &str,
    extras: &[(&str, String)],
) -> String {
    let mut url = format!(
        "{}/{resource}/{}/{}",
        base_url(transport_url),
        encode(kind),
        encode(id)
    );
    if !extras.is_empty() {
        let extra: Vec<String> = extras
            .iter()
            .map(|(name, value)| format!("{}={}", encode(name), encode(value)))
            .collect();
        url.push('/');
        url.push_str(&extra.join("&"));
    }
    url.push_str(".json");
    url
}

/// encodeURIComponent: everything but unreserved characters (and `:`, which ids use, is kept as
/// Stremio's own clients do).
fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'~'
            | b'!'
            | b'*'
            | b'\''
            | b'('
            | b')'
            | b':' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// An addon address as typed: `stremio://` becomes https, a bare host gets https and the manifest
/// path. Only http(s).
pub fn normalize_addon_url(text: &str) -> Result<String, String> {
    let text = text.trim();
    let text = text
        .strip_prefix("stremio://")
        .map(|rest| format!("https://{rest}"))
        .unwrap_or_else(|| text.to_owned());
    let text = if text.contains("://") {
        text
    } else {
        format!("https://{text}")
    };
    let url = url::Url::parse(&text).map_err(|_| "That isn't an addon address".to_owned())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("That isn't an addon address".into());
    }
    let mut url = url.to_string();
    if !url.ends_with("/manifest.json") {
        url = format!("{}/manifest.json", url.trim_end_matches('/'));
    }
    Ok(url)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaPreview {
    pub kind: String,
    pub id: String,
    pub name: String,
    pub poster: Option<String>,
    pub poster_shape: String,
    pub release_info: Option<String>,
    pub rating: Option<String>,
}

fn preview(item: &Value, fallback_kind: &str) -> Option<MetaPreview> {
    Some(MetaPreview {
        id: text(item.get("id"))?,
        name: text(item.get("name"))?,
        kind: text(item.get("type")).unwrap_or_else(|| fallback_kind.to_owned()),
        poster: http(item.get("poster")),
        poster_shape: text(item.get("posterShape"))
            .filter(|shape| matches!(shape.as_str(), "poster" | "landscape" | "square"))
            .unwrap_or_else(|| "poster".into()),
        release_info: text(item.get("releaseInfo")).or_else(|| text(item.get("year"))),
        rating: text(item.get("imdbRating")),
    })
}

/// A catalog page's items.
pub fn parse_metas(value: &Value, kind: &str) -> Vec<MetaPreview> {
    value
        .get("metas")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| preview(item, kind))
        .take(MAX_ITEMS)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Video {
    pub id: String,
    pub title: String,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub released: Option<String>,
    pub thumbnail: Option<String>,
    pub overview: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meta {
    pub preview: MetaPreview,
    /// Ratings with their sources (M46.45).
    pub ratings: Vec<raygun_model::TitleRating>,
    pub background: Option<String>,
    pub logo: Option<String>,
    pub description: Option<String>,
    pub genres: Vec<String>,
    pub runtime: Option<String>,
    pub cast: Vec<String>,
    pub director: Vec<String>,
    pub trailer_yt_ids: Vec<String>,
    /// Episodes in season and episode order (specials, season 0, last).
    pub videos: Vec<Video>,
}

/// Link categories that never hold a rating.
const NOT_RATINGS: [&str; 8] = [
    "genres",
    "genre",
    "cast",
    "directors",
    "writers",
    "share",
    "similar",
    "trailer",
];

/// Whether a link's name reads as a score: "7.8", "85%", "8/10".
fn is_score(name: &str) -> bool {
    let core = name.trim().trim_end_matches('%');
    let core = core.split_once('/').map_or(core, |(score, _)| score);
    !core.is_empty() && core.parse::<f32>().is_ok()
}

/// A title's ratings (M46.45): the links an addon gives with a score as their name (IMDb's,
/// and any other site), else its `imdbRating`, linked to IMDb when the id is IMDb's.
pub fn parse_ratings(
    meta: &Value,
    id: &str,
    imdb_rating: Option<&str>,
) -> Vec<raygun_model::TitleRating> {
    let mut ratings: Vec<raygun_model::TitleRating> = Vec::new();
    for link in meta
        .get("links")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let (Some(category), Some(name)) = (text(link.get("category")), text(link.get("name")))
        else {
            continue;
        };
        let lower = category.to_lowercase();
        if NOT_RATINGS.contains(&lower.as_str()) || !is_score(&name) {
            continue;
        }
        let source = if lower == "imdb" {
            "IMDb".to_owned()
        } else {
            category
        };
        if ratings.iter().any(|rating| rating.source == source) {
            continue;
        }
        ratings.push(raygun_model::TitleRating {
            source,
            value: name.trim().to_owned(),
            url: http(link.get("url")),
        });
    }
    if let Some(value) = imdb_rating.filter(|value| is_score(value))
        && !ratings.iter().any(|rating| rating.source == "IMDb")
    {
        let imdb_id = text(meta.get("imdb_id")).or_else(|| {
            id.starts_with("tt")
                .then(|| id.split(':').next().unwrap_or(id).to_owned())
        });
        ratings.insert(
            0,
            raygun_model::TitleRating {
                source: "IMDb".into(),
                value: value.to_owned(),
                url: imdb_id.map(|imdb| format!("https://www.imdb.com/title/{imdb}/")),
            },
        );
    }
    ratings
}

/// A `meta` answer.
pub fn parse_meta(value: &Value, kind: &str) -> Option<Meta> {
    let meta = value.get("meta")?;
    let preview = preview(meta, kind)?;
    let mut trailers: Vec<String> = meta
        .get("trailerStreams")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|trailer| text(trailer.get("ytId")))
        .collect();
    if trailers.is_empty() {
        trailers = meta
            .get("trailers")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|trailer| text(trailer.get("source")))
            .collect();
    }
    let number = |value: Option<&Value>| value.and_then(Value::as_u64).map(|number| number as u32);
    let mut videos: Vec<Video> = meta
        .get("videos")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|video| {
            let season = number(video.get("season"));
            let episode = number(video.get("episode")).or_else(|| number(video.get("number")));
            Some(Video {
                id: text(video.get("id"))?,
                title: text(video.get("title"))
                    .or_else(|| text(video.get("name")))
                    .unwrap_or_else(|| match (season, episode) {
                        (Some(season), Some(episode)) => format!("S{season} E{episode}"),
                        _ => "Video".into(),
                    }),
                season,
                episode,
                released: text(video.get("released")).or_else(|| text(video.get("firstAired"))),
                thumbnail: http(video.get("thumbnail")),
                overview: text(video.get("overview")).or_else(|| text(video.get("description"))),
            })
        })
        .collect();
    videos.sort_by_key(|video| {
        (
            video.season.map_or(
                u32::MAX,
                |season| if season == 0 { u32::MAX - 1 } else { season },
            ),
            video.episode.unwrap_or(u32::MAX),
        )
    });
    let names = |key: &str| -> Vec<String> {
        match meta.get(key) {
            Some(Value::String(list)) => list
                .split(',')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
                .collect(),
            other => texts(other),
        }
    };
    Some(Meta {
        ratings: parse_ratings(meta, &preview.id, preview.rating.as_deref()),
        background: http(meta.get("background")),
        logo: http(meta.get("logo")),
        description: text(meta.get("description")),
        genres: {
            let genres = texts(meta.get("genres"));
            if genres.is_empty() {
                texts(meta.get("genre"))
            } else {
                genres
            }
        },
        runtime: text(meta.get("runtime")),
        cast: names("cast").into_iter().take(12).collect(),
        director: names("director"),
        trailer_yt_ids: trailers,
        videos,
        preview,
    })
}

/// How a stream can be played.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamSource {
    Direct,
    YouTube,
    External,
    Torrent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subtitle {
    pub url: String,
    pub lang: Option<String>,
    /// A name some addons give ("English Signs"); the player shows the language otherwise.
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stream {
    pub source: StreamSource,
    pub name: Option<String>,
    pub description: Option<String>,
    /// The direct URL, YouTube id, external URL or info hash, by `source`.
    pub target: String,
    pub headers: BTreeMap<String, String>,
    pub subtitles: Vec<Subtitle>,
    /// `behaviorHints.bingeGroup`: streams of one release across episodes, for binge watching.
    pub binge_group: Option<String>,
}

/// Headers a stream may ask for; others (Host, Range...) are the player's business.
fn request_headers(stream: &Value) -> BTreeMap<String, String> {
    const ALLOWED: [&str; 5] = ["user-agent", "referer", "origin", "cookie", "authorization"];
    stream
        .pointer("/behaviorHints/proxyHeaders/request")
        .and_then(Value::as_object)
        .map(|headers| {
            headers
                .iter()
                .filter(|(name, _)| ALLOWED.contains(&name.to_ascii_lowercase().as_str()))
                .filter_map(|(name, value)| Some((name.clone(), value.as_str()?.to_owned())))
                .collect()
        })
        .unwrap_or_default()
}

/// A `stream` answer.
pub fn parse_streams(value: &Value) -> Vec<Stream> {
    value
        .get("streams")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|stream| {
            let (source, target) = if let Some(url) = http(stream.get("url")) {
                (StreamSource::Direct, url)
            } else if let Some(id) = text(stream.get("ytId")) {
                (StreamSource::YouTube, id)
            } else if let Some(url) = http(stream.get("externalUrl")) {
                (StreamSource::External, url)
            } else {
                (StreamSource::Torrent, text(stream.get("infoHash"))?)
            };
            Some(Stream {
                source,
                name: text(stream.get("name")),
                description: text(stream.get("description")).or_else(|| text(stream.get("title"))),
                headers: request_headers(stream),
                subtitles: parse_subtitles(stream),
                binge_group: text(stream.pointer("/behaviorHints/bingeGroup")),
                target,
            })
        })
        .take(MAX_ITEMS)
        .collect()
}

/// A `subtitles` answer (or a stream's own `subtitles`).
pub fn parse_subtitles(value: &Value) -> Vec<Subtitle> {
    value
        .get("subtitles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|subtitle| {
            Some(Subtitle {
                url: http(subtitle.get("url"))?,
                lang: text(subtitle.get("lang")),
                label: text(subtitle.get("label")),
            })
        })
        .take(MAX_ITEMS)
        .collect()
}

/// Subtitles with the preferred language first (by 2- or 3-letter code), at most `limit`, no
/// repeated URLs.
pub fn order_subtitles(subtitles: Vec<Subtitle>, preferred: &str, limit: usize) -> Vec<Subtitle> {
    let preferred = preferred
        .split('-')
        .next()
        .unwrap_or(preferred)
        .to_lowercase();
    let three = match preferred.as_str() {
        "en" => "eng",
        "es" => "spa",
        "fr" => "fre",
        "de" => "ger",
        "pt" => "por",
        "it" => "ita",
        "ja" => "jpn",
        "ru" => "rus",
        "zh" => "chi",
        "ko" => "kor",
        "ar" => "ara",
        _ => "",
    };
    let matches = |subtitle: &Subtitle| {
        subtitle.lang.as_deref().is_some_and(|lang| {
            let lang = lang.to_lowercase();
            lang == preferred || (!three.is_empty() && lang == three)
        })
    };
    let mut seen = std::collections::HashSet::new();
    let (mut first, rest): (Vec<_>, Vec<_>) = subtitles
        .into_iter()
        .filter(|subtitle| seen.insert(subtitle.url.clone()))
        .partition(matches);
    first.extend(rest);
    first.truncate(limit);
    first
}

#[cfg(test)]
mod tests;
