//! Nuvio repositories: a `manifest.json` naming the repository and listing its providers
//! (`scrapers`), each a JavaScript file beside it (or at its own address). Read as Nuvio's
//! `PluginManifestParser` and `PluginRepository` read them (tapframe/NuvioStreaming, GPL-3.0):
//!
//! ```json
//! { "name": "…", "version": "1.0.0", "description": "…", "author": "…",
//!   "scrapers": [{ "id": "example", "name": "Example", "version": "1.0.0",
//!     "filename": "providers/example.js", "supportedTypes": ["movie", "tv"],
//!     "enabled": true, "logo": "https://…", "contentLanguage": ["en"],
//!     "supportedPlatforms": ["android"], "disabledPlatforms": [], "formats": ["mp4"] }] }
//! ```
//!
//! A provider whose repository turned it off (`enabled: false`), that's made for other
//! platforms, or that finds streams for no type Raygun asks about is listed as not for Raygun.

use std::collections::HashSet;

use reqwest::Url;
use serde_json::Value;

use crate::manifest::{API_VERSION, is_extension_id};
use crate::repo::{INDEX_FORMAT, PackageFormat, RepoEntry, RepoIndex, RepoPermissions};

/// The platform Raygun runs providers as: Nuvio's Android app (its full runtime: fetch, cheerio,
/// crypto), on every device.
const PLATFORM: &str = "android";
/// Longest description kept from a repository.
const MAX_DESCRIPTION: usize = 300;

fn field(entry: &Value, key: &str) -> Option<String> {
    let text = match entry.get(key)? {
        Value::String(text) => text.trim().to_owned(),
        Value::Number(number) => number.to_string(),
        _ => return None,
    };
    (!text.is_empty()).then_some(text)
}

fn texts(entry: &Value, key: &str) -> Vec<String> {
    entry
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|text| text.trim().to_ascii_lowercase())
        .filter(|text| !text.is_empty())
        .collect()
}

/// A content type as Nuvio counts it (`normalizePluginType`): series, shows and others are `tv`.
pub fn normalize_type(kind: &str) -> String {
    match kind.trim().to_ascii_lowercase().as_str() {
        "series" | "show" | "other" => "tv".to_owned(),
        other => other.to_owned(),
    }
}

/// The types a provider finds streams for, as Nuvio counts them; Nuvio's default is both.
pub fn supported_types(scraper: &Value) -> Vec<String> {
    let mut types: Vec<String> = Vec::new();
    match scraper.get("supportedTypes") {
        Some(Value::Array(_)) => {
            for kind in texts(scraper, "supportedTypes") {
                let kind = normalize_type(&kind);
                if !types.contains(&kind) {
                    types.push(kind);
                }
            }
        }
        _ => types = vec!["movie".to_owned(), "tv".to_owned()],
    }
    types
}

/// The Raygun id of a Nuvio provider: `nuvio.` and its id (`example` → `nuvio.example`,
/// `My_Source` → `nuvio.my-source`).
pub fn extension_id(scraper: &str) -> Option<String> {
    let parts: Vec<String> = scraper
        .to_ascii_lowercase()
        .split('.')
        .map(crate::repo::id_label)
        .filter(|part| !part.is_empty())
        .collect();
    if parts.is_empty() {
        return None;
    }
    let id = format!("nuvio.{}", parts.join("."));
    is_extension_id(&id).then_some(id)
}

/// Whether `value` is shaped like a Nuvio repository's manifest: a name and a list of providers,
/// each with an id and a file. (A Stremio addon's manifest has `resources` instead.)
fn is_nuvio(value: &Value) -> bool {
    value.get("name").is_some()
        && value.get("resources").is_none()
        && value
            .get("scrapers")
            .and_then(Value::as_array)
            .is_some_and(|scrapers| {
                scrapers.iter().any(|scraper| {
                    ["id", "filename"]
                        .iter()
                        .all(|key| scraper.get(key).is_some())
                })
            })
}

/// Why a provider isn't for Raygun, from what its repository says about it.
fn not_for_raygun(scraper: &Value, types: &[String]) -> Option<String> {
    if scraper.get("enabled").and_then(Value::as_bool) == Some(false) {
        return Some("Its repository turned it off".to_owned());
    }
    let supported = texts(scraper, "supportedPlatforms");
    let disabled = texts(scraper, "disabledPlatforms");
    if (!supported.is_empty() && !supported.iter().any(|platform| platform == PLATFORM))
        || disabled.iter().any(|platform| platform == PLATFORM)
    {
        let made_for = if supported.is_empty() {
            "other platforms".to_owned()
        } else {
            supported.join(", ")
        };
        return Some(format!(
            "Made for {made_for}, not for Android, whose runtime Raygun's stands in for"
        ));
    }
    if !types.iter().any(|kind| kind == "movie" || kind == "tv") {
        return Some(format!(
            "Finds streams only for {}, which Raygun doesn't ask it for",
            if types.is_empty() {
                "no type".to_owned()
            } else {
                types.join(", ")
            }
        ));
    }
    None
}

/// The provider an entry installs (its id in its repository) and the types it serves, carried in
/// the package address's fragment, which is never sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub scraper: String,
    pub types: Vec<String>,
}

/// What an entry's package address says about the provider (see [`Listed`]).
pub fn listed(entry: &RepoEntry) -> Option<Listed> {
    let (_, fragment) = entry.package.rsplit_once('#')?;
    let mut scraper = None;
    let mut types = Vec::new();
    for (key, value) in url::form_urlencoded::parse(fragment.as_bytes()) {
        match key.as_ref() {
            "scraper" if !value.is_empty() => scraper = Some(value.into_owned()),
            "types" => {
                types = value
                    .split(',')
                    .map(str::trim)
                    .filter(|kind| !kind.is_empty())
                    .map(str::to_owned)
                    .collect();
            }
            _ => {}
        }
    }
    Some(Listed {
        scraper: scraper?,
        types,
    })
}

/// A provider's file as Nuvio finds it: an address of its own, else beside the manifest (Nuvio
/// drops a leading `/`).
fn package_of(filename: &str) -> Option<String> {
    if filename.starts_with("https://") || filename.starts_with("http://") {
        return Url::parse(filename).ok().map(|url| {
            let mut url = url;
            url.set_fragment(None);
            url.to_string()
        });
    }
    let relative = filename.trim_start_matches('/');
    (!relative.is_empty() && !relative.contains("://") && !relative.contains('#'))
        .then(|| relative.to_owned())
}

/// What a provider is for, said in its description.
fn what_for(types: &[String]) -> &'static str {
    let movies = types.iter().any(|kind| kind == "movie");
    let series = types.iter().any(|kind| kind == "tv");
    match (movies, series) {
        (true, true) => "movies and series",
        (true, false) => "movies",
        (false, true) => "series",
        (false, false) => "titles",
    }
}

/// Reads a Nuvio repository's manifest; `None` when `json` isn't one.
pub fn parse(json: &str) -> Option<RepoIndex> {
    let value: Value = serde_json::from_str(json).ok()?;
    if !is_nuvio(&value) {
        return None;
    }
    let mut seen = HashSet::new();
    let extensions = value
        .get("scrapers")?
        .as_array()?
        .iter()
        .filter_map(|scraper| {
            let id = field(scraper, "id")?;
            let extension = extension_id(&id)?;
            let package = package_of(&field(scraper, "filename")?)?;
            if !seen.insert(extension.clone()) {
                return None;
            }
            let name = field(scraper, "name").unwrap_or_else(|| id.clone());
            let types = supported_types(scraper);
            let about = field(scraper, "description")
                .map(|text| {
                    let text: String = text.chars().take(MAX_DESCRIPTION).collect();
                    let stop = if text.ends_with(['.', '!', '?']) {
                        ""
                    } else {
                        "."
                    };
                    format!("{text}{stop} ")
                })
                .unwrap_or_default();
            let languages = texts(scraper, "contentLanguage");
            let lang = match languages.as_slice() {
                [] => None,
                [one] => Some(one.clone()),
                _ => Some("multi".to_owned()),
            };
            let fragment: String = url::form_urlencoded::Serializer::new(String::new())
                .append_pair("scraper", &id)
                .append_pair("types", &types.join(","))
                .finish();
            Some(RepoEntry {
                id: extension,
                description: Some(format!(
                    "{about}A Nuvio provider of streams for {}. It may reach any website.",
                    what_for(&types)
                )),
                not_for_raygun: not_for_raygun(scraper, &types),
                name: name.chars().take(80).collect(),
                version: crate::lnreader::index::semver_version(
                    &field(scraper, "version").unwrap_or_else(|| "1.0.0".to_owned()),
                ),
                api_version: API_VERSION,
                author: field(&value, "author"),
                provides: vec![crate::video::VIDEO_STREAMS.to_owned()],
                permissions: RepoPermissions {
                    net: vec!["*".to_owned()],
                    webview: false,
                },
                content_rating: "safe".to_owned(),
                lang,
                package: format!("{package}#{fragment}"),
                icon: field(scraper, "logo")
                    .filter(|logo| logo.starts_with("https://") || logo.starts_with("http://")),
                sha256: String::new(),
                size: 0,
                format: PackageFormat::Nuvio,
                sites: Vec::new(),
            })
        })
        .collect();
    Some(RepoIndex {
        format_version: INDEX_FORMAT,
        name: field(&value, "name").unwrap_or_default(),
        extensions,
        libraries: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn manifest() -> String {
        json!({
            "name": "Public Domain Films", "version": "1.2.0", "author": "Raygun tests",
            "description": "Films whose copyright ran out.",
            "scrapers": [
                { "id": "archive", "name": "Internet Archive", "version": "1.1",
                  "filename": "/providers/archive.js", "supportedTypes": ["movie"],
                  "logo": "https://example.org/archive.png", "contentLanguage": ["en"] },
                { "id": "Old_TV", "name": "Old TV", "version": "2.0.0",
                  "filename": "https://cdn.example.org/old-tv.js#v2", "supportedTypes": ["series"],
                  "contentLanguage": ["en", "fr"] },
                { "id": "retired", "name": "Retired", "version": "1.0.0",
                  "filename": "retired.js", "enabled": false },
                { "id": "ios", "name": "iOS only", "version": "1.0.0",
                  "filename": "ios.js", "supportedPlatforms": ["ios"] },
                { "id": "music", "name": "Music", "version": "1.0.0",
                  "filename": "music.js", "supportedTypes": ["music"] },
                { "id": "archive", "name": "Twice", "version": "1.0.0", "filename": "twice.js" },
                { "id": "nofile", "name": "No file", "version": "1.0.0" },
            ]
        })
        .to_string()
    }

    #[test]
    fn reads_a_nuvio_manifest() {
        let (index, format) = crate::repo::read_index(&manifest()).unwrap();
        assert_eq!(format, PackageFormat::Nuvio);
        assert_eq!(index.name, "Public Domain Films");
        let ids: Vec<&str> = index
            .extensions
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        assert_eq!(
            ids,
            [
                "nuvio.archive",
                "nuvio.old-tv",
                "nuvio.retired",
                "nuvio.ios",
                "nuvio.music"
            ]
        );
        let archive = &index.extensions[0];
        assert_eq!(archive.format, PackageFormat::Nuvio);
        assert_eq!(archive.provides, ["video.streams"]);
        assert_eq!(archive.permissions.net, ["*"]);
        assert_eq!(archive.version, "1.1.0");
        assert_eq!(archive.lang.as_deref(), Some("en"));
        assert_eq!(archive.not_for_raygun, None);
        assert!(
            archive
                .description
                .as_deref()
                .unwrap()
                .contains("streams for movies")
        );
        assert_eq!(
            listed(archive),
            Some(Listed {
                scraper: "archive".into(),
                types: vec!["movie".into()],
            })
        );
        let base: Url = "https://example.org/repo/manifest.json?token=1"
            .parse()
            .unwrap();
        assert_eq!(
            crate::repo::package_url(&base, archive).unwrap().as_str(),
            "https://example.org/repo/providers/archive.js#scraper=archive&types=movie",
            "beside the manifest, without its leading slash"
        );
        let old_tv = &index.extensions[1];
        assert_eq!(old_tv.lang.as_deref(), Some("multi"));
        assert_eq!(
            crate::repo::package_url(&base, old_tv).unwrap().as_str(),
            "https://cdn.example.org/old-tv.js#scraper=Old_TV&types=tv"
        );
        assert_eq!(listed(old_tv).unwrap().types, ["tv"]);
        let reasons: Vec<Option<&str>> = index.extensions[2..]
            .iter()
            .map(|entry| entry.not_for_raygun.as_deref())
            .collect();
        assert_eq!(reasons[0], Some("Its repository turned it off"));
        assert!(reasons[1].unwrap().starts_with("Made for ios"));
        assert!(reasons[2].unwrap().contains("only for music"));
    }

    #[test]
    fn stremio_manifests_and_other_indexes_are_not_nuvio() {
        let addon = json!({ "id": "org.example", "name": "Addon", "version": "1.0.0",
            "resources": ["stream"], "types": ["movie"], "scrapers": [{ "id": "a", "filename": "a.js" }] });
        assert!(parse(&addon.to_string()).is_none());
        assert!(parse(r#"{"name":"X","scrapers":[]}"#).is_none());
        assert!(parse(r#"{"formatVersion":1,"name":"R","extensions":[]}"#).is_none());
        assert_eq!(normalize_type("Series"), "tv");
        assert_eq!(extension_id("..."), None);
    }
}
