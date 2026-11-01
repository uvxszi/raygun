//! Extension repositories: an `index.json` listing packages (ARCHITECTURE §7.1, M13).

use std::cmp::Ordering;
use std::net::IpAddr;

use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::manifest::{API_VERSION, is_extension_id};

pub const INDEX_FORMAT: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoIndex {
    pub format_version: u32,
    pub name: String,
    pub extensions: Vec<RepoEntry>,
    /// The shared Lua libraries of a Shosetsu repository, which scripts `Require` (M32).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub libraries: Vec<RepoLibrary>,
}

/// A Shosetsu library: `lib/<name>.lua`, with its SHA-256.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoLibrary {
    pub name: String,
    #[serde(default)]
    pub ver: String,
    pub hash: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoPermissions {
    #[serde(default)]
    pub net: Vec<String>,
    #[serde(default)]
    pub webview: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub api_version: u32,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub provides: Vec<String>,
    #[serde(default)]
    pub permissions: RepoPermissions,
    #[serde(default = "safe")]
    pub content_rating: String,
    /// The package zip, relative to the index (or absolute).
    pub package: String,
    /// Lower-case hex SHA-256 of the package; may be empty for Nuclear plugins.
    pub sha256: String,
    /// Package size in bytes; 0 when the repository doesn't say (Nuclear's registry).
    pub size: u64,
    #[serde(default)]
    pub format: PackageFormat,
}

/// What a package is: a Raygun extension, a Nuclear plugin converted at install (M17), or a
/// Shosetsu Lua script installed with its libraries (M32).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PackageFormat {
    #[default]
    Raygun,
    Nuclear,
    Shosetsu,
}

/// Shosetsu's repository index (`index.json` with `scripts` and `libraries`).
#[derive(Debug, Deserialize)]
struct ShosetsuIndex {
    scripts: Vec<ShosetsuScript>,
    #[serde(default)]
    libraries: Vec<RepoLibrary>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ShosetsuScript {
    name: String,
    file_name: String,
    lang: String,
    ver: String,
    #[serde(default)]
    lib_ver: String,
    /// Despite the name, a SHA-256.
    md5: String,
    #[serde(rename = "type", default)]
    kind: Option<String>,
}

/// The kotlin-lib major version Raygun's Shosetsu runtime stands in for.
const SHOSETSU_LIB_MAJOR: u64 = 1;

/// A label for an extension id: lower case, other characters as `-`.
fn id_label(text: &str) -> String {
    let label: String = text
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    label.trim_matches('-').to_owned()
}

fn from_shosetsu(index: ShosetsuIndex) -> RepoIndex {
    let extensions = index
        .scripts
        .into_iter()
        .filter_map(|script| {
            let lua = script
                .kind
                .as_deref()
                .is_none_or(|kind| kind == "LuaScript");
            let lib_ok = semver::Version::parse(&script.lib_ver)
                .is_ok_and(|version| version.major == SHOSETSU_LIB_MAJOR);
            let hash_ok =
                script.md5.len() == 64 && script.md5.bytes().all(|b| b.is_ascii_hexdigit());
            let id = format!(
                "shosetsu.{}.{}",
                id_label(&script.lang),
                id_label(&script.file_name)
            );
            let usable = lua
                && lib_ok
                && hash_ok
                && semver::Version::parse(&script.ver).is_ok()
                && is_extension_id(&id)
                && !script.file_name.contains(['/', '\\'])
                && !script.lang.contains(['/', '\\']);
            if !usable {
                tracing::warn!(script = %script.name, "skipping unusable Shosetsu script");
                return None;
            }
            Some(RepoEntry {
                id,
                description: Some(format!(
                    "{} ({}), a Shosetsu novel source. It may reach any website.",
                    script.name, script.lang
                )),
                name: script.name,
                version: script.ver,
                api_version: API_VERSION,
                author: None,
                provides: vec!["novel.source".to_owned()],
                permissions: RepoPermissions {
                    net: vec!["*".to_owned()],
                    webview: false,
                },
                content_rating: "suggestive".to_owned(),
                package: format!("src/{}/{}.lua", script.lang, script.file_name),
                sha256: script.md5.to_ascii_lowercase(),
                size: 0,
                format: PackageFormat::Shosetsu,
            })
        })
        .collect();
    RepoIndex {
        format_version: INDEX_FORMAT,
        name: "Shosetsu extensions".to_owned(),
        extensions,
        libraries: index.libraries,
    }
}

/// Nuclear's plugin registry (`plugins.json`), read as a repository.
#[derive(Debug, Deserialize)]
struct NuclearRegistry {
    version: u32,
    plugins: Vec<NuclearRegistryEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NuclearRegistryEntry {
    id: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    categories: Option<Vec<String>>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    download_url: Option<String>,
    #[serde(default)]
    sha256: Option<String>,
}

/// The Raygun id a converted Nuclear plugin gets; must match packages/nuclear-compat's `slug`.
pub fn nuclear_extension_id(package: &str) -> String {
    let lower = package.to_lowercase();
    let mut slug = String::new();
    for character in lower.trim_start_matches('@').chars() {
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            slug.push(character);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug: String = slug.trim_matches('-').chars().take(80).collect();
    let slug = slug.trim_end_matches('-');
    format!("nuclear.{}", if slug.is_empty() { "plugin" } else { slug })
}

/// Nuclear categories Raygun runs, as Raygun provider kinds.
fn nuclear_kind(category: &str) -> Option<&'static str> {
    match category {
        "metadata" => Some("music.metadata"),
        "streaming" => Some("music.stream"),
        "lyrics" => Some("lyrics"),
        _ => None,
    }
}

fn from_nuclear(registry: NuclearRegistry) -> RepoIndex {
    let extensions = registry
        .plugins
        .into_iter()
        .filter_map(|plugin| {
            let categories = plugin
                .categories
                .or_else(|| plugin.category.map(|category| vec![category]))
                .unwrap_or_default();
            let provides: Vec<String> = categories
                .iter()
                .filter_map(|category| nuclear_kind(category).map(str::to_owned))
                .collect();
            // Nothing Raygun can run, or nothing to download.
            if provides.is_empty() {
                return None;
            }
            let version = plugin
                .version
                .filter(|version| semver::Version::parse(version).is_ok())
                .unwrap_or_else(|| "0.0.0".to_owned());
            Some(RepoEntry {
                id: nuclear_extension_id(&plugin.id),
                name: plugin.name,
                version,
                api_version: API_VERSION,
                description: plugin.description,
                author: plugin.author,
                provides,
                permissions: RepoPermissions::default(),
                content_rating: safe(),
                package: plugin.download_url?,
                sha256: plugin
                    .sha256
                    .filter(|hash| hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
                    .unwrap_or_default()
                    .to_ascii_lowercase(),
                size: 0,
                format: PackageFormat::Nuclear,
            })
        })
        .collect();
    let _ = registry.version;
    RepoIndex {
        format_version: INDEX_FORMAT,
        name: "Nuclear plugins".to_owned(),
        extensions,
        libraries: Vec::new(),
    }
}

fn safe() -> String {
    "safe".to_owned()
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum RepoError {
    #[error("invalid repository URL: {0}")]
    Url(String),
    #[error("not a Raygun repository index: {0}")]
    Index(String),
    #[error("repository format {0} is not supported (this Raygun reads format {INDEX_FORMAT})")]
    Format(u32),
}

/// Repository URLs must be `https`, except plain `http` to this machine or the Android emulator's
/// host alias (`10.0.2.2`), which serve local test repositories.
pub fn check_index_url(url: &str) -> Result<Url, RepoError> {
    let parsed = Url::parse(url.trim()).map_err(|error| RepoError::Url(error.to_string()))?;
    let local = match parsed.host() {
        Some(url::Host::Domain(domain)) => domain == "localhost",
        Some(url::Host::Ipv4(ip)) => IpAddr::V4(ip).is_loopback() || ip.octets() == [10, 0, 2, 2],
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    };
    match parsed.scheme() {
        "https" => Ok(parsed),
        "http" if local => Ok(parsed),
        "http" => Err(RepoError::Url(
            "use https (plain http is only allowed for local test repositories)".to_owned(),
        )),
        other => Err(RepoError::Url(format!("unsupported scheme {other:?}"))),
    }
}

/// Parses an index, dropping entries this Raygun can't use (other API versions, malformed
/// fields) rather than rejecting the whole repository.
pub fn parse_index(json: &str) -> Result<RepoIndex, RepoError> {
    if let Ok(registry) = serde_json::from_str::<NuclearRegistry>(json) {
        return Ok(from_nuclear(registry));
    }
    if let Ok(index) = serde_json::from_str::<ShosetsuIndex>(json) {
        return Ok(from_shosetsu(index));
    }
    let mut index: RepoIndex =
        serde_json::from_str(json).map_err(|error| RepoError::Index(error.to_string()))?;
    if index.format_version != INDEX_FORMAT {
        return Err(RepoError::Format(index.format_version));
    }
    index.name = index.name.trim().to_owned();
    index.extensions.retain(|entry| {
        let usable = is_extension_id(&entry.id)
            && entry.api_version == API_VERSION
            && semver::Version::parse(&entry.version).is_ok()
            && entry.sha256.len() == 64
            && entry.sha256.bytes().all(|b| b.is_ascii_hexdigit());
        if !usable {
            tracing::warn!(extension = %entry.id, "skipping unusable repository entry");
        }
        usable
    });
    Ok(index)
}

/// Where an entry's package is, resolved against the index URL; http(s) only.
pub fn package_url(index_url: &Url, entry: &RepoEntry) -> Result<Url, RepoError> {
    let url = index_url
        .join(&entry.package)
        .map_err(|error| RepoError::Url(error.to_string()))?;
    if matches!(url.scheme(), "http" | "https") {
        Ok(url)
    } else {
        Err(RepoError::Url(format!("package URL {url} is not http(s)")))
    }
}

/// Semver ordering of two version strings; unparsable versions sort first.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    match (semver::Version::parse(a), semver::Version::parse(b)) {
        (Ok(a), Ok(b)) => a.cmp(&b),
        (Ok(_), Err(_)) => Ordering::Greater,
        (Err(_), Ok(_)) => Ordering::Less,
        (Err(_), Err(_)) => Ordering::Equal,
    }
}
