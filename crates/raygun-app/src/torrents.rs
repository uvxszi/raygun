//! Torrents in the video player (`raygun_torrent`): videos sources give as torrents (Stremio
//! addons' `infoHash` streams, video sources' magnet links and `.torrent` files, magnet links
//! opened with Open link) play like any file.
//!
//! [`TorrentService::address_for`] opens the torrent when the player is about to load it (a
//! `.torrent` file is fetched with the app's client, so Offline Mode and the local network's
//! permission apply), picks its file, and gives the stream proxy's `/torrent/<token>/<name>`
//! address, which players read with byte ranges as the pieces arrive. The subtitle files that
//! come with the video ("Sintel.en.srt") are subtitle tracks of their own, at addresses of the
//! same kind; their pieces are fetched first so the player has them as it starts. A link played
//! on its own keeps what its torrent holds and the poster that came with it (`video_links`).
//! While it plays, how it's doing goes to the player once a second (`transfer` updates: peers,
//! seeds, speed). The file closes when the player closes or loads something else; its pieces are
//! deleted unless sharing on is turned on (never in battery saver).
//!
//! `Settings::torrents` decides: off until turned on (Settings → Video player → Torrents explains
//! first), only on Wi-Fi by default, the cache's size, sharing on, the port and the DHT. Offline
//! Mode refuses torrents and stops the engine; Data Saver is warned about in the player (a
//! torrent's video comes whole).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use raygun_model::{StorageArea, StorageUsage, TorrentPrefs, VideoRequest, VideoSubtitle};
use raygun_torrent::{
    Engine, EngineConfig, Listing, OpenedFile, OpenedSubtitle, Source, TorrentError, TorrentLink,
};
use serde_json::json;
use tauri::{AppHandle, Manager};
use tokio::io::AsyncReadExt;

use crate::startup::AppDb;

/// How long a torrent's subtitle file is read in the background before it's left to the player.
const SUBTITLE_WARM: Duration = Duration::from_secs(180);
/// How long a torrent's poster may take to arrive.
const POSTER_WAIT: Duration = Duration::from_secs(180);

/// A torrent's video as the player opens it: its address, and the subtitle files that came with
/// it as tracks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TorrentStream {
    pub address: String,
    pub subtitles: Vec<VideoSubtitle>,
}

/// What the engine uploads at most while a video plays: enough for peers to trade with.
const UPLOAD_LIMIT: u32 = 256 * 1024;
/// How often the player hears how the torrent is doing.
const TRANSFER_EVERY: Duration = Duration::from_secs(1);
/// The largest `.torrent` file read.
const MAX_TORRENT_FILE: u64 = 16 * 1024 * 1024;
/// How long a `.torrent` file may take to arrive.
const TORRENT_FILE_TIMEOUT: Duration = Duration::from_secs(30);
/// The engine stops this long after nothing needs it (another torrent may open soon: its DHT
/// and peers are kept that long).
const IDLE_STOP: Duration = Duration::from_secs(60);
/// How often torrents shared on look at mobile data and battery saver.
const SHARING_CHECK: Duration = Duration::from_secs(60);

/// Why a torrent doesn't play now.
pub const TORRENTS_OFF: &str =
    "Torrents are off: turn them on in Settings → Video player → Torrents";
pub const WIFI_ONLY: &str = "Torrents play only on Wi-Fi (Settings → Video player → Torrents)";

/// The torrent file the player has open.
struct Playing {
    token: String,
    /// The video's progress key and link.
    key: String,
    url: String,
    address: String,
    subtitles: Vec<VideoSubtitle>,
}

pub struct TorrentService {
    app: AppHandle,
    engine: Arc<Engine>,
    links: raygun_net::TorrentLinks,
    client: reqwest::Client,
    dir: PathBuf,
    playing: Mutex<Option<Playing>>,
    /// Bumped by every open and close: an open that finishes after a newer one (or after the
    /// player closed) closes what it opened.
    generation: AtomicU64,
    ticker: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

/// What the stream proxy reads `/torrent/` from.
struct Files(Arc<Engine>);

impl raygun_net::ArrivingFiles for Files {
    fn file_len(&self, token: &str) -> Option<u64> {
        self.0.file_len(token)
    }

    fn read_from(
        &self,
        token: &str,
        start: u64,
    ) -> futures_util::future::BoxFuture<'static, std::io::Result<raygun_net::FileReader>> {
        let engine = self.0.clone();
        let token = token.to_owned();
        Box::pin(async move { engine.read_from(&token, start).await })
    }
}

fn engine_config(dir: &Path, prefs: &TorrentPrefs) -> EngineConfig {
    EngineConfig {
        dir: dir.to_path_buf(),
        port: prefs.port,
        dht: prefs.dht,
        upload_limit: Some(UPLOAD_LIMIT),
        // Android's checks of certificates need a part of the platform the engine's own client
        // doesn't have: its HTTPS trackers are left out there (UDP and HTTP ones stay).
        https_trackers: !cfg!(target_os = "android"),
    }
}

/// A torrent error as the player or a download shows it.
fn message(error: TorrentError) -> String {
    match error {
        TorrentError::TooLarge { .. } => {
            format!("{error}: a larger cache is in Settings → Video player → Torrents")
        }
        other => other.to_string(),
    }
}

impl TorrentService {
    pub fn new(
        app: AppHandle,
        dir: PathBuf,
        links: raygun_net::TorrentLinks,
        client: reqwest::Client,
        prefs: &TorrentPrefs,
    ) -> Arc<Self> {
        let engine = Arc::new(Engine::new(engine_config(&dir, prefs)));
        links.serve(Arc::new(Files(engine.clone())));
        Arc::new(Self {
            app,
            engine,
            links,
            client,
            dir,
            playing: Mutex::new(None),
            generation: AtomicU64::new(0),
            ticker: Mutex::new(None),
        })
    }

    /// Empties what an earlier run left (nothing is resumed), stops everything whenever Offline
    /// Mode turns on, and stops sharing on mobile data (when torrents are for Wi-Fi only) or in
    /// battery saver.
    pub fn start(self: &Arc<Self>) {
        let dir = self.dir.clone();
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            let _ = tokio::fs::remove_dir_all(&dir).await;
            let mut network = raygun_net::network_mode::subscribe();
            while network.changed().await.is_ok() {
                let offline = network.borrow_and_update().offline;
                if offline && service.engine.busy() {
                    tracing::info!("Offline Mode: torrents stop");
                    service.stop_everything().await;
                }
            }
        });
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(SHARING_CHECK).await;
                if !service.engine.sharing() {
                    continue;
                }
                let prefs = service.prefs().await;
                let saving = crate::device_state::current().power_save;
                if saving || (prefs.wifi_only && crate::network::on_mobile_data(&service.app)) {
                    tracing::info!("torrents stop sharing (battery saver or mobile data)");
                    service.engine.stop_sharing().await;
                    service.stop_when_idle();
                }
            }
        });
    }

    /// Stops the engine a while after nothing needs it.
    fn stop_when_idle(&self) {
        let engine = self.engine.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(IDLE_STOP).await;
            engine.stop_if_idle().await;
        });
    }

    async fn prefs(&self) -> TorrentPrefs {
        let db = self.app.state::<AppDb>().0.clone();
        crate::settings::load(&db)
            .await
            .map(|settings| settings.torrents)
            .unwrap_or_default()
    }

    /// Why torrents can't play now, if they can't: turned off, Offline Mode, mobile data.
    pub async fn unavailable(&self) -> Option<String> {
        unavailable(&self.app, &self.prefs().await)
    }

    /// What the player opens for a torrent link: the torrent's file through the stream proxy,
    /// with the subtitle files that came with it. The torrent opens now (its details asked of
    /// peers for a magnet link); the file playing before closes.
    pub async fn address_for(&self, request: &VideoRequest) -> Result<TorrentStream, String> {
        let prefs = self.prefs().await;
        if let Some(reason) = unavailable(&self.app, &prefs) {
            return Err(reason);
        }
        // The same video again (a retry, its link picked again): the open file goes on.
        if let Some(playing) = self.playing.lock().expect("playing poisoned").as_ref()
            && playing.key == request.key
            && playing.url == request.url
            && self.engine.file_len(&playing.token).is_some()
        {
            return Ok(TorrentStream {
                address: playing.address.clone(),
                subtitles: playing.subtitles.clone(),
            });
        }
        self.release().await;
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let link = raygun_torrent::parse(&request.url).map_err(|error| error.to_string())?;
        let source = self.source_of(&link).await?;
        let opened = match self
            .engine
            .open(source, link.hint(), prefs.cache_bytes())
            .await
        {
            Ok(opened) => opened,
            Err(error) => {
                self.stop_when_idle();
                return Err(message(error));
            }
        };
        if self.generation.load(Ordering::SeqCst) != generation {
            // Closed or replaced while it opened.
            self.engine.close(&opened.token, false).await;
            self.stop_when_idle();
            return Err("The player moved on before the torrent opened".into());
        }
        let address = self.links.link(&opened.token, &opened.name);
        let subtitles = subtitle_tracks(&self.links, &opened.subtitles);
        self.warm_subtitles(&opened.subtitles);
        // A link played on its own keeps what its torrent holds, and its poster.
        tauri::async_runtime::spawn(crate::video_links::torrent_opened(
            self.app.clone(),
            request.key.clone(),
            opened.listing.clone(),
        ));
        if let Some(poster) = opened.poster.clone() {
            self.fetch_poster(request.key.clone(), opened.info_hash.clone(), poster);
        }
        *self.playing.lock().expect("playing poisoned") = Some(Playing {
            token: opened.token,
            key: request.key.clone(),
            url: request.url.clone(),
            address: address.clone(),
            subtitles: subtitles.clone(),
        });
        self.follow();
        Ok(TorrentStream { address, subtitles })
    }

    /// Reads the subtitle files through once, in the background: their pieces are fetched
    /// before the rest (a reader's are), so the player finds them as it starts, wherever they sit
    /// in the torrent. A file that closes or doesn't come in time is left to the player's own
    /// read, which gets an empty track then (`/subtitle/`).
    fn warm_subtitles(&self, subtitles: &[OpenedSubtitle]) {
        for subtitle in subtitles {
            let engine = self.engine.clone();
            let token = subtitle.token.clone();
            tauri::async_runtime::spawn(async move {
                let read = async {
                    let mut reader = engine.read_from(&token, 0).await?;
                    tokio::io::copy(&mut reader, &mut tokio::io::sink()).await
                };
                match tokio::time::timeout(SUBTITLE_WARM, read).await {
                    Ok(Ok(bytes)) => tracing::debug!(bytes, "torrent subtitle file here"),
                    Ok(Err(error)) => tracing::debug!(%error, "torrent subtitle file not read"),
                    Err(_) => tracing::debug!("torrent subtitle file still coming"),
                }
            });
        }
    }

    /// Reads the poster that came with a link's torrent ("poster.jpg") and keeps it for the link
    /// (`video_links`), in the background; one kept already isn't read again.
    fn fetch_poster(&self, key: String, info_hash: String, poster: OpenedFile) {
        if !crate::video_links::is_link_key(&key) {
            return;
        }
        let engine = self.engine.clone();
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            if crate::video_links::has_poster(&app, &key).await {
                return;
            }
            let read = async {
                let reader = engine.read_from(&poster.token, 0).await?;
                let mut bytes = Vec::new();
                reader
                    .take(raygun_torrent::MAX_POSTER_BYTES)
                    .read_to_end(&mut bytes)
                    .await?;
                Ok::<_, std::io::Error>(bytes)
            };
            match tokio::time::timeout(POSTER_WAIT, read).await {
                Ok(Ok(bytes)) if bytes.len() as u64 == poster.len => {
                    crate::video_links::torrent_poster(&app, &key, &info_hash, &poster.name, bytes)
                        .await;
                }
                Ok(Ok(_)) => tracing::debug!("a torrent's poster came short"),
                Ok(Err(error)) => tracing::debug!(%error, "a torrent's poster wasn't read"),
                Err(_) => tracing::debug!("a torrent's poster is still coming"),
            }
        });
    }

    /// What a torrent link holds (its name and files), asked of peers for a magnet link while
    /// torrents may play; nothing is fetched. Opening it next needs no peers for its details.
    pub async fn list(&self, url: &str) -> Result<Listing, String> {
        let prefs = self.prefs().await;
        if let Some(reason) = unavailable(&self.app, &prefs) {
            return Err(reason);
        }
        let link = raygun_torrent::parse(url).map_err(|error| error.to_string())?;
        let source = self.source_of(&link).await?;
        let listed = self.engine.list(source).await.map_err(message);
        self.stop_when_idle();
        listed
    }

    /// The torrent to open: a magnet link as it is, a `.torrent` file fetched.
    async fn source_of(&self, link: &TorrentLink) -> Result<Source, String> {
        match link {
            TorrentLink::Magnet(magnet) => Ok(Source::Magnet(magnet.clone())),
            TorrentLink::File { url, .. } => {
                self.app
                    .state::<crate::local_network::LocalNetwork>()
                    .check(url, crate::local_network::VIDEOS_DENIED)
                    .await?;
                fetch_torrent_file(&self.client, url)
                    .await
                    .map(Source::File)
            }
        }
    }

    /// The player closed: the file playing closes.
    pub async fn player_closed(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.release().await;
        self.stop_when_idle();
    }

    /// The player loads something that isn't a torrent: the file playing closes, and one still
    /// opening is closed once it has.
    pub async fn other_video(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        if self.playing.lock().expect("playing poisoned").is_some() {
            self.release().await;
            self.stop_when_idle();
        }
    }

    /// Closes the file playing, its pieces shared on when the settings say so.
    async fn release(&self) {
        let playing = self.playing.lock().expect("playing poisoned").take();
        if let Some(ticker) = self.ticker.lock().expect("ticker poisoned").take() {
            ticker.abort();
        }
        if let Some(playing) = playing {
            let prefs = self.prefs().await;
            let share = prefs.seed && !crate::device_state::current().power_save;
            self.engine.close(&playing.token, share).await;
        }
    }

    /// Everything stops: what plays, what's shared on, the engine.
    async fn stop_everything(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.playing.lock().expect("playing poisoned").take();
        if let Some(ticker) = self.ticker.lock().expect("ticker poisoned").take() {
            ticker.abort();
        }
        self.engine.shut_down().await;
    }

    /// Tells the player how the torrent playing is doing, once a second, until it closes; stops
    /// it when the device moves to mobile data while torrents are for Wi-Fi only.
    fn follow(&self) {
        let app = self.app.clone();
        let engine = self.engine.clone();
        let task = tauri::async_runtime::spawn(async move {
            let mut ticks: u32 = 0;
            loop {
                let Some(service) = app.try_state::<Arc<TorrentService>>() else {
                    return;
                };
                let current = service
                    .playing
                    .lock()
                    .expect("playing poisoned")
                    .as_ref()
                    .map(|playing| (playing.token.clone(), playing.key.clone()));
                let Some((token, key)) = current else {
                    return;
                };
                let Some(progress) = engine.progress(&token) else {
                    return;
                };
                let _ = crate::video::update_player(
                    &app,
                    &json!({
                        "type": "transfer",
                        "id": key,
                        "peers": progress.peers,
                        "seeds": progress.seeds,
                        "known": progress.known,
                        "downloadBps": progress.download,
                        "uploadBps": progress.upload,
                        "have": progress.have,
                        "len": progress.len,
                        "dataSaver": raygun_net::network_mode::data_saver(),
                    }),
                );
                // Mobile data is asked about now and then (Android answers through the plugin).
                ticks = ticks.wrapping_add(1);
                if ticks.is_multiple_of(10) {
                    let prefs = service.prefs().await;
                    if prefs.wifi_only && crate::network::on_mobile_data(&app) {
                        tracing::info!("on mobile data: the torrent stops");
                        service.generation.fetch_add(1, Ordering::SeqCst);
                        let playing = service.playing.lock().expect("playing poisoned").take();
                        if let Some(playing) = playing {
                            engine.close(&playing.token, false).await;
                        }
                        service.stop_when_idle();
                        let _ = crate::video::update_player(
                            &app,
                            &json!({ "type": "error", "text": WIFI_ONLY, "showSources": true }),
                        );
                        return;
                    }
                }
                tokio::time::sleep(TRANSFER_EVERY).await;
            }
        });
        if let Some(old) = self.ticker.lock().expect("ticker poisoned").replace(task) {
            old.abort();
        }
    }

    /// New settings: the engine's apply when it next starts; turned off, everything stops; not
    /// sharing on any more, what's shared stops.
    pub async fn settings_changed(&self, prefs: &TorrentPrefs) {
        self.engine.configure(engine_config(&self.dir, prefs));
        if !prefs.enabled {
            self.stop_everything().await;
        } else if !prefs.seed {
            self.engine.stop_sharing().await;
            self.stop_when_idle();
        }
    }

    /// The torrent cache for Settings → Data → Storage.
    pub async fn storage_usage(&self, prefs: &TorrentPrefs) -> StorageUsage {
        let (bytes, files) = self.engine.usage().await;
        StorageUsage {
            area: StorageArea::Torrents,
            bytes: bytes as f64,
            files: u32::try_from(files).unwrap_or(u32::MAX),
            limit_bytes: Some(prefs.cache_bytes() as f64),
            clearable: true,
            converted: None,
        }
    }

    /// Clears the torrent cache: torrents shared on stop, and pieces not playing now go.
    pub async fn clear(&self) {
        self.engine.stop_sharing().await;
        self.stop_when_idle();
        let open = self.engine.open_hashes();
        let dir = self.dir.clone();
        let _ = tauri::async_runtime::spawn_blocking(move || {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                return;
            };
            for entry in entries.filter_map(Result::ok) {
                let name = entry.file_name().to_string_lossy().into_owned();
                if !open.contains(&name) {
                    let _ = std::fs::remove_dir_all(entry.path());
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        })
        .await;
    }

    /// Downloads a torrent's video into `folder` (the video downloads queue): the file the link
    /// names or the largest video. `report` hears the bytes here and the size; `stopped` ends it
    /// where it is (the next run continues from the pieces kept).
    pub async fn download(
        &self,
        url: &str,
        folder: &Path,
        report: &(dyn Fn(u64, u64) + Sync),
        stopped: &(dyn Fn() -> bool + Sync),
    ) -> Result<PathBuf, DownloadError> {
        let prefs = self.prefs().await;
        if let Some(reason) = unavailable(&self.app, &prefs) {
            return Err(DownloadError::Failed(reason));
        }
        let link =
            raygun_torrent::parse(url).map_err(|error| DownloadError::Failed(error.to_string()))?;
        let source = self.source_of(&link).await.map_err(DownloadError::Failed)?;
        match self
            .engine
            .download(source, link.hint(), folder, report, stopped)
            .await
        {
            Ok(path) => Ok(path),
            Err(TorrentError::Stopped) => Err(DownloadError::Stopped),
            Err(error) => Err(DownloadError::Failed(message(error))),
        }
    }
}

/// How a torrent download ended without its file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadError {
    /// Paused or removed: the pieces stay for the next run.
    Stopped,
    Failed(String),
}

/// A torrent's subtitle files as the player's subtitle tracks, read through the stream proxy
/// (`prepare` hands them to the player through `/subtitle/`, which never fails).
fn subtitle_tracks(
    links: &raygun_net::TorrentLinks,
    subtitles: &[OpenedSubtitle],
) -> Vec<VideoSubtitle> {
    subtitles
        .iter()
        .map(|subtitle| VideoSubtitle {
            url: links.link(&subtitle.token, &subtitle.name),
            lang: subtitle.language.clone(),
            label: subtitle.label.clone(),
            format: Some(subtitle.format.clone()),
            // Under its name in the players' lists: several in one language tell apart by it.
            source: Some(subtitle.name.clone()),
        })
        .collect()
}

/// Why torrents can't play now under `prefs`, if they can't.
fn unavailable(app: &AppHandle, prefs: &TorrentPrefs) -> Option<String> {
    if !prefs.enabled {
        return Some(TORRENTS_OFF.into());
    }
    // Peers are the network too.
    if let Err(offline) = raygun_net::network_mode::check_online() {
        return Some(offline.to_string());
    }
    if prefs.wifi_only && crate::network::on_mobile_data(app) {
        return Some(WIFI_ONLY.into());
    }
    None
}

/// Why torrents can't play now, for listing torrent links (they're listed, marked as failed).
pub async fn unavailable_now(app: &AppHandle) -> Option<String> {
    match app.try_state::<Arc<TorrentService>>() {
        Some(service) => service.unavailable().await,
        None => Some(TORRENTS_OFF.into()),
    }
}

/// What the torrent cache holds, in bytes: a torrent whose video is known to be larger can't play.
pub async fn cache_bytes(app: &AppHandle) -> u64 {
    let db = app.state::<AppDb>().0.clone();
    crate::settings::load(&db)
        .await
        .map(|settings| settings.torrents)
        .unwrap_or_default()
        .cache_bytes()
}

/// Why a torrent whose video is `size` bytes can't play: larger than the cache holds.
pub fn too_large(size: Option<u64>, cap: u64) -> Option<String> {
    size.filter(|size| *size > cap)
        .map(|size| message(TorrentError::TooLarge { size, cap }))
}

/// A `.torrent` file's bytes, fetched with the app's client.
async fn fetch_torrent_file(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, String> {
    raygun_net::network_mode::check(url).map_err(|offline| offline.to_string())?;
    let response = client
        .get(url)
        .timeout(TORRENT_FILE_TIMEOUT)
        .send()
        .await
        .map_err(|error| format!("The torrent file couldn't be fetched: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "The torrent file couldn't be fetched: the server answered {}",
            response.status()
        ));
    }
    if response
        .content_length()
        .is_some_and(|len| len > MAX_TORRENT_FILE)
    {
        return Err("The torrent file is too large".into());
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("The torrent file couldn't be fetched: {error}"))?;
    if bytes.len() as u64 > MAX_TORRENT_FILE {
        return Err("The torrent file is too large".into());
    }
    Ok(bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_video_too_large_for_the_cache_says_where_to_make_it_larger() {
        let text = message(TorrentError::TooLarge {
            size: 12_000_000_000,
            cap: 4_000_000_000,
        });
        assert_eq!(
            text,
            "This video is 12 GB, more than the torrent cache holds (4.0 GB): a larger cache is \
             in Settings → Video player → Torrents"
        );
        assert_eq!(message(TorrentError::NoVideo), "This torrent has no video");
        // Known sizes rule a torrent out before it's opened; unknown ones don't.
        assert_eq!(too_large(Some(12_000_000_000), 4_000_000_000), Some(text));
        assert_eq!(too_large(Some(1_000_000_000), 4_000_000_000), None);
        assert_eq!(too_large(None, 4_000_000_000), None);
    }

    #[tokio::test]
    async fn subtitle_files_become_tracks_at_the_proxys_torrent_addresses() {
        let proxy = raygun_net::StreamProxy::start(raygun_net::http_client().unwrap(), None)
            .await
            .unwrap();
        let opened = [
            OpenedSubtitle {
                token: "a1".into(),
                name: "Sintel.en.srt".into(),
                format: "srt".into(),
                language: Some("en".into()),
                label: None,
            },
            OpenedSubtitle {
                token: "b2".into(),
                name: "Commentary (forced).ass".into(),
                format: "ass".into(),
                language: None,
                label: Some("Forced".into()),
            },
        ];
        let tracks = subtitle_tracks(&proxy.torrents(), &opened);
        assert_eq!(tracks.len(), 2);
        assert!(
            tracks[0].url.ends_with("/torrent/a1/Sintel.en.srt"),
            "{}",
            tracks[0].url
        );
        assert_eq!(tracks[0].lang.as_deref(), Some("en"));
        assert_eq!(tracks[0].format.as_deref(), Some("srt"));
        // Names stay one path segment; the label says the kind.
        assert!(
            tracks[1]
                .url
                .ends_with("/torrent/b2/Commentary__forced_.ass")
        );
        assert_eq!(tracks[1].label.as_deref(), Some("Forced"));
    }

    #[test]
    fn https_trackers_stay_out_only_on_android() {
        let config = engine_config(Path::new("cache"), &TorrentPrefs::default());
        assert_eq!(config.https_trackers, !cfg!(target_os = "android"));
        assert_eq!(config.port, 0);
        assert!(config.dht);
        assert_eq!(config.upload_limit, Some(UPLOAD_LIMIT));
    }
}
