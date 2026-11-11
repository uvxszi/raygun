//! The torrent engine: one librqbit session, started when a torrent is first opened and stopped
//! once nothing is open, streaming, downloading or sharing.
//!
//! Opening a torrent asks peers for its details (a magnet link's metadata), picks the file to
//! play ([`crate::pick_file`]), and adds the torrent with only that file selected, and the
//! subtitle files and poster that come with it ([`crate::subtitles_for`],
//! [`crate::poster_file`]), each readable by a token of its own. Readers of a file
//! ([`Engine::read_from`]) wait for the pieces they need; librqbit fetches the pieces just ahead of
//! each reader first, so a seek starts fetching where it lands. Closing removes the torrent and its
//! pieces unless they're shared on (seeding). Details peers sent are kept for a while
//! ([`Engine::list`] then opening, or another of its files, needs no peers for them again).

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::SeekFrom;
use std::net::{Ipv6Addr, SocketAddr};
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use librqbit::api::TorrentIdOrHash;
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, DhtSessionConfig, ListenerMode,
    ListenerOptions, ManagedTorrent, Session, SessionOptions,
};
use tokio::io::{AsyncRead, AsyncSeekExt, ReadBuf};

use crate::link::{FileHint, Magnet};
use crate::pick::{TorrentFile, pick_file};
use crate::poster::poster_file;
use crate::subtitles::subtitles_for;

/// How long peers get to send a magnet link's details.
const METADATA_TIMEOUT: Duration = Duration::from_secs(60);
/// Torrents whose details are kept, newest last.
const KNOWN_DETAILS: usize = 8;
/// Readers of one file kept open at once; an older one ends when another opens (players drop
/// their connection on a seek, and a reader waiting for a piece holds a slot of the engine's).
const READERS_PER_FILE: usize = 3;
/// How often a download looks at its progress.
const DOWNLOAD_POLL: Duration = Duration::from_millis(700);
/// A download whose file hasn't grown for this long fails.
const DOWNLOAD_STALL: Duration = Duration::from_secs(10 * 60);
/// Peers connected at most for one torrent.
const PEER_LIMIT: usize = 80;

/// How the engine runs, from the app's settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineConfig {
    /// Where streamed torrents' pieces are kept, a folder per torrent (by info hash).
    pub dir: PathBuf,
    /// The port peers connect to; 0 takes any free one.
    pub port: u16,
    /// Peers found through the DHT as well as trackers.
    pub dht: bool,
    /// What the engine uploads at most, in bytes a second.
    pub upload_limit: Option<u32>,
    /// Trackers over HTTPS are announced to (their certificates can be checked).
    pub https_trackers: bool,
}

/// Why a torrent didn't open or download.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TorrentError {
    #[error("{0}")]
    Link(String),
    #[error("No peer sent this torrent's details in time")]
    NoDetails,
    #[error("This torrent has no video")]
    NoVideo,
    #[error(
        "This video is {}, more than the torrent cache holds ({})",
        gigabytes(*size),
        gigabytes(*cap)
    )]
    TooLarge { size: u64, cap: u64 },
    #[error("The torrent engine: {0}")]
    Engine(String),
    #[error("Stopped")]
    Stopped,
}

/// A size in GB with one decimal ("1.4 GB").
pub fn gigabytes(bytes: u64) -> String {
    let gb = bytes as f64 / 1_000_000_000.0;
    if gb >= 10.0 {
        format!("{gb:.0} GB")
    } else {
        format!("{gb:.1} GB")
    }
}

fn engine_error(error: impl std::fmt::Display) -> TorrentError {
    TorrentError::Engine(format!("{error:#}"))
}

/// A torrent to open: a magnet link, or a `.torrent` file's bytes.
pub enum Source {
    Magnet(Magnet),
    File(Vec<u8>),
}

/// What a torrent holds: its name and files (padding left out).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    pub info_hash: String,
    /// The torrent's own name (its folder, or its one file's name).
    pub name: Option<String>,
    pub files: Vec<TorrentFile>,
}

/// A file opened for playing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opened {
    /// What its readers and progress are asked by.
    pub token: String,
    /// The file's name (without folders).
    pub name: String,
    pub len: u64,
    pub info_hash: String,
    /// The subtitle files that came with it, open while it is.
    pub subtitles: Vec<OpenedSubtitle>,
    /// The poster that came with it ("poster.jpg"), open while it is.
    pub poster: Option<OpenedFile>,
    /// Everything the torrent holds.
    pub listing: Listing,
}

/// A file opened beside a video (its poster), read by its token ([`Engine::read_from`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenedFile {
    pub token: String,
    /// The file's name (without folders).
    pub name: String,
    pub len: u64,
}

/// A subtitle file opened with a video.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenedSubtitle {
    /// What its readers are asked by ([`Engine::read_from`]).
    pub token: String,
    /// The file's name (without folders).
    pub name: String,
    /// `srt`, `vtt`, `ass`, `ssa`, `ttml`.
    pub format: String,
    /// The language its name gives, as written there ("en", "English", "pt-BR").
    pub language: Option<String>,
    /// What else its name says ("Forced", "SDH"), or its name when it names no language.
    pub label: Option<String>,
}

/// How a torrent is doing.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Progress {
    /// Peers connected now, and how many of them have the whole torrent.
    pub peers: u32,
    pub seeds: u32,
    /// Peers known (found, connected or not).
    pub known: u32,
    /// Bytes a second.
    pub download: u64,
    pub upload: u64,
    /// The file's bytes already here, of `len`.
    pub have: u64,
    pub len: u64,
}

/// Lets a reader be ended from outside, waking it if it waits for a piece.
#[derive(Default)]
struct Gate {
    closed: AtomicBool,
    waker: Mutex<Option<Waker>>,
}

impl Gate {
    fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        if let Some(waker) = self.waker.lock().unwrap_or_else(|p| p.into_inner()).take() {
            waker.wake();
        }
    }
}

struct GatedReader<R> {
    inner: R,
    gate: Arc<Gate>,
}

impl<R: AsyncRead + Unpin> AsyncRead for GatedReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if self.gate.closed.load(Ordering::SeqCst) {
            return Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "the torrent was closed",
            )));
        }
        *self.gate.waker.lock().unwrap_or_else(|p| p.into_inner()) = Some(cx.waker().clone());
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

/// A reader of a torrent's file.
pub type Reader = Pin<Box<dyn AsyncRead + Send>>;

struct Open {
    torrent: Arc<ManagedTorrent>,
    id: usize,
    file: usize,
    len: u64,
    info_hash: String,
    readers: Vec<Arc<Gate>>,
    /// The tokens of the subtitle files and poster opened with this video: they close with it.
    companions: Vec<String>,
}

/// A torrent's details as peers sent them: a `.torrent` file's bytes and the peers seen then.
struct Known {
    info_hash: String,
    bytes: Vec<u8>,
    peers: Vec<SocketAddr>,
}

/// A torrent kept running after its video closed: shared on until room is needed.
struct Sharing {
    id: usize,
    info_hash: String,
}

#[derive(Default)]
struct State {
    open: HashMap<String, Open>,
    sharing: Vec<Sharing>,
    /// Downloads running, by info hash.
    downloading: HashSet<String>,
    /// Opens and downloads under way, before they're in `open` or `downloading`: the session
    /// mustn't stop under them.
    starting: usize,
}

/// An open or a download under way, for as long as it's held.
struct Starting<'a>(&'a Engine);

impl Drop for Starting<'_> {
    fn drop(&mut self) {
        let mut state = self.0.state();
        state.starting = state.starting.saturating_sub(1);
    }
}

pub struct Engine {
    config: Mutex<EngineConfig>,
    session: tokio::sync::Mutex<Option<Arc<Session>>>,
    state: Mutex<State>,
    /// Magnet links' details peers sent lately, oldest first ([`KNOWN_DETAILS`] at most).
    known: Mutex<VecDeque<Known>>,
}

impl Engine {
    pub fn new(config: EngineConfig) -> Self {
        Self {
            config: Mutex::new(config),
            session: tokio::sync::Mutex::new(None),
            state: Mutex::new(State::default()),
            known: Mutex::new(VecDeque::new()),
        }
    }

    /// A magnet link's details as peers sent them lately: the `.torrent` file's bytes and the
    /// peers seen then.
    fn known(&self, info_hash: &str) -> Option<(Vec<u8>, Vec<SocketAddr>)> {
        self.known
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .find(|known| known.info_hash == info_hash)
            .map(|known| (known.bytes.clone(), known.peers.clone()))
    }

    fn remember(&self, info_hash: String, bytes: Vec<u8>, peers: Vec<SocketAddr>) {
        let mut known = self.known.lock().unwrap_or_else(|p| p.into_inner());
        known.retain(|old| old.info_hash != info_hash);
        known.push_back(Known {
            info_hash,
            bytes,
            peers,
        });
        while known.len() > KNOWN_DETAILS {
            known.pop_front();
        }
    }

    fn config(&self) -> EngineConfig {
        self.config
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// New settings; a running session keeps the old ones until it next starts.
    pub fn configure(&self, config: EngineConfig) {
        *self.config.lock().unwrap_or_else(|p| p.into_inner()) = config;
    }

    /// Whether the engine holds anything: open files, shared torrents, downloads, and opens or
    /// downloads still starting.
    pub fn busy(&self) -> bool {
        let state = self.state();
        !state.open.is_empty()
            || !state.sharing.is_empty()
            || !state.downloading.is_empty()
            || state.starting > 0
    }

    fn starting(&self) -> Starting<'_> {
        self.state().starting += 1;
        Starting(self)
    }

    /// The info hashes of the torrents whose files are open: their folders in the cache are in
    /// use.
    pub fn open_hashes(&self) -> HashSet<String> {
        self.state()
            .open
            .values()
            .map(|open| open.info_hash.clone())
            .collect()
    }

    /// The session, started on first use. Its folder is emptied first: nothing from an earlier
    /// run is resumed.
    async fn session(&self) -> Result<Arc<Session>, TorrentError> {
        let mut session = self.session.lock().await;
        if let Some(running) = session.as_ref() {
            return Ok(running.clone());
        }
        let config = self.config();
        let dir = config.dir.clone();
        tokio::task::spawn_blocking(move || {
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir)
        })
        .await
        .map_err(engine_error)?
        .map_err(engine_error)?;
        // The engine's own HTTP client needs a TLS provider; the app's is ring.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let started = match start_session(&config, Some(config.port)).await {
            Ok(started) => started,
            Err(error) => {
                // The port is taken (any free one then), or nothing can listen here: peers are
                // still reached, they just can't call in.
                tracing::warn!(%error, port = config.port, "torrent port unavailable");
                let any = if config.port != 0 {
                    start_session(&config, Some(0)).await
                } else {
                    Err(error)
                };
                match any {
                    Ok(started) => started,
                    Err(error) => {
                        tracing::warn!(%error, "torrents run without listening");
                        start_session(&config, None).await?
                    }
                }
            }
        };
        tracing::info!(
            port = ?started.announce_port(),
            dht = config.dht,
            "torrent engine started"
        );
        *session = Some(started.clone());
        Ok(started)
    }

    /// Stops the session when nothing needs it.
    pub async fn stop_if_idle(&self) {
        if self.busy() {
            return;
        }
        let stopped = self.session.lock().await.take();
        if let Some(session) = stopped {
            session.stop().await;
            tracing::info!("torrent engine stopped");
        }
    }

    /// Ends everything: open files, sharing, downloads (which stop where they are), and the
    /// session; the cache folder is emptied. For Offline Mode, torrents turned off, Clear.
    pub async fn shut_down(&self) {
        let gates: Vec<Arc<Gate>> = {
            let mut state = self.state();
            let gates = state
                .open
                .values()
                .flat_map(|open| open.readers.clone())
                .collect();
            state.open.clear();
            state.sharing.clear();
            gates
        };
        for gate in gates {
            gate.close();
        }
        let stopped = self.session.lock().await.take();
        if let Some(session) = stopped {
            session.stop().await;
            tracing::info!("torrent engine shut down");
        }
        let dir = self.config().dir;
        let _ = tokio::task::spawn_blocking(move || std::fs::remove_dir_all(dir)).await;
    }

    /// The torrent's details: from the file, or asked of peers for a magnet link (unless they
    /// sent them lately).
    async fn details(
        &self,
        session: &Arc<Session>,
        source: &Source,
    ) -> Result<librqbit::ListOnlyResponse, TorrentError> {
        let https = self.config().https_trackers;
        let known = match source {
            Source::Magnet(magnet) => self.known(&magnet.info_hash),
            Source::File(_) => None,
        };
        let add = match (source, &known) {
            (_, Some((bytes, _))) => AddTorrent::from_bytes(bytes.clone()),
            (Source::Magnet(magnet), None) => AddTorrent::from_url(magnet.for_engine(https)),
            (Source::File(bytes), None) => AddTorrent::from_bytes(
                crate::announce::with_usable_trackers(bytes, https)
                    .unwrap_or_else(|| bytes.clone()),
            ),
        };
        let options = AddTorrentOptions {
            list_only: true,
            ..Default::default()
        };
        let listed =
            tokio::time::timeout(METADATA_TIMEOUT, session.add_torrent(add, Some(options)))
                .await
                .map_err(|_| TorrentError::NoDetails)?
                .map_err(|error| {
                    let text = format!("{error:#}");
                    if text.contains("no known way to resolve peers") || text.contains("exhausted")
                    {
                        TorrentError::NoDetails
                    } else {
                        TorrentError::Link(format!("This torrent can't be read: {text}"))
                    }
                })?;
        let mut listed = match listed {
            AddTorrentResponse::ListOnly(listed) => listed,
            _ => {
                return Err(TorrentError::Engine(
                    "the torrent's details didn't come back".into(),
                ));
            }
        };
        match (source, known) {
            // Details kept: the peers seen when they came are asked first again.
            (_, Some((_, peers))) => {
                if listed.seen_peers.is_empty() {
                    listed.seen_peers = peers;
                }
            }
            (Source::Magnet(_), None) => self.remember(
                listed.info_hash.as_string(),
                listed.torrent_bytes.to_vec(),
                listed.seen_peers.clone(),
            ),
            (Source::File(_), None) => {}
        }
        Ok(listed)
    }

    /// What a torrent holds (its name and files), from its file or asked of peers for a magnet
    /// link. Nothing is fetched; opening it soon after needs no peers for its details again.
    pub async fn list(&self, source: Source) -> Result<Listing, TorrentError> {
        let _starting = self.starting();
        let session = self.session().await?;
        let listed = self.details(&session, &source).await?;
        Ok(listing_of(&listed))
    }

    /// Adds the torrent with only `files` selected, its pieces kept in `folder`.
    async fn add(
        &self,
        session: &Arc<Session>,
        listed: librqbit::ListOnlyResponse,
        source: &Source,
        files: &[usize],
        folder: &Path,
    ) -> Result<(usize, Arc<ManagedTorrent>), TorrentError> {
        let https = self.config().https_trackers;
        let trackers = match source {
            Source::Magnet(magnet) => magnet.usable_trackers(https),
            Source::File(_) => Vec::new(),
        };
        let options = AddTorrentOptions {
            only_files: Some(files.to_vec()),
            output_folder: Some(folder.to_string_lossy().into_owned()),
            overwrite: true,
            initial_peers: Some(listed.seen_peers),
            trackers: (!trackers.is_empty()).then_some(trackers),
            peer_limit: Some(PEER_LIMIT),
            ..Default::default()
        };
        let added = session
            .add_torrent(AddTorrent::from_bytes(listed.torrent_bytes), Some(options))
            .await
            .map_err(engine_error)?;
        let (id, torrent) = match added {
            AddTorrentResponse::Added(id, torrent) => (id, torrent),
            AddTorrentResponse::AlreadyManaged(id, torrent) => {
                // Opened before (another episode of the same torrent, or shared on): these files
                // join the ones selected.
                let mut selected: HashSet<usize> = torrent
                    .only_files()
                    .unwrap_or_default()
                    .into_iter()
                    .collect();
                let before = selected.len();
                selected.extend(files.iter().copied());
                if selected.len() > before {
                    session
                        .update_only_files(&torrent, &selected)
                        .await
                        .map_err(engine_error)?;
                }
                if torrent.is_paused() {
                    session.unpause(&torrent).await.map_err(engine_error)?;
                }
                (id, torrent)
            }
            AddTorrentResponse::ListOnly(_) => {
                return Err(TorrentError::Engine("the torrent wasn't added".into()));
            }
        };
        torrent
            .wait_until_initialized()
            .await
            .map_err(engine_error)?;
        Ok((id, torrent))
    }

    /// Opens a torrent's video for playing: the file the hint names, else the episode's, else
    /// the largest video, with the subtitle files that come with it. `cap` is what the cache
    /// holds: older torrents' pieces make room, and a bigger file isn't opened.
    pub async fn open(
        &self,
        source: Source,
        hint: &FileHint,
        cap: u64,
    ) -> Result<Opened, TorrentError> {
        let _starting = self.starting();
        let session = self.session().await?;
        let listed = self.details(&session, &source).await?;
        let files = files_of(&listed);
        let index = pick_file(&files, hint).ok_or(TorrentError::NoVideo)?;
        let file = files
            .iter()
            .find(|file| file.index == index)
            .cloned()
            .ok_or(TorrentError::NoVideo)?;
        if file.len > cap {
            return Err(TorrentError::TooLarge {
                size: file.len,
                cap,
            });
        }
        let subtitles = subtitles_for(&files, &file);
        let poster = poster_file(&files).cloned();
        let listing = listing_of(&listed);
        let info_hash = listing.info_hash.clone();
        let needed = subtitles
            .iter()
            .map(|subtitle| &subtitle.file)
            .chain(&poster)
            .fold(file.len, |total, companion| total + companion.len);
        self.make_room(&session, needed, cap, &info_hash).await;
        let folder = self.config().dir.join(&info_hash);
        let selected: Vec<usize> = std::iter::once(file.index)
            .chain(subtitles.iter().map(|subtitle| subtitle.file.index))
            .chain(poster.iter().map(|poster| poster.index))
            .collect();
        let (id, torrent) = self
            .add(&session, listed, &source, &selected, &folder)
            .await?;
        let new_token = || uuid::Uuid::new_v4().simple().to_string();
        let token = new_token();
        let opened: Vec<OpenedSubtitle> = subtitles
            .iter()
            .map(|subtitle| OpenedSubtitle {
                token: new_token(),
                name: subtitle.file.name().to_owned(),
                format: subtitle.format.clone(),
                language: subtitle.language.clone(),
                label: subtitle.label.clone(),
            })
            .collect();
        let opened_poster = poster.as_ref().map(|poster| OpenedFile {
            token: new_token(),
            name: poster.name().to_owned(),
            len: poster.len,
        });
        let companions: Vec<(String, &TorrentFile)> = subtitles
            .iter()
            .zip(&opened)
            .map(|(subtitle, opened)| (opened.token.clone(), &subtitle.file))
            .chain(
                opened_poster
                    .as_ref()
                    .zip(poster.as_ref())
                    .map(|(opened, file)| (opened.token.clone(), file)),
            )
            .collect();
        let mut state = self.state();
        state.sharing.retain(|shared| shared.info_hash != info_hash);
        for (companion, file) in &companions {
            state.open.insert(
                companion.clone(),
                Open {
                    torrent: torrent.clone(),
                    id,
                    file: file.index,
                    len: file.len,
                    info_hash: info_hash.clone(),
                    readers: Vec::new(),
                    companions: Vec::new(),
                },
            );
        }
        state.open.insert(
            token.clone(),
            Open {
                torrent,
                id,
                file: file.index,
                len: file.len,
                info_hash: info_hash.clone(),
                readers: Vec::new(),
                companions: companions.into_iter().map(|(token, _)| token).collect(),
            },
        );
        tracing::info!(
            %info_hash,
            file = %file.name(),
            len = file.len,
            subtitles = opened.len(),
            poster = opened_poster.is_some(),
            "torrent opened"
        );
        Ok(Opened {
            token,
            name: file.name().to_owned(),
            len: file.len,
            info_hash,
            subtitles: opened,
            poster: opened_poster,
            listing,
        })
    }

    /// The length of the file behind `token`, while it's open.
    pub fn file_len(&self, token: &str) -> Option<u64> {
        self.state().open.get(token).map(|open| open.len)
    }

    /// A reader of the open file from `start`. The oldest reader of the file ends when more
    /// than a few are open.
    pub async fn read_from(&self, token: &str, start: u64) -> std::io::Result<Reader> {
        let (torrent, file) = {
            let state = self.state();
            let open = state
                .open
                .get(token)
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "not open"))?;
            (open.torrent.clone(), open.file)
        };
        let mut stream = torrent
            .stream(file)
            .await
            .map_err(|error| std::io::Error::other(format!("{error:#}")))?;
        stream.seek(SeekFrom::Start(start)).await?;
        let gate = Arc::new(Gate::default());
        let ended = {
            let mut state = self.state();
            let Some(open) = state.open.get_mut(token) else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "closed while opening",
                ));
            };
            open.readers
                .retain(|reader| !reader.closed.load(Ordering::SeqCst));
            open.readers.push(gate.clone());
            let extra = open.readers.len().saturating_sub(READERS_PER_FILE);
            open.readers.drain(..extra).collect::<Vec<_>>()
        };
        for old in ended {
            old.close();
        }
        Ok(Box::pin(GatedReader {
            inner: stream,
            gate,
        }))
    }

    /// How the torrent behind `token` is doing.
    pub fn progress(&self, token: &str) -> Option<Progress> {
        let (torrent, file, len) = {
            let state = self.state();
            let open = state.open.get(token)?;
            (open.torrent.clone(), open.file, open.len)
        };
        Some(progress_of(&torrent, file, len))
    }

    /// Closes the file behind `token`, and the subtitle files and poster opened with it. Its
    /// torrent is removed with its pieces, unless `share` keeps it running for other peers (until
    /// room is needed or the engine stops). The session keeps running (another torrent may open
    /// next): [`Self::stop_if_idle`] stops it.
    pub async fn close(&self, token: &str, share: bool) {
        let (open, readers) = {
            let mut state = self.state();
            let Some(open) = state.open.remove(token) else {
                return;
            };
            let mut readers = open.readers.clone();
            for companion in &open.companions {
                if let Some(closed) = state.open.remove(companion) {
                    readers.extend(closed.readers);
                }
            }
            (open, readers)
        };
        for reader in &readers {
            reader.close();
        }
        let still_open = self
            .state()
            .open
            .values()
            .any(|other| other.info_hash == open.info_hash);
        if still_open {
            return;
        }
        if share {
            self.state().sharing.push(Sharing {
                id: open.id,
                info_hash: open.info_hash.clone(),
            });
            tracing::info!(info_hash = %open.info_hash, "torrent shared on after closing");
            return;
        }
        self.remove(open.id, &open.info_hash).await;
        tracing::info!(info_hash = %open.info_hash, "torrent closed and removed");
    }

    /// Whether torrents are kept running for other peers after their videos closed.
    pub fn sharing(&self) -> bool {
        !self.state().sharing.is_empty()
    }

    /// Stops sharing every torrent kept for it, removing their pieces.
    pub async fn stop_sharing(&self) {
        let sharing = std::mem::take(&mut self.state().sharing);
        for shared in sharing {
            self.remove(shared.id, &shared.info_hash).await;
        }
    }

    /// Removes a torrent from the session with its files and folder.
    async fn remove(&self, id: usize, info_hash: &str) {
        let session = self.session.lock().await.clone();
        if let Some(session) = session
            && let Err(error) = session.delete(TorrentIdOrHash::Id(id), true).await
        {
            tracing::debug!(error = %format!("{error:#}"), "torrent not removed");
        }
        let folder = self.config().dir.join(info_hash);
        let _ = tokio::task::spawn_blocking(move || std::fs::remove_dir_all(folder)).await;
    }

    /// Makes room for `needed` bytes under `cap`: torrents shared on go first, then folders
    /// left over, oldest first. Open torrents and `keep`'s folder stay.
    async fn make_room(&self, session: &Arc<Session>, needed: u64, cap: u64, keep: &str) {
        let (open, sharing): (HashSet<String>, Vec<(usize, String)>) = {
            let state = self.state();
            (
                state
                    .open
                    .values()
                    .map(|open| open.info_hash.clone())
                    .collect(),
                state
                    .sharing
                    .iter()
                    .map(|shared| (shared.id, shared.info_hash.clone()))
                    .collect(),
            )
        };
        let dir = self.config().dir;
        let folders = {
            let dir = dir.clone();
            tokio::task::spawn_blocking(move || folders_by_age(&dir))
                .await
                .unwrap_or_default()
        };
        let mut used: u64 = folders.iter().map(|(_, size, _)| size).sum();
        for (name, size, _) in folders {
            if used.saturating_add(needed) <= cap {
                break;
            }
            if name == keep || open.contains(&name) {
                continue;
            }
            if let Some((id, _)) = sharing.iter().find(|(_, hash)| *hash == name) {
                let _ = session.delete(TorrentIdOrHash::Id(*id), true).await;
                self.state()
                    .sharing
                    .retain(|shared| shared.info_hash != name);
            }
            let folder = dir.join(&name);
            let _ = tokio::task::spawn_blocking(move || std::fs::remove_dir_all(folder)).await;
            used = used.saturating_sub(size);
        }
    }

    /// Bytes and files the cache folder holds.
    pub async fn usage(&self) -> (u64, u64) {
        let dir = self.config().dir;
        tokio::task::spawn_blocking(move || tree_size(&dir))
            .await
            .unwrap_or_default()
    }

    /// Downloads a torrent's video into `folder`, reporting the bytes here and the file's size;
    /// `stopped` ends it where it is (the pieces stay, so the next run continues). Returns the
    /// file's path.
    pub async fn download(
        &self,
        source: Source,
        hint: &FileHint,
        folder: &Path,
        report: &(dyn Fn(u64, u64) + Sync),
        stopped: &(dyn Fn() -> bool + Sync),
    ) -> Result<PathBuf, TorrentError> {
        let result = {
            let _starting = self.starting();
            self.download_whole(source, hint, folder, report, stopped)
                .await
        };
        self.stop_if_idle().await;
        result
    }

    async fn download_whole(
        &self,
        source: Source,
        hint: &FileHint,
        folder: &Path,
        report: &(dyn Fn(u64, u64) + Sync),
        stopped: &(dyn Fn() -> bool + Sync),
    ) -> Result<PathBuf, TorrentError> {
        let session = self.session().await?;
        let listed = self.details(&session, &source).await?;
        let files = files_of(&listed);
        let index = pick_file(&files, hint).ok_or(TorrentError::NoVideo)?;
        let file = files
            .iter()
            .find(|file| file.index == index)
            .cloned()
            .ok_or(TorrentError::NoVideo)?;
        let info_hash = listed.info_hash.as_string();
        self.state().downloading.insert(info_hash.clone());
        let result = self
            .download_file(&session, listed, &source, &file, folder, report, stopped)
            .await;
        self.state().downloading.remove(&info_hash);
        result
    }

    #[allow(clippy::too_many_arguments)]
    async fn download_file(
        &self,
        session: &Arc<Session>,
        listed: librqbit::ListOnlyResponse,
        source: &Source,
        file: &TorrentFile,
        folder: &Path,
        report: &(dyn Fn(u64, u64) + Sync),
        stopped: &(dyn Fn() -> bool + Sync),
    ) -> Result<PathBuf, TorrentError> {
        let (id, torrent) = self
            .add(session, listed, source, &[file.index], folder)
            .await?;
        let mut last = (0u64, Instant::now());
        loop {
            // The engine was shut down (Offline Mode, torrents turned off).
            if self
                .session
                .try_lock()
                .is_ok_and(|session| session.is_none())
            {
                return Err(TorrentError::Engine("the torrent engine stopped".into()));
            }
            let progress = progress_of(&torrent, file.index, file.len);
            report(progress.have, file.len);
            if progress.have >= file.len {
                break;
            }
            if let Some(error) = torrent.stats().error {
                let _ = session.delete(TorrentIdOrHash::Id(id), false).await;
                return Err(TorrentError::Engine(error));
            }
            if stopped() {
                // The pieces stay: the next run checks them and continues.
                let _ = session.delete(TorrentIdOrHash::Id(id), false).await;
                return Err(TorrentError::Stopped);
            }
            if progress.have > last.0 {
                last = (progress.have, Instant::now());
            } else if last.1.elapsed() > DOWNLOAD_STALL {
                let _ = session.delete(TorrentIdOrHash::Id(id), false).await;
                return Err(TorrentError::Engine(
                    "No peer has sent anything for ten minutes".into(),
                ));
            }
            tokio::time::sleep(DOWNLOAD_POLL).await;
        }
        let _ = session.delete(TorrentIdOrHash::Id(id), false).await;
        let mut path = folder.to_path_buf();
        for part in file.path.split('/') {
            path.push(part);
        }
        Ok(path)
    }
}

/// The session as `config` says, listening on `port` (`None`: not listening).
async fn start_session(
    config: &EngineConfig,
    port: Option<u16>,
) -> Result<Arc<Session>, TorrentError> {
    let options = SessionOptions {
        dht: config.dht.then_some(DhtSessionConfig {
            bootstrap_addrs: None,
            port: None,
            // Nothing is written outside the cache folder.
            persistence: None,
        }),
        persistence: None,
        fastresume: false,
        listen: port.map(|port| ListenerOptions {
            mode: ListenerMode::TcpOnly,
            listen_addr: SocketAddr::from((Ipv6Addr::UNSPECIFIED, port)),
            enable_upnp_port_forwarding: false,
            ..Default::default()
        }),
        ratelimits: librqbit::limits::LimitsConfig {
            upload_bps: config.upload_limit.and_then(NonZeroU32::new),
            download_bps: None,
        },
        // Local peer discovery and UPnP reach the local network, which Android asks a permission
        // for; public peers are enough.
        disable_local_service_discovery: true,
        peer_limit: Some(PEER_LIMIT),
        runtime_worker_threads: Some(16),
        ..Default::default()
    };
    Session::new_with_opts(config.dir.clone(), options)
        .await
        .map_err(engine_error)
}

/// The torrent's name and files.
fn listing_of(listed: &librqbit::ListOnlyResponse) -> Listing {
    Listing {
        info_hash: listed.info_hash.as_string(),
        name: listed
            .info
            .name()
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty()),
        files: files_of(listed),
    }
}

/// The torrent's files, padding files left out.
fn files_of(listed: &librqbit::ListOnlyResponse) -> Vec<TorrentFile> {
    listed
        .info
        .iter_file_details()
        .enumerate()
        .filter(|(_, details)| !details.attrs().padding)
        .map(|(index, details)| TorrentFile {
            index,
            path: details.filename.to_vec().join("/"),
            len: details.len,
        })
        .collect()
}

fn progress_of(torrent: &ManagedTorrent, file: usize, len: u64) -> Progress {
    let stats = torrent.stats();
    let have = stats.file_progress.get(file).copied().unwrap_or(0).min(len);
    let mut progress = Progress {
        have,
        len,
        ..Progress::default()
    };
    if let Some(live) = stats.live {
        let peers = &live.snapshot.peer_stats;
        progress.peers = peers.live;
        progress.seeds = peers.live_seeders;
        progress.known = peers.seen;
        progress.download = live.download_speed.as_bytes();
        progress.upload = live.upload_speed.as_bytes();
    }
    progress
}

/// The cache's torrent folders with their sizes and last change, oldest first.
fn folders_by_age(dir: &Path) -> Vec<(String, u64, std::time::SystemTime)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut folders: Vec<(String, u64, std::time::SystemTime)> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| {
            let changed = entry
                .metadata()
                .and_then(|meta| meta.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            (
                entry.file_name().to_string_lossy().into_owned(),
                tree_size(&entry.path()).0,
                changed,
            )
        })
        .collect();
    folders.sort_by_key(|(_, _, changed)| *changed);
    folders
}

/// Bytes and files under a folder.
fn tree_size(dir: &Path) -> (u64, u64) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (0, 0);
    };
    let mut total = (0, 0);
    for entry in entries.filter_map(Result::ok) {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            let (bytes, files) = tree_size(&entry.path());
            total.0 += bytes;
            total.1 += files;
        } else if let Ok(meta) = entry.metadata() {
            total.0 += meta.len();
            total.1 += 1;
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_as_gigabytes() {
        assert_eq!(gigabytes(1_400_000_000), "1.4 GB");
        assert_eq!(gigabytes(12_300_000_000), "12 GB");
        assert_eq!(
            TorrentError::TooLarge {
                size: 12_300_000_000,
                cap: 4_000_000_000
            }
            .to_string(),
            "This video is 12 GB, more than the torrent cache holds (4.0 GB)"
        );
    }

    #[test]
    fn the_engine_is_busy_while_a_torrent_starts_opening() {
        let engine = Engine::new(EngineConfig {
            dir: PathBuf::from("unused"),
            port: 0,
            dht: false,
            upload_limit: None,
            https_trackers: true,
        });
        assert!(!engine.busy());
        {
            let _first = engine.starting();
            let _second = engine.starting();
            assert!(engine.busy(), "an idle stop waits for opens under way");
        }
        assert!(!engine.busy());
        assert!(engine.file_len("nothing").is_none());
        assert!(engine.open_hashes().is_empty());
    }

    #[test]
    fn folders_are_measured_oldest_first() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old");
        std::fs::create_dir_all(old.join("sub")).unwrap();
        std::fs::write(old.join("sub").join("a.mkv"), vec![0u8; 300]).unwrap();
        std::thread::sleep(Duration::from_millis(50));
        let new = dir.path().join("new");
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(new.join("b.mkv"), vec![0u8; 100]).unwrap();
        let folders = folders_by_age(dir.path());
        let names: Vec<(&str, u64)> = folders
            .iter()
            .map(|(name, size, _)| (name.as_str(), *size))
            .collect();
        assert_eq!(names, vec![("old", 300), ("new", 100)]);
        assert_eq!(tree_size(dir.path()), (400, 2));
    }
}
