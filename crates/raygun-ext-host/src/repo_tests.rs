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
        package: "packages/dev.example.pkg-1.2.0.zip".into(),
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
    assert!(repo::check_index_url("file:///index.json").is_err());
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
    assert_eq!(region.label, "region");
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
