//! What an address added in Sources is (M46.52): a Stremio addon, a repository in one of the
//! formats Raygun reads (its own, Shosetsu's, Nuclear's registry, Miru's, LNReader's, Kodi's
//! `addons.xml`, Mihon's `index.min.json`), one extension on its own (a Raygun package, a Miru
//! or Shosetsu script), or a server people sign in to (Jellyfin, Emby, Subsonic, Audiobookshelf,
//! an OPDS catalogue).
//!
//! [`detect`] fetches the address once, following redirects, reading at most [`MAX_BYTES`], and
//! goes by what the answer holds. Only when that's nothing Raygun can add are the places beside
//! it asked, all at once, the first in order that answers winning: the addon manifest a bare
//! address stands for, a repository folder's index, Kodi's gzipped index, and the servers'
//! well-known endpoints. A GitHub project's page stands for what the project publishes
//! ([`github_places`]); a folder on this device for the index in it. [`SourceDetector`] shows
//! what was found and adds it.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;
use raygun_ext_host::package::{self, MAX_PACKAGE_BYTES};
use raygun_ext_host::repo::{self, PackageFormat, RepoEntry, RepoError, RepoIndex};
use raygun_ext_host::{kodi, mihon, miru, shosetsu};
use raygun_model::{DetectedKind, DetectedSource, ServerKind, SourceFormat};
use reqwest::Url;
use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::extensions::ExtensionService;
use crate::local_network::{LocalNetwork, SOURCES_DENIED};
use crate::servers::ServerService;
use crate::stremio::StremioService;
use crate::stremio::protocol::{self, Manifest};

/// The most of an answer that's read: a whole extension package, and so any index.
pub const MAX_BYTES: u64 = MAX_PACKAGE_BYTES;
/// How long one request may take.
const TIMEOUT: Duration = Duration::from_secs(20);
/// How long what an address turned out to be is kept for adding it without asking again.
const REMEMBERED: Duration = Duration::from_secs(10 * 60);
/// The content type a folder on this device answers with.
const FOLDER: &str = "inode/directory";

/// One answer.
#[derive(Debug, Clone)]
pub struct Fetched {
    /// Where the answer came from, after redirects.
    pub url: Url,
    pub status: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
    /// The answer was longer than [`MAX_BYTES`]: `body` is its start.
    pub truncated: bool,
}

impl Fetched {
    fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    fn folder(&self) -> bool {
        self.content_type.as_deref() == Some(FOLDER)
    }
}

/// Asks for one address. A request nothing answered (a host that can't be reached, a refused
/// certificate) is an error, saying why; any HTTP answer is an answer. A folder on this device
/// answers with [`FOLDER`] and no body.
pub trait Fetch: Sync {
    fn get(&self, url: &Url) -> impl Future<Output = Result<Fetched, String>> + Send;
}

/// What an address turned out to be.
#[derive(Debug, Clone)]
pub enum Found {
    /// A Stremio addon: its manifest's address and what the manifest says.
    Addon {
        url: String,
        manifest: Box<Manifest>,
    },
    /// A repository: its index's address, its format and what it lists.
    Repository {
        url: Url,
        format: PackageFormat,
        index: RepoIndex,
    },
    /// One extension: where it is, the entry a repository would list for it, and the file.
    Extension {
        url: Url,
        entry: Box<RepoEntry>,
        bytes: Vec<u8>,
    },
    /// A server people sign in to, at its address, and the name it gives itself.
    Server {
        kind: ServerKind,
        url: Url,
        name: Option<String>,
    },
}

/// What one answer says.
enum Reading {
    Found(Found),
    /// It names where the thing is: a Kodi repository's own add-on names its index.
    Follow(Url),
    /// It's something Raygun knows and can't add (another app's repository), saying why.
    Refused(String),
    /// A Mihon store (`index.min.json`, `repo.json`, `index.pb`), read with the files it names
    /// ([`mihon_store`]).
    Mihon(Box<Fetched>),
}

/// A Mihon store, followed to its extensions as adding it does (`raygun_ext_host::mihon::load`:
/// a legacy list's `repo.json`, then its newer index). The repository is the address read.
async fn mihon_store<F: Fetch>(fetch: &F, answer: &Fetched) -> Result<Found, String> {
    let (_, store) = mihon::load(&answer.url, answer.body.clone(), |url: Url| async move {
        let next = fetch.get(&url).await?;
        if next.ok() && !next.truncated {
            Ok(next.body)
        } else {
            Err(format!("{url} answered HTTP {}", next.status))
        }
    })
    .await?;
    Ok(Found::Repository {
        url: answer.url.clone(),
        format: PackageFormat::Mihon,
        index: mihon::to_repo_index(&store)?,
    })
}

/// A place to ask, and the address a server found there has.
#[derive(Debug, Clone, PartialEq)]
struct Place {
    url: Url,
    base: Url,
}

impl Place {
    fn at(url: Url) -> Self {
        Self {
            base: url.clone(),
            url,
        }
    }
}

/// Reads what `typed` is (see the module's notes).
pub async fn detect<F: Fetch>(fetch: &F, typed: &str) -> Result<Found, String> {
    let typed = typed.trim();
    if typed.is_empty() {
        return Err("Type or paste an address".to_owned());
    }
    let forms = addresses(typed)?;
    let first = forms[0].clone();
    if let Some(places) = github_places(&first).or_else(|| gitlab_places(&first)) {
        let project = first.path().trim_matches('/').to_owned();
        return match first_found(fetch, &places).await {
            Some(Ok(found)) => checked(found),
            Some(Err(refused)) => Err(refused),
            None => Err(format!(
                "{project} publishes no repository, addon or extension Raygun knows"
            )),
        };
    }
    // Stremio's first protocol: the manifest is a query.
    if protocol::is_legacy(first.as_str()) {
        let answer = fetch
            .get(&Url::parse(&protocol::manifest_url(first.as_str())).map_err(|e| e.to_string())?)
            .await?;
        let value: Value = serde_json::from_slice(&answer.body)
            .map_err(|_| format!("{first} didn't answer as a Stremio addon does"))?;
        let manifest = protocol::parse_manifest(&protocol::legacy_answer("manifest", value)?)?;
        return Ok(Found::Addon {
            url: first.to_string(),
            manifest: Box::new(manifest),
        });
    }
    let mut unreachable = None;
    for url in &forms {
        match fetch.get(url).await {
            Ok(answer) => return look_at(fetch, url, answer).await.and_then(checked),
            // Without a scheme, `http://` comes after `https://` didn't connect.
            Err(error) => unreachable = Some(error),
        }
    }
    Err(unreachable.unwrap_or_else(|| format!("Couldn't reach {typed}")))
}

/// What an address answered with is, else what the places beside it are.
async fn look_at<F: Fetch>(fetch: &F, asked: &Url, answer: Fetched) -> Result<Found, String> {
    if answer.folder() {
        return match first_found(fetch, &folder_places(&answer.url)).await {
            Some(found) => found,
            None => Err(format!(
                "{} holds no repository index",
                answer.url.to_file_path().map_or_else(
                    |()| answer.url.to_string(),
                    |path| path.display().to_string()
                )
            )),
        };
    }
    let mut refused = None;
    if answer.ok() {
        match read(&answer, asked) {
            Some(Reading::Found(found)) => return Ok(found),
            Some(Reading::Follow(next)) => {
                if let Some(found) = follow(fetch, &next).await {
                    return found;
                }
            }
            Some(Reading::Refused(message)) => refused = Some(message),
            Some(Reading::Mihon(store)) => return mihon_store(fetch, &store).await,
            None => {}
        }
    }
    if let Some(message) = refused {
        return Err(message);
    }
    match first_found(fetch, &beside(asked, &answer)).await {
        Some(found) => found,
        None => Err(nothing_at(asked, &answer)),
    }
}

/// Why nothing was found at `asked`, in a sentence, with what it answered after.
fn nothing_at(asked: &Url, answer: &Fetched) -> String {
    let host = asked.host_str().unwrap_or("the address");
    let summary = format!("Raygun found no repository, addon, server or extension at {host}");
    let detail = if answer.ok() {
        String::new()
    } else {
        format!("{} answered HTTP {}", answer.url, answer.status)
    };
    raygun_net::issue::message(&summary, &detail)
}

/// Repositories and extensions are read only from https, local test hosts and files Raygun may
/// read (`repo::check_index_url`).
fn checked(found: Found) -> Result<Found, String> {
    if let Found::Repository { url, .. } | Found::Extension { url, .. } = &found {
        repo::check_index_url(url.as_str()).map_err(|error| error.to_string())?;
    }
    Ok(found)
}

/// The addresses `typed` may be: as typed, Stremio's `stremio://` as https, a path on this
/// computer as a file, and without a scheme `https://` and then `http://` (servers and addons
/// on a home network rarely have certificates).
fn addresses(typed: &str) -> Result<Vec<Url>, String> {
    let not_one = || format!("“{typed}” isn't an address");
    let fragmentless = |mut url: Url| {
        url.set_fragment(None);
        url
    };
    if let Some(rest) = typed.strip_prefix("stremio://") {
        return Url::parse(&format!("https://{rest}"))
            .map(|url| vec![fragmentless(url)])
            .map_err(|_| not_one());
    }
    if std::path::Path::new(typed).is_absolute() {
        return Url::from_file_path(typed)
            .map(|url| vec![url])
            .map_err(|()| format!("{typed} isn't a usable path"));
    }
    if typed.contains("://") {
        let url = Url::parse(typed).map_err(|_| not_one())?;
        return match url.scheme() {
            "http" | "https" if url.host_str().is_some() => Ok(vec![fragmentless(url)]),
            "file" => Ok(vec![url]),
            _ => Err(format!("{typed} isn't a web address")),
        };
    }
    let forms: Vec<Url> = ["https", "http"]
        .iter()
        .filter_map(|scheme| Url::parse(&format!("{scheme}://{typed}")).ok())
        .filter(|url| {
            url.host_str().is_some_and(|host| {
                host.contains('.') || host == "localhost" || url.port().is_some()
            })
        })
        .map(fragmentless)
        .collect();
    if forms.is_empty() {
        return Err(not_one());
    }
    Ok(forms)
}

/// The raw files a GitHub project's page stands for, in the order they're taken: Mihon's and
/// Aniyomi's `repo` branch, an index at the root of the default branch (Raygun's, Shosetsu's,
/// Miru's), a static Stremio addon, Nuclear's registry, LNReader's plugins, a Kodi index. A page
/// of a branch's folder (`/tree/<branch>/<folder>`) stands for the same names in it, and a file's
/// page (`/blob/…`) for the file. Other GitHub addresses (release downloads) are asked as they
/// are.
fn github_places(url: &Url) -> Option<Vec<Place>> {
    if !matches!(url.host_str(), Some("github.com" | "www.github.com")) {
        return None;
    }
    let parts: Vec<&str> = url
        .path_segments()?
        .filter(|part| !part.is_empty())
        .collect();
    let (owner, project) = (*parts.first()?, parts.get(1)?.trim_end_matches(".git"));
    let raw = |path: &str| {
        Url::parse(&format!(
            "https://raw.githubusercontent.com/{owner}/{project}/{path}"
        ))
        .ok()
        .map(Place::at)
    };
    let places = match parts.get(2..) {
        Some([]) => [
            "repo/index.min.json",
            "HEAD/index.json",
            "HEAD/index.min.json",
            "HEAD/manifest.json",
            "HEAD/plugins.json",
            "plugins/v3.0.0/.dist/plugins.min.json",
            "HEAD/addons.xml",
        ]
        .iter()
        .filter_map(|path| raw(path))
        .collect(),
        Some(["tree", branch, folder @ ..]) => {
            let folder: String = folder.iter().map(|part| format!("{part}/")).collect();
            INDEX_NAMES
                .iter()
                .chain(["manifest.json"].iter())
                .filter_map(|name| raw(&format!("{branch}/{folder}{name}")))
                .collect()
        }
        Some(["blob", branch, file @ ..]) if !file.is_empty() => {
            raw(&format!("{branch}/{}", file.join("/")))
                .into_iter()
                .collect()
        }
        _ => return None,
    };
    Some(places)
}

/// A GitLab project's page: the index at the root of its usual branches.
fn gitlab_places(url: &Url) -> Option<Vec<Place>> {
    if url.host_str() != Some("gitlab.com") || url.path().contains("/-/raw/") {
        return None;
    }
    let places: Vec<Place> = repo::shosetsu_index_candidates(url)
        .into_iter()
        .map(Place::at)
        .collect();
    (!places.is_empty()).then_some(places)
}

/// The index files a repository's folder may hold, in the order they're taken.
const INDEX_NAMES: [&str; 5] = [
    "index.json",
    "index.min.json",
    "plugins.min.json",
    "plugins.json",
    "addons.xml",
];

/// A folder on this device: the index in it.
fn folder_places(folder: &Url) -> Vec<Place> {
    let mut folder = folder.clone();
    if !folder.path().ends_with('/') {
        folder.set_path(&format!("{}/", folder.path()));
    }
    INDEX_NAMES
        .iter()
        .filter_map(|name| folder.join(name).ok())
        .map(Place::at)
        .collect()
}

/// Servers' well-known endpoints, under a server's address: Jellyfin's and Emby's public
/// details, Subsonic's ping (which answers without a sign-in, with an error), Audiobookshelf's
/// status, and where Komga's and other servers' OPDS catalogues are.
const SERVER_PROBES: [&str; 6] = [
    "System/Info/Public",
    "emby/System/Info/Public",
    "rest/ping.view?v=1.16.1&c=Raygun&f=json",
    "status",
    "opds/v1.2/catalog",
    "opds",
];

/// `path` under `base`'s path, as a folder.
fn under(base: &Url, path: &str) -> Option<Url> {
    let mut folder = base.clone();
    folder.set_query(None);
    folder.set_fragment(None);
    if !folder.path().ends_with('/') {
        folder.set_path(&format!("{}/", folder.path()));
    }
    folder.join(path).ok()
}

/// Whether a URL's last part names a file (`index.json`, `page.html`) rather than a folder.
fn names_a_file(url: &Url) -> bool {
    url.path_segments()
        .and_then(|mut parts| parts.rfind(|part| !part.is_empty()))
        .is_some_and(|last| last.contains('.'))
}

/// Where to look when `asked` answered with nothing Raygun can add, in order: the addon a bare
/// address stands for, a repository folder's index, Kodi's gzipped index, then servers, under
/// the address and under its site's root.
fn beside(asked: &Url, answer: &Fetched) -> Vec<Place> {
    let mut places: Vec<Place> = Vec::new();
    let mut add = |place: Place| {
        if place.url != *asked && !places.iter().any(|known| known.url == place.url) {
            places.push(place);
        }
    };
    if !matches!(asked.scheme(), "http" | "https") {
        return Vec::new();
    }
    let file = names_a_file(asked);
    if !file
        && let Ok(manifest) = protocol::normalize_addon_url(asked.as_str())
        && let Ok(url) = Url::parse(&manifest)
    {
        add(Place::at(url));
    }
    if !file {
        for name in ["index.json", "index.min.json"] {
            if let Some(url) = under(asked, name) {
                add(Place::at(url));
            }
        }
    }
    if asked.path().to_ascii_lowercase().ends_with("/addons.xml")
        && let Ok(packed) = Url::parse(&format!("{asked}.gz"))
    {
        add(Place::at(packed));
    }
    let mut root = asked.clone();
    root.set_path("/");
    root.set_query(None);
    root.set_fragment(None);
    let mut bases = vec![root];
    if !file && !asked.path().trim_matches('/').is_empty() {
        let mut base = asked.clone();
        base.set_query(None);
        base.set_fragment(None);
        bases.insert(0, base);
    }
    // A redirect to another site (a web app's sign-in page) says where the server is.
    if answer.url.origin() != asked.origin() && matches!(answer.url.scheme(), "http" | "https") {
        let mut moved = answer.url.clone();
        moved.set_path("/");
        moved.set_query(None);
        bases.push(moved);
    }
    for base in bases {
        for probe in SERVER_PROBES {
            if let Some(url) = under(&base, probe) {
                add(Place {
                    url,
                    base: base.clone(),
                });
            }
        }
    }
    places
}

/// The first of `places`, in order, that's something to add (or that refused, saying why).
/// They're asked all at once; a later one that answered waits for the ones before it.
async fn first_found<F: Fetch>(fetch: &F, places: &[Place]) -> Option<Result<Found, String>> {
    let mut pending: FuturesUnordered<_> = places
        .iter()
        .enumerate()
        .map(|(index, place)| async move { (index, fetch.get(&place.url).await) })
        .collect();
    let mut readings: Vec<Option<Option<Reading>>> = places.iter().map(|_| None).collect();
    let mut next = 0;
    let mut refused = None;
    while let Some((index, answer)) = pending.next().await {
        readings[index] = Some(
            answer
                .ok()
                .filter(Fetched::ok)
                .and_then(|answer| read(&answer, &places[index].base)),
        );
        while next < places.len() {
            let Some(reading) = readings[next].as_mut() else {
                break;
            };
            next += 1;
            match reading.take() {
                Some(Reading::Found(found)) => return Some(Ok(found)),
                Some(Reading::Follow(url)) => {
                    if let Some(found) = follow(fetch, &url).await {
                        return Some(found);
                    }
                }
                Some(Reading::Refused(message)) => {
                    refused.get_or_insert(message);
                }
                Some(Reading::Mihon(store)) => return Some(mihon_store(fetch, &store).await),
                None => {}
            }
        }
    }
    refused.map(Err)
}

/// What the address an answer named is.
async fn follow<F: Fetch>(fetch: &F, url: &Url) -> Option<Result<Found, String>> {
    let answer = fetch.get(url).await.ok().filter(Fetched::ok)?;
    match read(&answer, url)? {
        Reading::Found(found) => Some(Ok(found)),
        Reading::Refused(message) => Some(Err(message)),
        Reading::Mihon(store) => Some(mihon_store(fetch, &store).await),
        Reading::Follow(_) => None,
    }
}

/// What one answer holds; a server found here is at `base`.
fn read(answer: &Fetched, base: &Url) -> Option<Reading> {
    let bytes = answer.body.as_slice();
    // A Mihon store: its extensions may be in the newer index it names (Keiyoushi's list only
    // says to update the app).
    if !answer.truncated && mihon::detect(bytes) {
        return Some(Reading::Mihon(Box::new(answer.clone())));
    }
    if bytes.starts_with(b"PK") {
        if answer.truncated {
            return None;
        }
        if let Ok(entry) = package::entry_of(bytes, answer.url.as_str()) {
            return Some(Reading::Found(Found::Extension {
                url: answer.url.clone(),
                entry: Box::new(entry),
                bytes: bytes.to_vec(),
            }));
        }
        return kodi::index_address(bytes, &answer.url).map(Reading::Follow);
    }
    let text = if bytes.starts_with(&[0x1f, 0x8b]) {
        kodi::unpack(bytes).ok()?
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };
    let text = text.trim_start_matches('\u{feff}').trim_start();
    match text.chars().next()? {
        '<' => read_xml(answer, text, base),
        '{' | '[' if answer.truncated => None,
        '{' | '[' => read_json(answer, text, base),
        _ if answer.truncated => None,
        _ => read_script(answer),
    }
}

/// The first element's name, lower case, without its namespace prefix.
fn root_element(text: &str) -> Option<String> {
    let mut reader = quick_xml::Reader::from_str(text);
    loop {
        match reader.read_event().ok()? {
            quick_xml::events::Event::Start(element) | quick_xml::events::Event::Empty(element) => {
                return Some(element.local_name().as_ref().to_ascii_lowercase());
            }
            quick_xml::events::Event::Eof => return None,
            _ => {}
        }
    }
}

/// The text of a feed's first `<title>`.
fn feed_title(text: &str) -> Option<String> {
    let mut reader = quick_xml::Reader::from_str(text);
    let mut inside = false;
    let mut title = String::new();
    loop {
        match reader.read_event().ok()? {
            quick_xml::events::Event::Start(element)
                if element.local_name().as_ref() == "title" =>
            {
                inside = true;
            }
            quick_xml::events::Event::Text(content) if inside => title.push_str(&content),
            quick_xml::events::Event::GeneralRef(reference) if inside => {
                title.push_str(
                    &quick_xml::escape::unescape(&format!("&{};", &*reference)).unwrap_or_default(),
                );
            }
            quick_xml::events::Event::End(_) if inside => {
                let title = title.trim();
                return (!title.is_empty()).then(|| title.to_owned());
            }
            quick_xml::events::Event::Eof => return None,
            _ => {}
        }
    }
}

/// A Subsonic server (Navidrome, Airsonic, gonic…) whose ping answered under `base`: at the
/// address before its API's `/rest/` part, so an address typed with `/rest/ping.view` (or
/// `/rest`) connects the server itself.
fn subsonic_at(base: &Url) -> Found {
    let mut url = base.clone();
    url.set_query(None);
    url.set_fragment(None);
    let path = url.path().to_owned();
    let lower = path.to_ascii_lowercase();
    let root = if let Some(at) = lower.find("/rest/") {
        &path[..=at]
    } else if lower.ends_with("/rest") {
        &path[..path.len() - "rest".len()]
    } else {
        &path
    };
    let root = if root.ends_with('/') {
        root.to_owned()
    } else {
        format!("{root}/")
    };
    url.set_path(&root);
    Found::Server {
        kind: ServerKind::Subsonic,
        url,
        name: None,
    }
}

fn read_xml(answer: &Fetched, text: &str, base: &Url) -> Option<Reading> {
    match root_element(text)?.as_str() {
        "addons" if answer.truncated => {
            // Too long to read here: Kodi's own indexes are read gzipped, as Kodi reads them.
            (answer.url.path().ends_with(".xml"))
                .then(|| Url::parse(&format!("{}.gz", answer.url)).ok())
                .flatten()
                .map(Reading::Follow)
        }
        "addons" => match repo::read_index(text) {
            Ok((index, format)) => Some(Reading::Found(Found::Repository {
                url: answer.url.clone(),
                format,
                index,
            })),
            Err(error) => Some(Reading::Refused(error.to_string())),
        },
        // A Kodi repository shared as its own add-on names its index.
        "addon" => kodi::index_address(text.as_bytes(), &answer.url).map(Reading::Follow),
        // Subsonic's ping in XML (servers that answer in XML whatever `f` asks for).
        "subsonic-response" => Some(Reading::Found(subsonic_at(base))),
        "feed" if is_opds(answer, text) => Some(Reading::Found(Found::Server {
            kind: ServerKind::Opds,
            url: answer.url.clone(),
            name: feed_title(text),
        })),
        _ => None,
    }
}

/// An Atom feed is an OPDS catalogue when it says so: its type, OPDS's namespace or its links'
/// profile.
fn is_opds(answer: &Fetched, text: &str) -> bool {
    answer
        .content_type
        .as_deref()
        .is_some_and(|kind| kind.contains("opds"))
        || text.contains("opds-spec.org")
        || text.contains("profile=opds-catalog")
}

fn read_json(answer: &Fetched, text: &str, base: &Url) -> Option<Reading> {
    let value: Value = serde_json::from_str(text).ok()?;
    let object = value.as_object();
    let has = |key: &str| object.is_some_and(|fields| fields.contains_key(key));
    // A Stremio manifest: an addon is added by its web address.
    if has("id") && has("resources") && matches!(answer.url.scheme(), "http" | "https") {
        return Some(match protocol::parse_manifest(&value) {
            Ok(manifest) => match protocol::normalize_addon_url(answer.url.as_str()) {
                Ok(url) => Reading::Found(Found::Addon {
                    url,
                    manifest: Box::new(manifest),
                }),
                Err(error) => Reading::Refused(error),
            },
            Err(error) => Reading::Refused(error),
        });
    }
    if let Some(found) = server_answer(&value, answer, base) {
        return Some(Reading::Found(found));
    }
    match repo::read_index(text) {
        Ok((index, format)) => Some(Reading::Found(Found::Repository {
            url: answer.url.clone(),
            format,
            index,
        })),
        Err(error @ (RepoError::Foreign(_) | RepoError::Format(_))) => {
            Some(Reading::Refused(error.to_string()))
        }
        Err(_) => None,
    }
}

/// A server's answer to one of [`SERVER_PROBES`], or an OPDS 2 catalogue.
fn server_answer(value: &Value, answer: &Fetched, base: &Url) -> Option<Found> {
    let object = value.as_object()?;
    let text = |key: &str| {
        object
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    };
    let server = |kind, name| {
        Some(Found::Server {
            kind,
            url: base.clone(),
            name,
        })
    };
    if object.contains_key("subsonic-response") {
        return Some(subsonic_at(base));
    }
    // Jellyfin's and Emby's `System/Info/Public`.
    if object.contains_key("Version")
        && (object.contains_key("ServerName") || object.contains_key("ProductName"))
        && object.contains_key("Id")
    {
        let emby =
            text("ProductName").is_some_and(|product| product.to_lowercase().contains("emby"));
        return server(
            if emby {
                ServerKind::Emby
            } else {
                ServerKind::Jellyfin
            },
            text("ServerName"),
        );
    }
    if text("app").is_some_and(|app| app.eq_ignore_ascii_case("audiobookshelf")) {
        return server(ServerKind::Audiobookshelf, None);
    }
    // OPDS 2: a catalogue in JSON, at its own address.
    let opds_type = answer
        .content_type
        .as_deref()
        .is_some_and(|kind| kind.contains("opds+json"));
    let metadata = object.get("metadata").and_then(Value::as_object);
    if metadata.is_some()
        && (opds_type
            || ["navigation", "publications", "groups"]
                .iter()
                .any(|key| object.contains_key(*key)))
    {
        return Some(Found::Server {
            kind: ServerKind::Opds,
            url: answer.url.clone(),
            name: metadata
                .and_then(|metadata| metadata.get("title"))
                .and_then(Value::as_str)
                .map(str::to_owned),
        });
    }
    None
}

/// One extension on its own: a Miru or Shosetsu script.
fn read_script(answer: &Fetched) -> Option<Reading> {
    let entry = miru::package::entry_of(&answer.body, &answer.url)
        .or_else(|| shosetsu::package::entry_of(&answer.body, &answer.url))?;
    Some(Reading::Found(Found::Extension {
        url: answer.url.clone(),
        entry: Box::new(entry),
        bytes: answer.body.clone(),
    }))
}

fn source_format(format: PackageFormat) -> SourceFormat {
    match format {
        PackageFormat::Raygun => SourceFormat::Raygun,
        PackageFormat::Nuclear => SourceFormat::Nuclear,
        PackageFormat::Shosetsu => SourceFormat::Shosetsu,
        PackageFormat::Miru => SourceFormat::Miru,
        PackageFormat::LnReader => SourceFormat::LnReader,
        PackageFormat::Kodi => SourceFormat::Kodi,
        PackageFormat::Mihon => SourceFormat::Mihon,
        PackageFormat::Stremio => SourceFormat::Stremio,
    }
}

// ---- Reading addresses for real ----

/// Raygun's HTTP client, and files on this device that Raygun may read
/// (`repo::local_files_allowed`).
pub struct HttpFetch {
    client: reqwest::Client,
}

impl HttpFetch {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

impl Fetch for HttpFetch {
    fn get(&self, url: &Url) -> impl Future<Output = Result<Fetched, String>> + Send {
        let client = self.client.clone();
        let url = url.clone();
        async move {
            if url.scheme() == "file" {
                return read_file(url).await;
            }
            let host = url.host_str().unwrap_or_default().to_owned();
            let mut response = client
                .get(url.clone())
                .timeout(TIMEOUT)
                .send()
                .await
                .map_err(|error| {
                    raygun_net::issue::message(
                        &raygun_net::issue::request_kind(&error).summary(&host),
                        &raygun_net::issue::request_detail(&host, &error),
                    )
                })?;
            let status = response.status().as_u16();
            let answered = response.url().clone();
            let content_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            let limit = usize::try_from(MAX_BYTES).unwrap_or(usize::MAX);
            let mut body = Vec::new();
            let mut truncated = false;
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|error| format!("{host}'s answer broke off: {error}"))?
            {
                let room = limit - body.len();
                if chunk.len() > room {
                    body.extend_from_slice(&chunk[..room]);
                    truncated = true;
                    break;
                }
                body.extend_from_slice(&chunk);
            }
            Ok(Fetched {
                url: answered,
                status,
                content_type,
                body,
                truncated,
            })
        }
    }
}

/// A file or a folder on this device.
async fn read_file(url: Url) -> Result<Fetched, String> {
    let path = url
        .to_file_path()
        .map_err(|()| format!("{url} isn't a usable path"))?;
    if !repo::local_files_allowed(&path) {
        return Err("Raygun can't read files there on this device".to_owned());
    }
    tokio::task::spawn_blocking(move || {
        use std::io::Read;
        let shown = path.display().to_string();
        let meta =
            std::fs::metadata(&path).map_err(|error| format!("Couldn't open {shown}: {error}"))?;
        if meta.is_dir() {
            return Ok(Fetched {
                url: Url::from_directory_path(&path)
                    .map_err(|()| format!("{shown} isn't a usable path"))?,
                status: 200,
                content_type: Some(FOLDER.to_owned()),
                body: Vec::new(),
                truncated: false,
            });
        }
        let mut body = Vec::new();
        std::fs::File::open(&path)
            .and_then(|file| file.take(MAX_BYTES + 1).read_to_end(&mut body))
            .map_err(|error| format!("Couldn't read {shown}: {error}"))?;
        let truncated = body.len() as u64 > MAX_BYTES;
        body.truncate(usize::try_from(MAX_BYTES).unwrap_or(usize::MAX));
        Ok(Fetched {
            url,
            status: 200,
            content_type: None,
            body,
            truncated,
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

// ---- Sources → Add ----

/// Looks at addresses before they're added, then adds them: addons to Stremio's list,
/// repositories to the extension repositories, an extension by installing it. A server needs its
/// sign-in, which `servers_add` takes.
pub struct SourceDetector {
    app: AppHandle,
    fetch: HttpFetch,
    /// What addresses looked at lately turned out to be, so Add needn't fetch them again.
    recent: Mutex<HashMap<String, (Instant, Found)>>,
}

impl SourceDetector {
    pub fn new(app: AppHandle, client: reqwest::Client) -> Self {
        Self {
            app,
            fetch: HttpFetch::new(client),
            recent: Mutex::new(HashMap::new()),
        }
    }

    fn extensions(&self) -> Arc<ExtensionService> {
        self.app.state::<Arc<ExtensionService>>().inner().clone()
    }

    fn stremio(&self) -> Arc<StremioService> {
        self.app.state::<Arc<StremioService>>().inner().clone()
    }

    /// What `address` is, as Sources shows it before it's added.
    pub async fn detect(&self, address: &str) -> Result<DetectedSource, String> {
        let found = self.look(address).await?;
        self.view(&found).await
    }

    async fn look(&self, address: &str) -> Result<Found, String> {
        // Android 17: an address on the home network needs local network access; the UI asks
        // first, so this only stops a look that would time out.
        if let Some(url) = addresses(address.trim())
            .ok()
            .and_then(|forms| forms.into_iter().next())
            .filter(|url| matches!(url.scheme(), "http" | "https"))
        {
            self.app
                .state::<LocalNetwork>()
                .check(url.as_str(), SOURCES_DENIED)
                .await?;
        }
        let found = detect(&self.fetch, address).await?;
        let mut recent = self.recent.lock().expect("recent looks poisoned");
        recent.retain(|_, (at, _)| at.elapsed() < REMEMBERED);
        recent.insert(address.trim().to_owned(), (Instant::now(), found.clone()));
        Ok(found)
    }

    /// Adds what `address` is, as it was found when looked at (or now): an addon, a repository,
    /// an extension. Answers with what it now is.
    pub async fn add(&self, address: &str) -> Result<DetectedSource, String> {
        let remembered = self
            .recent
            .lock()
            .expect("recent looks poisoned")
            .get(address.trim())
            .filter(|(at, _)| at.elapsed() < REMEMBERED)
            .map(|(_, found)| found.clone());
        let found = match remembered {
            Some(found) => found,
            None => self.look(address).await?,
        };
        match &found {
            Found::Addon { url, .. } => {
                self.stremio().add(url).await?;
            }
            Found::Repository { url, .. } => {
                self.extensions().add_repo(url.as_str()).await?;
            }
            Found::Extension { url, entry, bytes } => {
                self.extensions().install_file(url, entry, bytes).await?;
            }
            Found::Server { .. } => {
                return Err("Sign in to connect this server".to_owned());
            }
        }
        self.recent
            .lock()
            .expect("recent looks poisoned")
            .remove(address.trim());
        tracing::info!(address = %address.trim(), "source added");
        self.view(&found).await
    }

    /// What was found, as the UI shows it, marked against what this device has.
    async fn view(&self, found: &Found) -> Result<DetectedSource, String> {
        let base = |kind| DetectedSource {
            kind,
            format: None,
            server: None,
            url: String::new(),
            name: String::new(),
            description: None,
            added: false,
            addon: None,
            entries: Vec::new(),
            note: None,
        };
        Ok(match found {
            Found::Addon { url, manifest } => {
                let addon = self
                    .stremio()
                    .preview_manifest(url, manifest.as_ref().clone())
                    .await?;
                DetectedSource {
                    url: addon.url.clone(),
                    name: addon.name.clone(),
                    description: addon.description.clone(),
                    added: !addon.id.is_empty(),
                    addon: Some(addon),
                    ..base(DetectedKind::Addon)
                }
            }
            Found::Repository { url, format, index } => {
                let extensions = self.extensions();
                let listing = extensions.listing_of(url, index).await;
                let note = match format {
                    PackageFormat::Kodi => Some(kodi::UNSUPPORTED.to_owned()),
                    PackageFormat::Mihon if !mihon::runtime_present() => {
                        Some(mihon::UNSUPPORTED.to_owned())
                    }
                    _ => None,
                };
                DetectedSource {
                    format: Some(source_format(*format)),
                    added: extensions.has_repo(url.as_str()).await,
                    url: listing.repo.url,
                    name: listing.repo.name,
                    entries: listing.entries,
                    note,
                    ..base(DetectedKind::Repository)
                }
            }
            Found::Extension { url, entry, .. } => {
                let shown = self.extensions().entry_view(entry).await;
                DetectedSource {
                    format: Some(source_format(entry.format)),
                    url: url.to_string(),
                    name: entry.name.clone(),
                    description: entry.description.clone(),
                    added: shown.state == raygun_model::CatalogState::Installed,
                    entries: vec![shown],
                    ..base(DetectedKind::Extension)
                }
            }
            Found::Server { kind, url, name } => {
                let address = url.as_str().trim_end_matches('/');
                let added = self
                    .app
                    .state::<ServerService>()
                    .list()
                    .await
                    .unwrap_or_default()
                    .iter()
                    .any(|server| server.address.trim_end_matches('/') == address);
                DetectedSource {
                    server: Some(*kind),
                    url: address.to_owned(),
                    name: name
                        .clone()
                        .unwrap_or_else(|| url.host_str().unwrap_or(address).to_owned()),
                    added,
                    ..base(DetectedKind::Server)
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io::Write;
    use std::sync::Mutex;

    use serde_json::json;

    use super::*;

    /// Answers from a table; anything else is a 404, and hosts in `down` can't be reached. Every
    /// address asked is kept.
    #[derive(Default)]
    struct Table {
        answers: HashMap<String, (u16, Option<&'static str>, Vec<u8>)>,
        redirects: HashMap<String, String>,
        down: Vec<&'static str>,
        asked: Mutex<Vec<String>>,
    }

    impl Table {
        fn with(mut self, url: &str, body: impl Into<Vec<u8>>) -> Self {
            self.answers
                .insert(url.to_owned(), (200, None, body.into()));
            self
        }

        fn typed(mut self, url: &str, kind: &'static str, body: impl Into<Vec<u8>>) -> Self {
            self.answers
                .insert(url.to_owned(), (200, Some(kind), body.into()));
            self
        }

        fn moved(mut self, from: &str, to: &str) -> Self {
            self.redirects.insert(from.to_owned(), to.to_owned());
            self
        }

        fn asked(&self) -> Vec<String> {
            self.asked.lock().unwrap().clone()
        }
    }

    impl Fetch for Table {
        fn get(&self, url: &Url) -> impl Future<Output = Result<Fetched, String>> + Send {
            self.asked.lock().unwrap().push(url.to_string());
            let answer = if self.down.iter().any(|down| url.as_str().starts_with(down)) {
                Err(format!(
                    "Couldn't reach {}",
                    url.host_str().unwrap_or_default()
                ))
            } else {
                let target = self
                    .redirects
                    .get(url.as_str())
                    .cloned()
                    .unwrap_or_else(|| url.to_string());
                let (status, kind, body) = self.answers.get(&target).cloned().unwrap_or((
                    404,
                    None,
                    b"Not Found".to_vec(),
                ));
                Ok(Fetched {
                    url: Url::parse(&target).unwrap(),
                    status,
                    content_type: kind.map(str::to_owned),
                    body,
                    truncated: false,
                })
            };
            async move { answer }
        }
    }

    fn manifest() -> String {
        json!({
            "id": "org.example.films", "version": "1.2.0", "name": "Open Films",
            "resources": ["catalog", "meta", "stream"], "types": ["movie"],
            "catalogs": [{ "type": "movie", "id": "top", "name": "Top" }],
        })
        .to_string()
    }

    fn raygun_index() -> String {
        json!({ "formatVersion": 1, "name": "Example sources", "extensions": [] }).to_string()
    }

    fn package_zip() -> Vec<u8> {
        let manifest = json!({
            "id": "dev.example.pkg", "name": "Pkg", "version": "1.0.0", "apiVersion": 1,
            "runtime": "js", "entry": "dist/index.js", "provides": ["music.metadata"],
            "permissions": { "net": ["api.example.org"] },
        })
        .to_string();
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        for (name, body) in [
            ("raygun-extension.json", manifest.as_bytes()),
            ("dist/index.js", b"export default {}".as_slice()),
        ] {
            writer.start_file(name, options).unwrap();
            writer.write_all(body).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    async fn look(table: &Table, typed: &str) -> Found {
        detect(table, typed)
            .await
            .unwrap_or_else(|error| panic!("{typed}: {error}"))
    }

    fn repository(found: &Found) -> (&Url, PackageFormat, &RepoIndex) {
        match found {
            Found::Repository { url, format, index } => (url, *format, index),
            other => panic!("not a repository: {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_stremio_manifest_is_an_addon() {
        let table = Table::default().with("https://films.example/manifest.json", manifest());
        let Found::Addon {
            url,
            manifest: read,
        } = look(&table, "https://films.example/manifest.json").await
        else {
            panic!("not an addon")
        };
        assert_eq!(url, "https://films.example/manifest.json");
        assert_eq!(read.name, "Open Films");
        // Stremio's own links, and a bare host whose page isn't the manifest.
        let landing = Table::default()
            .with(
                "https://films.example/",
                "<!DOCTYPE html><html><body>Open Films</body></html>",
            )
            .with("https://films.example/manifest.json", manifest());
        assert!(matches!(
            look(&landing, "stremio://films.example/manifest.json").await,
            Found::Addon { .. }
        ));
        assert!(matches!(
            look(&landing, "films.example").await,
            Found::Addon { url, .. } if url == "https://films.example/manifest.json"
        ));
    }

    #[tokio::test]
    async fn every_repository_format_is_read() {
        let mihon = json!([{ "name": "Tachiyomi: X", "pkg": "eu.kanade.tachiyomi.extension.en.x",
            "apk": "tachiyomi-en.x-v1.4.1.apk", "lang": "en", "code": 1, "version": "1.4.1",
            "nsfw": 0, "sources": [{ "name": "X", "lang": "en", "id": "1", "baseUrl": "https://x.example" }] }]);
        let shosetsu = json!({ "libraries": [], "scripts": [{ "name": "NovelFull",
            "fileName": "NovelFull", "lang": "en", "ver": "1.0.0", "libVer": "1.0.0", "md5": "" }] });
        let nuclear = json!({ "version": 1, "plugins": [{ "id": "nuclear-musicbrainz",
            "name": "MusicBrainz", "category": "metadata", "version": "1.0.0",
            "downloadUrl": "https://example.org/mb.zip" }] });
        let miru = json!([{ "name": "M", "package": "m.example", "type": "manga", "version": "v1.0.0",
            "webSite": "https://m.example", "url": "m.example.js" }]);
        let lnreader = json!([{ "id": "royalroad", "name": "Royal Road", "site": "https://www.royalroad.com",
            "lang": "English", "version": "2.0.0", "url": "https://p.example/royalroad.js",
            "iconUrl": "https://p.example/royalroad.png" }]);
        let kodi = r#"<?xml version="1.0" encoding="UTF-8"?>
<addons><addon id="plugin.video.example" name="Example" version="1.0.0" provider-name="X">
<extension point="xbmc.python.pluginsource" library="main.py"><provides>video</provides></extension>
</addon></addons>"#;
        let table = Table::default()
            .with("https://r.example/index.json", raygun_index())
            .with("https://m.example/index.min.json", mihon.to_string())
            .with("https://s.example/index.json", shosetsu.to_string())
            .with("https://n.example/plugins.json", nuclear.to_string())
            .with("https://miru.example/index.json", miru.to_string())
            .with("https://ln.example/plugins.min.json", lnreader.to_string())
            .with("https://k.example/addons.xml", kodi);
        for (address, format, count) in [
            ("https://r.example/index.json", PackageFormat::Raygun, 0),
            ("https://m.example/index.min.json", PackageFormat::Mihon, 1),
            ("https://s.example/index.json", PackageFormat::Shosetsu, 1),
            ("https://n.example/plugins.json", PackageFormat::Nuclear, 1),
            ("https://miru.example/index.json", PackageFormat::Miru, 1),
            (
                "https://ln.example/plugins.min.json",
                PackageFormat::LnReader,
                1,
            ),
            ("https://k.example/addons.xml", PackageFormat::Kodi, 1),
        ] {
            let found = look(&table, address).await;
            let (url, read, index) = repository(&found);
            assert_eq!((url.as_str(), read), (address, format));
            assert_eq!(index.extensions.len(), count, "{address}");
        }
        // A folder's address stands for the index in it.
        let folder = Table::default().with("https://r.example/repo/index.json", raygun_index());
        let found = look(&folder, "https://r.example/repo").await;
        assert_eq!(
            repository(&found).0.as_str(),
            "https://r.example/repo/index.json"
        );
    }

    #[tokio::test]
    async fn one_extension_on_its_own_is_an_extension() {
        let miru = "// ==MiruExtension==\n// @name RoyalRoad\n// @version v0.0.3\n// @package royalroad.com\n// @type fikushon\n// @webSite https://www.royalroad.com\n// ==/MiruExtension==\nexport default class extends Extension {}\n";
        let shosetsu =
            "-- {\"id\":7,\"ver\":\"1.2.0\",\"libVer\":\"1.0.0\"}\nreturn { name = \"Example\" }\n";
        let table = Table::default()
            .with("https://x.example/pkg.zip", package_zip())
            .with("https://x.example/royalroad.com.js", miru)
            .with("https://x.example/src/en/Example.lua", shosetsu);
        for (address, id, format) in [
            (
                "https://x.example/pkg.zip",
                "dev.example.pkg",
                PackageFormat::Raygun,
            ),
            (
                "https://x.example/royalroad.com.js",
                "miru.royalroad.com",
                PackageFormat::Miru,
            ),
            (
                "https://x.example/src/en/Example.lua",
                "shosetsu.en.example",
                PackageFormat::Shosetsu,
            ),
        ] {
            let Found::Extension { url, entry, bytes } = look(&table, address).await else {
                panic!("{address}: not an extension");
            };
            assert_eq!(url.as_str(), address);
            assert_eq!((entry.id.as_str(), entry.format), (id, format));
            assert_eq!(entry.sha256, package::sha256_hex(&bytes));
        }
    }

    #[tokio::test]
    async fn subsonic_servers_are_found_however_their_address_is_typed() {
        let ping = json!({ "subsonic-response": { "status": "failed", "version": "1.16.1",
            "error": { "code": 10, "message": "Required parameter is missing" } } })
        .to_string();
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?><subsonic-response xmlns="http://subsonic.org/restapi" status="failed" version="1.15.0"><error code="10" message="Required parameter is missing"/></subsonic-response>"#;
        let table = Table::default()
            .with("https://music.example/", "<html>Navidrome</html>")
            .with(
                "https://music.example/rest/ping.view?v=1.16.1&c=Raygun&f=json",
                ping.clone(),
            )
            .with("https://music.example/rest/ping.view", xml)
            .with("https://home.example/navidrome/", "<html>Navidrome</html>")
            .with(
                "https://home.example/navidrome/rest/ping.view?v=1.16.1&c=Raygun&f=json",
                ping,
            );
        for (typed, server) in [
            ("music.example", "https://music.example/"),
            // The API's own address, answered in XML, stands for the server.
            (
                "https://music.example/rest/ping.view",
                "https://music.example/",
            ),
            // Served under a path.
            (
                "https://home.example/navidrome",
                "https://home.example/navidrome/",
            ),
        ] {
            let Found::Server { kind, url, .. } = look(&table, typed).await else {
                panic!("{typed}: not a server");
            };
            assert_eq!(
                (kind, url.as_str()),
                (ServerKind::Subsonic, server),
                "{typed}"
            );
        }
    }

    #[tokio::test]
    async fn servers_are_found_by_their_endpoints() {
        let table = Table::default()
            .with("https://media.example/", "<!DOCTYPE html><html>Jellyfin</html>")
            .with(
                "https://media.example/System/Info/Public",
                json!({ "ServerName": "Home Media", "Version": "10.10.0", "ProductName": "Jellyfin Server", "Id": "a1" })
                    .to_string(),
            );
        let Found::Server { kind, url, name } = look(&table, "media.example").await else {
            panic!("not a server");
        };
        assert_eq!(kind, ServerKind::Jellyfin);
        assert_eq!(url.as_str(), "https://media.example/");
        assert_eq!(name.as_deref(), Some("Home Media"));

        // On a home network without a certificate: http after https didn't connect.
        let subsonic = Table {
            down: vec!["https://192.168.1.5"],
            ..Table::default()
        }
        .with("http://192.168.1.5:4533/", "<html>Navidrome</html>")
        .with(
            "http://192.168.1.5:4533/rest/ping.view?v=1.16.1&c=Raygun&f=json",
            json!({ "subsonic-response": { "status": "failed", "version": "1.16.1", "type": "navidrome",
                "error": { "code": 10, "message": "Required parameter is missing" } } })
                .to_string(),
        );
        assert!(matches!(
            look(&subsonic, "192.168.1.5:4533").await,
            Found::Server { kind: ServerKind::Subsonic, url, .. } if url.as_str() == "http://192.168.1.5:4533/"
        ));

        let books = Table::default()
            .with("https://books.example/", "<html>Audiobookshelf</html>")
            .with(
                "https://books.example/status",
                json!({ "app": "audiobookshelf", "serverVersion": "2.17.0", "isInit": true })
                    .to_string(),
            );
        assert!(matches!(
            look(&books, "https://books.example").await,
            Found::Server {
                kind: ServerKind::Audiobookshelf,
                ..
            }
        ));

        let feed = r#"<?xml version="1.0"?><feed xmlns="http://www.w3.org/2005/Atom" xmlns:opds="http://opds-spec.org/2010/catalog"><title>Komga</title><link rel="start" type="application/atom+xml;profile=opds-catalog;kind=navigation" href="/opds/v1.2/catalog"/></feed>"#;
        let komga = Table::default()
            .with("https://comics.example/", "<html>Komga</html>")
            .typed(
                "https://comics.example/opds/v1.2/catalog",
                "application/atom+xml;profile=opds-catalog",
                feed,
            );
        let Found::Server { kind, url, name } = look(&komga, "https://comics.example").await else {
            panic!("not a catalogue");
        };
        assert_eq!(kind, ServerKind::Opds);
        assert_eq!(url.as_str(), "https://comics.example/opds/v1.2/catalog");
        assert_eq!(name.as_deref(), Some("Komga"));
    }

    /// Keiyoushi's list only says to update the app; its `repo.json` names the store that lists
    /// the extensions, which Raygun reads instead, as adding it does.
    #[tokio::test]
    async fn a_mihon_list_is_followed_to_its_newer_index() {
        let placeholder = json!([{ "name": "Update to Mihon 0.20.1+", "pkg": "eu.kanade.tachiyomi.extension.all.mihon",
            "apk": "tachiyomi-all.mihon-v1.4.1.apk", "lang": "all", "code": 1, "version": "1.4.1", "nsfw": 0,
            "sources": [{ "name": "Update", "lang": "all", "id": "1", "baseUrl": "https://mihon.app" }] }]);
        let repo = json!({ "index_v2": "https://store.example/index.json",
            "meta": { "name": "Store", "website": "https://store.example", "signingKeyFingerprint": "ab" } });
        let store = json!({ "name": "Store", "badgeLabel": "S", "signingKey": "ab",
        "contact": { "website": "https://store.example" },
        "extensionList": { "extensions": [
            { "name": "One", "packageName": "eu.kanade.tachiyomi.extension.en.one",
              "resources": { "apkUrl": "https://store.example/one.apk", "iconUrl": "https://store.example/one.png" },
              "extensionLib": "1.6", "versionCode": 106_001, "versionName": "1.6.1", "contentWarning": 1,
              "sources": [{ "id": 11, "name": "One", "language": "en", "homeUrl": "https://one.example" }] },
            { "name": "Two", "packageName": "eu.kanade.tachiyomi.extension.en.two",
              "resources": { "apkUrl": "https://store.example/two.apk", "iconUrl": "https://store.example/two.png" },
              "extensionLib": "1.6", "versionCode": 106_002, "versionName": "1.6.2", "contentWarning": 3,
              "sources": [{ "id": 12, "name": "Two", "language": "en", "homeUrl": "https://two.example" }] }
        ]}});
        let table = Table::default()
            .with(
                "https://store.example/repo/index.min.json",
                placeholder.to_string(),
            )
            .with("https://store.example/repo/repo.json", repo.to_string())
            .with("https://store.example/index.json", store.to_string());
        let found = look(&table, "https://store.example/repo/index.min.json").await;
        let (url, format, index) = repository(&found);
        assert_eq!(format, PackageFormat::Mihon);
        assert_eq!(
            url.as_str(),
            "https://store.example/repo/index.min.json",
            "the address added"
        );
        assert_eq!(index.name, "Store");
        let ids: Vec<&str> = index
            .extensions
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        assert_eq!(ids, ["mihon.en.one", "mihon.en.two"]);
        assert_eq!(index.extensions[1].content_rating, "nsfw");
    }

    #[tokio::test]
    async fn github_pages_stand_for_what_the_project_publishes() {
        let mihon = json!([{ "name": "Tachiyomi: X", "pkg": "eu.kanade.tachiyomi.extension.en.x",
            "apk": "x.apk", "lang": "en", "version": "1.4.1", "nsfw": 0, "sources": [] }]);
        let table = Table::default()
            .with(
                "https://raw.githubusercontent.com/keiyoushi/extensions/repo/index.min.json",
                mihon.to_string(),
            )
            // Another index on its default branch doesn't come first.
            .with(
                "https://raw.githubusercontent.com/keiyoushi/extensions/HEAD/index.json",
                raygun_index(),
            );
        let found = look(&table, "https://github.com/keiyoushi/extensions").await;
        let (url, format, _) = repository(&found);
        assert_eq!(format, PackageFormat::Mihon);
        assert_eq!(
            url.as_str(),
            "https://raw.githubusercontent.com/keiyoushi/extensions/repo/index.min.json"
        );
        assert!(
            !table
                .asked()
                .iter()
                .any(|asked| asked.starts_with("https://github.com/")),
            "GitHub's page itself isn't read"
        );

        let raygun = Table::default().with(
            "https://raw.githubusercontent.com/someone/sources/HEAD/index.json",
            raygun_index(),
        );
        let found = look(&raygun, "https://github.com/someone/sources.git").await;
        assert_eq!(repository(&found).1, PackageFormat::Raygun);

        let addon = Table::default().with(
            "https://raw.githubusercontent.com/someone/addon/HEAD/manifest.json",
            manifest(),
        );
        assert!(matches!(
            look(&addon, "https://github.com/someone/addon").await,
            Found::Addon { .. }
        ));

        let folder = Table::default().with(
            "https://raw.githubusercontent.com/someone/repo/dev/novels/index.json",
            raygun_index(),
        );
        assert!(matches!(
            look(&folder, "https://github.com/someone/repo/tree/dev/novels").await,
            Found::Repository { .. }
        ));

        let script = Table::default().with(
            "https://raw.githubusercontent.com/someone/repo/main/src/en/Example.lua",
            "-- {\"id\":7,\"ver\":\"1.2.0\"}\nreturn {}\n",
        );
        assert!(matches!(
            look(
                &script,
                "https://github.com/someone/repo/blob/main/src/en/Example.lua"
            )
            .await,
            Found::Extension { .. }
        ));

        let empty = Table::default();
        let error = detect(&empty, "https://github.com/someone/nothing")
            .await
            .unwrap_err();
        assert!(error.contains("someone/nothing"), "{error}");
        // A release download is a file, read as it is.
        let release = "https://github.com/someone/repo/releases/download/v1/pkg.zip";
        let download = Table::default()
            .moved(release, "https://objects.example/pkg.zip")
            .with("https://objects.example/pkg.zip", package_zip());
        assert!(matches!(
            look(&download, release).await,
            Found::Extension { .. }
        ));
    }

    #[tokio::test]
    async fn what_cant_be_added_says_why() {
        let nothing =
            Table::default().with("https://blog.example/", "<html><body>Hello</body></html>");
        let error = detect(&nothing, "https://blog.example/").await.unwrap_err();
        assert!(
            error.starts_with(
                "Raygun found no repository, addon, server or extension at blog.example"
            ),
            "{error}"
        );

        let cloudstream = Table::default().with(
            "https://cs.example/repo.json",
            json!({ "name": "Repo", "manifestVersion": 1, "pluginLists": [] }).to_string(),
        );
        let error = detect(&cloudstream, "https://cs.example/repo.json")
            .await
            .unwrap_err();
        assert!(error.contains("CloudStream"), "{error}");

        let plain = Table::default().with("http://r.example/index.json", raygun_index());
        let error = detect(&plain, "http://r.example/index.json")
            .await
            .unwrap_err();
        assert!(error.contains("https"), "{error}");

        let down = Table {
            down: vec!["https://gone.example"],
            ..Table::default()
        };
        assert!(
            detect(&down, "https://gone.example/index.json")
                .await
                .unwrap_err()
                .contains("gone.example")
        );
        assert_eq!(
            detect(&down, "  ").await.unwrap_err(),
            "Type or paste an address"
        );
        assert!(detect(&down, "not an address").await.is_err());
    }

    #[cfg(not(target_os = "android"))]
    #[tokio::test]
    async fn a_folder_on_this_computer_is_its_repository() {
        let temp = tempfile::TempDir::new().unwrap();
        let folder = temp.path().join("my-sources");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("index.json"), raygun_index()).unwrap();
        std::fs::write(folder.join("pkg.zip"), package_zip()).unwrap();
        let fetch = HttpFetch::new(raygun_net::http_client().unwrap());
        let found = detect(&fetch, &folder.display().to_string()).await.unwrap();
        let (url, format, index) = repository(&found);
        assert_eq!(format, PackageFormat::Raygun);
        assert_eq!(index.name, "Example sources");
        assert_eq!(url.to_file_path().unwrap(), folder.join("index.json"));
        let package = detect(&fetch, &folder.join("pkg.zip").display().to_string())
            .await
            .unwrap();
        assert!(matches!(package, Found::Extension { entry, .. } if entry.id == "dev.example.pkg"));
        let empty = temp.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        let error = detect(&fetch, &empty.display().to_string())
            .await
            .unwrap_err();
        assert!(error.contains("holds no repository index"), "{error}");
    }
}
