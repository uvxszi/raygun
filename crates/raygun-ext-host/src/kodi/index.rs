//! Kodi repositories (kodi.wiki/view/Add-on_repositories): `addons.xml` (often gzipped as
//! `addons.xml.gz`) lists every add-on's `addon.xml`, with the add-ons' folders beside it
//! (`<id>/<id>-<version>.zip`). A repository is often shared as its own add-on instead (a
//! `repository.*` zip, or its `addon.xml`), whose `xbmc.addon.repository` extension names the
//! index.
//!
//! Plugins (video and music sources) are listed with what they offer, and install through
//! [`super::package`]: Raygun runs their Python ([`super::runtime`]). Skins, services, programs,
//! screensavers and the like are listed as not for Raygun, saying what they do in Kodi
//! ([`PURPOSES`]); libraries, resources (languages, images, sounds), repositories and the
//! player's own parts are left out, as what other add-ons need rather than add-ons people pick.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read};

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use reqwest::Url;

use crate::manifest::{API_VERSION, is_extension_id};
use crate::repo::{INDEX_FORMAT, PackageFormat, RepoEntry, RepoIndex, RepoPermissions};

/// Why a Kodi plugin shared on its own (its zip, not its repository) isn't installed: what it
/// needs comes from its repository.
pub const UNSUPPORTED: &str =
    "Add the Kodi repository this add-on comes from, then install it from there";
/// What a picture plugin is, as the catalogue says why it's not for Raygun.
const PICTURES: &str = "Pictures for Kodi's picture viewer";
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

/// An add-on another needs (`<requires><import addon=… version=… optional=…/>`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Requirement {
    pub id: String,
    pub version: Option<String>,
    pub optional: bool,
}

/// One `<addon>`, as far as Raygun reads it.
#[derive(Debug, Default)]
pub(crate) struct Addon {
    pub id: String,
    pub name: String,
    pub version: String,
    pub provider: Option<String>,
    /// Its extension points, in order (`xbmc.python.pluginsource`, `xbmc.gui.skin`...).
    pub points: Vec<String>,
    /// The `xbmc.python.pluginsource` extension's `<provides>`: `video`, `audio`, `image`...
    pub plugin: Option<String>,
    /// The `xbmc.python.pluginsource` extension's script (`library`), relative to the add-on.
    pub library: Option<String>,
    /// The `xbmc.python.module` extension's folder (`library`), which goes on `sys.path`.
    pub module_library: Option<String>,
    /// The `xbmc.addon.repository` extension's index (`<info>`), for repository add-ons.
    pub repository_info: Option<String>,
    pub requires: Vec<Requirement>,
    pub summary: Option<(bool, String)>,
    pub description: Option<(bool, String)>,
    pub language: Option<String>,
    pub icon: Option<String>,
    pub fanart: Option<String>,
    pub website: Option<String>,
    /// `<platform>`: `all`, or the systems it's built for (`android-aarch64 windows-x86_64`...).
    pub platform: Option<String>,
    /// `<path>`: where its zip is in the repository, when the index says (Kodi's own repository
    /// keeps each system's build of a compiled add-on in a folder of its own,
    /// `pvr.example+android-aarch64/pvr.example-1.0.0.zip`).
    pub path: Option<String>,
}

impl Addon {
    /// Its summary, else its description (English first).
    pub fn blurb(&self) -> Option<String> {
        self.summary
            .as_ref()
            .or(self.description.as_ref())
            .map(|(_, text)| text.clone())
    }

    /// Where its zip is, relative to the index: the index's `<path>` when it's a plain relative
    /// one, else `<id>/<id>-<version>.zip`, as Kodi looks for it.
    pub fn package(&self) -> String {
        self.path
            .as_deref()
            .filter(|path| plain_relative(path))
            .map_or_else(
                || format!("{0}/{0}-{1}.zip", self.id, self.version),
                str::to_owned,
            )
    }

    /// The folder its zip is in, where the repository keeps its pictures too (`icon.png`).
    fn folder(&self) -> String {
        let package = self.package();
        package
            .rsplit_once('/')
            .map_or_else(String::new, |(folder, _)| folder.to_owned())
    }
}

/// A path that stays below the index's folder: relative, without `..`, a scheme or a query.
fn plain_relative(path: &str) -> bool {
    !path.contains(['\\', ':', '?', '#'])
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
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
pub(crate) fn addons(text: &str) -> Vec<Addon> {
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
                            let library = attribute(&element, "library");
                            match point.as_str() {
                                "xbmc.python.pluginsource" if addon.library.is_none() => {
                                    addon.library = library;
                                }
                                "xbmc.python.module" if addon.module_library.is_none() => {
                                    addon.module_library = library;
                                }
                                _ => {}
                            }
                        }
                    }
                    "import" if path.last().is_some_and(|parent| parent == "requires") => {
                        if let Some(addon) = current.as_mut()
                            && let Some(id) = attribute(&element, "addon")
                        {
                            addon.requires.push(Requirement {
                                id,
                                version: attribute(&element, "version"),
                                optional: attribute(&element, "optional")
                                    .is_some_and(|value| value.eq_ignore_ascii_case("true")),
                            });
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
                    "fanart" if !value.is_empty() => addon.fanart = Some(value),
                    "website" if !value.is_empty() => addon.website = Some(value),
                    "platform" if !value.is_empty() => addon.platform = Some(value),
                    "path" if point == "xbmc.addon.metadata" && !value.is_empty() => {
                        addon.path = Some(value);
                    }
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

/// Kodi's order of add-on versions (`CAddonVersion`): an epoch (`1:`) first, then the rest as
/// Debian compares versions, which Kodi's follows: numbers as numbers, other characters one by
/// one with letters first, and `~` before anything, even the end (`1.0~beta1` < `1.0`).
pub(crate) fn version_order(a: &str, b: &str) -> Ordering {
    let (epoch_a, rest_a) = epoch(a);
    let (epoch_b, rest_b) = epoch(b);
    epoch_a
        .cmp(&epoch_b)
        .then_with(|| debian_order(rest_a.as_bytes(), rest_b.as_bytes()))
}

fn epoch(version: &str) -> (u64, &str) {
    match version.trim().split_once(':') {
        Some((epoch, rest)) if !epoch.is_empty() && epoch.bytes().all(|c| c.is_ascii_digit()) => {
            (epoch.parse().unwrap_or(u64::MAX), rest)
        }
        _ => (0, version.trim()),
    }
}

/// dpkg's `verrevcmp`.
fn debian_order(mut a: &[u8], mut b: &[u8]) -> Ordering {
    /// Where a character sorts among those that aren't digits; the end of a version is 0.
    fn weight(c: Option<&u8>) -> i32 {
        match c {
            None => 0,
            Some(c) if c.is_ascii_digit() => 0,
            Some(c) if c.is_ascii_alphabetic() => i32::from(*c),
            Some(b'~') => -1,
            Some(c) => i32::from(*c) + 256,
        }
    }
    while !a.is_empty() || !b.is_empty() {
        while a.first().is_some_and(|c| !c.is_ascii_digit())
            || b.first().is_some_and(|c| !c.is_ascii_digit())
        {
            let order = weight(a.first()).cmp(&weight(b.first()));
            if order != Ordering::Equal {
                return order;
            }
            a = &a[1..];
            b = &b[1..];
        }
        let digits = |text: &[u8]| text.iter().take_while(|c| c.is_ascii_digit()).count();
        let (number_a, rest_a) = a.split_at(digits(a));
        let (number_b, rest_b) = b.split_at(digits(b));
        let trim = |number: &[u8]| -> usize { number.iter().take_while(|c| **c == b'0').count() };
        let (number_a, number_b) = (&number_a[trim(number_a)..], &number_b[trim(number_b)..]);
        let order = number_a
            .len()
            .cmp(&number_b.len())
            .then_with(|| number_a.cmp(number_b));
        if order != Ordering::Equal {
            return order;
        }
        a = rest_a;
        b = rest_b;
    }
    Ordering::Equal
}

/// The `<platform>` names of add-ons a build for `os` and `arch` installs, as Kodi's own builds
/// count them (`CAddonMgr::PlatformSupportsAddon`): `all`, the system, and the system with its
/// processor.
fn platforms(os: &str, arch: &str) -> Vec<&'static str> {
    let (system, builds): (&[&'static str], &[(&str, &'static str)]) = match os {
        "android" => (
            &["android"],
            &[
                ("aarch64", "android-aarch64"),
                ("arm", "android-armv7"),
                ("x86_64", "android-x86_64"),
                ("x86", "android-i686"),
            ],
        ),
        "windows" => (
            &["windows", "windx"],
            &[("x86_64", "windows-x86_64"), ("x86", "windows-i686")],
        ),
        "linux" => (
            &["linux"],
            &[
                ("x86_64", "linux-x86_64"),
                ("aarch64", "linux-aarch64"),
                ("arm", "linux-armv7"),
                ("x86", "linux-i686"),
            ],
        ),
        "macos" => (
            &["osx"],
            &[
                ("x86_64", "osx-x86_64"),
                ("x86_64", "osx64"),
                ("aarch64", "osx-arm64"),
            ],
        ),
        _ => (&[], &[]),
    };
    let mut names = vec!["all"];
    names.extend(system);
    names.extend(
        builds
            .iter()
            .filter(|(cpu, _)| *cpu == arch)
            .map(|(_, name)| *name),
    );
    names
}

/// The platform names of the system Raygun runs on.
fn this_platform() -> Vec<&'static str> {
    platforms(std::env::consts::OS, std::env::consts::ARCH)
}

/// Whether an add-on is made for one of `platforms`; one that names none is made for all.
fn made_for(addon: &Addon, platforms: &[&str]) -> bool {
    addon.platform.as_deref().is_none_or(|named| {
        named.split_whitespace().any(|name| {
            platforms
                .iter()
                .any(|known| known.eq_ignore_ascii_case(name))
        })
    })
}

/// Why a source made only for other systems isn't listed as one: "Made for Kodi on Android".
fn other_systems(platform: &str) -> String {
    let mut systems: Vec<&str> = Vec::new();
    for name in platform.split_whitespace() {
        let lower = name.to_ascii_lowercase();
        let system = if lower.starts_with("android") {
            "Android"
        } else if lower.starts_with("win") {
            "Windows"
        } else if lower.starts_with("osx") {
            "macOS"
        } else if lower.starts_with("linux") {
            "Linux"
        } else if lower.starts_with("ios") || lower == "darwin_embedded" {
            "iOS"
        } else if lower.starts_with("tvos") {
            "tvOS"
        } else if lower == "freebsd" {
            "FreeBSD"
        } else {
            name
        };
        if !systems.contains(&system) {
            systems.push(system);
        }
    }
    format!("Made for Kodi on {}", systems.join(", "))
}

/// Whether an `xbmc.python` version is Python 3's (Kodi 19 and later); none named counts as 3.
pub(crate) fn python3(version: Option<&str>) -> bool {
    version
        .and_then(|version| version.split('.').next())
        .and_then(|major| major.trim().parse::<u32>().ok())
        .is_none_or(|major| major >= 3)
}

/// Whether Raygun's Python runs it: it doesn't need Kodi's Python 2.
fn runs_in_raygun(addon: &Addon) -> bool {
    addon
        .requires
        .iter()
        .find(|need| need.id == "xbmc.python")
        .is_none_or(|python| python3(python.version.as_deref()))
}

/// Each add-on once, as this device would install it. An index lists an add-on once for each
/// system it's built for, sometimes in several versions (Kodi's own lists a compiled add-on six
/// times); the one kept is made for this system if any is, then one Raygun's Python runs, then
/// the highest version. Add-ons keep the place they're first listed at.
pub(crate) fn one_each(addons: Vec<Addon>) -> Vec<Addon> {
    one_each_for(addons, &this_platform())
}

fn one_each_for(addons: Vec<Addon>, platforms: &[&str]) -> Vec<Addon> {
    let rank = |addon: &Addon| (made_for(addon, platforms), runs_in_raygun(addon));
    let mut kept: Vec<Addon> = Vec::new();
    let mut places: HashMap<String, usize> = HashMap::new();
    for addon in addons {
        let key = addon.id.to_ascii_lowercase();
        let Some(&place) = places.get(&key) else {
            places.insert(key, kept.len());
            kept.push(addon);
            continue;
        };
        let held = &kept[place];
        let better = rank(&addon)
            .cmp(&rank(held))
            .then_with(|| version_order(&addon.version, &held.version))
            == Ordering::Greater;
        if better {
            kept[place] = addon;
        }
    }
    kept
}

/// The Raygun kinds a plugin provides, by what it says it provides: its videos as a video
/// source, its music (radio, podcasts, songs) as music to browse and play.
pub(crate) fn kinds(provides: &str) -> Vec<String> {
    let mut kinds = Vec::new();
    for word in provides.split_whitespace() {
        let found: &[&str] = match word {
            "video" => &[super::VIDEO_SOURCE],
            "audio" => &[crate::MUSIC_METADATA, crate::MUSIC_STREAM],
            _ => continue,
        };
        for kind in found {
            if !kinds.iter().any(|known| known == kind) {
                kinds.push((*kind).to_owned());
            }
        }
    }
    kinds
}

/// What an add-on that isn't a content source does in Kodi, when it's one people pick (a skin, a
/// service, a program); `None` for a source, and for libraries, resources, repositories and the
/// player's own parts.
pub(crate) fn purpose(addon: &Addon) -> Option<&'static str> {
    if let Some(provides) = addon.plugin.as_deref() {
        if !kinds(provides).is_empty() {
            return None;
        }
        if provides.split_whitespace().any(|word| word == "image") {
            return Some(PICTURES);
        }
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

/// The Raygun extension id of a Kodi add-on (`kodi.plugin.video.example`); `None` for an id
/// that can't be one.
pub(crate) fn extension_id(kodi_id: &str) -> Option<String> {
    if kodi_id.contains(['/', '\\']) || kodi_id.is_empty() {
        return None;
    }
    let label: String = kodi_id
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
    is_extension_id(&id).then_some(id)
}

/// Reads an `addons.xml`, each add-on once ([`one_each`]); `None` when the text isn't one.
pub fn parse(text: &str) -> Option<RepoIndex> {
    parse_for(text, &this_platform())
}

fn parse_for(text: &str, platforms: &[&str]) -> Option<RepoIndex> {
    if root_element(text)? != "addons" {
        return None;
    }
    let mut listed: HashSet<String> = HashSet::new();
    let extensions = one_each_for(addons(text), platforms)
        .into_iter()
        .filter_map(|addon| {
            let kinds = kinds(addon.plugin.as_deref().unwrap_or_default());
            // Not a source: listed as not for Raygun when it's an add-on people pick. A source
            // made only for other systems isn't one here either.
            let not_for_raygun = if kinds.is_empty() {
                Some(purpose(&addon)?.to_owned())
            } else if !made_for(&addon, platforms) {
                Some(other_systems(addon.platform.as_deref().unwrap_or_default()))
            } else {
                None
            };
            let id = extension_id(&addon.id)?;
            // Two add-ons whose ids differ only in what an extension id can't hold.
            if !listed.insert(id.clone()) {
                return None;
            }
            let description = addon.blurb();
            let package = addon.package();
            let folder = addon.folder();
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
                // Plugins reach whatever sites they're about, as in Kodi (public hosts only).
                permissions: RepoPermissions {
                    net: if kinds.is_empty() {
                        Vec::new()
                    } else {
                        vec!["*".to_owned()]
                    },
                    webview: false,
                },
                provides: kinds,
                content_rating: "safe".to_owned(),
                lang: addon
                    .language
                    .and_then(|languages| languages.split_whitespace().next().map(str::to_owned)),
                package,
                // Its pictures are beside its zip.
                icon: addon.icon.map(|icon| {
                    let icon = icon.trim_start_matches('/');
                    if folder.is_empty() {
                        icon.to_owned()
                    } else {
                        format!("{folder}/{icon}")
                    }
                }),
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

/// Why one add-on shared on its own (its `addon.xml` or zip) can't be added: a source installs
/// from its repository, which has what it needs ([`UNSUPPORTED`]); anything else isn't for
/// Raygun, saying what it is. `None` for a repository's own add-on (its index is read instead)
/// and for anything that isn't a Kodi add-on.
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
    fn lists_plugins_with_what_they_provide() {
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
        assert_eq!(
            radio.provides,
            ["music.metadata", "music.stream", "video.source"]
        );
        assert_eq!(
            radio.permissions.net,
            ["*"],
            "plugins reach the sites they're about"
        );
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
        let ids: HashSet<&str> = index
            .extensions
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        println!(
            "{} add-ons, {videos} for video, {others} not for Raygun",
            index.extensions.len()
        );
        assert!(videos > 10);
        assert_eq!(ids.len(), index.extensions.len(), "each add-on once");
        // A compiled add-on's zip is the build for this system, in a folder of its own.
        let compiled = index
            .extensions
            .iter()
            .find(|entry| entry.id == "kodi.pvr.iptvsimple")
            .unwrap();
        println!("{}: {}", compiled.id, compiled.package);
        assert!(compiled.package.starts_with("pvr.iptvsimple+"));
        if cfg!(all(windows, target_arch = "x86_64")) {
            assert!(
                compiled
                    .package
                    .starts_with("pvr.iptvsimple+windows-x86_64/")
            );
        }
        let base: Url = "https://mirrors.kodi.tv/addons/omega/addons.xml.gz"
            .parse()
            .unwrap();
        let zip = crate::repo::package_url(&base, compiled).unwrap();
        let answer = raygun_net::http_client()
            .unwrap()
            .head(zip.as_str())
            .send()
            .await
            .unwrap();
        assert!(answer.status().is_success(), "{zip}: {}", answer.status());
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

    /// Add-ons listed once for each system they're built for and in several versions, as Kodi's
    /// own repository lists compiled ones (`<platform>`, `<path>`).
    const REPEATED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<addons>
<addon id="pvr.example" name="Example TV" version="21.0.3">
  <extension point="kodi.pvrclient" library_windows="pvr.example.dll"/>
  <extension point="xbmc.addon.metadata">
    <platform>windows-x86_64</platform>
    <assets><icon>icon.png</icon></assets>
    <path>pvr.example+windows-x86_64/pvr.example-21.0.3.zip</path>
  </extension>
</addon>
<addon id="plugin.video.example.films" name="Films" version="1.0.0">
  <requires><import addon="xbmc.python" version="3.0.0"/></requires>
  <extension point="xbmc.python.pluginsource" library="default.py"><provides>video</provides></extension>
  <extension point="xbmc.addon.metadata">
    <platform>all</platform>
    <path>plugin.video.example.films/plugin.video.example.films-1.0.0.zip</path>
  </extension>
</addon>
<addon id="pvr.example" name="Example TV" version="21.0.2">
  <extension point="kodi.pvrclient" library_android="libpvr.example.so"/>
  <extension point="xbmc.addon.metadata">
    <platform>android-aarch64</platform>
    <assets><icon>icon.png</icon></assets>
    <path>pvr.example+android-aarch64/pvr.example-21.0.2.zip</path>
  </extension>
</addon>
<addon id="plugin.video.example.films" name="Films" version="3.0.0">
  <requires><import addon="xbmc.python" version="2.26.0"/></requires>
  <extension point="xbmc.python.pluginsource" library="default.py"><provides>video</provides></extension>
  <extension point="xbmc.addon.metadata">
    <platform>all</platform>
    <path>plugin.video.example.films/plugin.video.example.films-3.0.0.zip</path>
  </extension>
</addon>
<addon id="pvr.example" name="Example TV" version="21.0.4">
  <extension point="kodi.pvrclient" library_osx="pvr.example.dylib"/>
  <extension point="xbmc.addon.metadata">
    <platform>osx-arm64</platform>
    <assets><icon>icon.png</icon></assets>
    <path>pvr.example+osx-arm64/pvr.example-21.0.4.zip</path>
  </extension>
</addon>
<addon id="plugin.video.example.films" name="Films" version="1.2.0~beta1">
  <requires><import addon="xbmc.python" version="3.0.1"/></requires>
  <extension point="xbmc.python.pluginsource" library="default.py"><provides>video</provides></extension>
  <extension point="xbmc.addon.metadata">
    <platform>all</platform>
    <assets><icon>resources/icon.png</icon></assets>
    <path>plugin.video.example.films/plugin.video.example.films-1.2.0~beta1.zip</path>
  </extension>
</addon>
<addon id="plugin.video.example.phone" name="Phone films" version="2.0.0">
  <extension point="xbmc.python.pluginsource" library="default.py"><provides>video</provides></extension>
  <extension point="xbmc.addon.metadata"><platform>android-armv7 android-aarch64</platform></extension>
</addon>
<addon id="plugin.video.example.away" name="Elsewhere" version="1.0.0">
  <extension point="xbmc.python.pluginsource" library="default.py"><provides>video</provides></extension>
  <extension point="xbmc.addon.metadata"><path>../../elsewhere.zip</path></extension>
</addon>
</addons>"#;

    #[test]
    fn each_addon_is_listed_once_as_this_system_installs_it() {
        let listed = |os: &str, arch: &str| parse_for(REPEATED, &platforms(os, arch)).unwrap();
        let phone = listed("android", "aarch64");
        let ids: Vec<&str> = phone
            .extensions
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        assert_eq!(
            ids,
            [
                "kodi.pvr.example",
                "kodi.plugin.video.example.films",
                "kodi.plugin.video.example.phone",
                "kodi.plugin.video.example.away",
            ],
            "each once, where it's first listed"
        );
        let tv = &phone.extensions[0];
        assert_eq!(tv.version, "21.0.2", "the build for this system");
        assert_eq!(
            tv.package,
            "pvr.example+android-aarch64/pvr.example-21.0.2.zip"
        );
        assert_eq!(
            tv.icon.as_deref(),
            Some("pvr.example+android-aarch64/icon.png"),
            "its pictures are beside its zip"
        );
        assert_eq!(
            tv.not_for_raygun.as_deref(),
            Some("Live TV for Kodi's TV screens")
        );
        let films = &phone.extensions[1];
        assert_eq!(
            films.package, "plugin.video.example.films/plugin.video.example.films-1.2.0~beta1.zip",
            "the highest version Raygun runs: 3.0.0 needs Python 2"
        );
        assert_eq!(films.version, "1.2.0");
        assert_eq!(
            films.icon.as_deref(),
            Some("plugin.video.example.films/resources/icon.png")
        );
        assert!(films.not_for_raygun.is_none());
        let made_for_phones = &phone.extensions[2];
        assert!(made_for_phones.not_for_raygun.is_none());
        assert_eq!(
            made_for_phones.package,
            "plugin.video.example.phone/plugin.video.example.phone-2.0.0.zip",
            "without a <path>, beside its id"
        );
        assert_eq!(
            phone.extensions[3].package,
            "plugin.video.example.away/plugin.video.example.away-1.0.0.zip",
            "a <path> leaving the repository is passed over"
        );

        let desktop = listed("windows", "x86_64");
        assert_eq!(desktop.extensions.len(), 4);
        assert_eq!(
            desktop.extensions[0].package,
            "pvr.example+windows-x86_64/pvr.example-21.0.3.zip"
        );
        assert_eq!(
            desktop.extensions[2].not_for_raygun.as_deref(),
            Some("Made for Kodi on Android"),
            "a source made only for other systems"
        );
        // Built for no system of this one: listed once all the same, its highest version.
        let other = listed("linux", "x86_64");
        assert_eq!(other.extensions[0].version, "21.0.4");
        assert_eq!(
            other.extensions[0].not_for_raygun.as_deref(),
            Some("Live TV for Kodi's TV screens")
        );
    }

    #[test]
    fn platforms_are_named_as_kodi_names_them() {
        assert_eq!(
            platforms("android", "aarch64"),
            ["all", "android", "android-aarch64"]
        );
        assert_eq!(
            platforms("android", "x86_64"),
            ["all", "android", "android-x86_64"]
        );
        assert_eq!(
            platforms("windows", "x86_64"),
            ["all", "windows", "windx", "windows-x86_64"]
        );
        assert_eq!(platforms("plan9", "sparc"), ["all"]);
        assert_eq!(
            other_systems("android-armv7 windows-x86_64 android-aarch64 webos"),
            "Made for Kodi on Android, Windows, webos"
        );
    }

    #[test]
    fn versions_compare_as_kodi_compares_them() {
        use Ordering::{Equal, Greater, Less};
        for (a, b, order) in [
            ("1.0.2.52", "1.0.2.51", Greater),
            ("1.10.0", "1.9.9", Greater),
            ("1.0~beta1", "1.0", Less),
            ("1.0~beta1", "1.0~beta2", Less),
            ("2.1.0+omega.1", "2.1.0", Greater),
            ("1:0.1", "2.0", Greater),
            ("1.0", "1.00", Equal),
            ("21.0.3", "21.0.3", Equal),
            ("1.0a", "1.0+", Less),
        ] {
            assert_eq!(version_order(a, b), order, "{a} against {b}");
            assert_eq!(version_order(b, a), order.reverse(), "{b} against {a}");
        }
    }
}
