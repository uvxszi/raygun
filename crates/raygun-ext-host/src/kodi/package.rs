//! Installing a Kodi plugin: its zip, and the `script.module.*` libraries it imports (and theirs),
//! from its own repository first, then Kodi's ([`super::OFFICIAL_REPOSITORY`]), as Kodi installs
//! them. Raygun answers some of what plugins need itself ([`ANSWERED`]); a library with compiled
//! code, or a plugin written for Python 2, isn't installed, saying why.
//!
//! The extension's folder holds the add-on (`addon/`), its libraries (`lib/<id>/`), `kodi.json`
//! ([`PackageInfo`]) and a generated `raygun-extension.json` (runtime `kodi`).

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::future::Future;
use std::io::{Cursor, Read};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use reqwest::Url;

use super::addon::{
    ADDON_DIR, Dependency, KodiSetting, LIBRARY_DIR, PACKAGE_INFO, PackageInfo, manifest_settings,
    read_po, read_settings, read_strings_xml,
};
use super::index::{Addon, addons, kinds, one_each, python3};
use crate::manifest::API_VERSION;
use crate::package::VerifiedPackage;
use crate::repo::RepoEntry;

/// Most an add-on and its libraries may hold, unpacked.
pub const MAX_ADDON_BYTES: u64 = 64 * 1024 * 1024;
/// Most libraries a plugin may pull in, theirs included.
const MAX_DEPENDENCIES: usize = 40;

/// What Raygun answers itself, so these aren't installed: Kodi's own parts, `requests` and
/// `urllib3` over Raygun's HTTP, the stream helper (Raygun's player opens HLS and DASH),
/// `StorageServer`'s cache, and the parts of Kodi a plugin may only need to exist.
const ANSWERED: &[&str] = &[
    "script.module.requests",
    "script.module.urllib3",
    "script.module.inputstreamhelper",
    "script.common.plugin.cache",
    "inputstream.adaptive",
    "inputstream.ffmpegdirect",
    "inputstream.rtmp",
];
/// Kinds of add-on a plugin may list among its needs that Raygun goes without: Kodi's own parts,
/// pictures and languages, services and menus running beside it.
const GONE_WITHOUT: &[&str] = &[
    "xbmc.",
    "kodi.",
    "resource.",
    "metadata.",
    "skin.",
    "service.",
    "context.",
    "pvr.",
    "repository.",
    "webinterface.",
    "screensaver.",
    "visualization.",
];
/// Plugins other plugins hand videos to, which Raygun plays itself.
pub const ANSWERED_PLUGINS: &[&str] = &["plugin.video.youtube"];
/// Files a plugin's folder holds that Raygun leaves out: pictures (Raygun shows only web
/// addresses) and compiled leftovers.
const LEFT_OUT: &[&str] = &[
    ".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".tbn", ".ico", ".psd", ".pyc", ".pyo",
    ".mp4", ".mkv", ".mp3", ".wav",
];
/// Compiled code Raygun can't run.
const COMPILED: &[&str] = &[".so", ".pyd", ".dll", ".dylib"];

/// One add-on's files, its top folder left out.
struct Unpacked {
    addon: Addon,
    files: Vec<(String, Vec<u8>)>,
    compiled: bool,
}

fn unzip(bytes: &[u8], budget: &mut u64) -> Result<Unpacked, String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| format!("the add-on isn't a readable zip: {error}"))?;
    let mut files = Vec::new();
    let mut addon_xml = None;
    let mut compiled = false;
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|error| format!("the add-on's zip is damaged: {error}"))?;
        if file.is_dir() || file.is_symlink() {
            continue;
        }
        let Some(path) = file.enclosed_name() else {
            return Err("the add-on's zip holds an unsafe path".into());
        };
        let parts: Vec<String> = path
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect();
        // Everything sits in the add-on's own folder (`plugin.video.x/...`).
        if parts.len() < 2 {
            continue;
        }
        let relative = parts[1..].join("/");
        let lower = relative.to_ascii_lowercase();
        if COMPILED.iter().any(|suffix| lower.ends_with(suffix)) {
            compiled = true;
            continue;
        }
        if LEFT_OUT.iter().any(|suffix| lower.ends_with(suffix))
            || lower.contains("__pycache__/")
            || lower.starts_with(".git")
        {
            continue;
        }
        let mut contents = Vec::new();
        (&mut file)
            .take(*budget + 1)
            .read_to_end(&mut contents)
            .map_err(|error| format!("the add-on's zip is damaged: {error}"))?;
        if contents.len() as u64 > *budget {
            return Err("the add-on and its libraries are too large".into());
        }
        *budget -= contents.len() as u64;
        if relative == "addon.xml" {
            addon_xml = Some(String::from_utf8_lossy(&contents).into_owned());
        }
        files.push((relative, contents));
    }
    let text = addon_xml.ok_or("the add-on's zip has no addon.xml")?;
    let addon = addons(&text)
        .into_iter()
        .next()
        .ok_or("the add-on's addon.xml can't be read")?;
    Ok(Unpacked {
        addon,
        files,
        compiled,
    })
}

fn gone_without(id: &str) -> bool {
    GONE_WITHOUT.iter().any(|prefix| id.starts_with(prefix))
}

/// Libraries known to hold compiled code (their zips ship it for each system), refused before
/// they're fetched.
fn compiled_library(id: &str) -> bool {
    matches!(
        id,
        "script.module.pil"
            | "script.module.pycryptodome"
            | "script.module.cryptography"
            | "script.module.lxml"
            | "script.module.numpy"
            | "script.module.pyopenssl"
            | "script.module.cffi"
    )
}

/// A repository's add-ons, each once as this device would install it ([`one_each`]): where each
/// one's zip is (relative to `base`, the index), by its id in lower case.
struct Listing {
    base: Url,
    packages: HashMap<String, String>,
}

/// How long a repository's listing is reused by later installs (Kodi's own is 12 MB of XML).
const LISTING_LIFE: Duration = Duration::from_secs(15 * 60);

/// Listings read lately (when, and what), by index address.
type Listings = Mutex<HashMap<String, (Instant, Arc<Listing>)>>;

fn listings() -> &'static Listings {
    static LISTINGS: OnceLock<Listings> = OnceLock::new();
    LISTINGS.get_or_init(Mutex::default)
}

/// Reads one plugin and what it needs into a package: `bytes` is its zip from `entry`'s
/// repository. `repositories` are the index addresses libraries come from, in turn (its own
/// repository first); `download` fetches an address, at most `limit` bytes.
pub async fn assemble<F, Fut>(
    entry: &RepoEntry,
    bytes: &[u8],
    repositories: &[Url],
    download: F,
) -> Result<VerifiedPackage, String>
where
    F: Fn(Url, u64) -> Fut,
    Fut: Future<Output = Result<Vec<u8>, String>>,
{
    let mut budget = MAX_ADDON_BYTES;
    let plugin = unzip(bytes, &mut budget)?;
    let addon = &plugin.addon;
    if super::index::extension_id(&addon.id).as_deref() != Some(entry.id.as_str()) {
        return Err(format!(
            "the repository lists {} but its zip holds {}",
            entry.id, addon.id
        ));
    }
    let provides = addon.plugin.clone().unwrap_or_default();
    let raygun_kinds = kinds(&provides);
    if raygun_kinds.is_empty() {
        return Err(crate::purpose::message(
            super::index::purpose(addon).unwrap_or("A part other Kodi add-ons use"),
        ));
    }
    let library = addon
        .library
        .clone()
        .ok_or("the plugin's addon.xml names no script")?;
    if !plugin.files.iter().any(|(name, _)| *name == library) {
        return Err(format!(
            "the plugin's script {library} is missing from its zip"
        ));
    }
    if plugin.compiled {
        return Err("This add-on holds compiled code, which Raygun can't run".into());
    }
    if let Some(python) = addon.requires.iter().find(|need| need.id == "xbmc.python")
        && !python3(python.version.as_deref())
    {
        return Err(
            "This add-on was written for Python 2 (Kodi 18 and older), which Raygun doesn't run"
                .into(),
        );
    }

    let mut files: Vec<(String, Vec<u8>)> = plugin
        .files
        .iter()
        .map(|(name, contents)| (format!("{ADDON_DIR}/{name}"), contents.clone()))
        .collect();
    let mut listings: Vec<Option<Arc<Listing>>> = repositories.iter().map(|_| None).collect();
    let mut dependencies: Vec<Dependency> = Vec::new();
    let mut answered: Vec<String> = Vec::new();
    let mut queue: VecDeque<(super::index::Requirement, String)> = addon
        .requires
        .iter()
        .cloned()
        .map(|need| (need, addon.name.clone()))
        .collect();
    let mut seen: Vec<String> = vec![addon.id.clone()];
    while let Some((need, wanted_by)) = queue.pop_front() {
        let id = need.id.trim().to_owned();
        if id.is_empty() || seen.contains(&id) {
            continue;
        }
        seen.push(id.clone());
        if ANSWERED.contains(&id.as_str()) || gone_without(&id) {
            continue;
        }
        if ANSWERED_PLUGINS.contains(&id.as_str()) {
            answered.push(id);
            continue;
        }
        if id.starts_with("plugin.") {
            // Another plugin it hands some items to: those won't play, the rest do.
            tracing::debug!(plugin = %addon.id, needs = %id, "a plugin another plugin uses is left out");
            continue;
        }
        if compiled_library(&id) {
            if need.optional {
                continue;
            }
            return Err(format!(
                "This add-on needs {id}, a library with compiled code, which Raygun can't run"
            ));
        }
        if dependencies.len() >= MAX_DEPENDENCIES {
            return Err("This add-on needs too many libraries".into());
        }
        let mut found = None;
        for (position, url) in repositories.iter().enumerate() {
            if listings[position].is_none() {
                match read_listing(url, &download).await {
                    Ok(listing) => listings[position] = Some(listing),
                    Err(error) => {
                        tracing::warn!(repository = %url, %error, "a Kodi repository couldn't be read for libraries");
                        continue;
                    }
                }
            }
            let Some(listing) = listings[position].as_ref() else {
                continue;
            };
            if let Some(package) = listing.packages.get(&id.to_ascii_lowercase()) {
                let zip = listing
                    .base
                    .join(package)
                    .map_err(|error| error.to_string())?;
                found = Some(zip);
                break;
            }
        }
        let Some(zip_url) = found else {
            if need.optional {
                continue;
            }
            return Err(format!(
                "{wanted_by} needs {id}, which neither its repository nor Kodi's has"
            ));
        };
        let zip_bytes = download(zip_url.clone(), budget).await?;
        let unpacked = unzip(&zip_bytes, &mut budget).map_err(|error| format!("{id}: {error}"))?;
        if unpacked.compiled {
            if need.optional {
                continue;
            }
            return Err(format!(
                "This add-on needs {id}, a library with compiled code, which Raygun can't run"
            ));
        }
        let library_dir = unpacked
            .addon
            .module_library
            .clone()
            .unwrap_or_else(|| "lib".to_owned());
        let settings = unpacked
            .files
            .iter()
            .find(|(name, _)| name == "resources/settings.xml")
            .map(|(_, xml)| {
                read_settings(&String::from_utf8_lossy(xml), &BTreeMap::new())
                    .into_iter()
                    .map(|setting| (setting.id, setting.default))
                    .collect()
            })
            .unwrap_or_default();
        dependencies.push(Dependency {
            id: id.clone(),
            name: if unpacked.addon.name.is_empty() {
                id.clone()
            } else {
                unpacked.addon.name.clone()
            },
            version: unpacked.addon.version.clone(),
            library: library_dir.trim_matches('/').to_owned(),
            settings,
        });
        for need in unpacked.addon.requires.iter().cloned() {
            queue.push_back((need, unpacked.addon.name.clone()));
        }
        files.extend(
            unpacked
                .files
                .into_iter()
                .map(|(name, contents)| (format!("{LIBRARY_DIR}/{id}/{name}"), contents)),
        );
    }

    let strings = strings_of(&plugin.files);
    let settings: Vec<KodiSetting> = plugin
        .files
        .iter()
        .find(|(name, _)| name == "resources/settings.xml")
        .map(|(_, xml)| read_settings(&String::from_utf8_lossy(xml), &strings))
        .unwrap_or_default();
    let info = PackageInfo {
        addon: addon.id.clone(),
        name: addon.name.clone(),
        version: addon.version.clone(),
        author: addon.provider.clone().unwrap_or_default(),
        summary: addon.blurb().unwrap_or_default(),
        library: library.clone(),
        provides: provides.split_whitespace().map(str::to_owned).collect(),
        icon: addon.icon.clone().unwrap_or_default(),
        fanart: addon.fanart.clone().unwrap_or_default(),
        dependencies,
        settings,
        answered,
    };
    let mut manifest = serde_json::json!({
        "id": entry.id,
        "name": if entry.name.trim().is_empty() { addon.id.as_str() } else { entry.name.as_str() },
        "version": entry.version,
        "apiVersion": API_VERSION,
        "runtime": "kodi",
        "entry": format!("{ADDON_DIR}/{library}"),
        "provides": raygun_kinds,
        "permissions": { "net": ["*"] },
        "settings": manifest_settings(&info.settings),
    });
    if let Some(text) = &entry.description {
        manifest["description"] = serde_json::json!(text);
    }
    if let Some(author) = &entry.author {
        manifest["author"] = serde_json::json!(author);
    }
    if let Some(website) = addon
        .website
        .as_ref()
        .filter(|site| site.starts_with("https://") || site.starts_with("http://"))
    {
        manifest["website"] = serde_json::json!(website);
    }
    files.push((
        PACKAGE_INFO.to_owned(),
        serde_json::to_vec_pretty(&info).map_err(|error| error.to_string())?,
    ));
    crate::package::from_files(&manifest.to_string(), files).map_err(|error| error.to_string())
}

/// The plugin's strings in English, for its settings' names.
fn strings_of(files: &[(String, Vec<u8>)]) -> BTreeMap<u32, String> {
    for name in [
        "resources/language/resource.language.en_gb/strings.po",
        "resources/language/resource.language.en_us/strings.po",
        "resources/language/English/strings.po",
    ] {
        if let Some((_, text)) = files.iter().find(|(path, _)| path == name) {
            return read_po(&String::from_utf8_lossy(text));
        }
    }
    files
        .iter()
        .find(|(path, _)| path == "resources/language/English/strings.xml")
        .map(|(_, text)| read_strings_xml(&String::from_utf8_lossy(text)))
        .unwrap_or_default()
}

async fn read_listing<F, Fut>(url: &Url, download: &F) -> Result<Arc<Listing>, String>
where
    F: Fn(Url, u64) -> Fut,
    Fut: Future<Output = Result<Vec<u8>, String>>,
{
    if let Some((at, listing)) = listings()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(url.as_str())
        && at.elapsed() < LISTING_LIFE
    {
        return Ok(listing.clone());
    }
    let bytes = download(url.clone(), 32 * 1024 * 1024).await?;
    let text = super::index::unpack(&bytes)?;
    let found = addons(&text);
    if found.is_empty() {
        return Err("it lists no add-ons".into());
    }
    let listing = Arc::new(Listing {
        base: url.clone(),
        packages: one_each(found)
            .into_iter()
            .map(|addon| (addon.id.to_ascii_lowercase(), addon.package()))
            .collect(),
    });
    let mut kept = listings().lock().unwrap_or_else(PoisonError::into_inner);
    kept.retain(|_, (at, _)| at.elapsed() < LISTING_LIFE);
    kept.insert(url.to_string(), (Instant::now(), listing.clone()));
    Ok(listing)
}

/// The repositories a plugin's libraries come from: its own, then Kodi's.
pub fn library_sources(own: &Url) -> Vec<Url> {
    let mut sources = vec![own.clone()];
    if let Ok(official) = Url::parse(super::OFFICIAL_REPOSITORY)
        && official != *own
        && !own
            .as_str()
            .starts_with("https://mirrors.kodi.tv/addons/omega/")
    {
        sources.push(official);
    }
    sources
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    /// A zip holding one add-on's folder.
    fn zipped(folder: &str, files: &[(&str, &str)]) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut bytes));
            for (name, text) in files {
                zip.start_file(
                    format!("{folder}/{name}"),
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
                zip.write_all(text.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
        }
        bytes
    }

    const PLUGIN: &str = r#"<addon id="plugin.video.example.films" name="Films" version="1.0.0">
  <requires><import addon="xbmc.python" version="3.0.0"/><import addon="script.module.example" version="1.0.0"/></requires>
  <extension point="xbmc.python.pluginsource" library="default.py"><provides>video</provides></extension>
</addon>"#;

    /// A library listed for every system and for one other, in versions for Python 2 and 3.
    const LIBRARY_VERSIONS: &str = r#"
<addon id="script.module.example" name="Example library" version="1.0.0">
  <requires><import addon="xbmc.python" version="3.0.0"/></requires>
  <extension point="xbmc.python.module" library="lib"/>
  <extension point="xbmc.addon.metadata"><platform>all</platform></extension>
</addon>
<addon id="script.module.example" name="Example library" version="1.2.0">
  <requires><import addon="xbmc.python" version="3.0.0"/></requires>
  <extension point="xbmc.python.module" library="lib"/>
  <extension point="xbmc.addon.metadata">
    <platform>all</platform>
    <path>script.module.example+all/script.module.example-1.2.0.zip</path>
  </extension>
</addon>
<addon id="script.module.example" name="Example library" version="3.0.0">
  <requires><import addon="xbmc.python" version="2.25.0"/></requires>
  <extension point="xbmc.python.module" library="lib"/>
  <extension point="xbmc.addon.metadata"><platform>all</platform></extension>
</addon>
<addon id="script.module.example" name="Example library" version="9.0.0">
  <requires><import addon="xbmc.python" version="3.0.0"/></requires>
  <extension point="xbmc.python.module" library="lib"/>
  <extension point="xbmc.addon.metadata"><platform>plan9-sparc</platform></extension>
</addon>"#;

    #[tokio::test]
    async fn libraries_come_as_this_system_would_install_them() {
        let repository: Url = "https://kodi.example.org/repeated-libraries/addons.xml"
            .parse()
            .unwrap();
        let index = format!("<addons>{PLUGIN}{LIBRARY_VERSIONS}</addons>");
        let library = r#"<addon id="script.module.example" name="Example library" version="1.2.0">
  <requires><import addon="xbmc.python" version="3.0.0"/></requires>
  <extension point="xbmc.python.module" library="lib"/>
</addon>"#;
        // Only the version this system installs is there to fetch.
        let served: HashMap<String, Vec<u8>> = [
            (repository.to_string(), index.clone().into_bytes()),
            (
                "https://kodi.example.org/repeated-libraries/script.module.example+all/script.module.example-1.2.0.zip".to_owned(),
                zipped(
                    "script.module.example",
                    &[("addon.xml", library), ("lib/example.py", "VALUE = 12\n")],
                ),
            ),
        ]
        .into_iter()
        .collect();
        let listed = super::super::index::parse(&index).unwrap();
        assert_eq!(listed.extensions.len(), 1, "the library is left out");
        let plugin = zipped(
            "plugin.video.example.films",
            &[("addon.xml", PLUGIN), ("default.py", "import example\n")],
        );
        let verified = assemble(
            &listed.extensions[0],
            &plugin,
            std::slice::from_ref(&repository),
            |url, _| {
                let answer = served
                    .get(url.as_str())
                    .cloned()
                    .ok_or_else(|| format!("{url}: not found"));
                async move { answer }
            },
        )
        .await
        .unwrap();
        let file = |name: &str| {
            verified
                .files()
                .iter()
                .find(|(path, _)| path == name)
                .map(|(_, contents)| contents.clone())
        };
        let info: PackageInfo = serde_json::from_slice(&file(PACKAGE_INFO).unwrap()).unwrap();
        let versions: Vec<&str> = info
            .dependencies
            .iter()
            .map(|library| library.version.as_str())
            .collect();
        assert_eq!(
            versions,
            ["1.2.0"],
            "the highest version made for this system that Raygun's Python runs"
        );
        assert_eq!(
            file("lib/script.module.example/lib/example.py").as_deref(),
            Some(b"VALUE = 12\n".as_slice())
        );
    }
}
