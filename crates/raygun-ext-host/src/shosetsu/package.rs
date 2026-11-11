//! Installing a Shosetsu script (M32): checking it and the libraries it needs against the
//! repository index, and turning them into a Raygun extension folder (`raygun-extension.json`,
//! `index.lua`, `lib/<name>.lua`).
//!
//! Shosetsu itself never refuses a script or library over its index hash (it keeps the hash to
//! spot updates), and many community repositories list none, so a file whose hash doesn't match
//! installs when it is what it claims to be: Lua that compiles, a script with its metadata line.

use std::future::Future;

use reqwest::Url;
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
    for name in required(code) {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// The names in `Require("name")` calls.
fn required(code: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
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

/// The file as text, without a leading byte-order mark (which Lua can't read).
fn text(bytes: &[u8]) -> Result<String, PackageError> {
    if bytes.len() as u64 > MAX_PACKAGE_BYTES {
        return Err(PackageError::TooLarge);
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    String::from_utf8(bytes.to_vec())
        .map_err(|_| PackageError::Zip("the script is not UTF-8 text".into()))
}

/// Whether `code` compiles as a Shosetsu script or library (nothing runs).
fn compiles(code: &str) -> Result<(), String> {
    let lua = mlua::Lua::new_with(mlua::StdLib::NONE, mlua::LuaOptions::default())
        .map_err(|error| error.to_string())?;
    lua.load(super::runtime::lua52(code).as_ref())
        .into_function()
        .map(|_| ())
        .map_err(|error| super::runtime::first_line(&error.to_string()))
}

/// A downloaded script, compared with its index entry. Shosetsu's own repository updates scripts
/// without always updating their hash in the index, community repositories often list no hash
/// (or an MD5), and Shosetsu keeps the hash only to spot updates, never refusing a script over
/// it; a mismatch is logged, not refused (M46.9.1). Such a script must still start with a
/// script's metadata line and compile, so an error page never installs.
pub fn verify_script(bytes: &[u8], entry: &RepoEntry) -> Result<String, PackageError> {
    let code = text(bytes)?;
    if !sha256_hex(bytes).eq_ignore_ascii_case(&entry.sha256) {
        if header(&code).is_none_or(|header| header.get("id").is_none()) {
            return Err(PackageError::Checksum);
        }
        compiles(&code).map_err(|error| {
            PackageError::Mismatch(format!("the script isn't Lua that runs: {error}"))
        })?;
        if !entry.sha256.is_empty() {
            tracing::warn!(script = %entry.id, "the script doesn't match its repository index's hash");
        }
    }
    Ok(code)
}

/// The entry a repository would list for a script found on its own at `url` (an address added in
/// Sources, `…/src/<lang>/<File>.lua` as repositories lay them out): the id a repository gives
/// it, its metadata line's version, and the name its table gives (`name = "NovelFull"`), else
/// its file's. `None` when it isn't a Shosetsu script Raygun runs.
pub fn entry_of(bytes: &[u8], url: &Url) -> Option<RepoEntry> {
    if bytes.len() as u64 > MAX_PACKAGE_BYTES {
        return None;
    }
    let code = std::str::from_utf8(bytes).ok()?;
    let header = header(code)?;
    header.get("id")?;
    let version = header.get("ver")?.as_str()?;
    semver::Version::parse(version).ok()?;
    // Scripts for another kotlin-lib than the one Raygun stands in for are left out, as in
    // repositories.
    if let Some(lib) = header.get("libVer").and_then(serde_json::Value::as_str)
        && semver::Version::parse(lib).ok()?.major != 1
    {
        return None;
    }
    let parts: Vec<&str> = url
        .path_segments()?
        .filter(|part| !part.is_empty())
        .collect();
    let file = parts.last()?.strip_suffix(".lua")?;
    let lang = parts
        .iter()
        .rev()
        .nth(1)
        .filter(|lang| {
            (2..=8).contains(&lang.len())
                && lang
                    .bytes()
                    .all(|byte| byte.is_ascii_alphabetic() || byte == b'-')
        })
        .map_or("multi", |lang| lang);
    let id = format!(
        "shosetsu.{}.{}",
        crate::repo::id_label(lang),
        crate::repo::id_label(file)
    );
    if !crate::manifest::is_extension_id(&id) {
        return None;
    }
    let name = own_name(code).unwrap_or_else(|| file.to_owned());
    Some(RepoEntry {
        id,
        description: Some(format!(
            "{name} ({lang}), a Shosetsu novel source. It may reach any website."
        )),
        name,
        version: version.to_owned(),
        api_version: crate::manifest::API_VERSION,
        author: header
            .get("author")
            .and_then(serde_json::Value::as_str)
            .filter(|author| !author.trim().is_empty())
            .map(str::to_owned),
        provides: vec!["novel.source".to_owned()],
        permissions: crate::repo::RepoPermissions {
            net: vec!["*".to_owned()],
            webview: false,
        },
        content_rating: "suggestive".to_owned(),
        lang: Some(lang.to_owned()),
        package: url.to_string(),
        icon: None,
        sha256: sha256_hex(bytes),
        size: bytes.len() as u64,
        format: crate::repo::PackageFormat::Shosetsu,
        sites: Vec::new(),
    })
}

/// The name a script's table gives itself: its first `name = "…"` line.
fn own_name(code: &str) -> Option<String> {
    code.lines().find_map(|line| {
        let rest = line
            .trim()
            .strip_prefix("name")?
            .trim_start()
            .strip_prefix('=')?;
        let quoted = rest.trim().strip_prefix('"')?;
        let name = quoted.split('"').next()?.trim();
        (!name.is_empty()).then(|| name.to_owned())
    })
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

/// A downloaded library, checked against the hash the index lists for it. One the index lists
/// without a hash, or whose file changed since the index was made, installs when it compiles
/// (Shosetsu loads it either way).
pub fn verify_library(name: &str, bytes: &[u8], index: &RepoIndex) -> Result<String, PackageError> {
    let listed = index
        .libraries
        .iter()
        .find(|library| library.name == name)
        .ok_or_else(|| PackageError::Mismatch(format!("library {name} isn't in the repository")))?;
    let code = text(bytes)?;
    if !sha256_hex(bytes).eq_ignore_ascii_case(&listed.hash) {
        // An HTML error page or a cut-off download doesn't compile.
        if code.trim_start().starts_with('<') || compiles(&code).is_err() {
            return Err(PackageError::Checksum);
        }
        if !listed.hash.is_empty() {
            tracing::warn!(library = %name, "the library doesn't match its repository index's hash");
        }
    }
    Ok(code)
}

/// A repository index that can provide libraries, with the address its files are relative to.
pub struct LibrarySource<'a> {
    pub index_url: &'a Url,
    pub index: &'a RepoIndex,
}

/// Builds the extension folder for a downloaded script: the libraries it needs, and theirs, each
/// fetched with `download` from the first source listing it. Shosetsu shares every repository's
/// libraries with every script, so a community script may rely on a library only the official
/// repository has: `sources` is the script's own repository first, then the others. A library a
/// script `Require`s without declaring it, which no repository has, is left out (scripts guard
/// optional ones with `pcall`); a declared one is required.
pub async fn assemble<F, Fut>(
    entry: &RepoEntry,
    script: &[u8],
    sources: &[LibrarySource<'_>],
    mut download: F,
) -> Result<VerifiedPackage, String>
where
    F: FnMut(Url) -> Fut,
    Fut: Future<Output = Result<Vec<u8>, String>>,
{
    let script = verify_script(script, entry).map_err(|error| error.to_string())?;
    let mut needed: Vec<String> = declared(&script);
    let mut optional: Vec<String> = required(&script);
    let mut libraries: Vec<(String, String)> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    loop {
        let (name, declared_dep) = if let Some(name) = needed.pop() {
            (name, true)
        } else if let Some(name) = optional.pop() {
            (name, false)
        } else {
            break;
        };
        if libraries.iter().any(|(known, _)| *known == name) || skipped.contains(&name) {
            continue;
        }
        if libraries.len() >= MAX_LIBRARIES || !is_library_name(&name) {
            return Err(format!("{} needs an unusable library {name}", entry.name));
        }
        let Some(source) = sources.iter().find(|source| {
            source
                .index
                .libraries
                .iter()
                .any(|library| library.name == name)
        }) else {
            if declared_dep {
                return Err(format!(
                    "{} needs the library {name}, which none of the added repositories has",
                    entry.name
                ));
            }
            tracing::debug!(script = %entry.id, library = %name, "an undeclared library isn't in any repository");
            skipped.push(name);
            continue;
        };
        let url = source
            .index_url
            .join(&format!("lib/{name}.lua"))
            .map_err(|error| error.to_string())?;
        let bytes = download(url).await?;
        let code = verify_library(&name, &bytes, source.index)
            .map_err(|error| format!("library {name}: {error}"))?;
        needed.extend(declared(&code));
        optional.extend(required(&code));
        libraries.push((name, code));
    }
    tracing::info!(extension = %entry.id, libraries = libraries.len(), "Shosetsu script fetched");
    package(entry, script, libraries).map_err(|error| error.to_string())
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
        "contentRating": entry.content_rating,
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

    /// Community repositories (noaione's and its forks) list no hashes, and libraries without one.
    #[test]
    fn reads_community_repositories_without_hashes() {
        let index = parse_index(
            &json!({
                "libraries": [
                    { "name": "Madara", "ver": "2.0.0" },
                    { "name": "dkjson", "ver": "1.0.1", "url": null, "hash": null },
                    { "name": "../escape", "ver": "1.0.0" },
                ],
                "scripts": [
                    { "id": 5, "name": "AYA", "fileName": "AYATranslation", "lang": "en", "ver": "1.0.2",
                      "libVer": "1.0.0", "md5": "" },
                    { "id": 6, "name": "Sky", "fileName": "Sky", "lang": "en", "ver": "1.0.0",
                      "libVer": "1.0.0", "md5": "0cc175b9c0f1b6a831c399e269772661" },
                    { "id": 7, "name": "Broken", "lang": "en" },
                ],
            })
            .to_string(),
        )
        .unwrap();
        let ids: Vec<&str> = index
            .extensions
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        assert_eq!(ids, ["shosetsu.en.ayatranslation", "shosetsu.en.sky"]);
        assert!(index.extensions.iter().all(|entry| entry.sha256.is_empty()));
        let libraries: Vec<&str> = index
            .libraries
            .iter()
            .map(|library| library.name.as_str())
            .collect();
        assert_eq!(libraries, ["Madara", "dkjson"], "unsafe names are left out");

        let entry = &index.extensions[0];
        let script = "-- {\"id\":5,\"ver\":\"1.0.2\",\"libVer\":\"1.0.0\"}\nreturn {}\n";
        assert!(verify_script(script.as_bytes(), entry).is_ok());
        let bom = [b"\xEF\xBB\xBF".as_slice(), script.as_bytes()].concat();
        assert_eq!(
            verify_script(&bom, entry).unwrap(),
            script,
            "a byte-order mark is dropped"
        );
        assert!(
            verify_script(b"-- {\"id\":5}\nreturn {", entry).is_err(),
            "cut off: doesn't compile"
        );
        assert!(verify_script(b"<html>404</html>", entry).is_err());
        assert!(verify_library("Madara", b"return { x = 1 }", &index).is_ok());
        assert_eq!(
            verify_library("Madara", b"<!DOCTYPE html><p>Not found</p>", &index).unwrap_err(),
            PackageError::Checksum
        );
    }

    #[tokio::test]
    async fn libraries_come_from_the_other_repositories_when_needed() {
        let community = parse_index(
            &json!({
                "libraries": [],
                "scripts": [{ "id": 8, "name": "Fan", "fileName": "Fan", "lang": "en", "ver": "1.0.0",
                              "libVer": "1.0.0" }],
            })
            .to_string(),
        )
        .unwrap();
        let official = parse_index(
            &json!({
                "libraries": [{ "name": "Madara", "ver": "2.9.2" }, { "name": "dkjson", "ver": "1.0.1" }],
                "scripts": [],
            })
            .to_string(),
        )
        .unwrap();
        let community_url: Url = "https://raw.example/community/index.json".parse().unwrap();
        let official_url: Url = "https://raw.example/official/index.json".parse().unwrap();
        let sources = [
            LibrarySource {
                index_url: &community_url,
                index: &community,
            },
            LibrarySource {
                index_url: &official_url,
                index: &official,
            },
        ];
        let script = "-- {\"id\":8,\"ver\":\"1.0.0\",\"libVer\":\"1.0.0\",\"dep\":[\"Madara>=2.0.0\"]}\n\
                      local ok = pcall(function() return Require(\"optional\") end)\n\
                      return Require(\"Madara\")\n";
        let fetched = std::sync::Mutex::new(Vec::new());
        let package = assemble(
            &community.extensions[0],
            script.as_bytes(),
            &sources,
            |url| {
                fetched.lock().unwrap().push(url.to_string());
                let code = if url.path().ends_with("Madara.lua") {
                    "local json = Require(\"dkjson\")\nreturn {}"
                } else {
                    "return {}"
                };
                async move { Ok(code.as_bytes().to_vec()) }
            },
        )
        .await
        .unwrap();
        assert_eq!(package.manifest.id, "shosetsu.en.fan");
        let mut fetched = fetched.into_inner().unwrap();
        fetched.sort();
        assert_eq!(
            fetched,
            [
                "https://raw.example/official/lib/Madara.lua",
                "https://raw.example/official/lib/dkjson.lua",
            ],
            "an undeclared library no repository has is left out"
        );
        // A declared library nobody has stops the install, saying which.
        let needs = "-- {\"id\":8,\"ver\":\"1.0.0\",\"libVer\":\"1.0.0\",\"dep\":[\"Missing>=1.0.0\"]}\nreturn {}";
        let error = assemble(
            &community.extensions[0],
            needs.as_bytes(),
            &sources,
            |_| async { Ok(Vec::new()) },
        )
        .await
        .unwrap_err();
        assert!(error.contains("needs the library Missing"), "{error}");
    }

    #[test]
    fn repository_pages_and_folders_lead_to_the_index() {
        let candidates = |url: &str| -> Vec<String> {
            crate::repo::shosetsu_index_candidates(&url.parse().unwrap())
                .into_iter()
                .map(String::from)
                .collect()
        };
        assert_eq!(
            candidates("https://raw.githubusercontent.com/noaione/shosetsu-extensions/dev"),
            ["https://raw.githubusercontent.com/noaione/shosetsu-extensions/dev/index.json"]
        );
        assert_eq!(
            candidates("https://gitlab.com/shosetsuorg/extensions/-/raw/dev/"),
            ["https://gitlab.com/shosetsuorg/extensions/-/raw/dev/index.json"]
        );
        assert_eq!(
            candidates("https://github.com/jobobby04/ShosetsuExtensions/tree/master"),
            ["https://raw.githubusercontent.com/jobobby04/ShosetsuExtensions/master/index.json"]
        );
        assert_eq!(
            candidates("https://github.com/someone/novels").len(),
            3,
            "main, master and dev"
        );
        assert_eq!(
            candidates("https://gitlab.com/group/sub/novels/-/tree/dev/repo"),
            ["https://gitlab.com/group/sub/novels/-/raw/dev/repo/index.json"]
        );
        assert!(candidates("https://example.org/repo/index.json").is_empty());
    }

    #[test]
    fn checks_scripts_and_libraries_and_builds_the_folder() {
        let index = index();
        let entry = &index.extensions[0];
        assert_eq!(dependencies(SCRIPT), ["novelvault", "url"]);
        assert_eq!(dependencies(LIBRARY), ["unhtml"]);
        assert_eq!(dependencies("return {}"), Vec::<String>::new());

        let script = verify_script(SCRIPT.as_bytes(), entry).unwrap();
        // A script newer than its index entry installs, as in Shosetsu; libraries stay checked.
        let updated = SCRIPT.replace("return {}", "return { updated = true }");
        assert_eq!(verify_script(updated.as_bytes(), entry).unwrap(), updated);
        // Anything else with the wrong hash (an error page) doesn't.
        assert_eq!(
            verify_script(b"Too many requests", entry).unwrap_err(),
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

    #[test]
    fn a_script_on_its_own_gets_the_id_its_repository_gives() {
        let url: Url = "https://example.org/extensions/src/en/NovelFull.lua"
            .parse()
            .unwrap();
        let entry = entry_of(SCRIPT.as_bytes(), &url).unwrap();
        assert_eq!(entry.id, index().extensions[0].id);
        assert_eq!(entry.name, "NovelFull");
        assert_eq!(entry.version, "3.0.1");
        assert_eq!(entry.lang.as_deref(), Some("en"));
        assert_eq!(entry.package, url.as_str());
        // Its own checksum, so it checks out as listed.
        assert_eq!(verify_script(SCRIPT.as_bytes(), &entry).unwrap(), SCRIPT);

        let named = format!("{SCRIPT}local x = 1\n  name = \"Novel Full\",\n");
        let elsewhere: Url = "https://example.org/NovelFull.lua".parse().unwrap();
        let entry = entry_of(named.as_bytes(), &elsewhere).unwrap();
        assert_eq!(entry.name, "Novel Full");
        assert_eq!(entry.id, "shosetsu.multi.novelfull");
        assert!(entry_of(b"return {}", &url).is_none(), "no metadata line");
        assert!(entry_of(SCRIPT.as_bytes(), &"https://x.org/a.js".parse().unwrap()).is_none());
        let future = SCRIPT.replace("\"libVer\":\"1.0.0\"", "\"libVer\":\"2.0.0\"");
        assert!(
            entry_of(future.as_bytes(), &url).is_none(),
            "another kotlin-lib"
        );
    }
}
