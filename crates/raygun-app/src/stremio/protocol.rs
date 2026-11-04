//! The Stremio addon protocol over HTTP (M41): manifests, resource URLs and the catalog, meta,
//! stream and subtitle answers. The shapes, the request encoding and which addon is asked for what
//! follow stremio-core (github.com/Stremio/stremio-core, MIT: `types/addon`, `types/resource`,
//! `addon_transport`); parsing is lenient, as addons vary: unknown fields are ignored and entries
//! missing what Raygun needs are dropped.

use std::collections::{BTreeMap, HashSet};

use base64::Engine;
use serde_json::{Value, json};

/// Most items kept from one catalog page (some addons list every channel on one page).
const MAX_ITEMS: usize = 2_000;
/// Most streams kept from one addon.
const MAX_STREAMS: usize = 500;
/// Most subtitles read from one answer; the preferred language is picked out of them afterwards.
const MAX_SUBTITLES_READ: usize = 20_000;

/// Where a manifest is: every resource lives next to it.
pub const MANIFEST_PATH: &str = "/manifest.json";
/// Addons of Stremio's first protocol (JSON-RPC), such as the old OpenSubtitles.
pub const LEGACY_PATH: &str = "/stremio/v1";

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
    /// Its streams are torrents.
    pub p2p: bool,
    pub configurable: bool,
    pub configuration_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    pub name: String,
    /// The types this resource is for, when written as an object with its own.
    pub types: Option<Vec<String>>,
    pub id_prefixes: Option<Vec<String>>,
    /// Written as an object rather than a plain name: its own id prefixes apply, and none means
    /// every id (stremio-core's `ManifestResource::Full`).
    pub full: bool,
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
    /// How many options may be passed at once.
    pub options_limit: u32,
}

impl Catalog {
    pub fn extra(&self, name: &str) -> Option<&Extra> {
        self.extras.iter().find(|extra| extra.name == name)
    }

    /// stremio-core's `are_extra_names_supported`: the catalog knows every one of `names`, and
    /// every extra it requires is among them.
    pub fn takes(&self, names: &[&str]) -> bool {
        names.iter().all(|name| self.extra(name).is_some())
            && self
                .extras
                .iter()
                .filter(|extra| extra.required)
                .all(|extra| names.contains(&extra.name.as_str()))
    }

    /// Whether the catalog lists anything without being asked something: a row on the board.
    pub fn browsable(&self) -> bool {
        self.takes(&[])
    }

    /// Whether it answers a search (what it requires, if anything, is the search).
    pub fn searchable(&self) -> bool {
        self.takes(&["search"])
    }

    /// Whether it pages with `skip`.
    pub fn pages(&self) -> bool {
        self.extra("skip").is_some()
    }

    /// What it needs before it lists anything in Discover: each required extra's first option
    /// (stremio-core's `default_required_extra`); `None` when a required extra has no options to
    /// pick from (a search, a list of ids), so the catalog isn't browsed at all.
    pub fn default_required(&self) -> Option<Vec<(String, String)>> {
        self.extras
            .iter()
            .filter(|extra| extra.required)
            .map(|extra| {
                extra
                    .options
                    .first()
                    .map(|option| (extra.name.clone(), option.clone()))
            })
            .collect()
    }

    /// The extras a request may carry, as Stremio sends them: `skip` first, then the others in
    /// the manifest's order; unknown extras and values outside an extra's options are left out,
    /// and a required extra missing from `values` gets its first option.
    pub fn request_extras(&self, values: &[(String, String)], skip: u32) -> Vec<(String, String)> {
        let mut extras = Vec::new();
        if skip > 0 && self.pages() {
            extras.push(("skip".to_owned(), skip.to_string()));
        }
        for extra in self.extras.iter().filter(|extra| extra.name != "skip") {
            let mut given: Vec<&String> = values
                .iter()
                .filter(|(name, value)| {
                    *name == extra.name
                        && !value.is_empty()
                        && (extra.options.is_empty() || extra.options.contains(value))
                })
                .map(|(_, value)| value)
                .collect();
            given.truncate(extra.options_limit.max(1) as usize);
            match given.first() {
                Some(_) => extras.extend(
                    given
                        .into_iter()
                        .map(|value| (extra.name.clone(), value.clone())),
                ),
                None if extra.required => {
                    if let Some(first) = extra.options.first() {
                        extras.push((extra.name.clone(), first.clone()));
                    }
                }
                None => {}
            }
        }
        extras
    }
}

impl Manifest {
    /// Whether the addon answers `resource` (other than catalogs) for content of `kind` with this
    /// `id`: stremio-core's `is_resource_supported`. A plain resource takes the manifest's types and
    /// id prefixes; one written as an object its own, where no prefixes means every id. (An
    /// object without types falls back on the manifest's, as the SDK's documentation reads.)
    pub fn serves(&self, resource: &str, kind: &str, id: &str) -> bool {
        let Some(found) = self
            .resources
            .iter()
            .find(|candidate| candidate.name == resource)
        else {
            return false;
        };
        let types = found.types.as_ref().unwrap_or(&self.types);
        let prefixes = if found.full {
            found.id_prefixes.as_ref()
        } else {
            self.id_prefixes.as_ref()
        };
        types.iter().any(|known| known == kind)
            && prefixes.is_none_or(|prefixes| {
                prefixes.is_empty()
                    || prefixes
                        .iter()
                        .any(|prefix| id.starts_with(prefix.as_str()))
            })
    }

    pub fn has_resource(&self, resource: &str) -> bool {
        self.resources
            .iter()
            .any(|candidate| candidate.name == resource)
    }

    /// The types an addon answers `resource` for.
    pub fn resource_types(&self, resource: &str) -> Vec<String> {
        let mut types: Vec<String> = Vec::new();
        for found in self
            .resources
            .iter()
            .filter(|candidate| candidate.name == resource)
        {
            for kind in found.types.as_ref().unwrap_or(&self.types) {
                if !types.contains(kind) {
                    types.push(kind.clone());
                }
            }
        }
        types
    }
}

fn text(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

/// Text, or a number written as one (`"releaseInfo": 1999`, `"imdbRating": 8.1`).
fn text_or_number(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::Number(number)) => Some(number.to_string()),
        other => text(other),
    }
}

fn texts(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(|item| text(Some(item))).collect())
        .unwrap_or_default()
}

/// A list where one may be `null` or missing: `None` then, else its texts.
fn optional_texts(value: Option<&Value>) -> Option<Vec<String>> {
    value
        .filter(|value| value.is_array())
        .map(|value| texts(Some(value)))
}

fn http(value: Option<&Value>) -> Option<String> {
    text(value).filter(|url| url.starts_with("https://") || url.starts_with("http://"))
}

/// A whole number, also when written as text (`"season": "2"`).
fn number(value: Option<&Value>) -> Option<u32> {
    match value? {
        Value::Number(number) => number
            .as_u64()
            .and_then(|number| u32::try_from(number).ok()),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

fn flag(value: Option<&Value>, key: &str) -> bool {
    value
        .and_then(|hints| hints.get(key))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// A manifest as an addon serves it; `Err` when it lacks an id or anything to offer.
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
                name: name.trim().to_owned(),
                types: None,
                id_prefixes: None,
                full: false,
            }),
            Value::Object(_) => Some(Resource {
                name: text(resource.get("name"))?,
                types: optional_texts(resource.get("types")),
                id_prefixes: optional_texts(resource.get("idPrefixes")),
                full: true,
            }),
            _ => None,
        })
        .filter(|resource| !resource.name.is_empty())
        .collect();
    let mut seen = HashSet::new();
    let catalogs: Vec<Catalog> = value
        .get("catalogs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(parse_catalog)
        // One catalog per type and id, as stremio-core keeps them.
        .filter(|catalog| seen.insert((catalog.kind.clone(), catalog.id.clone())))
        .collect();
    if resources.is_empty() {
        return Err(format!("{name} doesn't offer anything Raygun can use"));
    }
    let hints = value.get("behaviorHints");
    Ok(Manifest {
        id,
        name,
        version: text(value.get("version")).unwrap_or_else(|| "0.0.0".into()),
        description: text(value.get("description")),
        logo: http(value.get("logo")),
        types: texts(value.get("types")),
        id_prefixes: optional_texts(value.get("idPrefixes")),
        resources,
        catalogs,
        adult: flag(hints, "adult"),
        p2p: flag(hints, "p2p"),
        configurable: flag(hints, "configurable"),
        configuration_required: flag(hints, "configurationRequired"),
    })
}

fn parse_catalog(catalog: &Value) -> Option<Catalog> {
    let required_names = texts(catalog.get("extraRequired"));
    let mut extras: Vec<Extra> = Vec::new();
    for extra in catalog
        .get("extra")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(name) = text(extra.get("name")) else {
            continue;
        };
        if extras.iter().any(|known| known.name == name) {
            continue;
        }
        // stremio-core reads `skip` as its own: never required, no options.
        let skip = name == "skip";
        extras.push(Extra {
            required: !skip
                && (extra
                    .get("isRequired")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                    || required_names.contains(&name)),
            options: if skip {
                Vec::new()
            } else {
                texts(extra.get("options"))
            },
            options_limit: extra
                .get("optionsLimit")
                .and_then(Value::as_u64)
                .and_then(|limit| u32::try_from(limit).ok())
                .unwrap_or(1),
            name,
        });
    }
    // Older manifests list extras by name only.
    for name in texts(catalog.get("extraSupported")) {
        if !extras.iter().any(|extra| extra.name == name) {
            extras.push(Extra {
                required: name != "skip" && required_names.contains(&name),
                options: if name == "genre" {
                    texts(catalog.get("genres"))
                } else {
                    Vec::new()
                },
                options_limit: 1,
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
}

/// An addon address as typed, as the address of its manifest: `stremio://` becomes https, a bare
/// host gets https, and the manifest's path is added unless it's there (after a configuration
/// page's address, in its place). Only http(s); queries are kept.
pub fn normalize_addon_url(text: &str) -> Result<String, String> {
    let text = text.trim();
    let text = match text.strip_prefix("stremio://") {
        Some(rest) => format!("https://{rest}"),
        None if text.contains("://") => text.to_owned(),
        None => format!("https://{text}"),
    };
    let mut url = url::Url::parse(&text).map_err(|_| "That isn't an addon address".to_owned())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("That isn't an addon address".into());
    }
    url.set_fragment(None);
    let path = url.path().trim_end_matches('/').to_owned();
    if !path.ends_with(MANIFEST_PATH) && !path.ends_with(LEGACY_PATH) {
        let base = path.strip_suffix("/configure").unwrap_or(&path);
        url.set_path(&format!("{base}{MANIFEST_PATH}"));
    } else {
        url.set_path(&path);
    }
    Ok(url.to_string())
}

/// Whether an addon speaks Stremio's first protocol.
pub fn is_legacy(transport_url: &str) -> bool {
    url::Url::parse(transport_url).is_ok_and(|url| url.path().ends_with(LEGACY_PATH))
}

/// Where an addon's own pages live: its manifest's folder.
pub fn base_url(transport_url: &str) -> String {
    match url::Url::parse(transport_url) {
        Ok(mut url) => {
            let path = url.path().to_owned();
            let base = path
                .strip_suffix(MANIFEST_PATH)
                .or_else(|| path.strip_suffix(LEGACY_PATH))
                .unwrap_or(&path);
            url.set_path(base);
            url.set_query(None);
            url.to_string().trim_end_matches('/').to_owned()
        }
        Err(_) => transport_url.trim_end_matches(MANIFEST_PATH).to_owned(),
    }
}

/// The page where an addon is set up (`configure` next to its manifest).
pub fn configure_url(transport_url: &str) -> String {
    format!("{}/configure", base_url(transport_url))
}

/// What a manifest is asked for at: the address itself, or the legacy protocol's query.
pub fn manifest_url(transport_url: &str) -> String {
    if is_legacy(transport_url) {
        // {"params":[],"method":"meta","id":1,"jsonrpc":"2.0"}
        format!(
            "{transport_url}/q.json?b=eyJwYXJhbXMiOltdLCJtZXRob2QiOiJtZXRhIiwiaWQiOjEsImpzb25ycGMiOiIyLjAifQ=="
        )
    } else {
        transport_url.to_owned()
    }
}

/// `<base>/<resource>/<type>/<id>[/<extra>].json` in place of the manifest's path, every part
/// encoded as stremio-core's `AddonHTTPTransport` does (encodeURIComponent: `:` and spaces too,
/// which matters to addons behind a cache that Stremio's own requests fill).
pub fn resource_url(
    transport_url: &str,
    resource: &str,
    kind: &str,
    id: &str,
    extras: &[(String, String)],
) -> String {
    if is_legacy(transport_url) {
        return legacy_url(transport_url, resource, kind, id, extras);
    }
    let mut path = format!("/{}/{}/{}", encode(resource), encode(kind), encode(id));
    if !extras.is_empty() {
        let pairs: Vec<String> = extras
            .iter()
            .map(|(name, value)| format!("{}={}", encode(name), encode(value)))
            .collect();
        path.push('/');
        path.push_str(&pairs.join("&"));
    }
    path.push_str(".json");
    transport_url.replace(MANIFEST_PATH, &path)
}

/// encodeURIComponent (stremio-core's `URI_COMPONENT_ENCODE_SET`): everything but letters, digits
/// and `-_.!~*'()`.
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
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

// ---- The legacy protocol (stremio-core's `AddonLegacyTransport`) ----

/// A JSON-RPC request in the query, base64 as the old clients sent it.
fn legacy_url(
    transport_url: &str,
    resource: &str,
    kind: &str,
    id: &str,
    extras: &[(String, String)],
) -> String {
    let extra = |name: &str| {
        extras
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    };
    let (method, params) = match resource {
        "catalog" => {
            let query = match extra("genre") {
                Some(genre) => json!({ "type": kind, "genre": genre }),
                None => json!({ "type": kind }),
            };
            let sort = if id == "top" {
                Value::Null
            } else {
                json!({ id: -1, "popularity": -1 })
            };
            let skip = extra("skip")
                .and_then(|skip| skip.parse::<u32>().ok())
                .unwrap_or(0);
            (
                "meta.find",
                json!({ "query": query, "limit": 100, "sort": sort, "skip": skip }),
            )
        }
        "meta" => ("meta.get", json!({ "query": legacy_query(id) })),
        "stream" => {
            let mut query = legacy_query(id);
            if let Value::Object(fields) = &mut query {
                fields.insert("type".into(), json!(kind));
            }
            ("stream.find", json!({ "query": query }))
        }
        _ => {
            let mut query = serde_json::Map::new();
            query.insert("itemHash".into(), json!(id.replace(':', " ")));
            for name in ["videoHash", "filename"] {
                if let Some(value) = extra(name) {
                    query.insert(name.into(), json!(value));
                }
            }
            if let Some(size) = extra("videoSize").and_then(|size| size.parse::<u64>().ok()) {
                query.insert("videoSize".into(), json!(size));
            }
            ("subtitles.find", json!({ "query": query }))
        }
    };
    let request =
        json!({ "params": [Value::Null, params], "method": method, "id": 1, "jsonrpc": "2.0" });
    let encoded = base64::engine::general_purpose::STANDARD.encode(request.to_string());
    format!("{transport_url}/q.json?b={encoded}")
}

/// An id as the legacy protocol's query: `tt…(:season:episode)`, a YouTube channel, or
/// `prefix:id(:video)`.
fn legacy_query(id: &str) -> Value {
    let parts: Vec<&str> = id.split(':').collect();
    if id.starts_with("tt") {
        return match parts.as_slice() {
            [imdb, season, episode] => json!({
                "imdb_id": imdb,
                "season": season.parse::<u16>().unwrap_or(1),
                "episode": episode.parse::<u16>().unwrap_or(1),
            }),
            _ => json!({ "imdb_id": parts[0] }),
        };
    }
    if id.starts_with("UC") {
        return match parts.as_slice() {
            [channel, video] => json!({ "yt_id": channel, "video_id": video }),
            _ => json!({ "yt_id": parts[0] }),
        };
    }
    match parts.as_slice() {
        [prefix, id, video] => json!({ *prefix: id, "video_id": video }),
        [prefix, id] => json!({ *prefix: id }),
        _ => Value::Null,
    }
}

/// A legacy answer in today's shape (`{"metas": …}`, `{"meta": …}`, `{"streams": …}`,
/// `{"subtitles": …}`, or the manifest), or the addon's error.
pub fn legacy_answer(resource: &str, value: Value) -> Result<Value, String> {
    if let Some(error) = value.get("error") {
        return Err(text(error.get("message")).unwrap_or_else(|| "The addon refused".to_owned()));
    }
    let result = value.get("result").cloned().unwrap_or(Value::Null);
    Ok(match resource {
        "manifest" => legacy_manifest(&result),
        "catalog" => json!({ "metas": result }),
        "meta" => json!({ "meta": result }),
        "stream" => json!({ "streams": result }),
        _ => json!({ "subtitles": result.get("all").cloned().unwrap_or(Value::Null) }),
    })
}

/// stremio-core's `LegacyManifest` conversion: catalogs from `meta.find` (by sort, else `top` per
/// type), resources from the methods, id prefixes from `idProperty`.
fn legacy_manifest(result: &Value) -> Value {
    let manifest = result.get("manifest").unwrap_or(result);
    let mut methods = texts(manifest.get("methods"));
    methods.extend(texts(result.get("methods")));
    let types = texts(manifest.get("types"));
    let catalogs: Vec<Value> = if methods.iter().any(|method| method == "meta.find") {
        match manifest.get("sorts").and_then(Value::as_array) {
            Some(sorts) => sorts
                .iter()
                .flat_map(|sort| {
                    let kinds = optional_texts(sort.get("types")).unwrap_or_else(|| types.clone());
                    let id = text(sort.get("prop"));
                    let name = text(sort.get("name"));
                    kinds.into_iter().filter_map(move |kind| {
                        Some(json!({ "type": kind, "id": id.clone()?, "name": name.clone(), "extraSupported": ["genre", "skip"] }))
                    })
                })
                .collect(),
            None => types
                .iter()
                .map(|kind| json!({ "type": kind, "id": "top", "extraSupported": ["genre", "skip"] }))
                .collect(),
        }
    } else {
        Vec::new()
    };
    let prefixes: Option<Vec<String>> = match manifest.get("idProperty") {
        Some(Value::String(one)) => Some(vec![one.clone()]),
        Some(Value::Array(many)) => Some(
            many.iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect(),
        ),
        _ => None,
    }
    .map(|properties| {
        properties
            .into_iter()
            .map(|property| match property.as_str() {
                "imdb_id" => "tt".to_owned(),
                "yt_id" => "UC".to_owned(),
                other => format!("{other}:"),
            })
            .collect()
    });
    let mut resources = Vec::new();
    for (method, resource) in [
        ("meta.find", "catalog"),
        ("meta.get", "meta"),
        ("stream.find", "stream"),
        ("subtitles.find", "subtitles"),
    ] {
        if methods.iter().any(|known| known == method) && !resources.contains(&resource) {
            resources.push(resource);
        }
    }
    json!({
        "id": manifest.get("id"),
        "name": manifest.get("name"),
        "description": manifest.get("description"),
        "logo": manifest.get("logo"),
        "version": manifest.get("version"),
        "types": types,
        "idPrefixes": prefixes,
        "resources": resources,
        "catalogs": catalogs,
    })
}

// ---- Answers ----

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
    let id = text_or_number(item.get("id"))?;
    let name = text(item.get("name")).unwrap_or_default();
    let poster = http(item.get("poster"));
    // A card needs something to show: a name or a picture.
    if name.is_empty() && poster.is_none() {
        return None;
    }
    Some(MetaPreview {
        id,
        name,
        kind: text(item.get("type")).unwrap_or_else(|| fallback_kind.to_owned()),
        poster,
        poster_shape: text(item.get("posterShape"))
            .filter(|shape| matches!(shape.as_str(), "poster" | "landscape" | "square"))
            .unwrap_or_else(|| "poster".into()),
        release_info: text_or_number(item.get("releaseInfo"))
            .or_else(|| text_or_number(item.get("year"))),
        rating: text_or_number(item.get("imdbRating")),
    })
}

/// A catalog page's items (`metas`, or `metasDetailed`), once each.
pub fn parse_metas(value: &Value, kind: &str) -> Vec<MetaPreview> {
    let mut seen = HashSet::new();
    value
        .get("metas")
        .or_else(|| value.get("metasDetailed"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| preview(item, kind))
        .filter(|item| seen.insert(item.id.clone()))
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
    /// Streams the details carry for it (channels' videos), or YouTube's for a `yt_id:` video.
    pub streams: Vec<Stream>,
    /// The IMDb episode it is (`imdb_id`, `imdbSeason`, `imdbEpisode`: anime addons map theirs).
    pub imdb: Option<(String, Option<u32>, Option<u32>)>,
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
    /// Episodes in watching order (specials, season 0, after the seasons).
    pub videos: Vec<Video>,
    /// `behaviorHints.defaultVideoId`: what plays for the title itself (a movie's IMDb id when
    /// its details came by another id).
    pub default_video_id: Option<String>,
    /// The title's IMDb id, which most stream and subtitle addons go by.
    pub imdb_id: Option<String>,
    /// TMDB's id, when the details give it.
    pub tmdb_id: Option<String>,
    /// A live channel (`behaviorHints.isLive`, or the `tv` type).
    pub live: bool,
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
        let (Some(category), Some(name)) =
            (text(link.get("category")), text_or_number(link.get("name")))
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

/// Whether an id is an IMDb title's (`tt` and digits).
fn is_imdb(id: &str) -> bool {
    id.len() > 2 && id.starts_with("tt") && id[2..].bytes().all(|byte| byte.is_ascii_digit())
}

fn imdb(value: Option<&Value>) -> Option<String> {
    text(value).filter(|id| is_imdb(id))
}

/// stremio-core's `Stream::youtube`: a `yt_id:<channel>:<video>` id plays that YouTube video.
fn youtube_stream(video_id: &str) -> Option<Stream> {
    let yt_id = video_id
        .strip_prefix("yt_id:")?
        .split(':')
        .nth(1)
        .filter(|id| !id.is_empty())?;
    Some(Stream {
        source: StreamSource::YouTube,
        target: yt_id.to_owned(),
        ..Stream::default()
    })
}

fn parse_video(video: &Value) -> Option<Video> {
    let id = text_or_number(video.get("id"))?;
    let season = number(video.get("season"));
    let episode = number(video.get("episode")).or_else(|| number(video.get("number")));
    // A video's own streams: `streams`, or one `stream`.
    let mut streams: Vec<Stream> = match video.get("streams").or_else(|| video.get("stream")) {
        Some(Value::Array(items)) => items.iter().filter_map(parse_stream).collect(),
        Some(item @ Value::Object(_)) => parse_stream(item).into_iter().collect(),
        _ => Vec::new(),
    };
    if streams.is_empty()
        && let Some(stream) = youtube_stream(&id)
    {
        streams.push(stream);
    }
    Some(Video {
        title: text(video.get("title"))
            .or_else(|| text(video.get("name")))
            .unwrap_or_else(|| match (season, episode) {
                (Some(season), Some(episode)) => format!("S{season} E{episode}"),
                _ => "Video".into(),
            }),
        season,
        episode,
        released: text(video.get("released"))
            .or_else(|| text(video.get("firstAired")))
            .or_else(|| text(video.get("publishedAt"))),
        thumbnail: http(video.get("thumbnail")),
        overview: text(video.get("overview")).or_else(|| text(video.get("description"))),
        streams,
        imdb: imdb(video.get("imdb_id")).map(|imdb| {
            (
                imdb,
                number(video.get("imdbSeason")),
                number(video.get("imdbEpisode")),
            )
        }),
        id,
    })
}

/// stremio-core's video order: numbered episodes by season (specials, season 0, last) and number,
/// then the rest by release, oldest first in a series and newest first otherwise (a channel's).
fn sort_videos(videos: &mut [Video]) {
    let numbered = |video: &Video| video.season.zip(video.episode);
    let series = videos.iter().any(|video| numbered(video).is_some());
    let released = |video: &Video| video.released.as_deref().and_then(release_ms);
    videos.sort_by(|a, b| match (numbered(a), numbered(b)) {
        (Some((a_season, a_episode)), Some((b_season, b_episode))) => {
            let order = |season: u32| if season == 0 { u32::MAX } else { season };
            order(a_season)
                .cmp(&order(b_season))
                .then(a_episode.cmp(&b_episode))
        }
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => match (released(a), released(b)) {
            (Some(a), Some(b)) if series => a.cmp(&b),
            (Some(a), Some(b)) => b.cmp(&a),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        },
    });
}

/// A release date (`2008-01-20T12:00:00.000Z` or `2008-01-20`) as Unix ms.
pub fn release_ms(text: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(text)
        .map(|date| date.timestamp_millis())
        .ok()
        .or_else(|| {
            let day = chrono::NaiveDate::parse_from_str(text.get(..10)?, "%Y-%m-%d").ok()?;
            Some(day.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis())
        })
}

/// A `meta` answer; `None` without details to show (no id or no name).
pub fn parse_meta(value: &Value, kind: &str) -> Option<Meta> {
    let meta = value.get("meta")?;
    let preview = preview(meta, kind).filter(|preview| !preview.name.is_empty())?;
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
            .filter(|trailer| {
                text(trailer.get("type")).is_none_or(|kind| kind.eq_ignore_ascii_case("trailer"))
            })
            .filter_map(|trailer| text(trailer.get("source")))
            .collect();
    }
    let mut seen = HashSet::new();
    let mut videos: Vec<Video> = meta
        .get("videos")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(parse_video)
        .filter(|video| seen.insert(video.id.clone()))
        .collect();
    sort_videos(&mut videos);
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
    let hints = meta.get("behaviorHints");
    let imdb_id = imdb(meta.get("imdb_id"))
        .or_else(|| imdb(meta.get("imdbId")))
        .or_else(|| is_imdb(&preview.id).then(|| preview.id.clone()));
    let tmdb_id = text_or_number(meta.get("moviedb_id"))
        .or_else(|| text_or_number(meta.get("tmdb_id")))
        .or_else(|| preview.id.strip_prefix("tmdb:").map(str::to_owned))
        .filter(|id| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()));
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
        runtime: text_or_number(meta.get("runtime")),
        cast: names("cast").into_iter().take(12).collect(),
        director: names("director"),
        trailer_yt_ids: trailers,
        videos,
        default_video_id: hints.and_then(|hints| text(hints.get("defaultVideoId"))),
        imdb_id,
        tmdb_id,
        live: flag(hints, "isLive") || preview.kind == "tv",
        preview,
    })
}

impl Meta {
    /// The id a movie (or a channel, or anything without episodes) plays by: its default video,
    /// else its own id.
    pub fn own_video_id(&self) -> String {
        self.default_video_id
            .clone()
            .unwrap_or_else(|| self.preview.id.clone())
    }

    pub fn video(&self, video_id: &str) -> Option<&Video> {
        self.videos.iter().find(|video| video.id == video_id)
    }

    /// The one place a title's videos map to the ids stream and subtitle addons are asked with:
    /// for a movie or episode, best first, its own id, then the details' default video, then
    /// IMDb's (`tt…:<season>:<episode>`) and TMDB's. An episode's IMDb season and number come from
    /// the video's own mapping (`imdbSeason`, `imdbEpisode`, which anime addons give, as Kitsu
    /// keeps one entry per season numbered from 1); the title's own numbering is used only for
    /// ids whose numbering is IMDb's (TMDB, TVDB, Trakt), never for anime databases'.
    pub fn stream_ids_for_video(&self, video_id: &str) -> Vec<String> {
        let mut ids = vec![video_id.to_owned()];
        let anime_numbering = ["kitsu", "mal", "anilist", "anidb"]
            .iter()
            .any(|prefix| self.preview.id.starts_with(prefix) || video_id.starts_with(prefix));
        match self.video(video_id) {
            Some(video) => {
                let numbered = video.season.zip(video.episode).filter(|_| !anime_numbering);
                if let Some((imdb, season, episode)) = &video.imdb {
                    match season.zip(*episode).or(numbered) {
                        Some((season, episode)) => ids.push(format!("{imdb}:{season}:{episode}")),
                        // A film among an anime's videos is its IMDb title.
                        None if video.season.is_none() => ids.push(imdb.clone()),
                        None => {}
                    }
                }
                if let (Some(imdb), Some((season, episode))) = (&self.imdb_id, numbered) {
                    ids.push(format!("{imdb}:{season}:{episode}"));
                }
                if let (Some(tmdb), Some((season, episode))) = (&self.tmdb_id, numbered) {
                    ids.push(format!("tmdb:{tmdb}:{season}:{episode}"));
                }
            }
            None => {
                ids.extend(self.default_video_id.clone());
                ids.extend(self.imdb_id.clone());
                ids.extend(self.tmdb_id.as_ref().map(|tmdb| format!("tmdb:{tmdb}")));
            }
        }
        let mut seen = HashSet::new();
        ids.retain(|id| seen.insert(id.clone()));
        ids
    }
}

/// How a stream can be played.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StreamSource {
    /// A web address Raygun's player plays.
    #[default]
    Direct,
    YouTube,
    /// A page or app outside Raygun.
    External,
    /// A torrent: a torrent app plays it (`target` is its magnet link).
    Torrent,
    /// Another title's page (`stremio:///detail/…`); `target` is `<type>/<id>`.
    Title,
    /// Archives, Usenet and other addresses only Stremio's streaming server opens.
    Server,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subtitle {
    pub url: String,
    pub lang: Option<String>,
    /// A name some addons give ("English Signs"); the player shows the language otherwise.
    pub label: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stream {
    pub source: StreamSource,
    pub name: Option<String>,
    pub description: Option<String>,
    /// The direct URL, YouTube id, external URL, magnet link or title, by `source`.
    pub target: String,
    /// `behaviorHints.proxyHeaders.request`.
    pub headers: BTreeMap<String, String>,
    pub subtitles: Vec<Subtitle>,
    /// `behaviorHints.bingeGroup`: streams of one release across episodes, for binge watching.
    pub binge_group: Option<String>,
    /// `behaviorHints.filename`, `videoSize` and `videoHash`, which subtitle addons match by.
    pub filename: Option<String>,
    pub size: Option<u64>,
    pub video_hash: Option<String>,
    /// `behaviorHints.notWebReady`: a browser can't play it as it is.
    pub not_web_ready: bool,
}

/// Request headers the player keeps for itself.
const PLAYER_HEADERS: [&str; 12] = [
    "host",
    "content-length",
    "range",
    "connection",
    "keep-alive",
    "transfer-encoding",
    "te",
    "trailer",
    "upgrade",
    "proxy-authorization",
    "proxy-connection",
    "accept-encoding",
];

/// Headers a stream asks for (`proxyHeaders.request`), without the ones the player sets itself
/// and without anything that couldn't be sent.
fn request_headers(stream: &Value) -> BTreeMap<String, String> {
    stream
        .pointer("/behaviorHints/proxyHeaders/request")
        .and_then(Value::as_object)
        .map(|headers| {
            headers
                .iter()
                .filter(|(name, _)| {
                    !name.is_empty()
                        && name
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
                        && !PLAYER_HEADERS.contains(&name.to_ascii_lowercase().as_str())
                })
                .filter_map(|(name, value)| {
                    let value = text_or_number(Some(value))?;
                    (!value.contains(['\r', '\n'])).then(|| (name.clone(), value))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A torrent's magnet link: its info hash, file, trackers and a name.
fn magnet(stream: &Value, hash: &str, name: Option<&str>) -> String {
    let mut link = format!("magnet:?xt=urn:btih:{}", hash.to_lowercase());
    if let Some(name) = name {
        link.push_str(&format!("&dn={}", encode(name)));
    }
    let trackers = stream
        .get("announce")
        .or_else(|| stream.get("sources"))
        .map(|value| texts(Some(value)))
        .unwrap_or_default();
    for tracker in trackers {
        if tracker.starts_with("dht:") {
            continue;
        }
        let tracker = tracker.strip_prefix("tracker:").unwrap_or(&tracker);
        link.push_str(&format!("&tr={}", encode(tracker)));
    }
    if let Some(index) = number(stream.get("fileIdx")) {
        link.push_str(&format!("&so={index}"));
    }
    link
}

/// A Stremio deep link to a title: `stremio:///detail/<type>/<id>[/<video>]`, as `<type>/<id>`.
fn title_link(url: &str) -> Option<String> {
    let rest = url.strip_prefix("stremio:///detail/")?;
    let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    let mut parts = rest.split('/');
    let kind = parts.next().filter(|kind| !kind.is_empty())?;
    let id = parts.next().filter(|id| !id.is_empty())?;
    let decode = |part: &str| {
        url::form_urlencoded::parse(format!("a={part}").as_bytes())
            .next()
            .map(|(_, value)| value.into_owned())
            .unwrap_or_else(|| part.to_owned())
    };
    Some(format!("{}/{}", decode(kind), decode(id)))
}

fn parse_stream(stream: &Value) -> Option<Stream> {
    let hints = stream.get("behaviorHints");
    let hint_text = |key: &str| hints.and_then(|hints| text(hints.get(key)));
    let filename = hint_text("filename");
    let name = text(stream.get("name"));
    // As stremio-core's untagged `StreamSource`: an address first, then YouTube, archives and
    // Usenet, a torrent, a player page, an outside link.
    let url = text(stream.get("url"));
    let (source, target) = if let Some(url) = url
        .as_deref()
        .filter(|url| url.starts_with("https://") || url.starts_with("http://"))
    {
        (StreamSource::Direct, url.to_owned())
    } else if let Some(url) = url.as_deref().filter(|url| url.starts_with("magnet:")) {
        (StreamSource::Torrent, url.to_owned())
    } else if let Some(id) = text(stream.get("ytId")) {
        (StreamSource::YouTube, id)
    } else if let Some(url) = url.filter(|url| url::Url::parse(url).is_ok()) {
        (StreamSource::Server, url)
    } else if let Some(key) = [
        "rarUrls", "zipUrls", "7zipUrls", "tgzUrls", "tarUrls", "nzbUrl", "nzbUrls",
    ]
    .into_iter()
    .find(|key| stream.get(*key).is_some_and(|value| !value.is_null()))
    {
        (StreamSource::Server, key.to_owned())
    } else if let Some(hash) = text(stream.get("infoHash"))
        .filter(|hash| hash.len() == 40 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        let title = filename.as_deref().or(name.as_deref());
        (StreamSource::Torrent, magnet(stream, &hash, title))
    } else if let Some(page) = http(stream.get("playerFrameUrl")) {
        (StreamSource::External, page)
    } else {
        let link = text(stream.get("externalUrl")).or_else(|| text(stream.get("androidTvUrl")))?;
        match title_link(&link) {
            Some(title) => (StreamSource::Title, title),
            None if link.starts_with("https://") || link.starts_with("http://") => {
                (StreamSource::External, link)
            }
            None => return None,
        }
    };
    Some(Stream {
        source,
        name,
        description: text(stream.get("description")).or_else(|| text(stream.get("title"))),
        headers: request_headers(stream),
        subtitles: parse_subtitles(stream),
        binge_group: hint_text("bingeGroup"),
        size: hints
            .and_then(|hints| hints.get("videoSize"))
            .and_then(Value::as_u64),
        video_hash: hint_text("videoHash"),
        not_web_ready: flag(hints, "notWebReady"),
        filename,
        target,
    })
}

/// A `stream` answer.
pub fn parse_streams(value: &Value) -> Vec<Stream> {
    value
        .get("streams")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(parse_stream)
        .take(MAX_STREAMS)
        .collect()
}

/// A `subtitles` answer (or a stream's own `subtitles`), each address once.
pub fn parse_subtitles(value: &Value) -> Vec<Subtitle> {
    let mut seen = HashSet::new();
    value
        .get("subtitles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|subtitle| {
            Some(Subtitle {
                url: http(subtitle.get("url"))?,
                lang: text(subtitle.get("lang")).or_else(|| text(subtitle.get("language"))),
                label: text(subtitle.get("label")),
            })
        })
        .filter(|subtitle| seen.insert(subtitle.url.clone()))
        .take(MAX_SUBTITLES_READ)
        .collect()
}

/// A language's other codes and English name, for matching what addons write in `lang` (ISO
/// 639-1, both kinds of ISO 639-2, OpenSubtitles' own, or the name).
pub(crate) fn language_names(code: &str) -> &'static [&'static str] {
    match code {
        "en" => &["eng", "english"],
        "es" => &["spa", "spn", "ea", "spanish"],
        "fr" => &["fre", "fra", "french"],
        "de" => &["ger", "deu", "german"],
        "pt" => &["por", "pob", "pb", "portuguese"],
        "it" => &["ita", "italian"],
        "ja" => &["jpn", "japanese"],
        "ru" => &["rus", "russian"],
        "zh" => &["chi", "zho", "zht", "zhs", "ze", "chinese"],
        "ko" => &["kor", "korean"],
        "ar" => &["ara", "arabic"],
        "nl" => &["dut", "nld", "dutch"],
        "pl" => &["pol", "polish"],
        "tr" => &["tur", "turkish"],
        "sv" => &["swe", "swedish"],
        "no" | "nb" => &["nor", "nob", "norwegian"],
        "da" => &["dan", "danish"],
        "fi" => &["fin", "finnish"],
        "cs" => &["cze", "ces", "czech"],
        "el" => &["gre", "ell", "greek"],
        "he" => &["heb", "hebrew"],
        "hu" => &["hun", "hungarian"],
        "ro" => &["rum", "ron", "romanian"],
        "hi" => &["hin", "hindi"],
        "id" => &["ind", "indonesian"],
        "vi" => &["vie", "vietnamese"],
        "th" => &["tha", "thai"],
        "uk" => &["ukr", "ukrainian"],
        "bg" => &["bul", "bulgarian"],
        "hr" => &["hrv", "croatian"],
        "sr" => &["srp", "scc", "serbian"],
        "sk" => &["slo", "slk", "slovak"],
        "sl" => &["slv", "slovenian"],
        "fa" => &["per", "fas", "persian"],
        "ms" => &["may", "msa", "malay"],
        _ => &[],
    }
}

/// Subtitles with the preferred language first (by any of its codes or its name), at most
/// `limit`, no repeated URLs.
pub fn order_subtitles(subtitles: Vec<Subtitle>, preferred: &str, limit: usize) -> Vec<Subtitle> {
    let preferred = preferred
        .split(['-', '_'])
        .next()
        .unwrap_or(preferred)
        .to_lowercase();
    let others = language_names(&preferred);
    let matches = |subtitle: &Subtitle| {
        subtitle.lang.as_deref().is_some_and(|lang| {
            let lang = lang.to_lowercase();
            let base = lang.split(['-', '_']).next().unwrap_or(&lang);
            base == preferred || others.contains(&base) || others.contains(&lang.as_str())
        })
    };
    let mut seen = HashSet::new();
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
