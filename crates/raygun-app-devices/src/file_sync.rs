//! Files to linked devices (M46.57): downloaded songs, chapters and episodes and picked covers go
//! to the devices this one is linked with, over the sync connection (`raygun_sync::files`): one
//! item at a time, checked, resumed where a cut-off transfer stopped. The other device keeps each
//! as its own (its download rows, its folders), so it plays and reads offline there as if
//! downloaded there.
//!
//! Each linked device has its own choices ([`DeviceFilesPrefs`]): which kinds go, and whether
//! only on Wi-Fi. Song edits (trims, volume, lyrics and their timing) are rows of the synced
//! `song_edit` table, not files: they go with the library unless switched off for a device
//! ([`Engine::set_row_filter`]); turning them back on sends them all again.
//!
//! Files go after each sync round with a device (`device_link`), and on Send now. Nothing goes
//! while Offline Mode is on, in battery saver (unless downloads may run there), or on mobile data
//! for a device set to Wi-Fi only. What a device kept is remembered here, so it isn't offered
//! again; the receiver also turns away what it already has.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex, RwLock};
use std::time::{Duration, Instant};

use raygun_db::sync::{self as store, Peer};
use raygun_model::{
    DeviceFileKind, DeviceFilesDirection, DeviceFilesPeer, DeviceFilesPrefs, DeviceFilesStatus,
    DeviceFilesTransfer,
};
use raygun_sync::files::{
    BoxFuture, Direction, FileSink, Outgoing, Progress, ProgressHook, SendReport,
};
use raygun_sync::protocol::{FileItem, ItemFile};
use raygun_sync::{Engine, RowFilter};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::events::DeviceFilesEvent;
use crate::notifications::{self, Notice};
use crate::startup::{AppDb, AppPaths};

/// `local_meta`: each linked device's choices, by sync id.
const PREFS_KEY: &str = "devices.files.prefs";
/// `local_meta`: what each linked device kept, by sync id (item ids).
const SENT_KEY: &str = "devices.files.sent";
/// Where transfers wait until complete, under the downloads folder.
const STAGING: &str = ".incoming";
/// Progress reaches the UI at most this often.
const TELL_EVERY: Duration = Duration::from_millis(400);
/// The synced table that only goes to devices with song edits on.
const SONG_EDITS: &str = "song_edit";

/// Item kinds on the wire.
const SONG: &str = "song";
const CHAPTER: &str = "chapter";
const EPISODE: &str = "episode";
const COVER: &str = "cover";

/// Choices by device, read by the row filter (which can't wait on the database).
static PREFS: LazyLock<RwLock<HashMap<String, DeviceFilesPrefs>>> = LazyLock::new(RwLock::default);

fn prefs_of(device_id: &str) -> DeviceFilesPrefs {
    PREFS
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .get(device_id)
        .cloned()
        .unwrap_or_else(default_prefs)
}

/// A device linked before anything was chosen gets song edits (rows, cheap) and no files.
fn default_prefs() -> DeviceFilesPrefs {
    DeviceFilesPrefs {
        kinds: vec![DeviceFileKind::SongEdits],
        wifi_only: true,
    }
}

#[derive(Debug, Default, Clone)]
struct PeerNote {
    last_at: Option<i64>,
    note: Option<String>,
    waiting: u32,
}

pub struct FileSyncService {
    app: AppHandle,
    engine: Arc<Engine>,
    /// One round at a time.
    round: tokio::sync::Mutex<()>,
    cancel: Mutex<Option<Arc<AtomicBool>>>,
    transfer: Mutex<Option<DeviceFilesTransfer>>,
    told: Mutex<Option<(Instant, String)>>,
    notes: Mutex<HashMap<String, PeerNote>>,
}

impl FileSyncService {
    pub fn new(app: AppHandle, engine: Arc<Engine>) -> Arc<Self> {
        Arc::new(Self {
            app,
            engine,
            round: tokio::sync::Mutex::new(()),
            cancel: Mutex::new(None),
            transfer: Mutex::new(None),
            told: Mutex::new(None),
            notes: Mutex::new(HashMap::new()),
        })
    }

    /// Reads the choices and takes files from linked devices.
    pub async fn start(self: &Arc<Self>) {
        let stored = raygun_db::local_value(&self.db(), PREFS_KEY)
            .await
            .ok()
            .flatten()
            .and_then(|json| serde_json::from_str::<HashMap<String, DeviceFilesPrefs>>(&json).ok())
            .unwrap_or_default();
        *PREFS.write().unwrap_or_else(|p| p.into_inner()) = stored;
        let filter: RowFilter = Arc::new(|device_id, table| {
            table != SONG_EDITS
                || prefs_of(device_id)
                    .kinds
                    .contains(&DeviceFileKind::SongEdits)
        });
        self.engine.set_row_filter(Some(filter));
        let staging = self.app.state::<AppPaths>().0.downloads().join(STAGING);
        self.engine.set_file_sink(Some(Arc::new(AppSink {
            app: self.app.clone(),
            staging,
            names: Mutex::new(HashMap::new()),
        })));
    }

    pub async fn status(&self) -> DeviceFilesStatus {
        let peers = store::peers(&self.db()).await.unwrap_or_default();
        let sent = self.sent_all().await;
        let notes = self.notes.lock().expect("notes poisoned").clone();
        DeviceFilesStatus {
            peers: peers
                .into_iter()
                // Browsers take no files.
                .filter(|peer| peer.address.is_some())
                .map(|peer| {
                    let note = notes.get(&peer.device_id).cloned().unwrap_or_default();
                    DeviceFilesPeer {
                        prefs: prefs_of(&peer.device_id),
                        delivered: sent.get(&peer.device_id).map_or(0, |ids| ids.len()) as u32,
                        waiting: note.waiting,
                        last_at: note.last_at.map(|at| at as f64),
                        note: note.note.or_else(|| self.held_back(&peer.device_id)),
                        device_id: peer.device_id,
                        name: peer.name,
                    }
                })
                .collect(),
            transfer: self.transfer.lock().expect("transfer poisoned").clone(),
        }
    }

    /// Changes what goes to `device_id`. Kinds turned on go at once.
    pub async fn set_prefs(
        self: &Arc<Self>,
        device_id: &str,
        prefs: DeviceFilesPrefs,
    ) -> Result<DeviceFilesStatus, String> {
        let before = prefs_of(device_id);
        let mut kinds: Vec<DeviceFileKind> = Vec::new();
        for kind in prefs.kinds {
            if !kinds.contains(&kind) {
                kinds.push(kind);
            }
        }
        let prefs = DeviceFilesPrefs { kinds, ..prefs };
        let all = {
            let mut all = PREFS.write().unwrap_or_else(|p| p.into_inner());
            all.insert(device_id.to_owned(), prefs.clone());
            all.clone()
        };
        let json = serde_json::to_string(&all).map_err(|error| error.to_string())?;
        raygun_db::set_local_value(&self.db(), PREFS_KEY, &json)
            .await
            .map_err(|error| error.to_string())?;
        // Song edits left out before go now: the whole log goes to that device again.
        let edits = DeviceFileKind::SongEdits;
        if prefs.kinds.contains(&edits) && !before.kinds.contains(&edits) {
            let kinds = self.engine.kinds();
            store::set_sent_seq(&self.db(), device_id, 0, &kinds)
                .await
                .map_err(|error| error.to_string())?;
        }
        let grew = prefs
            .kinds
            .iter()
            .any(|kind| *kind != edits && !before.kinds.contains(kind));
        if grew {
            let service = self.clone();
            let device = device_id.to_owned();
            tauri::async_runtime::spawn(async move { service.send_to(&device).await });
        }
        let status = self.status().await;
        self.emit(status.clone());
        Ok(status)
    }

    /// Sends what's waiting to every linked device (after a sync round); failures only go to
    /// the notes.
    pub async fn send_all(self: &Arc<Self>) {
        let peers = store::peers(&self.db()).await.unwrap_or_default();
        for peer in peers {
            if peer.address.is_some() && prefs_of(&peer.device_id).kinds.iter().any(is_files) {
                self.send_to(&peer.device_id).await;
            }
        }
    }

    /// Sends what's waiting to one device now.
    pub async fn send_now(self: &Arc<Self>, device_id: &str) -> Result<DeviceFilesStatus, String> {
        raygun_net::network_mode::check_online()?;
        let service = self.clone();
        let device = device_id.to_owned();
        tauri::async_runtime::spawn(async move { service.send_to(&device).await });
        Ok(self.status().await)
    }

    /// Stops the transfer going on; what arrived stays for next time.
    pub async fn cancel(&self) -> DeviceFilesStatus {
        if let Some(cancel) = self.cancel.lock().expect("cancel poisoned").as_ref() {
            cancel.store(true, Ordering::SeqCst);
        }
        self.status().await
    }

    /// Why files to `device_id` wait now, if they do.
    fn held_back(&self, device_id: &str) -> Option<String> {
        if raygun_net::network_mode::offline() {
            return Some("Waiting: Offline Mode is on".into());
        }
        if crate::device_state::downloads_held() {
            return Some("Waiting: battery saver is on".into());
        }
        if prefs_of(device_id).wifi_only && crate::network::on_mobile_data(&self.app) {
            return Some("Waiting for Wi-Fi".into());
        }
        None
    }

    /// One round to `device_id`; none while another runs (it's tried again after the next sync).
    async fn send_to(self: &Arc<Self>, device_id: &str) {
        let Ok(_round) = self.round.try_lock() else {
            return;
        };
        let prefs = prefs_of(device_id);
        let kinds: Vec<DeviceFileKind> = prefs.kinds.iter().copied().filter(is_files).collect();
        if kinds.is_empty() {
            return;
        }
        if let Some(note) = self.held_back(device_id) {
            self.note(device_id, |entry| entry.note = Some(note));
            return;
        }
        let Ok(Some(peer)) = store::peer(&self.db(), device_id).await else {
            return;
        };
        let sent = self.sent_all().await.remove(device_id).unwrap_or_default();
        let items: Vec<Outgoing> = self
            .items(&kinds)
            .await
            .into_iter()
            .filter(|out| !sent.contains(&out.item.id))
            .collect();
        self.note(device_id, |entry| {
            entry.waiting = items.len() as u32;
            entry.note = None;
        });
        if items.is_empty() {
            self.emit(self.status().await);
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        *self.cancel.lock().expect("cancel poisoned") = Some(cancel.clone());
        let service = self.clone();
        let name = peer.name.clone();
        let progress: ProgressHook = Arc::new(move |progress| service.moved(&name, progress));
        tracing::info!(device = %peer.name, items = items.len(), "sending files");
        let result = self
            .engine
            .send_files(device_id, items.clone(), progress, cancel)
            .await;
        *self.cancel.lock().expect("cancel poisoned") = None;
        let note = match result {
            Ok(report) => {
                self.remember_sent(device_id, &report).await;
                let left = items.len().saturating_sub(report.delivered.len()) as u32;
                self.note(device_id, |entry| entry.waiting = left);
                summary(&report)
            }
            Err(error) => {
                tracing::info!(device = %peer.name, %error, "files didn't go");
                Some(format!("Files didn't go: {error}"))
            }
        };
        self.note(device_id, |entry| {
            entry.note = note;
            entry.last_at = Some(crate::catalog::now_ms());
        });
        self.finished(&peer).await;
    }

    /// What can go of `kinds`, as items with their files.
    async fn items(&self, kinds: &[DeviceFileKind]) -> Vec<Outgoing> {
        let mut items = Vec::new();
        if kinds.contains(&DeviceFileKind::Songs) {
            let songs = self
                .app
                .state::<Arc<crate::music_downloads::SongDownloadService>>();
            for (row, path) in songs.shareable().await {
                items.extend(song_item(&row, path));
            }
        }
        if kinds.contains(&DeviceFileKind::Chapters) {
            let chapters = self
                .app
                .state::<Arc<crate::manga_downloads::MangaDownloadService>>();
            for chapter in chapters.shareable().await {
                items.push(chapter_item(chapter));
            }
        }
        if kinds.contains(&DeviceFileKind::Episodes) {
            let podcasts = self.app.state::<Arc<crate::podcasts::PodcastService>>();
            for (podcast, episode, path) in podcasts.shareable().await {
                items.extend(episode_item(&podcast, &episode, path));
            }
        }
        if kinds.contains(&DeviceFileKind::Covers) {
            let details = self.app.state::<Arc<crate::details::DetailService>>();
            for (row, path) in details.shareable().await {
                items.extend(cover_item(&row, path));
            }
        }
        items
    }

    /// Bytes moved, either way: the UI hears at most every [`TELL_EVERY`] (and on each new
    /// item), the notification on each new item.
    fn moved(&self, device_name: &str, progress: &Progress) {
        let transfer = DeviceFilesTransfer {
            device_id: progress.device_id.clone(),
            device_name: device_name.to_owned(),
            direction: match progress.direction {
                Direction::Sending => DeviceFilesDirection::Sending,
                Direction::Receiving => DeviceFilesDirection::Receiving,
            },
            item_name: progress.item_name.clone(),
            item_kind: progress.kind.clone(),
            index: progress.index as u32 + 1,
            count: progress.count as u32,
            done_bytes: progress.done_bytes as f64,
            total_bytes: progress.total_bytes as f64,
        };
        *self.transfer.lock().expect("transfer poisoned") = Some(transfer.clone());
        let (tell, new_item) = {
            let mut told = self.told.lock().expect("told poisoned");
            let new_item = told
                .as_ref()
                .is_none_or(|(_, item)| *item != progress.item_id);
            let tell = new_item
                || told
                    .as_ref()
                    .is_none_or(|(at, _)| at.elapsed() >= TELL_EVERY);
            if tell {
                *told = Some((Instant::now(), progress.item_id.clone()));
            }
            (tell, new_item)
        };
        if !tell {
            return;
        }
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let service = app.state::<Arc<FileSyncService>>();
            service.emit(service.status().await);
            if new_item {
                notifications::show_notice(&app, notice_for(&transfer)).await;
            }
        });
    }

    /// A round ended (either way): the transfer and its notification go.
    async fn finished(&self, peer: &Peer) {
        *self.transfer.lock().expect("transfer poisoned") = None;
        *self.told.lock().expect("told poisoned") = None;
        notifications::cancel_notice(&self.app, notice_id(&peer.device_id)).await;
        self.emit(self.status().await);
    }

    async fn sent_all(&self) -> HashMap<String, HashSet<String>> {
        raygun_db::local_value(&self.db(), SENT_KEY)
            .await
            .ok()
            .flatten()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    async fn remember_sent(&self, device_id: &str, report: &SendReport) {
        if report.delivered.is_empty() {
            return;
        }
        let mut all = self.sent_all().await;
        all.entry(device_id.to_owned())
            .or_default()
            .extend(report.delivered.iter().cloned());
        if let Ok(json) = serde_json::to_string(&all)
            && let Err(error) = raygun_db::set_local_value(&self.db(), SENT_KEY, &json).await
        {
            tracing::warn!(%error, "couldn't remember what a device has");
        }
    }

    /// Forgets what was sent to a device that's no longer linked.
    pub async fn forget(&self, device_id: &str) {
        let mut all = self.sent_all().await;
        if all.remove(device_id).is_some()
            && let Ok(json) = serde_json::to_string(&all)
        {
            let _ = raygun_db::set_local_value(&self.db(), SENT_KEY, &json).await;
        }
    }

    fn note(&self, device_id: &str, change: impl FnOnce(&mut PeerNote)) {
        change(
            self.notes
                .lock()
                .expect("notes poisoned")
                .entry(device_id.to_owned())
                .or_default(),
        );
    }

    fn emit(&self, status: DeviceFilesStatus) {
        if let Err(error) = DeviceFilesEvent(status).emit(&self.app) {
            tracing::debug!(%error, "failed to emit device files event");
        }
    }

    fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<AppDb>().inner().0.clone()
    }
}

fn is_files(kind: &DeviceFileKind) -> bool {
    *kind != DeviceFileKind::SongEdits
}

/// The note a finished round leaves: what didn't go, or nothing when everything did.
fn summary(report: &SendReport) -> Option<String> {
    if report.cancelled {
        return Some("Stopped; the rest goes next time".into());
    }
    match report.failed.as_slice() {
        [] => None,
        [(_, why)] => Some(format!("1 item didn't go: {why}")),
        many => Some(format!("{} items didn't go: {}", many.len(), many[0].1)),
    }
}

fn notice_id(device_id: &str) -> i32 {
    notifications::notice_id(&format!("devices.files.{device_id}"))
}

fn notice_for(transfer: &DeviceFilesTransfer) -> Notice {
    let title = match transfer.direction {
        DeviceFilesDirection::Sending => format!("Sending to {}", transfer.device_name),
        DeviceFilesDirection::Receiving => format!("Receiving from {}", transfer.device_name),
    };
    Notice {
        id: notice_id(&transfer.device_id),
        channel: &notifications::DEVICES,
        title,
        text: format!(
            "{} ({} of {})",
            transfer.item_name, transfer.index, transfer.count
        ),
        route: "/settings/devices".into(),
    }
}

// ---- What goes on the wire ----

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SongMeta {
    track_key: String,
    title: String,
    artist: Option<String>,
    album: Option<String>,
    duration_ms: Option<i64>,
    artwork_url: Option<String>,
    source: Option<String>,
    file: String,
    file_kind: String,
    provider: Option<String>,
    file_ms: Option<i64>,
    origin_provider: Option<String>,
    origin_stream: Option<String>,
    origin_source: Option<String>,
    origin_title: Option<String>,
    origin_artist: Option<String>,
}

fn song_item(row: &raygun_db::music_downloads::SongDownloadRow, path: PathBuf) -> Option<Outgoing> {
    let size = std::fs::metadata(&path).ok()?.len();
    let file = row.file.clone()?;
    let meta = SongMeta {
        track_key: row.song.track_key.clone(),
        title: row.song.title.clone(),
        artist: row.song.artist.clone(),
        album: row.song.album.clone(),
        duration_ms: row.song.duration_ms,
        artwork_url: row.song.artwork_url.clone(),
        source: row.song.source.clone(),
        file: file.clone(),
        file_kind: row.kind.clone().unwrap_or_else(|| "file".into()),
        provider: row.provider.clone(),
        file_ms: row.file_ms,
        origin_provider: row.origin.provider.clone(),
        origin_stream: row.origin.stream.clone(),
        origin_source: row.origin.source.clone(),
        origin_title: row.origin.title.clone(),
        origin_artist: row.origin.artist.clone(),
    };
    let name = match &row.song.artist {
        Some(artist) => format!("{} · {artist}", row.song.title),
        None => row.song.title.clone(),
    };
    Some(Outgoing {
        item: FileItem {
            id: format!("{SONG}:{}:{size}", row.song.track_key),
            kind: SONG.into(),
            name,
            meta: serde_json::to_value(meta).ok()?,
            files: vec![ItemFile { name: file, size }],
        },
        paths: vec![path],
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TitleMeta {
    kind: String,
    source_id: String,
    remote_id: String,
    title: String,
    cover: Option<String>,
    cover_headers: std::collections::BTreeMap<String, String>,
    authors: Vec<String>,
    artists: Vec<String>,
    description: Option<String>,
    genres: Vec<String>,
    status: String,
    url: Option<String>,
    in_library: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnitMeta {
    remote_id: String,
    name: String,
    number: Option<f64>,
    volume: Option<String>,
    scanlator: Option<String>,
    date: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChapterMeta {
    title: TitleMeta,
    units: Vec<UnitMeta>,
    unit: String,
}

fn unit_meta(fields: &raygun_db::titles::UnitFields) -> UnitMeta {
    UnitMeta {
        remote_id: fields.remote_id.clone(),
        name: fields.name.clone(),
        number: fields.number,
        volume: fields.volume.clone(),
        scanlator: fields.scanlator.clone(),
        date: fields.date,
    }
}

fn chapter_item(chapter: crate::manga_downloads::SharedChapter) -> Outgoing {
    let title = &chapter.title.fields;
    let meta = ChapterMeta {
        title: TitleMeta {
            kind: title.kind.clone(),
            source_id: title.source_id.clone(),
            remote_id: title.remote_id.clone(),
            title: title.title.clone(),
            cover: title.cover.clone(),
            cover_headers: title.cover_headers.clone(),
            authors: title.authors.clone(),
            artists: title.artists.clone(),
            description: title.description.clone(),
            genres: title.genres.clone(),
            status: title.status.clone(),
            url: title.url.clone(),
            in_library: chapter.title.in_library_at.is_some(),
        },
        units: chapter
            .units
            .iter()
            .map(|unit| unit_meta(&unit.fields))
            .collect(),
        unit: chapter.unit.fields.remote_id.clone(),
    };
    let files: Vec<ItemFile> = chapter
        .files
        .iter()
        .map(|(name, path)| ItemFile {
            name: name.clone(),
            size: std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0),
        })
        .collect();
    let size: u64 = files.iter().map(|file| file.size).sum();
    Outgoing {
        item: FileItem {
            id: format!(
                "{CHAPTER}:{}:{}:{}:{size}",
                title.source_id, title.remote_id, chapter.unit.fields.remote_id
            ),
            kind: CHAPTER.into(),
            name: format!("{} · {}", title.title, chapter.unit.fields.name),
            meta: serde_json::to_value(meta).unwrap_or(Value::Null),
            files,
        },
        paths: chapter.files.into_iter().map(|(_, path)| path).collect(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EpisodeMeta {
    feed_url: String,
    show_title: String,
    show_author: Option<String>,
    show_description: Option<String>,
    show_artwork: Option<String>,
    show_link: Option<String>,
    show_kind: String,
    subscribed: bool,
    guid: String,
    title: String,
    description: Option<String>,
    audio_url: String,
    audio_type: Option<String>,
    duration_ms: Option<i64>,
    published_at: Option<i64>,
    artwork: Option<String>,
    number: Option<i64>,
    transcript: Option<String>,
}

fn episode_item(
    podcast: &raygun_db::podcasts::PodcastRow,
    episode: &raygun_db::podcasts::EpisodeRow,
    path: PathBuf,
) -> Option<Outgoing> {
    let size = std::fs::metadata(&path).ok()?.len();
    let show = &podcast.fields;
    let fields = &episode.fields;
    let meta = EpisodeMeta {
        feed_url: show.feed_url.clone(),
        show_title: show.title.clone(),
        show_author: show.author.clone(),
        show_description: show.description.clone(),
        show_artwork: show.artwork.clone(),
        show_link: show.link.clone(),
        show_kind: show.kind.clone(),
        subscribed: podcast.subscribed_at.is_some(),
        guid: fields.guid.clone(),
        title: fields.title.clone(),
        description: fields.description.clone(),
        audio_url: fields.audio_url.clone(),
        audio_type: fields.audio_type.clone(),
        duration_ms: fields.duration_ms,
        published_at: fields.published_at,
        artwork: fields.artwork.clone(),
        number: fields.number,
        transcript: fields.transcript.clone(),
    };
    let file = path.file_name()?.to_string_lossy().into_owned();
    Some(Outgoing {
        item: FileItem {
            id: format!("{EPISODE}:{}:{}:{size}", show.feed_url, fields.guid),
            kind: EPISODE.into(),
            name: format!("{} · {}", show.title, fields.title),
            meta: serde_json::to_value(meta).ok()?,
            files: vec![ItemFile { name: file, size }],
        },
        paths: vec![path],
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CoverMeta {
    kind: String,
    key: String,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
}

fn cover_item(row: &raygun_db::detail_edits::DetailEditRow, path: PathBuf) -> Option<Outgoing> {
    let size = std::fs::metadata(&path).ok()?.len();
    let file = path.file_name()?.to_string_lossy().into_owned();
    let meta = CoverMeta {
        kind: row.kind.clone(),
        key: row.key.clone(),
        title: row.title.clone(),
        artist: row.artist.clone(),
        album: row.album.clone(),
    };
    let name = row
        .title
        .clone()
        .or_else(|| row.album.clone())
        .unwrap_or_else(|| row.key.clone());
    Some(Outgoing {
        item: FileItem {
            id: format!("{COVER}:{}:{}:{file}", row.kind, row.key),
            kind: COVER.into(),
            name,
            meta: serde_json::to_value(meta).ok()?,
            files: vec![ItemFile { name: file, size }],
        },
        paths: vec![path],
    })
}

// ---- Receiving ----

/// Keeps what linked devices send in this device's own stores.
struct AppSink {
    app: AppHandle,
    staging: PathBuf,
    /// Names of the devices sending, for progress lines.
    names: Mutex<HashMap<String, String>>,
}

impl AppSink {
    async fn wants(&self, item: &FileItem) -> bool {
        let app = &self.app;
        match item.kind.as_str() {
            SONG => match serde_json::from_value::<SongMeta>(item.meta.clone()) {
                Ok(meta) => {
                    !app.state::<Arc<crate::music_downloads::SongDownloadService>>()
                        .has_song(&meta.track_key)
                        .await
                }
                Err(_) => false,
            },
            CHAPTER => match serde_json::from_value::<ChapterMeta>(item.meta.clone()) {
                Ok(meta) => {
                    !app.state::<Arc<crate::manga_downloads::MangaDownloadService>>()
                        .has_chapter(&meta.title.source_id, &meta.title.remote_id, &meta.unit)
                        .await
                }
                Err(_) => false,
            },
            EPISODE => match serde_json::from_value::<EpisodeMeta>(item.meta.clone()) {
                Ok(meta) => {
                    !app.state::<Arc<crate::podcasts::PodcastService>>()
                        .has_episode(&meta.feed_url, &meta.guid)
                        .await
                }
                Err(_) => false,
            },
            COVER => match serde_json::from_value::<CoverMeta>(item.meta.clone()) {
                Ok(meta) => item.files.first().is_some_and(|file| {
                    !app.state::<Arc<crate::details::DetailService>>()
                        .has_cover(&meta.kind, &meta.key, &file.name)
                }),
                Err(_) => false,
            },
            // Kinds a newer Raygun sends.
            _ => false,
        }
    }

    async fn keep(&self, item: &FileItem, files: &[PathBuf]) -> Result<(), String> {
        let unreadable = |error: serde_json::Error| format!("What came can't be read: {error}");
        let one = || files.first().map(PathBuf::as_path).ok_or("Nothing came");
        let app = &self.app;
        match item.kind.as_str() {
            SONG => {
                let meta: SongMeta =
                    serde_json::from_value(item.meta.clone()).map_err(unreadable)?;
                let song = raygun_db::music_downloads::NewSong {
                    track_key: meta.track_key,
                    title: meta.title,
                    artist: meta.artist,
                    album: meta.album,
                    duration_ms: meta.duration_ms,
                    artwork_url: meta.artwork_url,
                    source: meta.source,
                };
                let bytes = std::fs::metadata(one()?).ok().map(|meta| meta.len() as i64);
                let saved = raygun_db::music_downloads::Saved {
                    file: meta.file,
                    kind: meta.file_kind,
                    provider: meta.provider,
                    bytes,
                    file_ms: meta.file_ms,
                    origin: raygun_db::music_downloads::Origin {
                        provider: meta.origin_provider,
                        stream: meta.origin_stream,
                        source: meta.origin_source,
                        title: meta.origin_title,
                        artist: meta.origin_artist,
                    },
                };
                app.state::<Arc<crate::music_downloads::SongDownloadService>>()
                    .add_received(song, saved, one()?)
                    .await
            }
            CHAPTER => {
                let meta: ChapterMeta =
                    serde_json::from_value(item.meta.clone()).map_err(unreadable)?;
                let title = raygun_db::titles::TitleFields {
                    kind: meta.title.kind,
                    source_id: meta.title.source_id,
                    remote_id: meta.title.remote_id,
                    title: meta.title.title,
                    cover: meta.title.cover,
                    cover_headers: meta.title.cover_headers,
                    authors: meta.title.authors,
                    artists: meta.title.artists,
                    description: meta.title.description,
                    genres: meta.title.genres,
                    status: meta.title.status,
                    url: meta.title.url,
                    media_type: None,
                    year: None,
                };
                let units: Vec<raygun_db::titles::UnitFields> = meta
                    .units
                    .into_iter()
                    .map(|unit| raygun_db::titles::UnitFields {
                        remote_id: unit.remote_id,
                        name: unit.name,
                        number: unit.number,
                        volume: unit.volume,
                        scanlator: unit.scanlator,
                        date: unit.date,
                    })
                    .collect();
                let named: Vec<(String, PathBuf)> = item
                    .files
                    .iter()
                    .map(|file| file.name.clone())
                    .zip(files.iter().cloned())
                    .collect();
                app.state::<Arc<crate::manga_downloads::MangaDownloadService>>()
                    .add_received(&title, meta.title.in_library, &units, &meta.unit, &named)
                    .await
            }
            EPISODE => {
                let meta: EpisodeMeta =
                    serde_json::from_value(item.meta.clone()).map_err(unreadable)?;
                let podcast = raygun_db::podcasts::PodcastFields {
                    feed_url: meta.feed_url,
                    title: meta.show_title,
                    author: meta.show_author,
                    description: meta.show_description,
                    artwork: meta.show_artwork,
                    link: meta.show_link,
                    kind: meta.show_kind,
                };
                let episode = raygun_db::podcasts::EpisodeFields {
                    guid: meta.guid,
                    title: meta.title,
                    description: meta.description,
                    audio_url: meta.audio_url,
                    audio_type: meta.audio_type,
                    duration_ms: meta.duration_ms,
                    published_at: meta.published_at,
                    artwork: meta.artwork,
                    number: meta.number,
                    transcript: meta.transcript,
                };
                let name = item
                    .files
                    .first()
                    .map_or("episode", |file| file.name.as_str());
                app.state::<Arc<crate::podcasts::PodcastService>>()
                    .add_received(&podcast, meta.subscribed, &episode, name, one()?)
                    .await
            }
            COVER => {
                let meta: CoverMeta =
                    serde_json::from_value(item.meta.clone()).map_err(unreadable)?;
                let row = raygun_db::detail_edits::DetailEditRow {
                    kind: meta.kind,
                    key: meta.key,
                    title: meta.title,
                    artist: meta.artist,
                    album: meta.album,
                    artwork: None,
                };
                let name = item
                    .files
                    .first()
                    .map_or("cover", |file| file.name.as_str());
                app.state::<Arc<crate::details::DetailService>>()
                    .add_received(row, name, one()?)
                    .await
            }
            other => Err(format!("This Raygun can't keep {other} items yet")),
        }
    }
}

impl FileSink for AppSink {
    fn staging(&self) -> PathBuf {
        self.staging.clone()
    }

    fn wanted<'a>(&'a self, from: &'a Peer, items: &'a [FileItem]) -> BoxFuture<'a, Vec<bool>> {
        self.names
            .lock()
            .expect("names poisoned")
            .insert(from.device_id.clone(), from.name.clone());
        Box::pin(async move {
            let mut wanted = Vec::with_capacity(items.len());
            for item in items {
                wanted.push(self.wants(item).await);
            }
            wanted
        })
    }

    fn store<'a>(
        &'a self,
        from: &'a Peer,
        item: &'a FileItem,
        files: &'a [PathBuf],
    ) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async move {
            let result = self.keep(item, files).await;
            match &result {
                Ok(()) => tracing::info!(from = %from.name, item = %item.name, "kept a file"),
                Err(error) => {
                    tracing::warn!(from = %from.name, item = %item.name, %error, "a file wasn't kept")
                }
            }
            if item.files.len() == files.len() {
                // What the stores didn't take goes with the staging folder.
                for file in files {
                    let _ = std::fs::remove_file(file);
                }
            }
            result
        })
    }

    fn progress(&self, progress: &Progress) {
        if let Some(service) = self.app.try_state::<Arc<FileSyncService>>() {
            let name = self
                .names
                .lock()
                .expect("names poisoned")
                .get(&progress.device_id)
                .cloned()
                .unwrap_or_else(|| "another device".into());
            service.moved(&name, progress);
        }
    }
}

// ---- Commands ----

/// Each linked device's file choices, and the transfer going on.
#[tauri::command]
#[specta::specta]
pub async fn device_files_status(
    files: tauri::State<'_, Arc<FileSyncService>>,
) -> Result<DeviceFilesStatus, String> {
    Ok(files.status().await)
}

/// Sets what goes to a linked device.
#[tauri::command]
#[specta::specta]
pub async fn device_files_set_prefs(
    files: tauri::State<'_, Arc<FileSyncService>>,
    device_id: String,
    prefs: DeviceFilesPrefs,
) -> Result<DeviceFilesStatus, String> {
    files.inner().set_prefs(&device_id, prefs).await
}

/// Sends what's waiting to a linked device now.
#[tauri::command]
#[specta::specta]
pub async fn device_files_send_now(
    files: tauri::State<'_, Arc<FileSyncService>>,
    device_id: String,
) -> Result<DeviceFilesStatus, String> {
    files.inner().send_now(&device_id).await
}

/// Stops the transfer going on.
#[tauri::command]
#[specta::specta]
pub async fn device_files_cancel(
    files: tauri::State<'_, Arc<FileSyncService>>,
) -> Result<DeviceFilesStatus, String> {
    Ok(files.cancel().await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn song_edits_are_rows_and_the_rest_files() {
        assert!(!is_files(&DeviceFileKind::SongEdits));
        assert!(is_files(&DeviceFileKind::Songs));
        assert_eq!(default_prefs().kinds, [DeviceFileKind::SongEdits]);
        assert!(default_prefs().wifi_only);
    }

    #[test]
    fn a_round_says_what_didnt_go() {
        assert_eq!(summary(&SendReport::default()), None);
        let one = SendReport {
            failed: vec![("song:a".into(), "it changed".into())],
            ..SendReport::default()
        };
        assert_eq!(
            summary(&one).as_deref(),
            Some("1 item didn't go: it changed")
        );
        let stopped = SendReport {
            cancelled: true,
            ..SendReport::default()
        };
        assert!(summary(&stopped).unwrap().starts_with("Stopped"));
    }

    #[test]
    fn notices_name_the_device_and_the_item() {
        let notice = notice_for(&DeviceFilesTransfer {
            device_id: "d".into(),
            device_name: "Laptop".into(),
            direction: DeviceFilesDirection::Sending,
            item_name: "Swim · BTS".into(),
            item_kind: SONG.into(),
            index: 2,
            count: 5,
            done_bytes: 0.0,
            total_bytes: 1.0,
        });
        assert_eq!(notice.title, "Sending to Laptop");
        assert_eq!(notice.text, "Swim · BTS (2 of 5)");
        assert!(!notice.channel.alerts);
    }

    #[test]
    fn items_carry_what_the_other_device_needs() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("track.m4a");
        std::fs::write(&path, b"abcd").unwrap();
        let row = raygun_db::music_downloads::SongDownloadRow {
            id: "1".into(),
            song: raygun_db::music_downloads::NewSong {
                track_key: "bts|swim".into(),
                title: "Swim".into(),
                artist: Some("BTS".into()),
                album: None,
                duration_ms: Some(1),
                artwork_url: None,
                source: None,
            },
            state: "done".into(),
            kind: Some("file".into()),
            provider: None,
            file: Some("track.m4a".into()),
            done_bytes: 4,
            total_bytes: Some(4),
            error: None,
            error_detail: None,
            created_at: 0,
            queued_at: 0,
            finished_at: None,
            origin: raygun_db::music_downloads::Origin::default(),
            file_ms: None,
            replacing: None,
            problem: None,
        };
        let out = song_item(&row, path).unwrap();
        assert_eq!(out.item.id, "song:bts|swim:4");
        assert_eq!(out.item.files[0].size, 4);
        let meta: SongMeta = serde_json::from_value(out.item.meta).unwrap();
        assert_eq!(meta.track_key, "bts|swim");
        assert_eq!(meta.file, "track.m4a");
    }
}
