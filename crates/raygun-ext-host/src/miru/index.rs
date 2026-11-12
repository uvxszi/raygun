//! Miru repositories: miru-repo's `index.json` (MIT), written by its `main.go`, is an array of each
//! script's metadata fields as strings (`name, package, type, lang, license, version, webSite`,
//! optional `icon`, `nsfw`, `description`, `tags`) plus `url`, the script's file under `repo/`.
//! The Miru app installs `<repository>/repo/<package>.js`; Raygun reads the file `url` names,
//! which is the same file except where two scripts share a package.

use std::collections::HashSet;

use serde_json::Value;

use super::header::{MiruType, semver_version};
use crate::manifest::{API_VERSION, is_extension_id};
use crate::repo::{INDEX_FORMAT, PackageFormat, RepoEntry, RepoIndex, RepoPermissions};

/// What a Miru repository is called in Raygun (the index names none).
pub const REPOSITORY_NAME: &str = "Miru extensions";

/// A label for an extension id: lower case, other characters as `-`.
fn label(text: &str) -> String {
    let label: String = text
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    label.trim_matches('-').to_owned()
}

/// The Raygun id of a Miru package: `miru.` and the package's parts (`de.ccc.media` →
/// `miru.de.ccc.media`, `9anime.to` → `miru.9anime.to`).
pub fn extension_id(package: &str) -> Option<String> {
    let parts: Vec<String> = package
        .split('.')
        .map(label)
        .filter(|part| !part.is_empty())
        .collect();
    if parts.is_empty() {
        return None;
    }
    let id = format!("miru.{}", parts.join("."));
    is_extension_id(&id).then_some(id)
}

/// A field as text: Miru writes strings, hand-made indexes sometimes numbers or booleans.
fn field(entry: &Value, key: &str) -> Option<String> {
    let text = match entry.get(key)? {
        Value::String(text) => text.trim().to_owned(),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        _ => return None,
    };
    (!text.is_empty()).then_some(text)
}

/// Whether `json` is shaped like a Miru index: an array of entries naming a package, a type
/// and a website.
fn is_miru(value: &Value) -> bool {
    value.as_array().is_some_and(|entries| {
        entries.iter().any(|entry| {
            ["package", "type", "webSite"]
                .iter()
                .all(|key| entry.get(key).is_some())
        })
    })
}

/// A Miru `lang` as Raygun writes languages.
fn language(lang: &str) -> String {
    match lang.to_ascii_lowercase().as_str() {
        "all" => "multi".to_owned(),
        "jp" => "ja".to_owned(),
        other => other.to_owned(),
    }
}

/// The script's file, relative to the index (`repo/<file>`), or an absolute address as listed.
/// A file not named after its package carries the package as the fragment (never sent), so the
/// install can check the script is the one listed.
fn script_path(url: Option<&str>, package: &str) -> String {
    let own = format!("{package}.js");
    let path = match url {
        Some(url) if url.starts_with("https://") || url.starts_with("http://") => url.to_owned(),
        Some(url) => format!("repo/{}", url.trim_start_matches('/')),
        None => return format!("repo/{own}"),
    };
    if path.rsplit('/').next() == Some(own.as_str()) || path.contains('#') {
        path
    } else {
        format!("{path}#{package}")
    }
}

/// The package an entry's script must be, when its file isn't named after it.
pub fn listed_package(entry: &RepoEntry) -> Option<&str> {
    entry
        .package
        .split_once('#')
        .map(|(_, package)| package)
        .filter(|package| !package.is_empty())
}

/// The description the catalogue shows: the repository's, and what kind of source it is.
pub fn description(description: Option<&str>, kind: MiruType) -> String {
    let what = format!("A Miru {} source.", kind.noun());
    match description
        .map(str::trim)
        .map(|text| text.trim_end_matches('.'))
        .filter(|text| !text.is_empty())
    {
        Some(text) => format!("{text}. {what}"),
        None => what,
    }
}

/// Reads a Miru index; `None` when `json` isn't one.
pub fn parse(json: &str) -> Option<RepoIndex> {
    let value: Value = serde_json::from_str(json).ok()?;
    if !is_miru(&value) {
        return None;
    }
    let entries = value.as_array()?;
    // A package's own file (`<package>.js`) keeps the package's id; another script naming the
    // same package (kuromanhua.xyz.js says komiku.com) takes its file's name.
    let own_file = |entry: &Value| {
        let package = field(entry, "package").unwrap_or_default();
        field(entry, "url").is_none_or(|url| url == format!("{package}.js"))
    };
    let mut order: Vec<(usize, &Value)> = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| own_file(entry))
        .collect();
    order.extend(
        entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| !own_file(entry)),
    );
    let mut used = HashSet::new();
    let mut extensions = Vec::new();
    for (position, entry) in order {
        let usable = (|| {
            let package = field(entry, "package")?;
            let kind = MiruType::parse(&field(entry, "type")?)?;
            let version = semver_version(&field(entry, "version")?);
            let url = field(entry, "url");
            let mut id = extension_id(&package)?;
            if used.contains(&id) {
                let stem = url.as_deref()?.rsplit('/').next()?.trim_end_matches(".js");
                id = extension_id(stem).filter(|id| !used.contains(id))?;
            }
            let nsfw = field(entry, "nsfw").is_some_and(|nsfw| nsfw == "true");
            Some(RepoEntry {
                description: Some(description(field(entry, "description").as_deref(), kind)),
                name: field(entry, "name").unwrap_or_else(|| package.clone()),
                version,
                api_version: API_VERSION,
                author: field(entry, "author"),
                provides: vec![kind.provider_kind().to_owned()],
                permissions: RepoPermissions {
                    net: vec!["*".to_owned()],
                    webview: false,
                },
                content_rating: if nsfw { "nsfw" } else { "safe" }.to_owned(),
                lang: field(entry, "lang").map(|lang| language(&lang)),
                package: script_path(url.as_deref(), &package),
                icon: field(entry, "icon"),
                sha256: String::new(),
                size: 0,
                format: PackageFormat::Miru,
                sites: Vec::new(),
                not_for_raygun: None,
                id,
            })
        })();
        match usable {
            Some(found) => {
                used.insert(found.id.clone());
                extensions.push((position, found));
            }
            None => tracing::warn!(entry = %entry, "skipping unusable Miru index entry"),
        }
    }
    // The index's own order, whatever order ids were given in.
    extensions.sort_by_key(|(position, _)| *position);
    Some(RepoIndex {
        format_version: INDEX_FORMAT,
        name: REPOSITORY_NAME.to_owned(),
        extensions: extensions.into_iter().map(|(_, entry)| entry).collect(),
        libraries: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn reads_a_miru_index() {
        let index = json!([
            { "author": "Christian Weiske", "description": "Video material provided by the Chaos Computer Club",
              "icon": "https://media.ccc.de/favicon-96x96.png", "lang": "en", "license": "AGPL",
              "name": "media.ccc.de", "nsfw": "false", "package": "de.ccc.media", "type": "bangumi",
              "url": "de.ccc.media.js", "version": "v0.0.1", "webSite": "https://api.media.ccc.de" },
            { "lang": "id", "name": "Kuromanhua.xyz", "package": "komiku.com", "type": "manga",
              "url": "kuromanhua.xyz.js", "version": "v0.0.1", "webSite": "https://kuromanhua.xyz" },
            { "lang": "id", "name": "Komiku.com", "package": "komiku.com", "type": "manga",
              "url": "komiku.com.js", "version": "v0.0.1", "webSite": "https://komiku.com", "nsfw": "true" },
            { "lang": "all", "name": "Comic", "package": "x.comic", "type": "comic",
              "url": "x.comic.js", "version": "v1", "webSite": "https://x" },
            { "name": "Old", "package": "old.one", "type": "fikushon", "version": "1.2", "webSite": "" },
        ])
        .to_string();
        let parsed = crate::repo::parse_index(&index).unwrap();
        assert_eq!(parsed.name, REPOSITORY_NAME);
        let ids: Vec<&str> = parsed
            .extensions
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        assert_eq!(
            ids,
            [
                "miru.de.ccc.media",
                "miru.kuromanhua.xyz",
                "miru.komiku.com",
                "miru.old.one"
            ],
            "an unknown type is left out; a shared package goes to its own file"
        );
        let ccc = &parsed.extensions[0];
        assert_eq!(ccc.format, PackageFormat::Miru);
        assert_eq!(ccc.version, "0.0.1");
        assert_eq!(ccc.provides, ["video.source"]);
        assert_eq!(ccc.permissions.net, ["*"]);
        assert_eq!(ccc.content_rating, "safe");
        assert_eq!(ccc.package, "repo/de.ccc.media.js");
        assert_eq!(
            ccc.description.as_deref(),
            Some("Video material provided by the Chaos Computer Club. A Miru video source.")
        );
        let base: url::Url = "https://miru-repo.0n0.dev/index.json".parse().unwrap();
        assert_eq!(
            crate::repo::package_url(&base, ccc).unwrap().as_str(),
            "https://miru-repo.0n0.dev/repo/de.ccc.media.js"
        );
        assert_eq!(
            crate::repo::icon_url(&base, ccc).unwrap().as_str(),
            "https://media.ccc.de/favicon-96x96.png"
        );
        let kuromanhua = &parsed.extensions[1];
        assert_eq!(kuromanhua.package, "repo/kuromanhua.xyz.js#komiku.com");
        assert_eq!(listed_package(kuromanhua), Some("komiku.com"));
        assert_eq!(listed_package(ccc), None);
        assert_eq!(
            crate::repo::package_url(&base, kuromanhua).unwrap().path(),
            "/repo/kuromanhua.xyz.js"
        );
        assert_eq!(parsed.extensions[2].content_rating, "nsfw");
        assert_eq!(parsed.extensions[2].provides, ["manga.source"]);
        let old = &parsed.extensions[3];
        assert_eq!(
            (old.version.as_str(), old.package.as_str()),
            ("1.2.0", "repo/old.one.js")
        );
        assert_eq!(old.provides, ["novel.source"]);
        assert_eq!(old.description.as_deref(), Some("A Miru novel source."));
    }

    #[test]
    fn ids_come_from_packages() {
        assert_eq!(extension_id("9anime.to").as_deref(), Some("miru.9anime.to"));
        assert_eq!(
            extension_id("dev.0n0.miru.7xi").as_deref(),
            Some("miru.dev.0n0.miru.7xi")
        );
        assert_eq!(
            extension_id("animeworld").as_deref(),
            Some("miru.animeworld")
        );
        assert_eq!(
            extension_id("Com_Example.App").as_deref(),
            Some("miru.com-example.app")
        );
        assert_eq!(extension_id("..."), None);
    }

    #[test]
    fn other_arrays_are_not_miru() {
        assert!(parse("[]").is_none());
        assert!(parse(r#"[{"name":"x","pkg":"y"}]"#).is_none());
        assert!(parse(r#"{"package":"x","type":"manga","webSite":"y"}"#).is_none());
    }
}
