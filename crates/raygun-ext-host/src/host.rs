//! The extension host: loads extensions and routes provider calls to them.
//!
//! Extensions live on a few dedicated threads, the lanes, each running a current-thread tokio
//! runtime and a `LocalSet`; an extension stays on its lane (a QuickJS runtime can't move between
//! threads). Requests arrive over a lane's channel and each runs as a local task, so one extension
//! waiting on the network doesn't block another, and one busy running script (a long loop, a big
//! page to parse) holds up only the extensions on its lane; their requests go on regardless, on
//! the network runtime (`js.rs`). Calls into the *same* extension take turns at its runtime lock
//! between awaits (a provider can still run its own requests in parallel with `Promise.all`).
//! What's loaded, and which lane has it, is known on this side, so listing extensions and
//! providers never waits on a lane.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, PoisonError, RwLock};
use std::time::Duration;

use futures_util::StreamExt;
use raygun_model::{
    ArtistEvent, BrowserCheckOutcome, BrowserCheckResult, ExtensionInfo, ExtensionOrigin,
    ExtensionSetting, ExtensionStatus, LyricLine, Lyrics, LyricsTranslation, LyricsTranslationKind,
    LyricsTranslationOrigin, MediaSource, MusicAlbum, MusicAlbumDetail, MusicArtist, MusicChart,
    MusicPlaylist, MusicPlaylistDetail, OnlineTrack, ProviderInfo, ProviderSearchResult,
    SearchListingsPrefs, SettingKind, SettingValue, SetupNeed, SetupReason, SourceSetup,
    TrackQuery,
};
use raygun_net::{Issue, IssueKind};
use raygun_resolver::Candidate;
use serde::Deserialize;
use tokio::sync::{mpsc, oneshot};

use crate::browser_check::{
    BrowserCookie, Challenge, Challenges, CheckWindow, FailedCall, Met, WindowOutcome,
};
use crate::content::ContentFilter;
use crate::cookies::CookieJar;
use crate::failure::{self, Failure};
use crate::fanout::{self, Asked, Health, Plan, Turns};
use crate::heap::{HeapMeter, HeapUse};
use crate::js::{JsExtension, Limits};
use crate::late::{Found, LateCalls, StopOnDrop};
use crate::manifest::{self, MANIFEST_FILE, Manifest, Runtime};
use crate::servers::ServerExtension;
use crate::shosetsu::ShosetsuExtension;
use crate::storage::KvStore;

pub const MUSIC_METADATA: &str = "music.metadata";
pub const MUSIC_STREAM: &str = "music.stream";
pub const LYRICS: &str = "lyrics";
pub const MUSIC_CHARTS: &str = "music.charts";
pub const MUSIC_TAGS: &str = "music.tags";
/// Artist facts and song credits (M46.14).
pub const MUSIC_INFO: &str = "music.info";
/// Similar artists, top songs, new releases and radios (M46.20).
pub const MUSIC_RECOMMEND: &str = "music.recommend";
/// How long a search or a stream lookup waits for one source (M46.17); background work (charts,
/// tags, downloads) keeps the longer call limit.
pub const INTERACTIVE_TIMEOUT: Duration = Duration::from_secs(12);
/// Sources a fan-out asks at once (searches, lookups, lyrics...); the rest wait their turn, and
/// a slow one makes way after a moment (`fanout::HOLD`). Requests are bounded per site and in
/// all as well (`js.rs`).
pub const FAN_OUT_CONCURRENCY: usize = 16;
/// A catalogue source's search or listing someone waits on (comics, books, videos): longer than
/// a music API's, shorter than the call limit, so a hanging site doesn't hold its row for long.
pub const CATALOG_SEARCH_TIMEOUT: Duration = Duration::from_secs(15);
/// Catalogue searches running at once across every source; more wait, good sources first.
const CATALOG_SEARCHES: usize = 6;

/// Longest tag name kept from a provider.
const MAX_TAG_CHARS: usize = 60;

/// How long a source gets when nobody waits on the answer (charts, tags, recommendations, and
/// anything run [`in_background`]): slow sources, such as converted plugins that scrape whole
/// pages, get there in the end.
pub const BACKGROUND_TIMEOUT: Duration = Duration::from_secs(30);

tokio::task_local! {
    /// Set while work nobody waits on runs ([`in_background`]).
    static BACKGROUND: ();
}

/// Runs `work` as work nobody waits on (a download, the next song found ahead): calls it makes
/// that give a source [`INTERACTIVE_TIMEOUT`] give it [`BACKGROUND_TIMEOUT`] instead, and they
/// don't count as calls someone waits on ([`interactive_calls`]). Calls started on other tasks
/// (spawned) aren't covered.
pub async fn in_background<F: std::future::Future>(work: F) -> F::Output {
    BACKGROUND.scope((), work).await
}

/// Whether the current task runs [`in_background`].
pub fn is_background() -> bool {
    BACKGROUND.try_with(|()| ()).is_ok()
}

/// How long a call someone may be waiting on gives a source: [`INTERACTIVE_TIMEOUT`], or
/// [`BACKGROUND_TIMEOUT`] [`in_background`].
pub fn waiting_limit() -> Duration {
    if is_background() {
        BACKGROUND_TIMEOUT
    } else {
        INTERACTIVE_TIMEOUT
    }
}

/// Calls someone waits on running now (searches, stream lookups, lyrics…), in every host.
static INTERACTIVE_CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// How many calls someone waits on are running: work in the background holds back while any do.
pub fn interactive_calls() -> usize {
    INTERACTIVE_CALLS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Counts one interactive call while it lives (also when it's dropped unfinished).
struct Interactive;

impl Interactive {
    fn start() -> Self {
        INTERACTIVE_CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self
    }
}

impl Drop for Interactive {
    fn drop(&mut self) {
        INTERACTIVE_CALLS.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// One `music.stream` provider's answer: its candidates, or why it failed.
#[derive(Debug, Clone)]
pub struct ProviderCandidates {
    pub provider: ProviderInfo,
    pub result: Result<Vec<Candidate>, String>,
}

/// A stream a provider resolved a candidate to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedStream {
    pub source: MediaSource,
    /// Unix milliseconds after which the URL stops working (signed URLs), if the provider knows.
    pub expires_at: Option<i64>,
}

/// What a resolved stream is for (M46.51): providers with several formats of a song pick by it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StreamUse {
    /// The quality to aim for.
    pub quality: raygun_resolver::QualityTarget,
    /// Raygun reads it itself (a download, captions): a format it decodes.
    pub decodable: bool,
}

/// The user's setting values, per extension id, then per setting key.
pub type UserSettings = std::collections::HashMap<String, BTreeMap<String, serde_json::Value>>;

/// The sign-ins of connected servers (`crate::servers`), per extension id: what the app keeps in
/// its secrets store and hands over at each load.
pub type ServerSecrets = std::collections::HashMap<String, String>;

#[derive(Debug, Clone)]
pub struct HostConfig {
    /// Where per-extension storage lives (`extensions/.data`); `None` keeps storage in memory.
    pub storage_dir: Option<PathBuf>,
    pub limits: Limits,
    /// Longest a provider call may take, waiting included.
    pub call_timeout: Duration,
    /// The NSFW switch (M35), shared with the app.
    pub content: ContentFilter,
    /// Threads extensions run on; 0 picks by the device's cores.
    pub lanes: usize,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            storage_dir: None,
            limits: Limits::default(),
            call_timeout: Duration::from_secs(30),
            content: ContentFilter::default(),
            lanes: 0,
        }
    }
}

/// Lanes when the config leaves it open: one per core, two to four.
fn default_lanes() -> usize {
    std::thread::available_parallelism()
        .map(std::num::NonZero::get)
        .unwrap_or(2)
        .clamp(2, 4)
}

/// An extension's files: compiled in, or a folder under `extensions/`.
#[derive(Debug, Clone)]
pub enum ExtensionFiles {
    Memory(BTreeMap<String, String>),
    Dir(PathBuf),
}

#[derive(Debug, Clone)]
pub struct ExtensionSource {
    pub origin: ExtensionOrigin,
    pub files: ExtensionFiles,
}

impl ExtensionSource {
    pub fn built_in(files: &[(&str, &str)]) -> Self {
        Self {
            origin: ExtensionOrigin::BuiltIn,
            files: ExtensionFiles::Memory(
                files
                    .iter()
                    .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
                    .collect(),
            ),
        }
    }

    pub fn installed(dir: PathBuf) -> Self {
        Self {
            origin: ExtensionOrigin::Installed,
            files: ExtensionFiles::Dir(dir),
        }
    }

    pub(crate) fn read(&self, path: &str) -> Result<String, String> {
        match &self.files {
            ExtensionFiles::Memory(files) => files
                .get(path)
                .cloned()
                .ok_or_else(|| format!("{path} is missing")),
            ExtensionFiles::Dir(dir) => std::fs::read_to_string(dir.join(path))
                .map_err(|error| format!("could not read {path}: {error}")),
        }
    }

    /// A name for errors when the manifest can't be read.
    fn label(&self) -> String {
        match &self.files {
            ExtensionFiles::Memory(_) => "built-in extension".to_owned(),
            ExtensionFiles::Dir(dir) => dir
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| dir.display().to_string()),
        }
    }
}

/// Every folder in `dir` holding a manifest. Folders starting with `.` (such as `.data`) are not
/// extensions.
pub fn discover_installed(dir: &Path) -> Vec<ExtensionSource> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
        .map(|entry| entry.path())
        .filter(|path| path.join(MANIFEST_FILE).is_file())
        .collect();
    dirs.sort();
    dirs.into_iter().map(ExtensionSource::installed).collect()
}

/// Why a provider call failed. Its text is one short sentence naming the source and what went
/// wrong, then (on the next lines) the raw details, for "More info".
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum CallError {
    /// No such provider: not installed, or its extension didn't load.
    #[error("That source isn't installed\nno provider {0}")]
    UnknownProvider(String),
    /// The provider doesn't have the call; asks for optional calls pass over it quietly.
    #[error("{name} can't do that\nprovider {provider} has no method {method}")]
    MissingMethod {
        name: String,
        provider: String,
        method: String,
    },
    /// No answer in the time allowed.
    #[error("{name} didn't answer in time\nno answer within {} seconds", .after.as_secs())]
    Timeout { name: String, after: Duration },
    /// No answer yet in the time someone waits; the call goes on, and asking again gets its
    /// answer (`crate::late`).
    #[error(
        "{name} is still working on it — try again in a moment\nno answer within {} seconds; it goes on in the background",
        .waited.as_secs()
    )]
    StillWorking { name: String, waited: Duration },
    /// The call failed; the issue says why.
    #[error("{0}")]
    Failed(Issue),
    /// The source's own words for what went wrong.
    #[error("{}", said(.name, .message))]
    Said { name: String, message: String },
    /// The script needed more than a source may use (`crate::heap`, `Limits`); `detail` is what
    /// its runtime threw and what it held.
    #[error("{}\n{detail}", .what.summary(.name))]
    Exhausted {
        name: String,
        what: Exhaustion,
        detail: String,
    },
    /// Not asked: the source failed again and again lately and sits out a while. Its last
    /// failure stands in for the answer.
    #[error("{last}\nnot asked for a while after failing {failures} times in a row")]
    Resting { last: Box<CallError>, failures: u32 },
    /// Not asked: the source needs a key, an account or a sign-in first (a required setting is
    /// empty, a server isn't signed in, or its own answers said its key was refused). Not a
    /// failure: nothing was sent, nothing counts against it, and it's asked again once it's set
    /// up. The second line is read by the UI ([`needs_setup_in`]).
    #[error("{name} needs a key or sign-in\nneeds setup ({extension}): {reason}")]
    NeedsSetup {
        name: String,
        extension: String,
        reason: String,
    },
    /// Nobody waits for the answer any more.
    #[error("Stopped")]
    Cancelled,
    #[error("Sources stopped working — restart Raygun\nthe extension host has stopped")]
    HostStopped,
}

/// What a script ran out of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exhaustion {
    /// Its runtime's memory limit (or its host's, for all runtimes together).
    Memory,
    /// Its time for script without a pause (`Limits::cpu_budget`).
    Time,
    /// Its stack, calls nested too deep.
    Stack,
}

impl Exhaustion {
    /// The sentence for `source` running out of this.
    pub fn summary(self, source: &str) -> String {
        match self {
            Self::Memory => format!("{source} needed more memory than a source may use"),
            Self::Time => format!("{source} kept the processor busy too long"),
            Self::Stack => format!("{source}'s script went too deep"),
        }
    }
}

impl CallError {
    /// What kind of failure it is, when the network or the site tells.
    pub fn kind(&self) -> Option<IssueKind> {
        match self {
            Self::Timeout { .. } | Self::StillWorking { .. } => Some(IssueKind::Timeout),
            Self::Failed(issue) => Some(issue.kind),
            Self::Exhausted { .. } => Some(IssueKind::ExtensionFailed),
            Self::Resting { last, .. } => last.kind(),
            _ => None,
        }
    }

    /// Whether the provider just doesn't have the call.
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::MissingMethod { .. })
    }

    /// Whether the source wasn't asked because it needs a key or sign-in first: not a failure.
    pub fn is_needs_setup(&self) -> bool {
        matches!(self, Self::NeedsSetup { .. })
    }

    /// The raw details: what the script threw, what its requests met.
    pub fn detail(&self) -> String {
        match self {
            Self::Failed(issue) => issue.detail.clone(),
            Self::Said { message, .. } => message.clone(),
            Self::Exhausted { detail, .. } => detail.clone(),
            Self::Resting { last, .. } => last.detail(),
            Self::NeedsSetup { reason, .. } => reason.clone(),
            other => other.to_string(),
        }
    }
}

/// The extension an error text names as needing a key or sign-in ([`CallError::NeedsSetup`]'s
/// words), for answers that only keep the text.
pub fn needs_setup_in(text: &str) -> Option<&str> {
    text.lines().find_map(|line| {
        let rest = line.trim().strip_prefix("needs setup (")?;
        let (extension, _) = rest.split_once(')')?;
        manifest::is_extension_id(extension).then_some(extension)
    })
}

/// A source's own message, with its name unless the message names it already.
fn said(name: &str, message: &str) -> String {
    let lower = message.to_lowercase();
    // "This source can't search" says who already.
    if lower.contains(&name.to_lowercase()) || lower.contains("source") {
        message.to_owned()
    } else {
        format!("{name}: {message}")
    }
}

/// What a lane is asked to do.
enum Request {
    /// Replaces the lane's extensions with these.
    Load {
        items: Vec<LoadItem>,
        reply: oneshot::Sender<Vec<ExtensionInfo>>,
    },
    Call {
        provider: String,
        method: String,
        args: serde_json::Value,
        reply: oneshot::Sender<Result<serde_json::Value, CallError>>,
    },
    /// The settings of the lane's Miru extensions, by extension id, as they are now.
    Settings {
        reply: oneshot::Sender<Vec<(String, Vec<ExtensionSetting>)>>,
    },
    /// A YouTube video's audio file from Raygun's YouTube module, asked through `extension`'s
    /// own fetch.
    YouTubeAudio {
        extension: String,
        video: String,
        prefs: crate::youtube::StreamPrefs,
        reply: oneshot::Sender<Result<crate::youtube::Stream, String>>,
    },
}

/// An extension for a lane to load.
struct LoadItem {
    source: ExtensionSource,
    manifest: Manifest,
    settings: BTreeMap<String, serde_json::Value>,
    /// A connected server's sign-in.
    secret: Option<String>,
}

/// An extension's version, settings and sign-in, hashed: what a source that said its key was
/// refused waits on to change.
fn fingerprint(item: &LoadItem) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    item.manifest.version.to_string().hash(&mut hasher);
    serde_json::to_string(&item.settings)
        .unwrap_or_default()
        .hash(&mut hasher);
    item.secret.hash(&mut hasher);
    hasher.finish()
}

/// The need of a source whose own answer said its key or sign-in was refused (`said`).
fn refused_need(said: &str) -> SetupNeed {
    SetupNeed {
        reason: SetupReason::Refused,
        missing: Vec::new(),
        detail: Some(said.to_owned()).filter(|said| !said.is_empty()),
    }
}

/// Why a source isn't asked, for [`CallError::NeedsSetup`]'s details.
fn need_reason(need: &SetupNeed) -> String {
    match need.reason {
        SetupReason::Settings => format!("{} not set", need.missing.join(", ")),
        SetupReason::SignIn => "the server isn't signed in".to_owned(),
        SetupReason::Refused => need
            .detail
            .clone()
            .unwrap_or_else(|| "its key or sign-in was refused".to_owned()),
    }
}

/// What a loaded extension needs before it's asked anything: a server's sign-in, or required
/// settings that are empty (`settings` as listed, with their values).
fn setup_need(extension: &Extension, settings: &[ExtensionSetting]) -> Option<SetupNeed> {
    if let Extension::Server(server) = extension
        && server.signed_out()
    {
        return Some(SetupNeed {
            reason: SetupReason::SignIn,
            missing: Vec::new(),
            detail: None,
        });
    }
    let missing: Vec<String> = settings
        .iter()
        .filter(|setting| setting.required)
        .filter(|setting| match &setting.value {
            None => true,
            Some(SettingValue::Text(text)) => text.trim().is_empty(),
            Some(_) => false,
        })
        .map(|setting| setting.label.clone())
        .collect();
    (!missing.is_empty()).then_some(SetupNeed {
        reason: SetupReason::Settings,
        missing,
        detail: None,
    })
}

/// How much an extension weighs on its lane: code written for other apps parses whole pages
/// in script; Shosetsu scripts and servers run their work elsewhere.
fn lane_weight(item: &LoadItem) -> usize {
    match item.manifest.runtime {
        Runtime::Shosetsu | Runtime::Server | Runtime::Site => 0,
        _ if crate::js::is_converted(&item.manifest) => 3,
        _ => 1,
    }
}

/// Each lane's own stack: QuickJS may use at most half of it ([`Limits::stack_bytes`] is held
/// to that), the rest is for Rust's frames around the script's.
const LANE_STACK_BYTES: usize = 8 * 1024 * 1024;

/// Handle to the host; cheap to clone and usable from any thread or runtime.
#[derive(Clone)]
pub struct ExtensionHost {
    shared: Arc<HostShared>,
}

struct HostShared {
    lanes: Vec<mpsc::UnboundedSender<Request>>,
    directory: RwLock<Directory>,
    content: ContentFilter,
    health: Health,
    catalog_turns: Arc<Turns>,
    /// Where extensions' cookie jars are saved (`HostConfig::storage_dir`).
    storage_dir: Option<PathBuf>,
    /// Browser checks sources met lately (`crate::browser_check`), noted by the lanes.
    challenges: Arc<Challenges>,
    /// What searches read of sources' listings lately (`crate::listing_search`).
    listings: crate::listing_search::Listings,
    /// The heaps of loaded JavaScript extensions, noted by the lanes as they load them.
    heaps: Heaps,
    /// Calls someone waited on that went on past their time.
    late: LateCalls,
    /// Extensions whose own answers said their key or sign-in is missing or was refused, by id:
    /// the fingerprint of the settings they said it with (`Directory::fingerprints`) and what
    /// they said. They're asked nothing until those settings change.
    refused: std::sync::Mutex<HashMap<String, (u64, String)>>,
    /// Counts changes to which extensions need setup (a load, a source saying so).
    setup_changes: tokio::sync::watch::Sender<u64>,
}

/// Each loaded JavaScript extension's heap meter and memory limit, by extension id.
type Heaps = Arc<std::sync::Mutex<HashMap<String, (Arc<HeapMeter>, usize)>>>;

/// What's loaded, as of the last load.
#[derive(Default)]
struct Directory {
    /// Every extension, failed ones included, by name.
    infos: Vec<ExtensionInfo>,
    /// The lane each loaded extension runs on.
    lanes: HashMap<String, usize>,
    /// Providers' names, by id.
    names: HashMap<String, String>,
    /// Miru extensions, whose settings may change after loading.
    miru: std::collections::HashSet<String>,
    /// What extensions need before they're asked anything, as loaded: a required setting left
    /// empty, a server without its sign-in.
    setup: HashMap<String, SetupNeed>,
    /// Each extension's version, settings and sign-in as loaded, hashed: a source that said its
    /// key was refused is asked again once this changes.
    fingerprints: HashMap<String, u64>,
    /// Site-scripts extensions (`crate::site`), with where their files are.
    sites: HashMap<String, (ExtensionSource, Manifest)>,
}

/// A fan-out's own limit outlasts its calls' by this much: a call someone waits on hands its
/// source over to the background at its own limit (`crate::late`), which a fan-out's limit
/// reached first would stop.
const PLAN_GRACE: Duration = Duration::from_secs(2);

/// Sources asked at once while Data Saver asks for fewer.
pub const SAVER_FAN_OUT_CONCURRENCY: usize = 4;

fn plan(timeout: Duration) -> Plan {
    Plan {
        concurrency: if raygun_net::network_mode::fewer_sources() {
            SAVER_FAN_OUT_CONCURRENCY
        } else {
            FAN_OUT_CONCURRENCY
        },
        timeout: timeout + PLAN_GRACE,
        skip_resting: true,
    }
}

impl ExtensionHost {
    pub fn start(mut config: HostConfig) -> std::io::Result<Self> {
        let count = if config.lanes == 0 {
            default_lanes()
        } else {
            config.lanes
        };
        // A script's stack must end before its lane's does, or a deep one would crash the app
        // rather than fail with "stack overflow".
        config.limits.stack_bytes = config.limits.stack_bytes.min(LANE_STACK_BYTES / 2);
        let challenges = Arc::new(Challenges::default());
        let heaps = Heaps::default();
        let mut lanes = Vec::with_capacity(count);
        for index in 0..count {
            let (requests, receiver) = mpsc::unbounded_channel();
            let config = config.clone();
            let challenges = challenges.clone();
            let heaps = heaps.clone();
            std::thread::Builder::new()
                .name(format!("raygun-ext-{index}"))
                .stack_size(LANE_STACK_BYTES)
                .spawn(move || run_lane(config, receiver, challenges, heaps))?;
            lanes.push(requests);
        }
        Ok(Self {
            shared: Arc::new(HostShared {
                lanes,
                directory: RwLock::new(Directory::default()),
                content: config.content.clone(),
                health: Health::default(),
                catalog_turns: Turns::new(CATALOG_SEARCHES),
                storage_dir: config.storage_dir.clone(),
                challenges,
                listings: crate::listing_search::Listings::default(),
                heaps,
                late: LateCalls::default(),
                refused: std::sync::Mutex::new(HashMap::new()),
                setup_changes: tokio::sync::watch::Sender::new(0),
            }),
        })
    }

    /// The lane a loaded extension runs on.
    #[cfg(test)]
    pub(crate) fn lane_of(&self, extension_id: &str) -> Option<usize> {
        self.directory().lanes.get(extension_id).copied()
    }

    /// What a loaded JavaScript extension's heap holds, the most it held since its peak was
    /// last taken (`take_peak` takes it, so the next read starts over), and its limit.
    pub fn heap_use(&self, extension_id: &str, take_peak: bool) -> Option<HeapUse> {
        let heaps = self
            .shared
            .heaps
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let (meter, limit) = heaps.get(extension_id)?;
        Some(HeapUse {
            now: meter.now(),
            peak: if take_peak {
                meter.take_peak()
            } else {
                meter.peak()
            },
            limit: *limit,
        })
    }

    /// The NSFW switch: off hides providers of `nsfw`-rated extensions and tells extensions to
    /// filter their results.
    pub fn content(&self) -> &ContentFilter {
        &self.shared.content
    }

    /// What sources' calls did lately: fan-outs ask the good ones first and rest failing ones.
    pub fn health(&self) -> &Health {
        &self.shared.health
    }

    /// Turns for catalogue searches someone waits on (comics, books, videos).
    pub(crate) fn catalog_turns(&self) -> &Arc<Turns> {
        &self.shared.catalog_turns
    }

    /// What searches read of sources' listings lately.
    pub(crate) fn listings(&self) -> &crate::listing_search::Listings {
        &self.shared.listings
    }

    fn directory(&self) -> std::sync::RwLockReadGuard<'_, Directory> {
        self.shared
            .directory
            .read()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Replaces every loaded extension with `sources`. An installed extension overrides a
    /// built-in one with the same id. Returns what loaded (or failed), sorted by name.
    pub async fn load(&self, sources: Vec<ExtensionSource>) -> Vec<ExtensionInfo> {
        self.load_with_settings(sources, UserSettings::new()).await
    }

    /// Like [`Self::load`], with the user's setting values (invalid ones are ignored). Lanes
    /// load their share at the same time; an extension keeps its lane across loads, and a new
    /// one goes to the lane with the fewest.
    pub async fn load_with_settings(
        &self,
        sources: Vec<ExtensionSource>,
        settings: UserSettings,
    ) -> Vec<ExtensionInfo> {
        self.load_with(sources, settings, ServerSecrets::new())
            .await
    }

    /// Like [`Self::load_with_settings`], with the sign-ins of connected servers.
    pub async fn load_with(
        &self,
        sources: Vec<ExtensionSource>,
        mut settings: UserSettings,
        mut secrets: ServerSecrets,
    ) -> Vec<ExtensionInfo> {
        let (chosen, mut infos) = choose(sources);
        let previous = self.directory().lanes.clone();
        let count = self.shared.lanes.len();
        let mut batches: Vec<Vec<LoadItem>> = (0..count).map(|_| Vec::new()).collect();
        let mut fresh = Vec::new();
        let mut fingerprints = HashMap::new();
        let mut sites = HashMap::new();
        for (id, (source, manifest)) in chosen {
            if manifest.runtime == Runtime::Site {
                sites.insert(id.clone(), (source.clone(), manifest.clone()));
            }
            let item = LoadItem {
                settings: settings.remove(&id).unwrap_or_default(),
                secret: secrets.remove(&id),
                source,
                manifest,
            };
            fingerprints.insert(id.clone(), fingerprint(&item));
            match previous.get(&id) {
                Some(&lane) if lane < count => batches[lane].push(item),
                _ => fresh.push(item),
            }
        }
        // A new extension goes to the lane with the least work: converted plugins and scripts
        // weigh more, as they parse whole pages in script.
        for item in fresh {
            let lane = (0..count)
                .min_by_key(|lane| batches[*lane].iter().map(lane_weight).sum::<usize>())
                .unwrap_or_default();
            batches[lane].push(item);
        }
        let miru = batches
            .iter()
            .flatten()
            .filter(|item| item.manifest.runtime.declares_settings())
            .map(|item| item.manifest.id.clone())
            .collect();
        let placed: HashMap<String, usize> = batches
            .iter()
            .enumerate()
            .flat_map(|(lane, items)| {
                items
                    .iter()
                    .map(move |item| (item.manifest.id.clone(), lane))
            })
            .collect();
        let loads = batches
            .into_iter()
            .enumerate()
            .map(|(lane, items)| async move {
                let (reply, answer) = oneshot::channel();
                if self.shared.lanes[lane]
                    .send(Request::Load { items, reply })
                    .is_err()
                {
                    return Vec::new();
                }
                answer.await.unwrap_or_default()
            });
        for loaded in futures_util::future::join_all(loads).await {
            infos.extend(loaded);
        }
        infos.sort_by_key(|info| info.name.to_lowercase());
        let lanes: HashMap<String, usize> = infos
            .iter()
            .filter(|info| info.status == ExtensionStatus::Loaded)
            .filter_map(|info| Some((info.id.clone(), *placed.get(&info.id)?)))
            .collect();
        self.shared
            .heaps
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|id, _| lanes.contains_key(id));
        let names = infos
            .iter()
            .flat_map(|info| &info.providers)
            .map(|provider| (provider.id.clone(), provider.name.clone()))
            .collect();
        let setup = infos
            .iter()
            .filter_map(|info| Some((info.id.clone(), info.needs_setup.clone()?)))
            .collect();
        // A source that said its key was refused is asked again once its settings (or sign-in,
        // or version) change, and forgotten once it's gone.
        self.refused()
            .retain(|id, (said_with, _)| fingerprints.get(id) == Some(said_with));
        let previous = std::mem::replace(
            &mut *self
                .shared
                .directory
                .write()
                .unwrap_or_else(PoisonError::into_inner),
            Directory {
                infos: infos.clone(),
                lanes,
                names,
                miru,
                setup,
                fingerprints,
                sites,
            },
        );
        self.shared
            .setup_changes
            .send_modify(|changes| *changes += 1);
        // An extension that changed (an update, other settings) starts with a clean record.
        for info in &infos {
            let same = previous.infos.iter().any(|old| {
                old.id == info.id && old.version == info.version && old.settings == info.settings
            });
            if !same {
                for provider in &info.providers {
                    self.shared.health.forget(&provider.id);
                }
            }
        }
        self.rate(&mut infos);
        self.with_setup(&mut infos);
        infos
    }

    /// Site-scripts extensions that loaded and are on, with the values of their settings.
    pub fn site_extensions(&self) -> Vec<crate::site::SiteExtension> {
        let directory = self.directory();
        let mut found: Vec<crate::site::SiteExtension> = directory
            .infos
            .iter()
            .filter(|info| info.status == ExtensionStatus::Loaded)
            .filter(|info| {
                !self
                    .shared
                    .content
                    .is_disabled(&crate::site::provider_id(&info.id))
            })
            .filter_map(|info| {
                let (source, manifest) = directory.sites.get(&info.id)?;
                let scripts = manifest.site.clone()?;
                let settings = info
                    .settings
                    .iter()
                    .map(|setting| {
                        let value = setting
                            .value
                            .as_ref()
                            .and_then(|value| serde_json::to_value(value).ok())
                            .unwrap_or(serde_json::Value::Null);
                        (setting.key.clone(), value)
                    })
                    .collect();
                Some(crate::site::SiteExtension::new(
                    manifest,
                    scripts,
                    settings,
                    source.clone(),
                ))
            })
            .collect();
        found.sort_by_key(|site| site.name.to_lowercase());
        found
    }

    fn refused(&self) -> std::sync::MutexGuard<'_, HashMap<String, (u64, String)>> {
        self.shared
            .refused
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Adds what sources said about their keys since they loaded to `infos`' setup needs.
    fn with_setup(&self, infos: &mut [ExtensionInfo]) {
        let refused = self.refused();
        for info in infos {
            if info.needs_setup.is_none()
                && let Some((_, said)) = refused.get(&info.id)
            {
                info.needs_setup = Some(refused_need(said));
            }
        }
    }

    /// What `extension_id` needs before it's asked anything (a key, an account, a sign-in), or
    /// `None` when it's ready: a required setting left empty, a server without its sign-in, or
    /// its own answers having said its key was refused (until its settings change). While it
    /// needs setup its providers are left out of every search and lookup, and calls to it
    /// answer [`CallError::NeedsSetup`] at once, without a request.
    pub fn needs_setup(&self, extension_id: &str) -> Option<SetupNeed> {
        if let Some(need) = self.directory().setup.get(extension_id) {
            return Some(need.clone());
        }
        self.refused()
            .get(extension_id)
            .map(|(_, said)| refused_need(said))
    }

    /// Whether a provider's extension needs setup (see [`Self::needs_setup`]).
    pub fn provider_needs_setup(&self, provider: &str) -> bool {
        let extension = provider.split_once('/').map_or(provider, |(id, _)| id);
        self.needs_setup(extension).is_some()
    }

    /// Notes that `extension_id`'s own answer said its key or sign-in is missing or refused: it's
    /// asked nothing more until its settings change.
    fn note_refused(&self, extension_id: &str, said: &str) {
        let Some(fingerprint) = self.directory().fingerprints.get(extension_id).copied() else {
            return;
        };
        let changed = self
            .refused()
            .insert(extension_id.to_owned(), (fingerprint, said.to_owned()))
            .is_none();
        if changed {
            tracing::info!(extension = %extension_id, reason = %said, "a source needs setup: it's asked nothing until its settings change");
            self.shared
                .setup_changes
                .send_modify(|changes| *changes += 1);
        }
    }

    /// The sources that need setup and would otherwise be asked, by name: each with the kinds
    /// of its providers that searches and lookups use (turned off ones and hidden 18+ ones left
    /// out; a source with none left isn't listed).
    pub fn sources_needing_setup(&self) -> Vec<SourceSetup> {
        let no_adult = crate::content::adult_left_out();
        let content = &self.shared.content;
        let mut infos = self.directory().infos.clone();
        self.with_setup(&mut infos);
        infos
            .into_iter()
            .filter(|info| info.status == ExtensionStatus::Loaded)
            .filter_map(|info| {
                let need = info.needs_setup?;
                let adult = content.extension_adult(&info.id, &info.content_rating);
                let mut kinds: Vec<String> = info
                    .providers
                    .iter()
                    .filter(|provider| !content.is_disabled(&provider.id))
                    .filter(|provider| {
                        !(adult && (no_adult || !content.allows_adult(&provider.kind)))
                    })
                    .map(|provider| provider.kind.clone())
                    .collect();
                kinds.sort();
                kinds.dedup();
                (!kinds.is_empty()).then(|| SourceSetup {
                    server: info.id.starts_with(crate::servers::ID_PREFIX),
                    extension_id: info.id,
                    name: info.name,
                    kinds,
                    need,
                })
            })
            .collect()
    }

    /// Changes to which sources need setup: a load, or a source saying its key was refused.
    pub fn setup_changes(&self) -> tokio::sync::watch::Receiver<u64> {
        self.shared.setup_changes.subscribe()
    }

    /// Every extension, failed ones included, by name. Miru extensions' settings are asked of
    /// their lanes (a script may register them after loading); everything else is as loaded.
    pub async fn list(&self) -> Vec<ExtensionInfo> {
        let (mut infos, lanes) = {
            let directory = self.directory();
            let mut lanes: Vec<usize> = directory
                .miru
                .iter()
                .filter_map(|id| directory.lanes.get(id).copied())
                .collect();
            lanes.sort_unstable();
            lanes.dedup();
            (directory.infos.clone(), lanes)
        };
        for lane in lanes {
            let (reply, answer) = oneshot::channel();
            if self.shared.lanes[lane]
                .send(Request::Settings { reply })
                .is_err()
            {
                continue;
            }
            for (id, settings) in answer.await.unwrap_or_default() {
                if let Some(info) = infos.iter_mut().find(|info| info.id == id) {
                    info.settings = settings;
                }
            }
        }
        self.rate(&mut infos);
        self.with_setup(&mut infos);
        infos
    }

    /// Marks extensions and their providers 18+ as the user's marks and their ratings say.
    fn rate(&self, infos: &mut [ExtensionInfo]) {
        for info in infos {
            let adult = self
                .shared
                .content
                .extension_adult(&info.id, &info.content_rating);
            info.adult = adult;
            for provider in &mut info.providers {
                provider.adult = adult;
            }
        }
    }

    /// Providers of `kind` that searches and lookups use: not the disabled ones, not those whose
    /// extension needs a key or sign-in first ([`Self::needs_setup`]), and those of NSFW-rated
    /// extensions only while NSFW content is shown, and no adult ones inside
    /// [`crate::without_adult_sources`].
    pub async fn providers(&self, kind: &str) -> Vec<ProviderInfo> {
        let no_adult = crate::content::adult_left_out();
        self.listed_providers(kind)
            .await
            .into_iter()
            .filter(|provider| !self.shared.content.is_disabled(&provider.id))
            .filter(|provider| !(no_adult && provider.adult))
            .filter(|provider| self.needs_setup(&provider.extension_id).is_none())
            .collect()
    }

    /// Providers of `kind` to list, disabled ones included (the user turns them back on there,
    /// M46.8); 18+ extensions' (rated `nsfw` or marked so by the user) only while NSFW content
    /// is shown (music: while explicit music is, M46.36).
    pub async fn listed_providers(&self, kind: &str) -> Vec<ProviderInfo> {
        let content = &self.shared.content;
        let allowed = content.allows_adult(kind);
        self.directory()
            .infos
            .iter()
            .filter_map(|extension| {
                let adult = content.extension_adult(&extension.id, &extension.content_rating);
                (allowed || !adult).then_some((extension, adult))
            })
            .flat_map(|(extension, adult)| {
                extension
                    .providers
                    .iter()
                    .filter(|provider| provider.kind == kind)
                    .map(move |provider| ProviderInfo {
                        adult,
                        ..provider.clone()
                    })
            })
            .collect()
    }

    /// A provider's name for people (`<extension>/<provider>` id), for messages.
    pub fn provider_name(&self, provider: &str) -> String {
        self.directory()
            .names
            .get(provider)
            .cloned()
            .unwrap_or_else(|| "The source".to_owned())
    }

    /// A provider's songs, without explicit ones while Explicit music is off (M46.36).
    fn tracks(&self, value: serde_json::Value) -> Vec<OnlineTrack> {
        let mut tracks = parse_tracks(value);
        if !self.shared.content.show_explicit_music() {
            tracks.retain(|track| !track.explicit);
        }
        tracks
    }

    /// Calls `method` on a provider (its full `<extension>/<provider>` id), JSON in and out.
    /// A call someone is waiting on (M46.17): a search or a stream lookup waits
    /// [`INTERACTIVE_TIMEOUT`] for the answer, so one source that never answers doesn't hold up
    /// the others. A source still working then goes on in the background, up to
    /// [`BACKGROUND_TIMEOUT`], and [`CallError::StillWorking`] says so; asking the same again gets
    /// its answer (`crate::late`). Counted while someone waits ([`interactive_calls`]).
    /// [`in_background`], it's an ordinary call with [`BACKGROUND_TIMEOUT`].
    pub async fn call_interactive(
        &self,
        provider: &str,
        method: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, CallError> {
        if is_background() {
            return self
                .call_within(provider, method, args, BACKGROUND_TIMEOUT)
                .await;
        }
        let _counted = Interactive::start();
        self.call_or_go_on(
            provider,
            method,
            args,
            INTERACTIVE_TIMEOUT,
            BACKGROUND_TIMEOUT,
        )
        .await
    }

    /// [`Self::call_within`] `limit`, waiting `wait` for the answer: a call still going then goes
    /// on in the background ([`crate::late`]) and [`CallError::StillWorking`] says so; the same
    /// ask again gets its answer, or waits `wait` again for it. Dropped before `wait`, the call
    /// stops, as any call nobody waits for.
    pub(crate) async fn call_or_go_on(
        &self,
        provider: &str,
        method: &str,
        args: serde_json::Value,
        wait: Duration,
        limit: Duration,
    ) -> Result<serde_json::Value, CallError> {
        let key = crate::late::key(provider, method, &args);
        let still_working = || CallError::StillWorking {
            name: self.provider_name(provider),
            waited: wait,
        };
        match self.shared.late.find(&key) {
            Some(Found::Answer(answer)) => return answer,
            Some(Found::Running(mut running)) => {
                return match tokio::time::timeout(wait, running.wait_for(Option::is_some)).await {
                    Ok(Ok(answer)) => {
                        let answer = answer.clone().unwrap_or(Err(CallError::HostStopped));
                        self.shared.late.forget(&key);
                        answer
                    }
                    Ok(Err(_)) => Err(CallError::HostStopped),
                    Err(_) => Err(still_working()),
                };
            }
            None => {}
        }
        let started = std::time::Instant::now();
        let host = self.clone();
        let (provider_id, method_name) = (provider.to_owned(), method.to_owned());
        let mut task = StopOnDrop(Some(tokio::spawn(async move {
            host.call_within(&provider_id, &method_name, args, limit)
                .await
        })));
        let Some(running) = task.0.as_mut() else {
            return Err(CallError::HostStopped);
        };
        if let Ok(joined) = tokio::time::timeout(wait, running).await {
            return joined.unwrap_or(Err(CallError::HostStopped));
        }
        // When this one doesn't go on, it stops as `task` is dropped.
        let answer_to = match self.shared.late.going_on(&key) {
            Ok(answer_to) => answer_to,
            // The same ask, started alongside, goes on already.
            Err(crate::late::NotKept::Already) => return Err(still_working()),
            Err(crate::late::NotKept::Full) => {
                return Err(CallError::Timeout {
                    name: self.provider_name(provider),
                    after: wait,
                });
            }
        };
        let Some(running) = task.keep() else {
            return Err(CallError::HostStopped);
        };
        let shared = self.shared.clone();
        let provider_id = provider.to_owned();
        tokio::spawn(async move {
            let answer = running.await.unwrap_or(Err(CallError::HostStopped));
            shared
                .health
                .record(&provider_id, answer.as_ref().map(|_| ()), started.elapsed());
            if let Err(error) = &answer {
                tracing::debug!(provider = %provider_id, %error, "a late call failed");
            }
            // Kept first: an ask waiting on it takes it (`forget`), so it's handed over once.
            shared.late.answered(&key, answer.clone());
            answer_to.send_replace(Some(answer));
        });
        Err(still_working())
    }

    /// Like [`Self::call`], giving up after `limit`; the extension stops working on it then.
    pub async fn call_within(
        &self,
        provider: &str,
        method: &str,
        args: serde_json::Value,
        limit: Duration,
    ) -> Result<serde_json::Value, CallError> {
        match tokio::time::timeout(limit, self.call(provider, method, args)).await {
            Ok(answer) => answer,
            Err(_) => Err(CallError::Timeout {
                name: self.provider_name(provider),
                after: limit,
            }),
        }
    }

    /// Calls `method` on a provider (its full `<extension>/<provider>` id), JSON in and out,
    /// within the host's call limit. Dropping the future stops the call.
    pub async fn call(
        &self,
        provider: &str,
        method: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, CallError> {
        let unknown = || CallError::UnknownProvider(provider.to_owned());
        let (extension_id, _) = provider.split_once('/').ok_or_else(unknown)?;
        let lane = self
            .directory()
            .lanes
            .get(extension_id)
            .copied()
            .ok_or_else(unknown)?;
        // A source that needs a key or sign-in first is asked nothing: no script runs, nothing
        // is sent, and nothing counts against it.
        if let Some(need) = self.needs_setup(extension_id) {
            return Err(CallError::NeedsSetup {
                name: self.provider_name(provider),
                extension: extension_id.to_owned(),
                reason: need_reason(&need),
            });
        }
        let (reply, answer) = oneshot::channel();
        self.shared.lanes[lane]
            .send(Request::Call {
                provider: provider.to_owned(),
                method: method.to_owned(),
                args,
                reply,
            })
            .map_err(|_| CallError::HostStopped)?;
        let answer = answer.await.unwrap_or(Err(CallError::HostStopped));
        // The source said so itself (`raygun.needsSetup`, a server refusing its sign-in).
        if let Err(CallError::NeedsSetup { reason, .. }) = &answer {
            self.note_refused(extension_id, reason);
        }
        answer
    }

    /// The browser check behind an error text (`crate::browser_check`): `source` when the page
    /// knows it, else the source the error names.
    pub fn browser_check(&self, error: &str, source: Option<&str>) -> Option<Challenge> {
        self.shared.challenges.find(error, source)
    }

    /// The browser check `extension_id`'s sources met last, if any.
    pub fn browser_check_of(&self, extension_id: &str) -> Option<Challenge> {
        self.shared.challenges.of_extension(extension_id)
    }

    /// Puts cookies a browser gave for `url` (a browser check passed by hand) into
    /// `extension_id`'s cookie jar, with the `user_agent` the browser passed it with (the
    /// extension's requests to the site go with it while those cookies last). Returns how many
    /// cookies were taken.
    pub fn import_browser_cookies(
        &self,
        extension_id: &str,
        url: &str,
        cookies: &[BrowserCookie],
        user_agent: Option<&str>,
    ) -> Result<usize, String> {
        let url = reqwest::Url::parse(url).map_err(|error| error.to_string())?;
        let dir = self
            .shared
            .storage_dir
            .as_ref()
            .ok_or("extension data is kept in memory")?;
        if extension_id.is_empty()
            || extension_id.contains(['/', '\\'])
            || extension_id.starts_with('.')
        {
            return Err(format!("not an extension id: {extension_id}"));
        }
        Ok(CookieJar::at(&dir.join(extension_id).join("cookies.json"))
            .import(&url, cookies, user_agent))
    }

    /// Forgets `extension_id`'s browser checks and its sources' failures, so they're asked again
    /// at once.
    fn forget_failures(&self, extension_id: &str) {
        self.shared.challenges.clear(extension_id);
        let prefix = format!("{extension_id}/");
        let providers: Vec<String> = self
            .directory()
            .names
            .keys()
            .filter(|id| id.starts_with(&prefix))
            .cloned()
            .collect();
        for provider in providers {
            self.shared.health.forget(&provider);
        }
    }

    /// Ends a browser check opened by hand (`crate::browser_check`), from what its `window` saw
    /// and the browser's `cookies` and `user_agent` for the checked request's address. Only a
    /// passed check changes anything: the extension then takes the cookies and the User-Agent,
    /// its failures are forgotten, and the call that failed on the check is asked again, which
    /// tells whether the source works now. A page that loaded without a check says so.
    pub async fn finish_browser_check(
        &self,
        challenge: &Challenge,
        window: &CheckWindow,
        cookies: &[BrowserCookie],
        user_agent: Option<&str>,
    ) -> Result<BrowserCheckResult, String> {
        let result = |outcome, cookies, error| BrowserCheckResult {
            outcome,
            cookies,
            error,
        };
        match window.outcome() {
            WindowOutcome::Passed => {}
            WindowOutcome::NoCheck => return Ok(result(BrowserCheckOutcome::NoCheck, 0, None)),
            WindowOutcome::NotPassed | WindowOutcome::NotLoaded => {
                return Ok(result(BrowserCheckOutcome::NotPassed, 0, None));
            }
        }
        let taken = self.import_browser_cookies(
            &challenge.extension_id,
            challenge.url(),
            cookies,
            user_agent,
        )?;
        let taken = u32::try_from(taken).unwrap_or(u32::MAX);
        self.forget_failures(&challenge.extension_id);
        let Some(call) = &challenge.call else {
            return Ok(result(BrowserCheckOutcome::Passed, taken, None));
        };
        Ok(
            match self
                .call(&call.provider, &call.method, call.args.clone())
                .await
            {
                Ok(_) => result(BrowserCheckOutcome::Passed, taken, None),
                Err(error) if error.kind() == Some(IssueKind::BotCheck) => result(
                    BrowserCheckOutcome::StillBlocked,
                    taken,
                    Some(error.to_string()),
                ),
                Err(error) => result(
                    BrowserCheckOutcome::StillFailing,
                    taken,
                    Some(error.to_string()),
                ),
            },
        )
    }

    /// Asks every `music.metadata` provider, a few at a time; each answers with tracks or an
    /// error, the good sources first.
    pub async fn search_music(&self, query: &str, limit: u32) -> Vec<ProviderSearchResult> {
        let providers = self.providers(MUSIC_METADATA).await;
        fanout::collect(self.search_asks(providers, query, limit))
            .await
            .into_iter()
            .map(search_result)
            .collect()
    }

    /// [`Self::search_music`] asking only the `music.metadata` providers of one extension (the
    /// catalogue of a source Raygun streamed a song from).
    pub async fn search_music_of(
        &self,
        extension_id: &str,
        query: &str,
        limit: u32,
    ) -> Vec<ProviderSearchResult> {
        let providers: Vec<ProviderInfo> = self
            .providers(MUSIC_METADATA)
            .await
            .into_iter()
            .filter(|provider| provider.extension_id == extension_id)
            .collect();
        if providers.is_empty() {
            return Vec::new();
        }
        fanout::collect(self.search_asks(providers, query, limit))
            .await
            .into_iter()
            .map(search_result)
            .collect()
    }

    /// [`Self::search_music`] for a search someone watches: each provider's answer goes to
    /// `report` as soon as it comes (with how many providers were asked), so the first answers
    /// show before the slowest provider's. The fan-out's order, limits and resting apply; the
    /// answers come back in the order they came. A provider whose search fails or finds nothing
    /// answers with the songs of its extension's charts that match, as `listings` allows
    /// ([`crate::listing_search`]).
    pub async fn search_music_as_answered(
        &self,
        query: &str,
        limit: u32,
        listings: SearchListingsPrefs,
        report: impl Fn(&ProviderSearchResult, usize),
    ) -> Vec<ProviderSearchResult> {
        let providers = self.providers(MUSIC_METADATA).await;
        let total = providers.len();
        let started = tokio::time::Instant::now();
        let mut answers = std::pin::pin!(
            self.search_asks(providers, query, limit)
                .map(|asked| self.helped_by_listings(asked, query, listings, started))
                .buffer_unordered(total.max(1))
        );
        let mut results = Vec::with_capacity(total);
        while let Some(result) = answers.next().await {
            report(&result, total);
            results.push(result);
        }
        results
    }

    /// A provider's search answer; one that failed or found nothing for what was typed gets the
    /// songs of its extension's charts that match instead, when `listings` says to look.
    async fn helped_by_listings(
        &self,
        asked: Asked<Vec<OnlineTrack>>,
        query: &str,
        listings: SearchListingsPrefs,
        started: tokio::time::Instant,
    ) -> ProviderSearchResult {
        let nothing = asked.answer.as_ref().map_or(true, Vec::is_empty);
        // A source that just said it needs a key isn't asked for its charts either.
        let unset = asked.answer.as_ref().is_err_and(CallError::is_needs_setup);
        if !listings.enabled || !nothing || unset || query.trim().is_empty() {
            return search_result(asked);
        }
        let deadline = (started + waiting_limit())
            .max(tokio::time::Instant::now() + crate::listing_search::LEAST_TIME);
        match self
            .listing_songs(&asked.provider, query, listings, deadline)
            .await
        {
            Ok(tracks) if !tracks.is_empty() => {
                tracing::debug!(
                    provider = %asked.provider.id,
                    found = tracks.len(),
                    "search helped by the source's charts"
                );
                ProviderSearchResult {
                    provider: asked.provider,
                    tracks,
                    error: None,
                    from_listings: true,
                }
            }
            _ => search_result(asked),
        }
    }

    fn search_asks<'a>(
        &'a self,
        providers: Vec<ProviderInfo>,
        query: &str,
        limit: u32,
    ) -> impl futures_util::Stream<Item = Asked<Vec<OnlineTrack>>> + 'a {
        let args = serde_json::json!({ "query": query, "limit": limit });
        fanout::fan_out(
            &self.shared.health,
            providers,
            plan(waiting_limit()),
            move |provider| {
                let args = args.clone();
                async move {
                    let started = std::time::Instant::now();
                    let value = self.call_interactive(&provider.id, "search", args).await?;
                    let tracks = self.tracks(value);
                    tracing::info!(
                        provider = %provider.id,
                        tracks = tracks.len(),
                        elapsed_ms = started.elapsed().as_millis(),
                        "search answered"
                    );
                    Ok(tracks)
                }
            },
        )
    }

    /// Asks every `music.stream` provider for candidates for `query`, a few at a time.
    pub async fn stream_candidates(
        &self,
        query: &TrackQuery,
        limit: u32,
        quality: raygun_resolver::QualityTarget,
    ) -> Vec<ProviderCandidates> {
        let providers = self.providers(MUSIC_STREAM).await;
        let mut args = serde_json::json!({
            "title": query.title,
            "artist": query.artist,
            "album": query.album,
            "durationMs": query.duration_ms,
            "limit": limit,
        });
        // M46.18: providers with several streams per song pick the one nearest this; `null`
        // asks for the best there is.
        if quality != raygun_resolver::QualityTarget::Any {
            args["targetKbps"] = serde_json::json!(quality.kbps());
        }
        let asks = fanout::fan_out(
            &self.shared.health,
            providers,
            plan(waiting_limit()),
            |provider| {
                let args = args.clone();
                async move {
                    self.call_interactive(&provider.id, "candidates", args)
                        .await
                        .map(|value| parse_candidates(&provider, value))
                }
            },
        );
        fanout::collect(asks)
            .await
            .into_iter()
            .map(|asked| ProviderCandidates {
                provider: asked.provider,
                result: asked.answer.map_err(|error| error.to_string()),
            })
            .collect()
    }

    /// Lyrics for `query` from every `lyrics` provider at once: synced lyrics beat plain ones,
    /// then providers keep their order. `Ok(None)` when no provider has any; an error only when
    /// every provider failed.
    pub async fn lyrics(&self, query: &TrackQuery) -> Result<Option<Lyrics>, String> {
        Ok(self.all_lyrics(query).await?.into_iter().next())
    }

    /// Every lyrics provider's lyrics for a song (M46.36: the user picks one), synced ones first.
    pub async fn all_lyrics(&self, query: &TrackQuery) -> Result<Vec<Lyrics>, String> {
        let providers = self.providers(LYRICS).await;
        if providers.is_empty() {
            return Ok(Vec::new());
        }
        let args = serde_json::json!({
            "title": query.title,
            "artist": query.artist,
            "album": query.album,
            "durationMs": query.duration_ms,
        });
        let asks = fanout::fan_out(
            &self.shared.health,
            providers,
            plan(waiting_limit()),
            |provider| {
                let args = args.clone();
                async move { self.call_interactive(&provider.id, "lyrics", args).await }
            },
        );
        let mut found: Vec<Lyrics> = Vec::new();
        let mut errors = Vec::new();
        for asked in fanout::collect(asks).await {
            match asked.answer {
                Ok(value) => found.extend(parse_lyrics(&asked.provider, &value)),
                Err(error) if error.is_missing() || error.is_needs_setup() => {}
                Err(error) => {
                    tracing::warn!(provider = %asked.provider.id, %error, "lyrics provider failed");
                    errors.push(error.to_string());
                }
            }
        }
        if found.is_empty() && !errors.is_empty() {
            return Err(errors.remove(0));
        }
        // Stable sort keeps provider order among equals.
        found.sort_by_key(|lyrics| lyrics.synced.is_empty());
        Ok(found)
    }

    /// Every `music.charts` provider's charts (M46.9), a few at a time; each answers with charts
    /// or an error. Charts' `tag`s are as the provider wrote them.
    pub async fn music_charts(
        &self,
        region: Option<&str>,
    ) -> Vec<(ProviderInfo, Result<Vec<MusicChart>, String>)> {
        let providers = self.providers(MUSIC_CHARTS).await;
        let args = serde_json::json!({ "region": region });
        let asks = fanout::fan_out(
            &self.shared.health,
            providers,
            plan(BACKGROUND_TIMEOUT),
            |provider| {
                let args = args.clone();
                async move {
                    self.call(&provider.id, "charts", args)
                        .await
                        .map(|value| parse_charts(&provider, &value))
                }
            },
        );
        fanout::collect(asks)
            .await
            .into_iter()
            .map(|asked| {
                (
                    asked.provider,
                    asked.answer.map_err(|error| error.to_string()),
                )
            })
            .collect()
    }

    /// Artists named like `query` from every `music.metadata` provider that searches artists
    /// (M46.9); providers without the call are left out.
    pub async fn search_artists(&self, query: &str, limit: u32) -> Vec<MusicArtist> {
        let answers = self.ask_metadata("searchArtists", query, limit).await;
        answers
            .iter()
            .flat_map(|(provider, value)| artists_in(provider, value))
            .collect()
    }

    /// [`Self::search_artists`] for a search someone watches: each provider's artists go to
    /// `report` as they come, with how many providers were asked.
    pub async fn search_artists_as_answered(
        &self,
        query: &str,
        limit: u32,
        report: impl Fn(&ProviderInfo, &[MusicArtist], usize),
    ) -> Vec<MusicArtist> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }
        let args = serde_json::json!({ "query": query, "limit": limit });
        let mut found = Vec::new();
        self.ask_metadata_as_answered("searchArtists", args, |provider, value, total| {
            let artists = value.map_or_else(Vec::new, |value| artists_in(provider, value));
            report(provider, &artists, total);
            found.extend(artists);
        })
        .await;
        found
    }

    /// Albums for `query` from every `music.metadata` provider that searches albums (M46.9).
    pub async fn search_albums(&self, query: &str, limit: u32) -> Vec<MusicAlbum> {
        let answers = self.ask_metadata("searchAlbums", query, limit).await;
        answers
            .iter()
            .flat_map(|(provider, value)| albums_in(provider, value, "albums"))
            .collect()
    }

    /// [`Self::search_albums`] for a search someone watches: each provider's albums go to
    /// `report` as they come, with how many providers were asked.
    pub async fn search_albums_as_answered(
        &self,
        query: &str,
        limit: u32,
        report: impl Fn(&ProviderInfo, &[MusicAlbum], usize),
    ) -> Vec<MusicAlbum> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }
        let args = serde_json::json!({ "query": query, "limit": limit });
        let mut found = Vec::new();
        self.ask_metadata_as_answered("searchAlbums", args, |provider, value, total| {
            let albums = value.map_or_else(Vec::new, |value| albums_in(provider, value, "albums"));
            report(provider, &albums, total);
            found.extend(albums);
        })
        .await;
        found
    }

    /// An artist's releases (M46.36: albums, singles, EPs...) from every `music.metadata` provider
    /// that lists them (`artistReleases`, asked with the artist's name as `query`).
    pub async fn artist_releases(&self, name: &str, limit: u32) -> Vec<MusicAlbum> {
        let answers = self.ask_metadata("artistReleases", name, limit).await;
        answers
            .iter()
            .flat_map(|(provider, value)| albums_in(provider, value, "releases"))
            .collect()
    }

    /// Playlists for `query` from every `music.metadata` provider that searches them (M46.39);
    /// `featured` asks for the services' own playlists only.
    pub async fn search_playlists(
        &self,
        query: &str,
        featured: bool,
        limit: u32,
    ) -> Vec<MusicPlaylist> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }
        let providers = self.providers(MUSIC_METADATA).await;
        let args = serde_json::json!({ "query": query, "limit": limit, "featured": featured });
        let asks = fanout::fan_out(
            &self.shared.health,
            providers,
            plan(waiting_limit()),
            |provider| {
                let args = args.clone();
                async move {
                    self.call_interactive(&provider.id, "searchPlaylists", args)
                        .await
                }
            },
        );
        fanout::collect(asks)
            .await
            .into_iter()
            .filter_map(|asked| match asked.answer {
                Ok(value) => Some((asked.provider, value)),
                Err(error) => {
                    log_unless_missing(&asked.provider, "searchPlaylists", &error);
                    None
                }
            })
            .flat_map(|(provider, value)| {
                value
                    .get("playlists")
                    .and_then(serde_json::Value::as_array)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(move |playlist| parse_playlist(&provider, &playlist))
            })
            .filter(|playlist| !featured || playlist.featured)
            .collect()
    }

    /// [`Self::search_playlists`] for a search someone watches: each provider's playlists go to
    /// `report` as they come, with how many providers were asked.
    pub async fn search_playlists_as_answered(
        &self,
        query: &str,
        featured: bool,
        limit: u32,
        report: impl Fn(&ProviderInfo, &[MusicPlaylist], usize),
    ) -> Vec<MusicPlaylist> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }
        let args = serde_json::json!({ "query": query, "limit": limit, "featured": featured });
        let mut found = Vec::new();
        self.ask_metadata_as_answered("searchPlaylists", args, |provider, value, total| {
            let playlists =
                value.map_or_else(Vec::new, |value| playlists_in(provider, value, featured));
            report(provider, &playlists, total);
            found.extend(playlists);
        })
        .await;
        found
    }

    /// Asks every `music.metadata` provider for `method` at once, handing each one's answer to
    /// `report` as it comes (`None` when it failed or lacks the call), with how many were asked.
    async fn ask_metadata_as_answered(
        &self,
        method: &str,
        args: serde_json::Value,
        mut report: impl FnMut(&ProviderInfo, Option<&serde_json::Value>, usize),
    ) {
        let providers = self.providers(MUSIC_METADATA).await;
        let total = providers.len();
        let mut answers = std::pin::pin!(fanout::fan_out(
            &self.shared.health,
            providers,
            plan(waiting_limit()),
            |provider| {
                let args = args.clone();
                async move { self.call_interactive(&provider.id, method, args).await }
            },
        ));
        while let Some(asked) = answers.next().await {
            if let Err(error) = &asked.answer {
                log_unless_missing(&asked.provider, method, error);
            }
            report(&asked.provider, asked.answer.as_ref().ok(), total);
        }
    }

    /// An artist's coming events (M46.42) from every `music.metadata` provider that lists them
    /// (`artistEvents`, asked with the artist's name as `query`).
    pub async fn artist_events(&self, name: &str, limit: u32) -> Vec<ArtistEvent> {
        let answers = self.ask_metadata("artistEvents", name, limit).await;
        answers
            .iter()
            .flat_map(|(provider, value)| events_in(provider, value))
            .collect()
    }

    /// The `music.metadata` provider `provider_id`, if it's one searches may use.
    async fn metadata_provider(&self, provider_id: &str) -> Result<ProviderInfo, CallError> {
        self.providers(MUSIC_METADATA)
            .await
            .into_iter()
            .find(|provider| provider.id == provider_id)
            .ok_or_else(|| CallError::UnknownProvider(provider_id.to_owned()))
    }

    /// A playlist's songs from the provider that has it (M46.39).
    pub async fn music_playlist(
        &self,
        provider_id: &str,
        playlist_id: &str,
    ) -> Result<MusicPlaylistDetail, CallError> {
        let provider = self.metadata_provider(provider_id).await?;
        let value = self
            .call(
                provider_id,
                "playlist",
                serde_json::json!({ "id": playlist_id }),
            )
            .await?;
        let playlist = value
            .get("playlist")
            .and_then(|playlist| parse_playlist(&provider, playlist))
            .ok_or_else(|| site_changed(&provider, "its answer had no playlist"))?;
        Ok(MusicPlaylistDetail {
            playlist,
            tracks: self.tracks(value),
        })
    }

    /// An album's songs from the provider that knows it (M46.9).
    pub async fn music_album(
        &self,
        provider_id: &str,
        album_id: &str,
    ) -> Result<MusicAlbumDetail, CallError> {
        let provider = self.metadata_provider(provider_id).await?;
        let value = self
            .call(provider_id, "album", serde_json::json!({ "id": album_id }))
            .await?;
        let album = value
            .get("album")
            .and_then(|album| parse_album(&provider, album))
            .ok_or_else(|| site_changed(&provider, "its answer had no album"))?;
        Ok(MusicAlbumDetail {
            album,
            tracks: self.tracks(value),
        })
    }

    /// Asks every `music.metadata` provider that has `method`, a few at a time; answers from
    /// providers without it, or that failed, are dropped (failures are logged).
    async fn ask_metadata(
        &self,
        method: &str,
        query: &str,
        limit: u32,
    ) -> Vec<(ProviderInfo, serde_json::Value)> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }
        let providers = self.providers(MUSIC_METADATA).await;
        let args = serde_json::json!({ "query": query, "limit": limit });
        let asks = fanout::fan_out(
            &self.shared.health,
            providers,
            plan(waiting_limit()),
            |provider| {
                let args = args.clone();
                async move { self.call_interactive(&provider.id, method, args).await }
            },
        );
        fanout::collect(asks)
            .await
            .into_iter()
            .filter_map(|asked| match asked.answer {
                Ok(value) => Some((asked.provider, value)),
                Err(error) => {
                    log_unless_missing(&asked.provider, method, &error);
                    None
                }
            })
            .collect()
    }

    /// Songs for a genre or a mood from every `music.charts` provider that has `tagSongs`
    /// (M46.19), a few at a time. Providers without the call are left out; each other answers
    /// with `(row name, songs)` or an error. `aliases` are the vocabulary's other spellings.
    pub async fn music_tag_songs(
        &self,
        tag: &str,
        aliases: &[String],
        kind: &str,
        region: Option<&str>,
        limit: u32,
    ) -> Vec<(
        ProviderInfo,
        Result<(Option<String>, Vec<OnlineTrack>), String>,
    )> {
        let providers = self.providers(MUSIC_CHARTS).await;
        let args = serde_json::json!({
            "tag": tag,
            "aliases": aliases,
            "kind": kind,
            "region": region,
            "limit": limit,
        });
        let asks = fanout::fan_out(
            &self.shared.health,
            providers,
            plan(waiting_limit()),
            |provider| {
                let args = args.clone();
                async move { self.call_interactive(&provider.id, "tagSongs", args).await }
            },
        );
        fanout::collect(asks)
            .await
            .into_iter()
            .filter_map(|asked| match asked.answer {
                Ok(value) => {
                    let name = value
                        .get("name")
                        .and_then(serde_json::Value::as_str)
                        .map(|name| name.trim().to_owned())
                        .filter(|name| !name.is_empty());
                    Some((asked.provider, Ok((name, self.tracks(value)))))
                }
                Err(error) if error.is_missing() => None,
                Err(error) => Some((asked.provider, Err(error.to_string()))),
            })
            .collect()
    }

    /// Providers of `kind` to ask one after another until one knows: the good ones first, and
    /// none that sits out after failing again and again.
    async fn in_turn(&self, kind: &str) -> Vec<ProviderInfo> {
        let health = &self.shared.health;
        health
            .order(self.providers(kind).await)
            .into_iter()
            .filter(|provider| health.resting(&provider.id).is_none())
            .collect()
    }

    /// A call of a provider asked in turn, noted in its health.
    async fn ask_in_turn(
        &self,
        provider: &ProviderInfo,
        method: &str,
        args: serde_json::Value,
        limit: Duration,
    ) -> Result<serde_json::Value, CallError> {
        let started = std::time::Instant::now();
        let answer = self.call_within(&provider.id, method, args, limit).await;
        self.shared
            .health
            .record(&provider.id, answer.as_ref().map(|_| ()), started.elapsed());
        answer
    }

    /// Artists like `artist` from the first `music.recommend` provider that knows some (M46.20),
    /// the closest first.
    pub async fn similar_artists(&self, artist: &str, limit: u32) -> Vec<String> {
        let args = serde_json::json!({ "artist": artist, "limit": limit });
        for provider in self.in_turn(MUSIC_RECOMMEND).await {
            match self
                .ask_in_turn(
                    &provider,
                    "similarArtists",
                    args.clone(),
                    BACKGROUND_TIMEOUT,
                )
                .await
            {
                Ok(value) => {
                    let names = artist_names(&value);
                    if !names.is_empty() {
                        return names;
                    }
                }
                Err(error) => log_unless_missing(&provider, "similarArtists", &error),
            }
        }
        Vec::new()
    }

    /// Songs from the first `music.recommend` provider whose `method` (`topTracks`,
    /// `newReleases` or `radio`) has some for `args` (M46.20).
    pub async fn recommended_tracks(
        &self,
        method: &str,
        args: serde_json::Value,
    ) -> Vec<OnlineTrack> {
        for provider in self.in_turn(MUSIC_RECOMMEND).await {
            match self
                .ask_in_turn(&provider, method, args.clone(), BACKGROUND_TIMEOUT)
                .await
            {
                Ok(value) => {
                    let tracks = self.tracks(value);
                    if !tracks.is_empty() {
                        return tracks;
                    }
                }
                Err(error) => log_unless_missing(&provider, method, &error),
            }
        }
        Vec::new()
    }

    /// A chart's songs from the provider that keeps it (M46.9).
    pub async fn music_chart(
        &self,
        provider_id: &str,
        chart_id: &str,
        region: Option<&str>,
        limit: u32,
    ) -> Result<Vec<OnlineTrack>, CallError> {
        let args = serde_json::json!({ "id": chart_id, "region": region, "limit": limit });
        self.call(provider_id, "chart", args)
            .await
            .map(|value| self.tracks(value))
    }

    /// Every `music.tags` provider's genres for a song (M46.9), a few at a time: `(name, weight)`
    /// pairs as each wrote them, weights between 0 and 1 (1 when a provider gives none).
    pub async fn music_tags(
        &self,
        query: &TrackQuery,
    ) -> Vec<(ProviderInfo, Result<Vec<(String, f64)>, String>)> {
        let providers = self.providers(MUSIC_TAGS).await;
        let args = serde_json::json!({
            "title": query.title,
            "artist": query.artist,
            "album": query.album,
            "durationMs": query.duration_ms,
        });
        let asks = fanout::fan_out(
            &self.shared.health,
            providers,
            plan(BACKGROUND_TIMEOUT),
            |provider| {
                let args = args.clone();
                async move {
                    self.call(&provider.id, "tags", args)
                        .await
                        .map(|value| parse_tags(&value))
                }
            },
        );
        fanout::collect(asks)
            .await
            .into_iter()
            .map(|asked| {
                (
                    asked.provider,
                    asked.answer.map_err(|error| error.to_string()),
                )
            })
            .collect()
    }

    /// What the first `music.info` provider that knows the artist says about them (M46.14).
    pub async fn artist_info(&self, name: &str) -> Option<raygun_model::ArtistInfo> {
        for provider in self.in_turn(MUSIC_INFO).await {
            let answer = self
                .ask_in_turn(
                    &provider,
                    "artist",
                    serde_json::json!({ "name": name }),
                    waiting_limit(),
                )
                .await;
            match answer {
                Ok(value) => {
                    if let Some(info) = parse_artist_info(&value, &provider.name) {
                        return Some(info);
                    }
                }
                Err(error) => {
                    tracing::debug!(provider = %provider.id, %error, "no artist info");
                }
            }
        }
        None
    }

    /// Who made a song, from the first `music.info` provider that knows it (M46.14).
    pub async fn song_credits(&self, query: &TrackQuery) -> Option<raygun_model::SongCredits> {
        let args = serde_json::json!({
            "title": query.title,
            "artist": query.artist,
            "album": query.album,
            "durationMs": query.duration_ms,
        });
        for provider in self.in_turn(MUSIC_INFO).await {
            match self
                .ask_in_turn(&provider, "credits", args.clone(), waiting_limit())
                .await
            {
                Ok(value) => {
                    if let Some(credits) = parse_credits(&value, &provider.name) {
                        return Some(credits);
                    }
                }
                Err(error) => tracing::debug!(provider = %provider.id, %error, "no credits"),
            }
        }
        None
    }

    /// Asks a provider for the stream behind one of its candidates, for `using` (M46.51). A play
    /// or a download waits on it, so it gives up after [`waiting_limit`].
    pub async fn resolve_stream(
        &self,
        provider_id: &str,
        candidate_id: &str,
        using: StreamUse,
    ) -> Result<ResolvedStream, CallError> {
        let mut args = serde_json::json!({ "id": candidate_id, "decodable": using.decodable });
        if using.quality != raygun_resolver::QualityTarget::Any {
            args["targetKbps"] = serde_json::json!(using.quality.kbps());
        }
        let value = self
            .call_within(provider_id, "resolve", args, waiting_limit())
            .await?;
        let source = value
            .get("source")
            .cloned()
            .and_then(|source| serde_json::from_value::<MediaSource>(source).ok())
            .filter(allowed_source)
            .ok_or_else(|| {
                CallError::Failed(Issue::new(
                    IssueKind::NotFound,
                    self.provider_name(provider_id),
                    "it gave no playable stream",
                ))
            })?;
        let expires_at = value.get("expiresAt").and_then(serde_json::Value::as_f64);
        Ok(ResolvedStream {
            source,
            expires_at: expires_at.filter(|ms| ms.is_finite()).map(|ms| ms as i64),
        })
    }

    /// A YouTube video's own audio file from Raygun's YouTube module (`raygun.youtube`), asked
    /// through `extension_id`'s own fetch, so its permissions and cookies apply. The module checks
    /// each file at its last bytes, so the files of app clients that stop after their first
    /// megabyte are passed over: downloads ask it when a YouTube source is installed.
    pub async fn youtube_audio(
        &self,
        extension_id: &str,
        video: &str,
        using: StreamUse,
        limit: Duration,
    ) -> Result<ResolvedStream, CallError> {
        let lane = self
            .directory()
            .lanes
            .get(extension_id)
            .copied()
            .ok_or_else(|| CallError::UnknownProvider(extension_id.to_owned()))?;
        let prefs = crate::youtube::StreamPrefs {
            target_kbps: using.quality.kbps().map(f64::from),
            decodable: using.decodable,
        };
        let (reply, answer) = oneshot::channel();
        self.shared.lanes[lane]
            .send(Request::YouTubeAudio {
                extension: extension_id.to_owned(),
                video: video.to_owned(),
                prefs,
                reply,
            })
            .map_err(|_| CallError::HostStopped)?;
        let name = || "YouTube".to_owned();
        let stream = match tokio::time::timeout(limit, answer).await {
            Err(_) => {
                return Err(CallError::Timeout {
                    name: name(),
                    after: limit,
                });
            }
            Ok(Err(_)) => return Err(CallError::HostStopped),
            Ok(Ok(Err(message))) => {
                return Err(CallError::Said {
                    name: name(),
                    message,
                });
            }
            Ok(Ok(Ok(stream))) => stream,
        };
        Ok(ResolvedStream {
            source: MediaSource::Url {
                url: stream.url,
                headers: stream.headers,
            },
            expires_at: stream.expires_at,
        })
    }
}

/// One source's songs, or why it had none.
fn search_result(asked: Asked<Vec<OnlineTrack>>) -> ProviderSearchResult {
    match asked.answer {
        Ok(tracks) => ProviderSearchResult {
            provider: asked.provider,
            tracks,
            error: None,
            from_listings: false,
        },
        Err(error) => {
            tracing::warn!(provider = %asked.provider.id, %error, "search failed");
            ProviderSearchResult {
                provider: asked.provider,
                tracks: Vec::new(),
                error: Some(error.to_string()),
                from_listings: false,
            }
        }
    }
}

/// A provider's answer that lacks what it should hold: the site (or its API) changed.
fn site_changed(provider: &ProviderInfo, detail: &str) -> CallError {
    CallError::Failed(Issue::new(IssueKind::SiteChanged, &provider.name, detail))
}

/// Every source's manifest read, installed copies chosen over built-in ones with the same id;
/// sources whose manifest can't be read come back as failed extensions.
fn choose(
    sources: Vec<ExtensionSource>,
) -> (
    BTreeMap<String, (ExtensionSource, Manifest)>,
    Vec<ExtensionInfo>,
) {
    let mut chosen: BTreeMap<String, (ExtensionSource, Manifest)> = BTreeMap::new();
    let mut infos = Vec::new();
    for source in sources {
        let manifest = source
            .read(MANIFEST_FILE)
            .and_then(|text| Manifest::parse(&text).map_err(|error| error.to_string()));
        match manifest {
            Ok(manifest) => {
                let replace = chosen.get(&manifest.id).is_none_or(|(existing, _)| {
                    source.origin == ExtensionOrigin::Installed
                        || existing.origin == ExtensionOrigin::BuiltIn
                });
                if replace {
                    chosen.insert(manifest.id.clone(), (source, manifest));
                }
            }
            Err(error) => {
                let label = source.label();
                tracing::warn!(extension = %label, %error, "extension manifest rejected");
                infos.push(failed_info(&label, &label, "", source.origin, error));
            }
        }
    }
    (chosen, infos)
}

/// A loaded extension: JavaScript, a Shosetsu script (M32), a connected server, or scripts for
/// websites (`crate::site`).
enum Extension {
    Js(JsExtension),
    Shosetsu(Box<ShosetsuExtension>),
    Server(Box<ServerExtension>),
    Site(Box<crate::site::LoadedSite>),
}

impl Extension {
    fn manifest(&self) -> &Manifest {
        match self {
            Self::Js(extension) => extension.manifest(),
            Self::Shosetsu(extension) => extension.manifest(),
            Self::Server(extension) => extension.manifest(),
            Self::Site(extension) => &extension.manifest,
        }
    }

    fn user_settings(&self) -> &BTreeMap<String, serde_json::Value> {
        match self {
            Self::Js(extension) => extension.user_settings(),
            Self::Shosetsu(extension) => extension.user_settings(),
            Self::Server(extension) => extension.user_settings(),
            Self::Site(extension) => &extension.user_settings,
        }
    }

    /// Its settings: the manifest's, and for a Miru script those it registered.
    fn settings(&self) -> Vec<manifest::SettingDef> {
        match self {
            Self::Js(extension) => extension.settings(),
            Self::Shosetsu(extension) => extension.manifest().settings.clone(),
            Self::Server(extension) => extension.manifest().settings.clone(),
            Self::Site(extension) => extension.manifest.settings.clone(),
        }
    }

    fn providers(&self) -> Vec<ProviderInfo> {
        match self {
            Self::Js(extension) => extension.providers(),
            Self::Shosetsu(extension) => extension.providers(),
            Self::Server(extension) => extension.providers(),
            Self::Site(extension) => extension.providers(),
        }
    }

    /// The manifest's website, else a Shosetsu script's `baseURL`.
    fn website(&self) -> Option<String> {
        let declared = self.manifest().website.clone();
        match self {
            Self::Js(_) | Self::Server(_) | Self::Site(_) => declared,
            Self::Shosetsu(extension) => {
                declared.or_else(|| extension.website().map(str::to_owned))
            }
        }
    }

    async fn call(
        &self,
        local_id: &str,
        method: &str,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, Failure> {
        match self {
            Self::Js(extension) => extension.call(local_id, method, args).await,
            Self::Shosetsu(extension) => extension.call_reporting(local_id, method, args).await,
            Self::Server(extension) => extension.call(local_id, method, args).await,
            Self::Site(_) => Err(Failure::said(
                "site scripts run in Raygun's websites browser and answer no calls",
            )),
        }
    }
}

/// One lane: the extensions on it.
struct LaneState {
    config: HostConfig,
    loaded: RefCell<BTreeMap<String, Rc<Extension>>>,
    /// Where calls that fail on a browser check are noted (the host's).
    challenges: Arc<Challenges>,
    /// Where the heaps of loaded JavaScript extensions are noted (the host's).
    heaps: Heaps,
}

fn run_lane(
    config: HostConfig,
    mut receiver: mpsc::UnboundedReceiver<Request>,
    challenges: Arc<Challenges>,
    heaps: Heaps,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            tracing::error!(%error, "an extension lane could not start");
            return;
        }
    };
    let local = tokio::task::LocalSet::new();
    local.block_on(&runtime, async move {
        let state = Rc::new(LaneState {
            config,
            loaded: RefCell::new(BTreeMap::new()),
            challenges,
            heaps,
        });
        while let Some(request) = receiver.recv().await {
            let state = state.clone();
            tokio::task::spawn_local(async move { state.handle(request).await });
        }
    });
}

impl LaneState {
    async fn handle(&self, request: Request) {
        match request {
            Request::Load { items, reply } => {
                let infos = self.load(items).await;
                let _ = reply.send(infos);
            }
            Request::Settings { reply } => {
                // A Miru script (or an LNReader plugin) registers settings while it sets up, which
                // may end after loading.
                let settings = self
                    .loaded
                    .borrow()
                    .iter()
                    .filter(|(_, extension)| extension.manifest().runtime.declares_settings())
                    .map(|(id, extension)| (id.clone(), settings_info(extension)))
                    .collect();
                let _ = reply.send(settings);
            }
            Request::Call {
                provider,
                method,
                args,
                mut reply,
            } => {
                // The caller gone (a search left or given up) stops the call, and with it the
                // extension's requests once none of its calls is left.
                tokio::select! {
                    answer = self.call(&provider, &method, &args) => {
                        let _ = reply.send(answer);
                    }
                    () = reply.closed() => {
                        tracing::debug!(%provider, %method, "call stopped: nobody waits for it");
                    }
                }
            }
            Request::YouTubeAudio {
                extension,
                video,
                prefs,
                mut reply,
            } => {
                let loaded = self.loaded.borrow().get(&extension).cloned();
                let answer = async {
                    match loaded.as_deref() {
                        Some(Extension::Js(js)) => js.youtube_stream(&video, prefs).await,
                        Some(_) => Err("This source can't ask YouTube".to_owned()),
                        None => Err("That source isn't installed".to_owned()),
                    }
                };
                tokio::select! {
                    answer = answer => {
                        let _ = reply.send(answer);
                    }
                    () = reply.closed() => {}
                }
            }
        }
    }

    async fn load(&self, items: Vec<LoadItem>) -> Vec<ExtensionInfo> {
        // The old ones go first, so two sets of runtimes never take memory at once.
        self.loaded.borrow_mut().clear();
        let mut scripts = Vec::new();
        let mut others = Vec::new();
        for LoadItem {
            source,
            manifest,
            settings,
            secret,
        } in items
        {
            let id = manifest.id.clone();
            let origin = source.origin;
            let (name, version) = (manifest.name.clone(), manifest.version.to_string());
            // A conversion that fails still says which converter made it, so a newer one is
            // offered (M46.9.1).
            let compatibility = manifest.nuclear.as_ref().and_then(compat_report);
            let shosetsu = manifest.runtime == Runtime::Shosetsu;
            let load = async move {
                let result = self.load_one(source, manifest, settings, secret).await;
                (id, origin, name, version, compatibility, result)
            };
            if shosetsu {
                scripts.push(load);
            } else {
                others.push(load);
            }
        }
        // Shosetsu scripts load on threads of their own, so they all start at once; JavaScript
        // extensions share the lane and load one after another.
        let (mut results, rest) =
            futures_util::future::join(futures_util::future::join_all(scripts), async {
                let mut done = Vec::new();
                for load in others {
                    done.push(load.await);
                }
                done
            })
            .await;
        results.extend(rest);
        let mut infos = Vec::new();
        for (id, origin, name, version, compatibility, result) in results {
            match result {
                Ok(extension) => {
                    let info = loaded_info(&extension, origin);
                    tracing::info!(extension = %id, providers = info.providers.len(), "extension loaded");
                    infos.push(info);
                    if let Extension::Js(js) = &extension {
                        self.heaps
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .insert(id.clone(), (js.heap().clone(), js.memory_limit()));
                    }
                    self.loaded.borrow_mut().insert(id, Rc::new(extension));
                }
                Err(error) => {
                    tracing::warn!(extension = %id, %error, "extension failed to load");
                    infos.push(ExtensionInfo {
                        compatibility,
                        ..failed_info(&id, &name, &version, origin, error)
                    });
                }
            }
        }
        infos
    }

    async fn load_one(
        &self,
        source: ExtensionSource,
        manifest: Manifest,
        mut user_settings: BTreeMap<String, serde_json::Value>,
        secret: Option<String>,
    ) -> Result<Extension, String> {
        if manifest.runtime == Runtime::Server {
            user_settings.retain(|key, value| manifest.check_setting(key, value).is_ok());
            let data_dir = self
                .config
                .storage_dir
                .as_ref()
                .map(|dir| dir.join(&manifest.id));
            return ServerExtension::load(manifest, &source, secret, user_settings, data_dir)
                .await
                .map(|extension| Extension::Server(Box::new(extension)));
        }
        if let Some(scripts) = manifest.site.as_ref() {
            // Nothing runs here: its files only need to be there for the browser.
            for path in scripts.files() {
                source.read(path)?;
            }
            user_settings.retain(|key, value| manifest.check_setting(key, value).is_ok());
            return Ok(Extension::Site(Box::new(crate::site::LoadedSite {
                manifest,
                user_settings,
            })));
        }
        let data_dir = self
            .config
            .storage_dir
            .as_ref()
            .map(|dir| dir.join(&manifest.id));
        let cookies = CookieJar::open(data_dir.as_ref().map(|dir| dir.join("cookies.json")));
        if manifest.runtime == Runtime::Shosetsu {
            // A script's settings come from the script itself, so they're checked there.
            let load = ShosetsuExtension::load(manifest, source, cookies, user_settings);
            return tokio::time::timeout(self.config.call_timeout, load)
                .await
                .map_err(|_| "loading timed out".to_owned())?
                .map(|extension| Extension::Shosetsu(Box::new(extension)));
        }
        // A Miru script, an LNReader plugin or a Nuclear plugin declares settings as it runs:
        // its values are checked when listed.
        if !manifest.registers_settings() {
            user_settings.retain(|key, value| manifest.check_setting(key, value).is_ok());
        }
        let code = source.read(&manifest.entry)?;
        let storage = KvStore::open(data_dir.map(|dir| dir.join("storage.json")));
        let load = JsExtension::load(
            manifest,
            code,
            storage,
            cookies,
            user_settings,
            &self.config.limits,
            self.config.content.clone(),
        );
        tokio::time::timeout(self.config.call_timeout, load)
            .await
            .map_err(|_| "loading timed out".to_owned())?
            .map(Extension::Js)
    }

    async fn call(
        &self,
        provider: &str,
        method: &str,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, CallError> {
        let unknown = || CallError::UnknownProvider(provider.to_owned());
        let (extension_id, local_id) = provider.split_once('/').ok_or_else(unknown)?;
        let extension = self
            .loaded
            .borrow()
            .get(extension_id)
            .cloned()
            .ok_or_else(unknown)?;
        let name = extension
            .providers()
            .into_iter()
            .find(|registered| registered.id == provider)
            .map(|registered| registered.name)
            .ok_or_else(unknown)?;
        // Shosetsu scripts block on each request and may page through long chapter lists.
        let timeout = match extension.as_ref() {
            Extension::Js(_) | Extension::Server(_) | Extension::Site(_) => {
                self.config.call_timeout
            }
            Extension::Shosetsu(_) => self
                .config
                .call_timeout
                .max(crate::shosetsu::runtime::CALL_BUDGET + Duration::from_secs(5)),
        };
        match tokio::time::timeout(timeout, extension.call(local_id, method, args)).await {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(mut failure)) => {
                let request = failure.checked.take().map(|request| *request);
                let error = failure::into_error(failure, &name, provider, method);
                if error.kind() == Some(IssueKind::BotCheck) {
                    // A check to pass by hand (`crate::browser_check`): the request that met it,
                    // and the call, to ask again.
                    let website = extension.website();
                    self.challenges.note(
                        error.kind(),
                        Met {
                            extension_id,
                            source: &name,
                            detail: &error.detail(),
                            website: website.as_deref(),
                            request,
                            call: Some(FailedCall {
                                provider: provider.to_owned(),
                                method: method.to_owned(),
                                args: args.clone(),
                            }),
                        },
                    );
                }
                Err(error)
            }
            Err(_) => Err(CallError::Timeout {
                name,
                after: timeout,
            }),
        }
    }
}

/// An extension's settings with their values; a stored value that no longer fits (a choice the
/// extension dropped) reads as the default.
fn settings_info(extension: &Extension) -> Vec<ExtensionSetting> {
    let values = extension.user_settings();
    extension
        .settings()
        .iter()
        .map(|setting| {
            let default = setting.default.as_ref().and_then(SettingValue::from_json);
            ExtensionSetting {
                key: setting.key.clone(),
                label: setting
                    .label
                    .clone()
                    .unwrap_or_else(|| key_label(&setting.key)),
                kind: match setting.kind {
                    manifest::SettingKind::Text => SettingKind::Text,
                    manifest::SettingKind::Number => SettingKind::Number,
                    manifest::SettingKind::Boolean => SettingKind::Boolean,
                    manifest::SettingKind::Select => SettingKind::Select,
                },
                options: setting.options.clone(),
                value: values
                    .get(&setting.key)
                    .filter(|value| setting.accepts(value))
                    .and_then(SettingValue::from_json)
                    .or_else(|| default.clone()),
                default,
                required: setting.required,
            }
        })
        .collect()
}

fn loaded_info(extension: &Extension, origin: ExtensionOrigin) -> ExtensionInfo {
    let manifest = extension.manifest();
    let settings = settings_info(extension);
    ExtensionInfo {
        needs_setup: setup_need(extension, &settings),
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        version: manifest.version.to_string(),
        description: manifest.description.clone(),
        content_rating: manifest.content_rating.as_str().to_owned(),
        adult: manifest.content_rating.as_str() == "nsfw",
        origin,
        status: ExtensionStatus::Loaded,
        error: None,
        provides: manifest.provides.clone(),
        net: manifest
            .net
            .patterns()
            .iter()
            .map(ToString::to_string)
            .collect(),
        providers: extension
            .providers()
            .into_iter()
            .map(|provider| ProviderInfo {
                adult: manifest.content_rating.as_str() == "nsfw",
                ..provider
            })
            .collect(),
        compatibility: manifest.nuclear.as_ref().and_then(compat_report),
        icon: None,
        repository: None,
        website: extension.website(),
        settings,
        sites: manifest
            .site
            .as_ref()
            .map(crate::site::SiteScripts::patterns)
            .unwrap_or_default(),
    }
}

/// The report the Nuclear converter stored in the manifest (`nuclear: { package, report }`).
fn compat_report(nuclear: &serde_json::Value) -> Option<raygun_model::CompatReport> {
    let report = nuclear.get("report")?;
    let list = |key: &str| -> Vec<String> {
        report
            .get(key)
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };
    Some(raygun_model::CompatReport {
        package: nuclear.get("package")?.as_str()?.to_owned(),
        converter: nuclear
            .get("converter")
            .and_then(serde_json::Value::as_u64)
            .and_then(|version| u32::try_from(version).ok())
            .unwrap_or(1),
        supported: list("supported"),
        unsupported_categories: list("unsupportedCategories"),
        stubbed_apis: list("stubbedApis"),
        ui_imports: list("uiImports"),
    })
}

/// An extension that didn't load; its error says so first, then why (for More info).
fn failed_info(
    id: &str,
    name: &str,
    version: &str,
    origin: ExtensionOrigin,
    error: String,
) -> ExtensionInfo {
    ExtensionInfo {
        id: id.to_owned(),
        name: name.to_owned(),
        version: version.to_owned(),
        description: None,
        content_rating: "safe".to_owned(),
        adult: false,
        origin,
        status: ExtensionStatus::Failed,
        error: Some(raygun_net::issue::message(
            &format!("{name} couldn't start"),
            &error,
        )),
        provides: Vec::new(),
        net: Vec::new(),
        providers: Vec::new(),
        settings: Vec::new(),
        compatibility: None,
        icon: None,
        repository: None,
        website: None,
        needs_setup: None,
        sites: Vec::new(),
    }
}

/// A short text field: trimmed, at most `limit` characters, none when empty.
fn text_field(value: &serde_json::Value, key: &str, limit: usize) -> Option<String> {
    let text = value.get(key)?.as_str()?.trim();
    if text.is_empty() {
        return None;
    }
    Some(text.chars().take(limit).collect())
}

/// An http(s) link field.
fn link_field(value: &serde_json::Value, key: &str) -> Option<String> {
    text_field(value, key, 2048).filter(|url| is_web_url(url))
}

/// An artist's years as shown: "1980" while still active (sources write "1980–"), "1980–2003";
/// none without a year.
fn active_years(info: &serde_json::Value) -> Option<String> {
    let years = raygun_model::year_range_label(&text_field(info, "active", 60)?);
    years.chars().any(char::is_numeric).then_some(years)
}

pub(crate) fn parse_artist_info(
    value: &serde_json::Value,
    provider: &str,
) -> Option<raygun_model::ArtistInfo> {
    let info = value.get("artist").unwrap_or(value);
    let name = text_field(info, "name", 200)?;
    Some(raygun_model::ArtistInfo {
        name,
        kind: text_field(info, "kind", 40),
        bio: text_field(info, "bio", 4000),
        image_url: link_field(info, "imageUrl"),
        url: link_field(info, "url"),
        origin: text_field(info, "origin", 200),
        active: active_years(info),
        members: info
            .get("members")
            .and_then(serde_json::Value::as_array)
            .map(|members| {
                members
                    .iter()
                    .filter_map(|member| member.as_str().map(str::trim))
                    .filter(|member| !member.is_empty())
                    .take(40)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        provider_name: provider.to_owned(),
    })
}

pub(crate) fn parse_credits(
    value: &serde_json::Value,
    provider: &str,
) -> Option<raygun_model::SongCredits> {
    let groups = value.get("credits")?.as_array()?;
    let credits: Vec<raygun_model::CreditGroup> = groups
        .iter()
        .filter_map(|group| {
            let role = text_field(group, "role", 80)?;
            let names: Vec<String> = group
                .get("names")?
                .as_array()?
                .iter()
                .filter_map(|name| name.as_str().map(str::trim))
                .filter(|name| !name.is_empty())
                .take(40)
                .map(str::to_owned)
                .collect();
            (!names.is_empty()).then_some(raygun_model::CreditGroup { role, names })
        })
        .take(40)
        .collect();
    if credits.is_empty() {
        return None;
    }
    Some(raygun_model::SongCredits {
        credits,
        url: link_field(value, "url"),
        provider_name: provider.to_owned(),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTrack {
    #[serde(default)]
    id: Option<serde_json::Value>,
    title: String,
    #[serde(default)]
    artist: Option<String>,
    #[serde(default)]
    album: Option<String>,
    #[serde(default)]
    duration_ms: Option<f64>,
    #[serde(default)]
    artwork_url: Option<String>,
    #[serde(default)]
    source: Option<MediaSource>,
    #[serde(default)]
    explicit: Option<bool>,
}

fn is_web_url(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
}

/// Reads `{ tracks: [...] }` leniently: malformed entries are skipped, not fatal. Extensions may
/// only hand out web streams and YouTube ids, never local files.
pub(crate) fn parse_tracks(value: serde_json::Value) -> Vec<OnlineTrack> {
    let Some(tracks) = value.get("tracks").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    tracks
        .iter()
        .filter_map(|track| RawTrack::deserialize(track).ok())
        .filter(|track| !track.title.trim().is_empty())
        .map(|track| OnlineTrack {
            external_id: track.id.map(|id| match id {
                serde_json::Value::String(id) => id,
                other => other.to_string(),
            }),
            title: track.title.trim().to_owned(),
            // Without the quotes some sources leave around a name.
            artist: track
                .artist
                .map(|artist| raygun_resolver::tidy_credit(&artist))
                .filter(|artist| !artist.is_empty()),
            album: track.album.filter(|album| !album.trim().is_empty()),
            duration_ms: track
                .duration_ms
                .filter(|ms| ms.is_finite() && *ms > 0.0 && *ms < f64::from(u32::MAX))
                .map(|ms| ms as u32),
            artwork_url: track.artwork_url.filter(|url| is_web_url(url)),
            source: track.source.filter(allowed_source),
            explicit: track.explicit.unwrap_or(false),
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawChart {
    id: serde_json::Value,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    artwork_url: Option<String>,
    #[serde(default)]
    tag: Option<String>,
    #[serde(default)]
    group: Option<String>,
}

/// Logs a failed optional call, unless the provider just doesn't have it.
fn log_unless_missing(provider: &ProviderInfo, method: &str, error: &CallError) {
    if !error.is_missing() {
        tracing::warn!(provider = %provider.id, %error, "{method} failed");
    }
}

/// A provider's id for something: a string, or a number written out.
fn json_id(value: Option<&serde_json::Value>) -> Option<String> {
    match value? {
        serde_json::Value::String(id) if !id.is_empty() => Some(id.clone()),
        serde_json::Value::Number(id) => Some(id.to_string()),
        _ => None,
    }
}

fn web_url(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .and_then(serde_json::Value::as_str)
        .filter(|url| is_web_url(url))
        .map(str::to_owned)
}

/// The artists in a provider's `{ artists: [{ id, name, imageUrl? }] }` answer.
pub(crate) fn artists_in(provider: &ProviderInfo, value: &serde_json::Value) -> Vec<MusicArtist> {
    value
        .get("artists")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|artist| {
            let name = artist.get("name")?.as_str()?.trim().to_owned();
            let id = json_id(artist.get("id"))?;
            (!name.is_empty()).then(|| MusicArtist {
                provider: provider.clone(),
                id,
                name,
                image_url: web_url(artist.get("imageUrl")),
            })
        })
        .collect()
}

/// The names in a `similarArtists` answer, `{ artists: [{ name }] }` (M46.20).
pub(crate) fn artist_names(value: &serde_json::Value) -> Vec<String> {
    value
        .get("artists")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|artist| artist.get("name")?.as_str())
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .collect()
}

/// The events in a provider's `{ events: [{ name, date, place?, url? }] }` answer (M46.42).
pub(crate) fn events_in(provider: &ProviderInfo, value: &serde_json::Value) -> Vec<ArtistEvent> {
    value
        .get("events")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|event| {
            let text = |key: &str| {
                event
                    .get(key)
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|text| !text.is_empty())
                    .map(str::to_owned)
            };
            Some(ArtistEvent {
                name: text("name")?,
                date: text("date")?,
                place: text("place"),
                url: web_url(event.get("url")),
                source: provider.name.clone(),
            })
        })
        .collect()
}

/// The albums listed under `key` in a provider's answer (`albums`, or `releases` for an
/// artist's discography).
pub(crate) fn albums_in(
    provider: &ProviderInfo,
    value: &serde_json::Value,
    key: &str,
) -> Vec<MusicAlbum> {
    value
        .get(key)
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|album| parse_album(provider, album))
        .collect()
}

/// The playlists in a provider's `{ playlists: [...] }` answer; only the services' own when
/// `featured`.
pub(crate) fn playlists_in(
    provider: &ProviderInfo,
    value: &serde_json::Value,
    featured: bool,
) -> Vec<MusicPlaylist> {
    value
        .get("playlists")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|playlist| parse_playlist(provider, playlist))
        .filter(|playlist| !featured || playlist.featured)
        .collect()
}

/// Reads `{ id, title, author?, artworkUrl?, trackCount?, featured? }` (M46.39).
pub(crate) fn parse_playlist(
    provider: &ProviderInfo,
    playlist: &serde_json::Value,
) -> Option<MusicPlaylist> {
    let title = playlist.get("title")?.as_str()?.trim().to_owned();
    if title.is_empty() {
        return None;
    }
    Some(MusicPlaylist {
        provider: provider.clone(),
        id: json_id(playlist.get("id"))?,
        title,
        author: playlist
            .get("author")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|author| !author.is_empty())
            .map(str::to_owned),
        artwork_url: web_url(playlist.get("artworkUrl")),
        track_count: playlist
            .get("trackCount")
            .and_then(serde_json::Value::as_u64)
            .and_then(|count| u32::try_from(count).ok()),
        featured: playlist
            .get("featured")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
    })
}

/// Reads `{ id, title, artist?, year?, artworkUrl?, trackCount? }`.
pub(crate) fn parse_album(
    provider: &ProviderInfo,
    album: &serde_json::Value,
) -> Option<MusicAlbum> {
    let title = album.get("title")?.as_str()?.trim().to_owned();
    if title.is_empty() {
        return None;
    }
    let number = |key: &str| {
        album
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
    };
    Some(MusicAlbum {
        provider: provider.clone(),
        id: json_id(album.get("id"))?,
        title,
        artist: album
            .get("artist")
            .and_then(serde_json::Value::as_str)
            .map(raygun_resolver::tidy_credit)
            .filter(|artist| !artist.is_empty()),
        year: number("year"),
        artwork_url: web_url(album.get("artworkUrl")),
        track_count: number("trackCount"),
        kind: album
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .map(raygun_model::ReleaseKind::parse),
    })
}

/// Reads `{ charts: [{ id, name, description?, artworkUrl?, tag? }] }`, dropping charts without an
/// id or a name.
pub(crate) fn parse_charts(provider: &ProviderInfo, value: &serde_json::Value) -> Vec<MusicChart> {
    let Some(charts) = value.get("charts").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    let text = |value: Option<String>| {
        value
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty())
    };
    charts
        .iter()
        .filter_map(|chart| RawChart::deserialize(chart).ok())
        .filter_map(|chart| {
            let id = match chart.id {
                serde_json::Value::String(id) => id,
                serde_json::Value::Number(id) => id.to_string(),
                _ => return None,
            };
            let name = chart.name.trim().to_owned();
            let tag = text(chart.tag);
            // Without a group (older sources), a genre's chart is a genre chart, else a main one.
            let group = chart
                .group
                .as_deref()
                .and_then(raygun_model::ChartGroup::parse)
                .unwrap_or(if tag.is_some() {
                    raygun_model::ChartGroup::Genre
                } else {
                    raygun_model::ChartGroup::Top
                });
            (!id.is_empty() && !name.is_empty()).then(|| MusicChart {
                provider: provider.clone(),
                id,
                name,
                description: text(chart.description),
                artwork_url: chart.artwork_url.filter(|url| is_web_url(url)),
                tag,
                group,
            })
        })
        .collect()
}

/// Reads `{ tags: [{ name, weight? }] }`: names trimmed and at most `MAX_TAG_CHARS` long,
/// weights clamped to 0-1.
pub(crate) fn parse_tags(value: &serde_json::Value) -> Vec<(String, f64)> {
    let Some(tags) = value.get("tags").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    tags.iter()
        .filter_map(|tag| {
            let name = tag.get("name")?.as_str()?.trim();
            if name.is_empty() || name.chars().count() > MAX_TAG_CHARS {
                return None;
            }
            let weight = tag
                .get("weight")
                .and_then(serde_json::Value::as_f64)
                .filter(|weight| weight.is_finite())
                .map_or(1.0, |weight| weight.clamp(0.0, 1.0));
            Some((name.to_owned(), weight))
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawLine {
    time_ms: f64,
    text: String,
}

/// Translations a lyrics answer may carry, at most.
const MAX_LYRICS_TRANSLATIONS: usize = 8;
/// A translated line longer than this is cut.
const MAX_TRANSLATED_LINE_CHARS: usize = 1_000;

/// Reads `{ synced?: [{ timeMs, text }], plain?, translations?: [{ language, kind?, lines }] }`.
/// Lines with bad times are dropped and the rest sorted; an answer with neither synced lines nor
/// plain text counts as "no lyrics". A translation's `lines[i]` belongs to `synced[i]` as the
/// provider gave it (M46.48), or to the plain text's i-th line; it follows its line through the
/// sorting.
pub(crate) fn parse_lyrics(provider: &ProviderInfo, value: &serde_json::Value) -> Option<Lyrics> {
    if value.is_null() {
        return None;
    }
    // Each kept line with its place in the provider's list.
    let mut synced: Vec<(usize, LyricLine)> = value
        .get("synced")
        .and_then(serde_json::Value::as_array)
        .map(|lines| {
            lines
                .iter()
                .enumerate()
                .filter_map(|(index, line)| {
                    let line = RawLine::deserialize(line).ok()?;
                    (line.time_ms.is_finite() && line.time_ms >= 0.0).then(|| {
                        let line = LyricLine {
                            time_ms: line.time_ms.min(f64::from(u32::MAX)) as u32,
                            text: line.text.trim().to_owned(),
                        };
                        (index, line)
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    synced.sort_by_key(|(_, line)| line.time_ms);
    let raw_plain = value.get("plain").and_then(serde_json::Value::as_str);
    let plain = raw_plain
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned);
    if synced.iter().all(|(_, line)| line.text.is_empty()) && plain.is_none() {
        return None;
    }
    // Where each line the translations line up with sat in the provider's list.
    let places: Vec<usize> = if synced.is_empty() {
        let leading = raw_plain.map_or(0, |raw| {
            raw[..raw.len() - raw.trim_start().len()]
                .matches('\n')
                .count()
        });
        let count = plain.as_deref().map_or(0, |plain| plain.lines().count());
        (leading..leading + count).collect()
    } else {
        synced.iter().map(|(index, _)| *index).collect()
    };
    let translations = value
        .get("translations")
        .and_then(serde_json::Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|translation| parse_translation(provider, translation, &places))
                .take(MAX_LYRICS_TRANSLATIONS)
                .collect()
        })
        .unwrap_or_default();
    Some(Lyrics {
        provider_name: provider.name.clone(),
        synced: synced.into_iter().map(|(_, line)| line).collect(),
        plain,
        offset_ms: 0,
        edited: false,
        translations,
    })
}

/// One of a lyrics answer's translations, its lines picked for `places` (the provider's indices
/// of the kept lines). `None` when it's malformed or every line is empty.
fn parse_translation(
    provider: &ProviderInfo,
    value: &serde_json::Value,
    places: &[usize],
) -> Option<LyricsTranslation> {
    let language = value.get("language")?.as_str()?.trim();
    let tag_like = (2..=35).contains(&language.len())
        && language
            .split('-')
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric()));
    if !tag_like {
        return None;
    }
    let kind = match value.get("kind").and_then(serde_json::Value::as_str) {
        None | Some("translation") => LyricsTranslationKind::Translation,
        Some("romanization") => LyricsTranslationKind::Romanization,
        Some(_) => return None,
    };
    let given = value.get("lines")?.as_array()?;
    let lines: Vec<String> = places
        .iter()
        .map(|place| {
            given
                .get(*place)
                .and_then(serde_json::Value::as_str)
                .map(|line| {
                    line.trim()
                        .chars()
                        .take(MAX_TRANSLATED_LINE_CHARS)
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect();
    if lines.iter().all(String::is_empty) {
        return None;
    }
    Some(LyricsTranslation {
        language: language.to_owned(),
        kind,
        origin: LyricsTranslationOrigin::Source,
        by: provider.name.clone(),
        translator: String::new(),
        lines,
    })
}

/// Sources an extension may hand out: web streams and YouTube ids, never local files or lookups.
pub(crate) fn allowed_source(source: &MediaSource) -> bool {
    match source {
        MediaSource::Url { url, .. } => is_web_url(url),
        MediaSource::YouTube { .. } => true,
        MediaSource::LocalFile { .. } | MediaSource::Lookup { .. } => false,
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawCandidate {
    id: serde_json::Value,
    title: String,
    #[serde(default)]
    artist: Option<String>,
    #[serde(default)]
    duration_ms: Option<f64>,
    #[serde(default)]
    quality: Option<String>,
    #[serde(default)]
    bitrate_kbps: Option<f64>,
    #[serde(default)]
    source: Option<MediaSource>,
}

/// Reads `{ candidates: [...] }` like [`parse_tracks`]: malformed entries are skipped, and a
/// candidate whose inline source isn't allowed is dropped (it would never play).
pub(crate) fn parse_candidates(
    provider: &ProviderInfo,
    value: serde_json::Value,
) -> Vec<Candidate> {
    let Some(candidates) = value
        .get("candidates")
        .and_then(serde_json::Value::as_array)
    else {
        return Vec::new();
    };
    candidates
        .iter()
        .filter_map(|candidate| RawCandidate::deserialize(candidate).ok())
        .filter(|candidate| candidate.source.as_ref().is_none_or(allowed_source))
        .filter(|candidate| !candidate.title.trim().is_empty())
        .map(|candidate| Candidate {
            provider_id: provider.id.clone(),
            provider_name: provider.name.clone(),
            id: match candidate.id {
                serde_json::Value::String(id) => id,
                other => other.to_string(),
            },
            title: candidate.title.trim().to_owned(),
            artist: candidate.artist.filter(|artist| !artist.trim().is_empty()),
            duration_ms: candidate
                .duration_ms
                .filter(|ms| ms.is_finite() && *ms > 0.0 && *ms < f64::from(u32::MAX))
                .map(|ms| ms as u32),
            quality: candidate.quality,
            bitrate_kbps: candidate
                .bitrate_kbps
                .filter(|kbps| kbps.is_finite() && *kbps >= 1.0 && *kbps < 100_000.0)
                .map(|kbps| kbps.round() as u32),
            source: candidate.source,
        })
        .collect()
}

/// A label for a setting that has none: `itemsPerSearch` and `items_per_search` read as
/// "Items per search".
pub(crate) fn key_label(key: &str) -> String {
    let mut words = String::new();
    let mut previous_lower = false;
    for character in key.chars() {
        if character == '_' || character == '-' {
            words.push(' ');
            previous_lower = false;
            continue;
        }
        if character.is_uppercase() && previous_lower {
            words.push(' ');
        }
        previous_lower = character.is_lowercase() || character.is_ascii_digit();
        words.extend(character.to_lowercase());
    }
    let mut chars = words.trim().chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}
