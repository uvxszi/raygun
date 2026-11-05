//! Local music library (M07): incremental scans with progress events, browsing queries and
//! turning a selection into a play queue.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use raygun_db::local_music::{self, AlbumRow, ArtistRow, TrackRecord, TrackRow};
use raygun_library::{FileEntry, ScannedTrack};
use raygun_model::{
    LibraryAlbum, LibraryArtist, LibraryScanProgress, LibraryStatus, LibraryTrack, MediaKind,
    MediaSource, PlayRequest, PlaySelection, QueueItemInput, ScanPhase, TrackQuery,
};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::events::LibraryScanEvent;
use crate::playback::PlaybackService;
use crate::startup::AppDb;

/// Files read (and progress reported) per batch.
const CHUNK_SIZE: usize = 200;
/// Threads reading tags within a batch.
const READ_THREADS: usize = 4;

pub struct LibraryService {
    app: AppHandle,
    art_dir: PathBuf,
    proxy_address: SocketAddr,
    scanning: AtomicBool,
}

impl LibraryService {
    pub fn new(app: AppHandle, art_dir: PathBuf, proxy_address: SocketAddr) -> Self {
        Self {
            app,
            art_dir,
            proxy_address,
            scanning: AtomicBool::new(false),
        }
    }

    fn db(&self) -> &raygun_db::Db {
        &self.app.state::<AppDb>().inner().0
    }

    pub async fn status(&self) -> Result<LibraryStatus, String> {
        let db = self.db();
        Ok(LibraryStatus {
            roots: local_music::roots(db).await.map_err(to_message)?,
            track_count: local_music::track_count(db).await.map_err(to_message)? as u32,
            scanning: self.scanning.load(Ordering::SeqCst),
        })
    }

    pub async fn add_root(&self, path: &str) -> Result<LibraryStatus, String> {
        if !Path::new(path).is_dir() {
            return Err(format!("{path} is not a folder"));
        }
        local_music::add_root(self.db(), path)
            .await
            .map_err(to_message)?;
        self.start_scan();
        self.status().await
    }

    pub async fn remove_root(&self, path: &str) -> Result<LibraryStatus, String> {
        local_music::remove_root(self.db(), path)
            .await
            .map_err(to_message)?;
        self.start_scan();
        self.status().await
    }

    /// Adds the device's music folders (asking for permission on Android) and scans them.
    pub async fn connect_device_music(&self) -> Result<LibraryStatus, String> {
        let folders = device_music_folders(&self.app).await?;
        for folder in folders.iter().filter(|folder| Path::new(folder).is_dir()) {
            local_music::add_root(self.db(), folder)
                .await
                .map_err(to_message)?;
        }
        self.start_scan();
        self.status().await
    }

    /// Starts a background scan unless one is already running. Progress arrives as
    /// `LibraryScanEvent`s.
    pub fn start_scan(&self) {
        if self.scanning.swap(true, Ordering::SeqCst) {
            return;
        }
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let library = app.state::<LibraryService>();
            if let Err(message) = library.run_scan().await {
                tracing::warn!(%message, "library scan failed");
                library.emit(LibraryScanProgress {
                    phase: ScanPhase::Failed,
                    found: 0,
                    to_read: 0,
                    read: 0,
                    removed: 0,
                    failed: 0,
                    message: Some(message),
                });
            }
            library.scanning.store(false, Ordering::SeqCst);
        });
    }

    async fn run_scan(&self) -> Result<(), String> {
        let started = std::time::Instant::now();
        let db = self.db();
        self.fill_track_keys().await?;
        let roots: Vec<PathBuf> = local_music::roots(db)
            .await
            .map_err(to_message)?
            .into_iter()
            .map(PathBuf::from)
            .collect();
        let mut progress = LibraryScanProgress {
            phase: ScanPhase::Discovering,
            found: 0,
            to_read: 0,
            read: 0,
            removed: 0,
            failed: 0,
            message: None,
        };
        self.emit(progress.clone());

        let files = tauri::async_runtime::spawn_blocking(move || raygun_library::discover(&roots))
            .await
            .map_err(to_message)?;
        let known = local_music::known_files(db).await.map_err(to_message)?;
        let seen: HashSet<String> = files.iter().map(|file| path_string(&file.path)).collect();
        let changed: Vec<FileEntry> = files
            .into_iter()
            .filter(|file| {
                known.get(&path_string(&file.path)) != Some(&(file.size as i64, file.modified))
            })
            .collect();
        let removed: Vec<String> = known
            .into_keys()
            .filter(|path| !seen.contains(path))
            .collect();

        progress.phase = ScanPhase::Reading;
        progress.found = seen.len() as u32;
        progress.to_read = changed.len() as u32;
        self.emit(progress.clone());

        for chunk in changed.chunks(CHUNK_SIZE) {
            let chunk = chunk.to_vec();
            let count = chunk.len() as u32;
            let art_dir = self.art_dir.clone();
            let (records, failed) =
                tauri::async_runtime::spawn_blocking(move || read_files(chunk, &art_dir))
                    .await
                    .map_err(to_message)?;
            local_music::upsert_tracks(db, &records)
                .await
                .map_err(to_message)?;
            progress.read += count;
            progress.failed += failed;
            self.emit(progress.clone());
        }

        local_music::delete_paths(db, &removed)
            .await
            .map_err(to_message)?;
        progress.removed = removed.len() as u32;
        progress.phase = ScanPhase::Done;
        tracing::info!(
            found = progress.found,
            read = progress.read,
            removed = progress.removed,
            failed = progress.failed,
            elapsed_ms = started.elapsed().as_millis() as u64,
            "library scan finished"
        );
        self.emit(progress);
        Ok(())
    }

    pub async fn albums(&self, artist: Option<&str>) -> Result<Vec<LibraryAlbum>, String> {
        let rows = local_music::albums(self.db(), artist)
            .await
            .map_err(to_message)?;
        Ok(rows.into_iter().map(|row| self.album(row)).collect())
    }

    pub async fn artists(&self) -> Result<Vec<LibraryArtist>, String> {
        let rows = local_music::artists(self.db()).await.map_err(to_message)?;
        Ok(rows.into_iter().map(|row| self.artist(row)).collect())
    }

    pub async fn album_tracks(
        &self,
        artist: &str,
        album: &str,
    ) -> Result<Vec<LibraryTrack>, String> {
        let rows = local_music::album_tracks(self.db(), artist, album)
            .await
            .map_err(to_message)?;
        Ok(rows.into_iter().map(|row| self.track(row)).collect())
    }

    pub async fn tracks(&self, offset: u32, limit: u32) -> Result<Vec<LibraryTrack>, String> {
        let rows = local_music::tracks(self.db(), i64::from(offset), i64::from(limit))
            .await
            .map_err(to_message)?;
        Ok(rows.into_iter().map(|row| self.track(row)).collect())
    }

    /// Replaces the play queue with the selection and starts playing.
    pub async fn play(&self, request: PlayRequest) -> Result<(), String> {
        let db = self.db();
        let rows = match &request.selection {
            PlaySelection::Album { artist, album } => {
                local_music::album_tracks(db, artist, album).await
            }
            PlaySelection::Artist { artist } => local_music::artist_tracks(db, artist).await,
            PlaySelection::AllTracks => local_music::tracks(db, 0, i64::MAX).await,
        }
        .map_err(to_message)?;
        let start = request
            .start_track_id
            .as_ref()
            .and_then(|id| rows.iter().position(|row| &row.id == id));
        let items = rows.into_iter().map(|row| self.queue_item(row)).collect();
        self.app
            .state::<PlaybackService>()
            .replace(items, start, request.shuffle);
        Ok(())
    }

    fn emit(&self, progress: LibraryScanProgress) {
        if let Err(error) = LibraryScanEvent(progress).emit(&self.app) {
            tracing::warn!(%error, "failed to emit scan progress");
        }
    }

    /// Local tracks matching every word of `query`, for search (M18).
    pub async fn search(&self, query: &str, limit: u32) -> Result<Vec<LibraryTrack>, String> {
        let words: Vec<String> = query.split_whitespace().map(str::to_owned).collect();
        let rows = local_music::search(self.db(), &words, limit.into())
            .await
            .map_err(to_message)?;
        Ok(rows.into_iter().map(|row| self.track(row)).collect())
    }

    /// Local tracks by track key, so saved tracks find the same song on this device (M18).
    pub async fn matches(&self, keys: &[String]) -> Result<HashMap<String, TrackRow>, String> {
        local_music::tracks_by_keys(self.db(), keys)
            .await
            .map_err(to_message)
    }

    /// Gives tracks stored before M18 their key. A no-op once every track has one.
    async fn fill_track_keys(&self) -> Result<(), String> {
        let keyless = local_music::keyless_tracks(self.db())
            .await
            .map_err(to_message)?;
        if keyless.is_empty() {
            return Ok(());
        }
        let keys: Vec<(String, String)> = keyless
            .into_iter()
            .map(|(id, title, artist)| (id, local_track_key(&title, artist.as_deref())))
            .collect();
        tracing::info!(count = keys.len(), "adding track keys to library tracks");
        local_music::set_track_keys(self.db(), &keys)
            .await
            .map_err(to_message)
    }

    pub(crate) fn artwork_url(&self, art: Option<String>) -> Option<String> {
        art.map(|name| raygun_net::artwork_url(self.proxy_address, &name))
    }

    fn album(&self, row: AlbumRow) -> LibraryAlbum {
        LibraryAlbum {
            artist: row.artist,
            title: row.album,
            year: row.year.map(|year| year as u32),
            track_count: row.track_count as u32,
            artwork_url: self.artwork_url(row.art),
        }
    }

    fn artist(&self, row: ArtistRow) -> LibraryArtist {
        LibraryArtist {
            name: row.name,
            album_count: row.album_count as u32,
            track_count: row.track_count as u32,
            artwork_url: self.artwork_url(row.art),
        }
    }

    pub(crate) fn track(&self, row: TrackRow) -> LibraryTrack {
        let mut track = LibraryTrack {
            id: row.id,
            title: row.title,
            artist: row.artist.unwrap_or_else(|| row.group_artist.clone()),
            album_artist: row.group_artist,
            album: row.group_album,
            track_no: row.track_no.map(|number| number as u32),
            disc_no: row.disc_no.map(|number| number as u32),
            year: row.year.map(|year| year as u32),
            duration_ms: row.duration_ms.map(|duration| duration as u32),
            artwork_url: self.artwork_url(row.art),
        };
        // The user's own details (M46.30), by the song's key.
        if let Some(details) = self
            .app
            .try_state::<std::sync::Arc<crate::details::DetailService>>()
        {
            let key = raygun_resolver::track_key(&raygun_model::TrackQuery {
                title: track.title.clone(),
                artist: Some(track.artist.clone()),
                album: Some(track.album.clone()),
                duration_ms: None,
            });
            let mut artist = Some(track.artist.clone());
            let mut album = Some(track.album.clone());
            details.apply(
                &key,
                &mut track.title,
                &mut artist,
                &mut album,
                &mut track.artwork_url,
            );
            track.artist = artist.unwrap_or_default();
            track.album = album.unwrap_or_default();
        }
        track
    }

    /// A song file outside the library (one another app opened with Raygun) as a queue item,
    /// named by its tags; `None` when they can't be read.
    pub(crate) async fn file_item(&self, path: PathBuf) -> Option<QueueItemInput> {
        let art_dir = self.art_dir.clone();
        let read = path.clone();
        let track = tauri::async_runtime::spawn_blocking(move || {
            let size = std::fs::metadata(&read).ok()?.len();
            let file = FileEntry {
                path: read,
                size,
                modified: 0,
            };
            raygun_library::read_track(file, &art_dir).ok()
        })
        .await
        .ok()
        .flatten()?;
        Some(QueueItemInput {
            explicit: false,
            title: track.title,
            artist: track.artist.or(track.album_artist),
            album: track.album,
            duration_ms: track
                .duration_ms
                .and_then(|duration| u32::try_from(duration).ok()),
            artwork_url: self.artwork_url(track.art),
            media_kind: MediaKind::Audio,
            source: MediaSource::LocalFile {
                path: path.to_string_lossy().into_owned(),
            },
        })
    }

    pub(crate) fn queue_item(&self, row: TrackRow) -> QueueItemInput {
        QueueItemInput {
            explicit: false,
            title: row.title,
            artist: Some(row.artist.unwrap_or(row.group_artist)),
            album: Some(row.group_album),
            duration_ms: row.duration_ms.map(|duration| duration as u32),
            artwork_url: self.artwork_url(row.art),
            media_kind: MediaKind::Audio,
            source: MediaSource::LocalFile { path: row.path },
        }
    }
}

/// Reads one batch of files on a few threads. Returns the records and how many files failed.
fn read_files(files: Vec<FileEntry>, art_dir: &Path) -> (Vec<TrackRecord>, u32) {
    let per_thread = files.len().div_ceil(READ_THREADS).max(1);
    let results: Vec<Result<ScannedTrack, String>> = std::thread::scope(|scope| {
        let workers: Vec<_> = files
            .chunks(per_thread)
            .map(|part| {
                scope.spawn(move || {
                    part.iter()
                        .map(|file| raygun_library::read_track(file.clone(), art_dir))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().unwrap_or_default())
            .collect()
    });

    let mut failed = 0;
    let records = results
        .into_iter()
        .filter_map(|result| match result {
            Ok(track) => Some(record(track)),
            Err(error) => {
                tracing::debug!(%error, "skipping unreadable file");
                failed += 1;
                None
            }
        })
        .collect();
    (records, failed)
}

/// The resolver's key for a local track, from its own artist (else the album artist).
pub(crate) fn local_track_key(title: &str, artist: Option<&str>) -> String {
    raygun_resolver::track_key(&TrackQuery {
        title: title.to_owned(),
        artist: artist.map(str::to_owned),
        album: None,
        duration_ms: None,
    })
}

fn record(track: ScannedTrack) -> TrackRecord {
    TrackRecord {
        track_key: local_track_key(
            &track.title,
            track.artist.as_deref().or(track.album_artist.as_deref()),
        ),
        group_artist: track.group_artist().to_owned(),
        group_album: track.group_album().to_owned(),
        path: path_string(&track.file.path),
        file_size: track.file.size as i64,
        file_mtime: track.file.modified,
        title: track.title,
        artist: track.artist,
        album_artist: track.album_artist,
        album: track.album,
        track_no: track.track_no.map(i64::from),
        disc_no: track.disc_no.map(i64::from),
        year: track.year.map(i64::from),
        genre: track.genre,
        duration_ms: track.duration_ms.map(|duration| duration as i64),
        art: track.art,
    }
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn to_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(target_os = "android")]
async fn device_music_folders(app: &AppHandle) -> Result<Vec<String>, String> {
    use tauri_plugin_raygun_audio::NativeAudio;

    let app = app.clone();
    let access = tauri::async_runtime::spawn_blocking(move || {
        app.state::<NativeAudio<tauri::Wry>>()
            .request_library_access()
    })
    .await
    .map_err(to_message)??;
    if !access.granted {
        return Err("Raygun needs permission to read your music.".into());
    }
    Ok(access.folders)
}

#[cfg(not(target_os = "android"))]
async fn device_music_folders(_app: &AppHandle) -> Result<Vec<String>, String> {
    Err("Add a music folder by path on this platform.".into())
}
