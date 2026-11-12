//! Kodi repositories (kodi.wiki/view/Add-on_repositories): `addons.xml` (often gzipped as
//! `addons.xml.gz`) lists every add-on's `addon.xml`, with the add-ons' folders beside it
//! (`<id>/<id>-<version>.zip`). A repository is often shared as its own add-on instead (a
//! `repository.*` zip, or its `addon.xml`), whose `xbmc.addon.repository` extension names the
//! index.
//!
//! Kodi's add-ons are Python programs run against Kodi's own modules (`xbmc`, `xbmcgui`,
//! `xbmcplugin`, `xbmcaddon`, `xbmcvfs`) and the `script.module.*` libraries they import, which
//! Raygun doesn't have. So a repository is read and its plugins (video, music and picture
//! sources) are listed with what they offer, marked as not installable
//! ([`UNSUPPORTED`]). Skins, services, programs, screensavers and the like are listed as not for
//! Raygun, saying what they do in Kodi ([`PURPOSES`]); libraries, resources (languages, images,
//! sounds), repositories and the player's own parts are left out, as what other add-ons need
//! rather than add-ons people pick.

use std::io::{Cursor, Read};

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use reqwest::Url;

use crate::manifest::{API_VERSION, is_extension_id};
use crate::repo::{INDEX_FORMAT, PackageFormat, RepoEntry, RepoIndex, RepoPermissions};

/// Why a Kodi add-on can't be installed, as the catalogue says it.
pub const UNSUPPORTED: &str = "Kodi add-ons are Python programs, which Raygun can't run yet";
/// Largest index read once unpacked (Kodi's own repository is about 12 MB).
const MAX_INDEX_BYTES: u64 = 32 * 1024 * 1024;

/// What Kodi add-ons that aren't content sources do, by their extension point (or the start of
/// it), as the catalogue says why they're not for Raygun.
const PURPOSES: &[(&str, &str)] = &[
    (
        "xbmc.gui.skin",
        "A skin for Kodi's own screens; Raygun has its own look",
    ),
    ("xbmc.python.script", "A program that runs inside Kodi"),
    ("xbmc.service", "A background service for Kodi"),
    ("xbmc.ui.screensaver", "A screensaver for Kodi"),
    (
        "xbmc.player.musicviz",
        "A visualization for Kodi's music player",
    ),
    ("xbmc.python.weather", "Weather for Kodi's home screen"),
    (
        "xbmc.subtitle.module",
        "Finds subtitles for Kodi's player; Raygun's come from its own sources",
    ),
    (
        "xbmc.python.lyrics",
        "Finds lyrics for Kodi's player; Raygun's come from its own sources",
    ),
    ("kodi.context.item", "Adds items to Kodi's menus"),
    (
        "xbmc.webinterface",
        "A web remote for Kodi; Raygun has its own remote",
    ),
    (
        "xbmc.metadata.scraper.",
        "Fills Kodi's library with details; Raygun gets them from its own sources",
    ),
    ("xbmc.pvrclient", "Live TV for Kodi's TV screens"),
    ("kodi.pvrclient", "Live TV for Kodi's TV screens"),
    ("kodi.gameclient", "A game emulator for Kodi"),
];
/// A plugin whose `<provides>` names no content (`executable`, `game`): a program in Kodi.
const PROGRAM: &str = "A program that runs inside Kodi";
/// An add-on on its own that other add-ons use (a library, a language, pictures).
const PART: &str = "A part other Kodi add-ons use, not an add-on of its own";

/// One `<addon>`, as far as Raygun reads it.
#[derive(Debug, Default)]
struct Addon {
    id: String,
    name: String,
    version: String,
    provider: Option<String>,
    /// Its extension points, in order (`xbmc.python.pluginsource`, `xbmc.gui.skin`...).
    points: Vec<String>,
    /// The `xbmc.python.pluginsource` extension's `<provides>`: `video`, `audio`, `image`...
    plugin: Option<String>,
    /// The `xbmc.addon.repository` extension's index (`<info>`), for repository add-ons.
    repository_info: Option<String>,
    summary: Option<(bool, String)>,
    description: Option<(bool, String)>,
    language: Option<String>,
    icon: Option<String>,
}

/// Text that is a Kodi document: its root is `<addons>` (an index) or `<addon>` (one add-on).
fn root_element(text: &str) -> Option<String> {
    let mut reader = Reader::from_str(text);
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer).ok()? {
            Event::Start(element) | Event::Empty(element) => {
                return Some(element.name().as_ref().to_ascii_lowercase());
            }
            Event::Eof => return None,
            _ => {}
        }
        buffer.clear();
    }
}

fn attribute(element: &BytesStart<'_>, key: &str) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.as_ref().eq_ignore_ascii_case(key))
        .and_then(|attribute| {
            attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .ok()
                .map(|value| value.trim().to_owned())
        })
        .filter(|value| !value.is_empty())
}

/// Every `<addon>` of a document.
fn addons(text: &str) -> Vec<Addon> {
    let mut reader = Reader::from_str(text);
    let mut buffer = Vec::new();
    let mut found = Vec::new();
    let mut current: Option<Addon> = None;
    let mut point = String::new();
    let mut lang_english = true;
    let mut path: Vec<String> = Vec::new();
    let mut text_buffer = String::new();
    while let Ok(event) = reader.read_event_into(&mut buffer) {
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(element) | Event::Empty(element) => {
                let name = element.name().as_ref().to_ascii_lowercase();
                match name.as_str() {
                    "addon" if path.last().is_none_or(|parent| parent == "addons") => {
                        current = Some(Addon {
                            id: attribute(&element, "id").unwrap_or_default(),
                            name: attribute(&element, "name").unwrap_or_default(),
                            version: attribute(&element, "version").unwrap_or_default(),
                            provider: attribute(&element, "provider-name"),
                            ..Addon::default()
                        });
                    }
                    "extension" => {
                        point = attribute(&element, "point").unwrap_or_default();
                        if let Some(addon) = current.as_mut()
                            && !point.is_empty()
                        {
                            addon.points.push(point.clone());
                        }
                    }
                    "summary" | "description" => {
                        lang_english = attribute(&element, "lang")
                            .is_none_or(|lang| lang.to_ascii_lowercase().starts_with("en"));
                    }
                    _ => {}
                }
                if !empty {
                    path.push(name);
                    text_buffer.clear();
                }
            }
            Event::Text(content) => text_buffer.push_str(&content),
            Event::CData(content) => text_buffer.push_str(&content),
            Event::GeneralRef(reference) => {
                let escaped = format!("&{};", &*reference);
                text_buffer.push_str(
                    &quick_xml::escape::unescape(&escaped)
                        .map(std::borrow::Cow::into_owned)
                        .unwrap_or(escaped),
                );
            }
            Event::End(_) => {
                let name = path.pop().unwrap_or_default();
                let value = std::mem::take(&mut text_buffer).trim().to_owned();
                let Some(addon) = current.as_mut() else {
                    continue;
                };
                match name.as_str() {
                    "addon" => {
                        if let Some(done) = current.take() {
                            found.push(done);
                        }
                    }
                    "provides" if point == "xbmc.python.pluginsource" => {
                        addon.plugin = Some(value);
                    }
                    "info"
                        if point == "xbmc.addon.repository" && addon.repository_info.is_none() =>
                    {
                        addon.repository_info = Some(value).filter(|value| !value.is_empty());
                    }
                    "summary" | "description" if !value.is_empty() => {
                        let slot = if name == "summary" {
                            &mut addon.summary
                        } else {
                            &mut addon.description
                        };
                        // English first, else the first one there is.
                        if slot
                            .as_ref()
                            .is_none_or(|(english, _)| !english && lang_english)
                        {
                            *slot = Some((lang_english, value));
                        }
                    }
                    "language" if !value.is_empty() => addon.language = Some(value),
                    "icon" if !value.is_empty() => addon.icon = Some(value),
                    "extension" => point.clear(),
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    found
}

/// A Kodi version as semver: `5.0.2+matrix.1` is one already; `1.2~beta3` becomes `1.2.0`.
fn semver_version(version: &str) -> String {
    if semver::Version::parse(version).is_ok() {
        return version.to_owned();
    }
    let numbers: Vec<u64> = version
        .split(|c: char| !c.is_ascii_digit() && c != '.')
        .next()
        .unwrap_or_default()
        .split('.')
        .filter_map(|part| part.parse().ok())
        .take(3)
        .collect();
    format!(
        "{}.{}.{}",
        numbers.first().copied().unwrap_or_default(),
        numbers.get(1).copied().unwrap_or_default(),
        numbers.get(2).copied().unwrap_or_default()
    )
}

/// The Raygun kinds a plugin would provide, by what it says it provides.
fn kinds(provides: &str) -> Vec<String> {
    let mut kinds = Vec::new();
    for word in provides.split_whitespace() {
        let kind = match word {
            "video" => "video.source",
            "audio" => "music.stream",
            "image" => "manga.source",
            _ => continue,
        };
        if !kinds.iter().any(|known| known == kind) {
            kinds.push(kind.to_owned());
        }
    }
    kinds
}

/// What an add-on that isn't a content source does in Kodi, when it's one people pick (a skin, a
/// service, a program); `None` for a source, and for libraries, resources, repositories and the
/// player's own parts.
fn purpose(addon: &Addon) -> Option<&'static str> {
    if addon
        .plugin
        .as_deref()
        .is_some_and(|provides| !kinds(provides).is_empty())
    {
        return None;
    }
    addon.points.iter().find_map(|point| {
        if point == "xbmc.python.pluginsource" {
            return Some(PROGRAM);
        }
        PURPOSES
            .iter()
            .find(|(known, _)| {
                point == known
                    || (known.ends_with('.')
                        && point.starts_with(known)
                        && !point.ends_with(".library"))
            })
            .map(|(_, purpose)| *purpose)
    })
}

/// Reads an `addons.xml`; `None` when the text isn't one.
pub fn parse(text: &str) -> Option<RepoIndex> {
    if root_element(text)? != "addons" {
        return None;
    }
    let extensions = addons(text)
        .into_iter()
        .filter_map(|addon| {
            let kinds = kinds(addon.plugin.as_deref().unwrap_or_default());
            // Not a source: listed as not for Raygun when it's an add-on people pick.
            let not_for_raygun = if kinds.is_empty() {
                Some(purpose(&addon)?.to_owned())
            } else {
                None
            };
            if addon.id.contains(['/', '\\']) || addon.id.is_empty() {
                return None;
            }
            let label: String = addon
                .id
                .to_ascii_lowercase()
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '.' {
                        c
                    } else {
                        '-'
                    }
                })
                .collect();
            let id = format!("kodi.{}", label.trim_matches(['-', '.']));
            if !is_extension_id(&id) {
                return None;
            }
            let description = match (addon.summary, addon.description) {
                (Some((_, summary)), _) => Some(summary),
                (None, Some((_, description))) => Some(description),
                (None, None) => None,
            };
            let folder = addon.id.clone();
            Some(RepoEntry {
                id,
                name: if addon.name.is_empty() {
                    addon.id.clone()
                } else {
                    addon.name
                },
                version: semver_version(&addon.version),
                api_version: API_VERSION,
                description: description.map(|text| text.chars().take(500).collect()),
                author: addon.provider,
                provides: kinds,
                permissions: RepoPermissions::default(),
                content_rating: "safe".to_owned(),
                lang: addon
                    .language
                    .and_then(|languages| languages.split_whitespace().next().map(str::to_owned)),
                package: format!("{folder}/{folder}-{}.zip", addon.version),
                icon: addon
                    .icon
                    .map(|icon| format!("{folder}/{}", icon.trim_start_matches('/'))),
                sha256: String::new(),
                size: 0,
                format: PackageFormat::Kodi,
                sites: Vec::new(),
                not_for_raygun,
            })
        })
        .collect();
    Some(RepoIndex {
        format_version: INDEX_FORMAT,
        // `addons.xml` doesn't name its repository: the app names it after its address.
        name: String::new(),
        extensions,
        libraries: Vec::new(),
    })
}

/// One add-on's `addon.xml`: the file itself, or the one inside its zip. `None` for anything
/// else.
fn addon_text(bytes: &[u8]) -> Option<String> {
    let text = if bytes.starts_with(b"PK") {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
        let name = (0..zip.len()).find_map(|index| {
            let file = zip.by_index(index).ok()?;
            let name = file.name().ok()?.into_owned();
            (name.ends_with("/addon.xml") && name.matches('/').count() == 1).then_some(name)
        })?;
        let mut file = zip.by_name(&name).ok()?;
        let mut text = String::new();
        (&mut file)
            .take(1024 * 1024)
            .read_to_string(&mut text)
            .ok()?;
        text
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };
    (root_element(&text)? == "addon").then_some(text)
}

/// The index a repository shared as its own add-on names: from its `addon.xml`, or from the
/// `addon.xml` inside its zip. `None` for anything else.
pub fn index_address(bytes: &[u8], from: &Url) -> Option<Url> {
    let text = addon_text(bytes)?;
    let info = addons(&text)
        .into_iter()
        .find_map(|addon| addon.repository_info)?;
    let url = from.join(&info).ok()?;
    matches!(url.scheme(), "http" | "https").then_some(url)
}

/// Why one add-on shared on its own (its `addon.xml` or zip) can't be added: a source is a
/// Python program Raygun can't run ([`UNSUPPORTED`]); anything else isn't for Raygun, saying
/// what it is. `None` for a repository's own add-on (its index is read instead) and for anything
/// that isn't a Kodi add-on.
pub fn refusal(bytes: &[u8]) -> Option<String> {
    let addon = addons(&addon_text(bytes)?).into_iter().next()?;
    if addon.repository_info.is_some() {
        return None;
    }
    let source = addon
        .plugin
        .as_deref()
        .is_some_and(|provides| !kinds(provides).is_empty());
    Some(if source {
        UNSUPPORTED.to_owned()
    } else {
        crate::purpose::message(purpose(&addon).unwrap_or(PART))
    })
}

/// An index's text, unpacked when it's gzipped (`addons.xml.gz`).
pub fn unpack(bytes: &[u8]) -> Result<String, String> {
    if !bytes.starts_with(&[0x1f, 0x8b]) {
        return Ok(String::from_utf8_lossy(bytes).into_owned());
    }
    let mut text = Vec::new();
    flate2::read::GzDecoder::new(bytes)
        .take(MAX_INDEX_BYTES + 1)
        .read_to_end(&mut text)
        .map_err(|error| format!("the index isn't a readable gzip file: {error}"))?;
    if text.len() as u64 > MAX_INDEX_BYTES {
        return Err("the index is too large".into());
    }
    Ok(String::from_utf8_lossy(&text).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shaped like Kodi's own repository (mirrors.kodi.tv/addons/omega/addons.xml).
    const INDEX: &str = r#"<?xml version='1.0' encoding='utf-8'?>
<addons>
<addon id="plugin.video.example.archive" name="Example Archive" version="2.1.0+omega.1" provider-name="Example Team">
  <requires><import addon="xbmc.python" version="3.0.1"/></requires>
  <extension point="xbmc.python.pluginsource" library="addon.py"><provides>video</provides></extension>
  <extension point="xbmc.addon.metadata">
    <summary lang="de_DE">Filme aus dem Archiv</summary>
    <summary lang="en_GB">Films from the archive &amp; more</summary>
    <description lang="en_GB">Public-domain films.</description>
    <language>en de</language>
    <assets><icon>resources/icon.png</icon></assets>
  </extension>
</addon>
<addon id="plugin.audio.example.radio" name="Radio" version="1.0~beta2">
  <extension point="xbmc.python.pluginsource" library="default.py"><provides>audio video</provides></extension>
</addon>
<addon id="script.module.requests" name="requests" version="2.31.0">
  <extension point="xbmc.python.module" library="lib"/>
</addon>
<addon id="skin.estuary" name="Estuary" version="4.0.0"><extension point="xbmc.gui.skin"/></addon>
</addons>"#;

    #[test]
    fn lists_plugins_as_not_installable() {
        let index = crate::repo::parse_index(INDEX).unwrap();
        assert!(index.name.is_empty(), "named after its address");
        let ids: Vec<(&str, Option<&str>)> = index
            .extensions
            .iter()
            .map(|entry| (entry.id.as_str(), entry.not_for_raygun.as_deref()))
            .collect();
        assert_eq!(
            ids,
            [
                ("kodi.plugin.video.example.archive", None),
                ("kodi.plugin.audio.example.radio", None),
                (
                    "kodi.skin.estuary",
                    Some("A skin for Kodi's own screens; Raygun has its own look")
                ),
            ],
            "libraries are left out; a skin isn't for Raygun"
        );
        let video = &index.extensions[0];
        assert_eq!(video.format, PackageFormat::Kodi);
        assert_eq!(video.version, "2.1.0+omega.1");
        assert_eq!(video.provides, ["video.source"]);
        assert_eq!(
            video.description.as_deref(),
            Some("Films from the archive & more")
        );
        assert_eq!(video.lang.as_deref(), Some("en"));
        assert_eq!(video.author.as_deref(), Some("Example Team"));
        let base: Url = "https://kodi.example.org/repo/addons.xml".parse().unwrap();
        assert_eq!(
            crate::repo::icon_url(&base, video).unwrap().as_str(),
            "https://kodi.example.org/repo/plugin.video.example.archive/resources/icon.png"
        );
        let radio = &index.extensions[1];
        assert_eq!(radio.version, "1.0.0");
        assert_eq!(radio.provides, ["music.stream", "video.source"]);
        let skin = &index.extensions[2];
        assert!(skin.provides.is_empty());
        assert_eq!(skin.format, PackageFormat::Kodi);
    }

    /// Add-ons that aren't sources, as Kodi's own repository lists them.
    const OTHERS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<addons>
<addon id="service.subtitles.example" name="Example Subtitles" version="1.0.0">
  <extension point="xbmc.subtitle.module" library="service.py"/>
  <extension point="xbmc.addon.metadata"><summary lang="en_GB">Subtitles</summary></extension>
</addon>
<addon id="service.example.backup" name="Backup" version="2.0.1">
  <extension point="xbmc.service" library="service.py"/>
</addon>
<addon id="script.example.tool" name="Tool" version="1.0.0">
  <extension point="xbmc.python.script" library="default.py"><provides>executable</provides></extension>
</addon>
<addon id="plugin.program.example" name="Program" version="1.0.0">
  <extension point="xbmc.python.pluginsource" library="default.py"><provides>executable</provides></extension>
</addon>
<addon id="screensaver.example" name="Stars" version="1.0.0"><extension point="xbmc.ui.screensaver" library="s.py"/></addon>
<addon id="metadata.example.movies" name="Films DB" version="1.0.0"><extension point="xbmc.metadata.scraper.movies" library="s.xml"/></addon>
<addon id="metadata.common.example" name="Scraper library" version="1.0.0"><extension point="xbmc.metadata.scraper.library" library="s.xml"/></addon>
<addon id="context.example" name="Menu item" version="1.0.0"><extension point="kodi.context.item"/></addon>
<addon id="resource.language.de_de" name="German" version="11.0.0"><extension point="kodi.resource.language"/></addon>
<addon id="repository.example" name="Example repository" version="1.0.0"><extension point="xbmc.addon.repository"><dir><info>https://kodi.example.org/addons.xml</info></dir></extension></addon>
</addons>"#;

    #[test]
    fn skins_services_and_programs_are_not_for_raygun() {
        let index = crate::repo::parse_index(OTHERS).unwrap();
        let listed: Vec<(&str, &str)> = index
            .extensions
            .iter()
            .map(|entry| {
                assert!(entry.provides.is_empty(), "{} calls nothing", entry.id);
                (
                    entry.id.as_str(),
                    entry.not_for_raygun.as_deref().unwrap_or_default(),
                )
            })
            .collect();
        assert_eq!(
            listed,
            [
                (
                    "kodi.service.subtitles.example",
                    "Finds subtitles for Kodi's player; Raygun's come from its own sources"
                ),
                (
                    "kodi.service.example.backup",
                    "A background service for Kodi"
                ),
                (
                    "kodi.script.example.tool",
                    "A program that runs inside Kodi"
                ),
                (
                    "kodi.plugin.program.example",
                    "A program that runs inside Kodi"
                ),
                ("kodi.screensaver.example", "A screensaver for Kodi"),
                (
                    "kodi.metadata.example.movies",
                    "Fills Kodi's library with details; Raygun gets them from its own sources"
                ),
                ("kodi.context.example", "Adds items to Kodi's menus"),
            ],
            "libraries, languages and repositories are what other add-ons need: left out"
        );
    }

    #[test]
    fn single_addons_say_why_they_cant_be_added() {
        let skin = r#"<?xml version="1.0"?><addon id="skin.example" name="Example" version="1.0.0">
<extension point="xbmc.gui.skin"/></addon>"#;
        assert_eq!(
            refusal(skin.as_bytes()).as_deref(),
            Some("Not for Raygun: A skin for Kodi's own screens; Raygun has its own look")
        );
        let plugin = r#"<addon id="plugin.video.example" name="Films" version="1.0.0">
<extension point="xbmc.python.pluginsource" library="addon.py"><provides>video</provides></extension></addon>"#;
        let mut zipped = Vec::new();
        {
            use std::io::Write;
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut zipped));
            zip.start_file(
                "plugin.video.example/addon.xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(plugin.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        assert_eq!(refusal(&zipped).as_deref(), Some(UNSUPPORTED));
        let module = r#"<addon id="script.module.example" name="Library" version="1.0.0">
<extension point="xbmc.python.module" library="lib"/></addon>"#;
        assert_eq!(
            refusal(module.as_bytes()),
            Some(crate::purpose::message(PART))
        );
        let repository = r#"<addon id="repository.example" name="Example" version="1.0.0">
<extension point="xbmc.addon.repository"><dir><info>https://kodi.example.org/addons.xml</info></dir></extension></addon>"#;
        assert_eq!(refusal(repository.as_bytes()), None, "its index is read");
        assert_eq!(refusal(INDEX.as_bytes()), None, "an index is no add-on");
        assert_eq!(refusal(b"{}"), None);
    }

    #[test]
    fn repository_addons_name_their_index() {
        let addon = r#"<?xml version="1.0"?><addon id="repository.example" name="Example" version="1.0.0">
<extension point="xbmc.addon.repository" name="Example">
<dir><info compressed="true">https://kodi.example.org/repo/addons.xml.gz</info><checksum>https://kodi.example.org/repo/addons.xml.gz.md5</checksum><datadir zip="true">https://kodi.example.org/repo/</datadir></dir>
</extension></addon>"#;
        let from: Url = "https://kodi.example.org/repository.example/addon.xml"
            .parse()
            .unwrap();
        assert_eq!(
            index_address(addon.as_bytes(), &from).unwrap().as_str(),
            "https://kodi.example.org/repo/addons.xml.gz"
        );
        let mut zipped = Vec::new();
        {
            use std::io::Write;
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut zipped));
            zip.start_file(
                "repository.example/addon.xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(addon.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        assert!(
            index_address(&zipped, &from).is_some(),
            "the repository's zip too"
        );
        assert!(
            index_address(INDEX.as_bytes(), &from).is_none(),
            "an index is no add-on"
        );
        assert!(parse(addon).is_none());
    }

    /// Kodi's own repository (mirrors.kodi.tv), gzipped as Kodi reads it.
    #[tokio::test]
    #[ignore = "network"]
    async fn kodi_official_live() {
        let client = raygun_net::http_client().unwrap();
        let bytes = client
            .get("https://mirrors.kodi.tv/addons/omega/addons.xml.gz")
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        let index = crate::repo::parse_index(&unpack(&bytes).unwrap()).unwrap();
        let videos = index
            .extensions
            .iter()
            .filter(|entry| entry.provides.iter().any(|kind| kind == "video.source"))
            .count();
        let others = index
            .extensions
            .iter()
            .filter(|entry| entry.not_for_raygun.is_some())
            .count();
        println!(
            "{} add-ons, {videos} for video, {others} not for Raygun",
            index.extensions.len()
        );
        assert!(videos > 10);
    }

    #[test]
    fn gzipped_indexes_unpack() {
        use std::io::Write;
        let mut packed = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        packed.write_all(INDEX.as_bytes()).unwrap();
        let packed = packed.finish().unwrap();
        assert_eq!(unpack(&packed).unwrap(), INDEX);
        assert_eq!(unpack(b"<addons/>").unwrap(), "<addons/>");
    }
}
