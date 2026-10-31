//! `raygun-extension.json` (ARCHITECTURE §7.1).

use std::collections::{BTreeMap, HashSet};

use serde::Deserialize;

use crate::permissions::{HostPattern, NetPolicy};

pub const MANIFEST_FILE: &str = "raygun-extension.json";
pub const API_VERSION: u32 = 1;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawManifest {
    id: String,
    name: String,
    version: String,
    api_version: u32,
    runtime: String,
    entry: String,
    #[serde(default)]
    provides: Vec<String>,
    #[serde(default)]
    permissions: RawPermissions,
    #[serde(default)]
    content_rating: ContentRating,
    #[serde(default)]
    settings: Vec<SettingDef>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    author: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawPermissions {
    #[serde(default)]
    net: Vec<String>,
    #[serde(default)]
    webview: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContentRating {
    #[default]
    Safe,
    Suggestive,
    Nsfw,
}

/// A setting the extension declares; the settings UI (M13) renders these.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingDef {
    pub key: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub default: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// A validated manifest.
#[derive(Debug, Clone)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: semver::Version,
    pub entry: String,
    pub provides: Vec<String>,
    pub net: NetPolicy,
    pub webview: bool,
    pub content_rating: ContentRating,
    pub settings: Vec<SettingDef>,
    pub description: Option<String>,
    pub author: Option<String>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ManifestError {
    #[error("invalid manifest JSON: {0}")]
    Json(String),
    #[error("invalid extension id {0:?}: use reverse-DNS like dev.example.name")]
    Id(String),
    #[error("name must be 1-80 characters")]
    Name,
    #[error("version {0:?} is not semver")]
    Version(String),
    #[error("apiVersion {0} is not supported (this Raygun supports {API_VERSION})")]
    ApiVersion(u32),
    #[error("runtime {0:?} is not supported")]
    Runtime(String),
    #[error("entry {0:?} must be a relative path to a .js file inside the extension")]
    Entry(String),
    #[error("invalid provider kind {0:?}")]
    Kind(String),
    #[error("invalid net permission {0:?}: use a host like api.example.org or *.example.org")]
    Net(String),
    #[error("duplicate or empty setting key {0:?}")]
    Setting(String),
}

impl Manifest {
    pub fn parse(json: &str) -> Result<Self, ManifestError> {
        let raw: RawManifest =
            serde_json::from_str(json).map_err(|error| ManifestError::Json(error.to_string()))?;
        if !is_extension_id(&raw.id) {
            return Err(ManifestError::Id(raw.id));
        }
        let name = raw.name.trim().to_owned();
        if name.is_empty() || name.chars().count() > 80 {
            return Err(ManifestError::Name);
        }
        let version = semver::Version::parse(&raw.version)
            .map_err(|_| ManifestError::Version(raw.version))?;
        if raw.api_version != API_VERSION {
            return Err(ManifestError::ApiVersion(raw.api_version));
        }
        if raw.runtime != "js" {
            return Err(ManifestError::Runtime(raw.runtime));
        }
        if !is_safe_entry(&raw.entry) {
            return Err(ManifestError::Entry(raw.entry));
        }
        if let Some(kind) = raw.provides.iter().find(|kind| !is_kind(kind)) {
            return Err(ManifestError::Kind(kind.clone()));
        }
        let patterns = raw
            .permissions
            .net
            .iter()
            .map(|pattern| {
                HostPattern::parse(pattern).ok_or_else(|| ManifestError::Net(pattern.clone()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut keys = HashSet::new();
        if let Some(setting) = raw
            .settings
            .iter()
            .find(|setting| setting.key.is_empty() || !keys.insert(setting.key.as_str()))
        {
            return Err(ManifestError::Setting(setting.key.clone()));
        }
        Ok(Self {
            id: raw.id,
            name,
            version,
            entry: raw.entry,
            provides: raw.provides,
            net: NetPolicy::new(patterns),
            webview: raw.permissions.webview,
            content_rating: raw.content_rating,
            settings: raw.settings,
            description: raw.description,
            author: raw.author,
        })
    }

    /// The default value of a declared setting.
    pub fn setting_default(&self, key: &str) -> Option<&serde_json::Value> {
        self.settings
            .iter()
            .find(|setting| setting.key == key)
            .and_then(|setting| setting.default.as_ref())
    }
}

/// Reverse-DNS: at least two dot-separated labels of `[a-z0-9-]`, each starting alphanumerically.
pub fn is_extension_id(id: &str) -> bool {
    id.len() <= 100
        && id.split('.').count() >= 2
        && id.split('.').all(|label| is_label(label, b"-"))
}

/// Provider kinds look like `music.metadata`, `manga.source`, `lyrics`.
fn is_kind(kind: &str) -> bool {
    kind.len() <= 64 && kind.split('.').all(|label| is_label(label, b"-"))
}

/// A provider id within an extension: `[a-z0-9][a-z0-9_-]*`.
pub fn is_provider_id(id: &str) -> bool {
    id.len() <= 64 && is_label(id, b"-_")
}

fn is_label(label: &str, extra: &[u8]) -> bool {
    let bytes = label.as_bytes();
    !bytes.is_empty()
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || extra.contains(b))
}

fn is_safe_entry(entry: &str) -> bool {
    entry.ends_with(".js")
        && !entry.contains('\\')
        && !entry.contains(':')
        && entry
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}
