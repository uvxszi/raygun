//! Installing a Shosetsu script (M32): checking it and the libraries it needs against the
//! repository index, and turning them into a Raygun extension folder (`raygun-extension.json`,
//! `index.lua`, `lib/<name>.lua`).

use serde_json::json;

use crate::package::{MAX_PACKAGE_BYTES, PackageError, VerifiedPackage, from_files, sha256_hex};
use crate::repo::{RepoEntry, RepoIndex};

/// Most libraries one script may pull in, dependencies of dependencies included.
pub const MAX_LIBRARIES: usize = 32;

/// The JSON metadata on a script's or library's first comment line:
/// `-- {"id":1,"ver":"3.0.1","libVer":"1.0.0","dep":["novelvault>=1.0.0"]}`.
fn header(code: &str) -> Option<serde_json::Value> {
    let line = code.lines().find(|line| !line.trim().is_empty())?;
    let json = line.trim().strip_prefix("--")?.trim();
    serde_json::from_str(json).ok()
}

/// Library names a script or library depends on: its header's `dep` list
/// (`"novelvault>=1.0.0"` → `novelvault`) and any `Require("name")` it makes without declaring.
pub fn dependencies(code: &str) -> Vec<String> {
    let mut names = declared(code);
    for (index, _) in code.match_indices("Require(") {
        let rest = code[index + "Require(".len()..].trim_start();
        let Some(quote) = rest.chars().next().filter(|c| matches!(c, '"' | '\'')) else {
            continue;
        };
        if let Some(end) = rest[1..].find(quote) {
            let name = rest[1..1 + end].to_owned();
            if is_library_name(&name) && !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names
}

fn declared(code: &str) -> Vec<String> {
    header(code)
        .and_then(|header| header.get("dep").cloned())
        .and_then(|deps| deps.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|dep| {
            let dep = dep.as_str()?;
            let name: String = dep
                .chars()
                .take_while(|c| !matches!(c, '<' | '>' | '=' | '!' | ' '))
                .collect();
            (!name.is_empty()).then_some(name)
        })
        .collect()
}

fn text(bytes: &[u8]) -> Result<String, PackageError> {
    if bytes.len() as u64 > MAX_PACKAGE_BYTES {
        return Err(PackageError::TooLarge);
    }
    String::from_utf8(bytes.to_vec())
        .map_err(|_| PackageError::Zip("the script is not UTF-8 text".into()))
}

/// A downloaded script, checked against its index entry.
pub fn verify_script(bytes: &[u8], entry: &RepoEntry) -> Result<String, PackageError> {
    let code = text(bytes)?;
    if !sha256_hex(bytes).eq_ignore_ascii_case(&entry.sha256) {
        return Err(PackageError::Checksum);
    }
    Ok(code)
}

/// A library name that maps to `lib/<name>.lua` and nothing else.
pub fn is_library_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

/// A downloaded library, checked against the hash the index lists for it.
pub fn verify_library(name: &str, bytes: &[u8], index: &RepoIndex) -> Result<String, PackageError> {
    let listed = index
        .libraries
        .iter()
        .find(|library| library.name == name)
        .ok_or_else(|| PackageError::Mismatch(format!("library {name} isn't in the repository")))?;
    let code = text(bytes)?;
    if !sha256_hex(bytes).eq_ignore_ascii_case(&listed.hash) {
        return Err(PackageError::Checksum);
    }
    Ok(code)
}

/// The extension folder for a script and its libraries.
pub fn package(
    entry: &RepoEntry,
    script: String,
    libraries: Vec<(String, String)>,
) -> Result<VerifiedPackage, PackageError> {
    let manifest = json!({
        "id": entry.id,
        "name": entry.name,
        "version": entry.version,
        "apiVersion": crate::manifest::API_VERSION,
        "runtime": "shosetsu",
        "entry": "index.lua",
        "provides": ["novel.source"],
        "permissions": { "net": ["*"] },
        "contentRating": "suggestive",
        "description": entry.description,
        "author": header(&script)
            .and_then(|header| header.get("author").and_then(|author| author.as_str().map(str::to_owned))),
    });
    let mut files = vec![("index.lua".to_owned(), script.into_bytes())];
    for (name, code) in libraries {
        if !is_library_name(&name) {
            return Err(PackageError::UnsafePath(name));
        }
        files.push((format!("lib/{name}.lua"), code.into_bytes()));
    }
    from_files(&manifest.to_string(), files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::parse_index;

    const SCRIPT: &str = "-- {\"id\":1,\"ver\":\"3.0.1\",\"libVer\":\"1.0.0\",\"author\":\"Doomsdayrs\",\"dep\":[\"novelvault>=1.0.0\", \"url\"]}\nreturn {}\n";
    const LIBRARY: &str = "-- {\"ver\":\"1.0.0\",\"dep\":[\"unhtml>=1.0.0\"]}\nreturn {}\n";

    fn index() -> RepoIndex {
        parse_index(
            &json!({
                "libraries": [
                    { "name": "novelvault", "ver": "1.0.11", "hash": sha256_hex(LIBRARY.as_bytes()) },
                ],
                "scripts": [
                    { "id": 1, "name": "NovelFull", "fileName": "NovelFull", "lang": "en", "ver": "3.0.1",
                      "libVer": "1.0.0", "md5": sha256_hex(SCRIPT.as_bytes()), "type": "LuaScript",
                      "imageURL": "https://example.org/icons/NovelFull.png" },
                    { "id": 2, "name": "Future", "fileName": "Future", "lang": "en", "ver": "1.0.0",
                      "libVer": "2.0.0", "md5": sha256_hex(b"x"), "type": "LuaScript" },
                    { "id": 3, "name": "Js", "fileName": "Js", "lang": "en", "ver": "1.0.0",
                      "libVer": "1.0.0", "md5": sha256_hex(b"x"), "type": "JS" },
                ],
                "authors": [], "styles": [], "js": [],
            })
            .to_string(),
        )
        .unwrap()
    }

    #[test]
    fn reads_a_shosetsu_index() {
        let index = index();
        assert_eq!(
            index.extensions.len(),
            1,
            "other lib majors and JS scripts are left out"
        );
        let entry = &index.extensions[0];
        assert_eq!(entry.id, "shosetsu.en.novelfull");
        assert_eq!(entry.package, "src/en/NovelFull.lua");
        assert_eq!(entry.permissions.net, ["*"]);
        assert_eq!(entry.provides, ["novel.source"]);
        assert_eq!(entry.format, crate::repo::PackageFormat::Shosetsu);
        assert_eq!(index.libraries[0].name, "novelvault");
        // The icon its repository lists (M46.2), resolved like packages.
        let base: url::Url = "https://example.org/repo/index.json".parse().unwrap();
        assert_eq!(
            crate::repo::icon_url(&base, entry)
                .map(String::from)
                .as_deref(),
            Some("https://example.org/icons/NovelFull.png")
        );
        let relative = crate::repo::RepoEntry {
            icon: Some("icons/x.png".into()),
            ..entry.clone()
        };
        assert_eq!(
            crate::repo::icon_url(&base, &relative)
                .map(String::from)
                .as_deref(),
            Some("https://example.org/repo/icons/x.png")
        );
        let unsafe_icon = crate::repo::RepoEntry {
            icon: Some("javascript:alert(1)".into()),
            ..entry.clone()
        };
        assert!(crate::repo::icon_url(&base, &unsafe_icon).is_none());
    }

    #[test]
    fn checks_scripts_and_libraries_and_builds_the_folder() {
        let index = index();
        let entry = &index.extensions[0];
        assert_eq!(dependencies(SCRIPT), ["novelvault", "url"]);
        assert_eq!(dependencies(LIBRARY), ["unhtml"]);
        assert_eq!(dependencies("return {}"), Vec::<String>::new());

        let script = verify_script(SCRIPT.as_bytes(), entry).unwrap();
        assert_eq!(
            verify_script(b"tampered", entry).unwrap_err(),
            PackageError::Checksum
        );
        let library = verify_library("novelvault", LIBRARY.as_bytes(), &index).unwrap();
        assert_eq!(
            verify_library("novelvault", b"tampered", &index).unwrap_err(),
            PackageError::Checksum
        );
        assert!(verify_library("url", b"x", &index).is_err(), "not listed");

        let package = package(entry, script, vec![("novelvault".into(), library)]).unwrap();
        assert_eq!(package.manifest.id, "shosetsu.en.novelfull");
        assert_eq!(package.manifest.runtime, crate::manifest::Runtime::Shosetsu);
        assert_eq!(package.manifest.author.as_deref(), Some("Doomsdayrs"));
        assert!(
            super::package(entry, SCRIPT.into(), vec![("../x".into(), String::new())]).is_err()
        );
        assert!(!is_library_name("../etc"));
    }
}
