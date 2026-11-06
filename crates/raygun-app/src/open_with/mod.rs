//! Media and links other apps hand to Raygun: Android's "Open with" and "Share" (the plugin's
//! `OpenWith.kt` reads the intent), and files the desktop app is started with, also when a copy
//! is already running (the second copy passes them on). Videos open in the video player, songs
//! and playlists play in the music player, a Spotify link opens the song import, and other links
//! and words go to Search, where a YouTube link shows its card.
//!
//! Android documents (`content://`) are read through the access the sharing app gave: the video
//! player plays them as they are, songs are copied into `opened/` first (Rust reads files by path:
//! the tags, and the stream proxy that serves them to the player), and playlists are read through
//! a descriptor from the plugin.

mod playlist;
mod route;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime};

use raygun_model::{MediaKind, MediaSource, OpenWithProblem, OpenWithProblemKind, QueueItemInput};
use serde::Deserialize;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use playlist::{Base, Entry};
pub use route::file_path;
use route::{Kind, Location, StreamFormat, TextTarget};

/// Where copies of shared songs go, under the data root.
pub const COPIES_DIR: &str = "opened";
/// Copies kept, the most recently opened; older ones are deleted.
const KEPT_COPIES: usize = 20;
/// The largest song copied (an audiobook in one file).
const COPY_LIMIT: u64 = 2 * 1024 * 1024 * 1024;
/// The largest playlist read.
const PLAYLIST_LIMIT: u64 = 2 * 1024 * 1024;
const FETCH_TIMEOUT: Duration = Duration::from_secs(20);
/// How much of a song at a web address is read for its tags: ID3v2 tags, FLAC's and Ogg's
/// comments come first in the file (a large cover included).
const TAG_BYTES: usize = 512 * 1024;
const TAG_TIMEOUT: Duration = Duration::from_secs(8);
const LOCAL_DENIED: &str = "Raygun may not reach devices on this network";

/// What another app handed over: files and addresses, and shared text.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Handover {
    #[serde(default)]
    pub items: Vec<Handed>,
    #[serde(default)]
    pub text: Option<String>,
    /// The shared text's title, when its app gives one.
    #[serde(default)]
    pub subject: Option<String>,
}

/// One file or address.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Handed {
    pub uri: String,
    #[serde(default)]
    pub mime: Option<String>,
    /// Its name as its app shows it.
    #[serde(default)]
    pub name: Option<String>,
    /// Bytes, for knowing a document opened again (Android).
    #[serde(default)]
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub size: Option<u64>,
}

impl Handed {
    fn kind(&self) -> Kind {
        route::kind_of(&self.uri, self.mime.as_deref(), self.name.as_deref())
    }

    fn shown_name(&self) -> String {
        route::shown_name(self.name.as_deref(), &self.uri)
    }
}

/// What a playlist turned out to hold.
enum Listed {
    Songs(Vec<QueueItemInput>),
    /// An HLS stream, or a list of video streams (an IPTV list): the video player plays it, or
    /// its first entry.
    Video(Handed, Kind),
}

pub struct OpenWith {
    app: AppHandle,
    /// The UI is up: it asked for the last queue back (`playback_restore`), so its players and
    /// listeners are there.
    ready: AtomicBool,
    /// Handed over before that on desktop, where the players live in the webview.
    waiting: Mutex<Vec<Handover>>,
    /// Problems before that, shown once the UI is up.
    problems: Mutex<Vec<OpenWithProblem>>,
}

/// On desktop the players are the webview's, so what's handed over waits for the UI. Android's
/// players are native and start at once.
const WAITS_FOR_UI: bool = cfg!(not(target_os = "android"));

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn to_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// Starts listening: Android's intents through the plugin, the desktop app's own arguments.
pub fn install(app: &AppHandle) {
    let service = Arc::new(OpenWith {
        app: app.clone(),
        ready: AtomicBool::new(false),
        waiting: Mutex::default(),
        problems: Mutex::default(),
    });
    app.manage(service.clone());
    #[cfg(target_os = "android")]
    tauri_plugin_raygun_audio::set_open_with_handler(move |json| {
        match serde_json::from_str::<Handover>(&json) {
            Ok(handover) => service.receive(handover),
            Err(error) => tracing::warn!(%error, "unreadable handover from another app"),
        }
    });
    #[cfg(not(target_os = "android"))]
    {
        drop(service);
        if let Ok(cwd) = std::env::current_dir() {
            from_args(app, &std::env::args().collect::<Vec<_>>(), &cwd);
        }
    }
}

/// The UI is up (it asked for the last queue back): what waited for it goes ahead.
pub fn ready(app: &AppHandle) {
    if let Some(service) = app.try_state::<Arc<OpenWith>>() {
        service.set_ready();
    }
}

/// Files and addresses the desktop app was started with; the first argument is the program.
#[cfg_attr(target_os = "android", allow(dead_code))]
pub fn from_args(app: &AppHandle, args: &[String], cwd: &Path) {
    let items: Vec<Handed> = args
        .iter()
        .skip(1)
        .filter_map(|arg| argument(arg, cwd))
        .collect();
    if items.is_empty() {
        return;
    }
    if let Some(service) = app.try_state::<Arc<OpenWith>>() {
        service.receive(Handover {
            items,
            ..Handover::default()
        });
    }
}

/// A file (relative to `cwd`) or web address on the command line; options and anything that
/// isn't there are left out.
#[cfg_attr(target_os = "android", allow(dead_code))]
fn argument(arg: &str, cwd: &Path) -> Option<Handed> {
    let arg = arg.trim().trim_matches('"');
    if arg.is_empty() || arg.starts_with('-') {
        return None;
    }
    if matches!(route::location(arg), Some(Location::Web(_))) {
        return Some(Handed {
            uri: arg.to_owned(),
            ..Handed::default()
        });
    }
    let path = match route::location(arg) {
        Some(Location::File(path)) => path,
        _ => cwd.join(arg),
    };
    path.is_file().then(|| Handed {
        name: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned()),
        uri: path.to_string_lossy().into_owned(),
        ..Handed::default()
    })
}

impl OpenWith {
    fn receive(self: &Arc<Self>, handover: Handover) {
        {
            let mut waiting = lock(&self.waiting);
            if WAITS_FOR_UI && !self.ready.load(Ordering::SeqCst) {
                waiting.push(handover);
                return;
            }
        }
        self.start(handover);
    }

    fn start(self: &Arc<Self>, handover: Handover) {
        let service = self.clone();
        tauri::async_runtime::spawn(async move { service.handle(handover).await });
    }

    fn set_ready(self: &Arc<Self>) {
        let waiting = {
            let mut waiting = lock(&self.waiting);
            self.ready.store(true, Ordering::SeqCst);
            std::mem::take(&mut *waiting)
        };
        for handover in waiting {
            self.start(handover);
        }
        let problems = std::mem::take(&mut *lock(&self.problems));
        for problem in problems {
            self.emit(problem);
        }
    }

    fn problem(&self, kind: OpenWithProblemKind, name: String) {
        let problem = OpenWithProblem { kind, name };
        {
            let mut problems = lock(&self.problems);
            if !self.ready.load(Ordering::SeqCst) {
                problems.push(problem);
                return;
            }
        }
        self.emit(problem);
    }

    fn emit(&self, problem: OpenWithProblem) {
        if let Err(error) = crate::events::OpenWithEvent(problem).emit(&self.app) {
            tracing::debug!(%error, "couldn't say what didn't open");
        }
    }

    /// Shows a screen: `player`, a route with its search (taken by the UI when it starts too).
    fn open_route(&self, route: String) {
        if let Some(notices) = self
            .app
            .try_state::<Arc<crate::notifications::DownloadNotices>>()
        {
            notices.open(route);
        }
    }

    async fn handle(self: Arc<Self>, handover: Handover) {
        let sorted: Vec<(Handed, Kind)> = handover
            .items
            .into_iter()
            .map(|item| {
                let kind = item.kind();
                (item, kind)
            })
            .collect();
        tracing::info!(
            kinds = ?sorted.iter().map(|(_, kind)| *kind).collect::<Vec<_>>(),
            text = handover.text.is_some(),
            "handed over by another app"
        );
        // The first item that plays says which player: a video plays alone, songs and
        // playlists play in order.
        let first = sorted
            .iter()
            .map(|(_, kind)| *kind)
            .find(|kind| *kind != Kind::Other);
        match first {
            Some(kind @ (Kind::Video | Kind::Streaming(_))) => {
                if let Some((item, _)) = sorted.iter().find(|(_, other)| *other == kind) {
                    self.play_video(item, kind).await;
                }
            }
            Some(_) => {
                let songs = sorted
                    .into_iter()
                    .filter(|(_, kind)| matches!(kind, Kind::Audio | Kind::Playlist))
                    .collect();
                self.play_songs(songs).await;
            }
            None => match handover.text.as_deref().and_then(route::text_target) {
                Some(target) => self.open_text(target, handover.subject).await,
                None => {
                    if let Some((item, _)) = sorted.first() {
                        self.problem(OpenWithProblemKind::Unsupported, item.shown_name());
                    }
                }
            },
        }
    }

    async fn open_text(&self, target: TextTarget, subject: Option<String>) {
        let encode =
            |text: &str| url::form_urlencoded::byte_serialize(text.as_bytes()).collect::<String>();
        match target {
            TextTarget::Media { url, kind } => {
                let item = Handed {
                    uri: url,
                    name: subject,
                    ..Handed::default()
                };
                match kind {
                    Kind::Video | Kind::Streaming(_) => self.play_video(&item, kind).await,
                    _ => self.play_songs(vec![(item, kind)]).await,
                }
            }
            TextTarget::Import(url) => self.open_route(format!("import?link={}", encode(&url))),
            TextTarget::Search(query) => {
                self.open_route(format!("/browse?tab=search&q={}", encode(&query)));
            }
        }
    }

    /// Plays one video or stream in the video player.
    async fn play_video(&self, item: &Handed, kind: Kind) {
        let address = match route::location(&item.uri) {
            Some(Location::Web(url)) => Some(url),
            Some(Location::File(path)) => {
                self.media_access().await;
                url::Url::from_file_path(&path).ok().map(String::from)
            }
            // The player reads the document through the access its app gave.
            Some(Location::Document(uri)) if cfg!(target_os = "android") => Some(uri),
            _ => None,
        };
        let Some(address) = address else {
            return self.problem(OpenWithProblemKind::Unreadable, item.shown_name());
        };
        let title = route::title_of(item.name.as_deref(), &item.uri);
        let mut request = crate::video::link_request(&address, title.as_deref(), None);
        if let Kind::Streaming(format) = kind {
            request.format = Some(format.as_str().to_owned());
        }
        if let Err(error) = crate::video::play(&self.app, request).await {
            tracing::warn!(%error, "a video another app opened didn't play");
            self.problem(OpenWithProblemKind::Unreadable, item.shown_name());
        }
    }

    /// Plays songs and playlists in order in the music player, and shows it.
    async fn play_songs(&self, items: Vec<(Handed, Kind)>) {
        let mut queue = Vec::new();
        for (item, kind) in items {
            if kind == Kind::Playlist {
                match self.playlist(&item).await {
                    Ok(Listed::Songs(songs)) if songs.is_empty() => {
                        self.problem(OpenWithProblemKind::EmptyPlaylist, item.shown_name());
                    }
                    Ok(Listed::Songs(songs)) => queue.extend(songs),
                    Ok(Listed::Video(video, kind)) if queue.is_empty() => {
                        return self.play_video(&video, kind).await;
                    }
                    Ok(Listed::Video(..)) => {}
                    Err(error) => {
                        tracing::warn!(%error, "a playlist another app opened couldn't be read");
                        self.problem(OpenWithProblemKind::Unreadable, item.shown_name());
                    }
                }
                continue;
            }
            match self.song(&item).await {
                Ok(song) => queue.push(song),
                Err(error) => {
                    tracing::warn!(%error, "a song another app opened couldn't be read");
                    self.problem(OpenWithProblemKind::Unreadable, item.shown_name());
                }
            }
        }
        if queue.is_empty() {
            return;
        }
        let playback = self.app.state::<crate::playback::PlaybackService>();
        // Played from nowhere in Raygun: no page's context goes with it.
        playback.arm_context(None);
        playback.replace(queue, Some(0), false);
        self.open_route("player".to_owned());
    }

    async fn song(&self, item: &Handed) -> Result<QueueItemInput, String> {
        let entry = Entry {
            title: route::title_of(item.name.as_deref(), &item.uri),
            ..Entry::default()
        };
        match route::location(&item.uri).ok_or("not an address")? {
            Location::Web(url) => {
                let tags = self.web_tags(&url).await;
                Ok(web_song(url, entry, tags))
            }
            Location::File(path) => {
                self.media_access().await;
                if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
                    return Err(format!("{} isn't there", path.display()));
                }
                Ok(self.file_song(path, entry).await)
            }
            Location::Document(uri) => {
                let path = self.copy_document(&uri, item).await?;
                Ok(self.file_song(path, entry).await)
            }
        }
    }

    /// A song file named by its tags, else by the name it came with.
    async fn file_song(&self, path: PathBuf, entry: Entry) -> QueueItemInput {
        if let Some(song) = self
            .app
            .state::<crate::library::LibraryService>()
            .file_item(path.clone())
            .await
        {
            return song;
        }
        let title = entry
            .title
            .or_else(|| route::title_of(None, &path.to_string_lossy()))
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        QueueItemInput {
            explicit: false,
            title,
            artist: entry.artist,
            album: None,
            duration_ms: entry.duration_ms,
            artwork_url: None,
            media_kind: MediaKind::Audio,
            source: MediaSource::LocalFile {
                path: path.to_string_lossy().into_owned(),
            },
        }
    }

    async fn playlist(&self, item: &Handed) -> Result<Listed, String> {
        let location = route::location(&item.uri).ok_or("not an address")?;
        let (text, base) = match &location {
            Location::Web(url) => (
                self.fetch_text(url).await?,
                url::Url::parse(url).map_or(Base::Nowhere, Base::Web),
            ),
            Location::File(path) => {
                self.media_access().await;
                (
                    read_text(path).await?,
                    path.parent()
                        .map_or(Base::Nowhere, |folder| Base::Folder(folder.to_path_buf())),
                )
            }
            Location::Document(uri) => (
                self.read_document(uri).await?,
                crate::local_files::document_path(uri)
                    .and_then(|path| Path::new(&path).parent().map(Path::to_path_buf))
                    .map_or(Base::Nowhere, Base::Folder),
            ),
        };
        if playlist::is_hls(&text) {
            return Ok(Listed::Video(
                item.clone(),
                Kind::Streaming(StreamFormat::Hls),
            ));
        }
        let entries: Vec<(Entry, Location)> = playlist::entries(&text)
            .into_iter()
            .filter_map(|entry| {
                let found = playlist::resolve(&entry.location, &base)?;
                Some((entry, found))
            })
            .collect();
        if entries
            .iter()
            .any(|(_, found)| matches!(found, Location::File(_)))
        {
            // Songs named by path are read by path.
            self.media_access().await;
        }
        let mut songs = Vec::new();
        let mut first_video = None;
        for (entry, found) in entries {
            let kind = route::kind_of(&found.address(), None, None);
            if matches!(kind, Kind::Video | Kind::Streaming(_)) {
                first_video.get_or_insert_with(|| {
                    let video = Handed {
                        uri: found.address(),
                        name: entry.title.clone(),
                        ..Handed::default()
                    };
                    (video, kind)
                });
                continue;
            }
            match found {
                // Radio streams' addresses often name no file: each is tried.
                Location::Web(url) => songs.push(web_song(url, entry, None)),
                Location::File(path) if kind == Kind::Audio => {
                    if tokio::fs::try_exists(&path).await.unwrap_or(false) {
                        songs.push(self.file_song(path, entry).await);
                    }
                }
                // Pictures and other lists beside the songs, and other apps' documents, which
                // Raygun has no access to.
                Location::File(_) | Location::Document(_) => {}
            }
        }
        match first_video {
            Some((video, kind)) if songs.is_empty() => Ok(Listed::Video(video, kind)),
            _ => Ok(Listed::Songs(songs)),
        }
    }

    /// Android asks once for the device's songs and videos, read by path; desktop needs nothing.
    /// The tags at the start of the song at `url`, read from its first bytes: nothing when the
    /// server sends none, or they aren't there.
    async fn web_tags(&self, url: &str) -> Option<Tags> {
        self.app
            .state::<crate::local_network::LocalNetwork>()
            .check(url, LOCAL_DENIED)
            .await
            .ok()?;
        let client = raygun_net::http_client().ok()?;
        let mut response = raygun_net::with_user_agent(client.get(url), std::iter::empty())
            .header(reqwest::header::RANGE, format!("bytes=0-{}", TAG_BYTES - 1))
            .timeout(TAG_TIMEOUT)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .ok()?;
        let mut head = Vec::new();
        while head.len() < TAG_BYTES {
            match response.chunk().await {
                Ok(Some(chunk)) => head.extend_from_slice(&chunk),
                Ok(None) => break,
                Err(_) => return None,
            }
        }
        head.truncate(TAG_BYTES);
        tokio::task::spawn_blocking(move || read_tags(&head))
            .await
            .ok()
            .flatten()
    }

    async fn media_access(&self) {
        crate::commands::request_media_access(&self.app).await;
    }

    async fn fetch_text(&self, url: &str) -> Result<String, String> {
        self.app
            .state::<crate::local_network::LocalNetwork>()
            .check(url, LOCAL_DENIED)
            .await?;
        let client = raygun_net::http_client().map_err(to_message)?;
        let mut response = raygun_net::with_user_agent(client.get(url), std::iter::empty())
            .timeout(FETCH_TIMEOUT)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(to_message)?;
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(to_message)? {
            body.extend_from_slice(&chunk);
            if body.len() as u64 > PLAYLIST_LIMIT {
                return Err("The playlist is too large".into());
            }
        }
        Ok(String::from_utf8_lossy(&body).into_owned())
    }

    #[cfg(target_os = "android")]
    async fn read_document(&self, uri: &str) -> Result<String, String> {
        let app = self.app.clone();
        let uri = uri.to_owned();
        tauri::async_runtime::spawn_blocking(move || {
            use std::io::Read;
            let mut bytes = Vec::new();
            open_document(&app, &uri)?
                .take(PLAYLIST_LIMIT + 1)
                .read_to_end(&mut bytes)
                .map_err(to_message)?;
            if bytes.len() as u64 > PLAYLIST_LIMIT {
                return Err("The playlist is too large".to_owned());
            }
            Ok(String::from_utf8_lossy(&bytes).into_owned())
        })
        .await
        .map_err(to_message)?
    }

    #[cfg(not(target_os = "android"))]
    async fn read_document(&self, _uri: &str) -> Result<String, String> {
        Err("documents open only on Android".into())
    }

    /// Copies a shared song into `opened/`, under the name it came with; the same document
    /// opened again uses its copy.
    #[cfg(target_os = "android")]
    async fn copy_document(&self, uri: &str, item: &Handed) -> Result<PathBuf, String> {
        let root = copies_dir(&self.app.state::<crate::startup::AppPaths>().0);
        let name = route::copy_name(item.name.as_deref(), item.mime.as_deref());
        let key = copy_key(uri, item.size);
        let (app, uri, size) = (self.app.clone(), uri.to_owned(), item.size);
        tauri::async_runtime::spawn_blocking(move || {
            store_copy(&root, &key, &name, size, || open_document(&app, &uri))
        })
        .await
        .map_err(to_message)?
    }

    #[cfg(not(target_os = "android"))]
    async fn copy_document(&self, _uri: &str, _item: &Handed) -> Result<PathBuf, String> {
        Err("documents open only on Android".into())
    }
}

/// The folder of copies of songs other apps handed over (Settings → Data clears it with the
/// caches).
pub fn copies_dir(paths: &raygun_paths::Paths) -> PathBuf {
    paths.root().join(COPIES_DIR)
}

/// Opens a document for reading through the plugin. Blocking.
#[cfg(target_os = "android")]
fn open_document(app: &AppHandle, uri: &str) -> Result<std::fs::File, String> {
    use std::os::fd::FromRawFd;
    let fd = app
        .state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
        .open_input(uri)?;
    // The plugin detached the descriptor for Rust, which now owns it.
    Ok(unsafe { std::fs::File::from_raw_fd(fd) })
}

/// What a song file's tags say of it.
#[derive(Debug, Default, PartialEq, Eq)]
struct Tags {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
}

/// The tags in the first bytes of a song file (`head`), without its audio: lofty reads ID3v2,
/// FLAC and Ogg comments from there.
fn read_tags(head: &[u8]) -> Option<Tags> {
    use lofty::prelude::*;
    let tagged = lofty::probe::Probe::new(std::io::Cursor::new(head))
        .options(lofty::config::ParseOptions::new().read_properties(false))
        .guess_file_type()
        .ok()?
        .read()
        .ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    let text = |value: Option<std::borrow::Cow<'_, str>>| {
        value
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty())
    };
    let tags = Tags {
        title: text(tag.title()),
        artist: text(tag.artist()),
        album: text(tag.album()),
    };
    (tags != Tags::default()).then_some(tags)
}

/// A song or stream at a web address, named by its playlist entry, else its tags (read from its
/// first bytes when it's opened on its own), else its address.
fn web_song(url: String, entry: Entry, tags: Option<Tags>) -> QueueItemInput {
    let tags = tags.unwrap_or_default();
    QueueItemInput {
        explicit: false,
        title: entry
            .title
            .filter(|_| tags.title.is_none())
            .or(tags.title)
            .or_else(|| route::title_of(None, &url))
            .unwrap_or_else(|| url.clone()),
        artist: entry.artist.or(tags.artist),
        album: tags.album,
        duration_ms: entry.duration_ms,
        artwork_url: None,
        media_kind: MediaKind::Audio,
        source: MediaSource::Url {
            url,
            headers: Default::default(),
        },
    }
}

async fn read_text(path: &Path) -> Result<String, String> {
    let metadata = tokio::fs::metadata(path).await.map_err(to_message)?;
    if metadata.len() > PLAYLIST_LIMIT {
        return Err("The playlist is too large".into());
    }
    let bytes = tokio::fs::read(path).await.map_err(to_message)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// The folder a document's copy goes in: the same document (address and size) the same folder.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn copy_key(uri: &str, size: Option<u64>) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(uri.as_bytes());
    hasher.update(size.unwrap_or_default().to_le_bytes());
    hasher.finalize()[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Copies what `open` reads to `<root>/<key>/<name>`, unless a copy of the same size is there,
/// then keeps only the [`KEPT_COPIES`] most recently opened copies. Blocking.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn store_copy(
    root: &Path,
    key: &str,
    name: &str,
    size: Option<u64>,
    open: impl FnOnce() -> Result<std::fs::File, String>,
) -> Result<PathBuf, String> {
    use std::io::Read;
    let folder = root.join(key);
    let target = folder.join(name);
    if let Ok(metadata) = std::fs::metadata(&target)
        && size.is_none_or(|size| size == metadata.len())
    {
        // Opened again: it counts as recent.
        if let Ok(file) = std::fs::OpenOptions::new().write(true).open(&target) {
            let _ = file.set_modified(SystemTime::now());
        }
        return Ok(target);
    }
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).map_err(to_message)?;
    let partial = folder.join(".partial");
    let copied = open().and_then(|input| {
        let mut output = std::fs::File::create(&partial).map_err(to_message)?;
        std::io::copy(&mut input.take(COPY_LIMIT + 1), &mut output).map_err(to_message)
    });
    let finished = match copied {
        Ok(bytes) if bytes > COPY_LIMIT => Err("The file is too large to copy".to_owned()),
        Ok(_) => std::fs::rename(&partial, &target).map_err(to_message),
        Err(error) => Err(error),
    };
    if let Err(error) = finished {
        let _ = std::fs::remove_dir_all(&folder);
        return Err(error);
    }
    prune(root, &folder);
    Ok(target)
}

/// Deletes all but the [`KEPT_COPIES`] most recently opened copies; `current` always stays.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn prune(root: &Path, current: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let mut others: Vec<(SystemTime, PathBuf)> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && path != current)
        .map(|folder| (last_opened(&folder), folder))
        .collect();
    others.sort_by_key(|(opened, _)| std::cmp::Reverse(*opened));
    for (_, folder) in others.into_iter().skip(KEPT_COPIES.saturating_sub(1)) {
        let _ = std::fs::remove_dir_all(&folder);
    }
}

fn last_opened(folder: &Path) -> SystemTime {
    std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.metadata().ok()?.modified().ok())
        .max()
        .unwrap_or(SystemTime::UNIX_EPOCH)
}
