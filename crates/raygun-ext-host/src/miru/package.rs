//! Installing a Miru script: checking it against its repository entry and turning it into a
//! Raygun extension folder (`raygun-extension.json` and the script as `index.js`, unchanged).
//! Miru's index carries no hashes; the script must be a Miru extension (its metadata block) for
//! the package and type the index lists, so an error page or another file never installs.

use serde_json::json;

use super::header::{self, Header};
use super::index::{extension_id, listed_package};
use crate::package::{MAX_PACKAGE_BYTES, PackageError, VerifiedPackage, from_files};
use crate::repo::RepoEntry;

/// The script's file in the extension folder.
pub const ENTRY: &str = "index.js";

/// Longest name a manifest takes.
const MAX_NAME_CHARS: usize = 80;

/// Whether the script asks Miru to open its site in a web view (to pass a bot check), which
/// Raygun doesn't do.
pub fn needs_web_view(code: &str) -> bool {
    code.contains("openWebView")
}

/// A downloaded script and its metadata, checked against the entry it was listed under.
pub fn verify_script(bytes: &[u8], entry: &RepoEntry) -> Result<(String, Header), PackageError> {
    if bytes.len() as u64 > MAX_PACKAGE_BYTES {
        return Err(PackageError::TooLarge);
    }
    // Miru reads scripts as UTF-8, replacing what isn't.
    let code = String::from_utf8_lossy(bytes)
        .trim_start_matches('\u{feff}')
        .to_owned();
    let header = header::parse(&code).ok_or_else(|| {
        PackageError::Mismatch("it isn't a Miru extension (no ==MiruExtension== metadata)".into())
    })?;
    let listed = listed_package(entry);
    let same = match listed {
        Some(package) => header.package == package,
        None => extension_id(&header.package).as_deref() == Some(entry.id.as_str()),
    };
    if !same {
        return Err(PackageError::Mismatch(format!(
            "the script is the package {} rather than {}",
            header.package,
            listed.unwrap_or(&entry.name)
        )));
    }
    if !entry
        .provides
        .iter()
        .any(|kind| kind == header.kind.provider_kind())
    {
        return Err(PackageError::Mismatch(format!(
            "the script is a {} source",
            header.kind.noun()
        )));
    }
    if header::semver_version(&header.version) != entry.version {
        // The repository's version is kept, so the catalogue doesn't offer the same update again.
        tracing::warn!(extension = %entry.id, script = %header.version, listed = %entry.version, "the Miru script's version differs from its index");
    }
    Ok((code, header))
}

/// The extension folder for a checked script.
pub fn package(
    entry: &RepoEntry,
    script: String,
    header: &Header,
) -> Result<VerifiedPackage, PackageError> {
    let name: String = header.name.chars().take(MAX_NAME_CHARS).collect();
    let mut description = entry
        .description
        .clone()
        .unwrap_or_else(|| super::index::description(header.description.as_deref(), header.kind));
    if needs_web_view(&script) {
        description.push_str(
            " It opens its site in a web view to pass a bot check, which Raygun doesn't do, so it may fail.",
        );
    }
    let nsfw = header.nsfw || entry.content_rating == "nsfw";
    let website = (header.web_site.starts_with("https://")
        || header.web_site.starts_with("http://"))
    .then(|| header.web_site.clone());
    let manifest = json!({
        "id": entry.id,
        "name": name,
        "version": entry.version,
        "apiVersion": crate::manifest::API_VERSION,
        "runtime": "miru",
        "entry": ENTRY,
        "provides": [header.kind.provider_kind()],
        "permissions": { "net": ["*"] },
        "contentRating": if nsfw { "nsfw" } else { "safe" },
        "description": description,
        "author": header.author,
        "website": website,
    });
    from_files(
        &manifest.to_string(),
        vec![(ENTRY.to_owned(), script.into_bytes())],
    )
}

/// The entry a repository would list for a script found on its own at `url` (an address added in
/// Sources), from its metadata block; `None` when it isn't a Miru extension.
pub fn entry_of(bytes: &[u8], url: &url::Url) -> Option<RepoEntry> {
    if bytes.len() as u64 > MAX_PACKAGE_BYTES {
        return None;
    }
    let code = String::from_utf8_lossy(bytes);
    let header = header::parse(code.trim_start_matches('\u{feff}'))?;
    let id = extension_id(&header.package)?;
    Some(RepoEntry {
        id,
        name: header.name.clone(),
        version: header::semver_version(&header.version),
        api_version: crate::manifest::API_VERSION,
        description: Some(super::index::description(
            header.description.as_deref(),
            header.kind,
        )),
        author: header.author.clone(),
        provides: vec![header.kind.provider_kind().to_owned()],
        permissions: crate::repo::RepoPermissions {
            net: vec!["*".to_owned()],
            webview: false,
        },
        content_rating: if header.nsfw { "nsfw" } else { "safe" }.to_owned(),
        lang: header.lang.clone(),
        package: url.to_string(),
        icon: header.icon.clone(),
        sha256: crate::package::sha256_hex(bytes),
        size: bytes.len() as u64,
        format: crate::repo::PackageFormat::Miru,
        sites: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::parse_index;

    #[test]
    fn a_script_on_its_own_is_listed_from_its_metadata() {
        let url: url::Url = "https://example.org/repo/royalroad.com.js".parse().unwrap();
        let entry = entry_of(SCRIPT.as_bytes(), &url).unwrap();
        assert_eq!(entry.id, "miru.royalroad.com");
        assert_eq!(entry.name, "RoyalRoad");
        assert_eq!(entry.version, "0.0.3");
        assert_eq!(entry.provides, ["novel.source"]);
        let (code, header) = verify_script(SCRIPT.as_bytes(), &entry).unwrap();
        assert_eq!(
            package(&entry, code, &header).unwrap().manifest.id,
            entry.id
        );
        assert!(entry_of(b"export default 1", &url).is_none());
    }

    const SCRIPT: &str = "// ==MiruExtension==\n// @name         RoyalRoad\n// @version      v0.0.3\n// @author       appdevelpo\n// @lang         en\n// @license      MIT\n// @package      royalroad.com\n// @type         fikushon\n// @webSite      https://www.royalroad.com\n// @nsfw         false\n// ==/MiruExtension==\n\nexport default class extends Extension {}\n";

    fn entry() -> RepoEntry {
        let index = serde_json::json!([
            { "author": "appdevelpo", "lang": "en", "license": "MIT", "name": "RoyalRoad", "nsfw": "false",
              "package": "royalroad.com", "type": "fikushon", "url": "royalroad.com.js",
              "version": "v0.0.3", "webSite": "https://www.royalroad.com" },
        ]);
        parse_index(&index.to_string())
            .unwrap()
            .extensions
            .remove(0)
    }

    #[test]
    fn checks_the_script_and_builds_the_folder() {
        let entry = entry();
        let (code, header) = verify_script(SCRIPT.as_bytes(), &entry).unwrap();
        let package = package(&entry, code, &header).unwrap();
        let manifest = &package.manifest;
        assert_eq!(manifest.id, "miru.royalroad.com");
        assert_eq!(manifest.runtime, crate::manifest::Runtime::Miru);
        assert_eq!(manifest.version.to_string(), "0.0.3");
        assert_eq!(manifest.provides, ["novel.source"]);
        assert_eq!(manifest.entry, ENTRY);
        assert_eq!(manifest.author.as_deref(), Some("appdevelpo"));
        assert_eq!(
            manifest.website.as_deref(),
            Some("https://www.royalroad.com")
        );
        assert_eq!(
            manifest
                .net
                .patterns()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["*"]
        );
    }

    #[test]
    fn refuses_what_isnt_the_listed_script() {
        let entry = entry();
        assert!(matches!(
            verify_script(b"<html>Too many requests</html>", &entry),
            Err(PackageError::Mismatch(message)) if message.contains("isn't a Miru extension")
        ));
        let other = SCRIPT.replace("royalroad.com", "evil.example");
        assert!(matches!(
            verify_script(other.as_bytes(), &entry),
            Err(PackageError::Mismatch(message)) if message.contains("evil.example")
        ));
        let manga = SCRIPT.replace("fikushon", "manga");
        assert!(matches!(
            verify_script(manga.as_bytes(), &entry),
            Err(PackageError::Mismatch(message)) if message.contains("comics")
        ));
        let huge = vec![b' '; MAX_PACKAGE_BYTES as usize + 1];
        assert_eq!(
            verify_script(&huge, &entry).unwrap_err(),
            PackageError::TooLarge
        );
    }

    #[test]
    fn web_view_scripts_say_so() {
        let entry = entry();
        let script = SCRIPT.replace(
            "{}",
            "{ async request(url) { await this.openWebView(url); } }",
        );
        let (code, header) = verify_script(script.as_bytes(), &entry).unwrap();
        let package = package(&entry, code, &header).unwrap();
        assert!(
            package
                .manifest
                .description
                .as_deref()
                .unwrap()
                .contains("web view")
        );
    }
}
