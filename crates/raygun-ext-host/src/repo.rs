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
    /// Lower-case hex SHA-256 of the package.
    pub sha256: String,
    pub size: u64,
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
