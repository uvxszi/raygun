//! Where an extension's source code is, and fetching what conversion needs of it: its build file
//! and its Kotlin files, never its pictures.
//!
//! Stores publish APKs; their code sits in a GitHub repository beside them. A Keiyoushi store's
//! icons come from that repository (`cdn.jsdelivr.net/gh/keiyoushi/extensions-source@main/src/
//! all/ahottie/res/…`), which names the folder. For a store that doesn't say (a legacy list with
//! `icon/<pkg>.png` beside it), the folder is found by the layout Keiyoushi and its forks share:
//! `src/<lang>/<name>` of the store repository's `-source` twin.
//!
//! The folder's files are listed with one call to GitHub's API (`git/trees/<ref>:<folder>`), then
//! read from raw.githubusercontent.com, which has no such limit. When the API refuses (60 calls an
//! hour without a sign-in), the main file is looked for by the names the extension's class takes.

use std::collections::BTreeMap;
use std::future::Future;

use reqwest::Url;
use serde::Deserialize;

use super::index::{PACKAGE_PREFIX, StoreExtension};

/// Most files read for one extension, the largest one, and all of them together.
const MAX_FILES: usize = 64;
const MAX_FILE_BYTES: u64 = 512 * 1024;
const MAX_TOTAL_BYTES: u64 = 2 * 1024 * 1024;

/// A folder of a GitHub repository at a branch or commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFolder {
    pub owner: String,
    pub repo: String,
    pub git_ref: String,
    /// Inside the repository, without slashes at either end (`src/all/ahottie`).
    pub path: String,
}

impl SourceFolder {
    /// GitHub's listing of every file under the folder.
    pub fn tree_url(&self) -> Option<Url> {
        Url::parse(&format!(
            "https://api.github.com/repos/{}/{}/git/trees/{}:{}?recursive=1",
            self.owner, self.repo, self.git_ref, self.path
        ))
        .ok()
    }

    /// A file of the folder (`file` relative to it).
    pub fn raw_url(&self, file: &str) -> Option<Url> {
        if file.split('/').any(|part| part.is_empty() || part == "..") {
            return None;
        }
        Url::parse(&format!(
            "https://raw.githubusercontent.com/{}/{}/{}/{}/{file}",
            self.owner, self.repo, self.git_ref, self.path
        ))
        .ok()
    }

    /// The folder on GitHub, for people.
    pub fn web_url(&self) -> String {
        format!(
            "https://github.com/{}/{}/tree/{}/{}",
            self.owner, self.repo, self.git_ref, self.path
        )
    }
}

/// Where a library's code is in the store's repository: Keiyoushi's libraries (`lib:unpacker` is
/// `lib/unpacker`), and the parts of its `core` the runtime doesn't have itself, which an
/// extension's code imports (`core:zip` is `core/src/main/kotlin/keiyoushi/zip`).
pub fn library_path(library: &str) -> Option<String> {
    if let Some(module) = library.strip_prefix("core:") {
        return is_part(module).then(|| format!("core/src/main/kotlin/keiyoushi/{module}"));
    }
    let name = library.trim_start_matches("lib:");
    is_part(name).then(|| format!("lib/{name}"))
}

fn is_part(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && text != "."
        && text != ".."
}

/// Where `extension`'s code is; `store_url` is where its store was read.
pub fn locate(extension: &StoreExtension, store_url: &Url) -> Option<SourceFolder> {
    let parts = package_parts(&extension.pkg);
    if let Some(mut folder) = extension
        .icon
        .as_deref()
        .and_then(|icon| Url::parse(icon).ok())
        .and_then(|icon| from_icon(&icon))
    {
        // An extension showing its theme's icon (`lib-multisrc/madara/res/…`) has its own
        // folder where its package name says.
        let own = folder.path.starts_with("src/") && folder.path.split('/').count() == 3;
        if !own {
            let (lang, name) = parts?;
            folder.path = format!("src/{lang}/{name}");
        }
        return Some(folder);
    }
    // A legacy store at `raw.githubusercontent.com/<owner>/<repo>/<branch>/…`: the code is in
    // `<owner>/<repo>-source`, as Keiyoushi's is.
    let (lang, name) = parts?;
    let segments: Vec<&str> = store_url.path_segments()?.collect();
    let (owner, repo) = match store_url.host_str()? {
        "raw.githubusercontent.com" | "github.com" => (*segments.first()?, *segments.get(1)?),
        "cdn.jsdelivr.net" if segments.first() == Some(&"gh") => {
            (*segments.get(1)?, segments.get(2)?.split('@').next()?)
        }
        _ => return None,
    };
    if !is_part(owner) || !is_part(repo) {
        return None;
    }
    Some(SourceFolder {
        owner: owner.to_owned(),
        repo: format!("{repo}-source"),
        git_ref: "main".to_owned(),
        path: format!("src/{lang}/{name}"),
    })
}

/// The `<lang>` and `<name>` of `eu.kanade.tachiyomi.extension.<lang>.<name>`.
pub fn package_parts(pkg: &str) -> Option<(&str, &str)> {
    let rest = pkg.strip_prefix(PACKAGE_PREFIX)?;
    let (lang, name) = rest.split_once('.')?;
    (is_part(lang) && is_part(name) && !name.contains('.')).then_some((lang, name))
}

/// The folder an icon's address names: `…/<folder>/res/…` on jsDelivr's GitHub mirror,
/// raw.githubusercontent.com or github.com.
fn from_icon(icon: &Url) -> Option<SourceFolder> {
    let segments: Vec<&str> = icon.path_segments()?.collect();
    let res = segments.iter().rposition(|part| *part == "res")?;
    let (owner, repo, git_ref, start) = match icon.host_str()? {
        "cdn.jsdelivr.net" if segments.first() == Some(&"gh") => {
            let (repo, git_ref) = segments.get(2)?.split_once('@')?;
            (*segments.get(1)?, repo, git_ref, 3)
        }
        "raw.githubusercontent.com" => {
            (*segments.first()?, *segments.get(1)?, *segments.get(2)?, 3)
        }
        "github.com" if matches!(segments.get(2), Some(&"raw" | &"blob")) => {
            (*segments.first()?, *segments.get(1)?, *segments.get(3)?, 4)
        }
        _ => return None,
    };
    let path = segments.get(start..res)?.join("/");
    if [owner, repo, git_ref].iter().any(|part| !is_part(part))
        || path.is_empty()
        || path.split('/').any(|part| !is_part(part))
    {
        return None;
    }
    Some(SourceFolder {
        owner: owner.to_owned(),
        repo: repo.to_owned(),
        git_ref: git_ref.to_owned(),
        path,
    })
}

#[derive(Debug, Deserialize)]
struct Tree {
    #[serde(default)]
    tree: Vec<TreeEntry>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TreeEntry {
    path: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    size: u64,
}

/// Whether conversion reads a file: the build file, Kotlin sources and Keiyoushi's message files
/// (`assets/i18n/messages_en.properties`).
fn wanted(path: &str) -> bool {
    matches!(path, "build.gradle" | "build.gradle.kts")
        // A part of `core` (`core:zip`) keeps its files at the top of its folder.
        || (path.ends_with(".kt") && (path.starts_with("src/") || !path.contains('/')))
        || (path.starts_with("assets/i18n/messages_")
            && path.ends_with(".properties")
            && !path["assets/i18n/".len()..].contains('/'))
}

/// Whether a file is one of the extension's resources (`assets/page_list.js`, read with
/// `javaClass.getResource(…)`), which conversion keeps with the code: `i18n` messages aside.
fn wanted_asset(path: &str) -> bool {
    path.starts_with("assets/") && !path.starts_with("assets/i18n/")
}

/// The largest resource kept, and all of an extension's together.
const MAX_ASSET_BYTES: u64 = 1024 * 1024;
const MAX_ASSETS_BYTES: u64 = 2 * 1024 * 1024;

/// A Java `.properties` file's entries: `key=value` or `key: value`, `#` and `!` comments,
/// lines continued with `\`, and `\uXXXX` escapes.
pub fn properties(text: &str) -> BTreeMap<String, String> {
    let mut entries = BTreeMap::new();
    let mut logical = String::new();
    for line in text.lines() {
        let line = line.trim_start();
        if logical.is_empty() && (line.is_empty() || line.starts_with('#') || line.starts_with('!'))
        {
            continue;
        }
        let continued = line.ends_with('\\') && !line.ends_with("\\\\");
        logical.push_str(if continued {
            &line[..line.len() - 1]
        } else {
            line
        });
        if continued {
            continue;
        }
        let entry = std::mem::take(&mut logical);
        let split = entry
            .char_indices()
            .find(|(index, char)| matches!(char, '=' | ':') && !entry[..*index].ends_with('\\'))
            .map(|(index, _)| index);
        let (key, value) = match split {
            Some(index) => (&entry[..index], &entry[index + 1..]),
            None => (entry.as_str(), ""),
        };
        entries.insert(unescape(key.trim()), unescape(value.trim_start()));
    }
    entries
}

fn unescape(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(char) = chars.next() {
        if char != '\\' {
            out.push(char);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('u') => {
                let code: String = chars.by_ref().take(4).collect();
                if let Some(decoded) = u32::from_str_radix(&code, 16).ok().and_then(char::from_u32)
                {
                    out.push(decoded);
                }
            }
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// The files of a folder listing conversion reads, with their sizes.
pub fn listing(json: &[u8]) -> Result<Vec<(String, u64)>, String> {
    let tree: Tree = serde_json::from_slice(json)
        .map_err(|error| format!("GitHub's listing of the folder isn't readable: {error}"))?;
    if tree.tree.is_empty() {
        return Err(tree.message.map_or_else(
            || "the extension's folder is empty".to_owned(),
            |message| format!("GitHub didn't list the folder: {message}"),
        ));
    }
    Ok(tree
        .tree
        .into_iter()
        .filter(|entry| entry.kind == "blob" && (wanted(&entry.path) || wanted_asset(&entry.path)))
        .map(|entry| (entry.path, entry.size))
        .collect())
}

/// Files of a listing, by path, with their sizes.
type Listed = Vec<(String, u64)>;

/// A listing's code files, and its resources that fit (the smallest first).
fn split_assets(files: Listed) -> (Listed, Listed) {
    let (mut assets, code): (Vec<_>, Vec<_>) =
        files.into_iter().partition(|(path, _)| wanted_asset(path));
    assets.retain(|(_, size)| *size <= MAX_ASSET_BYTES);
    assets.sort_by_key(|(_, size)| *size);
    let mut total = 0;
    assets.retain(|(_, size)| {
        total += size;
        total <= MAX_ASSETS_BYTES
    });
    (code, assets)
}

/// An extension's code: its folder and the files read, by path inside it; its theme's
/// (`lib-multisrc/<theme>`), when it's built on one Raygun converts from code; and the code of
/// Keiyoushi's libraries it builds on (`lib/<name>`), by the library's name (`lib:unpacker`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sources {
    pub files: BTreeMap<String, String>,
    pub theme_files: BTreeMap<String, String>,
    pub library_files: BTreeMap<String, BTreeMap<String, String>>,
    /// Resources of the extension, its theme and its libraries (`assets/…`), as an APK merges
    /// them: the extension's own first.
    pub assets: BTreeMap<String, Vec<u8>>,
}

fn build_file_of(files: &BTreeMap<String, String>) -> Option<&str> {
    files
        .get("build.gradle.kts")
        .or_else(|| files.get("build.gradle"))
        .map(String::as_str)
}

impl Sources {
    pub fn build_file(&self) -> Option<&str> {
        build_file_of(&self.files)
    }

    pub fn kotlin(&self) -> impl Iterator<Item = (&String, &String)> {
        self.files.iter().filter(|(path, _)| path.ends_with(".kt"))
    }

    pub fn theme_build_file(&self) -> Option<&str> {
        build_file_of(&self.theme_files)
    }

    pub fn theme_kotlin(&self) -> impl Iterator<Item = (&String, &String)> {
        self.theme_files
            .iter()
            .filter(|(path, _)| path.ends_with(".kt"))
    }

    /// The Kotlin files of the libraries read, each path under its library's folder
    /// (`lib/unpacker/src/…`).
    pub fn library_kotlin(&self) -> impl Iterator<Item = (String, &String)> {
        self.library_files.iter().flat_map(|(library, files)| {
            let folder = library_path(library).unwrap_or_else(|| library.clone());
            files
                .iter()
                .filter(|(path, _)| path.ends_with(".kt"))
                .map(move |(path, text)| (format!("{folder}/{path}"), text))
        })
    }

    /// Message files by language (`pt_br`), the theme's first and the extension's over them.
    pub fn messages(&self) -> BTreeMap<String, BTreeMap<String, String>> {
        let mut found: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        for (path, text) in self.theme_files.iter().chain(&self.files) {
            let Some(lang) = path
                .strip_prefix("assets/i18n/messages_")
                .and_then(|rest| rest.strip_suffix(".properties"))
            else {
                continue;
            };
            found
                .entry(lang.to_ascii_lowercase())
                .or_default()
                .extend(properties(text));
        }
        found
    }

    /// Reads a folder on disk (a checkout of the repository), for surveys and tests.
    pub fn from_dir(dir: &std::path::Path) -> std::io::Result<Self> {
        Ok(Self {
            files: read_dir(dir)?,
            assets: read_assets(dir),
            ..Self::default()
        })
    }

    /// Adds a theme's folder on disk (`lib-multisrc/<theme>` of a checkout).
    pub fn add_theme_dir(&mut self, dir: &std::path::Path) -> std::io::Result<()> {
        self.theme_files = read_dir(dir)?;
        self.add_assets(read_assets(dir));
        Ok(())
    }

    /// Adds a library's folder on disk (`lib/<name>` of a checkout) as `library` (`lib:<name>`).
    pub fn add_library_dir(&mut self, library: &str, dir: &std::path::Path) -> std::io::Result<()> {
        self.library_files
            .insert(library.to_owned(), read_dir(dir)?);
        self.add_assets(read_assets(dir));
        Ok(())
    }

    /// Resources a theme or a library brings, under the extension's own of the same name.
    pub fn add_assets(&mut self, assets: BTreeMap<String, Vec<u8>>) {
        for (path, bytes) in assets {
            let total: usize = self.assets.values().map(Vec::len).sum();
            if total + bytes.len() <= MAX_ASSETS_BYTES as usize {
                self.assets.entry(path).or_insert(bytes);
            }
        }
    }
}

/// A folder's resources on disk (`assets/…` but its messages), the smallest first, within the
/// limits.
fn read_assets(dir: &std::path::Path) -> BTreeMap<String, Vec<u8>> {
    let mut found = Vec::new();
    let mut pending = vec![dir.join("assets")];
    while let Some(current) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if let Ok(relative) = path.strip_prefix(dir) {
                let relative = relative.to_string_lossy().replace('\\', "/");
                let size = entry.metadata().map(|meta| meta.len()).unwrap_or(u64::MAX);
                if wanted_asset(&relative) {
                    found.push((relative, size, path));
                }
            }
        }
    }
    let (_, kept) = split_assets(
        found
            .iter()
            .map(|(relative, size, _)| (relative.clone(), *size))
            .collect(),
    );
    found
        .into_iter()
        .filter(|(relative, _, _)| kept.iter().any(|(path, _)| path == relative))
        .filter_map(|(relative, _, path)| Some((relative, std::fs::read(path).ok()?)))
        .collect()
}

fn read_dir(dir: &std::path::Path) -> std::io::Result<BTreeMap<String, String>> {
    {
        let mut files = BTreeMap::new();
        let mut pending = vec![dir.to_path_buf()];
        while let Some(current) = pending.pop() {
            for entry in std::fs::read_dir(&current)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                    continue;
                }
                let Ok(relative) = path.strip_prefix(dir) else {
                    continue;
                };
                let relative = relative.to_string_lossy().replace('\\', "/");
                if wanted(&relative) && files.len() < MAX_FILES {
                    files.insert(relative, std::fs::read_to_string(&path)?);
                }
            }
        }
        Ok(files)
    }
}

/// Names the extension's class may have, for finding its main file without a listing.
fn class_names(extension: &StoreExtension) -> Vec<String> {
    let name = extension.name.trim_start_matches("Tachiyomi: ");
    let words: Vec<&str> = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    let capitalized: String = words
        .iter()
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + chars.as_str()
            })
        })
        .collect();
    let joined: String = words.concat();
    let mut names = vec![capitalized, joined];
    names.retain(|name| name.chars().next().is_some_and(char::is_alphabetic));
    names.dedup();
    names
}

/// Reads `folder`'s build file and Kotlin sources through `fetch`.
pub async fn fetch<F, Fut>(
    folder: &SourceFolder,
    extension: &StoreExtension,
    fetch: F,
) -> Result<Sources, String>
where
    F: Fn(Url) -> Fut,
    Fut: Future<Output = Result<Vec<u8>, String>>,
{
    let listed = match folder.tree_url() {
        Some(url) => fetch(url).await.and_then(|json| listing(&json)),
        None => Err("the folder's address is invalid".to_owned()),
    };
    let (wanted_files, assets): (Listed, Listed) = match listed {
        Ok(files) => split_assets(files),
        Err(error) => {
            // Without a listing: the build file and the main file by its likely names.
            tracing::debug!(%error, folder = %folder.web_url(), "no listing; guessing the files");
            let mut sources = Sources::default();
            for build in ["build.gradle.kts", "build.gradle"] {
                if let Some(url) = folder.raw_url(build)
                    && let Ok(bytes) = fetch(url).await
                {
                    sources.files.insert(
                        build.to_owned(),
                        String::from_utf8_lossy(&bytes).into_owned(),
                    );
                    break;
                }
            }
            if sources.files.is_empty() {
                return Err(format!(
                    "Raygun couldn't read the extension's code at {}\n{error}",
                    folder.web_url()
                ));
            }
            let package_path = extension.pkg.replace('.', "/");
            for class in class_names(extension) {
                let file = format!("src/{package_path}/{class}.kt");
                if let Some(url) = folder.raw_url(&file)
                    && let Ok(bytes) = fetch(url).await
                {
                    sources
                        .files
                        .insert(file, String::from_utf8_lossy(&bytes).into_owned());
                    return Ok(sources);
                }
            }
            return Err(format!(
                "Raygun couldn't list the extension's files (GitHub allows 60 listings an hour; try again later)\n{error}"
            ));
        }
    };
    if wanted_files.len() > MAX_FILES {
        return Err(format!(
            "the extension has {} code files, more than Raygun converts",
            wanted_files.len()
        ));
    }
    let total: u64 = wanted_files.iter().map(|(_, size)| size).sum();
    if total > MAX_TOTAL_BYTES || wanted_files.iter().any(|(_, size)| *size > MAX_FILE_BYTES) {
        return Err("the extension's code is larger than Raygun converts".into());
    }
    let mut sources = Sources::default();
    for (path, _) in wanted_files {
        let url = folder
            .raw_url(&path)
            .ok_or_else(|| format!("the folder lists an odd file name: {path}"))?;
        let bytes = fetch(url).await?;
        sources
            .files
            .insert(path, String::from_utf8_lossy(&bytes).into_owned());
    }
    sources.add_assets(fetch_assets(folder, assets, &fetch).await);
    Ok(sources)
}

/// A folder's resources, as they're listed (one that can't be read is left out).
async fn fetch_assets<F, Fut>(
    folder: &SourceFolder,
    assets: Vec<(String, u64)>,
    fetch: &F,
) -> BTreeMap<String, Vec<u8>>
where
    F: Fn(Url) -> Fut,
    Fut: Future<Output = Result<Vec<u8>, String>>,
{
    let mut read = BTreeMap::new();
    for (path, _) in assets {
        if let Some(url) = folder.raw_url(&path)
            && let Ok(bytes) = fetch(url).await
        {
            read.insert(path, bytes);
        }
    }
    read
}

/// Reads the code of the theme `theme` (`lib-multisrc/<theme>` in the extension's repository)
/// into `sources`, for converting a themed extension from its theme's code.
pub async fn fetch_theme<F, Fut>(
    folder: &SourceFolder,
    theme: &str,
    sources: &mut Sources,
    fetch: F,
) -> Result<(), String>
where
    F: Fn(Url) -> Fut,
    Fut: Future<Output = Result<Vec<u8>, String>>,
{
    if !is_part(theme) {
        return Err(format!("the theme's name is odd: {theme}"));
    }
    let theme_folder = SourceFolder {
        path: format!("lib-multisrc/{theme}"),
        ..folder.clone()
    };
    let url = theme_folder
        .tree_url()
        .ok_or("the theme's folder address is invalid")?;
    let (files, assets) = split_assets(
        listing(&fetch(url).await?)
            .map_err(|error| format!("Raygun couldn't list the {theme} theme's files\n{error}"))?,
    );
    let total: u64 = files.iter().map(|(_, size)| size).sum();
    if files.len() > MAX_FILES
        || total > MAX_TOTAL_BYTES
        || files.iter().any(|(_, size)| *size > MAX_FILE_BYTES)
    {
        return Err(format!(
            "the {theme} theme's code is larger than Raygun converts"
        ));
    }
    for (path, _) in files {
        let url = theme_folder
            .raw_url(&path)
            .ok_or_else(|| format!("the theme's folder lists an odd file name: {path}"))?;
        let bytes = fetch(url).await?;
        sources
            .theme_files
            .insert(path, String::from_utf8_lossy(&bytes).into_owned());
    }
    sources.add_assets(fetch_assets(&theme_folder, assets, &fetch).await);
    Ok(())
}

/// Reads the code of Keiyoushi's library `library` (`lib:unpacker` → `lib/unpacker` in the
/// extension's repository) into `sources`, for converting an extension that builds on it.
pub async fn fetch_library<F, Fut>(
    folder: &SourceFolder,
    library: &str,
    sources: &mut Sources,
    fetch: F,
) -> Result<(), String>
where
    F: Fn(Url) -> Fut,
    Fut: Future<Output = Result<Vec<u8>, String>>,
{
    let Some(path) = library_path(library) else {
        return Err(format!("the library's name is odd: {library}"));
    };
    let library_folder = SourceFolder {
        path,
        ..folder.clone()
    };
    let url = library_folder
        .tree_url()
        .ok_or("the library's folder address is invalid")?;
    let (files, assets) = split_assets(
        listing(&fetch(url).await?)
            .map_err(|error| format!("Raygun couldn't list {library}'s files\n{error}"))?,
    );
    let total: u64 = files.iter().map(|(_, size)| size).sum();
    if files.len() > MAX_FILES
        || total > MAX_TOTAL_BYTES
        || files.iter().any(|(_, size)| *size > MAX_FILE_BYTES)
    {
        return Err(format!("{library}'s code is larger than Raygun converts"));
    }
    let mut read = BTreeMap::new();
    for (path, _) in files {
        let url = library_folder
            .raw_url(&path)
            .ok_or_else(|| format!("the library's folder lists an odd file name: {path}"))?;
        let bytes = fetch(url).await?;
        read.insert(path, String::from_utf8_lossy(&bytes).into_owned());
    }
    sources.library_files.insert(library.to_owned(), read);
    sources.add_assets(fetch_assets(&library_folder, assets, &fetch).await);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mihon::index::ContentWarning;

    fn extension(pkg: &str, icon: Option<&str>) -> StoreExtension {
        StoreExtension {
            name: "Comics Valley".into(),
            pkg: pkg.into(),
            apk: "https://example.org/x.apk".into(),
            icon: icon.map(str::to_owned),
            lib_version: "1.6".into(),
            version_code: 106_002,
            version_name: "1.6.2".into(),
            content: ContentWarning::Safe,
            sources: Vec::new(),
        }
    }

    #[test]
    fn finds_the_folder_from_the_icon_or_the_layout() {
        let store: Url =
            "https://raw.githubusercontent.com/keiyoushi/extensions/repo/index.min.json"
                .parse()
                .unwrap();
        let jsdelivr = extension(
            "eu.kanade.tachiyomi.extension.all.ahottie",
            Some(
                "https://cdn.jsdelivr.net/gh/keiyoushi/extensions-source@main/src/all/ahottie/res/mipmap-xhdpi/ic_launcher.png",
            ),
        );
        let folder = locate(&jsdelivr, &store).unwrap();
        assert_eq!(folder.repo, "extensions-source");
        assert_eq!(folder.path, "src/all/ahottie");
        assert_eq!(
            folder.tree_url().unwrap().as_str(),
            "https://api.github.com/repos/keiyoushi/extensions-source/git/trees/main:src/all/ahottie?recursive=1"
        );
        assert_eq!(
            folder.raw_url("build.gradle.kts").unwrap().as_str(),
            "https://raw.githubusercontent.com/keiyoushi/extensions-source/main/src/all/ahottie/build.gradle.kts"
        );
        assert!(folder.raw_url("../secrets").is_none());

        let legacy = extension(
            "eu.kanade.tachiyomi.extension.en.comicsvalley",
            Some(
                "https://raw.githubusercontent.com/keiyoushi/extensions/repo/icon/eu.kanade.tachiyomi.extension.en.comicsvalley.png",
            ),
        );
        let folder = locate(&legacy, &store).unwrap();
        assert_eq!(folder.owner, "keiyoushi");
        assert_eq!(folder.repo, "extensions-source");
        assert_eq!(folder.path, "src/en/comicsvalley");
        assert_eq!(class_names(&legacy), ["ComicsValley"]);
    }

    #[test]
    fn listings_keep_the_build_file_and_code() {
        let json = br#"{"sha":"x","tree":[
            {"path":"build.gradle.kts","type":"blob","size":403},
            {"path":"res","type":"tree"},
            {"path":"res/mipmap-hdpi/ic_launcher.png","type":"blob","size":3850},
            {"path":"src/eu/kanade/tachiyomi/extension/all/ahottie/AHottie.kt","type":"blob","size":5000}
        ],"truncated":false}"#;
        let files = listing(json).unwrap();
        assert_eq!(files.len(), 2);
        assert!(
            listing(br#"{"message":"API rate limit exceeded"}"#)
                .unwrap_err()
                .contains("rate limit")
        );
    }
}
