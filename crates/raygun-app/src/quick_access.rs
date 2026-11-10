//! Quick access: the desktop sidebar's list (M46.29) and the phone's drawer.
//!
//! - The audio library lists itself: Favorites, Downloaded, Local files, playlists, the device's
//!   albums, followed and local artists, followed shows and books ([`list`]). Nothing is pinned
//!   to get there, and what leaves the library leaves quick access.
//! - The rest is sorted by `RailPrefs::order`, by default the page last played from first
//!   ([`played`], from the play contexts the UI arms before each play); what was never played
//!   follows, the newest added first. At most `RailPrefs::shown` of them, the most recent.
//! - Pins (`RailPrefs::pins`) stay on top in the user's order and always show. Comics, books,
//!   videos and sites show only while pinned: by hand, or when added to the library (M46.41,
//!   [`add`]), up to `RailPrefs::limit` automatic pins, the oldest going first.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use raygun_db::Db;
use raygun_db::titles::TitleRow;
use raygun_model::{
    QuickAccessItem, QuickAccessList, QuickAccessNames, RAIL_PLAYED_KEPT, RAIL_VERSION, RailKinds,
    RailOrder, RailPin, RailPlayed, RailPrefs, SettingChange, Settings,
};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::sync::Mutex;

use crate::events::{QuickAccessChangedEvent, SettingsChangedEvent};
use crate::startup::AppDb;

/// One change at a time: two additions at once would each save the other's pins away.
static CHANGES: Mutex<()> = Mutex::const_new(());

/// Kinds the audio library lists by itself; they're never pinned automatically.
const AUDIO_LIBRARY: &[&str] = &[
    "favorites",
    "downloaded",
    "localFiles",
    "playlist",
    "album",
    "artist",
    "podcast",
    "audiobook",
];

/// The fixed blocks, in the order they keep among equals.
const FIXED: [&str; 3] = ["favorites", "downloaded", "localFiles"];

/// Play contexts whose pages are stamped when played from: the library's kinds and mixes. Other
/// pages (a search, a chart) only while pinned.
const STAMPED: &[&str] = &[
    "favorites",
    "downloaded",
    "localFiles",
    "playlist",
    "album",
    "artist",
    "podcast",
    "audiobook",
    "mix",
];

/// Plays from one page this close together are stamped once.
const STAMP_EVERY_S: u32 = 30;

/// Older versions pinned the first run's blocks and playlists at once, within this many seconds.
const SEED_WINDOW_S: u32 = 2;

/// Something with a page, to pin.
pub struct QuickPin {
    /// As `RailPin::kind`.
    pub kind: &'static str,
    pub to: String,
    pub search: BTreeMap<String, String>,
    pub name: String,
    pub artwork_url: Option<String>,
}

fn now_s() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| u32::try_from(elapsed.as_secs()).unwrap_or(u32::MAX))
        .unwrap_or_default()
}

fn ms_to_s(ms: f64) -> u32 {
    if ms.is_finite() && ms > 0.0 {
        (ms / 1000.0).min(f64::from(u32::MAX)) as u32
    } else {
        0
    }
}

fn search(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

/// A page as one string: its route and its search parameters in order.
fn page_key(to: &str, search: &BTreeMap<String, String>) -> String {
    let mut key = to.to_owned();
    for (index, (name, value)) in search.iter().enumerate() {
        key.push(if index == 0 { '?' } else { '&' });
        key.push_str(name);
        key.push('=');
        key.push_str(value);
    }
    key
}

/// A library title's page (comics, books, videos; a Stremio title's own page).
pub fn title_pin(row: &TitleRow) -> QuickPin {
    let fields = &row.fields;
    let (kind, to, search) = match fields.kind.as_str() {
        "novel" => (
            "novel",
            "/novel/title",
            search(&[("source", &fields.source_id), ("id", &fields.remote_id)]),
        ),
        "video" if fields.source_id == "stremio" => {
            let (kind, id) = fields
                .remote_id
                .split_once(':')
                .unwrap_or(("movie", &fields.remote_id));
            (
                "video",
                "/watch/title",
                search(&[("type", kind), ("id", id)]),
            )
        }
        "video" => (
            "video",
            "/watch/video",
            search(&[("source", &fields.source_id), ("id", &fields.remote_id)]),
        ),
        _ => (
            "manga",
            "/manga/title",
            search(&[("source", &fields.source_id), ("id", &fields.remote_id)]),
        ),
    };
    QuickPin {
        kind,
        to: to.to_owned(),
        search,
        name: fields.title.clone(),
        artwork_url: fields.cover.clone(),
    }
}

/// A page with one `id` parameter (a playlist, a podcast, an audiobook).
pub fn page_pin(
    kind: &'static str,
    to: &str,
    id: &str,
    name: &str,
    art: Option<String>,
) -> QuickPin {
    QuickPin {
        kind,
        to: to.to_owned(),
        search: search(&[("id", id)]),
        name: name.to_owned(),
        artwork_url: art,
    }
}

/// Something was added to the library. The audio library lists itself; comics, books, videos and
/// sites are pinned, unless quick access is set not to or incognito is on (M47).
pub async fn add(app: &AppHandle, pin: QuickPin) {
    if AUDIO_LIBRARY.contains(&pin.kind) {
        changed(app, &pin.to);
        return;
    }
    if crate::privacy::incognito(app) {
        return;
    }
    let to = pin.to.clone();
    if !change(app, |_, rail| add_auto_pin(rail, pin, now_s())).await {
        changed(app, &to);
    }
}

/// Pins `pin` as added to the library; past `RailPrefs::limit`, the oldest automatic pins go.
fn add_auto_pin(rail: &mut RailPrefs, pin: QuickPin, at: u32) -> bool {
    if !rail.auto_add
        || rail
            .pins
            .iter()
            .any(|other| other.to == pin.to && other.search == pin.search)
    {
        return false;
    }
    rail.pins.push(RailPin {
        kind: pin.kind.to_owned(),
        to: pin.to,
        search: pin.search,
        name: pin.name,
        artwork_url: pin.artwork_url,
        pinned_at: at,
        played_at: 0,
        opened_at: 0,
        auto: true,
    });
    let limit = rail.limit as usize;
    let mut automatic: Vec<(u32, usize)> = rail
        .pins
        .iter()
        .enumerate()
        .filter(|(_, pin)| pin.auto)
        .map(|(index, pin)| (pin.pinned_at, index))
        .collect();
    if automatic.len() > limit {
        automatic.sort();
        let gone: HashSet<usize> = automatic
            .iter()
            .take(automatic.len() - limit)
            .map(|(_, index)| *index)
            .collect();
        let mut index = 0;
        rail.pins.retain(|_| {
            let keep = !gone.contains(&index);
            index += 1;
            keep
        });
    }
    rail.pins.len() <= raygun_model::RAIL_MAX_PINS
}

/// Something left the library: its automatic pin goes. Pages that are gone with it (a playlist,
/// a site) and the audio library's (an artist, a show, a book) lose their pins made by hand too.
pub async fn remove(app: &AppHandle, to: &str, search: &BTreeMap<String, String>) {
    if !change(app, |_, rail| drop_pins(rail, to, search)).await {
        changed(app, to);
    }
}

/// Pins pages by hand, or takes their pins away, all at once (a selection's Pin); returns how
/// many changed. New pins follow the others, in the order given.
pub async fn set_pinned(app: &AppHandle, pins: Vec<QuickPin>, pinned: bool) -> u32 {
    let mut changed = 0;
    change(app, |_, rail| {
        changed = set_page_pins(rail, pins, pinned, now_s());
        changed > 0
    })
    .await;
    changed
}

/// [`set_pinned`]'s change to `rail`: a page pinned automatically becomes pinned by hand, so it
/// stays when it leaves the library.
fn set_page_pins(rail: &mut RailPrefs, pins: Vec<QuickPin>, pinned: bool, at: u32) -> u32 {
    let mut changed = 0;
    for pin in pins {
        let same = |other: &RailPin| other.to == pin.to && other.search == pin.search;
        if !pinned {
            let before = rail.pins.len();
            rail.pins.retain(|other| !same(other));
            changed += (before - rail.pins.len()) as u32;
            continue;
        }
        if let Some(existing) = rail.pins.iter_mut().find(|other| same(other)) {
            if existing.auto {
                existing.auto = false;
                changed += 1;
            }
            continue;
        }
        if rail.pins.len() >= raygun_model::RAIL_MAX_PINS {
            break;
        }
        rail.pins.push(RailPin {
            kind: pin.kind.to_owned(),
            to: pin.to,
            search: pin.search,
            name: pin.name,
            artwork_url: pin.artwork_url,
            pinned_at: at,
            played_at: 0,
            opened_at: 0,
            auto: false,
        });
        changed += 1;
    }
    changed
}

fn drop_pins(rail: &mut RailPrefs, to: &str, search: &BTreeMap<String, String>) -> bool {
    let every = matches!(
        to,
        "/library/playlist" | "/artist" | "/podcast" | "/audiobook"
    ) || search.contains_key("site");
    let before = rail.pins.len();
    rail.pins
        .retain(|pin| !((pin.auto || every) && pin.to == to && &pin.search == search));
    rail.pins.len() != before
}

/// Tells the UI to list quick access again (a playlist was renamed, say).
pub fn changed(app: &AppHandle, page: &str) {
    let _ = QuickAccessChangedEvent(page.to_owned()).emit(app);
}

/// Something was played from the page `to`/`search` (a play context's, `kind` as
/// `PlayContext::kind`): Recently played puts it first. Nothing is kept while incognito is on
/// (M47) or recently played is switched off.
pub async fn played(app: &AppHandle, kind: &str, to: &str, search: &BTreeMap<String, String>) {
    if crate::privacy::incognito(app) || !to.starts_with('/') {
        return;
    }
    let at = now_s();
    change(app, |settings, rail| {
        settings.privacy.recently_played
            && (STAMPED.contains(&kind)
                || rail
                    .pins
                    .iter()
                    .any(|pin| pin.to == to && &pin.search == search))
            && stamp(&mut rail.played, to, search, at)
    })
    .await;
}

/// Puts `to`/`search` first, played `at`; false when it was moments ago already.
fn stamp(
    played: &mut Vec<RailPlayed>,
    to: &str,
    search: &BTreeMap<String, String>,
    at: u32,
) -> bool {
    let same = |stamp: &RailPlayed| stamp.to == to && &stamp.search == search;
    if played
        .iter()
        .any(|stamp| same(stamp) && at.saturating_sub(stamp.at) < STAMP_EVERY_S)
    {
        return false;
    }
    played.retain(|stamp| !same(stamp));
    played.insert(
        0,
        RailPlayed {
            to: to.to_owned(),
            search: search.clone(),
            at,
        },
    );
    played.truncate(RAIL_PLAYED_KEPT);
    true
}

/// Settings → Data clears quick access (M46.51): its pins, when its pages were last played from,
/// or both. True when anything went.
pub async fn forget(app: &AppHandle, pins: bool, played: bool) -> bool {
    change(app, |_, rail| {
        let before = (rail.pins.len(), rail.played.len());
        if pins {
            rail.pins.clear();
        }
        if played {
            rail.played.clear();
        }
        before != (rail.pins.len(), rail.played.len())
    })
    .await
}

/// A quick access change from the UI keeps the stamps stored: Rust keeps when pages were played
/// from, and the UI's copy may be older (or made while incognito). Other settings pass unchanged.
pub async fn keep_stamps(db: &Db, change: SettingChange) -> SettingChange {
    let SettingChange::Rail(rail) = change else {
        return change;
    };
    match crate::settings::load(db).await {
        Ok(stored) => SettingChange::Rail(with_stamps_of(&stored.rail, rail)),
        Err(_) => SettingChange::Rail(rail),
    }
}

fn with_stamps_of(stored: &RailPrefs, mut rail: RailPrefs) -> RailPrefs {
    rail.played = stored.played.clone();
    for pin in &mut rail.pins {
        pin.played_at = 0;
        pin.opened_at = 0;
    }
    rail
}

/// Changes quick access and saves it when `edit` says it changed; the UI hears of it. True when
/// it was saved.
async fn change(app: &AppHandle, edit: impl FnOnce(&Settings, &mut RailPrefs) -> bool) -> bool {
    let _turn = CHANGES.lock().await;
    let db = app.state::<AppDb>().0.clone();
    let Ok(settings) = crate::settings::load(&db).await else {
        return false;
    };
    let mut rail = settings.rail.clone();
    if !edit(&settings, &mut rail) {
        return false;
    }
    match crate::settings::update(&db, SettingChange::Rail(rail)).await {
        Ok(_) => {
            let _ = SettingsChangedEvent("rail".into()).emit(app);
            changed(app, "");
            true
        }
        Err(error) => {
            tracing::warn!(%error, "could not change quick access");
            false
        }
    }
}

/// A stored row's JSON before it's read: older versions' orders, the custom one and Recently
/// opened, are Recently played now.
pub fn migrate_row(value: &str) -> String {
    let Ok(serde_json::Value::Object(mut fields)) = serde_json::from_str(value) else {
        return value.to_owned();
    };
    if matches!(
        fields.get("order").and_then(serde_json::Value::as_str),
        Some("custom" | "opened")
    ) {
        fields.insert("order".to_owned(), "played".into());
    }
    serde_json::Value::Object(fields).to_string()
}

/// A row of an older version (`version` 0) as quick access is now:
/// - its pins' played stamps move to `played`;
/// - automatic pins of the audio library go, since it lists itself;
/// - the first run's pins (Favorites, Downloaded, Local files and the playlists there were, all
///   pinned in the same moment) go too, so they're sorted like the rest of the library;
/// - every other pin, made by hand, stays pinned, in its order.
///
/// Its custom order and Recently opened are Recently played by then ([`migrate_row`]).
pub fn migrated(mut rail: RailPrefs) -> RailPrefs {
    if rail.version >= RAIL_VERSION {
        return rail;
    }
    for pin in &rail.pins {
        if pin.played_at == 0 {
            continue;
        }
        let known = rail
            .played
            .iter_mut()
            .find(|stamp| stamp.to == pin.to && stamp.search == pin.search);
        match known {
            Some(stamp) => stamp.at = stamp.at.max(pin.played_at),
            None => rail.played.push(RailPlayed {
                to: pin.to.clone(),
                search: pin.search.clone(),
                at: pin.played_at,
            }),
        }
    }
    rail.played.sort_by_key(|stamp| std::cmp::Reverse(stamp.at));
    rail.played.truncate(RAIL_PLAYED_KEPT);

    let seeded_at = rail
        .pins
        .iter()
        .filter(|pin| !pin.auto && FIXED.contains(&pin.kind.as_str()))
        .map(|pin| pin.pinned_at)
        .min()
        .filter(|_| rail.seeded);
    let first_run = |pin: &RailPin| {
        seeded_at.is_some_and(|at| {
            !pin.auto
                && (FIXED.contains(&pin.kind.as_str()) || pin.kind == "playlist")
                && pin.pinned_at.abs_diff(at) <= SEED_WINDOW_S
        })
    };
    rail.pins.retain(|pin| {
        let listed_anyway = pin.auto && AUDIO_LIBRARY.contains(&pin.kind.as_str());
        !listed_anyway && !first_run(pin)
    });
    for pin in &mut rail.pins {
        pin.played_at = 0;
        pin.opened_at = 0;
    }
    rail.version = RAIL_VERSION;
    rail
}

/// Whether `kind` is listed by `kinds` (kinds outside the audio library always are).
fn kind_on(kinds: &RailKinds, kind: &str) -> bool {
    match kind {
        "favorites" => kinds.favorites,
        "downloaded" => kinds.downloaded,
        "localFiles" => kinds.local_files,
        "playlist" => kinds.playlists,
        "album" => kinds.albums,
        "artist" => kinds.artists,
        "podcast" => kinds.podcasts,
        "audiobook" => kinds.audiobooks,
        _ => true,
    }
}

/// Equals keep the fixed blocks' order, before everything else.
fn fixed_rank(item: &QuickAccessItem) -> usize {
    FIXED
        .iter()
        .position(|kind| *kind == item.kind)
        .unwrap_or(FIXED.len())
}

fn by_name(a: &QuickAccessItem, b: &QuickAccessItem) -> Ordering {
    a.name
        .to_lowercase()
        .cmp(&b.name.to_lowercase())
        .then_with(|| a.name.cmp(&b.name))
}

/// The last played from first, then what never was, the newest added first.
fn by_played(a: &QuickAccessItem, b: &QuickAccessItem) -> Ordering {
    b.played_at.cmp(&a.played_at).then_with(|| by_added(a, b))
}

fn by_added(a: &QuickAccessItem, b: &QuickAccessItem) -> Ordering {
    b.added_at
        .cmp(&a.added_at)
        .then_with(|| fixed_rank(a).cmp(&fixed_rank(b)))
        .then_with(|| by_name(a, b))
}

/// How a list in `order` shows.
fn shown_order(order: RailOrder) -> fn(&QuickAccessItem, &QuickAccessItem) -> Ordering {
    match order {
        RailOrder::Played => by_played,
        RailOrder::Added => by_added,
        RailOrder::Name => |a, b| by_name(a, b).then_with(|| fixed_rank(a).cmp(&fixed_rank(b))),
    }
}

/// Which items are the most recent, for the cap: as added for Recently added, else as played.
fn recent_order(order: RailOrder) -> fn(&QuickAccessItem, &QuickAccessItem) -> Ordering {
    match order {
        RailOrder::Added => by_added,
        RailOrder::Played | RailOrder::Name => by_played,
    }
}

/// Quick access from `library` (every audio library page, unpinned, never played) and `rail`:
/// - pins first, in their order (with the library's own row when it has one, else as pinned;
///   pins `shows_pin` refuses, of pages that are gone, are left out);
/// - then the library pages of kinds that are on, the `shown` most recent, in `order`.
///
/// Without `pinned_first` the pins are sorted with the rest. `cover` makes a pin's saved cover
/// address one to show.
pub fn arrange(
    rail: &RailPrefs,
    library: Vec<QuickAccessItem>,
    shows_pin: impl Fn(&RailPin) -> bool,
    cover: impl Fn(&str) -> String,
) -> QuickAccessList {
    let mut played: HashMap<String, u32> = HashMap::new();
    for stamp in &rail.played {
        let at = played
            .entry(page_key(&stamp.to, &stamp.search))
            .or_default();
        *at = (*at).max(stamp.at);
    }
    let played_at = |item: &QuickAccessItem| {
        played
            .get(&page_key(&item.to, &item.search))
            .copied()
            .unwrap_or(0)
    };

    let mut library: Vec<Option<QuickAccessItem>> = library.into_iter().map(Some).collect();
    let mut place: HashMap<String, usize> = HashMap::new();
    for (index, item) in library.iter().enumerate() {
        if let Some(item) = item {
            place
                .entry(page_key(&item.to, &item.search))
                .or_insert(index);
        }
    }

    let mut pins: Vec<QuickAccessItem> = Vec::new();
    let mut pinned_keys = HashSet::new();
    for pin in &rail.pins {
        let key = page_key(&pin.to, &pin.search);
        if !pinned_keys.insert(key.clone()) {
            continue;
        }
        let own = place
            .get(&key)
            .and_then(|index| library.get_mut(*index))
            .and_then(Option::take);
        let item = match own {
            Some(item) => item,
            None if shows_pin(pin) => QuickAccessItem {
                kind: pin.kind.clone(),
                to: pin.to.clone(),
                search: pin.search.clone(),
                name: pin.name.clone(),
                subtitle: None,
                artwork_url: pin.artwork_url.as_deref().map(&cover),
                artwork_urls: Vec::new(),
                pinned: false,
                library: false,
                removable: false,
                added_at: pin.pinned_at,
                played_at: 0,
            },
            None => continue,
        };
        pins.push(QuickAccessItem {
            pinned: true,
            played_at: played_at(&item),
            ..item
        });
    }

    let mut rest: Vec<QuickAccessItem> = library
        .into_iter()
        .flatten()
        .filter(|item| kind_on(&rail.kinds, &item.kind))
        .map(|item| QuickAccessItem {
            pinned: false,
            played_at: played_at(&item),
            ..item
        })
        .collect();
    rest.sort_by(recent_order(rail.order));
    let shown = rail.shown as usize;
    let more = rest.len().saturating_sub(shown);
    rest.truncate(shown);
    rest.sort_by(shown_order(rail.order));

    let items = if rail.pinned_first {
        pins.into_iter().chain(rest).collect()
    } else {
        let mut all: Vec<QuickAccessItem> = pins.into_iter().chain(rest).collect();
        all.sort_by(shown_order(rail.order));
        all
    };
    QuickAccessList {
        items,
        more: u32::try_from(more).unwrap_or(u32::MAX),
    }
}

/// What quick access shows now; `names` are the fixed blocks' names in the UI's language.
pub async fn list(app: &AppHandle, names: QuickAccessNames) -> Result<QuickAccessList, String> {
    let db = app.state::<AppDb>().0.clone();
    let settings = crate::settings::load(&db)
        .await
        .map_err(|error| error.to_string())?;
    let library = library(app, &db, &names).await;
    let playlists: HashSet<String> = library
        .iter()
        .filter(|item| item.kind == "playlist")
        .filter_map(|item| item.search.get("id").cloned())
        .collect();
    let sites: Option<HashSet<String>> = raygun_db::websites::list(&db)
        .await
        .ok()
        .map(|sites| sites.into_iter().map(|site| site.id).collect());
    let images = app.state::<raygun_net::ImageLinks>();
    Ok(arrange(
        &settings.rail,
        library,
        // A playlist's or a site's pin shows only while it's there (a new playlist's pin can
        // arrive before the playlist).
        |pin| {
            if pin.to == "/library/playlist" {
                return pin
                    .search
                    .get("id")
                    .is_some_and(|id| playlists.contains(id));
            }
            match (pin.search.get("site"), &sites) {
                (Some(site), Some(sites)) if pin.kind == "website" => sites.contains(site),
                _ => true,
            }
        },
        |url| shown_cover(&images, url),
    ))
}

/// A cover address quick access saved (a pin's, a followed artist's) as a link to show: a
/// source's through the cover cache, kept there whatever its size limit ([`kept_tokens`]); one of
/// the proxy's own links from an earlier start at this run's address, where its copy on disk is.
fn shown_cover(images: &raygun_net::ImageLinks, url: &str) -> String {
    let relinked = images.relink(&images.original(url));
    if images.is_remote(&relinked) {
        images.kept_cover(&relinked, &BTreeMap::new())
    } else {
        relinked
    }
}

/// The cover cache's names for what quick access shows from its own addresses: pinned pages'
/// covers, followed artists' pictures and the pictures found elsewhere for them (M46.40). The
/// cache's sweep keeps them on the device.
pub async fn kept_tokens(app: &AppHandle) -> Vec<String> {
    let db = app.state::<AppDb>().0.clone();
    let images = app.state::<raygun_net::ImageLinks>();
    let follows = app
        .state::<Arc<crate::follows::ArtistFollows>>()
        .list()
        .await;
    let pins = crate::settings::load(&db)
        .await
        .map(|settings| settings.rail.pins)
        .unwrap_or_default();
    let mut artists: Vec<String> = follows.iter().map(|artist| artist.name.clone()).collect();
    artists.extend(
        pins.iter()
            .filter(|pin| pin.kind == "artist")
            .map(|pin| pin.name.clone()),
    );
    let fallbacks = app.state::<Arc<crate::cover_fallback::CoverFallbacks>>();
    let mut addresses: Vec<String> = follows
        .into_iter()
        .filter_map(|artist| artist.image_url)
        .chain(pins.into_iter().filter_map(|pin| pin.artwork_url))
        .collect();
    addresses.extend(fallbacks.artist_addresses(&artists).await);
    addresses
        .iter()
        .filter_map(|url| cover_token(&images, url))
        .collect()
}

/// The cover cache's name for a saved cover address: a source's hashed as the proxy does, one of
/// the proxy's own cover links by the name it carries.
fn cover_token(images: &raygun_net::ImageLinks, url: &str) -> Option<String> {
    let relinked = images.relink(&images.original(url));
    if images.is_remote(&relinked) {
        return Some(raygun_net::proxy_image_token(&relinked, &BTreeMap::new()));
    }
    relinked
        .split_once("/cover/")
        .map(|(_, token)| token.to_owned())
        .filter(|token| !token.is_empty() && !token.contains('/'))
}

/// A quick access change from the UI with its pins' covers at their sources' addresses: a
/// proxy link lasts only while this run knows it.
pub fn with_source_covers(app: &AppHandle, change: SettingChange) -> SettingChange {
    let SettingChange::Rail(mut rail) = change else {
        return change;
    };
    if let Some(images) = app.try_state::<raygun_net::ImageLinks>() {
        for pin in &mut rail.pins {
            if let Some(url) = pin.artwork_url.as_deref() {
                pin.artwork_url = Some(images.original(url));
            }
        }
    }
    SettingChange::Rail(rail)
}

fn library_item(
    kind: &str,
    to: &str,
    search: BTreeMap<String, String>,
    name: String,
) -> QuickAccessItem {
    QuickAccessItem {
        kind: kind.to_owned(),
        to: to.to_owned(),
        search,
        name,
        subtitle: None,
        artwork_url: None,
        artwork_urls: Vec::new(),
        pinned: false,
        library: true,
        removable: false,
        added_at: 0,
        played_at: 0,
    }
}

/// Every page of the audio library; a part that can't be read is left out (and logged).
async fn library(app: &AppHandle, db: &Arc<Db>, names: &QuickAccessNames) -> Vec<QuickAccessItem> {
    let mut items = vec![
        library_item(
            "favorites",
            "/library",
            search(&[("view", "favorites")]),
            names.favorites.clone(),
        ),
        library_item(
            "downloaded",
            "/library",
            search(&[("view", "downloaded")]),
            names.downloaded.clone(),
        ),
        library_item(
            "localFiles",
            "/local-files",
            BTreeMap::new(),
            names.local_files.clone(),
        ),
    ];
    let warn = |part: &str, error: &dyn std::fmt::Display| {
        tracing::warn!(%error, part, "quick access left part of the library out");
    };

    // Playlists, with when each was made.
    let collections = app.state::<crate::collections::CollectionsService>();
    match (
        collections.playlists().await,
        raygun_db::collections::playlists(db).await,
    ) {
        (Ok(summaries), Ok(rows)) => {
            let made: HashMap<String, i64> = rows
                .into_iter()
                .map(|row| (row.id, row.created_at))
                .collect();
            for playlist in summaries {
                let added = made.get(&playlist.id).copied().unwrap_or(0) as f64;
                items.push(QuickAccessItem {
                    artwork_url: playlist.cover_url,
                    artwork_urls: playlist.artwork_urls,
                    removable: true,
                    added_at: ms_to_s(added),
                    ..library_item(
                        "playlist",
                        "/library/playlist",
                        search(&[("id", &playlist.id)]),
                        playlist.name,
                    )
                });
            }
        }
        (Err(error), _) => warn("playlists", &error),
        (_, Err(error)) => warn("playlists", &error),
    }

    // The device's own albums, from when their first song came.
    let local = app.state::<crate::library::LibraryService>();
    let first_seen: HashMap<(String, String), u32> =
        match raygun_db::local_music::albums_first_seen(db).await {
            Ok(rows) => rows
                .into_iter()
                .map(|(artist, album, seconds)| {
                    let seconds = u32::try_from(seconds.max(0)).unwrap_or(u32::MAX);
                    ((artist.to_lowercase(), album.to_lowercase()), seconds)
                })
                .collect(),
            Err(error) => {
                warn("albums", &error);
                HashMap::new()
            }
        };
    let mut artist_seen: HashMap<String, u32> = HashMap::new();
    for ((artist, _), seconds) in &first_seen {
        let earliest = artist_seen.entry(artist.clone()).or_insert(*seconds);
        *earliest = (*earliest).min(*seconds);
    }
    match local.albums(None).await {
        Ok(albums) => {
            for album in albums {
                let added = first_seen
                    .get(&(album.artist.to_lowercase(), album.title.to_lowercase()))
                    .copied()
                    .unwrap_or(0);
                items.push(QuickAccessItem {
                    subtitle: Some(album.artist.clone()),
                    artwork_url: album.artwork_url,
                    added_at: added,
                    ..library_item(
                        "album",
                        "/library/album",
                        search(&[("artist", &album.artist), ("album", &album.title)]),
                        album.title,
                    )
                });
            }
        }
        Err(error) => warn("albums", &error),
    }

    // Followed artists and the device's own, once each.
    let follows = app.state::<Arc<crate::follows::ArtistFollows>>();
    let images = app.state::<raygun_net::ImageLinks>();
    let no_headers = BTreeMap::new();
    let mut artists: Vec<QuickAccessItem> = follows
        .list()
        .await
        .into_iter()
        .map(|artist| QuickAccessItem {
            artwork_url: artist
                .image_url
                .as_deref()
                .map(|url| shown_cover(&images, url)),
            removable: true,
            added_at: ms_to_s(artist.followed_at),
            ..library_item(
                "artist",
                "/artist",
                search(&[("name", &artist.name)]),
                artist.name,
            )
        })
        .collect();
    match local.artists().await {
        Ok(local_artists) => {
            for artist in local_artists {
                let folded = artist.name.trim().to_lowercase();
                let seen = artist_seen
                    .get(&artist.name.to_lowercase())
                    .copied()
                    .unwrap_or(0);
                match artists
                    .iter_mut()
                    .find(|followed| followed.name.trim().to_lowercase() == folded)
                {
                    // Unfollowing keeps it here: its songs are on the device.
                    Some(followed) => {
                        followed.removable = false;
                        followed.artwork_url = followed.artwork_url.take().or(artist.artwork_url);
                        if seen > 0 && (followed.added_at == 0 || seen < followed.added_at) {
                            followed.added_at = seen;
                        }
                    }
                    None => artists.push(QuickAccessItem {
                        artwork_url: artist.artwork_url,
                        added_at: seen,
                        ..library_item(
                            "artist",
                            "/artist",
                            search(&[("name", &artist.name)]),
                            artist.name,
                        )
                    }),
                }
            }
        }
        Err(error) => warn("artists", &error),
    }
    items.extend(artists);

    // Followed shows and the books kept.
    match raygun_db::podcasts::subscriptions(db).await {
        Ok(rows) => {
            for row in rows {
                let podcast = row.podcast;
                items.push(QuickAccessItem {
                    subtitle: podcast.fields.author,
                    artwork_url: podcast
                        .fields
                        .artwork
                        .as_deref()
                        .map(|url| images.kept_cover(url, &no_headers)),
                    removable: true,
                    added_at: ms_to_s(podcast.subscribed_at.unwrap_or(0) as f64),
                    ..library_item(
                        "podcast",
                        "/podcast",
                        search(&[("id", &podcast.id)]),
                        podcast.fields.title,
                    )
                });
            }
        }
        Err(error) => warn("podcasts", &error),
    }
    let podcasts = app.state::<Arc<crate::podcasts::PodcastService>>();
    match (
        podcasts.audiobooks().await,
        raygun_db::podcasts::audiobooks(db).await,
    ) {
        (Ok(books), Ok(rows)) => {
            let kept: HashMap<String, i64> = rows
                .into_iter()
                .map(|row| (row.podcast.id, row.podcast.subscribed_at.unwrap_or(0)))
                .collect();
            for book in books {
                let added = kept.get(&book.id).copied().unwrap_or(0) as f64;
                items.push(QuickAccessItem {
                    subtitle: book.author,
                    artwork_url: book.artwork_url,
                    removable: true,
                    added_at: ms_to_s(added),
                    ..library_item(
                        "audiobook",
                        "/audiobook",
                        search(&[("id", &book.id)]),
                        book.title,
                    )
                });
            }
        }
        (Err(error), _) => warn("audiobooks", &error),
        (_, Err(error)) => warn("audiobooks", &error),
    }
    items
}

#[cfg(test)]
mod tests {
    use raygun_model::RAIL_SHOWN_DEFAULT;

    use super::*;

    fn pin(kind: &str, to: &str, id: &str, pinned_at: u32) -> RailPin {
        RailPin {
            kind: kind.into(),
            to: to.into(),
            search: search(&[("id", id)]),
            name: id.into(),
            artwork_url: None,
            pinned_at,
            played_at: 0,
            opened_at: 0,
            auto: false,
        }
    }

    fn fixed_pin(kind: &str, pinned_at: u32) -> RailPin {
        let (to, search) = match kind {
            "favorites" => ("/library", search(&[("view", "favorites")])),
            "downloaded" => ("/library", search(&[("view", "downloaded")])),
            _ => ("/local-files", BTreeMap::new()),
        };
        RailPin {
            to: to.into(),
            search,
            ..pin(kind, "", "", pinned_at)
        }
    }

    fn item(kind: &str, id: &str, added_at: u32) -> QuickAccessItem {
        let to = match kind {
            "playlist" => "/library/playlist",
            "podcast" => "/podcast",
            "audiobook" => "/audiobook",
            _ => "/artist",
        };
        QuickAccessItem {
            added_at,
            ..library_item(kind, to, search(&[("id", id)]), id.to_owned())
        }
    }

    fn fixed(kind: &str) -> QuickAccessItem {
        let pin = fixed_pin(kind, 0);
        library_item(kind, &pin.to, pin.search, kind.to_owned())
    }

    fn played(to: &str, id: &str, at: u32) -> RailPlayed {
        RailPlayed {
            to: to.into(),
            search: search(&[("id", id)]),
            at,
        }
    }

    fn names(list: &QuickAccessList) -> Vec<(&str, bool)> {
        list.items
            .iter()
            .map(|item| (item.name.as_str(), item.pinned))
            .collect()
    }

    fn arranged(rail: &RailPrefs, library: Vec<QuickAccessItem>) -> QuickAccessList {
        arrange(rail, library, |_| true, str::to_owned)
    }

    #[test]
    fn the_last_played_from_comes_first_then_the_newest_added() {
        let rail = RailPrefs {
            played: vec![
                played("/podcast", "show", 500),
                RailPlayed {
                    to: "/library".into(),
                    search: search(&[("view", "favorites")]),
                    at: 300,
                },
            ],
            ..RailPrefs::default()
        };
        let library = vec![
            fixed("favorites"),
            fixed("downloaded"),
            fixed("localFiles"),
            item("playlist", "old", 10),
            item("playlist", "new", 20),
            item("podcast", "show", 5),
        ];
        let list = arranged(&rail, library);
        assert_eq!(
            names(&list),
            [
                ("show", false),
                ("favorites", false),
                ("new", false),
                ("old", false),
                ("downloaded", false),
                ("localFiles", false),
            ],
            "Favorites leads only once played from; never played ones keep their order"
        );
        assert_eq!(list.items[0].played_at, 500);
        assert_eq!(list.more, 0);
    }

    #[test]
    fn pins_stay_on_top_in_their_order_and_always_show() {
        let mut rail = RailPrefs {
            pins: vec![
                pin("manga", "/manga/title", "comic", 1),
                pin("playlist", "/library/playlist", "b", 2),
            ],
            played: vec![played("/library/playlist", "a", 900)],
            shown: 1,
            kinds: RailKinds {
                playlists: false,
                ..RailKinds::default()
            },
            ..RailPrefs::default()
        };
        let library = vec![
            item("playlist", "a", 10),
            QuickAccessItem {
                removable: true,
                ..item("playlist", "b", 20)
            },
            item("podcast", "show", 30),
            item("audiobook", "book", 40),
        ];
        let list = arranged(&rail, library.clone());
        // Playlists are off: only the pinned one shows. One more is listed, the newest.
        assert_eq!(
            names(&list),
            [("comic", true), ("b", true), ("book", false)]
        );
        assert_eq!(list.more, 1, "the show is past the cap");
        assert!(
            list.items[1].removable,
            "a pinned library page is the library's own row"
        );
        assert!(
            !list.items[0].removable && !list.items[0].library,
            "a pin of another page isn't"
        );

        // Not on top: pins are sorted with the rest, and still don't count.
        rail.pinned_first = false;
        rail.order = RailOrder::Name;
        let list = arranged(&rail, library);
        assert_eq!(
            names(&list),
            [("b", true), ("book", false), ("comic", true)]
        );
    }

    #[test]
    fn many_pages_pin_and_unpin_together() {
        let mut rail = RailPrefs {
            pins: vec![
                pin("manga", "/manga/title", "comic", 1),
                RailPin {
                    auto: true,
                    ..pin("audiobook", "/audiobook", "book", 2)
                },
            ],
            ..RailPrefs::default()
        };
        let pages = || {
            vec![
                page_pin("podcast", "/podcast", "show", "Show", None),
                page_pin("audiobook", "/audiobook", "book", "Book", None),
                page_pin("podcast", "/podcast", "show", "Show", None),
            ]
        };
        // The show is new; the book's automatic pin becomes one made by hand; the repeat is one.
        assert_eq!(set_page_pins(&mut rail, pages(), true, 50), 2);
        let pinned: Vec<(&str, bool, u32)> = rail
            .pins
            .iter()
            .map(|pin| (pin.name.as_str(), pin.auto, pin.pinned_at))
            .collect();
        assert_eq!(
            pinned,
            [("comic", false, 1), ("book", false, 2), ("Show", false, 50)]
        );
        assert_eq!(
            set_page_pins(&mut rail, pages(), true, 60),
            0,
            "pinned already"
        );

        assert_eq!(set_page_pins(&mut rail, pages(), false, 70), 2);
        assert_eq!(
            rail.pins
                .iter()
                .map(|pin| pin.name.as_str())
                .collect::<Vec<_>>(),
            ["comic"]
        );
    }

    #[test]
    fn the_cap_keeps_the_most_recent_and_counts_no_pins() {
        let library: Vec<QuickAccessItem> = (0..150)
            .map(|index| item("playlist", &format!("p{index:03}"), index))
            .collect();
        let rail = RailPrefs {
            pins: vec![pin("playlist", "/library/playlist", "p000", 1)],
            played: vec![played("/library/playlist", "p001", 99)],
            ..RailPrefs::default()
        };
        assert_eq!(rail.shown, RAIL_SHOWN_DEFAULT);
        let list = arranged(&rail, library.clone());
        assert_eq!(list.items.len(), 101, "100 and the pin");
        assert_eq!(list.more, 49);
        assert_eq!(
            names(&list)[..3],
            [("p000", true), ("p001", false), ("p149", false)]
        );
        // The oldest added ones are left out.
        assert!(!list.items.iter().any(|item| item.name == "p002"));

        // By name, the same most recent ones, in name order.
        let by_name = RailPrefs {
            order: RailOrder::Name,
            ..rail.clone()
        };
        let list = arranged(&by_name, library);
        assert_eq!(list.items.len(), 101);
        assert_eq!(
            names(&list)[..3],
            [("p000", true), ("p001", false), ("p051", false)]
        );
    }

    #[test]
    fn pins_of_pages_that_are_gone_are_left_out() {
        let rail = RailPrefs {
            pins: vec![
                pin("playlist", "/library/playlist", "deleted", 1),
                pin("album", "/music/album", "online", 2),
            ],
            ..RailPrefs::default()
        };
        let list = arrange(
            &rail,
            vec![item("playlist", "kept", 1)],
            |pin| pin.to != "/library/playlist",
            |url| format!("cover:{url}"),
        );
        assert_eq!(names(&list), [("online", true), ("kept", false)]);
        assert_eq!(
            list.items[0].added_at, 2,
            "a pin's page was added when pinned"
        );
    }

    #[test]
    fn plays_are_stamped_once_a_moment_and_newest_first() {
        let mut stamps = Vec::new();
        let page = search(&[("id", "a")]);
        assert!(stamp(&mut stamps, "/library/playlist", &page, 100));
        assert!(
            !stamp(&mut stamps, "/library/playlist", &page, 110),
            "moments later"
        );
        assert!(stamp(&mut stamps, "/podcast", &page, 120));
        assert!(stamp(&mut stamps, "/library/playlist", &page, 200));
        let order: Vec<(&str, u32)> = stamps
            .iter()
            .map(|stamp| (stamp.to.as_str(), stamp.at))
            .collect();
        assert_eq!(order, [("/library/playlist", 200), ("/podcast", 120)]);
    }

    #[test]
    fn the_ui_never_sets_the_stamps() {
        let stored = RailPrefs {
            played: vec![played("/podcast", "show", 500)],
            ..RailPrefs::default()
        };
        let mut sent = RailPrefs {
            pins: vec![pin("playlist", "/library/playlist", "a", 1)],
            played: vec![played("/podcast", "show", 999)],
            order: RailOrder::Name,
            ..RailPrefs::default()
        };
        sent.pins[0].played_at = 999;
        let kept = with_stamps_of(&stored, sent);
        assert_eq!(kept.played, stored.played);
        assert_eq!(kept.pins[0].played_at, 0);
        assert_eq!(kept.order, RailOrder::Name, "the rest of the change stays");
    }

    #[test]
    fn older_rows_keep_the_pins_made_by_hand_and_their_plays() {
        let seeded = 1_000;
        let mut road_trip = pin("playlist", "/library/playlist", "road-trip", seeded);
        road_trip.played_at = 700;
        let mut show = pin("podcast", "/podcast", "show", 5_000);
        show.auto = true;
        show.played_at = 800;
        let mut comic = pin("manga", "/manga/title", "comic", 6_000);
        comic.auto = true;
        let old = RailPrefs {
            pins: vec![
                fixed_pin("favorites", seeded),
                fixed_pin("downloaded", seeded + 1),
                fixed_pin("localFiles", seeded + 1),
                road_trip,
                show,
                pin("playlist", "/library/playlist", "pinned-later", 4_000),
                pin("album", "/music/album", "online", 4_500),
                comic,
            ],
            seeded: true,
            version: 0,
            ..RailPrefs::default()
        };
        let new = migrated(old);
        let kept: Vec<&str> = new.pins.iter().map(|pin| pin.name.as_str()).collect();
        assert_eq!(
            kept,
            ["pinned-later", "online", "comic"],
            "the first run's pins and the library's automatic ones go"
        );
        assert!(new.pins.iter().all(|pin| pin.played_at == 0));
        let plays: Vec<(&str, u32)> = new
            .played
            .iter()
            .map(|stamp| (stamp.search["id"].as_str(), stamp.at))
            .collect();
        assert_eq!(plays, [("show", 800), ("road-trip", 700)]);
        assert_eq!(new.version, RAIL_VERSION);
        assert_eq!(migrated(new.clone()), new, "once only");
    }

    #[test]
    fn older_rows_read_with_their_order_and_new_fields() {
        let row = r#"{"pins":[{"kind":"favorites","to":"/library","search":{"view":"favorites"},
            "name":"","artworkUrl":null,"pinnedAt":5,"playedAt":0,"openedAt":0}],
            "order":"custom","seeded":false}"#;
        let old: RailPrefs = serde_json::from_str(&migrate_row(row)).unwrap();
        assert_eq!(old.order, RailOrder::Played);
        assert_eq!(old.version, 0);
        assert_eq!(old.shown, RAIL_SHOWN_DEFAULT);
        assert!(old.pinned_first && old.kinds.local_files);
        let new = migrated(old);
        assert_eq!(
            new.pins.len(),
            1,
            "not the first run's: it was never seeded"
        );
        for (stored, read) in [
            ("opened", RailOrder::Played),
            ("played", RailOrder::Played),
            ("name", RailOrder::Name),
        ] {
            let row = format!(r#"{{"pins":[],"order":"{stored}","seeded":true}}"#);
            let prefs: RailPrefs = serde_json::from_str(&migrate_row(&row)).unwrap();
            assert_eq!(prefs.order, read);
        }
        assert_eq!(migrate_row("not json"), "not json");
        assert!(RailPrefs::default().validate().is_ok());
        assert!(
            RailPrefs {
                shown: 5,
                ..RailPrefs::default()
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn audio_pages_leaving_the_library_lose_every_pin() {
        let mut automatic = pin("manga", "/manga/title", "comic", 1);
        automatic.auto = true;
        let mut rail = RailPrefs {
            pins: vec![
                pin("artist", "/artist", "x", 1),
                pin("manga", "/manga/title", "kept", 2),
                automatic,
            ],
            ..RailPrefs::default()
        };
        assert!(drop_pins(&mut rail, "/artist", &search(&[("id", "x")])));
        assert!(!drop_pins(
            &mut rail,
            "/manga/title",
            &search(&[("id", "kept")])
        ));
        assert!(drop_pins(
            &mut rail,
            "/manga/title",
            &search(&[("id", "comic")])
        ));
        let left: Vec<&str> = rail.pins.iter().map(|pin| pin.name.as_str()).collect();
        assert_eq!(left, ["kept"], "a comic pinned by hand stays");
    }

    #[test]
    fn only_other_kinds_are_pinned_when_added() {
        let mut rail = RailPrefs {
            limit: 2,
            ..RailPrefs::default()
        };
        for (index, id) in ["a", "b", "c"].into_iter().enumerate() {
            let comic = QuickPin {
                kind: "manga",
                to: "/manga/title".into(),
                search: search(&[("id", id)]),
                name: id.into(),
                artwork_url: None,
            };
            assert!(add_auto_pin(&mut rail, comic, index as u32));
        }
        let left: Vec<&str> = rail.pins.iter().map(|pin| pin.name.as_str()).collect();
        assert_eq!(left, ["b", "c"], "past the limit the oldest go");
        assert!(rail.pins.iter().all(|pin| pin.auto));
    }
}
