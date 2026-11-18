//! Keeping a Subsonic or Navidrome server's library in step with Raygun's, as Feishin and Navic
//! do with theirs: stars and Favorites, playlists both ways, plays counted on the server
//! (`scrobble`) and the play queue kept there for other apps to go on from. Each is switched on
//! per connection ([`ServerSyncPrefs`], device-local in `local_meta`); counting plays is on unless
//! turned off, the rest off unless turned on.
//!
//! Songs are matched by their names (`collections::identity::song_key`), as saved songs are:
//! a song from any source is starred on the server when the server has it. Each sync compares
//! both sides with what both held after the last one ([`SyncMemory`]), so a song taken out on
//! one side is taken out on the other, and one added on either side is added to the other.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use raygun_ext_host::servers::subsonic::{Library, Song};
use raygun_ext_host::servers::{self, ServerConfig};
use raygun_model::{
    MediaKind, MediaSource, PlaylistSummary, QueueItem, QueueItemInput, SavedTrack,
    ServerSyncPrefs, ServerSyncReport, ServerSyncStatus, TrackInput,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::collections::CollectionsService;
use crate::collections::identity::{any_same, keys_of, song_key};
use crate::servers::ServerService;
use crate::startup::AppDb;

/// `local_meta` keys: a connection's settings and what its last sync left.
const PREFS: &str = "servers.sync.prefs.";
const MEMORY: &str = "servers.sync.memory.";
/// How often lists are synced while nothing changes.
const SYNC_EVERY: Duration = Duration::from_secs(15 * 60);
/// The first sync after start, and how long a change in Raygun's lists waits for more.
const FIRST_SYNC: Duration = Duration::from_secs(90);
const SETTLE: Duration = Duration::from_secs(20);
/// How long a changed queue waits before it's kept on the server.
const QUEUE_SETTLE: Duration = Duration::from_secs(15);
/// Songs of a queue kept on the server at most.
const QUEUE_MOST: usize = 300;
/// A song the server didn't have is looked for again after this long.
const MISSING_FOR_MS: i64 = 7 * 24 * 60 * 60 * 1000;
/// Songs looked up by name stay known this long.
const KNOWN_FOR: Duration = Duration::from_secs(60 * 60);

/// What a connection's last sync left: what both sides held, so the next one tells additions
/// from removals.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct SyncMemory {
    /// Favorites' songs (keys) that were starred on the server too.
    favorites: Vec<String>,
    playlists: Vec<PlaylistPair>,
    /// Songs the server didn't have, by key, and when that was found (Unix ms).
    missing: HashMap<String, i64>,
    /// The server's names for Raygun's songs where they differ ("Song (Remastered)" by
    /// "Artist feat. Other"): server key to Raygun's key, so both count as one song.
    aliases: HashMap<String, String>,
    synced_at: Option<f64>,
    report: Option<ServerSyncReport>,
    error: Option<String>,
}

/// A Raygun playlist and the server's that are kept in step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlaylistPair {
    local: String,
    remote: String,
    /// Its name and songs (keys, in order) after the last sync.
    name: String,
    songs: Vec<String>,
    /// The server's playlist can't be changed by this user (another's, a smart playlist):
    /// Raygun's follows it.
    #[serde(default)]
    follows: bool,
}

/// What a three-way comparison of two lists of songs says to do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Plan {
    /// Songs to add to Raygun's list, and to take out of it.
    add_local: Vec<String>,
    remove_local: Vec<String>,
    /// Songs to add to the server's, and to take out of it.
    add_remote: Vec<String>,
    remove_remote: Vec<String>,
    /// The list both should hold, in order.
    result: Vec<String>,
}

/// Compares Raygun's list and the server's with what both held after the last sync (`last`):
/// what only one side added goes to the other, what one side took out leaves the other. The
/// result keeps Raygun's order, with the server's additions after; when only the server's list
/// changed, the server's order.
fn plan(local: &[String], remote: &[String], last: &[String]) -> Plan {
    let local_set: HashSet<&String> = local.iter().collect();
    let remote_set: HashSet<&String> = remote.iter().collect();
    let last_set: HashSet<&String> = last.iter().collect();
    let add_local: Vec<String> = remote
        .iter()
        .filter(|key| !local_set.contains(key) && !last_set.contains(key))
        .cloned()
        .collect();
    let remove_remote: Vec<String> = remote
        .iter()
        .filter(|key| last_set.contains(key) && !local_set.contains(key))
        .cloned()
        .collect();
    let add_remote: Vec<String> = local
        .iter()
        .filter(|key| !remote_set.contains(key) && !last_set.contains(key))
        .cloned()
        .collect();
    let remove_local: Vec<String> = local
        .iter()
        .filter(|key| last_set.contains(key) && !remote_set.contains(key))
        .cloned()
        .collect();
    let removed: HashSet<&String> = remove_local.iter().chain(&remove_remote).collect();
    let result = if local == last && remote != last {
        remote
            .iter()
            .filter(|key| !removed.contains(key))
            .cloned()
            .collect()
    } else {
        let mut result: Vec<String> = local
            .iter()
            .filter(|key| !removed.contains(key))
            .cloned()
            .collect();
        result.extend(add_local.iter().cloned());
        result
    };
    let mut seen = HashSet::new();
    let result = result
        .into_iter()
        .filter(|key| seen.insert(key.clone()))
        .collect();
    Plan {
        add_local,
        remove_local,
        add_remote,
        remove_remote,
        result,
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

fn key_of_song(song: &Song) -> String {
    song_key(&song.title, song.artist.as_deref())
}

/// A server song's key, as Raygun's song it was matched with when the names differ.
fn aliased(aliases: &HashMap<String, String>, song: &Song) -> String {
    let key = key_of_song(song);
    aliases.get(&key).cloned().unwrap_or(key)
}

fn key_of_track(track: &SavedTrack) -> String {
    song_key(&track.title, track.artist.as_deref())
}

/// A song of the server, to save in Raygun: by its names, found again by the resolver (the
/// server's own stream provider among the sources).
fn track_of(song: &Song) -> TrackInput {
    TrackInput {
        title: song.title.clone(),
        artist: song.artist.clone(),
        album: song.album.clone(),
        duration_ms: song.duration_ms,
        artwork_url: song.artwork_url.clone(),
        source: None,
        explicit: false,
    }
}

fn input_of(track: &SavedTrack) -> TrackInput {
    TrackInput {
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: track.album.clone(),
        duration_ms: track.duration_ms,
        artwork_url: track.artwork_url.clone(),
        source: track.source.clone(),
        explicit: track.explicit,
    }
}

/// Keys in order, each once, with the first song or track each stands for.
fn keyed<T>(items: Vec<T>, key: impl Fn(&T) -> String) -> (Vec<String>, HashMap<String, T>) {
    let mut order = Vec::new();
    let mut by_key = HashMap::new();
    for item in items {
        if let std::collections::hash_map::Entry::Vacant(place) = by_key.entry(key(&item)) {
            order.push(place.key().clone());
            place.insert(item);
        }
    }
    (order, by_key)
}

/// The song among `found` that is `title` by `artist`.
fn same_song<'a>(found: &'a [Song], title: &str, artist: Option<&str>) -> Option<&'a Song> {
    let wanted = song_key(title, artist);
    found
        .iter()
        .find(|song| key_of_song(song) == wanted)
        .or_else(|| {
            let keys = keys_of(title, artist);
            found
                .iter()
                .find(|song| any_same(&keys_of(&song.title, song.artist.as_deref()), &keys))
        })
}

/// Songs found by name, per connection and key: their id and key on the server (`None` for
/// none), and when.
type Known = HashMap<(String, String), (Option<(String, String)>, Instant)>;

/// A connection that keeps a library, signed in.
struct Connection {
    id: String,
    config: ServerConfig,
    library: Library,
    prefs: ServerSyncPrefs,
}

pub struct ServerSync {
    app: AppHandle,
    /// Connections syncing now.
    running: Mutex<HashSet<String>>,
    /// A change in Raygun's lists, to sync soon.
    wake: tokio::sync::Notify,
    /// Set while a sync changes Raygun's lists itself: those changes wake nothing.
    quiet: AtomicBool,
    /// Song ids found by name, per connection and key; `None` for none.
    known: Mutex<Known>,
    /// The queue waiting to be kept on servers, and whether a task is waiting with it.
    queue: Mutex<(Option<String>, bool)>,
}

impl ServerSync {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            running: Mutex::new(HashSet::new()),
            wake: tokio::sync::Notify::new(),
            quiet: AtomicBool::new(false),
            known: Mutex::new(HashMap::new()),
            queue: Mutex::new((None, false)),
        }
    }

    fn db(&self) -> std::sync::Arc<raygun_db::Db> {
        self.app.state::<AppDb>().inner().0.clone()
    }

    /// Syncs every connection's lists a while after start, every [`SYNC_EVERY`], and soon after
    /// Raygun's lists change.
    pub fn start(&self) {
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(FIRST_SYNC).await;
            loop {
                let sync = app.state::<ServerSync>();
                sync.sync_all().await;
                tokio::select! {
                    () = crate::device_state::rest(SYNC_EVERY) => {}
                    () = sync.wake.notified() => tokio::time::sleep(SETTLE).await,
                }
            }
        });
    }

    /// Raygun's Favorites or playlists changed: synced soon.
    pub fn lists_changed(&self) {
        if !self.quiet.load(Ordering::SeqCst) {
            self.wake.notify_one();
        }
    }

    async fn prefs(&self, id: &str) -> ServerSyncPrefs {
        raygun_db::local_value(&self.db(), &format!("{PREFS}{id}"))
            .await
            .ok()
            .flatten()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    async fn memory(&self, id: &str) -> SyncMemory {
        raygun_db::local_value(&self.db(), &format!("{MEMORY}{id}"))
            .await
            .ok()
            .flatten()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    async fn keep_memory(&self, id: &str, memory: &SyncMemory) {
        let json = serde_json::to_string(memory).unwrap_or_default();
        if let Err(error) =
            raygun_db::set_local_value(&self.db(), &format!("{MEMORY}{id}"), &json).await
        {
            tracing::warn!(%error, "couldn't keep what a server sync did");
        }
    }

    /// The Subsonic and Navidrome connections that are on, signed in, with their settings.
    async fn connections(&self) -> Vec<Connection> {
        let services = self.app.state::<ServerService>();
        let Ok(configs) = services.configs().await else {
            return Vec::new();
        };
        let disabled = crate::settings::load(&self.db())
            .await
            .map(|settings| settings.disabled_sources)
            .unwrap_or_default();
        let mut found = Vec::new();
        for (id, config) in configs {
            if !config.kind.is_subsonic() || disabled.contains(&id) {
                continue;
            }
            let Ok((config, secret)) = services.signed_in(&id).await else {
                continue;
            };
            let Ok(library) = Library::open(&config, &secret) else {
                continue;
            };
            let prefs = self.prefs(&id).await;
            found.push(Connection {
                id,
                config,
                library,
                prefs,
            });
        }
        found
    }

    async fn connection(&self, id: &str) -> Result<Connection, String> {
        let services = self.app.state::<ServerService>();
        let (config, secret) = services.signed_in(id).await?;
        let library = Library::open(&config, &secret)?;
        Ok(Connection {
            id: id.to_owned(),
            config,
            library,
            prefs: self.prefs(id).await,
        })
    }

    /// A connection's settings and how its last sync went.
    pub async fn status(&self, id: &str) -> ServerSyncStatus {
        let memory = self.memory(id).await;
        ServerSyncStatus {
            server_id: id.to_owned(),
            prefs: self.prefs(id).await,
            synced_at: memory.synced_at,
            report: memory.report,
            error: memory.error,
            running: self
                .running
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .contains(id),
        }
    }

    /// Changes what a connection keeps in step; lists just switched on sync at once.
    pub async fn set_prefs(
        &self,
        id: &str,
        prefs: ServerSyncPrefs,
    ) -> Result<ServerSyncStatus, String> {
        let before = self.prefs(id).await;
        let json = serde_json::to_string(&prefs).map_err(|error| error.to_string())?;
        raygun_db::set_local_value(&self.db(), &format!("{PREFS}{id}"), &json)
            .await
            .map_err(|error| error.to_string())?;
        if (prefs.favorites && !before.favorites) || (prefs.playlists && !before.playlists) {
            return self.sync_now(id).await;
        }
        Ok(self.status(id).await)
    }

    /// Forgets a connection's settings and sync (it was removed).
    pub async fn forget(&self, id: &str) {
        let db = self.db();
        for key in [format!("{PREFS}{id}"), format!("{MEMORY}{id}")] {
            let _ = raygun_db::set_local_value(&db, &key, "").await;
        }
    }

    async fn sync_all(&self) {
        for connection in self.connections().await {
            if connection.prefs.favorites || connection.prefs.playlists {
                self.run(&connection).await;
            }
        }
    }

    /// Syncs a connection's lists now.
    pub async fn sync_now(&self, id: &str) -> Result<ServerSyncStatus, String> {
        let connection = self.connection(id).await?;
        self.run(&connection).await;
        Ok(self.status(id).await)
    }

    async fn run(&self, connection: &Connection) {
        if !self
            .running
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .insert(connection.id.clone())
        {
            return;
        }
        let mut memory = self.memory(&connection.id).await;
        let mut report = ServerSyncReport::default();
        self.quiet.store(true, Ordering::SeqCst);
        let mut outcome = Ok(());
        if connection.prefs.favorites {
            outcome = self
                .sync_favorites(connection, &mut memory, &mut report)
                .await;
        }
        if outcome.is_ok() && connection.prefs.playlists {
            outcome = self
                .sync_playlists(connection, &mut memory, &mut report)
                .await;
        }
        self.quiet.store(false, Ordering::SeqCst);
        match outcome {
            Ok(()) => {
                memory.synced_at = Some(now_ms() as f64);
                memory.error = None;
                tracing::info!(server = %connection.id, ?report, "server synced");
            }
            Err(error) => {
                tracing::warn!(server = %connection.id, %error, "server sync stopped");
                memory.error = Some(error);
            }
        }
        memory.report = Some(report);
        self.keep_memory(&connection.id, &memory).await;
        self.running
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .remove(&connection.id);
    }

    /// The server's id and key for a song Raygun has, found by its names (remembered a while);
    /// `None` when the server doesn't have it.
    async fn find(
        &self,
        connection: &Connection,
        title: &str,
        artist: Option<&str>,
    ) -> Result<Option<(String, String)>, String> {
        let key = (connection.id.clone(), song_key(title, artist));
        if let Some((id, at)) = self
            .known
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .get(&key)
            && at.elapsed() < KNOWN_FOR
        {
            return Ok(id.clone());
        }
        let mut found = None;
        if let Some(artist) = artist {
            let songs = connection
                .library
                .search(&format!("{title} {artist}"), 10)
                .await?;
            found = same_song(&songs, title, Some(artist))
                .map(|song| (song.id.clone(), key_of_song(song)));
        }
        if found.is_none() {
            let songs = connection.library.search(title, 20).await?;
            found =
                same_song(&songs, title, artist).map(|song| (song.id.clone(), key_of_song(song)));
        }
        self.known
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .insert(key, (found.clone(), Instant::now()));
        Ok(found)
    }

    /// Server ids for Raygun's songs `keys` (from `tracks`), skipping songs the server lately
    /// didn't have; those it doesn't have are noted in `memory.missing`.
    async fn ids_for(
        &self,
        connection: &Connection,
        keys: &[String],
        tracks: &HashMap<String, SavedTrack>,
        memory: &mut SyncMemory,
        report: &mut ServerSyncReport,
    ) -> Result<HashMap<String, String>, String> {
        let now = now_ms();
        let mut ids = HashMap::new();
        for key in keys {
            if memory
                .missing
                .get(key)
                .is_some_and(|at| now - at < MISSING_FOR_MS)
            {
                report.not_on_server += 1;
                continue;
            }
            let Some(track) = tracks.get(key) else {
                continue;
            };
            match self
                .find(connection, &track.title, track.artist.as_deref())
                .await?
            {
                Some((id, server_key)) => {
                    memory.missing.remove(key);
                    if &server_key != key {
                        memory.aliases.insert(server_key, key.clone());
                    }
                    ids.insert(key.clone(), id);
                }
                None => {
                    memory.missing.insert(key.clone(), now);
                    report.not_on_server += 1;
                }
            }
        }
        Ok(ids)
    }

    async fn sync_favorites(
        &self,
        connection: &Connection,
        memory: &mut SyncMemory,
        report: &mut ServerSyncReport,
    ) -> Result<(), String> {
        let collections = self.app.state::<CollectionsService>();
        let starred = connection.library.starred().await?;
        let (remote, remote_songs) = keyed(starred, |song| aliased(&memory.aliases, song));
        let (local, local_tracks) = keyed(collections.favorites().await?, key_of_track);
        let plan = plan(&local, &remote, &memory.favorites);

        let adding: Vec<TrackInput> = plan
            .add_local
            .iter()
            .filter_map(|key| remote_songs.get(key).map(track_of))
            .collect();
        report.favorited += adding.len() as u32;
        if !adding.is_empty() {
            collections.set_favorites(adding, true).await?;
        }
        let removing: Vec<TrackInput> = plan
            .remove_local
            .iter()
            .filter_map(|key| local_tracks.get(key).map(input_of))
            .collect();
        report.unfavorited += removing.len() as u32;
        if !removing.is_empty() {
            collections.set_favorites(removing, false).await?;
        }
        let unstar: Vec<String> = plan
            .remove_remote
            .iter()
            .filter_map(|key| remote_songs.get(key).map(|song| song.id.clone()))
            .collect();
        connection.library.star(&unstar, false).await?;
        report.unstarred += unstar.len() as u32;
        let found = self
            .ids_for(connection, &plan.add_remote, &local_tracks, memory, report)
            .await?;
        let star: Vec<String> = plan
            .add_remote
            .iter()
            .filter_map(|key| found.get(key).cloned())
            .collect();
        connection.library.star(&star, true).await?;
        report.starred += star.len() as u32;
        // What both hold now: songs the server doesn't have stay Raygun's own.
        memory.favorites = plan
            .result
            .into_iter()
            .filter(|key| remote_songs.contains_key(key) || found.contains_key(key))
            .collect();
        Ok(())
    }

    async fn sync_playlists(
        &self,
        connection: &Connection,
        memory: &mut SyncMemory,
        report: &mut ServerSyncReport,
    ) -> Result<(), String> {
        let collections = self.app.state::<CollectionsService>();
        let remote_lists = connection.library.playlists().await?;
        let local_lists = collections.playlists().await?;
        let mut pairs: Vec<PlaylistPair> = Vec::new();
        let mut paired_local: HashSet<String> = HashSet::new();
        let mut paired_remote: HashSet<String> = HashSet::new();
        for pair in std::mem::take(&mut memory.playlists) {
            let local = local_lists.iter().find(|list| list.id == pair.local);
            let remote = remote_lists.iter().find(|list| list.id == pair.remote);
            match (local, remote) {
                (Some(_), Some(_)) => {
                    paired_local.insert(pair.local.clone());
                    paired_remote.insert(pair.remote.clone());
                    pairs.push(pair);
                }
                // Deleted on one side: deleted on the other unless it changed since.
                (Some(local), None) => {
                    let songs = self.local_keys(&local.id).await?;
                    if songs == pair.songs || pair.follows {
                        collections.delete_playlist(&local.id).await?;
                        paired_local.insert(local.id.clone());
                        report.playlists_changed += 1;
                    }
                }
                (None, Some(remote)) => {
                    let held = connection.library.playlist(&remote.id).await?;
                    let (songs, _) = keyed(held, |song| aliased(&memory.aliases, song));
                    if songs == pair.songs && remote.editable && !pair.follows {
                        connection.library.delete_playlist(&remote.id).await?;
                        paired_remote.insert(remote.id.clone());
                        report.playlists_changed += 1;
                    }
                }
                (None, None) => {}
            }
        }
        // The server's playlists not kept in step yet: with Raygun's of the same name, else as new.
        for remote in &remote_lists {
            if paired_remote.contains(&remote.id) {
                continue;
            }
            let local = local_lists.iter().find(|list| {
                !paired_local.contains(&list.id)
                    && list.name.eq_ignore_ascii_case(remote.name.trim())
            });
            let local_id = match local {
                Some(local) => local.id.clone(),
                None => {
                    report.playlists_made += 1;
                    collections.create_playlist(&remote.name).await?.id
                }
            };
            paired_local.insert(local_id.clone());
            paired_remote.insert(remote.id.clone());
            pairs.push(PlaylistPair {
                local: local_id,
                remote: remote.id.clone(),
                name: remote.name.clone(),
                songs: Vec::new(),
                follows: !remote.editable,
            });
        }
        // Raygun's playlists not on the server: made there.
        for local in &local_lists {
            if paired_local.contains(&local.id) {
                continue;
            }
            let remote = connection.library.create_playlist(&local.name, &[]).await?;
            report.playlists_made += 1;
            pairs.push(PlaylistPair {
                local: local.id.clone(),
                remote,
                name: local.name.clone(),
                songs: Vec::new(),
                follows: false,
            });
        }
        for mut pair in pairs {
            let local = local_lists.iter().find(|list| list.id == pair.local);
            let remote = remote_lists.iter().find(|list| list.id == pair.remote);
            self.sync_playlist(
                connection,
                &mut pair,
                local,
                remote.map(|list| list.name.as_str()),
                memory,
                report,
            )
            .await?;
            memory.playlists.push(pair);
        }
        Ok(())
    }

    /// The keys of a Raygun playlist's songs, in order.
    async fn local_keys(&self, id: &str) -> Result<Vec<String>, String> {
        let detail = self.app.state::<CollectionsService>().playlist(id).await?;
        Ok(keyed(
            detail.items.into_iter().map(|item| item.track).collect(),
            key_of_track,
        )
        .0)
    }

    /// Brings one pair of playlists in step: names, then songs.
    async fn sync_playlist(
        &self,
        connection: &Connection,
        pair: &mut PlaylistPair,
        local: Option<&PlaylistSummary>,
        remote_name: Option<&str>,
        memory: &mut SyncMemory,
        report: &mut ServerSyncReport,
    ) -> Result<(), String> {
        let collections = self.app.state::<CollectionsService>();
        let library = &connection.library;
        // A name changed on one side changes on the other.
        let local_name = local.map_or(pair.name.clone(), |list| list.name.clone());
        let remote_name = remote_name.map_or(local_name.clone(), str::to_owned);
        if local_name != remote_name {
            if pair.follows || local_name == pair.name {
                collections
                    .rename_playlist(&pair.local, &remote_name)
                    .await?;
                pair.name = remote_name;
            } else {
                library.rename_playlist(&pair.remote, &local_name).await?;
                pair.name = local_name;
            }
            report.playlists_changed += 1;
        } else {
            pair.name = local_name;
        }

        let detail = collections.playlist(&pair.local).await?;
        let items: Vec<(String, SavedTrack)> = detail
            .items
            .into_iter()
            .map(|item| (item.id, item.track))
            .collect();
        let (local, local_tracks) = keyed(
            items.iter().map(|(_, track)| track.clone()).collect(),
            key_of_track,
        );
        let held = library.playlist(&pair.remote).await?;
        let (remote, remote_songs) = keyed(held, |song| aliased(&memory.aliases, song));
        // A playlist Raygun can't change on the server is followed as it is.
        let last = if pair.follows {
            local.clone()
        } else {
            pair.songs.clone()
        };
        let plan = plan(&local, &remote, &last);
        let found = if pair.follows {
            HashMap::new()
        } else {
            self.ids_for(connection, &plan.add_remote, &local_tracks, memory, report)
                .await?
        };
        let id_of = |key: &String| {
            remote_songs
                .get(key)
                .map(|song| song.id.clone())
                .or_else(|| found.get(key).cloned())
        };

        // Raygun's side: songs out, songs in, then the order.
        let mut changed_local = false;
        let gone: HashSet<&String> = plan.remove_local.iter().collect();
        for (item, track) in &items {
            if gone.contains(&key_of_track(track)) {
                collections.remove_playlist_item(item).await?;
                changed_local = true;
            }
        }
        let adding: Vec<TrackInput> = plan
            .add_local
            .iter()
            .filter_map(|key| remote_songs.get(key).map(track_of))
            .collect();
        if !adding.is_empty() {
            collections.add_tracks(&pair.local, adding, false).await?;
            changed_local = true;
        }
        let order_now = self.local_keys(&pair.local).await?;
        let wanted: Vec<&String> = plan
            .result
            .iter()
            .filter(|key| order_now.contains(key))
            .collect();
        if order_now.iter().collect::<Vec<_>>() != wanted {
            let detail = collections.playlist(&pair.local).await?;
            let mut by_key: HashMap<String, Vec<String>> = HashMap::new();
            for item in &detail.items {
                by_key
                    .entry(key_of_track(&item.track))
                    .or_default()
                    .push(item.id.clone());
            }
            let mut order: Vec<String> = wanted
                .iter()
                .filter_map(|key| {
                    by_key
                        .get_mut(*key)
                        .and_then(|ids| (!ids.is_empty()).then(|| ids.remove(0)))
                })
                .collect();
            order.extend(by_key.into_values().flatten());
            collections.set_playlist_order(&pair.local, &order).await?;
            changed_local = true;
        }

        // The server's side: exactly the songs it has of the result, in its order.
        let remote_result: Vec<String> = plan.result.iter().filter_map(id_of).collect();
        let remote_now: Vec<String> = remote
            .iter()
            .filter_map(|key| remote_songs.get(key).map(|song| song.id.clone()))
            .collect();
        let mut changed_remote = false;
        if !pair.follows && remote_result != remote_now {
            library
                .replace_playlist(&pair.remote, &remote_result)
                .await?;
            changed_remote = true;
        }
        if changed_local || changed_remote {
            report.playlists_changed += 1;
        }
        pair.songs = plan
            .result
            .into_iter()
            .filter(|key| remote_songs.contains_key(key) || found.contains_key(key))
            .collect();
        Ok(())
    }

    /// A song played long enough to count: counted on the servers that count plays and have it.
    pub fn played(&self, item: &QueueItem) {
        if crate::privacy::incognito(&self.app) || item.media_kind != MediaKind::Audio {
            return;
        }
        let app = self.app.clone();
        let (title, artist) = (item.title.clone(), item.artist.clone());
        tauri::async_runtime::spawn(async move {
            let sync = app.state::<ServerSync>();
            let at = now_ms();
            for connection in sync.connections().await {
                if !connection.prefs.scrobble {
                    continue;
                }
                let counted = match sync.find(&connection, &title, artist.as_deref()).await {
                    Ok(Some((id, _))) => connection.library.scrobble(&id, at, true).await,
                    Ok(None) => continue,
                    Err(error) => Err(error),
                };
                if let Err(error) = counted {
                    tracing::debug!(server = %connection.id, %error, "a play wasn't counted on the server");
                }
            }
        });
    }

    /// The queue changed (`last_played`'s snapshot): kept on the servers that keep it, a little
    /// later.
    pub fn queue_changed(&self, json: &str) {
        let mut queue = self
            .queue
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        queue.0 = Some(json.to_owned());
        if queue.1 {
            return;
        }
        queue.1 = true;
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(QUEUE_SETTLE).await;
                let sync = app.state::<ServerSync>();
                let json = {
                    let mut queue = sync
                        .queue
                        .lock()
                        .unwrap_or_else(|poison| poison.into_inner());
                    match queue.0.take() {
                        Some(json) => json,
                        None => {
                            queue.1 = false;
                            return;
                        }
                    }
                };
                sync.keep_queue(&json).await;
            }
        });
    }

    async fn keep_queue(&self, json: &str) {
        let Ok(last) = serde_json::from_str::<crate::last_played::LastPlayed>(json) else {
            return;
        };
        let connections: Vec<Connection> = self
            .connections()
            .await
            .into_iter()
            .filter(|connection| connection.prefs.play_queue)
            .collect();
        if connections.is_empty() {
            return;
        }
        // Songs in play order, from the one playing: the server keeps no order of its own.
        let order: Vec<usize> = if last.order.len() == last.items.len() {
            last.order.clone()
        } else {
            (0..last.items.len()).collect()
        };
        let start = order
            .iter()
            .position(|&index| index == last.current)
            .unwrap_or(0);
        let items: Vec<&QueueItemInput> = order[start..]
            .iter()
            .chain(&order[..start])
            .filter_map(|&index| last.items.get(index))
            .filter(|item| item.media_kind == MediaKind::Audio)
            .take(QUEUE_MOST)
            .collect();
        for connection in connections {
            let mut ids = Vec::new();
            let mut current = None;
            let mut failed = None;
            for (place, item) in items.iter().enumerate() {
                match self
                    .find(&connection, &item.title, item.artist.as_deref())
                    .await
                {
                    Ok(Some((id, _))) => {
                        if place == 0 {
                            current = Some(id.clone());
                        }
                        ids.push(id);
                    }
                    Ok(None) => {}
                    Err(error) => {
                        failed = Some(error);
                        break;
                    }
                }
            }
            let kept = match failed {
                Some(error) => Err(error),
                None if ids.is_empty() => Ok(()),
                None => {
                    // The place in the song playing; the first song the server has otherwise.
                    let position = if current.is_some() {
                        last.position_ms.max(0.0) as u64
                    } else {
                        0
                    };
                    let current = current.or_else(|| ids.first().cloned());
                    connection
                        .library
                        .save_play_queue(&ids, current.as_deref(), position)
                        .await
                }
            };
            if let Err(error) = kept {
                tracing::debug!(server = %connection.id, %error, "the queue wasn't kept on the server");
            }
        }
    }

    /// Plays the queue the server keeps (left by Feishin, Navic, the web app or Raygun
    /// elsewhere), paused where it was; how many songs it holds.
    pub async fn take_queue(&self, id: &str) -> Result<u32, String> {
        let connection = self.connection(id).await?;
        let queue = connection
            .library
            .play_queue()
            .await?
            .ok_or_else(|| format!("{} keeps no play queue for you", connection.config.name))?;
        let images = self.app.state::<raygun_net::ImageLinks>();
        let current = queue
            .current
            .as_ref()
            .and_then(|current| queue.songs.iter().position(|song| &song.id == current))
            .unwrap_or(0);
        let items: Vec<QueueItemInput> = queue
            .songs
            .iter()
            .map(|song| QueueItemInput {
                title: song.title.clone(),
                artist: song.artist.clone(),
                album: song.album.clone(),
                duration_ms: song.duration_ms,
                artwork_url: song
                    .artwork_url
                    .as_deref()
                    .map(|url| images.cover(url, &Default::default())),
                media_kind: MediaKind::Audio,
                source: MediaSource::Lookup {
                    title: song.title.clone(),
                    artist: song.artist.clone(),
                    album: song.album.clone(),
                    duration_ms: song.duration_ms,
                },
                explicit: false,
            })
            .collect();
        let count = items.len() as u32;
        let position = if queue.current.is_some() {
            queue.position_ms as f64
        } else {
            0.0
        };
        self.app
            .state::<crate::playback::PlaybackService>()
            .take_queue(items, current, position);
        Ok(count)
    }
}

/// Raygun's lists changed: servers that keep them in step sync soon.
pub fn lists_changed(app: &AppHandle) {
    if let Some(sync) = app.try_state::<ServerSync>() {
        sync.lists_changed();
    }
}

/// What a Subsonic or Navidrome server keeps in step with Raygun, and how its last sync went.
#[tauri::command]
#[specta::specta]
pub async fn server_sync_status(app: AppHandle, id: String) -> Result<ServerSyncStatus, String> {
    if !id.starts_with(servers::ID_PREFIX) {
        return Err("That isn't a connected server".into());
    }
    Ok(app.state::<ServerSync>().status(&id).await)
}

/// Sets what a server keeps in step; Favorites or playlists just switched on sync at once.
#[tauri::command]
#[specta::specta]
pub async fn server_sync_set(
    app: AppHandle,
    id: String,
    prefs: ServerSyncPrefs,
) -> Result<ServerSyncStatus, String> {
    if !id.starts_with(servers::ID_PREFIX) {
        return Err("That isn't a connected server".into());
    }
    app.state::<ServerSync>().set_prefs(&id, prefs).await
}

/// Syncs a server's Favorites and playlists now.
#[tauri::command]
#[specta::specta]
pub async fn server_sync_now(app: AppHandle, id: String) -> Result<ServerSyncStatus, String> {
    app.state::<ServerSync>().sync_now(&id).await
}

/// Plays the queue a server keeps, paused where it was; how many songs it holds.
#[tauri::command]
#[specta::specta]
pub async fn server_queue_take(app: AppHandle, id: String) -> Result<u32, String> {
    app.state::<ServerSync>().take_queue(&id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn a_first_sync_joins_both_lists() {
        let planned = plan(&keys(&["a", "b"]), &keys(&["b", "c"]), &[]);
        assert_eq!(planned.add_local, keys(&["c"]));
        assert_eq!(planned.add_remote, keys(&["a"]));
        assert!(planned.remove_local.is_empty() && planned.remove_remote.is_empty());
        assert_eq!(planned.result, keys(&["a", "b", "c"]));
    }

    #[test]
    fn what_one_side_took_out_leaves_the_other() {
        // Raygun took "a" out, the server took "c" out and starred "d".
        let planned = plan(
            &keys(&["b", "c"]),
            &keys(&["a", "b", "d"]),
            &keys(&["a", "b", "c"]),
        );
        assert_eq!(planned.remove_remote, keys(&["a"]));
        assert_eq!(planned.remove_local, keys(&["c"]));
        assert_eq!(planned.add_local, keys(&["d"]));
        assert!(planned.add_remote.is_empty());
        assert_eq!(planned.result, keys(&["b", "d"]));
    }

    #[test]
    fn a_list_changed_only_on_the_server_takes_its_order() {
        let planned = plan(
            &keys(&["a", "b", "c"]),
            &keys(&["c", "a", "b"]),
            &keys(&["a", "b", "c"]),
        );
        assert_eq!(planned.result, keys(&["c", "a", "b"]));
        let unchanged = plan(&keys(&["a", "b"]), &keys(&["a", "b"]), &keys(&["a", "b"]));
        assert_eq!(
            unchanged,
            Plan {
                result: keys(&["a", "b"]),
                ..Plan::default()
            }
        );
    }

    #[test]
    fn songs_are_matched_by_their_names() {
        let song = |id: &str, title: &str, artist: &str| Song {
            id: id.into(),
            title: title.into(),
            artist: Some(artist.into()),
            album: None,
            duration_ms: None,
            artwork_url: None,
        };
        let found = [
            song("1", "Tone 440 Hz", "Raygun Test Tones"),
            song("2", "Tone 220 Hz", "Raygun Test Tones"),
            song("3", "Get Lucky", "Daft Punk & Pharrell Williams"),
        ];
        assert_eq!(
            same_song(&found, "tone 220 hz", Some("Raygun Test Tones"))
                .map(|song| song.id.as_str()),
            Some("2")
        );
        assert_eq!(
            same_song(&found, "Get Lucky", Some("Daft Punk")).map(|song| song.id.as_str()),
            Some("3"),
            "one credit holds the other"
        );
        assert!(same_song(&found, "Tone 880 Hz", Some("Raygun Test Tones")).is_none());
    }
}
