//! The user's own details for songs, albums and artists (M46.30): sources get titles, names and
//! covers wrong, so the user can fix them. Edits sit over what sources said (Reset goes back) and
//! are applied in Rust wherever songs are listed or queued, so every screen shows them.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use raygun_db::detail_edits::{self as store, DetailEditRow};
use raygun_model::{DetailEdit, DetailTarget, OnlineTrack, QueueItemInput, SavedTrack};
use raygun_resolver::{normalize, track_key};
use tauri::{AppHandle, Manager};

use crate::startup::AppDb;

/// Covers the user picks are kept under the downloads folder, served by the proxy's `/local/`.
const COVERS: &str = "edits/covers";
/// How a stored cover names a file under the downloads folder.
const LOCAL: &str = "local:";

type Key = (String, String);

pub struct DetailService {
    app: AppHandle,
    /// The downloads folder, which the proxy's `/local/` serves.
    downloads: std::path::PathBuf,
    edits: RwLock<HashMap<Key, DetailEditRow>>,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

/// An album's key: its artist and title, normalized.
pub fn album_key(artist: Option<&str>, album: &str) -> String {
    format!(
        "{}|{}",
        normalize(artist.unwrap_or_default()),
        normalize(album)
    )
}

fn target_key(target: &DetailTarget) -> Key {
    match target {
        DetailTarget::Song { query } => ("song".into(), track_key(query)),
        DetailTarget::Album { artist, album } => {
            ("album".into(), album_key(artist.as_deref(), album))
        }
        DetailTarget::Artist { name } => ("artist".into(), normalize(name)),
        DetailTarget::Playlist { id } => ("playlist".into(), id.clone()),
    }
}

fn some(text: Option<String>) -> Option<String> {
    text.map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

impl DetailService {
    pub fn new(app: AppHandle, downloads: std::path::PathBuf) -> Arc<Self> {
        Arc::new(Self {
            app,
            downloads,
            edits: RwLock::new(HashMap::new()),
        })
    }

    fn db(&self) -> &raygun_db::Db {
        &self.app.state::<AppDb>().inner().0
    }

    /// Loads the stored edits; called once at start.
    pub async fn start(&self) {
        match store::all(self.db()).await {
            Ok(rows) => {
                let mut edits = self.edits.write().unwrap_or_else(|p| p.into_inner());
                for row in rows {
                    edits.insert((row.kind.clone(), row.key.clone()), row);
                }
            }
            Err(error) => tracing::warn!(%error, "couldn't load detail edits"),
        }
    }

    fn row(&self, kind: &str, key: &str) -> Option<DetailEditRow> {
        self.edits
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .get(&(kind.to_owned(), key.to_owned()))
            .cloned()
    }

    /// A stored cover as an address the webview can load.
    /// A cover the user picked, as it shows: a picked file through `/local/`, an address through
    /// the cover cache as a kept cover (M46.46: it stays cached whatever the size limit).
    fn shown_artwork(&self, artwork: &str) -> String {
        let images = self.app.state::<raygun_net::ImageLinks>();
        match artwork.strip_prefix(LOCAL) {
            Some(path) => images.local(path),
            None if artwork.starts_with("http") => images.kept_cover(artwork, &Default::default()),
            None => artwork.to_owned(),
        }
    }

    /// Every cover address the user picked (M46.46: the cache sweep keeps them).
    pub fn picked_cover_addresses(&self) -> Vec<String> {
        self.edits
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .values()
            .filter_map(|row| row.artwork.clone())
            .filter(|art| art.starts_with("http"))
            .collect()
    }

    /// The stored key for a target, also when it names an album or artist by its edited name
    /// (pages show edited names, and editing again must find the same edit).
    fn key_of(&self, target: &DetailTarget) -> Key {
        let (kind, key) = target_key(target);
        if self.row(&kind, &key).is_some() || kind == "playlist" {
            return (kind, key);
        }
        let edits = self.edits.read().unwrap_or_else(|p| p.into_inner());
        // A song shown with its edited title or artist: the edit whose result is this song.
        if kind == "song" {
            let found = edits.values().find(|row| {
                if row.kind != "song" {
                    return false;
                }
                let (artist, title) = row.key.split_once('|').unwrap_or(("", row.key.as_str()));
                track_key(&raygun_model::TrackQuery {
                    title: row.title.clone().unwrap_or_else(|| title.to_owned()),
                    artist: Some(row.artist.clone().unwrap_or_else(|| artist.to_owned())),
                    album: None,
                    duration_ms: None,
                }) == key
            });
            return (kind, found.map_or(key, |row| row.key.clone()));
        }
        let found = edits.values().find(|row| {
            row.kind == kind
                && row.title.as_deref().map(normalize).as_deref()
                    == Some(
                        match target {
                            DetailTarget::Album { album, .. } => normalize(album),
                            DetailTarget::Artist { name } => normalize(name),
                            DetailTarget::Song { .. } | DetailTarget::Playlist { .. } => {
                                String::new()
                            }
                        }
                        .as_str(),
                    )
                && match target {
                    DetailTarget::Album { artist, .. } => row.key.starts_with(&format!(
                        "{}|",
                        normalize(artist.as_deref().unwrap_or_default())
                    )),
                    _ => true,
                }
        });
        match found {
            Some(row) => (kind, row.key.clone()),
            None => (kind, key),
        }
    }

    /// The name sources know an artist by, when the user renamed them (M46.30).
    pub fn original_artist(&self, name: &str) -> String {
        let wanted = normalize(name);
        let edits = self.edits.read().unwrap_or_else(|p| p.into_inner());
        edits
            .values()
            .find(|row| {
                row.kind == "artist" && row.title.as_deref().map(normalize) == Some(wanted.clone())
            })
            .map_or_else(|| name.to_owned(), |row| row.key.clone())
    }

    pub fn get(&self, target: &DetailTarget) -> DetailEdit {
        let (kind, key) = self.key_of(target);
        let row = self.row(&kind, &key).unwrap_or_default();
        DetailEdit {
            artwork_url: row.artwork.as_deref().map(|art| self.shown_artwork(art)),
            title: row.title,
            artist: row.artist,
            album: row.album,
        }
    }

    /// Saves an edit; an empty one is a reset.
    pub async fn set(&self, target: &DetailTarget, edit: DetailEdit) -> Result<DetailEdit, String> {
        let (kind, key) = self.key_of(target);
        // A cover shown through the proxy is stored as the file it names.
        let images = self.app.state::<raygun_net::ImageLinks>();
        let local_prefix = images.local("");
        // Stored as the source's address, never the cache's link (M46.46).
        let artwork = some(edit.artwork_url).map(|art| match art.strip_prefix(&local_prefix) {
            Some(path) => format!("{LOCAL}{path}"),
            None => images.original(&art),
        });
        let row = DetailEditRow {
            kind: kind.clone(),
            key: key.clone(),
            title: some(edit.title),
            artist: if kind == "song" {
                some(edit.artist)
            } else {
                None
            },
            album: if kind == "song" {
                some(edit.album)
            } else {
                None
            },
            artwork,
        };
        store::put(self.db(), &row, now_ms())
            .await
            .map_err(|error| error.to_string())?;
        let before = {
            let mut edits = self.edits.write().unwrap_or_else(|p| p.into_inner());
            if row.is_empty() {
                edits.remove(&(kind.clone(), key.clone()))
            } else {
                edits.insert((kind.clone(), key.clone()), row.clone())
            }
        };
        self.retouch_queue(before.unwrap_or_default(), row);
        Ok(self.get(target))
    }

    /// Songs already queued (the one playing too) take an edit at once: the player, its side
    /// panel and the media notification follow the queue.
    fn retouch_queue(&self, before: DetailEditRow, after: DetailEditRow) {
        let same = before.title == after.title
            && before.artist == after.artist
            && before.album == after.album
            && before.artwork == after.artwork;
        if after.kind == "playlist" || same {
            return;
        }
        let Some(playback) = self.app.try_state::<crate::playback::PlaybackService>() else {
            return;
        };
        let change = Change {
            cover: after.artwork.as_deref().map(|art| self.shown_artwork(art)),
            old_cover: before.artwork.as_deref().map(|art| self.shown_artwork(art)),
            before,
            after,
        };
        playback.update_items(|item| {
            retouch(item, &change, |item| {
                let (_, key) = self.key_of(&DetailTarget::Song {
                    query: raygun_model::TrackQuery {
                        title: item.title.clone(),
                        artist: item.artist.clone(),
                        album: None,
                        duration_ms: None,
                    },
                });
                self.row("song", &key)
                    .is_some_and(|edit| edit.artwork.is_some())
            })
        });
    }

    /// Keeps a picked image as a cover; returns its address for an edit.
    pub async fn import_cover(&self, upload: std::path::PathBuf) -> Result<String, String> {
        let downloads = self.downloads.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let bytes = std::fs::read(&upload).map_err(|error| error.to_string())?;
            let _ = std::fs::remove_file(&upload);
            let format =
                image::guess_format(&bytes).map_err(|_| "That isn't an image".to_owned())?;
            let extension = format.extensions_str().first().copied().unwrap_or("img");
            let name = format!(
                "{}.{extension}",
                raygun_ext_host::package::sha256_hex(&bytes)
            );
            let dir = downloads.join(COVERS);
            std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
            std::fs::write(dir.join(&name), &bytes).map_err(|error| error.to_string())?;
            Ok::<_, String>(name)
        })
        .await
        .map_err(|error| error.to_string())?
        .map(|name| {
            self.app
                .state::<raygun_net::ImageLinks>()
                .local(&format!("{COVERS}/{name}"))
        })
    }

    /// An artist credit with the user's names: the whole credit, else each of its artists.
    fn artist_credit(&self, credit: &str) -> Option<String> {
        if let Some(name) = self
            .row("artist", &normalize(credit))
            .and_then(|row| row.title)
        {
            return Some(name);
        }
        let parts: Vec<&str> = credit.split(", ").collect();
        let mut changed = false;
        let renamed: Vec<String> = parts
            .iter()
            .map(|part| {
                match self
                    .row("artist", &normalize(part))
                    .and_then(|row| row.title)
                {
                    Some(name) => {
                        changed = true;
                        name
                    }
                    None => (*part).to_owned(),
                }
            })
            .collect();
        changed.then(|| renamed.join(", "))
    }

    /// The user's details over a song's: its own edit, then its album's, then its artists'.
    pub fn apply(
        &self,
        key: &str,
        title: &mut String,
        artist: &mut Option<String>,
        album: &mut Option<String>,
        artwork: &mut Option<String>,
    ) {
        let original_artist = artist.clone();
        let original_album = album.clone();
        let mut own_art = false;
        if let Some(edit) = self.row("song", key) {
            if let Some(value) = edit.title {
                *title = value;
            }
            if edit.artist.is_some() {
                *artist = edit.artist;
            }
            if edit.album.is_some() {
                *album = edit.album;
            }
            if let Some(value) = edit.artwork {
                *artwork = Some(self.shown_artwork(&value));
                own_art = true;
            }
        }
        if let Some(name) = original_album.as_deref()
            && let Some(edit) = self.row("album", &album_key(original_artist.as_deref(), name))
        {
            if let Some(value) = edit.title {
                *album = Some(value);
            }
            if !own_art && let Some(value) = edit.artwork {
                *artwork = Some(self.shown_artwork(&value));
            }
        }
        if let Some(credit) = artist.clone()
            && let Some(renamed) = self.artist_credit(&credit)
        {
            *artist = Some(renamed);
        }
    }

    fn is_empty(&self) -> bool {
        self.edits
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .is_empty()
    }

    /// The title and artist a song stored under `key` shows with the user's details, when they
    /// differ from its own (M46.48: saved songs are found by the names screens show).
    pub fn shown_names(
        &self,
        key: &str,
        title: &str,
        artist: Option<&str>,
    ) -> Option<(String, Option<String>)> {
        if self.is_empty() {
            return None;
        }
        let (mut shown_title, mut shown_artist) = (title.to_owned(), artist.map(str::to_owned));
        let (mut album, mut artwork) = (None, None);
        self.apply(
            key,
            &mut shown_title,
            &mut shown_artist,
            &mut album,
            &mut artwork,
        );
        (shown_title != title || shown_artist.as_deref() != artist)
            .then_some((shown_title, shown_artist))
    }

    /// The key of the song the user gave these names to (Edit details, Find a source), when
    /// they're an edit's names rather than a song's own (M46.48).
    pub fn original_song_key(&self, title: &str, artist: Option<&str>) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        let query = raygun_model::TrackQuery {
            title: title.to_owned(),
            artist: artist.map(str::to_owned),
            album: None,
            duration_ms: None,
        };
        let shown = track_key(&query);
        let (_, key) = self.key_of(&DetailTarget::Song { query });
        (key != shown).then_some(key)
    }

    pub fn apply_saved(&self, track: &mut SavedTrack) {
        if self.is_empty() {
            return;
        }
        let key = track.track_key.clone();
        self.apply(
            &key,
            &mut track.title,
            &mut track.artist,
            &mut track.album,
            &mut track.artwork_url,
        );
    }

    pub fn apply_online(&self, tracks: &mut [OnlineTrack]) {
        if self.is_empty() {
            return;
        }
        for track in tracks {
            let key = track_key(&raygun_model::TrackQuery {
                title: track.title.clone(),
                artist: track.artist.clone(),
                album: track.album.clone(),
                duration_ms: None,
            });
            self.apply(
                &key,
                &mut track.title,
                &mut track.artist,
                &mut track.album,
                &mut track.artwork_url,
            );
        }
    }

    pub fn apply_queue(&self, items: &mut [QueueItemInput]) {
        if self.is_empty() {
            return;
        }
        for item in items {
            let key = track_key(&raygun_model::TrackQuery {
                title: item.title.clone(),
                artist: item.artist.clone(),
                album: item.album.clone(),
                duration_ms: None,
            });
            self.apply(
                &key,
                &mut item.title,
                &mut item.artist,
                &mut item.album,
                &mut item.artwork_url,
            );
        }
    }

    /// An album's own title and cover with the user's edit.
    pub fn apply_album(
        &self,
        artist: Option<&str>,
        title: &mut String,
        artwork: &mut Option<String>,
    ) {
        if let Some(edit) = self.row("album", &album_key(artist, title)) {
            if let Some(value) = edit.artwork {
                *artwork = Some(self.shown_artwork(&value));
            }
            if let Some(value) = edit.title {
                *title = value;
            }
        }
    }
}

/// An edit as songs already queued take it: the edit before and after, with its covers as shown.
struct Change {
    before: DetailEditRow,
    after: DetailEditRow,
    cover: Option<String>,
    old_cover: Option<String>,
}

/// The names a lookup plays a song by, when they're the song's own (`key`): what a reset goes
/// back to, since queued songs keep only the names they show.
fn own_names(item: &raygun_model::QueueItem, key: &str) -> Option<raygun_model::TrackQuery> {
    match &item.source {
        raygun_model::MediaSource::Lookup { .. } => {
            let query = raygun_model::TrackQuery::for_item(item);
            (track_key(&query) == key).then_some(query)
        }
        _ => None,
    }
}

fn assign(field: &mut Option<String>, value: Option<String>) -> bool {
    if *field == value {
        return false;
    }
    *field = value;
    true
}

/// A field after an edit: the edit's value, else (an edit taken back) `original` when it's known.
fn edited(
    field: &mut Option<String>,
    after: &Option<String>,
    before: &Option<String>,
    original: Option<String>,
) -> bool {
    match (after, before, original) {
        (Some(value), _, _) => assign(field, Some(value.clone())),
        (None, Some(_), Some(original)) => assign(field, Some(original)),
        _ => false,
    }
}

/// Applies `change` to a queued song it's about; says whether the song changed. `own_cover`
/// says whether a song has a cover of its own from the user (an album's cover doesn't replace
/// it).
fn retouch(
    item: &mut raygun_model::QueueItem,
    change: &Change,
    own_cover: impl Fn(&raygun_model::QueueItem) -> bool,
) -> bool {
    let Change {
        before,
        after,
        cover,
        old_cover,
    } = change;
    let key = after.key.as_str();
    let cover_changed = |item: &mut raygun_model::QueueItem| match cover {
        Some(cover) => assign(&mut item.artwork_url, Some(cover.clone())),
        // A cover taken back: the picked one goes (the player then looks for another).
        None if old_cover.is_some() && item.artwork_url == *old_cover => {
            assign(&mut item.artwork_url, None)
        }
        None => false,
    };
    match after.kind.as_str() {
        "song" => {
            let shown = track_key(&raygun_model::TrackQuery {
                title: item.title.clone(),
                artist: item.artist.clone(),
                album: None,
                duration_ms: None,
            });
            // The names it showed before this edit, from the key's own and the edit's.
            let (key_artist, key_title) = key.split_once('|').unwrap_or(("", key));
            let shown_before = track_key(&raygun_model::TrackQuery {
                title: before.title.clone().unwrap_or_else(|| key_title.to_owned()),
                artist: Some(
                    before
                        .artist
                        .clone()
                        .unwrap_or_else(|| key_artist.to_owned()),
                ),
                album: None,
                duration_ms: None,
            });
            let original = own_names(item, key);
            if shown != key && shown != shown_before && original.is_none() {
                return false;
            }
            let mut changed = false;
            let mut title = Some(item.title.clone());
            changed |= edited(
                &mut title,
                &after.title,
                &before.title,
                original.as_ref().map(|query| query.title.clone()),
            );
            if let Some(title) = title {
                item.title = title;
            }
            changed |= edited(
                &mut item.artist,
                &after.artist,
                &before.artist,
                original.as_ref().and_then(|query| query.artist.clone()),
            );
            changed |= edited(
                &mut item.album,
                &after.album,
                &before.album,
                original.as_ref().and_then(|query| query.album.clone()),
            );
            changed | cover_changed(item)
        }
        "album" => {
            let (key_artist, _) = key.split_once('|').unwrap_or(("", key));
            let Some(album) = item.album.clone() else {
                return false;
            };
            let named = album_key(item.artist.as_deref(), &album) == key
                || (normalize(item.artist.as_deref().unwrap_or_default()) == key_artist
                    && before
                        .title
                        .as_deref()
                        .is_some_and(|title| normalize(title) == normalize(&album)));
            if !named {
                return false;
            }
            let original = own_names(item, &track_key(&raygun_model::TrackQuery::for_item(item)))
                .and_then(|query| query.album);
            let mut changed = edited(&mut item.album, &after.title, &before.title, original);
            if !own_cover(item) {
                changed |= cover_changed(item);
            }
            changed
        }
        "artist" => {
            let Some(artist) = item.artist.clone() else {
                return false;
            };
            let named = normalize(&artist) == key
                || before
                    .title
                    .as_deref()
                    .is_some_and(|title| normalize(title) == normalize(&artist));
            if !named {
                return false;
            }
            let original = match &item.source {
                raygun_model::MediaSource::Lookup {
                    artist: Some(name), ..
                } if normalize(name) == key => Some(name.clone()),
                _ => None,
            };
            edited(&mut item.artist, &after.title, &before.title, original)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queued(
        title: &str,
        artist: &str,
        album: Option<&str>,
        artwork: Option<&str>,
    ) -> raygun_model::QueueItem {
        raygun_model::QueueItem {
            id: "1".into(),
            title: title.into(),
            artist: Some(artist.into()),
            album: album.map(String::from),
            duration_ms: None,
            artwork_url: artwork.map(String::from),
            media_kind: raygun_model::MediaKind::Audio,
            source: raygun_model::MediaSource::Lookup {
                title: title.into(),
                artist: Some(artist.into()),
                album: album.map(String::from),
                duration_ms: None,
            },
            explicit: false,
        }
    }

    fn edit(kind: &str, key: &str) -> DetailEditRow {
        DetailEditRow {
            kind: kind.into(),
            key: key.into(),
            ..Default::default()
        }
    }

    fn change(before: DetailEditRow, after: DetailEditRow) -> Change {
        Change {
            cover: after.artwork.clone().map(|art| format!("/cover/{art}")),
            old_cover: before.artwork.clone().map(|art| format!("/cover/{art}")),
            before,
            after,
        }
    }

    #[test]
    fn the_playing_song_takes_a_cover_picked_for_it_at_once() {
        // No cover of its own until the user picked one from another source.
        let mut item = queued("Sinnerman", "Nina Simone", None, None);
        let key = "nina simone|sinnerman";
        let picked = DetailEditRow {
            artwork: Some("picked.jpg".into()),
            ..edit("song", key)
        };
        assert!(retouch(
            &mut item,
            &change(edit("song", key), picked.clone()),
            |_| false
        ));
        assert_eq!(item.artwork_url.as_deref(), Some("/cover/picked.jpg"));

        // Renamed after: found by the names it shows now, and named again.
        let renamed = DetailEditRow {
            title: Some("Sinnerman (Live)".into()),
            ..picked.clone()
        };
        assert!(retouch(
            &mut item,
            &change(picked.clone(), renamed.clone()),
            |_| false
        ));
        assert_eq!(item.title, "Sinnerman (Live)");
        let retitled = DetailEditRow {
            title: Some("Sinner Man".into()),
            ..renamed.clone()
        };
        assert!(retouch(
            &mut item,
            &change(renamed, retitled.clone()),
            |_| false
        ));
        assert_eq!(item.title, "Sinner Man");

        // Reset: its own names come back from the lookup; the picked cover goes.
        assert!(retouch(
            &mut item,
            &change(retitled, edit("song", key)),
            |_| false
        ));
        assert_eq!(item.title, "Sinnerman");
        assert_eq!(item.artwork_url, None);

        // Another song is left alone.
        let mut other = queued("Feeling Good", "Nina Simone", None, Some("own.jpg"));
        assert!(!retouch(
            &mut other,
            &change(edit("song", key), picked),
            |_| false
        ));
        assert_eq!(other.artwork_url.as_deref(), Some("own.jpg"));
    }

    #[test]
    fn album_and_artist_edits_reach_queued_songs() {
        let mut item = queued(
            "Sinnerman",
            "Nina Simone",
            Some("Pastel Blues"),
            Some("a.jpg"),
        );
        let key = album_key(Some("Nina Simone"), "Pastel Blues");
        let album = DetailEditRow {
            title: Some("Pastel Blues (Remastered)".into()),
            artwork: Some("album.jpg".into()),
            ..edit("album", &key)
        };
        assert!(retouch(
            &mut item,
            &change(edit("album", &key), album.clone()),
            |_| false
        ));
        assert_eq!(item.album.as_deref(), Some("Pastel Blues (Remastered)"));
        assert_eq!(item.artwork_url.as_deref(), Some("/cover/album.jpg"));
        // A song with a cover of its own keeps it.
        let mut own = queued(
            "Be My Husband",
            "Nina Simone",
            Some("Pastel Blues"),
            Some("own.jpg"),
        );
        retouch(&mut own, &change(edit("album", &key), album), |_| true);
        assert_eq!(own.artwork_url.as_deref(), Some("own.jpg"));

        let renamed = DetailEditRow {
            title: Some("Dr. Nina Simone".into()),
            ..edit("artist", "nina simone")
        };
        assert!(retouch(
            &mut item,
            &change(edit("artist", "nina simone"), renamed.clone()),
            |_| false
        ));
        assert_eq!(item.artist.as_deref(), Some("Dr. Nina Simone"));
        // Taken back: the name the lookup plays it by.
        assert!(retouch(
            &mut item,
            &change(renamed, edit("artist", "nina simone")),
            |_| false
        ));
        assert_eq!(item.artist.as_deref(), Some("Nina Simone"));
    }

    #[test]
    fn track_keys_are_stable_when_normalized_again() {
        let key = track_key(&raygun_model::TrackQuery {
            title: "Get Lucky (feat. Pharrell)".into(),
            artist: Some("Daft Punk".into()),
            album: None,
            duration_ms: None,
        });
        let (artist, title) = key.split_once('|').unwrap();
        let again = track_key(&raygun_model::TrackQuery {
            title: title.into(),
            artist: Some(artist.into()),
            album: None,
            duration_ms: None,
        });
        assert_eq!(again, key);
    }

    #[test]
    fn album_keys_ignore_case_and_marks() {
        assert_eq!(
            album_key(Some("Daft Punk"), "Discovery"),
            album_key(Some("daft punk"), "DISCOVERY")
        );
    }
}
