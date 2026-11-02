//! `raygun-extension.json` (ARCHITECTURE §7.1).

use std::collections::HashSet;

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
    /// Written by the Nuclear converter (M17): the original package and its compatibility report.
    #[serde(default)]
    nuclear: Option<serde_json::Value>,
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

impl ContentRating {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::Suggestive => "suggestive",
            Self::Nsfw => "nsfw",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SettingKind {
    Text,
    Number,
    Boolean,
    Select,
}

/// A setting the extension declares; the app's settings form renders these (M13).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingDef {
    pub key: String,
    #[serde(rename = "type")]
    pub kind: SettingKind,
    #[serde(default)]
    pub label: Option<String>,
    /// The choices of a `select`.
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub default: Option<serde_json::Value>,
}

impl SettingDef {
    /// Whether `value` fits this setting: the right JSON type, and one of the options for a select.
    pub fn accepts(&self, value: &serde_json::Value) -> bool {
        match self.kind {
            SettingKind::Text => value.is_string(),
            SettingKind::Number => value.as_f64().is_some_and(f64::is_finite),
            SettingKind::Boolean => value.is_boolean(),
            SettingKind::Select => value
                .as_str()
                .is_some_and(|choice| self.options.iter().any(|option| option == choice)),
        }
    }
}

/// What runs the extension's entry file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Runtime {
    Js,
    /// A Shosetsu Lua script converted at install (M32).
    Shosetsu,
}

/// A validated manifest.
#[derive(Debug, Clone)]
pub struct Manifest {
    pub id: String,
    pub runtime: Runtime,
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
    /// Present for converted Nuclear plugins.
    pub nuclear: Option<serde_json::Value>,
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
    #[error(
        "setting {0:?}: duplicate or empty key, a select without options, or a default of the wrong type"
    )]
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
        let runtime = match raw.runtime.as_str() {
            "js" => Runtime::Js,
            "shosetsu" => Runtime::Shosetsu,
            _ => return Err(ManifestError::Runtime(raw.runtime)),
        };
        if !is_safe_entry(&raw.entry, runtime) {
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
                HostPattern::parse(pattern)
                    // Any-host access is for Shosetsu scripts (Nuclear conversions get it below).
                    .filter(|parsed| {
                        *parsed != HostPattern::AnyPublic || runtime == Runtime::Shosetsu
                    })
                    .ok_or_else(|| ManifestError::Net(pattern.clone()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut keys = HashSet::new();
        if let Some(setting) = raw.settings.iter().find(|setting| {
            setting.key.is_empty()
                || !keys.insert(setting.key.as_str())
                || (setting.kind == SettingKind::Select && setting.options.is_empty())
                || setting
                    .default
                    .as_ref()
                    .is_some_and(|value| !setting.accepts(value))
        }) {
            return Err(ManifestError::Setting(setting.key.clone()));
        }
        // Nuclear runs its plugins without a host list, and their code builds addresses the
        // conversion can't see (SoundCloud's CDN, consent pages): converted plugins also reach any
        // public host, as Shosetsu scripts do (M46.9.1). The hosts found in their code stay.
        let mut patterns = patterns;
        if raw.nuclear.is_some() && !patterns.contains(&HostPattern::AnyPublic) {
            patterns.push(HostPattern::AnyPublic);
        }
        Ok(Self {
            id: raw.id,
            runtime,
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
            nuclear: raw.nuclear,
        })
    }

    pub fn setting(&self, key: &str) -> Option<&SettingDef> {
        self.settings.iter().find(|setting| setting.key == key)
    }

    /// The default value of a declared setting.
    pub fn setting_default(&self, key: &str) -> Option<&serde_json::Value> {
        self.setting(key)
            .and_then(|setting| setting.default.as_ref())
    }

    /// Checks a user's value for one of this extension's settings.
    pub fn check_setting(&self, key: &str, value: &serde_json::Value) -> Result<(), String> {
        let setting = self
            .setting(key)
            .ok_or_else(|| format!("{} has no setting {key:?}", self.name))?;
        if setting.accepts(value) {
            Ok(())
        } else {
            Err(format!("{value} is not a valid value for {key:?}"))
        }
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

fn is_safe_entry(entry: &str, runtime: Runtime) -> bool {
    let extension = match runtime {
        Runtime::Js => ".js",
        Runtime::Shosetsu => ".lua",
    };
    entry.ends_with(extension)
        && !entry.contains('\\')
        && !entry.contains(':')
        && entry
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}
