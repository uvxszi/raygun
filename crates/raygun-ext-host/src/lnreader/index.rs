//! LNReader repositories: `plugins.min.json` (or `plugins.json`), an array of each plugin's
//! `{ id, name, site, lang, version, url, iconUrl }`, `url` being the plugin's compiled script.
//! Languages are written as their names in themselves ("English", "Español", "日本語").

use std::collections::HashSet;

use serde_json::Value;

use crate::manifest::{API_VERSION, is_extension_id};
use crate::repo::{INDEX_FORMAT, PackageFormat, RepoEntry, RepoIndex, RepoPermissions};

/// What an LNReader repository is called in Raygun (the index names none).
pub const REPOSITORY_NAME: &str = "LNReader plugins";

fn field(entry: &Value, key: &str) -> Option<String> {
    let text = match entry.get(key)? {
        Value::String(text) => text.trim().to_owned(),
        Value::Number(number) => number.to_string(),
        _ => return None,
    };
    (!text.is_empty()).then_some(text)
}

/// Whether `value` is shaped like an LNReader index: entries naming an id, a site, a version
/// and a script.
fn is_lnreader(value: &Value) -> bool {
    value.as_array().is_some_and(|entries| {
        entries.iter().any(|entry| {
            ["id", "site", "version", "url"]
                .iter()
                .all(|key| entry.get(key).is_some())
                && entry.get("package").is_none()
        })
    })
}

/// The Raygun id of an LNReader plugin: `lnreader.` and its id (`royalroad` →
/// `lnreader.royalroad`, `wuxiaworld.site` → `lnreader.wuxiaworld.site`).
pub fn extension_id(plugin: &str) -> Option<String> {
    let parts: Vec<String> = plugin
        .to_ascii_lowercase()
        .split('.')
        .map(|part| {
            let label: String = part
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                .collect();
            label.trim_matches('-').to_owned()
        })
        .filter(|part| !part.is_empty())
        .collect();
    if parts.is_empty() {
        return None;
    }
    let id = format!("lnreader.{}", parts.join("."));
    is_extension_id(&id).then_some(id)
}

/// A language as LNReader names it, as Raygun writes languages.
fn language(name: &str) -> Option<String> {
    // LNReader's names carry a left-to-right mark before right-to-left ones.
    let name = name.trim_matches(|c: char| c.is_whitespace() || c == '\u{200e}' || c == '\u{200f}');
    let code = match name.to_lowercase().as_str() {
        "english" => "en",
        "español" | "spanish" => "es",
        "français" | "french" => "fr",
        "português" | "portuguese" => "pt",
        "deutsch" | "german" => "de",
        "italiano" | "italian" => "it",
        "русский" | "russian" => "ru",
        "українська" | "ukrainian" => "uk",
        "polski" | "polish" => "pl",
        "türkçe" | "turkish" => "tr",
        "العربية" | "arabic" => "ar",
        "indonesia" | "bahasa indonesia" | "indonesian" => "id",
        "tiếng việt" | "vietnamese" => "vi",
        "ไทย" | "thai" => "th",
        "日本語" | "japanese" => "ja",
        "한국어" | "korean" => "ko",
        "multi" | "multilingual" => "multi",
        other if other.starts_with("中文") || other == "chinese" => "zh",
        _ => return None,
    };
    Some(code.to_owned())
}

/// LNReader versions are semver; one that isn't is read leniently (`1.2` is `1.2.0`).
fn semver_version(version: &str) -> String {
    if semver::Version::parse(version).is_ok() {
        return version.to_owned();
    }
    let numbers: Vec<u64> = version
        .trim_start_matches(['v', 'V'])
        .split('.')
        .map_while(|part| part.parse().ok())
        .take(3)
        .collect();
    format!(
        "{}.{}.{}",
        numbers.first().copied().unwrap_or_default(),
        numbers.get(1).copied().unwrap_or_default(),
        numbers.get(2).copied().unwrap_or_default()
    )
}

/// The plugin an entry's script must be (its LNReader id), carried in the package address's
/// fragment, which is never sent.
pub fn listed_plugin(entry: &RepoEntry) -> Option<&str> {
    entry
        .package
        .rsplit_once('#')
        .map(|(_, plugin)| plugin)
        .filter(|plugin| !plugin.is_empty())
}

/// Reads an LNReader index; `None` when `json` isn't one.
pub fn parse(json: &str) -> Option<RepoIndex> {
    let value: Value = serde_json::from_str(json).ok()?;
    if !is_lnreader(&value) {
        return None;
    }
    let mut seen = HashSet::new();
    let extensions = value
        .as_array()?
        .iter()
        .filter_map(|entry| {
            let plugin = field(entry, "id")?;
            let url = field(entry, "url")?;
            if !(url.starts_with("https://") || url.starts_with("http://")) || url.contains('#') {
                return None;
            }
            let id = extension_id(&plugin)?;
            if !seen.insert(id.clone()) {
                return None;
            }
            let name = field(entry, "name").unwrap_or_else(|| plugin.clone());
            Some(RepoEntry {
                id,
                description: Some(format!(
                    "{name}, an LNReader novel source. It may reach any website."
                )),
                name,
                version: semver_version(&field(entry, "version")?),
                api_version: API_VERSION,
                author: None,
                provides: vec!["novel.source".to_owned()],
                permissions: RepoPermissions {
                    net: vec!["*".to_owned()],
                    webview: false,
                },
                content_rating: "suggestive".to_owned(),
                lang: field(entry, "lang").and_then(|lang| language(&lang)),
                package: format!("{url}#{plugin}"),
                icon: field(entry, "iconUrl"),
                sha256: String::new(),
                size: 0,
                format: PackageFormat::LnReader,
                sites: Vec::new(),
                not_for_raygun: None,
            })
        })
        .collect();
    Some(RepoIndex {
        format_version: INDEX_FORMAT,
        name: REPOSITORY_NAME.to_owned(),
        extensions,
        libraries: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn reads_an_lnreader_index() {
        let index = json!([
            { "id": "royalroad", "name": "Royal Road", "site": "https://www.royalroad.com/",
              "lang": "English", "version": "2.3.4",
              "url": "https://plugins.example.org/.js/src/plugins/english/royalroad.js",
              "iconUrl": "https://plugins.example.org/icons/royalroad.png" },
            { "id": "arnovel", "name": "ArNovel", "site": "https://ar.example.org/", "lang": "\u{200e}العربية",
              "version": "1.0", "url": "https://plugins.example.org/arnovel.js" },
            { "id": "royalroad", "name": "Twice", "site": "x", "version": "1.0.0", "url": "https://plugins.example.org/twice.js" },
            { "id": "local", "name": "No script", "site": "x", "version": "1.0.0", "url": "file:///x.js" },
        ])
        .to_string();
        let parsed = crate::repo::parse_index(&index).unwrap();
        assert_eq!(parsed.name, REPOSITORY_NAME);
        let ids: Vec<&str> = parsed
            .extensions
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        assert_eq!(ids, ["lnreader.royalroad", "lnreader.arnovel"]);
        let royal = &parsed.extensions[0];
        assert_eq!(royal.format, PackageFormat::LnReader);
        assert_eq!(royal.provides, ["novel.source"]);
        assert_eq!(royal.permissions.net, ["*"]);
        assert_eq!(royal.lang.as_deref(), Some("en"));
        assert_eq!(listed_plugin(royal), Some("royalroad"));
        let base: url::Url = "https://plugins.example.org/plugins.min.json"
            .parse()
            .unwrap();
        assert_eq!(
            crate::repo::package_url(&base, royal).unwrap().as_str(),
            "https://plugins.example.org/.js/src/plugins/english/royalroad.js#royalroad"
        );
        assert_eq!(parsed.extensions[1].lang.as_deref(), Some("ar"));
        assert_eq!(parsed.extensions[1].version, "1.0.0");
    }

    #[test]
    fn ids_keep_the_plugins_dots() {
        assert_eq!(
            extension_id("wuxiaworld.site").as_deref(),
            Some("lnreader.wuxiaworld.site")
        );
        assert_eq!(
            extension_id("Novel_Bin").as_deref(),
            Some("lnreader.novel-bin")
        );
        assert_eq!(extension_id("..."), None);
        assert!(parse(r#"[{"package":"x","type":"manga","webSite":"y","id":"a","site":"b","version":"1","url":"c"}]"#).is_none());
    }
}
