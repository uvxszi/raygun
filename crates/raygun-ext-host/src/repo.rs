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

/// A Shosetsu library: `lib/<name>.lua`, with its SHA-256 (which community repositories often
/// leave out).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoLibrary {
    pub name: String,
    #[serde(default)]
    pub ver: String,
    #[serde(default, deserialize_with = "text_or_null")]
    pub hash: String,
}

/// A string field that may be `null`.
fn text_or_null<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
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
    /// The content's language (`en`, `ja`, `multi`), when the extension serves one (M36).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// The package zip, relative to the index (or absolute).
    pub package: String,
    /// The extension's icon, relative to the index (or absolute) (M46.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Lower-case hex SHA-256 of the package; may be empty for Nuclear plugins.
    pub sha256: String,
    /// Package size in bytes; 0 when the repository doesn't say (Nuclear's registry).
    pub size: u64,
    #[serde(default)]
    pub format: PackageFormat,
}

/// What a package is: a Raygun extension, a Nuclear plugin converted at install (M17), a
/// Shosetsu Lua script installed with its libraries (M32), a Miru script (`crate::miru`), an
/// LNReader plugin (`crate::lnreader`), a Kodi add-on, listed but not installable
/// (`crate::kodi`), or a Mihon extension whose source code is converted at install
/// (`crate::mihon`; its package is the APK, which Raygun doesn't run).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PackageFormat {
    #[default]
    Raygun,
    Nuclear,
    Shosetsu,
    Miru,
    #[serde(rename = "lnreader")]
    LnReader,
    Kodi,
    Mihon,
}

/// Shosetsu's repository index (`index.json` with `scripts` and `libraries`). Entries are read one
/// by one, so one malformed script or library doesn't hide the rest of the repository.
#[derive(Debug, Deserialize)]
struct ShosetsuIndex {
    scripts: Vec<serde_json::Value>,
    #[serde(default)]
    libraries: Vec<serde_json::Value>,
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
    /// Despite the name, a SHA-256 in Shosetsu's own repository; community repositories leave it
    /// empty or put an MD5 there. Shosetsu only compares it to spot updates.
    #[serde(default, deserialize_with = "text_or_null")]
    md5: String,
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(rename = "imageURL", default)]
    image_url: Option<String>,
}

/// The kotlin-lib major version Raygun's Shosetsu runtime stands in for.
const SHOSETSU_LIB_MAJOR: u64 = 1;

/// Where the index may be for an address that isn't one: Shosetsu repositories are shared by
/// their folder (Shosetsu adds `index.json` itself), or as their page on GitHub or GitLab, whose
/// raw files sit elsewhere. Tried in order; an address ending in `.json` has none.
pub fn shosetsu_index_candidates(url: &Url) -> Vec<Url> {
    if url.path().ends_with(".json") {
        return Vec::new();
    }
    let parts: Vec<&str> = url
        .path_segments()
        .map(|segments| segments.filter(|part| !part.is_empty()).collect())
        .unwrap_or_default();
    let raw = |text: String| Url::parse(&text).ok();
    let mut candidates = Vec::new();
    match url.host_str() {
        // github.com/<owner>/<repo>[/tree/<branch>/<folder…>]
        Some("github.com" | "www.github.com") if parts.len() >= 2 => {
            let (owner, repository) = (parts[0], parts[1]);
            let (branches, folder): (Vec<&str>, String) = match parts.get(2..) {
                Some(["tree" | "blob", branch, rest @ ..]) => (vec![*branch], rest.join("/")),
                _ => (vec!["main", "master", "dev"], String::new()),
            };
            for branch in branches {
                let folder = if folder.is_empty() {
                    String::new()
                } else {
                    format!("{folder}/")
                };
                candidates.extend(raw(format!(
                    "https://raw.githubusercontent.com/{owner}/{repository}/{branch}/{folder}index.json"
                )));
            }
        }
        // gitlab.com/<group…>/<project>[/-/tree/<branch>/<folder…>]
        Some("gitlab.com") if parts.len() >= 2 && !parts.contains(&"raw") => {
            let split = parts.iter().position(|part| *part == "-");
            let project = parts[..split.unwrap_or(parts.len())].join("/");
            let (branches, folder): (Vec<&str>, String) =
                match split.and_then(|at| parts.get(at + 1..)) {
                    Some(["tree" | "blob", branch, rest @ ..]) => (vec![*branch], rest.join("/")),
                    _ => (vec!["main", "master", "dev"], String::new()),
                };
            for branch in branches {
                let folder = if folder.is_empty() {
                    String::new()
                } else {
                    format!("{folder}/")
                };
                candidates.extend(raw(format!(
                    "https://gitlab.com/{project}/-/raw/{branch}/{folder}index.json"
                )));
            }
        }
        _ => {
            let mut folder = url.clone();
            if !folder.path().ends_with('/') {
                folder.set_path(&format!("{}/", folder.path()));
            }
            candidates.extend(folder.join("index.json").ok());
        }
    }
    candidates
}

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
            let script: ShosetsuScript = serde_json::from_value(script)
                .inspect_err(|error| tracing::warn!(%error, "skipping a malformed Shosetsu script"))
                .ok()?;
            let lua = script
                .kind
                .as_deref()
                .is_none_or(|kind| kind == "LuaScript");
            let lib_ok = semver::Version::parse(&script.lib_ver)
                .is_ok_and(|version| version.major == SHOSETSU_LIB_MAJOR);
            // Only a SHA-256 can be checked; anything else (none, an MD5) is checked by what the
            // file is when it's installed (`shosetsu::package::verify_script`).
            let sha256 =
                if script.md5.len() == 64 && script.md5.bytes().all(|b| b.is_ascii_hexdigit()) {
                    script.md5.to_ascii_lowercase()
                } else {
                    String::new()
                };
            let id = format!(
                "shosetsu.{}.{}",
                id_label(&script.lang),
                id_label(&script.file_name)
            );
            let usable = lua
                && lib_ok
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
                lang: Some(script.lang.clone()),
                package: format!("src/{}/{}.lua", script.lang, script.file_name),
                icon: script.image_url.filter(|url| !url.trim().is_empty()),
                sha256,
                size: 0,
                format: PackageFormat::Shosetsu,
            })
        })
        .collect();
    RepoIndex {
        format_version: INDEX_FORMAT,
        // A Shosetsu index doesn't name itself: the app names it after its address
        // (`default_name`), so only Shosetsu's own repository reads as Shosetsu's.
        name: String::new(),
        extensions,
        libraries: index
            .libraries
            .into_iter()
            .filter_map(|library| serde_json::from_value::<RepoLibrary>(library).ok())
            .filter(|library| crate::shosetsu::package::is_library_name(&library.name))
            .collect(),
    }
}

/// The name of the official Shosetsu repository.
pub const SHOSETSU_REPO_NAME: &str = "Shosetsu extensions";

/// A name for a repository whose index gives none (Shosetsu's): Shosetsu's own repository by its
/// name; one a hosting service serves by its project's name (`raw.githubusercontent.com/<owner>/
/// <project>/…`, jsDelivr's `gh/<owner>/<project>@<ref>/…`, `gitlab.com/<group>/<project>/-/raw/…`,
/// `<owner>.github.io/<project>/…`); anything else by its host, and a local index by its folder.
pub fn default_name(url: &Url) -> String {
    let segments: Vec<&str> = url
        .path_segments()
        .map(|segments| segments.filter(|part| !part.is_empty()).collect())
        .unwrap_or_default();
    if url.scheme() == "file" {
        return url
            .to_file_path()
            .ok()
            .and_then(|path| Some(path.parent()?.file_name()?.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "Local repository".to_owned());
    }
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let part = |index: usize| segments.get(index).map(|part| (*part).to_owned());
    let (owner, project) = match host.as_str() {
        "raw.githubusercontent.com" | "github.com" | "codeberg.org" => (part(0), part(1)),
        // Groups nest; the project is the last part before GitLab's `-`.
        "gitlab.com" => match segments.iter().position(|part| *part == "-") {
            Some(end) if end >= 2 => (part(0), part(end - 1)),
            _ => (part(0), None),
        },
        "cdn.jsdelivr.net" if segments.first() == Some(&"gh") => (
            part(1),
            part(2).map(|project| project.split('@').next().unwrap_or_default().to_owned()),
        ),
        _ => match ["github.io", "gitlab.io"]
            .iter()
            .find_map(|pages| host.strip_suffix(pages)?.strip_suffix('.'))
        {
            // A project's pages; a user's own site (`<owner>.github.io/index.json`) has none.
            Some(owner) => (
                Some(owner.to_owned()),
                part(0).filter(|_| segments.len() > 1),
            ),
            None => (None, None),
        },
    };
    if owner.is_some_and(|owner| owner.eq_ignore_ascii_case("shosetsuorg")) {
        return SHOSETSU_REPO_NAME.to_owned();
    }
    match (project.filter(|project| !project.is_empty()), url.port()) {
        (Some(project), _) => project,
        (None, Some(port)) => format!("{host}:{port}"),
        (None, None) => host,
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
                lang: None,
                package: plugin.download_url?,
                icon: None,
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
    /// Another app's repository (M46.38): named, so the user knows why it can't be added.
    #[error("{0}")]
    Foreign(String),
}

/// Repository URLs must be `https`, except plain `http` to this machine or the Android emulator's
/// host alias (`10.0.2.2`), which serve local test repositories.
pub fn check_index_url(url: &str) -> Result<Url, RepoError> {
    // On desktop a repository can be an index on this computer (M46.28): a plain path or a
    // `file:` address, its packages read from beside it.
    let trimmed = url.trim();
    if cfg!(not(target_os = "android")) && std::path::Path::new(trimmed).is_absolute() {
        return Url::from_file_path(trimmed)
            .map_err(|()| RepoError::Url(format!("{trimmed} isn't a usable path")));
    }
    let parsed = Url::parse(trimmed).map_err(|error| RepoError::Url(error.to_string()))?;
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
        "file" if cfg!(not(target_os = "android")) => Ok(parsed),
        other => Err(RepoError::Url(format!("unsupported scheme {other:?}"))),
    }
}

/// The index in a repository's folder, for an address that names the folder rather than a file
/// (`https://miru-repo.0n0.dev`, as Miru takes repositories, means its `index.json`).
pub fn index_beside(url: &Url) -> Option<Url> {
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let path = url.path();
    if path
        .rsplit('/')
        .next()
        .is_some_and(|last| last.contains('.'))
    {
        return None;
    }
    let mut folder = url.clone();
    folder.set_query(None);
    folder.set_fragment(None);
    if !path.ends_with('/') {
        folder.set_path(&format!("{path}/"));
    }
    folder.join("index.json").ok()
}

/// Parses an index, dropping entries this Raygun can't use (other API versions, malformed
/// fields) rather than rejecting the whole repository.
///
/// A Raygun index may list packages of every format, so one repository can hold Raygun
/// extensions, Nuclear plugins and Shosetsu scripts (as Paraline does): a `shosetsu` entry's
/// package is its script, and the libraries scripts need are the index's `libraries`, found at
/// `lib/<name>.lua` beside it, as in a Shosetsu repository.
pub fn parse_index(json: &str) -> Result<RepoIndex, RepoError> {
    // Kodi's `addons.xml` (`crate::kodi`), the one index that isn't JSON.
    if json
        .trim_start_matches('\u{feff}')
        .trim_start()
        .starts_with('<')
    {
        return crate::kodi::parse(json).ok_or_else(|| {
            RepoError::Index("it's XML, but not a Kodi add-on index (addons.xml)".into())
        });
    }
    if let Ok(registry) = serde_json::from_str::<NuclearRegistry>(json) {
        return Ok(from_nuclear(registry));
    }
    if let Ok(index) = serde_json::from_str::<ShosetsuIndex>(json) {
        return Ok(from_shosetsu(index));
    }
    if let Some(index) = crate::miru::index::parse(json) {
        return Ok(index);
    }
    if let Some(index) = crate::lnreader::index::parse(json) {
        return Ok(index);
    }
    let mut index: RepoIndex = serde_json::from_str(json).map_err(|error| {
        foreign_format(json).map_or_else(|| RepoError::Index(error.to_string()), RepoError::Foreign)
    })?;
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
    // A script installs as a novel source that may reach any public website, whatever the index
    // says; the catalogue shows that.
    for entry in &mut index.extensions {
        if entry.format == PackageFormat::Shosetsu {
            entry.provides = vec!["novel.source".to_owned()];
            entry.permissions = RepoPermissions {
                net: vec!["*".to_owned()],
                webview: false,
            };
        }
    }
    Ok(index)
}

/// What another app's repository is, by the shape of its index (M46.38), with what Raygun can do
/// about it; `None` for anything else.
fn foreign_format(json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let first = value.as_array().and_then(|items| items.first());
    let has = |object: Option<&serde_json::Value>, keys: &[&str]| {
        object
            .and_then(serde_json::Value::as_object)
            .is_some_and(|fields| keys.iter().all(|key| fields.contains_key(*key)))
    };
    let message = if has(first, &["pkg", "apk", "sources"]) {
        "This is a Mihon or Aniyomi extension repository: its extensions are Android packages, which Raygun can't run"
    } else if has(Some(&value), &["pluginLists", "manifestVersion"]) {
        "This is a CloudStream repository: its plugins are Kotlin code, which Raygun can't run"
    } else if has(first, &["internalName", "tvTypes", "url"]) {
        "This is a CloudStream plugin list: its plugins are Kotlin code, which Raygun can't run"
    } else if has(Some(&value), &["resources", "types", "id"]) {
        "This is a Stremio addon, not a repository: add it in Browse → Sources → Addons"
    } else {
        return None;
    };
    Some(message.to_owned())
}

/// Where an entry's icon is, resolved against the index URL; http(s) only (M46.2).
pub fn icon_url(index_url: &Url, entry: &RepoEntry) -> Option<Url> {
    let url = index_url.join(entry.icon.as_deref()?).ok()?;
    matches!(url.scheme(), "http" | "https").then_some(url)
}

/// Where an entry's package is, resolved against the index URL: http(s), or a file when the
/// index is one on this computer (desktop, M46.28). A web index never reaches local files.
pub fn package_url(index_url: &Url, entry: &RepoEntry) -> Result<Url, RepoError> {
    let url = index_url
        .join(&entry.package)
        .map_err(|error| RepoError::Url(error.to_string()))?;
    let local =
        cfg!(not(target_os = "android")) && index_url.scheme() == "file" && url.scheme() == "file";
    if matches!(url.scheme(), "http" | "https") || local {
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
