//! Installing an LNReader plugin: checking the downloaded script is the plugin its repository
//! lists, and turning it into a Raygun extension folder (`raygun-extension.json` and the script as
//! `index.js`, unchanged). LNReader's index carries no hashes, so the script must be a compiled
//! plugin (it exports one) naming the listed id: an error page or another file never installs.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::json;

use super::index::listed_plugin;
use crate::package::{MAX_PACKAGE_BYTES, PackageError, VerifiedPackage, from_files};
use crate::repo::RepoEntry;

/// The script's file in the extension folder.
pub const ENTRY: &str = "index.js";

/// The `site` a compiled plugin sets (`this.site="https://…"`, or `site: "https://…"`).
static SITE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\bsite\s*[=:]\s*["'](https?://[^"'\s]+)["']"#).expect("valid regex")
});

/// A downloaded plugin, checked against the entry it was listed under.
pub fn verify_script(bytes: &[u8], entry: &RepoEntry) -> Result<String, PackageError> {
    if bytes.len() as u64 > MAX_PACKAGE_BYTES {
        return Err(PackageError::TooLarge);
    }
    let code = String::from_utf8_lossy(bytes)
        .trim_start_matches('\u{feff}')
        .to_owned();
    if !code.contains("exports") || code.trim_start().starts_with('<') {
        return Err(PackageError::Mismatch(
            "it isn't an LNReader plugin (it exports nothing)".into(),
        ));
    }
    if let Some(plugin) = listed_plugin(entry) {
        let named = [
            format!("\"{plugin}\""),
            format!("'{plugin}'"),
            format!("`{plugin}`"),
        ];
        if !named.iter().any(|quoted| code.contains(quoted.as_str())) {
            return Err(PackageError::Mismatch(format!(
                "the script doesn't name the plugin {plugin}"
            )));
        }
    }
    Ok(code)
}

/// The extension folder for a checked plugin.
pub fn package(entry: &RepoEntry, script: String) -> Result<VerifiedPackage, PackageError> {
    let website = SITE.captures(&script).map(|found| found[1].to_owned());
    let manifest = json!({
        "id": entry.id,
        "name": entry.name.chars().take(80).collect::<String>(),
        "version": entry.version,
        "apiVersion": crate::manifest::API_VERSION,
        "runtime": "lnreader",
        "entry": ENTRY,
        "provides": ["novel.source"],
        "permissions": { "net": ["*"] },
        "contentRating": entry.content_rating,
        "description": entry.description,
        "website": website,
    });
    from_files(
        &manifest.to_string(),
        vec![(ENTRY.to_owned(), script.into_bytes())],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::parse_index;

    fn entry() -> RepoEntry {
        let index = serde_json::json!([
            { "id": "examplenovels", "name": "Example Novels", "site": "https://novels.example.org/",
              "lang": "English", "version": "1.0.2", "url": "https://plugins.example.org/examplenovels.js" },
        ]);
        parse_index(&index.to_string())
            .unwrap()
            .extensions
            .remove(0)
    }

    const SCRIPT: &str = r#"Object.defineProperty(exports,"__esModule",{value:!0});var p=function(){function e(){this.id="examplenovels",this.name="Example Novels",this.site="https://novels.example.org/",this.version="1.0.2"}return e}();exports.default=new p;"#;

    #[test]
    fn checks_the_script_and_builds_the_folder() {
        let entry = entry();
        let code = verify_script(SCRIPT.as_bytes(), &entry).unwrap();
        let package = package(&entry, code).unwrap();
        let manifest = &package.manifest;
        assert_eq!(manifest.id, "lnreader.examplenovels");
        assert_eq!(manifest.runtime, crate::manifest::Runtime::LnReader);
        assert_eq!(manifest.version.to_string(), "1.0.2");
        assert_eq!(manifest.provides, ["novel.source"]);
        assert_eq!(
            manifest.website.as_deref(),
            Some("https://novels.example.org/")
        );
    }

    #[test]
    fn refuses_what_isnt_the_listed_plugin() {
        let entry = entry();
        assert!(matches!(
            verify_script(b"<html>Too many requests</html>", &entry),
            Err(PackageError::Mismatch(message)) if message.contains("isn't an LNReader plugin")
        ));
        let other = SCRIPT.replace("examplenovels", "otherplugin");
        assert!(matches!(
            verify_script(other.as_bytes(), &entry),
            Err(PackageError::Mismatch(message)) if message.contains("examplenovels")
        ));
    }
}
