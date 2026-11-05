//! Mihon and Aniyomi repositories: `index.min.json`, an array of each extension's
//! `{ name, pkg, apk, lang, code, version, nsfw, sources: [{ name, lang, id, baseUrl }] }`, with
//! the APKs under `apk/` and the icons under `icon/` beside it (keiyoushi's layout; Aniyomi's is
//! the same with `animeextension` packages).
//!
//! The extensions are Android packages. They run in the `mihon` runtime where a build has one;
//! without it, installing one answers [`UNSUPPORTED`].

use std::collections::HashSet;

use serde_json::Value;

use crate::manifest::{API_VERSION, is_extension_id};
use crate::package::VerifiedPackage;
use crate::repo::{INDEX_FORMAT, PackageFormat, RepoEntry, RepoIndex, RepoPermissions};

/// What installing a Mihon extension answers where this build has no `mihon` runtime.
pub const UNSUPPORTED: &str = "Mihon extensions aren't supported on this device yet";

/// Whether this build has the `mihon` runtime, which runs Mihon and Aniyomi extensions.
pub const fn runtime_present() -> bool {
    false
}

/// The package the `mihon` runtime installs for a listed extension's APK.
pub fn package(entry: &RepoEntry, _apk: &[u8]) -> Result<VerifiedPackage, String> {
    tracing::info!(extension = %entry.id, "no mihon runtime in this build");
    Err(UNSUPPORTED.to_owned())
}

/// Mihon's and Aniyomi's package prefixes.
const MANGA_PREFIX: &str = "eu.kanade.tachiyomi.extension.";
const ANIME_PREFIX: &str = "eu.kanade.tachiyomi.animeextension.";

/// A label for an extension id: lower case, other characters as `-`.
fn label(text: &str) -> String {
    let label: String = text
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    label.trim_matches('-').to_owned()
}

/// The Raygun id of a Mihon package: `mihon.` and what follows Mihon's prefix
/// (`eu.kanade.tachiyomi.extension.en.mangadex` → `mihon.en.mangadex`); Aniyomi's under
/// `mihon.anime.`.
pub fn extension_id(pkg: &str) -> Option<String> {
    let pkg = pkg.trim().to_ascii_lowercase();
    let (prefix, rest) = match (
        pkg.strip_prefix(ANIME_PREFIX),
        pkg.strip_prefix(MANGA_PREFIX),
    ) {
        (Some(rest), _) => ("mihon.anime", rest),
        (None, Some(rest)) => ("mihon", rest),
        (None, None) => ("mihon", pkg.as_str()),
    };
    let labels: Vec<String> = rest
        .split('.')
        .map(label)
        .filter(|part| !part.is_empty())
        .collect();
    if labels.is_empty() {
        return None;
    }
    let id = format!("{prefix}.{}", labels.join("."));
    is_extension_id(&id).then_some(id)
}

/// Whether `value` is shaped like a Mihon index: entries naming a package, an APK and sources.
pub fn is_index(value: &Value) -> bool {
    value.as_array().is_some_and(|entries| {
        entries.iter().any(|entry| {
            ["pkg", "apk", "sources"]
                .iter()
                .all(|key| entry.get(key).is_some())
        })
    })
}

fn text(entry: &Value, key: &str) -> Option<String> {
    let text = match entry.get(key)? {
        Value::String(text) => text.trim().to_owned(),
        Value::Number(number) => number.to_string(),
        _ => return None,
    };
    (!text.is_empty()).then_some(text)
}

/// Mihon's versions are `1.4.183`; anything else is read leniently (`14.7` is `14.7.0`).
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

/// Reads a Mihon or Aniyomi index; `None` when `json` isn't one.
pub fn parse(json: &str) -> Option<RepoIndex> {
    let value: Value = serde_json::from_str(json).ok()?;
    if !is_index(&value) {
        return None;
    }
    let mut seen = HashSet::new();
    let extensions = value
        .as_array()?
        .iter()
        .filter_map(|entry| {
            let pkg = text(entry, "pkg")?;
            let apk = text(entry, "apk")?;
            if apk.contains(['/', '\\']) || pkg.contains(['/', '\\']) {
                return None;
            }
            let id = extension_id(&pkg).filter(|id| seen.insert(id.clone()))?;
            let anime = pkg.to_ascii_lowercase().starts_with(ANIME_PREFIX);
            let name = text(entry, "name").unwrap_or_else(|| pkg.clone());
            let name = ["Tachiyomi: ", "Aniyomi: "]
                .iter()
                .find_map(|prefix| name.strip_prefix(prefix))
                .unwrap_or(&name)
                .to_owned();
            let sources = entry
                .get("sources")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            let app = if anime { "Aniyomi" } else { "Mihon" };
            Some(RepoEntry {
                id,
                name,
                version: semver_version(&text(entry, "version").unwrap_or_default()),
                api_version: API_VERSION,
                description: Some(if sources > 1 {
                    format!("{sources} sources in one {app} extension.")
                } else {
                    format!("A {app} extension.")
                }),
                author: None,
                provides: vec![
                    if anime {
                        "video.source"
                    } else {
                        "manga.source"
                    }
                    .to_owned(),
                ],
                permissions: RepoPermissions {
                    net: vec!["*".to_owned()],
                    webview: false,
                },
                content_rating: if entry.get("nsfw").and_then(Value::as_i64) == Some(1) {
                    "nsfw"
                } else {
                    "safe"
                }
                .to_owned(),
                lang: text(entry, "lang").map(|lang| {
                    if lang == "all" {
                        "multi".to_owned()
                    } else {
                        lang
                    }
                }),
                package: format!("apk/{apk}"),
                icon: Some(format!("icon/{pkg}.png")),
                sha256: String::new(),
                size: 0,
                format: PackageFormat::Mihon,
            })
        })
        .collect();
    Some(RepoIndex {
        format_version: INDEX_FORMAT,
        // The index doesn't name its repository: the app names it after its address.
        name: String::new(),
        extensions,
        libraries: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn reads_mihon_and_aniyomi_indexes() {
        let index = json!([
            { "name": "Tachiyomi: MangaDex", "pkg": "eu.kanade.tachiyomi.extension.all.mangadex",
              "apk": "tachiyomi-all.mangadex-v1.4.190.apk", "lang": "all", "code": 190,
              "version": "1.4.190", "nsfw": 1,
              "sources": [
                { "name": "MangaDex", "lang": "en", "id": "1", "baseUrl": "https://mangadex.org" },
                { "name": "MangaDex", "lang": "ja", "id": "2", "baseUrl": "https://mangadex.org" },
              ] },
            { "name": "Aniyomi: AllAnime", "pkg": "eu.kanade.tachiyomi.animeextension.en.allanime",
              "apk": "aniyomi-en.allanime-v14.7.apk", "lang": "en", "code": 7, "version": "14.7",
              "nsfw": 0, "sources": [{ "name": "AllAnime", "lang": "en", "id": "3", "baseUrl": "https://allanime.to" }] },
            { "name": "Bad", "pkg": "eu.kanade.tachiyomi.extension.en.bad", "apk": "../x.apk", "sources": [] },
        ])
        .to_string();
        let parsed = parse(&index).unwrap();
        assert_eq!(parsed.name, "");
        let [manga, anime] = parsed.extensions.as_slice() else {
            panic!("{:?}", parsed.extensions);
        };
        assert_eq!(manga.id, "mihon.all.mangadex");
        assert_eq!(manga.name, "MangaDex");
        assert_eq!(manga.provides, ["manga.source"]);
        assert_eq!(manga.package, "apk/tachiyomi-all.mangadex-v1.4.190.apk");
        assert_eq!(
            manga.icon.as_deref(),
            Some("icon/eu.kanade.tachiyomi.extension.all.mangadex.png")
        );
        assert_eq!(manga.content_rating, "nsfw");
        assert_eq!(manga.lang.as_deref(), Some("multi"));
        assert_eq!(manga.format, PackageFormat::Mihon);
        assert_eq!(anime.id, "mihon.anime.en.allanime");
        assert_eq!(anime.version, "14.7.0");
        assert_eq!(anime.provides, ["video.source"]);
        assert!(parse(r#"[{"id":"x","site":"y","version":"1","url":"z"}]"#).is_none());
    }

    #[test]
    fn installs_wait_for_the_runtime() {
        let index = parse(
            r#"[{"name":"X","pkg":"eu.kanade.tachiyomi.extension.en.x","apk":"x.apk","sources":[]}]"#,
        )
        .unwrap();
        assert!(!runtime_present());
        assert_eq!(
            package(&index.extensions[0], b"PK").unwrap_err(),
            UNSUPPORTED
        );
    }
}
