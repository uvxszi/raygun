//! Live checks against real addons (network): `cargo test -p raygun-app -- --ignored stremio_live
//! --nocapture`.
//!
//! The survey asks every addon of Stremio's official collection and Cinemeta's addon catalogs
//! (fetched at run time), plus `STREMIO_ADDONS` (comma-separated manifest URLs), for its manifest,
//! a catalog page and the next one, a search, a title's details, and streams and subtitles for
//! well-known titles. Each failure is the addon's own (down, slow, broken answers) or Raygun's (an
//! answer Stremio would read that Raygun doesn't, or a request Stremio would make that Raygun
//! doesn't); a reading after stremio-core's rules decides which. `STREMIO_REPORT` names a file
//! for the results as JSON lines.

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::Value;

use super::protocol;

const TIMEOUT: Duration = Duration::from_secs(20);
/// Addons asked at the same time.
const CONCURRENCY: usize = 8;

/// Movies, episodes and anime most stream and subtitle addons know.
const PROBES: [(&str, &str); 4] = [
    ("movie", "tt0032138"),
    ("series", "tt0903747:1:1"),
    ("series", "kitsu:1:1"),
    ("movie", "tt0063350"),
];
/// Titles whose details most metadata addons have.
const META_PROBES: [(&str, &str); 3] = [
    ("movie", "tt0032138"),
    ("series", "tt0903747"),
    ("series", "kitsu:1"),
];

/// Legal addons always in the survey, whatever the lists say.
const ALWAYS: [&str; 9] = [
    "https://v3-cinemeta.strem.io/manifest.json",
    "https://opensubtitles-v3.strem.io/manifest.json",
    "https://caching.stremio.net/publicdomainmovies.now.sh/manifest.json",
    "https://watchhub.strem.io/manifest.json",
    "https://v3-channels.strem.io/manifest.json",
    "https://dev.nebulawp.org/stremio/pluto-tv-addon/manifest.json",
    "https://1fe84bc728af-stremio-radios.baby-beamup.club/manifest.json",
    "https://anime-kitsu.strem.fun/manifest.json",
    "https://94c8cb9f702d-tmdb-addon.baby-beamup.club/manifest.json",
];

const LISTS: [&str; 3] = [
    "https://api.strem.io/addonscollection.json",
    "https://v3-cinemeta.strem.io/addon_catalog/all/community.json",
    "https://v3-cinemeta.strem.io/addon_catalog/all/official.json",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Outcome {
    Ok,
    /// Not applicable: the addon doesn't offer it.
    Skip,
    /// The addon's own failure: unreachable, an error status, no JSON, or nothing to read.
    Addon,
    /// Raygun's: the addon answered what Stremio reads, and Raygun didn't.
    Raygun,
}

#[derive(Debug, Default)]
struct Report {
    url: String,
    name: String,
    /// It must be set up on its website before anyone can use it.
    needs_setup: bool,
    checks: BTreeMap<&'static str, (Outcome, String)>,
}

impl Report {
    fn note(&mut self, check: &'static str, outcome: Outcome, detail: impl Into<String>) {
        // A check run several times (streams for each probe) keeps its worst outcome.
        let detail = detail.into();
        match self.checks.get(check) {
            Some((have, _)) if rank(*have) >= rank(outcome) => {}
            _ => {
                self.checks.insert(check, (outcome, detail));
            }
        }
    }
}

fn rank(outcome: Outcome) -> u8 {
    match outcome {
        Outcome::Skip => 0,
        Outcome::Ok => 1,
        Outcome::Addon => 2,
        Outcome::Raygun => 3,
    }
}

// ---- stremio-core's reading (the reference) ----

/// encodeURIComponent, as stremio-core's `URI_COMPONENT_ENCODE_SET`.
fn reference_encode(text: &str) -> String {
    let mut out = String::new();
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

/// stremio-core's `AddonHTTPTransport::resource`: the path replaces `/manifest.json`.
fn reference_url(
    transport: &str,
    resource: &str,
    kind: &str,
    id: &str,
    extra: &[(&str, String)],
) -> String {
    if protocol::is_legacy(transport) {
        // stremio-core's legacy transport builds the same JSON-RPC query.
        return raygun::url(transport, resource, kind, id, extra);
    }
    let mut path = format!(
        "/{}/{}/{}",
        reference_encode(resource),
        reference_encode(kind),
        reference_encode(id)
    );
    if !extra.is_empty() {
        let pairs: Vec<String> = extra
            .iter()
            .map(|(name, value)| format!("{}={}", reference_encode(name), reference_encode(value)))
            .collect();
        path.push('/');
        path.push_str(&pairs.join("&"));
    }
    path.push_str(".json");
    transport.replace("/manifest.json", &path)
}

fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// stremio-core's `Manifest::is_resource_supported` for resources other than catalogs.
fn reference_serves(manifest: &Value, resource: &str, kind: &str, id: &str) -> bool {
    let Some(resources) = manifest.get("resources").and_then(Value::as_array) else {
        return false;
    };
    let Some(found) = resources.iter().find(|candidate| match candidate {
        Value::String(name) => name == resource,
        Value::Object(object) => object.get("name").and_then(Value::as_str) == Some(resource),
        _ => false,
    }) else {
        return false;
    };
    let (types, prefixes) = match found {
        Value::String(_) => (
            Some(strings(manifest.get("types"))),
            manifest
                .get("idPrefixes")
                .filter(|value| value.is_array())
                .map(|value| strings(Some(value))),
        ),
        _ => (
            found
                .get("types")
                .filter(|value| value.is_array())
                .map(|value| strings(Some(value))),
            found
                .get("idPrefixes")
                .filter(|value| value.is_array())
                .map(|value| strings(Some(value))),
        ),
    };
    types.is_some_and(|types| types.iter().any(|known| known == kind))
        && prefixes.is_none_or(|prefixes| {
            prefixes.is_empty()
                || prefixes
                    .iter()
                    .any(|prefix| id.starts_with(prefix.as_str()))
        })
}

/// Items of a list answer that stremio-core keeps (an object with what it requires).
fn reference_count(value: &Value, key: &str, valid: impl Fn(&Value) -> bool) -> usize {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| items.iter().filter(|item| valid(item)).count())
        .unwrap_or(0)
}

/// A catalog item Stremio shows: an id, a type, and something to show (Raygun leaves out cards
/// with neither a name nor a picture).
fn reference_meta_preview(item: &Value) -> bool {
    let shown = item
        .get("name")
        .and_then(Value::as_str)
        .is_some_and(|name| !name.trim().is_empty())
        || item
            .get("poster")
            .and_then(Value::as_str)
            .is_some_and(|poster| poster.starts_with("http"));
    item.get("id").and_then(Value::as_str).is_some()
        && item.get("type").and_then(Value::as_str).is_some()
        && shown
}

/// Details Stremio shows (Raygun asks the next addon when they have no name).
fn reference_meta(meta: &Value) -> bool {
    meta.get("id").and_then(Value::as_str).is_some()
        && meta.get("type").and_then(Value::as_str).is_some()
        && meta
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|name| !name.trim().is_empty())
}

fn is_url(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .is_some_and(|text| url::Url::parse(text).is_ok())
}

/// A stream stremio-core reads, and whether Raygun can play it here (a web address or YouTube).
fn reference_stream(stream: &Value) -> (bool, bool) {
    let web = stream
        .get("url")
        .and_then(Value::as_str)
        .is_some_and(|url| url.starts_with("http://") || url.starts_with("https://"));
    let youtube = stream.get("ytId").and_then(Value::as_str).is_some();
    let torrent = stream
        .get("infoHash")
        .and_then(Value::as_str)
        .is_some_and(|hash| hash.len() == 40 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let other = is_url(stream.get("url"))
        || is_url(stream.get("externalUrl"))
        || is_url(stream.get("playerFrameUrl"))
        || [
            "rarUrls", "zipUrls", "7zipUrls", "tgzUrls", "tarUrls", "nzbUrl",
        ]
        .iter()
        .any(|key| stream.get(*key).is_some());
    (web || youtube || torrent || other, web || youtube)
}

fn reference_subtitle(subtitle: &Value) -> bool {
    is_url(subtitle.get("url"))
}

// ---- HTTP ----

enum Fetched {
    Json(Value),
    Failed(String),
}

async fn fetch(client: &reqwest::Client, url: &str) -> Fetched {
    let response = match client.get(url).timeout(TIMEOUT).send().await {
        Ok(response) => response,
        Err(error) => {
            let cause = if error.is_timeout() {
                "timed out".to_owned()
            } else {
                super::root_cause(&error)
            };
            return Fetched::Failed(cause);
        }
    };
    let status = response.status();
    if !status.is_success() {
        return Fetched::Failed(format!("HTTP {status}"));
    }
    match response.bytes().await {
        Ok(bytes) => match serde_json::from_slice(&bytes) {
            Ok(value) => Fetched::Json(value),
            Err(_) => Fetched::Failed(format!(
                "not JSON: {}",
                String::from_utf8_lossy(&bytes[..bytes.len().min(80)])
            )),
        },
        Err(error) => Fetched::Failed(format!("body: {error}")),
    }
}

/// Fetches what Raygun asks for; when that fails, the reference address too, to tell whose
/// failure it is.
async fn ask(
    client: &reqwest::Client,
    raygun_url: &str,
    reference: &str,
) -> Result<Value, (Outcome, String)> {
    match fetch(client, raygun_url).await {
        Fetched::Json(value) => Ok(value),
        Fetched::Failed(error) => {
            if raygun_url != reference
                && let Fetched::Json(_) = fetch(client, reference).await
            {
                return Err((
                    Outcome::Raygun,
                    format!("{error} at {raygun_url}, but {reference} answers"),
                ));
            }
            Err((Outcome::Addon, error))
        }
    }
}

// ---- Raygun's side (the code under test) ----

mod raygun {
    use super::*;

    pub fn normalize(url: &str) -> Result<String, String> {
        protocol::normalize_addon_url(url)
    }

    pub fn manifest_url(transport: &str) -> String {
        protocol::manifest_url(transport)
    }

    pub fn url(
        transport: &str,
        resource: &str,
        kind: &str,
        id: &str,
        extra: &[(&str, String)],
    ) -> String {
        let extra: Vec<(String, String)> = extra
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect();
        protocol::resource_url(transport, resource, kind, id, &extra)
    }

    /// An answer in today's shape (a legacy addon's JSON-RPC unwrapped).
    pub fn answer(transport: &str, resource: &str, value: Value) -> Result<Value, String> {
        if protocol::is_legacy(transport) {
            protocol::legacy_answer(resource, value)
        } else {
            Ok(value)
        }
    }

    pub struct Manifest(pub protocol::Manifest);

    pub fn manifest(value: &Value) -> Result<Manifest, String> {
        protocol::parse_manifest(value).map(Manifest)
    }

    pub struct Catalog {
        pub kind: String,
        pub id: String,
        pub skip: bool,
        pub search: bool,
        pub board: bool,
    }

    impl Manifest {
        pub fn serves(&self, resource: &str, kind: &str, id: &str) -> bool {
            self.0.serves(resource, kind, id)
        }

        pub fn needs_setup(&self) -> bool {
            self.0.configuration_required
        }

        pub fn catalogs(&self) -> Vec<Catalog> {
            self.0
                .catalogs
                .iter()
                .map(|catalog| Catalog {
                    kind: catalog.kind.clone(),
                    id: catalog.id.clone(),
                    skip: catalog.pages(),
                    search: catalog.searchable(),
                    board: catalog.browsable(),
                })
                .collect()
        }
    }

    /// Items as (type, id).
    pub fn metas(value: &Value, kind: &str) -> Vec<(String, String)> {
        protocol::parse_metas(value, kind)
            .into_iter()
            .map(|item| (item.kind, item.id))
            .collect()
    }

    pub fn meta(value: &Value, kind: &str) -> Option<protocol::Meta> {
        protocol::parse_meta(value, kind)
    }

    /// (streams read, playable here).
    pub fn streams(value: &Value) -> (usize, usize) {
        let streams = protocol::parse_streams(value);
        let playable = streams
            .iter()
            .filter(|stream| {
                matches!(
                    stream.source,
                    protocol::StreamSource::Direct | protocol::StreamSource::YouTube
                )
            })
            .count();
        (streams.len(), playable)
    }

    pub fn subtitles(value: &Value) -> usize {
        protocol::parse_subtitles(value).len()
    }
}

/// Asks for a resource the Raygun way, its answer unwrapped.
async fn ask_resource(
    client: &reqwest::Client,
    transport: &str,
    resource: &str,
    kind: &str,
    id: &str,
    extra: &[(&str, String)],
) -> Result<Value, (Outcome, String)> {
    let url = raygun::url(transport, resource, kind, id, extra);
    let reference = reference_url(transport, resource, kind, id, extra);
    let value = ask(client, &url, &reference).await?;
    raygun::answer(transport, resource, value).map_err(|error| (Outcome::Addon, error))
}

async fn survey_one(client: &reqwest::Client, listed: String) -> Report {
    let mut report = Report {
        url: listed.clone(),
        ..Default::default()
    };
    // The address as added.
    let transport = match raygun::normalize(&listed) {
        Ok(url) => url,
        Err(error) => {
            report.note(
                "manifest",
                Outcome::Raygun,
                format!("address refused: {error}"),
            );
            return report;
        }
    };
    let fetched = ask(client, &raygun::manifest_url(&transport), &listed)
        .await
        .and_then(|value| {
            raygun::answer(&transport, "manifest", value).map_err(|error| (Outcome::Addon, error))
        });
    let value = match fetched {
        Ok(value) => value,
        Err((outcome, error)) => {
            report.note("manifest", outcome, error);
            return report;
        }
    };
    report.name = value
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let manifest = match raygun::manifest(&value) {
        Ok(manifest) => {
            report.note("manifest", Outcome::Ok, "");
            manifest
        }
        Err(error) => {
            // Stremio needs an id, a name, a version, types and resources.
            let valid = ["id", "name", "version"]
                .iter()
                .all(|key| value.get(*key).and_then(Value::as_str).is_some())
                && value.get("types").is_some_and(Value::is_array)
                && value
                    .get("resources")
                    .and_then(Value::as_array)
                    .is_some_and(|resources| !resources.is_empty());
            let outcome = if valid {
                Outcome::Raygun
            } else {
                Outcome::Addon
            };
            report.note("manifest", outcome, error);
            return report;
        }
    };
    if manifest.needs_setup() {
        // Neither Stremio nor Raygun uses it before it's set up on its website.
        report.needs_setup = true;
        return report;
    }
    let catalogs = manifest.catalogs();

    // A catalog page, the next one, and what's on it.
    let mut listed_item: Option<(String, String)> = None;
    match catalogs.iter().find(|catalog| catalog.board) {
        None => {
            report.note("catalog", Outcome::Skip, "");
            report.note("skip", Outcome::Skip, "");
        }
        Some(catalog) => {
            match ask_resource(
                client,
                &transport,
                "catalog",
                &catalog.kind,
                &catalog.id,
                &[],
            )
            .await
            {
                Err((outcome, error)) => report.note("catalog", outcome, error),
                Ok(page) => {
                    let ours = raygun::metas(&page, &catalog.kind);
                    let theirs = reference_count(&page, "metas", reference_meta_preview);
                    if theirs == 0 && ours.is_empty() {
                        report.note("catalog", Outcome::Addon, "empty page");
                    } else if ours.len() < theirs {
                        report.note(
                            "catalog",
                            Outcome::Raygun,
                            format!("read {} of {theirs} items", ours.len()),
                        );
                    } else {
                        report.note("catalog", Outcome::Ok, format!("{} items", ours.len()));
                    }
                    listed_item = ours.first().cloned();
                    if catalog.skip && !ours.is_empty() {
                        let extra = [("skip", ours.len().to_string())];
                        match ask_resource(
                            client,
                            &transport,
                            "catalog",
                            &catalog.kind,
                            &catalog.id,
                            &extra,
                        )
                        .await
                        {
                            Err((outcome, error)) => report.note("skip", outcome, error),
                            Ok(next) => {
                                let ours = raygun::metas(&next, &catalog.kind);
                                let theirs =
                                    reference_count(&next, "metas", reference_meta_preview);
                                if ours.len() < theirs {
                                    report.note(
                                        "skip",
                                        Outcome::Raygun,
                                        format!("read {} of {theirs}", ours.len()),
                                    );
                                } else {
                                    report.note(
                                        "skip",
                                        Outcome::Ok,
                                        format!("{} items", ours.len()),
                                    );
                                }
                            }
                        }
                    } else {
                        report.note("skip", Outcome::Skip, "");
                    }
                }
            }
        }
    }

    // A search.
    match catalogs.iter().find(|catalog| catalog.search) {
        None => report.note("search", Outcome::Skip, ""),
        Some(catalog) => {
            let extra = [("search", "star wars".to_owned())];
            match ask_resource(
                client,
                &transport,
                "catalog",
                &catalog.kind,
                &catalog.id,
                &extra,
            )
            .await
            {
                Err((outcome, error)) => report.note("search", outcome, error),
                Ok(page) => {
                    let ours = raygun::metas(&page, &catalog.kind);
                    let theirs = reference_count(&page, "metas", reference_meta_preview);
                    if ours.len() < theirs {
                        report.note(
                            "search",
                            Outcome::Raygun,
                            format!("read {} of {theirs}", ours.len()),
                        );
                    } else {
                        report.note("search", Outcome::Ok, format!("{} items", ours.len()));
                    }
                }
            }
        }
    }

    // Details: of an item it lists, else of the probes.
    let mut meta_asks: Vec<(String, String)> = listed_item.into_iter().collect();
    meta_asks.extend(
        META_PROBES
            .iter()
            .map(|(kind, id)| ((*kind).to_owned(), (*id).to_owned())),
    );
    let mut asked_meta = false;
    for (kind, id) in meta_asks {
        let ours = manifest.serves("meta", &kind, &id);
        let theirs = reference_serves(&value, "meta", &kind, &id);
        if !ours && !theirs {
            continue;
        }
        asked_meta = true;
        if !ours {
            let reference = reference_url(&transport, "meta", &kind, &id, &[]);
            if let Fetched::Json(answer) = fetch(client, &reference).await
                && answer.get("meta").is_some_and(reference_meta)
            {
                report.note(
                    "meta",
                    Outcome::Raygun,
                    format!("never asked for {kind}/{id}"),
                );
            }
            break;
        }
        match ask_resource(client, &transport, "meta", &kind, &id, &[]).await {
            Err((outcome, error)) => report.note("meta", outcome, error),
            Ok(answer) => match raygun::meta(&answer, &kind) {
                Some(meta) => {
                    // Streams and subtitles are asked by an id most addons know.
                    let video = meta
                        .videos
                        .first()
                        .map_or_else(|| meta.own_video_id(), |video| video.id.clone());
                    let ids = meta.stream_ids_for_video(&video);
                    report.note(
                        "meta",
                        Outcome::Ok,
                        format!("{kind}/{id}: streams by {ids:?}"),
                    );
                }
                None if answer.get("meta").is_some_and(reference_meta) => report.note(
                    "meta",
                    Outcome::Raygun,
                    format!("couldn't read {kind}/{id}"),
                ),
                None => report.note(
                    "meta",
                    Outcome::Addon,
                    format!("no details for {kind}/{id}"),
                ),
            },
        }
        break;
    }
    if !asked_meta {
        report.note("meta", Outcome::Skip, "");
    }

    // Streams and subtitles for the probes.
    for (resource, check) in [("stream", "streams"), ("subtitles", "subtitles")] {
        let key = if resource == "stream" {
            "streams"
        } else {
            "subtitles"
        };
        let mut asked = false;
        for (kind, id) in PROBES {
            let ours = manifest.serves(resource, kind, id);
            let theirs = reference_serves(&value, resource, kind, id);
            if !ours && !theirs {
                continue;
            }
            asked = true;
            let answer = if ours {
                match ask_resource(client, &transport, resource, kind, id, &[]).await {
                    Ok(answer) => answer,
                    Err((outcome, error)) => {
                        report.note(check, outcome, format!("{kind}/{id}: {error}"));
                        continue;
                    }
                }
            } else {
                let reference = reference_url(&transport, resource, kind, id, &[]);
                if let Fetched::Json(answer) = fetch(client, &reference).await {
                    let found = reference_count(&answer, key, |_| true);
                    if found > 0 {
                        report.note(
                            check,
                            Outcome::Raygun,
                            format!("never asked for {kind}/{id} ({found} there)"),
                        );
                    }
                }
                continue;
            };
            if resource == "stream" {
                let (read, playable) = raygun::streams(&answer);
                let all = answer.get("streams").and_then(Value::as_array);
                let theirs = all.map_or(0, |items| {
                    items.iter().filter(|item| reference_stream(item).0).count()
                });
                let theirs_playable = all.map_or(0, |items| {
                    items.iter().filter(|item| reference_stream(item).1).count()
                });
                if read < theirs || playable < theirs_playable {
                    report.note(
                        check,
                        Outcome::Raygun,
                        format!(
                            "{kind}/{id}: read {read} of {theirs} ({playable} of {theirs_playable} playable)"
                        ),
                    );
                } else {
                    report.note(
                        check,
                        Outcome::Ok,
                        format!("{kind}/{id}: {read} ({playable} playable)"),
                    );
                }
            } else {
                let read = raygun::subtitles(&answer);
                let theirs = reference_count(&answer, "subtitles", reference_subtitle);
                if read < theirs {
                    report.note(
                        check,
                        Outcome::Raygun,
                        format!("{kind}/{id}: read {read} of {theirs}"),
                    );
                } else {
                    report.note(check, Outcome::Ok, format!("{kind}/{id}: {read}"));
                }
            }
        }
        if !asked {
            report.note(check, Outcome::Skip, "");
        }
    }
    report
}

async fn addon_urls(client: &reqwest::Client) -> Vec<String> {
    let mut urls: Vec<String> = ALWAYS.iter().map(|url| (*url).to_owned()).collect();
    for list in LISTS {
        let Fetched::Json(value) = fetch(client, list).await else {
            continue;
        };
        let entries = value
            .as_array()
            .cloned()
            .or_else(|| value.get("addons").and_then(Value::as_array).cloned())
            .unwrap_or_default();
        for entry in entries {
            if let Some(url) = entry.get("transportUrl").and_then(Value::as_str) {
                urls.push(url.to_owned());
            }
        }
    }
    if let Ok(extra) = std::env::var("STREMIO_ADDONS") {
        urls.extend(
            extra
                .split(',')
                .map(str::trim)
                .filter(|url| !url.is_empty())
                .map(str::to_owned),
        );
    }
    let mut seen = std::collections::HashSet::new();
    urls.retain(|url| {
        // The local streaming server isn't an addon anyone else reaches.
        !url.contains("127.0.0.1") && seen.insert(url.trim_end_matches('/').to_lowercase())
    });
    urls
}

#[tokio::test]
#[ignore = "network: surveys every addon of Stremio's lists"]
async fn stremio_live_survey() {
    let client = raygun_net::http_client().unwrap();
    let urls = addon_urls(&client).await;
    let reports = super::at_most(
        CONCURRENCY,
        urls.into_iter().map(|url| survey_one(&client, url)),
    )
    .await;

    let checks = [
        "manifest",
        "catalog",
        "skip",
        "search",
        "meta",
        "streams",
        "subtitles",
    ];
    let mut totals: BTreeMap<&str, [usize; 4]> = BTreeMap::new();
    let (mut working, mut ours, mut theirs, mut setup) = (0, 0, 0, 0);
    for report in &reports {
        let mut worst = Outcome::Skip;
        let mut line = String::new();
        for check in checks {
            let (outcome, detail) = report
                .checks
                .get(check)
                .cloned()
                .unwrap_or((Outcome::Skip, String::new()));
            totals.entry(check).or_default()[outcome as usize] += 1;
            if rank(outcome) > rank(worst) {
                worst = outcome;
            }
            if outcome != Outcome::Skip {
                line.push_str(&format!(" {check}={outcome:?}"));
                if !detail.is_empty() && outcome != Outcome::Ok {
                    line.push_str(&format!(" ({detail})"));
                }
            }
        }
        let label = match worst {
            Outcome::Raygun => {
                ours += 1;
                "Raygun"
            }
            Outcome::Addon => {
                theirs += 1;
                "Addon"
            }
            _ if report.needs_setup => {
                setup += 1;
                "Setup"
            }
            _ => {
                working += 1;
                "Ok"
            }
        };
        println!("[{label}] {} <{}>{line}", report.name, report.url);
    }
    println!();
    for (check, [ok, skip, addon, raygun]) in &totals {
        println!("{check:>9}: ok {ok}, n/a {skip}, addon's fault {addon}, Raygun's fault {raygun}");
    }
    let total = reports.len();
    let usable = total - setup;
    println!(
        "{total} addons ({setup} need setting up on their website first): {working} fully \
         working, {theirs} with the addon's own failures, {ours} with Raygun failures; {:.0}% of \
         the usable ones fully work, {:.0}% have no Raygun failure",
        100.0 * working as f64 / usable.max(1) as f64,
        100.0 * (usable - ours) as f64 / usable.max(1) as f64,
    );
    if let Ok(path) = std::env::var("STREMIO_REPORT") {
        let lines: Vec<String> = reports
            .iter()
            .map(|report| {
                serde_json::json!({
                    "url": report.url,
                    "name": report.name,
                    "needsSetup": report.needs_setup,
                    "checks": report.checks.iter().map(|(check, (outcome, detail))| {
                        (check.to_string(), serde_json::json!([format!("{outcome:?}"), detail]))
                    }).collect::<serde_json::Map<_, _>>(),
                })
                .to_string()
            })
            .collect();
        std::fs::write(path, lines.join("\n")).unwrap();
    }
}

/// The flows behind "addons added aren't used": a TMDB catalog's movie gets TMDB's details and
/// its streams are asked by the IMDb id they give; an anime's episodes by their IMDb episode; a
/// YouTube channel's videos play on YouTube.
#[tokio::test]
#[ignore = "network: TMDB, Kitsu and YouTube addons"]
async fn stremio_live_flows() {
    let client = raygun_net::http_client().unwrap();
    let get = |url: String| {
        let client = &client;
        async move {
            match fetch(client, &url).await {
                Fetched::Json(value) => Some(value),
                Fetched::Failed(error) => {
                    println!("{url}: {error}");
                    None
                }
            }
        }
    };

    let tmdb = "https://94c8cb9f702d-tmdb-addon.baby-beamup.club/manifest.json";
    if let Some(page) = get(raygun::url(tmdb, "catalog", "movie", "tmdb.top", &[])).await {
        let items = protocol::parse_metas(&page, "movie");
        assert!(!items.is_empty(), "TMDB's popular movies");
        let mut checked = 0;
        for item in items.iter().take(5) {
            let Some(answer) = get(raygun::url(tmdb, "meta", "movie", &item.id, &[])).await else {
                continue;
            };
            let meta = protocol::parse_meta(&answer, "movie").expect("TMDB's details read");
            let ids = meta.stream_ids_for_video(&meta.own_video_id());
            println!("{} {}: streams by {ids:?}", item.id, meta.preview.name);
            assert!(
                ids.iter().any(|id| id.starts_with("tt")),
                "an IMDb id for the stream addons"
            );
            checked += 1;
        }
        println!("{checked} TMDB movies checked");
    }

    let kitsu = "https://anime-kitsu.strem.fun/manifest.json";
    if let Some(answer) = get(raygun::url(kitsu, "meta", "series", "kitsu:1", &[])).await {
        let meta = protocol::parse_meta(&answer, "series").expect("Kitsu's details read");
        assert!(meta.videos.len() >= 26, "Cowboy Bebop's sessions");
        assert_eq!(
            meta.stream_ids_for_video("kitsu:1:5"),
            ["kitsu:1:5", "tt0213338:1:5"]
        );
    }

    let channels = "https://v3-channels.strem.io/manifest.json";
    if let Some(page) = get(raygun::url(channels, "catalog", "channel", "top", &[])).await
        && let Some(channel) = protocol::parse_metas(&page, "channel").first()
        && let Some(answer) = get(raygun::url(channels, "meta", "channel", &channel.id, &[])).await
    {
        let meta = protocol::parse_meta(&answer, "channel").expect("a channel's details read");
        let first = meta.videos.first().expect("a channel's videos");
        assert_eq!(
            first.streams.first().map(|stream| stream.source),
            Some(protocol::StreamSource::YouTube),
            "a channel's videos play on YouTube"
        );
        let newest = meta
            .videos
            .iter()
            .filter_map(|video| video.released.clone())
            .max();
        assert_eq!(first.released, newest, "newest first");
    }
}
