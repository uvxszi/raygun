//! Mihon extension stores, read the way Mihon reads them (mihon `ExtensionStoreService`,
//! `NetworkExtensionStore`, `NetworkLegacyExtension`, Apache-2.0):
//!
//! - a legacy list, `index.min.json`: `[{ name, pkg, apk, lang, code, version, nsfw, sources:
//!   [{ id, lang, name, baseUrl }] }]`, with `apk/<apk>` and `icon/<pkg>.png` beside it, and the
//!   repository's `repo.json` (`{ meta: { name, shortName, website, signingKeyFingerprint },
//!   index_v2 }`) beside that;
//! - a store (`index_v2`, often gzipped protobuf, `index.pb`): `name` 1, `badgeLabel` 2,
//!   `signingKey` 3, `contact` 4, and the extensions inline (101) or at an address (102). Each
//!   extension: `name` 1, `packageName` 2, `resources` 3 (`apkUrl` 1, `iconUrl` 2), `extensionLib`
//!   4, `versionCode` 5, `versionName` 6, `contentWarning` 7 and `sources` 8 (`id` 1, `name` 2,
//!   `language` 3, `homeUrl` 4). The same store may come as JSON with those names.
//!
//! [`load`] follows a list to its repository's newer index, as Mihon does (Keiyoushi's list only
//! says to update the app since its index moved to `index.pb`). [`to_repo_index`] lists a store's
//! extensions as Raygun catalogue entries of format `mihon`: comic sources whose source code
//! Raygun converts when it installs one (`crate::mihon::convert`).

use std::collections::BTreeSet;
use std::future::Future;
use std::io::Read as _;

use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::proto::{self, Message};
use crate::manga::MANGA_SOURCE;
use crate::manifest::{API_VERSION, is_extension_id};
use crate::repo::{INDEX_FORMAT, PackageFormat, RepoEntry, RepoIndex, RepoPermissions};

/// What a store is called when it names itself nowhere.
pub const STORE_NAME: &str = "Mihon extensions";
/// The prefix of a Mihon extension's package name; what follows is `<lang>.<name>`.
pub const PACKAGE_PREFIX: &str = "eu.kanade.tachiyomi.extension.";
/// Largest store read once unpacked (Keiyoushi's is under 1 MB).
const MAX_STORE_BYTES: u64 = 32 * 1024 * 1024;
/// Hops from the address added to the extensions (list → repo.json → index → its list).
const MAX_HOPS: usize = 4;

/// How a store rates an extension's content.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContentWarning {
    #[default]
    Safe,
    /// Some of it is for adults.
    Mixed,
    Nsfw,
}

/// One source of an extension (an extension often has one per language).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreSource {
    /// Mihon's id for the source: a 63-bit number, as text.
    pub id: String,
    pub name: String,
    pub lang: String,
    pub base_url: String,
}

/// One extension of a store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreExtension {
    pub name: String,
    /// Its Android package name, `eu.kanade.tachiyomi.extension.<lang>.<name>`.
    pub pkg: String,
    /// Where its APK is (absolute).
    pub apk: String,
    /// Its icon (absolute), when the store gives one.
    pub icon: Option<String>,
    /// The extensions-lib version it was built against (`1.4`, `1.6`).
    pub lib_version: String,
    pub version_code: i64,
    pub version_name: String,
    pub content: ContentWarning,
    pub sources: Vec<StoreSource>,
}

/// A store: who publishes it and what it lists.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Store {
    pub name: String,
    /// The SHA-256 of the certificate the store's APKs are signed with, as hex.
    pub signing_key: Option<String>,
    pub website: Option<String>,
    pub extensions: Vec<StoreExtension>,
}

/// What a file read from a store's address turned out to be.
#[derive(Debug, Clone, PartialEq)]
pub enum Read {
    /// The store and its extensions.
    Store(Store),
    /// The extensions are at another address; the store (without them) names itself here.
    Follow { url: Url, store: Store },
    /// A legacy list: its repository's `repo.json` may name a newer index.
    List {
        extensions: Vec<StoreExtension>,
        repo: Url,
    },
    /// A legacy `repo.json`, naming the store; its list (`index.min.json`) is beside it unless it
    /// names a newer index.
    Repo {
        store: Store,
        newer: Option<Url>,
        list: Url,
    },
}

/// Whether `bytes` look like a Mihon store, list or `repo.json` (gzipped or not), for telling
/// repository formats apart.
pub fn detect(bytes: &[u8]) -> bool {
    let Ok(bytes) = unpack(bytes) else {
        return false;
    };
    match first_byte(&bytes) {
        Some(b'[') => serde_json::from_slice::<Value>(&bytes).is_ok_and(|value| is_index(&value)),
        Some(b'{') => serde_json::from_slice::<Value>(&bytes).is_ok_and(|value| {
            value
                .get("meta")
                .is_some_and(|meta| meta.get("signingKeyFingerprint").is_some())
                || (value.get("signingKey").is_some()
                    && (value.get("extensionList").is_some()
                        || value.get("extensionListUrl").is_some()))
        }),
        _ => Message::parse(&bytes).is_some_and(|message| {
            message.text(1).is_some()
                && message.text(3).is_some()
                && (message.get(101).is_some() || message.get(102).is_some())
        }),
    }
}

/// Reads one file of a store at `from`.
pub fn read(bytes: &[u8], from: &Url) -> Result<Read, String> {
    let bytes = unpack(bytes)?;
    match first_byte(&bytes) {
        Some(b'[') => {
            let items: Vec<Value> = serde_json::from_slice(&bytes)
                .map_err(|error| format!("the extension list isn't readable JSON: {error}"))?;
            let folder = from
                .join("./")
                .map_err(|error| format!("invalid store address: {error}"))?;
            let extensions = items
                .iter()
                .filter_map(|item| legacy_extension(item, Some(&folder)))
                .collect();
            let repo = folder
                .join("repo.json")
                .map_err(|error| format!("invalid store address: {error}"))?;
            Ok(Read::List { extensions, repo })
        }
        Some(b'{') => {
            let value: Value = serde_json::from_slice(&bytes)
                .map_err(|error| format!("the store isn't readable JSON: {error}"))?;
            if let Some(meta) = value.get("meta") {
                let store = Store {
                    name: text(meta, "name").unwrap_or_else(|| STORE_NAME.to_owned()),
                    signing_key: text(meta, "signingKeyFingerprint"),
                    website: text(meta, "website"),
                    extensions: Vec::new(),
                };
                let newer = text(&value, "index_v2")
                    .map(|address| join(from, &address))
                    .transpose()?;
                let list = join(from, "index.min.json")?;
                return Ok(Read::Repo { store, newer, list });
            }
            json_store(&value, from)
        }
        _ => {
            let message =
                Message::parse(&bytes).ok_or("it's neither JSON nor a Mihon store (protobuf)")?;
            proto_store(&message, from)
        }
    }
}

/// Reads a store from `first` (the file at `url`) to its extensions, asking `fetch` for the other
/// files it names. Returns the address the extensions came from and the store.
pub async fn load<F, Fut>(url: &Url, first: Vec<u8>, fetch: F) -> Result<(Url, Store), String>
where
    F: Fn(Url) -> Fut,
    Fut: Future<Output = Result<Vec<u8>, String>>,
{
    let mut at = url.clone();
    let mut bytes = first;
    // A store named by `repo.json` before its extensions were read.
    let mut named: Option<Store> = None;
    // A legacy list waiting on its `repo.json`.
    let mut pending: Option<(Url, Vec<StoreExtension>)> = None;
    for _ in 0..=MAX_HOPS {
        match read(&bytes, &at)? {
            Read::Store(mut store) => {
                if let Some(named) = named.take() {
                    store.signing_key = store.signing_key.or(named.signing_key);
                    store.website = store.website.or(named.website);
                    if store.name.is_empty() {
                        store.name = named.name;
                    }
                }
                return Ok((at, store));
            }
            Read::Follow { url: next, store } => {
                // A `repo.json` read earlier keeps its signing key when the index has none.
                named = Some(match named.take() {
                    Some(earlier) => Store {
                        signing_key: store.signing_key.or(earlier.signing_key),
                        website: store.website.or(earlier.website),
                        name: if store.name.is_empty() {
                            earlier.name
                        } else {
                            store.name
                        },
                        extensions: Vec::new(),
                    },
                    None => store,
                });
                bytes = fetch(next.clone()).await?;
                at = next;
            }
            Read::List { extensions, repo } => {
                if let Some(store) = named.take() {
                    return Ok((
                        at,
                        Store {
                            extensions,
                            ..store
                        },
                    ));
                }
                match fetch(repo.clone()).await {
                    Ok(found) => {
                        pending = Some((at.clone(), extensions));
                        bytes = found;
                        at = repo;
                    }
                    // A list without `repo.json` is a store of its own.
                    Err(_) => {
                        return Ok((
                            at.clone(),
                            Store {
                                name: name_from(&at),
                                extensions,
                                ..Store::default()
                            },
                        ));
                    }
                }
            }
            Read::Repo { store, newer, list } => {
                if let Some(newer) = newer {
                    named = Some(store);
                    bytes = fetch(newer.clone()).await?;
                    at = newer;
                } else if let Some((list_url, extensions)) = pending.take() {
                    return Ok((
                        list_url,
                        Store {
                            extensions,
                            ..store
                        },
                    ));
                } else {
                    named = Some(store);
                    bytes = fetch(list.clone()).await?;
                    at = list;
                }
            }
        }
    }
    Err("the store names one address after another without listing extensions".into())
}

/// The prefix of an Aniyomi anime extension's package name.
pub const ANIME_PREFIX: &str = "eu.kanade.tachiyomi.animeextension.";

/// The Raygun extension id of a Mihon package: `mihon.` and what follows the usual prefix
/// (`eu.kanade.tachiyomi.extension.all.ahottie` → `mihon.all.ahottie`); Aniyomi's under
/// `mihon.anime.`.
pub fn extension_id(pkg: &str) -> Option<String> {
    let pkg = pkg.trim();
    let (prefix, rest) = match (
        pkg.strip_prefix(ANIME_PREFIX),
        pkg.strip_prefix(PACKAGE_PREFIX),
    ) {
        (Some(rest), _) => ("mihon.anime", rest.to_owned()),
        (None, Some(rest)) => ("mihon", rest.to_owned()),
        // Another prefix: its language and name are its last two parts.
        (None, None) => {
            let parts: Vec<&str> = pkg.split('.').collect();
            ("mihon", parts[parts.len().saturating_sub(2)..].join("."))
        }
    };
    let labels: Vec<String> = rest
        .to_ascii_lowercase()
        .split('.')
        .map(|part| {
            let label: String = part
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                .collect();
            label.trim_matches('-').to_owned()
        })
        .filter(|label| !label.is_empty())
        .collect();
    if labels.is_empty() {
        return None;
    }
    let id = format!("{prefix}.{}", labels.join("."));
    is_extension_id(&id).then_some(id)
}

/// Whether `value` is shaped like a legacy Mihon list: entries naming a package and an APK.
pub fn is_index(value: &Value) -> bool {
    value.as_array().is_some_and(|entries| {
        entries
            .iter()
            .any(|entry| entry.get("pkg").is_some() && entry.get("apk").is_some())
    })
}

/// Reads a legacy list on its own (`repo::read_index`, which has no address): its APKs and icons
/// stay relative to it (`apk/…`, `icon/…`). `None` when `json` isn't one. Reading a store by its
/// address ([`load`]) also follows it to its newer index.
pub fn parse(json: &str) -> Option<RepoIndex> {
    let value: Value = serde_json::from_str(json).ok()?;
    if !is_index(&value) {
        return None;
    }
    let extensions = value
        .as_array()?
        .iter()
        .filter_map(|item| legacy_extension(item, None))
        .collect();
    to_repo_index(&Store {
        extensions,
        ..Store::default()
    })
    .ok()
}

/// The extension of `store` an entry stands for.
pub fn find<'a>(store: &'a Store, extension_id: &str) -> Option<&'a StoreExtension> {
    store
        .extensions
        .iter()
        .find(|extension| self::extension_id(&extension.pkg).as_deref() == Some(extension_id))
}

/// Whether a package is an Aniyomi anime extension, which isn't a comic source.
fn is_anime(pkg: &str) -> bool {
    pkg.contains(".animeextension.")
}

/// A semver version for an extension: its version name (`1.4.190`, the lib version and its
/// code), else one made from its code, so updates go by the code as Mihon's do.
pub fn semver_version(extension: &StoreExtension) -> String {
    match semver::Version::parse(&extension.version_name) {
        Ok(version) if version.pre.is_empty() && version.build.is_empty() => version.to_string(),
        _ => format!("0.0.{}", extension.version_code.max(0)),
    }
}

/// The hosts an extension's sources are on, as `net` patterns: each host and its subdomains (and
/// the domain without `www.`).
pub fn source_hosts(extension: &StoreExtension) -> Vec<String> {
    let mut hosts = BTreeSet::new();
    for source in &extension.sources {
        let Some(host) = Url::parse(source.base_url.trim())
            .ok()
            .filter(|url| matches!(url.scheme(), "http" | "https"))
            .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
        else {
            continue;
        };
        let bare = host.strip_prefix("www.").unwrap_or(&host).to_owned();
        if crate::permissions::HostPattern::parse(&bare).is_none() {
            continue;
        }
        hosts.insert(bare.clone());
        if bare.contains('.') {
            hosts.insert(format!("*.{bare}"));
        }
    }
    hosts.into_iter().collect()
}

/// The language Raygun lists for an extension: its sources' one language, else `multi`.
fn language(extension: &StoreExtension) -> Option<String> {
    let languages: BTreeSet<&str> = extension
        .sources
        .iter()
        .map(|source| source.lang.as_str())
        .filter(|lang| !lang.is_empty())
        .collect();
    match languages.len() {
        0 => None,
        1 => languages.first().map(|lang| match *lang {
            "all" => "multi".to_owned(),
            other => other.to_owned(),
        }),
        _ => Some("multi".to_owned()),
    }
}

/// A store's extensions as a Raygun index of `mihon` entries; anime extensions are left out.
pub fn to_repo_index(store: &Store) -> Result<RepoIndex, String> {
    let comics: Vec<&StoreExtension> = store
        .extensions
        .iter()
        .filter(|extension| !is_anime(&extension.pkg))
        .collect();
    if comics.is_empty() && !store.extensions.is_empty() {
        return Err(
            "This is an Aniyomi repository: its anime extensions can't be converted".into(),
        );
    }
    let mut seen = BTreeSet::new();
    let extensions = comics
        .into_iter()
        .filter_map(|extension| {
            let id = extension_id(&extension.pkg)?;
            if !seen.insert(id.clone()) {
                return None;
            }
            let name = display_name(&extension.name);
            Some(RepoEntry {
                id,
                description: Some(format!(
                    "{name}, a Mihon comic source. Raygun converts its source code to run it."
                )),
                name,
                version: semver_version(extension),
                api_version: API_VERSION,
                author: Some(store.name.clone()).filter(|name| !name.is_empty()),
                provides: vec![MANGA_SOURCE.to_owned()],
                permissions: RepoPermissions {
                    net: source_hosts(extension),
                    webview: false,
                },
                content_rating: match extension.content {
                    ContentWarning::Safe => "safe",
                    ContentWarning::Mixed => "suggestive",
                    ContentWarning::Nsfw => "nsfw",
                }
                .to_owned(),
                lang: language(extension),
                package: extension.apk.clone(),
                icon: extension.icon.clone(),
                sha256: String::new(),
                size: 0,
                format: PackageFormat::Mihon,
                sites: Vec::new(),
            })
        })
        .collect();
    Ok(RepoIndex {
        format_version: INDEX_FORMAT,
        name: store.name.clone(),
        extensions,
        libraries: Vec::new(),
    })
}

/// An extension's name without the `Tachiyomi: ` legacy lists put first.
fn display_name(name: &str) -> String {
    let name = name.trim();
    let name = name.strip_prefix("Tachiyomi: ").unwrap_or(name);
    name.chars().take(80).collect()
}

/// A store's text: gunzipped when it's gzipped.
fn unpack(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if !bytes.starts_with(&[0x1f, 0x8b]) {
        return Ok(bytes.to_vec());
    }
    let mut unpacked = Vec::new();
    flate2::read::GzDecoder::new(bytes)
        .take(MAX_STORE_BYTES + 1)
        .read_to_end(&mut unpacked)
        .map_err(|error| format!("the store isn't a readable gzip file: {error}"))?;
    if unpacked.len() as u64 > MAX_STORE_BYTES {
        return Err("the store is too large".into());
    }
    Ok(unpacked)
}

fn first_byte(bytes: &[u8]) -> Option<u8> {
    let text = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    text.iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
}

fn join(base: &Url, address: &str) -> Result<Url, String> {
    let url = base
        .join(address.trim())
        .map_err(|error| format!("the store names an invalid address: {error}"))?;
    if matches!(url.scheme(), "http" | "https")
        || (base.scheme() == "file" && url.scheme() == "file")
    {
        Ok(url)
    } else {
        Err(format!("the store names {url}, which isn't a web address"))
    }
}

/// A name for a store from its address (`raw.githubusercontent.com/<owner>/…` → the owner).
fn name_from(url: &Url) -> String {
    let name = crate::repo::default_name(url);
    if name.is_empty() {
        STORE_NAME.to_owned()
    } else {
        name
    }
}

fn text(value: &Value, key: &str) -> Option<String> {
    let text = match value.get(key)? {
        Value::String(text) => text.trim().to_owned(),
        Value::Number(number) => number.to_string(),
        _ => return None,
    };
    (!text.is_empty()).then_some(text)
}

fn integer(value: &Value, key: &str) -> Option<i64> {
    match value.get(key)? {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

/// An extension of a legacy list, its files beside the list (relative to it without `folder`).
fn legacy_extension(item: &Value, folder: Option<&Url>) -> Option<StoreExtension> {
    let pkg = text(item, "pkg")?;
    let apk = text(item, "apk")?;
    if apk.contains(['/', '\\']) || pkg.contains(['/', '\\']) {
        return None;
    }
    let beside = |path: String| match folder {
        Some(folder) => folder.join(&path).ok().map(String::from),
        None => Some(path),
    };
    let name = text(item, "name").unwrap_or_else(|| pkg.clone());
    let lang = text(item, "lang").unwrap_or_else(|| "all".to_owned());
    let version_name = text(item, "version").unwrap_or_default();
    let mut sources: Vec<StoreSource> = item
        .get("sources")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|source| {
            Some(StoreSource {
                id: text(source, "id")?,
                name: text(source, "name").unwrap_or_else(|| display_name(&name)),
                lang: text(source, "lang").unwrap_or_else(|| lang.clone()),
                base_url: text(source, "baseUrl").unwrap_or_default(),
            })
        })
        .collect();
    if sources.is_empty() {
        sources.push(StoreSource {
            id: "0".to_owned(),
            name: display_name(&name),
            lang: lang.clone(),
            base_url: String::new(),
        });
    }
    Some(StoreExtension {
        lib_version: version_name
            .rsplit_once('.')
            .map(|(lib, _)| lib.to_owned())
            .unwrap_or_default(),
        apk: beside(format!("apk/{apk}"))?,
        icon: beside(format!("icon/{pkg}.png")),
        version_code: integer(item, "code").unwrap_or_default(),
        content: if integer(item, "nsfw") == Some(1) {
            ContentWarning::Nsfw
        } else {
            ContentWarning::Safe
        },
        name,
        pkg,
        version_name,
        sources,
    })
}

fn content_warning(value: Option<&Value>) -> ContentWarning {
    match value {
        Some(Value::Number(number)) => match number.as_u64() {
            Some(2) => ContentWarning::Mixed,
            Some(3) => ContentWarning::Nsfw,
            _ => ContentWarning::Safe,
        },
        Some(Value::String(name)) => match name.trim_start_matches("CONTENT_WARNING_") {
            "MIXED" => ContentWarning::Mixed,
            "NSFW" => ContentWarning::Nsfw,
            _ => ContentWarning::Safe,
        },
        _ => ContentWarning::Safe,
    }
}

/// A store as JSON (the protobuf store's fields by name).
fn json_store(value: &Value, from: &Url) -> Result<Read, String> {
    let list = value.get("extensionList");
    let extensions_value = list
        .and_then(|list| list.get("extensions"))
        .or_else(|| value.get("extensions"));
    let named = || Store {
        name: text(value, "name").unwrap_or_default(),
        signing_key: text(value, "signingKey"),
        website: value
            .get("contact")
            .and_then(|contact| text(contact, "website")),
        extensions: Vec::new(),
    };
    let Some(items) = extensions_value.and_then(Value::as_array) else {
        if let Some(address) = text(value, "extensionListUrl") {
            return Ok(Read::Follow {
                url: join(from, &address)?,
                store: named(),
            });
        }
        return Err("the store lists no extensions".into());
    };
    let extensions = items
        .iter()
        .filter_map(|item| {
            let resources = item.get("resources");
            let sources = item
                .get("sources")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|source| {
                    Some(StoreSource {
                        id: text(source, "id")?,
                        name: text(source, "name").unwrap_or_default(),
                        lang: text(source, "language")
                            .or_else(|| text(source, "lang"))
                            .unwrap_or_default(),
                        base_url: text(source, "homeUrl")
                            .or_else(|| text(source, "baseUrl"))
                            .unwrap_or_default(),
                    })
                })
                .collect();
            Some(StoreExtension {
                name: text(item, "name")?,
                pkg: text(item, "packageName").or_else(|| text(item, "pkg"))?,
                apk: join(from, &text(resources?, "apkUrl")?).ok()?.to_string(),
                icon: resources
                    .and_then(|resources| text(resources, "iconUrl"))
                    .and_then(|icon| join(from, &icon).ok())
                    .map(String::from),
                lib_version: text(item, "extensionLib").unwrap_or_default(),
                version_code: integer(item, "versionCode").unwrap_or_default(),
                version_name: text(item, "versionName").unwrap_or_default(),
                content: content_warning(item.get("contentWarning")),
                sources,
            })
        })
        .collect();
    // A bare extension list (an `extensionListUrl`'s answer) names no store.
    Ok(Read::Store(Store {
        extensions,
        ..named()
    }))
}

/// A protobuf store, or a bare protobuf extension list.
fn proto_store(message: &Message<'_>, from: &Url) -> Result<Read, String> {
    // A store names itself (1) and its signing key (3); a bare list only repeats extensions (1).
    let is_store =
        message.text(3).is_some() || message.get(101).is_some() || message.get(102).is_some();
    let named = || Store {
        name: if is_store {
            message.text(1).unwrap_or_default()
        } else {
            String::new()
        },
        signing_key: is_store.then(|| message.text(3)).flatten(),
        website: is_store
            .then(|| message.message(4).and_then(|contact| contact.text(1)))
            .flatten(),
        extensions: Vec::new(),
    };
    let list = if is_store {
        match message.message(101) {
            Some(list) => list,
            None => {
                let address = message.text(102).ok_or("the store lists no extensions")?;
                return Ok(Read::Follow {
                    url: join(from, &address)?,
                    store: named(),
                });
            }
        }
    } else {
        message.clone()
    };
    let extensions = list
        .all(1)
        .filter_map(proto::Value::message)
        .filter_map(|extension| {
            let resources = extension.message(3);
            let sources = extension
                .all(8)
                .filter_map(proto::Value::message)
                .filter_map(|source| {
                    Some(StoreSource {
                        // Ids are positive (Mihon masks off the sign bit).
                        id: source.number(1)?.to_string(),
                        name: source.text(2).unwrap_or_default(),
                        lang: source.text(3).unwrap_or_default(),
                        base_url: source.text(4).unwrap_or_default(),
                    })
                })
                .collect();
            Some(StoreExtension {
                name: extension.text(1)?,
                pkg: extension.text(2)?,
                apk: join(from, &resources.as_ref()?.text(1)?).ok()?.to_string(),
                icon: resources
                    .as_ref()
                    .and_then(|resources| resources.text(2))
                    .and_then(|icon| join(from, &icon).ok())
                    .map(String::from),
                lib_version: extension.text(4).unwrap_or_default(),
                version_code: extension
                    .number(5)
                    .and_then(|code| i64::try_from(code).ok())
                    .unwrap_or_default(),
                version_name: extension.text(6).unwrap_or_default(),
                content: match extension.number(7) {
                    Some(2) => ContentWarning::Mixed,
                    Some(3) => ContentWarning::Nsfw,
                    _ => ContentWarning::Safe,
                },
                sources,
            })
        })
        .collect();
    Ok(Read::Store(Store {
        extensions,
        ..named()
    }))
}
