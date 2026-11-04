//! Hidden artists and "Not interested", as Spotify's "Don't play this artist": `Settings::hidden`,
//! applied where Home's rows, Made for You's mixes, charts and genre rows are made. Searches and
//! an artist's own page still show them. Changes go through here one at a time, so two quick ones
//! don't save each other away.

use std::collections::{BTreeMap, HashSet};

use raygun_model::{
    HIDDEN_ARTISTS_MAX, HIDDEN_ITEMS_MAX, HiddenArtist, HiddenItem, HiddenPrefs, OnlineTrack,
    PlayContext, SettingChange, Settings, TrackQuery,
};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::sync::Mutex;

use crate::events::SettingsChangedEvent;
use crate::search::{credited_artists, credits_include};
use crate::startup::AppDb;

/// One change at a time: two at once would each save the other's away.
static CHANGES: Mutex<()> = Mutex::const_new(());

fn now_s() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| u32::try_from(elapsed.as_secs()).unwrap_or(u32::MAX))
        .unwrap_or_default()
}

/// A name as compared: case and runs of spaces don't count.
pub fn fold(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// A song's key, as the resolver makes it (and the UI's `trackKey`).
pub fn song_key(title: &str, artist: Option<&str>) -> String {
    raygun_resolver::track_key(&TrackQuery {
        title: title.to_owned(),
        artist: artist.map(str::to_owned),
        album: None,
        duration_ms: None,
    })
}

/// A page's address as a hidden item's key: its path and its parameters in order
/// (`/music/album?id=1&provider=x`), as the UI's `pageKey` makes it.
pub fn page_key(to: &str, search: &BTreeMap<String, String>) -> String {
    let parameters: Vec<String> = search
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect();
    format!("{to}?{}", parameters.join("&"))
}

/// What's hidden, ready to check songs and rows against.
#[derive(Debug, Clone, Default)]
pub struct Hidden {
    artists: HashSet<String>,
    /// Hidden names that are credits themselves ("Simon & Garfunkel"), found inside longer ones.
    compound: Vec<String>,
    /// Keys of hidden songs, pages and feeds.
    items: HashSet<String>,
}

impl Hidden {
    pub fn new(prefs: &HiddenPrefs) -> Self {
        let artists: HashSet<String> = prefs
            .artists
            .iter()
            .map(|artist| fold(&artist.name))
            .filter(|name| !name.is_empty())
            .collect();
        let compound = artists
            .iter()
            .filter(|name| credited_artists(name).len() > 1)
            .cloned()
            .collect();
        Self {
            artists,
            compound,
            items: prefs.items.iter().map(|item| item.key.clone()).collect(),
        }
    }

    /// What's hidden now. Only the one setting is read; a broken row hides nothing.
    pub async fn load(db: &raygun_db::Db) -> Self {
        let rows = raygun_db::settings::load(db).await.unwrap_or_default();
        rows.into_iter()
            .find(|(key, _)| key == "hidden")
            .and_then(|(_, json)| serde_json::from_str::<HiddenPrefs>(&json).ok())
            .map(|prefs| Self::new(&prefs))
            .unwrap_or_default()
    }

    pub fn is_empty(&self) -> bool {
        self.artists.is_empty() && self.items.is_empty()
    }

    pub fn hides_artist(&self, name: &str) -> bool {
        !self.artists.is_empty() && self.artists.contains(&fold(name))
    }

    /// Whether a credit ("A, B & C feat. D") names a hidden artist.
    pub fn hides_credit(&self, credit: Option<&str>) -> bool {
        let Some(credit) = credit.map(str::trim).filter(|credit| !credit.is_empty()) else {
            return false;
        };
        if self.artists.is_empty() {
            return false;
        }
        self.artists.contains(&fold(credit))
            || credited_artists(credit)
                .iter()
                .any(|part| self.artists.contains(&fold(part)))
            || self
                .compound
                .iter()
                .any(|name| credits_include(credit, name))
    }

    pub fn hides_item(&self, key: &str) -> bool {
        self.items.contains(key)
    }

    /// A song by a hidden artist, or one marked Not interested.
    pub fn hides_song(&self, title: &str, artist: Option<&str>) -> bool {
        self.hides_credit(artist)
            || (!self.items.is_empty() && self.items.contains(&song_key(title, artist)))
    }

    pub fn hides_track(&self, track: &OnlineTrack) -> bool {
        self.hides_song(&track.title, track.artist.as_deref())
    }

    /// Leaves out hidden songs.
    pub fn retain_tracks(&self, tracks: &mut Vec<OnlineTrack>) {
        if !self.is_empty() {
            tracks.retain(|track| !self.hides_track(track));
        }
    }

    /// A Your music tile for a hidden artist's page, or one marked Not interested.
    pub fn hides_context(&self, context: &PlayContext) -> bool {
        (context.kind == "artist" && self.hides_artist(&context.name))
            || self.hides_item(&page_key(&context.to, &context.search))
    }
}

/// Keeps the newest `max` of `list`, by `at`.
fn keep_newest<T>(list: &mut Vec<T>, max: usize, at: impl Fn(&T) -> u32) {
    if list.len() <= max {
        return;
    }
    list.sort_by_key(|entry| std::cmp::Reverse(at(entry)));
    list.truncate(max);
}

/// `prefs` with `name` hidden or shown again; the same artist is listed once.
pub fn with_artist(mut prefs: HiddenPrefs, name: &str, hidden: bool, now: u32) -> HiddenPrefs {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    let folded = fold(&name);
    prefs.artists.retain(|artist| fold(&artist.name) != folded);
    if hidden && !name.is_empty() {
        prefs.artists.push(HiddenArtist {
            name,
            hidden_at: now,
        });
        keep_newest(&mut prefs.artists, HIDDEN_ARTISTS_MAX, |artist| {
            artist.hidden_at
        });
    }
    prefs
}

/// `prefs` with `item` hidden or shown again, by its key.
pub fn with_item(mut prefs: HiddenPrefs, item: HiddenItem, hidden: bool, now: u32) -> HiddenPrefs {
    prefs.items.retain(|other| other.key != item.key);
    if hidden {
        prefs.items.push(HiddenItem {
            hidden_at: now,
            ..item
        });
        keep_newest(&mut prefs.items, HIDDEN_ITEMS_MAX, |item| item.hidden_at);
    }
    prefs
}

/// Hides an artist, or shows them again; returns the settings as saved.
pub async fn set_artist(app: &AppHandle, name: &str, hidden: bool) -> Result<Settings, String> {
    if name.trim().is_empty() {
        return Err("No artist named".into());
    }
    let now = now_s();
    change(app, |prefs| with_artist(prefs, name, hidden, now)).await
}

/// Marks something Not interested, or takes the mark away.
pub async fn set_item(app: &AppHandle, item: HiddenItem, hidden: bool) -> Result<Settings, String> {
    let now = now_s();
    change(app, |prefs| with_item(prefs, item, hidden, now)).await
}

/// Shows every hidden artist, every hidden item, or both, again.
pub async fn clear(app: &AppHandle, artists: bool, items: bool) -> Result<Settings, String> {
    change(app, |mut prefs| {
        if artists {
            prefs.artists.clear();
        }
        if items {
            prefs.items.clear();
        }
        prefs
    })
    .await
}

async fn change(
    app: &AppHandle,
    edit: impl FnOnce(HiddenPrefs) -> HiddenPrefs,
) -> Result<Settings, String> {
    let _turn = CHANGES.lock().await;
    let db = app.state::<AppDb>().0.clone();
    let current = crate::settings::load(&db)
        .await
        .map_err(|error| error.to_string())?
        .hidden;
    let settings = crate::settings::update(&db, SettingChange::Hidden(edit(current)))
        .await
        .map_err(|error| error.to_string())?;
    // Other windows (the miniplayer) read settings again.
    if let Err(error) = SettingsChangedEvent("hidden".into()).emit(app) {
        tracing::warn!(%error, "couldn't announce hidden artists");
    }
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prefs(artists: &[&str], items: &[&str]) -> HiddenPrefs {
        HiddenPrefs {
            artists: artists
                .iter()
                .map(|name| HiddenArtist {
                    name: (*name).to_owned(),
                    hidden_at: 1,
                })
                .collect(),
            items: items
                .iter()
                .map(|key| HiddenItem {
                    kind: "song".into(),
                    key: (*key).to_owned(),
                    name: (*key).to_owned(),
                    ..HiddenItem::default()
                })
                .collect(),
        }
    }

    fn track(title: &str, artist: Option<&str>) -> OnlineTrack {
        OnlineTrack {
            external_id: None,
            title: title.into(),
            artist: artist.map(str::to_owned),
            album: None,
            duration_ms: None,
            artwork_url: None,
            source: None,
            explicit: false,
        }
    }

    #[test]
    fn credits_naming_a_hidden_artist_are_hidden() {
        let hidden = Hidden::new(&prefs(&["Daft  Punk", "Simon & Garfunkel"], &[]));
        assert!(hidden.hides_artist("daft punk"));
        assert!(hidden.hides_credit(Some("Daft Punk")));
        assert!(hidden.hides_credit(Some("Pharrell Williams, DAFT PUNK")));
        assert!(hidden.hides_credit(Some("The Weeknd feat. Daft Punk")));
        assert!(!hidden.hides_credit(Some("Daft Punkers")));
        // A duo is hidden as itself, also inside a longer credit, but its members aren't.
        assert!(hidden.hides_credit(Some("Simon & Garfunkel")));
        assert!(hidden.hides_credit(Some("Simon & Garfunkel feat. Someone")));
        assert!(!hidden.hides_credit(Some("Paul Simon")));
        assert!(!hidden.hides_credit(None));
        assert!(!Hidden::default().hides_credit(Some("Daft Punk")));
    }

    #[test]
    fn songs_marked_not_interested_are_hidden_by_their_key() {
        let key = song_key("Get Lucky (Radio Edit)", Some("Daft Punk"));
        let hidden = Hidden::new(&prefs(&[], &[&key]));
        assert!(hidden.hides_song("Get Lucky", Some("Daft Punk")));
        assert!(!hidden.hides_song("One More Time", Some("Daft Punk")));
        let mut tracks = vec![
            track("Get Lucky", Some("Daft Punk")),
            track("One More Time", Some("Daft Punk")),
        ];
        hidden.retain_tracks(&mut tracks);
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "One More Time");
    }

    #[test]
    fn your_music_tiles_are_hidden_by_artist_or_page() {
        let search: BTreeMap<String, String> = [
            ("provider".to_owned(), "x/y".to_owned()),
            ("id".to_owned(), "1".to_owned()),
        ]
        .into_iter()
        .collect();
        assert_eq!(
            page_key("/music/album", &search),
            "/music/album?id=1&provider=x/y"
        );
        let hidden = Hidden::new(&prefs(
            &["Nina Simone"],
            &["/music/album?id=1&provider=x/y"],
        ));
        let context =
            |kind: &str, name: &str, to: &str, search: BTreeMap<String, String>| PlayContext {
                kind: kind.into(),
                name: name.into(),
                subtitle: None,
                artwork_url: None,
                to: to.into(),
                search,
                played_at: 0.0,
            };
        let artist_page: BTreeMap<String, String> =
            [("name".to_owned(), "Nina Simone".to_owned())].into();
        assert!(hidden.hides_context(&context("artist", "Nina Simone", "/artist", artist_page)));
        assert!(hidden.hides_context(&context("album", "RAM", "/music/album", search.clone())));
        assert!(!hidden.hides_context(&context("album", "RAM", "/music/album", BTreeMap::new())));
    }

    #[test]
    fn only_known_kinds_with_keys_can_be_hidden() {
        assert!(prefs(&["Someone"], &["a|b"]).validate().is_ok());
        assert!(prefs(&[" "], &[]).validate().is_err());
        let mut odd = prefs(&[], &["a|b"]);
        odd.items[0].kind = "planet".into();
        assert!(odd.validate().is_err());
        let mut keyless = prefs(&[], &["a|b"]);
        keyless.items[0].key = String::new();
        assert!(keyless.validate().is_err());
    }

    #[test]
    fn hiding_again_lists_once_and_the_oldest_go_past_the_limit() {
        let once = with_artist(HiddenPrefs::default(), " Daft   Punk ", true, 5);
        let twice = with_artist(once, "daft punk", true, 6);
        assert_eq!(twice.artists.len(), 1);
        assert_eq!(twice.artists[0].name, "daft punk");
        assert_eq!(twice.artists[0].hidden_at, 6);
        let shown = with_artist(twice, "DAFT PUNK", false, 7);
        assert!(shown.artists.is_empty());

        let mut many = HiddenPrefs::default();
        for at in 0..(HIDDEN_ITEMS_MAX as u32 + 2) {
            many = with_item(
                many,
                HiddenItem {
                    kind: "song".into(),
                    key: format!("key {at}"),
                    name: "x".into(),
                    ..HiddenItem::default()
                },
                true,
                at,
            );
        }
        assert_eq!(many.items.len(), HIDDEN_ITEMS_MAX);
        assert!(!many.items.iter().any(|item| item.key == "key 0"));
        let item = many.items[0].clone();
        let fewer = with_item(many, item, false, 0);
        assert_eq!(fewer.items.len(), HIDDEN_ITEMS_MAX - 1);
    }
}
