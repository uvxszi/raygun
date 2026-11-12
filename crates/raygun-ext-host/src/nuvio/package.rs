//! Installing a Nuvio provider: checking the downloaded file is a provider (a script with a
//! `getStreams` to call) and turning it into a Raygun extension folder (`raygun-extension.json`
//! and the script as `index.js`, unchanged). Nuvio's manifests carry no hashes, so the file must
//! read as a provider: an error page or another file never installs.

use serde_json::json;

use super::index::listed;
use crate::package::{MAX_PACKAGE_BYTES, PackageError, VerifiedPackage, from_files};
use crate::repo::RepoEntry;

/// The script's file in the extension folder.
pub const ENTRY: &str = "index.js";

/// Why a provider without a `getStreams` isn't for Raygun: Nuvio calls nothing else.
pub const NO_GET_STREAMS: &str =
    "It defines no getStreams function, the only thing Nuvio and Raygun call";

/// The setting a provider's `TMDB_API_KEY` comes from.
pub const TMDB_KEY_SETTING: &str = "tmdbApiKey";

/// A downloaded provider, checked: the script's text, or why it can't be installed (an error
/// page, a file too large, or a script with nothing to call, which isn't for Raygun).
pub fn verify_script(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() as u64 > MAX_PACKAGE_BYTES {
        return Err(PackageError::TooLarge.to_string());
    }
    let code = String::from_utf8_lossy(bytes)
        .trim_start_matches('\u{feff}')
        .to_owned();
    let start = code.trim_start();
    let page = start.starts_with('<');
    let data = start.starts_with('{') && serde_json::from_str::<serde_json::Value>(&code).is_ok();
    if page || data {
        return Err(
            PackageError::Mismatch("it isn't a Nuvio provider's script".into()).to_string(),
        );
    }
    if !code.contains("getStreams") {
        return Err(crate::purpose::message(NO_GET_STREAMS));
    }
    Ok(code)
}

/// The extension folder for a checked provider.
pub fn package(entry: &RepoEntry, script: String) -> Result<VerifiedPackage, PackageError> {
    let listed = listed(entry).ok_or_else(|| {
        PackageError::Mismatch("the repository entry doesn't name its provider".into())
    })?;
    let types = if listed.types.is_empty() {
        vec!["movie".to_owned(), "tv".to_owned()]
    } else {
        listed.types
    };
    let manifest = json!({
        "id": entry.id,
        "name": entry.name.chars().take(80).collect::<String>(),
        "version": entry.version,
        "apiVersion": crate::manifest::API_VERSION,
        "runtime": "nuvio",
        "entry": ENTRY,
        "provides": [crate::video::VIDEO_STREAMS],
        "permissions": { "net": ["*"] },
        "contentRating": entry.content_rating,
        "description": entry.description,
        "author": entry.author,
        "settings": [{
            "key": TMDB_KEY_SETTING,
            "type": "text",
            "label": "TMDB API key (for providers that look titles up on TMDB)",
            "default": "",
        }],
        "nuvio": { "scraper": listed.scraper, "types": types },
    });
    from_files(
        &manifest.to_string(),
        vec![(ENTRY.to_owned(), script.into_bytes())],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> RepoEntry {
        let manifest = serde_json::json!({
            "name": "Public Domain Films", "version": "1.0.0",
            "scrapers": [{ "id": "archive", "name": "Internet Archive", "version": "1.0.0",
              "filename": "archive.js", "supportedTypes": ["movie"] }],
        });
        crate::repo::parse_index(&manifest.to_string())
            .unwrap()
            .extensions
            .remove(0)
    }

    #[test]
    fn checks_the_script_and_builds_the_folder() {
        let entry = entry();
        let code = verify_script(b"module.exports = { getStreams: async () => [] };").unwrap();
        let package = package(&entry, code).unwrap();
        let manifest = &package.manifest;
        assert_eq!(manifest.id, "nuvio.archive");
        assert_eq!(manifest.runtime, crate::manifest::Runtime::Nuvio);
        assert_eq!(manifest.provides, ["video.streams"]);
        assert_eq!(manifest.settings[0].key, TMDB_KEY_SETTING);
        assert_eq!(
            manifest.nuvio,
            Some(serde_json::json!({ "scraper": "archive", "types": ["movie"] }))
        );
    }

    #[test]
    fn refuses_what_isnt_a_provider() {
        assert!(
            verify_script(b"<html>Too many requests</html>")
                .unwrap_err()
                .contains("isn't a Nuvio provider")
        );
        assert!(
            verify_script(br#"{"name":"A manifest","scrapers":[]}"#)
                .unwrap_err()
                .contains("isn't a Nuvio provider")
        );
        assert_eq!(
            verify_script(b"module.exports = { search: async () => [] };").unwrap_err(),
            crate::purpose::message(NO_GET_STREAMS)
        );
    }
}
