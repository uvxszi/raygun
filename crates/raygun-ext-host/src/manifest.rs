//! `raygun-extension.json` (ARCHITECTURE §7.1).

use std::collections::HashSet;

use serde::Deserialize;

use crate::permissions::{HostPattern, NetPolicy};
use crate::site::{RawSite, SiteScripts};

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
    /// Written by the Mihon converter (`crate::mihon`): the extension's package, version code
    /// and the converter that made it.
    #[serde(default)]
    mihon: Option<serde_json::Value>,
    /// Written when a Nuvio provider is installed (`crate::nuvio`): the provider's id in its
    /// repository and the content types it finds streams for.
    #[serde(default)]
    nuvio: Option<serde_json::Value>,
    /// The site its sources read, opened from their menus (M46.11).
    #[serde(default)]
    website: Option<String>,
    /// Scripts for the websites browser (runtime `site`, `crate::site`).
    #[serde(default)]
    site: Option<RawSite>,
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
    /// The extension can't work without it (a key, a secret, a server's address, a user name):
    /// while it's empty, the host asks the extension nothing (`ExtensionHost::needs_setup`).
    /// Not for a switch, which always has a value.
    #[serde(default)]
    pub required: bool,
}

impl SettingDef {
    /// Whether `value` (the user's, else the default) leaves a required setting empty: nothing,
    /// or text that's only spaces.
    pub fn is_empty(value: Option<&serde_json::Value>) -> bool {
        match value {
            None | Some(serde_json::Value::Null) => true,
            Some(serde_json::Value::String(text)) => text.trim().is_empty(),
            Some(_) => false,
        }
    }

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
    /// A Miru JavaScript extension, run unchanged in Raygun's Miru runtime (`crate::miru`).
    Miru,
    /// An LNReader plugin, run unchanged in Raygun's LNReader runtime (`crate::lnreader`).
    LnReader,
    /// A server people connect (Jellyfin, Subsonic, Audiobookshelf, OPDS), answered in Rust
    /// (`crate::servers`); the entry is its `server.json`.
    Server,
    /// A Mihon extension converted at install (`crate::mihon`), run with the Mihon prelude.
    Mihon,
    /// Scripts and styles for websites (`crate::site`): nothing runs in Raygun; the websites
    /// browser adds them to the pages they name. The entry is one of its scripts.
    Site,
    /// A Nuvio provider (a stream scraper from a Nuvio repository), run unchanged in Raygun's
    /// Nuvio runtime (`crate::nuvio`).
    Nuvio,
    /// A Kodi plugin, run unchanged in Raygun's Python (`crate::kodi`). The entry is its script.
    Kodi,
}

impl Runtime {
    /// Whether the script declares its settings as it runs (Miru's `registerSetting`, LNReader's
    /// `pluginSettings`, a Nuvio provider's `onSettings`), so they're known only once it has
    /// loaded.
    pub fn declares_settings(self) -> bool {
        matches!(self, Self::Miru | Self::LnReader | Self::Nuvio)
    }
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
    /// Present for converted Mihon extensions: `{ pkg, versionCode, converter, theme, source }`.
    pub mihon: Option<serde_json::Value>,
    /// Present for Nuvio providers: `{ scraper, types }`.
    pub nuvio: Option<serde_json::Value>,
    /// An http(s) address; others are dropped.
    pub website: Option<String>,
    /// A site-scripts extension's scripts (runtime `site`).
    pub site: Option<SiteScripts>,
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
        "setting {0:?}: duplicate or empty key, a select without options, a default of the wrong type, or a required switch"
    )]
    Setting(String),
    #[error("site scripts: {0}")]
    Site(String),
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
            "miru" => Runtime::Miru,
            "lnreader" => Runtime::LnReader,
            "server" => Runtime::Server,
            "mihon" => Runtime::Mihon,
            "site" => Runtime::Site,
            "nuvio" => Runtime::Nuvio,
            "kodi" => Runtime::Kodi,
            _ => return Err(ManifestError::Runtime(raw.runtime)),
        };
        if !is_safe_entry(&raw.entry, runtime) {
            return Err(ManifestError::Entry(raw.entry));
        }
        if let Some(kind) = raw.provides.iter().find(|kind| !is_kind(kind)) {
            return Err(ManifestError::Kind(kind.clone()));
        }
        let site = site_section(
            raw.site.as_ref(),
            &raw.provides,
            &raw.permissions,
            &raw.entry,
            runtime,
        )?;
        let patterns = raw
            .permissions
            .net
            .iter()
            // `*` is any public host: Shosetsu scripts, Nuclear conversions (below) and sources
            // spread over many servers, like PeerTube's federated instances (M46.35). Never this
            // machine or the local network; the install sheet says it reaches any site.
            .map(|pattern| {
                HostPattern::parse(pattern).ok_or_else(|| ManifestError::Net(pattern.clone()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut keys = HashSet::new();
        if let Some(setting) = raw.settings.iter().find(|setting| {
            setting.key.is_empty()
                || !keys.insert(setting.key.as_str())
                || (setting.kind == SettingKind::Select && setting.options.is_empty())
                || (setting.required && setting.kind == SettingKind::Boolean)
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
            mihon: raw.mihon,
            nuvio: raw.nuvio,
            website: raw
                .website
                .filter(|url| url.starts_with("https://") || url.starts_with("http://")),
            site,
        })
    }

    /// Whether the script adds settings of its own as it runs (Miru's `registerSetting`,
    /// LNReader's `pluginSettings`, a Nuclear plugin's `Settings.register`): the user's values
    /// for them are kept at load and checked when they're listed.
    pub fn registers_settings(&self) -> bool {
        self.runtime.declares_settings() || self.nuclear.is_some()
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

/// A site-scripts extension's `site` section, checked: only runtime `site` has one (and needs
/// it), provides `site.scripts` and nothing else, reaches nothing through Raygun (its pages'
/// requests are their own), and its entry is one of its scripts.
fn site_section(
    site: Option<&RawSite>,
    provides: &[String],
    permissions: &RawPermissions,
    entry: &str,
    runtime: Runtime,
) -> Result<Option<SiteScripts>, ManifestError> {
    let wrong = |reason: &str| Err(ManifestError::Site(reason.to_owned()));
    if runtime != Runtime::Site {
        if site.is_some() || provides.iter().any(|kind| kind == crate::site::KIND) {
            return wrong("only runtime \"site\" has site scripts");
        }
        return Ok(None);
    }
    let Some(section) = site.cloned() else {
        return wrong("runtime \"site\" needs a \"site\" section");
    };
    if provides != [crate::site::KIND] {
        return wrong("runtime \"site\" provides [\"site.scripts\"] and nothing else");
    }
    if !permissions.net.is_empty() || permissions.webview {
        return wrong("site scripts take no permissions: their pages' requests are the pages' own");
    }
    let scripts = SiteScripts::parse(section).map_err(ManifestError::Site)?;
    if !scripts
        .groups
        .iter()
        .any(|group| group.js.iter().any(|path| path == entry))
    {
        return wrong("the entry must be one of the scripts' js files");
    }
    Ok(Some(scripts))
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
        Runtime::Js
        | Runtime::Miru
        | Runtime::LnReader
        | Runtime::Mihon
        | Runtime::Site
        | Runtime::Nuvio => ".js",
        Runtime::Shosetsu => ".lua",
        Runtime::Server => ".json",
        Runtime::Kodi => ".py",
    };
    entry.ends_with(extension)
        && !entry.contains('\\')
        && !entry.contains(':')
        && entry
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}
