//! A Kodi plugin's folders as Raygun's sources. Kodi browses a plugin as a tree of `plugin://`
//! addresses; Raygun asks for lists, titles, episodes and streams:
//!
//! - a video source's popular list is the plugin's top folder; its other folders are the
//!   "Section" filter, and opening a folder lists what's in it (its videos as episodes, its
//!   sub-folders' videos as seasons when it has only sub-folders);
//! - search opens the plugin's own search entry with the keyboard answered;
//! - an episode's streams are what the plugin resolves (`setResolvedUrl`) or plays; a plugin that
//!   asks which stream (`Dialog().select`) offers each choice as a server;
//! - a music source's playlists are its folders, and its songs play through its own
//!   `music.stream` provider, which resolves the songs it listed.
//!
//! "Next page" entries become further pages. Ids are the plugin's own addresses, so a title
//! saved in the library opens the same folder later.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::addon::PackageInfo;
use super::python::{Item, RunRequest, RunResult};

/// How long a folder's listing is reused.
const LISTING_LIFE: Duration = Duration::from_secs(10 * 60);
/// Folders' listings kept.
const LISTINGS_KEPT: usize = 64;
/// Items remembered for titles' details and songs' streams.
const ITEMS_KEPT: usize = 4000;
/// Sub-folders opened for a folder holding only folders (its seasons).
const SEASONS_OPENED: usize = 12;
/// Further pages read for a folder's episodes.
const EPISODE_PAGES: usize = 5;
/// How long a title's episodes may take to gather, pages and seasons included: past it, what was
/// found is the list (the call has [super::runtime::CALL_BUDGET]).
const EPISODES_TIME: Duration = Duration::from_secs(35);
/// Addresses followed when a plugin sends Kodi elsewhere (`Container.Update`) or resolves to
/// another address of its own.
const HOPS: usize = 3;
/// Separates a server's choice from its episode in a server id.
const CHOICE: char = '\u{1f}';
/// Words a search entry's label starts with or holds.
const SEARCH_WORDS: &[&str] = &[
    "search",
    "suche",
    "suchen",
    "recherche",
    "rechercher",
    "buscar",
    "búsqueda",
    "cerca",
    "ricerca",
    "zoek",
    "sök",
    "søk",
    "haku",
    "szukaj",
    "pesquisa",
    "procurar",
    "hledat",
    "поиск",
    "пошук",
    "検索",
    "搜索",
    "검색",
    "ara",
];
/// Words a "next page" entry's label starts with.
const NEXT_WORDS: &[&str] = &[
    "next",
    "more",
    "weiter",
    "nächste",
    "mehr",
    "suivant",
    "page suivante",
    "plus",
    "siguiente",
    "más",
    "successiv",
    "prossim",
    "altri",
    "volgende",
    "meer",
    "nästa",
    "neste",
    "næste",
    "seuraava",
    "następn",
    "dalej",
    "další",
    "próxim",
    "далее",
    "следующ",
    "наступн",
    "次",
    "下一",
    "다음",
    ">>",
    "»",
];
/// Words of entries that only manage the plugin (its settings, its cache).
const MANAGING_WORDS: &[&str] = &[
    "settings",
    "einstellungen",
    "paramètres",
    "configuración",
    "impostazioni",
    "instellingen",
    "clear cache",
    "cache leeren",
    "vider le cache",
    "add-on settings",
];
/// Query keys that count pages.
const PAGE_KEYS: &[&str] = &[
    "page",
    "p",
    "pg",
    "pagina",
    "seite",
    "offset",
    "start",
    "from",
    "skip",
    "pagenum",
    "page_no",
    "pageno",
    "pagenumber",
    "index",
];

/// Runs the plugin once.
pub(crate) trait Runner {
    fn run(&mut self, request: RunRequest) -> Result<RunResult, String>;
}

/// Which of the plugin's sides a call is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Content {
    Video,
    Audio,
}

/// What Raygun keeps of a listed item.
#[derive(Debug, Clone, Default)]
pub(crate) struct Remembered {
    pub title: String,
    pub folder: bool,
    pub art: BTreeMap<String, String>,
    pub info: serde_json::Map<String, Value>,
    pub path: String,
    pub content: Option<Content>,
}

/// Items by id, the oldest forgotten first.
#[derive(Debug, Default)]
pub(crate) struct Memory {
    items: HashMap<String, Remembered>,
    order: VecDeque<String>,
}

impl Memory {
    fn keep(&mut self, id: String, item: Remembered) {
        if self.items.insert(id.clone(), item).is_none() {
            self.order.push_back(id);
        }
        while self.order.len() > ITEMS_KEPT {
            if let Some(oldest) = self.order.pop_front() {
                self.items.remove(&oldest);
            }
        }
    }

    pub fn get(&self, id: &str) -> Option<&Remembered> {
        self.items.get(id)
    }

    /// Songs listed lately whose title (and artist, when both say) match.
    pub fn songs(&self, title: &str, artist: Option<&str>) -> Vec<(String, Remembered)> {
        let wanted = simplified(title);
        if wanted.is_empty() {
            return Vec::new();
        }
        let artist = artist.map(simplified).filter(|artist| !artist.is_empty());
        self.order
            .iter()
            .rev()
            .filter_map(|id| Some((id, self.items.get(id)?)))
            .filter(|(_, item)| item.content == Some(Content::Audio) && !item.folder)
            .filter(|(_, item)| {
                let title = simplified(&item.title);
                if title != wanted {
                    return false;
                }
                match (&artist, artist_of(&item.info)) {
                    (Some(wanted), Some(found)) => simplified(&found) == *wanted,
                    _ => true,
                }
            })
            .map(|(id, item)| (id.clone(), item.clone()))
            .collect()
    }
}

fn simplified(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Kodi's `plugin://` address for an id, and back.
#[derive(Debug, Clone)]
pub(crate) struct Addresses {
    base: String,
}

impl Addresses {
    pub fn new(addon: &str) -> Self {
        Self {
            base: format!("plugin://{addon}"),
        }
    }

    /// The id Raygun keeps for an address: the plugin's own as its path (`/?mode=list`), any
    /// other as it is.
    pub fn id(&self, url: &str) -> String {
        match url.strip_prefix(&self.base) {
            Some("") => "/".to_owned(),
            Some(rest) if rest.starts_with('/') || rest.starts_with('?') => {
                if rest.starts_with('?') {
                    format!("/{rest}")
                } else {
                    rest.to_owned()
                }
            }
            _ => url.to_owned(),
        }
    }

    pub fn url(&self, id: &str) -> String {
        if id.starts_with('/') {
            format!("{}{id}", self.base)
        } else {
            id.to_owned()
        }
    }

    pub fn is_own(&self, url: &str) -> bool {
        url.starts_with(&self.base)
            && url[self.base.len()..]
                .chars()
                .next()
                .is_none_or(|c| c == '/' || c == '?')
    }
}

/// A run's error, in one sentence for people and the details after it.
fn failure(result: &RunResult, fallback: &str) -> String {
    let said = result
        .messages
        .iter()
        .find(|message| !message.trim().is_empty())
        .cloned();
    let detail = result.error.as_ref().map(|error| {
        let mut text = format!("{}: {}", error.kind, error.message);
        if !error.traceback.is_empty() {
            text.push('\n');
            text.push_str(&error.traceback);
        }
        text
    });
    let headline = match (&said, &result.error) {
        (Some(message), _) => message.clone(),
        (None, Some(error)) if error.kind == "KeyboardInterrupt" => {
            "The add-on took too long".to_owned()
        }
        (None, Some(error)) if error.kind == "ModuleNotFoundError" => {
            format!(
                "The add-on needs a module Raygun doesn't have ({})",
                error.message
            )
        }
        (None, Some(_)) => "The add-on failed".to_owned(),
        (None, None) => fallback.to_owned(),
    };
    let mut text = headline;
    if let Some(detail) = detail {
        text.push('\n');
        text.push_str(&detail);
    }
    if !result.log.is_empty() {
        text.push_str("\nLog:\n");
        text.push_str(&result.log.join("\n"));
    }
    text
}

fn label_of(item: &Item) -> String {
    if !item.label.trim().is_empty() {
        return item.label.trim().to_owned();
    }
    item.info
        .get("title")
        .and_then(Value::as_str)
        .map(|title| title.trim().to_owned())
        .unwrap_or_default()
}

fn lower_has_word(text: &str, words: &[&str]) -> bool {
    let lower = text.to_lowercase();
    words.iter().any(|word| {
        lower == *word
            || lower.starts_with(&format!("{word} "))
            || lower.starts_with(&format!("{word}."))
            || lower.starts_with(&format!("{word}:"))
            || lower.starts_with(&format!("{word}…"))
            || lower.ends_with(&format!(" {word}"))
            || lower.contains(&format!(" {word} "))
    })
}

/// An address without what follows `#` (Raygun's own marks on searches).
fn unmarked(url: &str) -> &str {
    url.split_once('#').map_or(url, |(address, _)| address)
}

fn query_pairs(url: &str) -> Vec<(String, String)> {
    let query = unmarked(url)
        .split_once('?')
        .map(|(_, query)| query)
        .unwrap_or_default();
    url::form_urlencoded::parse(query.as_bytes())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect()
}

fn path_of(url: &str) -> &str {
    let url = unmarked(url);
    url.split_once('?').map_or(url, |(path, _)| path)
}

/// Whether `item` is the "next page" entry of the folder at `current`.
fn is_next(current: &str, item: &Item) -> bool {
    if !item.folder {
        return false;
    }
    let label = label_of(item);
    let lower = label.to_lowercase();
    let worded = NEXT_WORDS.iter().any(|word| lower.starts_with(word))
        || lower.contains("next page")
        || lower.contains("nächste seite")
        || lower.contains("page suivante")
        || lower.contains("página siguiente")
        || lower.ends_with(">>")
        || lower.ends_with('»')
        || lower.ends_with('>');
    if worded {
        return true;
    }
    // The same address with a bigger page number.
    if path_of(current) != path_of(&item.url) {
        return page_in_path(current, &item.url);
    }
    let before: BTreeMap<String, String> = query_pairs(current).into_iter().collect();
    let after: BTreeMap<String, String> = query_pairs(&item.url).into_iter().collect();
    let changed: Vec<&String> = after
        .iter()
        .filter(|(key, value)| before.get(*key) != Some(value))
        .map(|(key, _)| key)
        .collect();
    if changed.len() != 1 || before.len() > after.len() {
        return false;
    }
    let key = changed[0];
    if !PAGE_KEYS.contains(&key.to_lowercase().as_str()) {
        return false;
    }
    let old = before
        .get(key)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(if key == "page" { 1 } else { 0 });
    after
        .get(key)
        .and_then(|value| value.parse::<i64>().ok())
        .is_some_and(|new| new > old)
}

/// `…/list/2/` after `…/list/1/` (or `…/list/`): routing plugins put the page in the path.
fn page_in_path(current: &str, next: &str) -> bool {
    let (a, b) = (
        path_of(current).trim_end_matches('/'),
        path_of(next).trim_end_matches('/'),
    );
    let Some((head, number)) = b.rsplit_once('/') else {
        return false;
    };
    let Ok(number) = number.parse::<u32>() else {
        return false;
    };
    if head == a {
        return number >= 2;
    }
    a.rsplit_once('/').is_some_and(|(head_a, old)| {
        head_a == head && old.parse::<u32>().is_ok_and(|old| number > old)
    })
}

fn is_search(item: &Item) -> bool {
    let label = label_of(item);
    if lower_has_word(&label, SEARCH_WORDS) || label.to_lowercase().starts_with("search") {
        return true;
    }
    let url = item.url.to_lowercase();
    let path = path_of(&url);
    path.split('/')
        .any(|part| part == "search" || part.starts_with("search"))
        || query_pairs(&url).iter().any(|(key, value)| {
            matches!(
                key.as_str(),
                "action" | "mode" | "do" | "cmd" | "func" | "route"
            ) && value.contains("search")
        })
}

fn is_managing(item: &Item) -> bool {
    let label = label_of(item).to_lowercase();
    if MANAGING_WORDS
        .iter()
        .any(|word| label == *word || label.starts_with(word))
    {
        return true;
    }
    query_pairs(&item.url).iter().any(|(key, value)| {
        matches!(key.as_str(), "action" | "mode" | "do")
            && matches!(
                value.to_lowercase().as_str(),
                "settings"
                    | "clear"
                    | "clearcache"
                    | "clear_cache"
                    | "cache"
                    | "opensettings"
                    | "open_settings"
            )
    })
}

/// A web address and the headers Kodi writes after it (`url|Referer=…&User-Agent=…`).
fn split_headers(path: &str) -> (String, BTreeMap<String, String>) {
    let Some((url, extra)) = path.rsplit_once('|') else {
        return (path.to_owned(), BTreeMap::new());
    };
    (url.to_owned(), header_list(extra))
}

fn header_list(text: &str) -> BTreeMap<String, String> {
    text.split('&')
        .filter_map(|pair| {
            let (name, value) = pair.split_once('=')?;
            let decode = |part: &str| {
                url::form_urlencoded::parse(format!("x={part}").as_bytes())
                    .next()
                    .map(|(_, value)| value.into_owned())
                    .unwrap_or_default()
            };
            let name = decode(name);
            let name = name.trim();
            (!name.is_empty()).then(|| (name.to_owned(), decode(value)))
        })
        .collect()
}

fn web(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

/// The picture for a card: a poster, a thumbnail, an icon, a wide picture, fan art; web
/// addresses only.
fn cover(art: &BTreeMap<String, String>) -> Option<(String, BTreeMap<String, String>)> {
    [
        "poster",
        "thumb",
        "icon",
        "landscape",
        "banner",
        "fanart",
        "clearart",
    ]
    .iter()
    .filter_map(|key| art.get(*key))
    .map(|address| split_headers(address))
    .find(|(url, _)| web(url))
}

fn text_of(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) => Some(text.trim().to_owned()).filter(|text| !text.is_empty()),
        Value::Number(number) => Some(number.to_string()),
        Value::Array(items) => {
            let parts: Vec<String> = items
                .iter()
                .filter_map(|item| text_of(Some(item)))
                .collect();
            (!parts.is_empty()).then(|| parts.join(", "))
        }
        _ => None,
    }
}

fn number_of(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

fn artist_of(info: &serde_json::Map<String, Value>) -> Option<String> {
    text_of(info.get("artist")).or_else(|| text_of(info.get("albumartist")))
}

/// `2024-05-01`, `01.05.2024` or `2024-05-01 12:00:00`, as Unix milliseconds.
fn date_ms(text: &str) -> Option<i64> {
    let text = text.trim();
    let date = text.split([' ', 'T']).next()?;
    let (year, month, day) = if date.contains('-') {
        let mut parts = date.split('-').map(str::parse::<i32>);
        (
            parts.next()?.ok()?,
            parts.next()?.ok()?,
            parts.next()?.ok()?,
        )
    } else if date.contains('.') {
        let mut parts = date.split('.').map(str::parse::<i32>);
        let day = parts.next()?.ok()?;
        let month = parts.next()?.ok()?;
        (parts.next()?.ok()?, month, day)
    } else {
        return None;
    };
    if !(1900..=2200).contains(&year) || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let date = chrono::NaiveDate::from_ymd_opt(year, month as u32, day as u32)?;
    Some(date.and_hms_opt(12, 0, 0)?.and_utc().timestamp_millis())
}

fn item_date(info: &serde_json::Map<String, Value>) -> Option<i64> {
    ["aired", "premiered", "date", "dateadded"]
        .iter()
        .find_map(|key| text_of(info.get(*key)).and_then(|text| date_ms(&text)))
}

fn plot_of(info: &serde_json::Map<String, Value>) -> Option<String> {
    ["plot", "plotoutline", "comment", "tagline"]
        .iter()
        .find_map(|key| text_of(info.get(*key)))
        .map(|text| clean_tags(&text))
}

fn genres_of(info: &serde_json::Map<String, Value>) -> Vec<String> {
    match info.get("genre") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| text_of(Some(item)))
            .collect(),
        Some(Value::String(text)) => text
            .split([',', '/', '|'])
            .map(|part| part.trim().to_owned())
            .filter(|part| !part.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

/// Text without Kodi's formatting tags (`[B]`, `[COLOR red]`, `[CR]`...).
fn clean_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('[') {
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        let Some(end) = tail.find(']') else {
            out.push_str(tail);
            rest = "";
            break;
        };
        let tag = tail[1..end].trim_start_matches('/').to_ascii_uppercase();
        let known = [
            "B",
            "I",
            "LIGHT",
            "UPPERCASE",
            "LOWERCASE",
            "CAPITALIZE",
            "TABS",
        ]
        .contains(&tag.as_str())
            || tag.starts_with("COLOR");
        if tag == "CR" {
            out.push('\n');
        } else if !known {
            out.push_str(&tail[..=end]);
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out.trim().to_owned()
}

/// Kodi's YouTube plugin's addresses, which Raygun plays in YouTube's player.
fn youtube_id(url: &str) -> Option<String> {
    if !url.starts_with("plugin://plugin.video.youtube") {
        return None;
    }
    query_pairs(url)
        .into_iter()
        .find(|(key, _)| matches!(key.as_str(), "video_id" | "videoid" | "videoId"))
        .map(|(_, value)| value)
        .filter(|id| {
            id.len() == 11
                && id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
}

/// The video Raygun's player opens for a resolved or played item.
fn video_of(item: &Item, drm: Option<&str>) -> Result<Value, String> {
    let path = if item.path.is_empty() {
        item.url.as_str()
    } else {
        item.path.as_str()
    };
    if let Some(id) = youtube_id(path) {
        return Ok(json!({ "url": format!("youtube:{id}"), "label": "YouTube" }));
    }
    let properties = &item.properties;
    let protected = drm.is_some()
        || properties.keys().any(|key| {
            key.starts_with("inputstream.adaptive.license")
                || key == "inputstream.adaptive.drm"
                || key == "inputstream.adaptive.drm_legacy"
        });
    if protected {
        return Err("This video is protected (DRM), which Raygun's player can't play".into());
    }
    let (url, mut headers) = split_headers(path);
    if !web(&url) {
        if url.starts_with("plugin://") {
            return Err(format!(
                "This video plays through another Kodi add-on ({}), which Raygun doesn't run from inside this one",
                url.trim_start_matches("plugin://")
                    .split(['/', '?'])
                    .next()
                    .unwrap_or_default()
            ));
        }
        return Err("The add-on gave something other than a web address to play".into());
    }
    for key in [
        "inputstream.adaptive.stream_headers",
        "inputstream.adaptive.manifest_headers",
    ] {
        if let Some(extra) = properties.get(key) {
            for (name, value) in header_list(extra) {
                headers.entry(name).or_insert(value);
            }
        }
    }
    let manifest = properties
        .get("inputstream.adaptive.manifest_type")
        .map(|kind| kind.to_lowercase());
    let mime = item.mime.to_lowercase();
    let lower = path_of(&url).to_lowercase();
    let format = match manifest.as_deref() {
        Some("hls") => Some("hls"),
        Some("mpd") => Some("dash"),
        Some("ism") => {
            return Err(
                "This video is a Smooth Streaming manifest, which Raygun's player doesn't open"
                    .into(),
            );
        }
        _ if mime.contains("mpegurl") => Some("hls"),
        _ if mime.contains("dash+xml") => Some("dash"),
        _ if lower.ends_with(".m3u8") => Some("hls"),
        _ if lower.ends_with(".mpd") => Some("dash"),
        _ => None,
    };
    let mut video = json!({ "url": url });
    let label = label_of(item);
    if !label.is_empty() {
        video["label"] = json!(label);
    }
    if let Some(height) = quality_of(&label) {
        video["quality"] = json!(height);
    }
    if !headers.is_empty() {
        video["headers"] = json!(headers);
    }
    if let Some(format) = format {
        video["format"] = json!(format);
    }
    let subtitles: Vec<Value> = item
        .subtitles
        .iter()
        .filter(|subtitle| web(subtitle))
        .map(|subtitle| json!({ "url": subtitle }))
        .collect();
    if !subtitles.is_empty() {
        video["subtitles"] = json!(subtitles);
    }
    Ok(video)
}

/// `720p`, `1080`, `4K` in a label, as a height.
fn quality_of(label: &str) -> Option<u32> {
    let lower = label.to_lowercase();
    if lower.contains("4k") || lower.contains("2160") {
        return Some(2160);
    }
    for height in [1440, 1080, 720, 576, 540, 480, 360, 240, 144] {
        if lower.contains(&format!("{height}p")) || lower.contains(&format!("{height} p")) {
            return Some(height);
        }
    }
    None
}

/// A listing being turned into pages: entries kept, the next page's address.
struct Page {
    items: Vec<Item>,
    next: Option<String>,
}

/// One plugin's catalogue.
pub(crate) struct Catalog {
    pub info: PackageInfo,
    addresses: Addresses,
    /// The request's fixed parts: the script, add-ons' details, settings.
    template: RunRequest,
    listings: VecDeque<(String, Instant, Arc<RunResult>)>,
    /// The address of each further page of a list, by the list's first address.
    pages: HashMap<String, Vec<String>>,
    /// Where each search's results start, by side and words.
    searches: HashMap<(Content, String), String>,
    /// Items listed, by id (details, songs' streams).
    pub memory: Arc<Mutex<Memory>>,
    /// Settings the plugin changed in this run, to keep.
    pub changed_settings: BTreeMap<String, BTreeMap<String, String>>,
}

impl Catalog {
    pub fn new(info: PackageInfo, template: RunRequest, memory: Arc<Mutex<Memory>>) -> Self {
        Self {
            addresses: Addresses::new(&info.addon),
            info,
            template,
            listings: VecDeque::new(),
            pages: HashMap::new(),
            searches: HashMap::new(),
            memory,
            changed_settings: BTreeMap::new(),
        }
    }

    /// Settings now in effect (Raygun's changed, or the plugin changed some).
    pub fn set_addons(&mut self, addons: BTreeMap<String, Value>) {
        self.template.addons = addons;
        self.listings.clear();
    }

    fn root(&self, content: Content) -> String {
        let both = self.info.provides_video() && self.info.provides_audio();
        let base = format!("plugin://{}/", self.info.addon);
        if both {
            format!(
                "{base}?content_type={}",
                match content {
                    Content::Video => "video",
                    Content::Audio => "audio",
                }
            )
        } else {
            base
        }
    }

    fn run(
        &mut self,
        runner: &mut dyn Runner,
        url: &str,
        keyboard: Option<&str>,
        select: Vec<i64>,
        probe: bool,
    ) -> Result<RunResult, String> {
        let request = RunRequest {
            url: url.to_owned(),
            handle: 1,
            keyboard: keyboard.map(str::to_owned),
            select,
            probe,
            ..self.template.clone()
        };
        let result = runner.run(request)?;
        for (addon, values) in &result.settings {
            let kept = self.changed_settings.entry(addon.clone()).or_default();
            for (key, value) in values {
                kept.insert(key.clone(), value.clone());
                if let Some(settings) = self
                    .template
                    .addons
                    .get_mut(addon)
                    .and_then(|details| details.get_mut("settings"))
                    .and_then(Value::as_object_mut)
                {
                    settings.insert(key.clone(), json!(value));
                }
            }
        }
        Ok(result)
    }

    /// A folder's listing (reused for a while), following where the plugin sends Kodi.
    fn list(
        &mut self,
        runner: &mut dyn Runner,
        url: &str,
        content: Content,
    ) -> Result<Arc<RunResult>, String> {
        if let Some((_, at, found)) = self.listings.iter().find(|(known, _, _)| known == url)
            && at.elapsed() < LISTING_LIFE
        {
            return Ok(found.clone());
        }
        let mut address = url.to_owned();
        let mut result = self.run(runner, &address, None, Vec::new(), false)?;
        for _ in 0..HOPS {
            match result.redirect.clone() {
                Some(next) if result.items.is_empty() && self.addresses.is_own(&next) => {
                    address = next;
                    result = self.run(runner, &address, None, Vec::new(), false)?;
                }
                _ => break,
            }
        }
        if result.items.is_empty() && (result.error.is_some() || !result.messages.is_empty()) {
            return Err(failure(&result, "The add-on listed nothing here"));
        }
        let result = Arc::new(result);
        self.remember(&result, content);
        self.listings.retain(|(known, _, _)| known != url);
        self.listings
            .push_back((url.to_owned(), Instant::now(), result.clone()));
        while self.listings.len() > LISTINGS_KEPT {
            self.listings.pop_front();
        }
        Ok(result)
    }

    fn remember(&self, result: &RunResult, content: Content) {
        let mut memory = self.memory.lock().unwrap_or_else(PoisonError::into_inner);
        for item in &result.items {
            if item.url.is_empty() {
                continue;
            }
            memory.keep(
                self.addresses.id(&item.url),
                Remembered {
                    title: label_of(item),
                    folder: item.folder,
                    art: item.art.clone(),
                    info: item.info.clone(),
                    path: item.path.clone(),
                    content: Some(content),
                },
            );
        }
    }

    /// The entries of a listing worth showing, and its next page.
    fn page_of(&self, url: &str, result: &RunResult) -> Page {
        let mut next = None;
        let mut items = Vec::new();
        for item in &result.items {
            if item.url.is_empty() {
                continue;
            }
            if is_next(url, item) {
                // A next page leading back here ends the list.
                if item.url != url {
                    next.get_or_insert_with(|| item.url.clone());
                }
                continue;
            }
            if is_managing(item) {
                continue;
            }
            items.push(item.clone());
        }
        Page { items, next }
    }

    /// Page `page` (from 1) of the list that starts at `first`, following its next pages.
    fn listing_page(
        &mut self,
        runner: &mut dyn Runner,
        first: &str,
        page: u32,
        content: Content,
    ) -> Result<Page, String> {
        let page = page.max(1) as usize;
        let mut address = first.to_owned();
        if page > 1 {
            let known = self.pages.get(first).cloned().unwrap_or_default();
            if let Some(found) = known.get(page - 2) {
                address = found.clone();
            } else {
                // Walk from the last page known.
                let (mut at, mut current) = match known.last() {
                    Some(last) => (known.len() + 1, last.clone()),
                    None => (1, first.to_owned()),
                };
                while at < page {
                    let result = self.list(runner, &current, content)?;
                    let Some(next) = self.page_of(&current, &result).next else {
                        return Ok(Page {
                            items: Vec::new(),
                            next: None,
                        });
                    };
                    self.note_page(first, at + 1, &next);
                    current = next;
                    at += 1;
                }
                address = current;
            }
        }
        let result = self.list(runner, &address, content)?;
        let found = self.page_of(&address, &result);
        if let Some(next) = &found.next {
            self.note_page(first, page + 1, next);
        }
        Ok(found)
    }

    fn note_page(&mut self, first: &str, page: usize, url: &str) {
        let known = self.pages.entry(first.to_owned()).or_default();
        if known.len() == page - 2 {
            known.push(url.to_owned());
        }
    }

    fn card(&self, item: &Item) -> Value {
        let mut card = json!({
            "id": self.addresses.id(&item.url),
            "title": label_of(item),
        });
        if let Some((url, headers)) = cover(&item.art) {
            card["coverUrl"] = json!(url);
            if !headers.is_empty() {
                card["coverHeaders"] = json!(headers);
            }
        }
        let line = if item.label2.trim().is_empty() {
            text_of(item.info.get("tvshowtitle")).or_else(|| text_of(item.info.get("studio")))
        } else {
            Some(item.label2.trim().to_owned())
        };
        if let Some(line) = line.filter(|line| *line != label_of(item)) {
            card["subtitle"] = json!(line.chars().take(120).collect::<String>());
        }
        if item.folder {
            card["folder"] = json!(true);
        }
        card
    }

    fn cards(&self, page: Page, search: bool) -> Value {
        let items: Vec<Value> = page
            .items
            .iter()
            .filter(|item| search || !is_search(item))
            .filter(|item| !label_of(item).is_empty())
            .map(|item| self.card(item))
            .collect();
        json!({ "items": items, "hasNextPage": page.next.is_some() })
    }

    // ---- video.source ----

    pub fn popular(&mut self, runner: &mut dyn Runner, page: u32) -> Result<Value, String> {
        let root = self.root(Content::Video);
        let found = self.listing_page(runner, &root, page, Content::Video)?;
        Ok(self.cards(found, false))
    }

    pub fn filters(&mut self, runner: &mut dyn Runner) -> Result<Value, String> {
        let root = self.root(Content::Video);
        let result = self.list(runner, &root, Content::Video)?;
        let page = self.page_of(&root, &result);
        let options: Vec<Value> = page
            .items
            .iter()
            .filter(|item| item.folder && !is_search(item) && !label_of(item).is_empty())
            .take(500)
            .map(|item| json!({ "value": self.addresses.id(&item.url), "label": label_of(item) }))
            .collect();
        if options.is_empty() {
            return Ok(json!([]));
        }
        Ok(json!([{ "kind": "select", "key": "section", "label": "Section", "options": options }]))
    }

    pub fn search(
        &mut self,
        runner: &mut dyn Runner,
        query: &str,
        page: u32,
        section: Option<&str>,
        content: Content,
    ) -> Result<Value, String> {
        let query = query.trim();
        if query.is_empty() {
            let first = match section {
                Some(section) => self.addresses.url(section),
                None => self.root(content),
            };
            let found = self.listing_page(runner, &first, page, content)?;
            return Ok(self.cards(found, true));
        }
        let Some(first) = self.search_start(runner, query, page, content)? else {
            return Ok(json!({ "items": [], "hasNextPage": false }));
        };
        let found = self.listing_page(runner, &first, page, content)?;
        Ok(self.cards(found, true))
    }

    /// Where a search's results start: known from its first page when a later one is asked for,
    /// else the plugin's search run again.
    fn search_start(
        &mut self,
        runner: &mut dyn Runner,
        query: &str,
        page: u32,
        content: Content,
    ) -> Result<Option<String>, String> {
        if page > 1
            && let Some(first) = self.searches.get(&(content, query.to_owned()))
            && self
                .pages
                .get(first)
                .is_some_and(|known| known.len() >= page as usize - 1)
        {
            return Ok(Some(first.clone()));
        }
        let found = self.search_results(runner, query, content)?;
        if let Some(first) = &found {
            self.searches
                .insert((content, query.to_owned()), first.clone());
        }
        Ok(found)
    }

    /// The address of a search's results: the plugin's search entry run with the keyboard
    /// answered, followed to where it sends Kodi, or into its "new search" entry. `None` when the
    /// plugin has no search.
    fn search_results(
        &mut self,
        runner: &mut dyn Runner,
        query: &str,
        content: Content,
    ) -> Result<Option<String>, String> {
        let root = self.root(content);
        let listing = self.list(runner, &root, content)?;
        let Some(entry) = listing.items.iter().find(|item| is_search(item)).cloned() else {
            return Ok(None);
        };
        let mut address = entry.url.clone();
        for _ in 0..HOPS {
            let result = self.run(runner, &address, Some(query), Vec::new(), false)?;
            if let Some(next) = result
                .redirect
                .clone()
                .filter(|next| self.addresses.is_own(next))
            {
                // The results are where the plugin sends Kodi (`Container.Update`).
                return Ok(Some(next));
            }
            let playable = result.items.iter().any(|item| !item.folder);
            let searches: Vec<&Item> = result.items.iter().filter(|item| is_search(item)).collect();
            if !playable && !searches.is_empty() && searches.len() + 3 >= result.items.len() {
                // A menu: a new search, past searches. The first is the new search.
                address = searches[0].url.clone();
                continue;
            }
            if result.items.is_empty() {
                if result.error.is_some() {
                    return Err(failure(&result, "The add-on's search failed"));
                }
                return Ok(None);
            }
            // Its results, listed by this run: kept as the list's first page.
            let key = format!("{address}#search={query}");
            let result = Arc::new(result);
            self.remember(&result, content);
            self.listings.retain(|(known, _, _)| *known != key);
            self.listings
                .push_back((key.clone(), Instant::now(), result));
            return Ok(Some(key));
        }
        Ok(None)
    }

    fn remembered(&self, id: &str) -> Option<Remembered> {
        self.memory
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(id)
            .cloned()
    }

    pub fn details(&mut self, runner: &mut dyn Runner, id: &str) -> Result<Value, String> {
        let known = match self.remembered(id) {
            Some(known) => known,
            None => {
                // Not listed in this session: what the folder says of itself.
                let url = self.addresses.url(id);
                let title = if web(&url) {
                    url.clone()
                } else {
                    let result = self.list(runner, &url, Content::Video)?;
                    if result.category.is_empty() {
                        self.info.name.clone()
                    } else {
                        result.category.clone()
                    }
                };
                Remembered {
                    title,
                    folder: true,
                    ..Remembered::default()
                }
            }
        };
        let title = if known.title.is_empty() {
            self.info.name.clone()
        } else {
            known.title.clone()
        };
        let mut details = json!({ "id": id, "title": title });
        if let Some((url, headers)) = cover(&known.art) {
            details["coverUrl"] = json!(url);
            if !headers.is_empty() {
                details["coverHeaders"] = json!(headers);
            }
        }
        if let Some(plot) = plot_of(&known.info) {
            details["description"] = json!(plot);
        }
        let genres = genres_of(&known.info);
        if !genres.is_empty() {
            details["genres"] = json!(genres);
        }
        if let Some(year) =
            number_of(known.info.get("year")).filter(|year| (1800.0..=2200.0).contains(year))
        {
            details["year"] = json!(year as i64);
        }
        let kind = match text_of(known.info.get("mediatype")).as_deref() {
            Some("movie") => "movie",
            Some("tvshow" | "season" | "episode") => "series",
            _ if !known.folder => "movie",
            _ => "other",
        };
        details["type"] = json!(kind);
        if let Some(cast) = known.info.get("cast").and_then(Value::as_array) {
            let names: Vec<String> = cast
                .iter()
                .filter_map(|name| text_of(Some(name)))
                .take(20)
                .collect();
            if !names.is_empty() {
                details["cast"] = json!(names);
            }
        }
        if let Some(directors) = text_of(known.info.get("director")) {
            details["directors"] =
                json!(directors.split(", ").map(str::to_owned).collect::<Vec<_>>());
        }
        Ok(details)
    }

    fn episode(&self, item: &Item, season: Option<i64>) -> Value {
        let mut episode = json!({ "id": self.addresses.id(&item.url), "name": label_of(item) });
        if let Some(number) = number_of(item.info.get("episode")).filter(|number| *number > 0.0) {
            episode["number"] = json!(number);
        }
        let season = number_of(item.info.get("season"))
            .filter(|season| *season >= 0.0)
            .map(|season| season as i64)
            .or(season);
        if let Some(season) = season {
            episode["season"] = json!(season);
        }
        if let Some(date) = item_date(&item.info) {
            episode["dateUpload"] = json!(date);
        }
        episode
    }

    pub fn episodes(&mut self, runner: &mut dyn Runner, id: &str) -> Result<Value, String> {
        let known = self.remembered(id);
        let url = self.addresses.url(id);
        if web(&url) || known.as_ref().is_some_and(|item| !item.folder) {
            let name = known
                .map(|item| item.title)
                .filter(|title| !title.is_empty())
                .unwrap_or_else(|| "Video".to_owned());
            return Ok(json!({ "episodes": [{ "id": id, "name": name }] }));
        }
        // Further pages and seasons only while there's time: a partial list beats none.
        let started = Instant::now();
        let mut episodes: Vec<Value> = Vec::new();
        let mut folders: Vec<Item> = Vec::new();
        let mut page = 1;
        loop {
            let found = self.listing_page(runner, &url, page, Content::Video)?;
            for item in &found.items {
                if item.folder {
                    folders.push(item.clone());
                } else {
                    episodes.push(self.episode(item, None));
                }
            }
            if found.next.is_none()
                || page as usize >= EPISODE_PAGES
                || started.elapsed() > EPISODES_TIME
            {
                break;
            }
            page += 1;
        }
        if episodes.is_empty() && !folders.is_empty() {
            // A folder of folders (seasons): each one's videos.
            for (index, folder) in folders.iter().take(SEASONS_OPENED).enumerate() {
                if started.elapsed() > EPISODES_TIME {
                    break;
                }
                let season = number_of(folder.info.get("season"))
                    .map(|season| season as i64)
                    .unwrap_or(index as i64 + 1);
                let Ok(found) = self.listing_page(runner, &folder.url, 1, Content::Video) else {
                    continue;
                };
                for item in found.items.iter().filter(|item| !item.folder) {
                    episodes.push(self.episode(item, Some(season)));
                }
            }
        }
        let mut seen = std::collections::HashSet::new();
        episodes
            .retain(|episode| seen.insert(episode["id"].as_str().unwrap_or_default().to_owned()));
        Ok(json!({ "episodes": episodes }))
    }

    /// What the plugin plays for an item: its streams, or the choices it asks to be made.
    fn resolve(
        &mut self,
        runner: &mut dyn Runner,
        id: &str,
        select: Vec<i64>,
    ) -> Result<Resolution, String> {
        let mut url = self.addresses.url(id);
        if web(&url) {
            let item = Item {
                url: url.clone(),
                path: url,
                label: self
                    .remembered(id)
                    .map(|known| known.title)
                    .unwrap_or_default(),
                ..Item::default()
            };
            return Ok(Resolution::Videos(vec![video_of(&item, None)?]));
        }
        if let Some(found) = youtube_id(&url) {
            return Ok(Resolution::Videos(vec![
                json!({ "url": format!("youtube:{found}"), "label": "YouTube" }),
            ]));
        }
        // A listed item whose path is already a stream (`setPath` on a direct item).
        if let Some(known) = self.remembered(id)
            && !known.folder
            && web(&split_headers(&known.path).0)
        {
            let item = Item {
                url: url.clone(),
                path: known.path.clone(),
                label: known.title.clone(),
                ..Item::default()
            };
            if let Ok(video) = video_of(&item, None) {
                return Ok(Resolution::Videos(vec![video]));
            }
        }
        let probe = select.is_empty();
        for _ in 0..HOPS {
            let result = self.run(runner, &url, None, select.clone(), probe)?;
            // A choice declined first: its options are servers (the plugin then resolves
            // nothing, or says it failed).
            if probe
                && let Some(choice) = result.selects.first()
                && result.resolved.as_ref().is_none_or(|resolved| !resolved.ok)
                && result.played.is_none()
            {
                return Ok(Resolution::Choices(choice.options.clone()));
            }
            if let Some(resolved) = &result.resolved {
                if !resolved.ok {
                    return Err(failure(
                        &result,
                        "The add-on couldn't find this video's stream",
                    ));
                }
                let path = if resolved.item.path.is_empty() {
                    resolved.item.url.clone()
                } else {
                    resolved.item.path.clone()
                };
                if self.addresses.is_own(&path) && path != url {
                    url = path;
                    continue;
                }
                let mut item = resolved.item.clone();
                item.path = path;
                if item.label.is_empty() {
                    item.label = self
                        .remembered(id)
                        .map(|known| known.title)
                        .unwrap_or_default();
                }
                return Ok(Resolution::Videos(vec![video_of(
                    &item,
                    result.drm.as_deref(),
                )?]));
            }
            if let Some(played) = &result.played {
                let mut videos = Vec::new();
                let mut problem = None;
                for item in &played.items {
                    match video_of(item, result.drm.as_deref()) {
                        Ok(video) => videos.push(video),
                        Err(error) => problem = Some(error),
                    }
                }
                if videos.is_empty() {
                    return Err(
                        problem.unwrap_or_else(|| failure(&result, "The add-on played nothing"))
                    );
                }
                return Ok(Resolution::Videos(videos));
            }
            if probe && let Some(choice) = result.selects.first() {
                return Ok(Resolution::Choices(choice.options.clone()));
            }
            if let Some(next) = result
                .redirect
                .clone()
                .filter(|next| self.addresses.is_own(next) && *next != url)
            {
                url = next;
                continue;
            }
            if result.drm.is_some() {
                return Err(
                    "This video is protected (DRM), which Raygun's player can't play".into(),
                );
            }
            if !result.items.is_empty() {
                return Err("This is a folder of the add-on, not a video".into());
            }
            return Err(failure(&result, "The add-on found nothing to play"));
        }
        Err("The add-on sent Raygun from address to address without a video".into())
    }

    pub fn servers(&mut self, runner: &mut dyn Runner, episode: &str) -> Result<Value, String> {
        match self.resolve(runner, episode, Vec::new())? {
            Resolution::Videos(videos) => Ok(json!({
                "servers": [{ "id": episode, "name": self.info.name, "videos": videos }]
            })),
            Resolution::Choices(options) => {
                let servers: Vec<Value> = options
                    .iter()
                    .enumerate()
                    .take(50)
                    .map(|(index, name)| {
                        json!({ "id": format!("{index}{CHOICE}{episode}"), "name": name })
                    })
                    .collect();
                Ok(json!({ "servers": servers }))
            }
        }
    }

    pub fn videos(&mut self, runner: &mut dyn Runner, server: &str) -> Result<Value, String> {
        let (select, episode) = match server.split_once(CHOICE) {
            Some((index, episode)) => (
                vec![
                    index
                        .parse::<i64>()
                        .map_err(|_| "That server is no longer offered")?,
                ],
                episode,
            ),
            None => (Vec::new(), server),
        };
        match self.resolve(runner, episode, select)? {
            Resolution::Videos(videos) => Ok(json!({ "videos": videos })),
            Resolution::Choices(_) => Err("The add-on asked for another choice".into()),
        }
    }

    // ---- music.metadata and music.stream ----

    fn track(&self, item: &Item) -> Value {
        let id = self.addresses.id(&item.url);
        let title = text_of(item.info.get("title")).unwrap_or_else(|| label_of(item));
        let mut track = json!({ "id": id, "title": title });
        if let Some(artist) = artist_of(&item.info)
            .or_else(|| (!item.label2.trim().is_empty()).then(|| item.label2.trim().to_owned()))
        {
            track["artist"] = json!(artist);
        }
        if let Some(album) = text_of(item.info.get("album")) {
            track["album"] = json!(album);
        }
        if let Some(seconds) = number_of(item.info.get("duration")).filter(|seconds| *seconds > 0.0)
        {
            track["durationMs"] = json!((seconds * 1000.0) as i64);
        }
        if let Some((url, _)) = cover(&item.art) {
            track["artworkUrl"] = json!(url);
        }
        let (stream, headers) = split_headers(if item.path.is_empty() {
            &item.url
        } else {
            &item.path
        });
        if web(&stream)
            && !item
                .properties
                .keys()
                .any(|key| key.starts_with("inputstream"))
        {
            track["source"] = json!({ "kind": "url", "url": stream, "headers": headers });
        }
        track
    }

    pub fn music_search(
        &mut self,
        runner: &mut dyn Runner,
        query: &str,
        limit: usize,
    ) -> Result<Value, String> {
        let Some(first) = self.search_start(runner, query, 1, Content::Audio)? else {
            return Ok(json!({ "tracks": [] }));
        };
        let found = self.listing_page(runner, &first, 1, Content::Audio)?;
        let tracks: Vec<Value> = found
            .items
            .iter()
            .filter(|item| !item.folder && !label_of(item).is_empty())
            .take(limit.clamp(1, 100))
            .map(|item| self.track(item))
            .collect();
        Ok(json!({ "tracks": tracks }))
    }

    pub fn playlists(
        &mut self,
        runner: &mut dyn Runner,
        query: &str,
        limit: usize,
    ) -> Result<Value, String> {
        let first = if query.trim().is_empty() {
            Some(self.root(Content::Audio))
        } else {
            self.search_start(runner, query, 1, Content::Audio)?
        };
        let Some(first) = first else {
            return Ok(json!({ "playlists": [] }));
        };
        let found = self.listing_page(runner, &first, 1, Content::Audio)?;
        let mut playlists: Vec<Value> = found
            .items
            .iter()
            .filter(|item| item.folder && !is_search(item) && !label_of(item).is_empty())
            .take(limit.clamp(1, 100))
            .map(|item| {
                let mut playlist = json!({
                    "id": self.addresses.id(&item.url),
                    "title": label_of(item),
                    "author": self.info.name,
                    "featured": query.trim().is_empty(),
                });
                if let Some((url, _)) = cover(&item.art) {
                    playlist["artworkUrl"] = json!(url);
                }
                playlist
            })
            .collect();
        // Songs or stations at the top of the plugin are a playlist of their own.
        if query.trim().is_empty() && found.items.iter().any(|item| !item.folder) {
            playlists.insert(
                0,
                json!({
                    "id": self.addresses.id(&first),
                    "title": self.info.name,
                    "author": self.info.name,
                    "featured": true,
                }),
            );
        }
        Ok(json!({ "playlists": playlists }))
    }

    pub fn playlist(&mut self, runner: &mut dyn Runner, id: &str) -> Result<Value, String> {
        let url = self.addresses.url(id);
        let found = self.listing_page(runner, &url, 1, Content::Audio)?;
        let mut tracks: Vec<Value> = found
            .items
            .iter()
            .filter(|item| !item.folder && !label_of(item).is_empty())
            .map(|item| self.track(item))
            .collect();
        if tracks.is_empty() {
            let folders: Vec<Item> = found
                .items
                .iter()
                .filter(|item| item.folder && !is_search(item))
                .take(SEASONS_OPENED)
                .cloned()
                .collect();
            for folder in folders {
                if let Ok(inner) = self.listing_page(runner, &folder.url, 1, Content::Audio) {
                    tracks.extend(
                        inner
                            .items
                            .iter()
                            .filter(|item| !item.folder && !label_of(item).is_empty())
                            .map(|item| self.track(item)),
                    );
                }
            }
        }
        let title = self
            .remembered(id)
            .map(|known| known.title)
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| self.info.name.clone());
        Ok(json!({
            "playlist": { "id": id, "title": title, "author": self.info.name, "trackCount": tracks.len() },
            "tracks": tracks,
        }))
    }

    /// A song this plugin listed, played: its stream.
    pub fn resolve_song(&mut self, runner: &mut dyn Runner, id: &str) -> Result<Value, String> {
        let videos = match self.resolve(runner, id, Vec::new())? {
            Resolution::Videos(videos) => videos,
            // A choice of streams: the first, as Kodi's default.
            Resolution::Choices(_) => match self.resolve(runner, id, vec![0])? {
                Resolution::Videos(videos) => videos,
                Resolution::Choices(_) => Vec::new(),
            },
        };
        let video = videos.first().ok_or("The add-on found nothing to play")?;
        let url = video["url"].as_str().unwrap_or_default();
        if let Some(found) = url.strip_prefix("youtube:") {
            return Ok(json!({ "source": { "kind": "youTube", "videoId": found } }));
        }
        let headers = video.get("headers").cloned().unwrap_or_else(|| json!({}));
        Ok(json!({ "source": { "kind": "url", "url": url, "headers": headers } }))
    }
}

/// What resolving an item gave.
enum Resolution {
    Videos(Vec<Value>),
    /// The plugin asks which stream: its choices.
    Choices(Vec<String>),
}

/// Candidates among the songs a plugin listed lately (answered without running it).
pub(crate) fn candidates(memory: &Mutex<Memory>, args: &Value) -> Value {
    let title = args
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let artist = args.get("artist").and_then(Value::as_str);
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(10)
        .clamp(1, 50) as usize;
    let memory = memory.lock().unwrap_or_else(PoisonError::into_inner);
    let found: Vec<Value> = memory
        .songs(title, artist)
        .into_iter()
        .take(limit)
        .map(|(id, item)| {
            let mut candidate = json!({ "id": id, "title": item.title });
            if let Some(artist) = artist_of(&item.info) {
                candidate["artist"] = json!(artist);
            }
            if let Some(seconds) =
                number_of(item.info.get("duration")).filter(|seconds| *seconds > 0.0)
            {
                candidate["durationMs"] = json!((seconds * 1000.0) as i64);
            }
            let (stream, headers) = split_headers(&item.path);
            if web(&stream) {
                candidate["source"] = json!({ "kind": "url", "url": stream, "headers": headers });
            }
            candidate
        })
        .collect();
    json!({ "candidates": found })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(url: &str, label: &str, folder: bool) -> Item {
        Item {
            url: url.to_owned(),
            label: label.to_owned(),
            folder,
            ..Item::default()
        }
    }

    #[test]
    fn next_pages_are_found_by_label_or_by_number() {
        let here = "plugin://plugin.video.x/?action=list&page=1";
        assert!(is_next(
            here,
            &item(
                "plugin://plugin.video.x/?action=list&page=2",
                "More...",
                true
            )
        ));
        assert!(is_next(
            here,
            &item("plugin://plugin.video.x/?action=list&page=2", "(2/7)", true)
        ));
        assert!(!is_next(
            here,
            &item("plugin://plugin.video.x/?action=list&page=2", "Film", false)
        ));
        assert!(!is_next(
            here,
            &item("plugin://plugin.video.x/?action=show&id=4", "Show", true)
        ));
        assert!(is_next(
            "plugin://plugin.video.x/list/",
            &item("plugin://plugin.video.x/list/2/", "Page 2", true)
        ));
        assert!(is_next(
            "plugin://plugin.video.x/?mode=films",
            &item("plugin://plugin.video.x/?mode=films&offset=50", "50", true)
        ));
    }

    #[test]
    fn search_and_settings_entries_are_known() {
        assert!(is_search(&item(
            "plugin://x/?action=search",
            "Search",
            true
        )));
        assert!(is_search(&item("plugin://x/search/", "Find", true)));
        assert!(is_search(&item("plugin://x/?mode=7", "Suche", true)));
        assert!(!is_search(&item(
            "plugin://x/?mode=7",
            "Research news",
            true
        )));
        assert!(is_managing(&item(
            "plugin://x/?action=clear",
            "Clear cache",
            false
        )));
        assert!(is_managing(&item("plugin://x/?mode=9", "Settings", false)));
    }

    #[test]
    fn streams_keep_their_headers_and_formats() {
        let mut resolved = item("plugin://plugin.video.x/?play=1", "Film 1080p", false);
        resolved.path = "https://cdn.example.org/film/master.m3u8|User-Agent=Kodi%2F21&Referer=https%3A%2F%2Fexample.org%2F".into();
        let video = video_of(&resolved, None).unwrap();
        assert_eq!(video["url"], "https://cdn.example.org/film/master.m3u8");
        assert_eq!(video["format"], "hls");
        assert_eq!(video["quality"], 1080);
        assert_eq!(video["headers"]["User-Agent"], "Kodi/21");
        assert_eq!(video["headers"]["Referer"], "https://example.org/");
        let mut dash = item("", "", false);
        dash.path = "https://cdn.example.org/manifest".into();
        dash.properties
            .insert("inputstream.adaptive.manifest_type".into(), "mpd".into());
        assert_eq!(video_of(&dash, None).unwrap()["format"], "dash");
        dash.properties.insert(
            "inputstream.adaptive.license_key".into(),
            "https://lic".into(),
        );
        assert!(video_of(&dash, None).unwrap_err().contains("DRM"));
        let mut youtube = item("", "", false);
        youtube.path = "plugin://plugin.video.youtube/play/?video_id=dQw4w9WgXcQ".into();
        assert_eq!(
            video_of(&youtube, None).unwrap()["url"],
            "youtube:dQw4w9WgXcQ"
        );
    }

    #[test]
    fn ids_are_the_plugins_addresses() {
        let addresses = Addresses::new("plugin.video.x");
        assert_eq!(addresses.id("plugin://plugin.video.x/?a=1"), "/?a=1");
        assert_eq!(addresses.id("plugin://plugin.video.x?a=1"), "/?a=1");
        assert_eq!(
            addresses.id("plugin://plugin.video.x/shows/4/"),
            "/shows/4/"
        );
        assert_eq!(addresses.url("/?a=1"), "plugin://plugin.video.x/?a=1");
        assert_eq!(
            addresses.id("https://example.org/a.mp4"),
            "https://example.org/a.mp4"
        );
        assert!(addresses.is_own("plugin://plugin.video.x/"));
        assert!(!addresses.is_own("plugin://plugin.video.xy/"));
    }

    #[test]
    fn dates_read_as_kodi_writes_them() {
        assert_eq!(date_ms("2024-05-01"), date_ms("01.05.2024"));
        assert!(date_ms("2024-05-01 10:00:00").is_some());
        assert_eq!(date_ms("soon"), None);
    }

    /// Answers runs from a script of addresses.
    struct Script(BTreeMap<String, RunResult>);

    impl Runner for Script {
        fn run(&mut self, request: RunRequest) -> Result<RunResult, String> {
            let key = match &request.keyboard {
                Some(words) => format!("{}#{words}", request.url),
                None if !request.select.is_empty() => {
                    format!("{}#{:?}", request.url, request.select)
                }
                None => request.url.clone(),
            };
            self.0
                .get(&key)
                .cloned()
                .ok_or_else(|| format!("no run for {key}"))
        }
    }

    fn info() -> PackageInfo {
        PackageInfo {
            addon: "plugin.video.x".into(),
            name: "X".into(),
            version: "1.0.0".into(),
            author: String::new(),
            summary: String::new(),
            library: "default.py".into(),
            provides: vec!["video".into()],
            icon: String::new(),
            fanart: String::new(),
            dependencies: Vec::new(),
            settings: Vec::new(),
            answered: Vec::new(),
        }
    }

    fn listed(items: Vec<Item>) -> RunResult {
        RunResult {
            items,
            ended: true,
            succeeded: true,
            ..RunResult::default()
        }
    }

    #[test]
    fn folders_list_pages_and_choices_become_servers() {
        let mut film = item("plugin://plugin.video.x/?play=film", "A film", false);
        film.art
            .insert("thumb".into(), "https://img.example.org/film.jpg".into());
        let runs = BTreeMap::from([
            (
                "plugin://plugin.video.x/".to_owned(),
                listed(vec![
                    item("plugin://plugin.video.x/?list=films", "Films", true),
                    item("plugin://plugin.video.x/?search=1", "Search", true),
                    item(
                        "plugin://plugin.video.x/?action=settings",
                        "Settings",
                        false,
                    ),
                ]),
            ),
            (
                "plugin://plugin.video.x/?list=films".to_owned(),
                listed(vec![
                    film.clone(),
                    item(
                        "plugin://plugin.video.x/?list=films&page=2",
                        "Next page",
                        true,
                    ),
                ]),
            ),
            (
                "plugin://plugin.video.x/?list=films&page=2".to_owned(),
                listed(vec![item(
                    "plugin://plugin.video.x/?play=other",
                    "Another film",
                    false,
                )]),
            ),
            (
                "plugin://plugin.video.x/?search=1#moon".to_owned(),
                RunResult {
                    redirect: Some("plugin://plugin.video.x/?found=moon".into()),
                    ..RunResult::default()
                },
            ),
            (
                "plugin://plugin.video.x/?found=moon".to_owned(),
                listed(vec![item(
                    "plugin://plugin.video.x/?play=moon",
                    "Moon",
                    false,
                )]),
            ),
            (
                "plugin://plugin.video.x/?play=film".to_owned(),
                RunResult {
                    selects: vec![super::super::python::Select {
                        heading: "Quality".into(),
                        options: vec!["MP4 720p".into(), "OGV 480p".into()],
                    }],
                    ..RunResult::default()
                },
            ),
            (
                "plugin://plugin.video.x/?play=film#[1]".to_owned(),
                RunResult {
                    resolved: Some(super::super::python::Resolved {
                        ok: true,
                        item: Item {
                            path: "https://archive.example.org/film.ogv".into(),
                            ..Item::default()
                        },
                    }),
                    ..RunResult::default()
                },
            ),
        ]);
        let mut runner = Script(runs);
        let mut catalog = Catalog::new(info(), RunRequest::default(), Arc::default());
        let popular = catalog.popular(&mut runner, 1).unwrap();
        assert_eq!(
            popular["items"].as_array().unwrap().len(),
            1,
            "search and settings left out"
        );
        assert_eq!(popular["items"][0]["id"], "/?list=films");
        assert_eq!(popular["items"][0]["folder"], true);
        let filters = catalog.filters(&mut runner).unwrap();
        assert_eq!(filters[0]["options"][0]["value"], "/?list=films");
        let films = catalog
            .search(&mut runner, "", 1, Some("/?list=films"), Content::Video)
            .unwrap();
        assert_eq!(films["items"][0]["title"], "A film");
        assert_eq!(
            films["items"][0]["coverUrl"],
            "https://img.example.org/film.jpg"
        );
        assert_eq!(films["hasNextPage"], true);
        let second = catalog
            .search(&mut runner, "", 2, Some("/?list=films"), Content::Video)
            .unwrap();
        assert_eq!(second["items"][0]["title"], "Another film");
        let found = catalog
            .search(&mut runner, "moon", 1, None, Content::Video)
            .unwrap();
        assert_eq!(found["items"][0]["title"], "Moon");
        let episodes = catalog.episodes(&mut runner, "/?list=films").unwrap();
        assert_eq!(
            episodes["episodes"].as_array().unwrap().len(),
            2,
            "both pages"
        );
        let details = catalog.details(&mut runner, "/?play=film").unwrap();
        assert_eq!(details["title"], "A film");
        assert_eq!(details["type"], "movie");
        let servers = catalog.servers(&mut runner, "/?play=film").unwrap();
        assert_eq!(servers["servers"][1]["name"], "OGV 480p");
        let server = servers["servers"][1]["id"].as_str().unwrap().to_owned();
        let videos = catalog.videos(&mut runner, &server).unwrap();
        assert_eq!(
            videos["videos"][0]["url"],
            "https://archive.example.org/film.ogv"
        );
    }
}
