//! Extension packages: verifying a downloaded zip against its repository entry, installing it into
//! `extensions/<id>/` and uninstalling (M13).

use std::collections::BTreeSet;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::manifest::{MANIFEST_FILE, Manifest, is_extension_id};
use crate::permissions::HostPattern;
use crate::repo::RepoEntry;

pub const MAX_PACKAGE_BYTES: u64 = 10 * 1024 * 1024;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PackageError {
    #[error("package is larger than {MAX_PACKAGE_BYTES} bytes")]
    TooLarge,
    #[error("package checksum doesn't match the repository index")]
    Checksum,
    #[error("package is not a valid zip: {0}")]
    Zip(String),
    #[error("package contains an unsafe path: {0}")]
    UnsafePath(String),
    #[error("package has no {MANIFEST_FILE}")]
    NoManifest,
    #[error("package manifest is invalid: {0}")]
    Manifest(String),
    #[error("package doesn't match the repository index: {0}")]
    Mismatch(String),
    #[error("package is missing its entry file {0}")]
    NoEntry(String),
    #[error("package is missing {0}, which its site scripts use")]
    MissingFile(String),
    #[error("could not install: {0}")]
    Io(String),
}

/// A package whose checksum, contents and manifest have all been checked.
#[derive(Debug)]
pub struct VerifiedPackage {
    pub manifest: Manifest,
    files: Vec<(String, Vec<u8>)>,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn normalized_net(patterns: impl IntoIterator<Item = String>) -> Option<BTreeSet<String>> {
    patterns
        .into_iter()
        .map(|pattern| HostPattern::parse(&pattern).map(|pattern| pattern.to_string()))
        .collect()
}

/// Checks a downloaded package against the index entry it was listed under.
pub fn verify(bytes: &[u8], entry: &RepoEntry) -> Result<VerifiedPackage, PackageError> {
    if bytes.len() as u64 > MAX_PACKAGE_BYTES {
        return Err(PackageError::TooLarge);
    }
    if !sha256_hex(bytes).eq_ignore_ascii_case(&entry.sha256) {
        return Err(PackageError::Checksum);
    }

    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| PackageError::Zip(error.to_string()))?;
    let mut files = Vec::new();
    let mut total = 0u64;
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|error| PackageError::Zip(error.to_string()))?;
        let name = file
            .name()
            .map_or_else(|_| "(unreadable name)".to_owned(), |name| name.into_owned());
        if file.is_symlink() {
            return Err(PackageError::UnsafePath(name));
        }
        // `enclosed_name` rejects absolute paths and `..` escapes.
        let path = file
            .enclosed_name()
            .ok_or_else(|| PackageError::UnsafePath(name.clone()))?;
        if file.is_dir() {
            continue;
        }
        let relative = path
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        // Guard against zip bombs: count real bytes, not the declared size.
        let mut contents = Vec::new();
        let limit = MAX_PACKAGE_BYTES.saturating_sub(total) + 1;
        (&mut file)
            .take(limit)
            .read_to_end(&mut contents)
            .map_err(|error| PackageError::Zip(error.to_string()))?;
        total += contents.len() as u64;
        if total > MAX_PACKAGE_BYTES {
            return Err(PackageError::TooLarge);
        }
        files.push((relative, contents));
    }

    let manifest_text = files
        .iter()
        .find(|(name, _)| name == MANIFEST_FILE)
        .map(|(_, contents)| String::from_utf8_lossy(contents).into_owned())
        .ok_or(PackageError::NoManifest)?;
    let manifest = Manifest::parse(&manifest_text)
        .map_err(|error| PackageError::Manifest(error.to_string()))?;

    if manifest.id != entry.id {
        return Err(PackageError::Mismatch(format!(
            "id {} instead of {}",
            manifest.id, entry.id
        )));
    }
    if semver::Version::parse(&entry.version).ok().as_ref() != Some(&manifest.version) {
        return Err(PackageError::Mismatch(format!(
            "version {} instead of {}",
            manifest.version, entry.version
        )));
    }
    let listed = normalized_net(entry.permissions.net.iter().cloned());
    let actual = normalized_net(manifest.net.patterns().iter().map(ToString::to_string));
    if listed != actual || entry.permissions.webview != manifest.webview {
        return Err(PackageError::Mismatch(
            "its permissions differ from the ones the repository lists".to_owned(),
        ));
    }
    if !files.iter().any(|(name, _)| name == &manifest.entry) {
        return Err(PackageError::NoEntry(manifest.entry.clone()));
    }
    check_site(&manifest, entry, &files)?;
    Ok(VerifiedPackage { manifest, files })
}

/// A site-scripts extension's package: every file its scripts use is in it, and the sites they
/// run on are the ones its repository lists (and people saw before installing).
fn check_site(
    manifest: &Manifest,
    entry: &RepoEntry,
    files: &[(String, Vec<u8>)],
) -> Result<(), PackageError> {
    let listed: BTreeSet<&str> = entry.sites.iter().map(String::as_str).collect();
    let Some(site) = &manifest.site else {
        return if listed.is_empty() {
            Ok(())
        } else {
            Err(PackageError::Mismatch(
                "the repository lists sites for scripts it doesn't have".to_owned(),
            ))
        };
    };
    if let Some(missing) = site
        .files()
        .into_iter()
        .find(|path| !files.iter().any(|(name, _)| name == path))
    {
        return Err(PackageError::MissingFile(missing.to_owned()));
    }
    let patterns = site.patterns();
    let actual: BTreeSet<&str> = patterns.iter().map(String::as_str).collect();
    if listed != actual {
        return Err(PackageError::Mismatch(
            "the sites its scripts run on differ from the ones the repository lists".to_owned(),
        ));
    }
    Ok(())
}

/// The entry a repository would list for a package found on its own, at `package` (an address
/// added in Sources): its manifest's id, name, version and permissions, with the package's own
/// checksum and size, so [`verify`] checks it as one from a repository. Servers are connected by
/// their address, never installed this way.
pub fn entry_of(bytes: &[u8], package: &str) -> Result<RepoEntry, PackageError> {
    if bytes.len() as u64 > MAX_PACKAGE_BYTES {
        return Err(PackageError::TooLarge);
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| PackageError::Zip(error.to_string()))?;
    let mut file = archive
        .by_name(MANIFEST_FILE)
        .map_err(|_| PackageError::NoManifest)?;
    let mut text = String::new();
    (&mut file)
        .take(1024 * 1024)
        .read_to_string(&mut text)
        .map_err(|error| PackageError::Manifest(error.to_string()))?;
    let manifest =
        Manifest::parse(&text).map_err(|error| PackageError::Manifest(error.to_string()))?;
    if manifest.runtime == crate::manifest::Runtime::Server {
        return Err(PackageError::Manifest(
            "a server is connected by its address, not installed".to_owned(),
        ));
    }
    let mut entry = RepoEntry {
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        version: manifest.version.to_string(),
        api_version: crate::manifest::API_VERSION,
        description: manifest.description.clone(),
        author: manifest.author.clone(),
        provides: manifest.provides.clone(),
        permissions: crate::repo::RepoPermissions {
            net: manifest
                .net
                .patterns()
                .iter()
                .map(ToString::to_string)
                .collect(),
            webview: manifest.webview,
        },
        content_rating: manifest.content_rating.as_str().to_owned(),
        lang: None,
        package: package.to_owned(),
        icon: None,
        sha256: sha256_hex(bytes),
        size: bytes.len() as u64,
        format: crate::repo::PackageFormat::Raygun,
        sites: manifest
            .site
            .as_ref()
            .map(crate::site::SiteScripts::patterns)
            .unwrap_or_default(),
        not_for_raygun: None,
    };
    // A package that would provide nothing isn't for Raygun.
    crate::purpose::mark(&mut entry);
    Ok(entry)
}

/// A converted Nuclear plugin (M17) as a package to install: its generated manifest and bundle.
pub fn from_converted(manifest_json: &str, bundle: &str) -> Result<VerifiedPackage, PackageError> {
    let manifest = Manifest::parse(manifest_json)
        .map_err(|error| PackageError::Manifest(error.to_string()))?;
    let files = vec![
        (MANIFEST_FILE.to_owned(), manifest_json.as_bytes().to_vec()),
        (manifest.entry.clone(), bundle.as_bytes().to_vec()),
    ];
    Ok(VerifiedPackage { manifest, files })
}

impl VerifiedPackage {
    /// Its files, the manifest first, as they'll be installed.
    pub fn files(&self) -> &[(String, Vec<u8>)] {
        &self.files
    }
}

/// A package built at install from a generated manifest and its files (Shosetsu scripts, M32).
pub fn from_files(
    manifest_json: &str,
    files: Vec<(String, Vec<u8>)>,
) -> Result<VerifiedPackage, PackageError> {
    let manifest = Manifest::parse(manifest_json)
        .map_err(|error| PackageError::Manifest(error.to_string()))?;
    let mut all = vec![(MANIFEST_FILE.to_owned(), manifest_json.as_bytes().to_vec())];
    all.extend(files);
    if !all.iter().any(|(name, _)| *name == manifest.entry) {
        return Err(PackageError::NoEntry(manifest.entry));
    }
    Ok(VerifiedPackage {
        manifest,
        files: all,
    })
}

/// Checks a Nuclear plugin download against the registry's checksum, when it lists one.
pub fn check_nuclear_download(bytes: &[u8], entry: &RepoEntry) -> Result<(), PackageError> {
    if bytes.len() as u64 > MAX_PACKAGE_BYTES {
        return Err(PackageError::TooLarge);
    }
    if !entry.sha256.is_empty() && !sha256_hex(bytes).eq_ignore_ascii_case(&entry.sha256) {
        return Err(PackageError::Checksum);
    }
    Ok(())
}

/// Writes a verified package to `extensions/<id>/`, replacing any previous version. Files land in
/// a staging folder first (ignored by discovery), so a failure leaves the old version intact.
pub fn install(extensions_dir: &Path, package: &VerifiedPackage) -> Result<PathBuf, PackageError> {
    let io = |error: std::io::Error| PackageError::Io(error.to_string());
    let id = &package.manifest.id;
    let staging = extensions_dir.join(format!(".staging-{id}"));
    let target = extensions_dir.join(id);
    if staging.exists() {
        std::fs::remove_dir_all(&staging).map_err(io)?;
    }
    for (name, contents) in &package.files {
        let path = staging.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(io)?;
        }
        std::fs::write(&path, contents).map_err(io)?;
    }
    if target.exists() {
        std::fs::remove_dir_all(&target).map_err(io)?;
    }
    std::fs::rename(&staging, &target).map_err(io)?;
    Ok(target)
}

/// Removes an installed extension and its storage.
/// Deletes what an extension stored (`raygun.storage`) and its cookies (M46.11); it starts afresh
/// on its next load.
pub fn clear_data(storage_dir: &Path, id: &str) -> std::io::Result<()> {
    if !is_extension_id(id) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid extension id {id:?}"),
        ));
    }
    let dir = storage_dir.join(id);
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}

pub fn uninstall(extensions_dir: &Path, storage_dir: &Path, id: &str) -> std::io::Result<()> {
    if !is_extension_id(id) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid extension id {id:?}"),
        ));
    }
    for dir in [extensions_dir.join(id), storage_dir.join(id)] {
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
        }
    }
    Ok(())
}
