//! Repositories and packages (M13).

use std::io::Write;

use raygun_model::SettingValue;
use serde_json::json;

use crate::package::{self, PackageError};
use crate::repo::{self, RepoEntry, RepoError, RepoPermissions};
use crate::{ExtensionHost, HostConfig, UserSettings, discover_installed};

const MANIFEST: &str = r#"{"id":"dev.example.pkg","name":"Pkg","version":"1.2.0","apiVersion":1,
    "runtime":"js","entry":"dist/index.js","provides":["music.metadata"],
    "permissions":{"net":["api.example.org","*.cdn.example.org"]},
    "settings":[{"key":"region","type":"select","options":["eu","us"],"default":"eu"},
                {"key":"limit","type":"number","default":5}]}"#;
const CODE: &str = "raygun.providers.register({ kind: 'music.metadata', id: 'p', run: async () => [raygun.settings.get('region'), raygun.settings.get('limit')] });";

fn zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, contents) in files {
        writer.start_file(*name, options).unwrap();
        writer.write_all(contents).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn entry_for(bytes: &[u8]) -> RepoEntry {
    RepoEntry {
        id: "dev.example.pkg".into(),
        name: "Pkg".into(),
        version: "1.2.0".into(),
        api_version: 1,
        description: None,
        author: None,
        provides: vec!["music.metadata".into()],
        permissions: RepoPermissions {
            // Order and case don't matter.
            net: vec!["*.cdn.example.org".into(), "API.example.org".into()],
            webview: false,
        },
        content_rating: "safe".into(),
        lang: None,
        package: "packages/dev.example.pkg-1.2.0.zip".into(),
        icon: None,
        sha256: package::sha256_hex(bytes),
        size: bytes.len() as u64,
        format: Default::default(),
    }
}

fn good_package() -> Vec<u8> {
    zip(&[
        ("raygun-extension.json", MANIFEST.as_bytes()),
        ("dist/index.js", CODE.as_bytes()),
    ])
}

#[test]
fn index_urls_must_be_https_or_local() {
    assert!(repo::check_index_url("https://example.org/repo/index.json").is_ok());
    assert!(repo::check_index_url("http://127.0.0.1:8765/index.json").is_ok());
    assert!(repo::check_index_url("http://localhost/index.json").is_ok());
    assert!(repo::check_index_url("http://10.0.2.2:8765/index.json").is_ok());
    assert!(matches!(
        repo::check_index_url("http://example.org/index.json"),
        Err(RepoError::Url(message)) if message.contains("https")
    ));
    // Local files are repositories on desktop only (M46.28).
    assert_eq!(
        repo::check_index_url("file:///index.json").is_ok(),
        cfg!(not(target_os = "android"))
    );
    assert!(repo::check_index_url("ftp://example.org/index.json").is_err());
    assert!(repo::check_index_url("not a url").is_err());
}

#[test]
fn parses_indexes_and_skips_unusable_entries() {
    let bytes = good_package();
    let good = serde_json::to_value(entry_for(&bytes)).unwrap();
    let mut future_api = good.clone();
    future_api["apiVersion"] = json!(2);
    future_api["id"] = json!("dev.example.future");
    let mut bad_hash = good.clone();
    bad_hash["sha256"] = json!("abc");
    let index = json!({ "formatVersion": 1, "name": " Test Repo ", "extensions": [good, future_api, bad_hash] });

    let parsed = repo::parse_index(&index.to_string()).unwrap();
    assert_eq!(parsed.name, "Test Repo");
    assert_eq!(parsed.extensions.len(), 1);
    assert_eq!(parsed.extensions[0].id, "dev.example.pkg");

    assert_eq!(
        repo::parse_index(r#"{"formatVersion":2,"name":"x","extensions":[]}"#),
        Err(RepoError::Format(2))
    );
    assert!(matches!(repo::parse_index("[]"), Err(RepoError::Index(_))));

    let base = "https://example.org/repo/index.json".parse().unwrap();
    assert_eq!(
        repo::package_url(&base, &parsed.extensions[0])
            .unwrap()
            .as_str(),
        "https://example.org/repo/packages/dev.example.pkg-1.2.0.zip"
    );
    assert_eq!(
        repo::compare_versions("1.10.0", "1.9.0"),
        std::cmp::Ordering::Greater
    );
    assert_eq!(
        repo::compare_versions("1.0.0-beta", "1.0.0"),
        std::cmp::Ordering::Less
    );
}

/// One Raygun index can list every format (as Paraline does): Raygun packages by absolute address
/// (another repository's) or beside the index, Nuclear plugins, and Shosetsu scripts with the
/// libraries they need.
#[test]
fn one_index_holds_every_format() {
    let bytes = good_package();
    let mut raygun_entry = serde_json::to_value(entry_for(&bytes)).unwrap();
    raygun_entry["package"] =
        json!("https://raw.githubusercontent.com/uvxszi/raygun/main/repo/packages/a-1.2.0.zip");
    let script = json!({
        "id": "shosetsu.ja.syosetsu", "name": "Syosetsu", "version": "2.2.6", "apiVersion": 1,
        "author": "Bigrand", "provides": [], "permissions": { "net": [] },
        "contentRating": "suggestive", "lang": "ja", "format": "shosetsu",
        "package": "shosetsu/src/ja/Syosetsu.lua", "icon": "icons/Syosetsu.png",
        "sha256": "ab".repeat(32), "size": 120,
    });
    let plugin = json!({
        "id": "nuclear.nuclear-plugin-lrclib", "name": "LRCLIB", "version": "0.1.0", "apiVersion": 1,
        "provides": ["lyrics"], "format": "nuclear",
        "package": "https://example.org/plugin.zip", "sha256": "cd".repeat(32), "size": 0,
    });
    let index = json!({
        "formatVersion": 1, "name": "Paraline",
        "extensions": [raygun_entry, script, plugin],
        "libraries": [{ "name": "url", "ver": "1.0.0", "hash": "ef".repeat(32) }],
    });
    let parsed = repo::parse_index(&index.to_string()).unwrap();
    assert_eq!(parsed.name, "Paraline");
    let formats: Vec<_> = parsed.extensions.iter().map(|entry| entry.format).collect();
    assert_eq!(
        formats,
        [
            repo::PackageFormat::Raygun,
            repo::PackageFormat::Shosetsu,
            repo::PackageFormat::Nuclear
        ]
    );
    assert_eq!(parsed.libraries[0].name, "url");

    // The script's entry says what installing it gives, whatever the index listed.
    let novel = &parsed.extensions[1];
    assert_eq!(novel.provides, ["novel.source"]);
    assert_eq!(novel.permissions.net, ["*"]);
    assert_eq!(novel.lang.as_deref(), Some("ja"));

    let base: reqwest::Url =
        "https://raw.githubusercontent.com/uvxszi/paraline-raygun/main/index.json"
            .parse()
            .unwrap();
    let url = |entry: &RepoEntry| repo::package_url(&base, entry).unwrap().to_string();
    assert_eq!(
        url(&parsed.extensions[0]),
        "https://raw.githubusercontent.com/uvxszi/raygun/main/repo/packages/a-1.2.0.zip"
    );
    assert_eq!(
        url(novel),
        "https://raw.githubusercontent.com/uvxszi/paraline-raygun/main/shosetsu/src/ja/Syosetsu.lua"
    );
    assert_eq!(
        repo::icon_url(&base, novel).unwrap().as_str(),
        "https://raw.githubusercontent.com/uvxszi/paraline-raygun/main/icons/Syosetsu.png"
    );
}

/// `cargo test -p raygun-ext-host -- --ignored paraline_live --nocapture`: every package in
/// Paraline's index (`PARALINE_INDEX`: another address, or the `index.json` of a checkout)
/// downloads, verifies and installs as the app does it, and loads.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs network access to GitHub"]
async fn paraline_live() {
    use crate::shosetsu::package as shosetsu_package;
    let index_url = match std::env::var("PARALINE_INDEX") {
        Ok(place) if std::path::Path::new(&place).is_absolute() => {
            reqwest::Url::from_file_path(&place).unwrap()
        }
        Ok(place) => place.parse().unwrap(),
        Err(_) => "https://raw.githubusercontent.com/uvxszi/paraline-raygun/main/index.json"
            .parse()
            .unwrap(),
    };
    let client = raygun_net::http_client().unwrap();
    let download = |url: reqwest::Url| {
        let client = client.clone();
        async move {
            if url.scheme() == "file" {
                return std::fs::read(url.to_file_path().unwrap())
                    .unwrap_or_else(|error| panic!("{url}: {error}"));
            }
            let response = client.get(url.clone()).send().await.unwrap();
            assert!(
                response.status().is_success(),
                "{url}: {}",
                response.status()
            );
            response.bytes().await.unwrap().to_vec()
        }
    };
    let index =
        repo::parse_index(&String::from_utf8(download(index_url.clone()).await).unwrap()).unwrap();
    assert_eq!(index.name, "Paraline");
    let formats = |format| {
        index
            .extensions
            .iter()
            .filter(|entry| entry.format == format)
            .count()
    };
    assert!(formats(repo::PackageFormat::Raygun) > 10);
    assert!(formats(repo::PackageFormat::Shosetsu) > 0);

    let temp = tempfile::TempDir::new().unwrap();
    for entry in &index.extensions {
        let bytes = download(repo::package_url(&index_url, entry).unwrap()).await;
        let verified = match entry.format {
            repo::PackageFormat::Raygun => package::verify(&bytes, entry)
                .unwrap_or_else(|error| panic!("{}: {error}", entry.id)),
            repo::PackageFormat::Shosetsu => {
                let script = shosetsu_package::verify_script(&bytes, entry).unwrap();
                let mut queue = shosetsu_package::dependencies(&script);
                let mut libraries: Vec<(String, String)> = Vec::new();
                while let Some(name) = queue.pop() {
                    if libraries.iter().any(|(known, _)| *known == name) {
                        continue;
                    }
                    let bytes = download(index_url.join(&format!("lib/{name}.lua")).unwrap()).await;
                    let code = shosetsu_package::verify_library(&name, &bytes, &index)
                        .unwrap_or_else(|error| panic!("{}: library {name}: {error}", entry.id));
                    queue.extend(shosetsu_package::dependencies(&code));
                    libraries.push((name, code));
                }
                shosetsu_package::package(entry, script, libraries).unwrap()
            }
            repo::PackageFormat::Miru => {
                let (script, header) = crate::miru::package::verify_script(&bytes, entry).unwrap();
                crate::miru::package::package(entry, script, &header).unwrap()
            }
            repo::PackageFormat::Nuclear | repo::PackageFormat::Kodi => continue,
        };
        assert_eq!(verified.manifest.id, entry.id);
        assert_eq!(verified.manifest.version.to_string(), entry.version);
        package::install(temp.path(), &verified).unwrap();
        eprintln!("{} {} ({:?})", entry.id, entry.version, entry.format);
    }
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    for info in host.load(discover_installed(temp.path())).await {
        assert_eq!(
            info.status,
            raygun_model::ExtensionStatus::Loaded,
            "{}: {:?}",
            info.id,
            info.error
        );
    }
}

#[test]
fn shosetsu_indexes_are_named_after_their_address() {
    let index = json!({ "libraries": [], "scripts": [] });
    assert_eq!(repo::parse_index(&index.to_string()).unwrap().name, "");

    let name = |url: &str| repo::default_name(&url.parse().unwrap());
    // Only Shosetsu's own repository reads as Shosetsu's, from any of its addresses.
    for official in [
        "https://gitlab.com/shosetsuorg/extensions/-/raw/dev/index.json",
        "https://shosetsuorg.gitlab.io/extensions/index.json",
        "https://raw.githubusercontent.com/shosetsuorg/extensions/dev/index.json",
    ] {
        assert_eq!(name(official), repo::SHOSETSU_REPO_NAME, "{official}");
    }
    assert_eq!(
        name("https://raw.githubusercontent.com/someone/novel-sources/main/shosetsu/index.json"),
        "novel-sources"
    );
    assert_eq!(
        name("https://cdn.jsdelivr.net/gh/someone/novel-sources@main/index.json"),
        "novel-sources"
    );
    assert_eq!(
        name("https://gitlab.com/group/sub/novels/-/raw/main/index.json"),
        "novels"
    );
    assert_eq!(
        name("https://someone.github.io/novels/index.json"),
        "novels"
    );
    assert_eq!(
        name("https://someone.github.io/index.json"),
        "someone.github.io"
    );
    assert_eq!(
        name("https://Example.org/shosetsu/index.json"),
        "example.org"
    );
    assert_eq!(name("http://10.0.2.2:8765/index.json"), "10.0.2.2:8765");
    #[cfg(not(target_os = "android"))]
    {
        let folder = tempfile::TempDir::new().unwrap();
        let index = folder.path().join("my-sources").join("index.json");
        assert_eq!(
            repo::default_name(&reqwest::Url::from_file_path(&index).unwrap()),
            "my-sources"
        );
    }
}

#[test]
fn other_apps_repositories_are_named() {
    let mihon = r#"[{"name":"Tachiyomi: X","pkg":"eu.kanade.x","apk":"x.apk","lang":"en","code":1,"version":"1.4.1","nsfw":0,"sources":[]}]"#;
    let cloudstream = r#"{"name":"Repo","description":"","manifestVersion":1,"pluginLists":["https://example.com/plugins.json"]}"#;
    let lnreader = r#"[{"id":"x","name":"X","site":"https://example.com","lang":"English","version":"1.0.0","url":"https://example.com/x.js","iconUrl":"https://example.com/x.png"}]"#;
    for (json, word) in [
        (mihon, "Mihon"),
        (cloudstream, "CloudStream"),
        (lnreader, "LNReader"),
    ] {
        match repo::parse_index(json) {
            Err(RepoError::Foreign(message)) => assert!(message.contains(word), "{message}"),
            other => panic!("{word}: {other:?}"),
        }
    }
    assert!(matches!(repo::parse_index("{}"), Err(RepoError::Index(_))));
}

#[test]
fn verifies_packages_against_their_entry() {
    let bytes = good_package();
    let entry = entry_for(&bytes);
    let verified = package::verify(&bytes, &entry).unwrap();
    assert_eq!(verified.manifest.id, "dev.example.pkg");

    let mut tampered = bytes.clone();
    *tampered.last_mut().unwrap() ^= 1;
    assert_eq!(
        package::verify(&tampered, &entry).unwrap_err(),
        PackageError::Checksum
    );

    let check = |files: &[(&str, &[u8])], adjust: &dyn Fn(&mut RepoEntry)| {
        let bytes = zip(files);
        let mut entry = entry_for(&bytes);
        adjust(&mut entry);
        package::verify(&bytes, &entry).unwrap_err()
    };
    let manifest = ("raygun-extension.json", MANIFEST.as_bytes());
    let code = ("dist/index.js", CODE.as_bytes());

    assert_eq!(check(&[code], &|_| {}), PackageError::NoManifest);
    assert_eq!(
        check(&[manifest], &|_| {}),
        PackageError::NoEntry("dist/index.js".into())
    );
    assert!(matches!(
        check(&[manifest, code, ("../evil.js", b"x")], &|_| {}),
        PackageError::UnsafePath(_)
    ));
    assert!(matches!(
        check(&[manifest, code], &|entry| entry.id = "dev.example.other".into()),
        PackageError::Mismatch(message) if message.contains("id")
    ));
    assert!(matches!(
        check(&[manifest, code], &|entry| entry.version = "1.3.0".into()),
        PackageError::Mismatch(message) if message.contains("version")
    ));
    assert!(matches!(
        check(&[manifest, code], &|entry| { entry.permissions.net.pop(); }),
        PackageError::Mismatch(message) if message.contains("permissions")
    ));
    let huge = vec![0u8; package::MAX_PACKAGE_BYTES as usize + 1];
    assert_eq!(
        check(&[manifest, code, ("big.bin", &huge)], &|_| {}),
        PackageError::TooLarge
    );
}

#[tokio::test]
async fn installs_updates_and_uninstalls_with_settings() {
    let dir = tempfile::tempdir().unwrap();
    let extensions = dir.path().join("extensions");
    let storage = extensions.join(".data");
    std::fs::create_dir_all(storage.join("dev.example.pkg")).unwrap();

    let bytes = good_package();
    let verified = package::verify(&bytes, &entry_for(&bytes)).unwrap();
    let target = package::install(&extensions, &verified).unwrap();
    assert!(target.join("dist/index.js").is_file());
    // Reinstalling (an update) replaces the folder and leaves no staging copy.
    package::install(&extensions, &verified).unwrap();
    let mut names: Vec<_> = std::fs::read_dir(&extensions)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, vec![".data", "dev.example.pkg"]);

    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let mut settings = UserSettings::new();
    settings.insert(
        "dev.example.pkg".into(),
        [
            ("region".to_owned(), json!("us")),
            // Invalid values are dropped, so the default applies.
            ("limit".to_owned(), json!("lots")),
        ]
        .into(),
    );
    let infos = host
        .load_with_settings(discover_installed(&extensions), settings)
        .await;
    assert_eq!(
        host.call("dev.example.pkg/p", "run", json!({}))
            .await
            .unwrap(),
        json!(["us", 5])
    );
    let region = &infos[0].settings[0];
    assert_eq!(
        region.label, "Region",
        "a setting without a label reads as words"
    );
    assert_eq!(region.options, vec!["eu", "us"]);
    assert_eq!(region.value, Some(SettingValue::Text("us".into())));
    assert_eq!(infos[0].settings[1].value, Some(SettingValue::Number(5.0)));

    package::uninstall(&extensions, &storage, "dev.example.pkg").unwrap();
    assert!(!extensions.join("dev.example.pkg").exists());
    assert!(!storage.join("dev.example.pkg").exists());
    assert!(package::uninstall(&extensions, &storage, "../escape").is_err());
}

/// The committed official repository (`repo/`, written by `raygun-ext pack`) must verify with the
/// same checks the app runs before installing.
#[test]
fn official_repository_packages_verify() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../repo");
    let index =
        repo::parse_index(&std::fs::read_to_string(root.join("index.json")).unwrap()).unwrap();
    assert!(!index.extensions.is_empty());
    for entry in &index.extensions {
        let bytes = std::fs::read(root.join(&entry.package)).unwrap();
        assert_eq!(bytes.len() as u64, entry.size, "{}", entry.id);
        let verified =
            package::verify(&bytes, entry).unwrap_or_else(|error| panic!("{}: {error}", entry.id));
        assert_eq!(verified.manifest.id, entry.id);
    }
}

#[test]
fn reads_nuclears_plugin_registry_as_a_repository() {
    let registry = json!({
        "version": 1,
        "plugins": [
            { "id": "nuclear-plugin-lrclib", "name": "LRCLIB", "description": "Lyrics", "author": "nukeop",
              "repo": "NuclearPlayer/nuclear-plugin-lrclib", "categories": ["lyrics"], "version": "0.1.0",
              "downloadUrl": "https://github.com/NuclearPlayer/nuclear-plugin-lrclib/releases/download/v0.1.0/plugin.zip",
              "addedAt": "2026-01-01T00:00:00Z" },
            { "id": "nuclear-plugin-bandcamp", "name": "Bandcamp", "repo": "x/y", "category": "streaming",
              "downloadUrl": "https://example.org/b.zip", "sha256": "AB".repeat(32), "addedAt": "2026-01-01T00:00:00Z" },
            { "id": "nuclear-plugin-dash", "name": "Dash", "repo": "x/y", "categories": ["dashboard"],
              "downloadUrl": "https://example.org/d.zip", "addedAt": "2026-01-01T00:00:00Z" },
            { "id": "nuclear-plugin-nozip", "name": "No zip", "repo": "x/y", "categories": ["metadata"],
              "addedAt": "2026-01-01T00:00:00Z" },
        ],
    });
    let index = repo::parse_index(&registry.to_string()).unwrap();
    assert_eq!(index.name, "Nuclear plugins");
    let ids: Vec<_> = index
        .extensions
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    assert_eq!(
        ids,
        vec![
            "nuclear.nuclear-plugin-lrclib",
            "nuclear.nuclear-plugin-bandcamp"
        ],
        "plugins Raygun can't run, or without a download, are left out"
    );
    let lrclib = &index.extensions[0];
    assert_eq!(lrclib.format, repo::PackageFormat::Nuclear);
    assert_eq!(lrclib.provides, vec!["lyrics"]);
    assert_eq!(lrclib.version, "0.1.0");
    assert_eq!(lrclib.sha256, "");
    assert_eq!(lrclib.size, 0);
    let bandcamp = &index.extensions[1];
    assert_eq!(bandcamp.version, "0.0.0");
    assert_eq!(bandcamp.sha256, "ab".repeat(32));
    // Absolute download URLs stay as they are.
    let base = "https://cdn.jsdelivr.net/gh/NuclearPlayer/plugin-registry@master/plugins.json"
        .parse()
        .unwrap();
    assert_eq!(
        repo::package_url(&base, lrclib).unwrap().as_str(),
        "https://github.com/NuclearPlayer/nuclear-plugin-lrclib/releases/download/v0.1.0/plugin.zip"
    );
}

#[test]
fn nuclear_ids_match_the_converter() {
    // Same rules as packages/nuclear-compat's slug().
    assert_eq!(
        repo::nuclear_extension_id("nuclear-plugin-lrclib"),
        "nuclear.nuclear-plugin-lrclib"
    );
    assert_eq!(
        repo::nuclear_extension_id("@Scope/My  Plugin!"),
        "nuclear.scope-my-plugin"
    );
    assert_eq!(repo::nuclear_extension_id("???"), "nuclear.plugin");
    assert!(crate::manifest::is_extension_id(
        &repo::nuclear_extension_id("@Scope/My  Plugin!")
    ));
}

#[test]
fn nuclear_downloads_are_checked_when_the_registry_lists_a_hash() {
    let bytes = good_package();
    let mut entry = entry_for(&bytes);
    entry.format = repo::PackageFormat::Nuclear;
    assert!(package::check_nuclear_download(&bytes, &entry).is_ok());
    entry.sha256 = "0".repeat(64);
    assert_eq!(
        package::check_nuclear_download(&bytes, &entry),
        Err(PackageError::Checksum)
    );
    entry.sha256 = String::new();
    assert!(
        package::check_nuclear_download(&bytes, &entry).is_ok(),
        "no hash listed"
    );
}

#[cfg(not(target_os = "android"))]
#[test]
fn local_repositories_are_files_on_desktop() {
    let folder = tempfile::TempDir::new().unwrap();
    let index = folder.path().join("index.json");
    let from_path = repo::check_index_url(index.to_str().unwrap()).unwrap();
    assert_eq!(from_path.scheme(), "file");
    assert_eq!(from_path.to_file_path().unwrap(), index);
    assert_eq!(
        repo::check_index_url(from_path.as_str()).unwrap(),
        from_path
    );
    // Packages sit beside the index, and install from there.
    let bytes = good_package();
    let entry = entry_for(&bytes);
    let package = repo::package_url(&from_path, &entry).unwrap();
    assert_eq!(
        package.to_file_path().unwrap(),
        folder
            .path()
            .join("packages")
            .join("dev.example.pkg-1.2.0.zip")
    );
    // A web index never reaches this computer's files.
    let web: reqwest::Url = "https://example.org/repo/index.json".parse().unwrap();
    let local_file = RepoEntry {
        package: package.to_string(),
        ..entry
    };
    assert!(repo::package_url(&web, &local_file).is_err());
}
