//! YouTube for extensions (`raygun.youtube`): the host side of Nuclear's `Ytdlp` API, which
//! converted Nuclear plugins call. It searches, finds a video's audio stream and lists a
//! playlist's videos through YouTube's own web API (InnerTube, the requests YouTube's apps make),
//! so it works on phones, where yt-dlp can't run; on desktop yt-dlp answers what InnerTube
//! didn't (`set_ytdlp`).
//!
//! - Search asks YouTube Music for songs and for music videos at once: songs first.
//! - Streams come from the `player` requests of YouTube's app clients whose answers carry plain
//!   addresses (`clients::PLAYER_CLIENTS`, ANDROID_VR first; the one that worked last goes first
//!   from then on), with visitor data from youtube.com's service worker data, as Grayjay's
//!   YouTube plugin asks for them (futo-org/grayjay-plugin-youtube, AGPL-3.0). Nothing is
//!   deciphered and no proof-of-origin token is made: the files of a client that wants one stop
//!   after their first megabyte, so each address is checked at its last bytes before it's handed
//!   out, with the client's own User-Agent, which the stream answer carries for the player.
//! - Playlists are youtube.com's `browse` of `VL<id>`, page by page; a mix (`RD…`), which has no
//!   page, is its watch panel.
//! - Live streams: youtube.com's search with its Live filter, YouTube's Live page (the streams it
//!   features), a channel's Live tab, and a stream's HLS playlist from the `player` answers of the
//!   same app clients. Not every client's playlist plays (some want a proof-of-origin token for
//!   their segments), so one segment near the live edge is asked for before it's handed out.
//!
//! Requests go through the calling extension's own fetch: its network permissions, cookies and
//! turns with each site apply.

mod clients;
mod parse;
mod ytdlp;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use clients::{PLAYER_CLIENTS, PlayerClient};
use parse::{Format, Playability};
pub use ytdlp::{YtDlp, set_ytdlp};

/// A request the module needs sent.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Request {
    pub method: &'static str,
    pub url: String,
    pub headers: Vec<(&'static str, String)>,
    pub body: Option<String>,
    /// How long to wait for its answer.
    pub timeout: Duration,
}

/// An answer: its status and text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Response {
    pub status: u16,
    pub body: String,
}

/// What sends the module's requests: the calling extension's fetch.
pub(crate) trait Http {
    async fn send(&self, request: Request) -> Result<Response, String>;
}

/// A picture of a video.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Thumbnail {
    pub url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VideoKind {
    /// YouTube Music's own audio of a song.
    Song,
    Video,
}

/// A video a search or a playlist found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Video {
    pub id: String,
    pub title: String,
    /// Who made it: a song's artists, else the channel that uploaded it (an artist's own
    /// "… - Topic" channel by the artist's name).
    pub channel: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    /// The largest picture.
    pub thumbnail_url: Option<String>,
    pub thumbnails: Vec<Thumbnail>,
    pub kind: VideoKind,
}

/// A video's audio stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stream {
    pub url: String,
    /// What the player sends with it (the User-Agent of the client it was given to).
    pub headers: BTreeMap<String, String>,
    /// Unix milliseconds after which the address stops working.
    pub expires_at: Option<i64>,
    pub mime_type: Option<String>,
    /// `webm` or `m4a`.
    pub container: Option<String>,
    /// `opus` or `mp4a.40.2`.
    pub codec: Option<String>,
    pub bitrate_kbps: Option<u32>,
    pub content_length: Option<u64>,
    pub duration_ms: Option<u64>,
    pub title: Option<String>,
    pub channel: Option<String>,
    pub album: Option<String>,
    pub artists: Vec<String>,
    /// `YYYYMMDD`, when known.
    pub upload_date: Option<String>,
    /// What gave it: an app client's name, or `yt-dlp`.
    pub client: String,
}

/// A playlist and its videos, in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub videos: Vec<Video>,
}

/// A video on a live listing: on the air now, coming up, or streamed before.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveVideo {
    pub id: String,
    pub title: String,
    pub channel: Option<String>,
    /// The channel's id (`UC…`), when the listing names it.
    pub channel_id: Option<String>,
    /// On the air now.
    pub live: bool,
    /// Scheduled and not started yet.
    pub upcoming: bool,
    /// People watching now, when YouTube says.
    pub viewers: Option<u64>,
    /// The largest picture.
    pub thumbnail_url: Option<String>,
    pub thumbnails: Vec<Thumbnail>,
}

/// A channel's Live tab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveChannel {
    /// The channel's id (`UC…`).
    pub id: String,
    pub title: Option<String>,
    pub avatar_url: Option<String>,
    /// On the air first, then coming up, then streamed before.
    pub videos: Vec<LiveVideo>,
}

/// A live stream's playlist, checked to play.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveStream {
    /// An HLS playlist naming every quality.
    pub url: String,
    /// What the player sends with its requests (the User-Agent of the client it was given to).
    pub headers: BTreeMap<String, String>,
    /// Unix milliseconds after which the address stops working.
    pub expires_at: Option<i64>,
    pub title: Option<String>,
    pub channel: Option<String>,
    /// On the air now (else a broadcast that has just ended).
    pub live: bool,
    /// The app client that gave it.
    pub client: String,
}

/// What a stream is for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamPrefs {
    /// The bitrate to aim for, in kbps; none asks for the best.
    #[serde(default)]
    pub target_kbps: Option<f64>,
    /// Raygun reads the file itself (a download, captions): AAC, which it decodes, over Opus.
    #[serde(default)]
    pub decodable: bool,
    /// YouTube's default audio track rather than the video's original one, where it has dubbed
    /// tracks too (`set_original_audio`, never from an extension).
    #[serde(skip)]
    pub default_track: bool,
}

/// Whether videos with dubbed tracks play their original audio (Settings, on by default).
static ORIGINAL_AUDIO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// Says whether a video's original audio plays over a dubbed or auto-translated track.
pub fn set_original_audio(on: bool) {
    ORIGINAL_AUDIO.store(on, std::sync::atomic::Ordering::Relaxed);
}

fn original_audio() -> bool {
    ORIGINAL_AUDIO.load(std::sync::atomic::Ordering::Relaxed)
}

/// Results a search gives at most.
const MAX_SEARCH: usize = 50;
/// A search gives this many when the caller names no number (yt-dlp's `ytsearch10`).
const DEFAULT_SEARCH: usize = 10;
/// Visitor data is read again after this long.
const VISITOR_TTL: Duration = Duration::from_secs(12 * 60 * 60);
/// After this many videos in a row without a stream, InnerTube's streams aren't asked for a while.
const FAILURES_TO_REST: u32 = 3;
const REST: Duration = Duration::from_secs(10 * 60);
/// No other client is asked once looking for a stream took this long: the caller waits 12 s in
/// all, and yt-dlp needs some of it.
const STREAM_BUDGET: Duration = Duration::from_secs(7);
/// The pages of a playlist read at most (100 videos each).
const MAX_PAGES: usize = 50;
/// No further page is asked once reading a playlist took this long.
const PLAYLIST_BUDGET: Duration = Duration::from_secs(45);
/// Live streams a live listing gives when the caller names no number.
const DEFAULT_LIVE: usize = 20;
/// No other client is asked for a live playlist once looking took this long.
const LIVE_BUDGET: Duration = Duration::from_secs(10);
/// Said of a video that isn't a live stream.
pub const NOT_LIVE: &str = "This YouTube video isn't a live stream";

/// Whether this platform's players play Opus: everywhere Raygun runs but Apple's.
const fn opus_plays() -> bool {
    !cfg!(any(target_os = "macos", target_os = "ios"))
}

/// Whether AAC (M4A) is wanted over Opus.
pub(crate) fn wants_aac(prefs: StreamPrefs) -> bool {
    prefs.decodable || !opus_plays()
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

/// A YouTube video id: 11 characters of `[A-Za-z0-9_-]`.
pub(crate) fn is_video_id(id: &str) -> bool {
    id.len() == 11
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// A YouTube channel id: `UC` and 22 characters of `[A-Za-z0-9_-]`.
pub(crate) fn is_channel_id(id: &str) -> bool {
    id.len() == 24
        && id.starts_with("UC")
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// What a text names as a channel: its id, or an address YouTube resolves to one.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ChannelRef {
    Id(String),
    Address(String),
}

/// A channel named by its id, its `@handle`, or an address (`/channel/UC…`, `/@handle`, `/c/…`,
/// `/user/…`).
fn channel_ref(text: &str) -> Option<ChannelRef> {
    let text = text.trim();
    if is_channel_id(text) {
        return Some(ChannelRef::Id(text.to_owned()));
    }
    let handle_ok = |handle: &str| {
        !handle.is_empty()
            && handle.len() <= 100
            && handle
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '·'))
    };
    if let Some(handle) = text.strip_prefix('@') {
        return handle_ok(handle)
            .then(|| ChannelRef::Address(format!("{}/@{handle}", clients::WEB)));
    }
    let url = youtube_url(text)?;
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();
    match segments.as_slice() {
        ["channel", id, ..] if is_channel_id(id) => Some(ChannelRef::Id((*id).to_owned())),
        [handle, ..] if handle.strip_prefix('@').is_some_and(handle_ok) => {
            Some(ChannelRef::Address(format!("{}/{handle}", clients::WEB)))
        }
        [kind @ ("c" | "user"), name, ..] => Some(ChannelRef::Address(format!(
            "{}/{kind}/{name}",
            clients::WEB
        ))),
        _ => None,
    }
}

fn is_playlist_id(id: &str) -> bool {
    (10..=64).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn youtube_url(text: &str) -> Option<url::Url> {
    let text = text.trim();
    let address = if text.contains("://") {
        text.to_owned()
    } else {
        format!("https://{text}")
    };
    let url = url::Url::parse(&address).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    let host = host
        .trim_start_matches("www.")
        .trim_start_matches("m.")
        .trim_start_matches("music.");
    matches!(host, "youtube.com" | "youtu.be" | "youtube-nocookie.com").then_some(url)
}

/// The video a text names: an id, or a YouTube address (`watch?v=`, `youtu.be/`, `shorts/`,
/// `live/`, `embed/`).
pub(crate) fn video_id(text: &str) -> Option<String> {
    let text = text.trim();
    if is_video_id(text) {
        return Some(text.to_owned());
    }
    let url = youtube_url(text)?;
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();
    let id = if url.host_str()?.ends_with("youtu.be") {
        segments.first().map(|id| (*id).to_owned())
    } else {
        match segments.as_slice() {
            ["watch"] => url
                .query_pairs()
                .find(|(name, _)| name == "v")
                .map(|(_, id)| id.into_owned()),
            ["shorts" | "live" | "embed" | "v", id, ..] => Some((*id).to_owned()),
            _ => None,
        }
    };
    id.filter(|id| is_video_id(id))
}

/// The playlist a text names: an id (`PL…`, `OLAK5uy_…`, `RD…`, also `VL`-prefixed), or a
/// YouTube address with `list=`.
pub(crate) fn playlist_id(text: &str) -> Option<String> {
    let text = text.trim();
    if is_playlist_id(text) && !text.contains('.') {
        let id = text
            .strip_prefix("VL")
            .filter(|id| is_playlist_id(id))
            .unwrap_or(text);
        return Some(id.to_owned());
    }
    let url = youtube_url(text)?;
    url.query_pairs()
        .find(|(name, _)| name == "list")
        .map(|(_, id)| id.into_owned())
        .filter(|id| is_playlist_id(id))
}

/// What the module keeps between calls, for the whole app.
#[derive(Debug, Default)]
struct Session {
    /// The visitor token and when it was read.
    visitor: Option<(String, Instant)>,
    /// The client whose stream worked last: asked first.
    preferred: Option<usize>,
    /// The client whose live playlist played last: asked first for live streams.
    live_preferred: Option<usize>,
    /// Videos in a row InnerTube gave no stream for.
    failures: u32,
    resting_until: Option<Instant>,
    /// Client versions read from YouTube's pages after one was refused.
    music_version: Option<String>,
    web_version: Option<String>,
}

/// Why InnerTube gave no stream.
#[derive(Debug, Clone, PartialEq, Eq)]
enum NoStream {
    /// The video is gone for everyone: yt-dlp can't play it either.
    Gone(String),
    Failed(String),
}

pub(crate) struct YouTube {
    session: Mutex<Session>,
    /// yt-dlp for tests; the app's is read from `set_ytdlp`.
    ytdlp: Option<YtDlp>,
}

impl YouTube {
    /// The one every extension shares.
    pub(crate) fn shared() -> &'static Self {
        static SHARED: LazyLock<YouTube> = LazyLock::new(|| YouTube::new(None));
        &SHARED
    }

    pub(crate) fn new(ytdlp: Option<YtDlp>) -> Self {
        Self {
            session: Mutex::new(Session::default()),
            ytdlp,
        }
    }

    fn session(&self) -> std::sync::MutexGuard<'_, Session> {
        self.session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn ytdlp(&self) -> Option<YtDlp> {
        self.ytdlp.clone().or_else(ytdlp::installed)
    }

    // ---- Search ----

    /// Songs, then music videos, for `query` (at most `limit`, 10 without one).
    pub(crate) async fn search(
        &self,
        http: &impl Http,
        query: &str,
        limit: Option<usize>,
    ) -> Result<Vec<Video>, String> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let limit = limit.unwrap_or(DEFAULT_SEARCH).clamp(1, MAX_SEARCH);
        let (songs, videos) = futures_util::future::join(
            self.music_search(http, query, clients::SONGS, VideoKind::Song),
            self.music_search(http, query, clients::VIDEOS, VideoKind::Video),
        )
        .await;
        let error = match (songs, videos) {
            (Err(error), Err(_)) => error,
            (songs, videos) => {
                let mut found: Vec<Video> = Vec::new();
                for video in songs.into_iter().chain(videos).flatten() {
                    if !found.iter().any(|seen| seen.id == video.id) {
                        found.push(video);
                    }
                }
                found.truncate(limit);
                return Ok(found);
            }
        };
        let Some(ytdlp) = self.ytdlp() else {
            return Err(error);
        };
        match ytdlp::search(&ytdlp, query, limit).await {
            Ok(found) => Ok(found),
            Err(fallback) => {
                tracing::info!(%error, %fallback, "YouTube search failed, yt-dlp's too");
                Err(error)
            }
        }
    }

    async fn music_search(
        &self,
        http: &impl Http,
        query: &str,
        chip: &str,
        kind: VideoKind,
    ) -> Result<Vec<Video>, String> {
        let stated = self.session().music_version.clone();
        let version = stated.unwrap_or_else(|| clients::MUSIC_VERSION.to_owned());
        let mut answer = send(http, clients::music_search(&version, query, chip)).await?;
        // An outdated client: the page states the current one.
        if answer.status == 400
            && let Some(current) = self.read_version(http, clients::MUSIC, &version).await
        {
            self.session().music_version = Some(current.clone());
            answer = send(http, clients::music_search(&current, query, chip)).await?;
        }
        let answer = json_of(answer, "YouTube Music")?;
        self.remember_visitor(&answer);
        Ok(parse::music_search(&answer, kind))
    }

    /// The client version a YouTube page states, when it's another than `refused`.
    async fn read_version(&self, http: &impl Http, site: &str, refused: &str) -> Option<String> {
        let page = send(http, clients::page(&format!("{site}/"))).await.ok()?;
        parse::client_version_in_page(&page.body).filter(|version| version != refused)
    }

    // ---- Visitor data ----

    fn remember_visitor(&self, answer: &Value) {
        if let Some(visitor) = answer
            .pointer("/responseContext/visitorData")
            .and_then(Value::as_str)
            .filter(|visitor| parse::is_visitor(visitor))
        {
            let mut session = self.session();
            if session.visitor.is_none() {
                session.visitor = Some((visitor.to_owned(), Instant::now()));
            }
        }
    }

    /// The visitor token requests carry: kept, else read from youtube.com's service worker data.
    async fn visitor(&self, http: &impl Http) -> Option<String> {
        let kept = self.session().visitor.clone();
        if let Some((visitor, at)) = kept
            && at.elapsed() < VISITOR_TTL
        {
            return Some(visitor);
        }
        let read = send(http, clients::sw_data())
            .await
            .ok()
            .filter(|answer| answer.status == 200)
            .and_then(|answer| parse::visitor_in_sw_data(&answer.body));
        let mut session = self.session();
        match read {
            Some(visitor) => {
                session.visitor = Some((visitor.clone(), Instant::now()));
                Some(visitor)
            }
            // An old token beats none.
            None => session.visitor.as_ref().map(|(visitor, _)| visitor.clone()),
        }
    }

    // ---- Streams ----

    /// A video's audio stream, checked to play to its end, nearest `prefs`.
    pub(crate) async fn stream(
        &self,
        http: &impl Http,
        video: &str,
        mut prefs: StreamPrefs,
    ) -> Result<Stream, String> {
        prefs.default_track = !original_audio();
        let id = video_id(video).ok_or_else(|| format!("{video:?} isn't a YouTube video"))?;
        let error = match self.innertube_stream(http, &id, prefs).await {
            Ok(stream) => return Ok(stream),
            Err(NoStream::Gone(reason)) => return Err(reason),
            Err(NoStream::Failed(error)) => error,
        };
        let Some(ytdlp) = self.ytdlp() else {
            return Err(error);
        };
        match ytdlp::stream(&ytdlp, &id, prefs).await {
            Ok(stream) => Ok(stream),
            Err(fallback) => {
                tracing::info!(video = %id, %error, %fallback, "no YouTube stream, from yt-dlp neither");
                Err(error)
            }
        }
    }

    /// The order clients are asked in: the one that worked last, then the rest in theirs.
    fn client_order(&self) -> Vec<&'static PlayerClient> {
        let preferred = self.session().preferred;
        preferred
            .into_iter()
            .chain((0..PLAYER_CLIENTS.len()).filter(|index| Some(*index) != preferred))
            .filter_map(|index| PLAYER_CLIENTS.get(index))
            .collect()
    }

    async fn innertube_stream(
        &self,
        http: &impl Http,
        video_id: &str,
        prefs: StreamPrefs,
    ) -> Result<Stream, NoStream> {
        if self
            .session()
            .resting_until
            .is_some_and(|until| Instant::now() < until)
        {
            return Err(NoStream::Failed(
                "YouTube isn't giving streams right now".to_owned(),
            ));
        }
        let started = Instant::now();
        let visitor = self.visitor(http).await;
        let mut refusal: Option<String> = None;
        let mut trouble: Option<String> = None;
        for client in self.client_order() {
            if started.elapsed() > STREAM_BUDGET {
                break;
            }
            let answer = match send(http, clients::player(client, video_id, visitor.as_deref()))
                .await
                .and_then(|answer| json_of(answer, "YouTube"))
            {
                Ok(answer) => answer,
                Err(error) => {
                    trouble.get_or_insert(error);
                    continue;
                }
            };
            match parse::playability(&answer) {
                Playability::Ok => {}
                Playability::Gone(reason) => return Err(NoStream::Gone(reason)),
                Playability::Refused(reason) => {
                    refusal.get_or_insert(reason);
                    continue;
                }
            }
            let formats = parse::formats(&answer);
            let Some(format) = pick_format(&formats, prefs) else {
                continue;
            };
            let Some(url) = format.url.as_deref() else {
                continue;
            };
            if !plays_to_the_end(http, url, format.content_length, client.user_agent).await {
                tracing::debug!(
                    client = client.name,
                    video = video_id,
                    "its stream stops early"
                );
                continue;
            }
            let index = PLAYER_CLIENTS.iter().position(|known| known == client);
            {
                let mut session = self.session();
                session.preferred = index;
                session.failures = 0;
            }
            return Ok(stream_of(&answer, format, url, client));
        }
        // YouTube said why this video doesn't play: nothing to hold against the clients.
        if let Some(refusal) = refusal {
            return Err(NoStream::Failed(refusal));
        }
        {
            let mut session = self.session();
            session.failures += 1;
            if session.failures >= FAILURES_TO_REST {
                session.failures = 0;
                session.resting_until = Some(Instant::now() + REST);
            }
        }
        Err(NoStream::Failed(trouble.unwrap_or_else(|| {
            "YouTube didn't give a stream for it".to_owned()
        })))
    }

    // ---- Playlists ----

    /// A playlist's videos, every page of them.
    pub(crate) async fn playlist(
        &self,
        http: &impl Http,
        playlist: &str,
    ) -> Result<Playlist, String> {
        let id = playlist_id(playlist)
            .ok_or_else(|| format!("{playlist:?} isn't a YouTube playlist"))?;
        let error = match self.innertube_playlist(http, &id).await {
            Ok(found) if !found.videos.is_empty() => return Ok(found),
            Ok(_) => "YouTube lists no videos in it (a private or deleted playlist?)".to_owned(),
            Err(error) => error,
        };
        let Some(ytdlp) = self.ytdlp() else {
            return Err(error);
        };
        match ytdlp::playlist(&ytdlp, &id).await {
            Ok(found) => Ok(found),
            Err(fallback) => {
                tracing::info!(playlist = %id, %error, %fallback, "no YouTube playlist, from yt-dlp neither");
                Err(error)
            }
        }
    }

    /// A youtube.com request, with the client version read again when YouTube refuses the one sent.
    async fn web(
        &self,
        http: &impl Http,
        endpoint: &str,
        visitor: Option<&str>,
        ask: &Value,
    ) -> Result<Value, String> {
        let stated = self.session().web_version.clone();
        let version = stated.unwrap_or_else(|| clients::WEB_VERSION.to_owned());
        let mut answer = send(http, clients::web(endpoint, &version, visitor, ask.clone())).await?;
        if answer.status == 400
            && let Some(current) = self.read_version(http, clients::WEB, &version).await
        {
            self.session().web_version = Some(current.clone());
            answer = send(http, clients::web(endpoint, &current, visitor, ask.clone())).await?;
        }
        json_of(answer, "YouTube")
    }

    async fn innertube_playlist(&self, http: &impl Http, id: &str) -> Result<Playlist, String> {
        let started = Instant::now();
        let visitor = self.visitor(http).await;
        let visitor = visitor.as_deref();
        let first = self
            .web(
                http,
                "browse",
                visitor,
                &json!({ "browseId": format!("VL{id}") }),
            )
            .await?;
        let (title, author) = parse::playlist_header(&first);
        let mut videos = parse::playlist_videos(&first);
        if videos.is_empty() && id.starts_with("RD") {
            // A mix has no page of its own: its watch panel lists it.
            let watch = self
                .web(http, "next", visitor, &json!({ "playlistId": id }))
                .await?;
            let (title, author) = parse::mix_header(&watch);
            return Ok(Playlist {
                id: id.to_owned(),
                title: title.unwrap_or_else(|| "Mix".to_owned()),
                author,
                videos: parse::mix_videos(&watch),
            });
        }
        let mut next = parse::next_page(&first);
        let mut pages = 1;
        while let Some(token) = next.take() {
            if pages >= MAX_PAGES || started.elapsed() > PLAYLIST_BUDGET {
                tracing::info!(playlist = id, videos = videos.len(), "playlist cut short");
                break;
            }
            let page = self
                .web(http, "browse", visitor, &json!({ "continuation": token }))
                .await?;
            let more = parse::playlist_videos(&page);
            let before = videos.len();
            for video in more {
                if !videos.iter().any(|seen| seen.id == video.id) {
                    videos.push(video);
                }
            }
            // A page with nothing new is YouTube starting over: the end.
            if videos.len() == before {
                break;
            }
            next = parse::next_page(&page);
            pages += 1;
        }
        Ok(Playlist {
            id: id.to_owned(),
            title: title.unwrap_or_else(|| "Playlist".to_owned()),
            author,
            videos,
        })
    }

    // ---- Live streams ----

    /// Streams on the air now: those matching `query` (youtube.com's search with its Live
    /// filter), else the ones YouTube's Live page features. At most `limit` (20 without one).
    pub(crate) async fn live_search(
        &self,
        http: &impl Http,
        query: &str,
        limit: Option<usize>,
    ) -> Result<Vec<LiveVideo>, String> {
        let limit = limit.unwrap_or(DEFAULT_LIVE).clamp(1, MAX_SEARCH);
        let visitor = self.visitor(http).await;
        let query = query.trim();
        let ask = if query.is_empty() {
            ("browse", json!({ "browseId": clients::LIVE_PAGE }))
        } else {
            (
                "search",
                json!({ "query": query, "params": clients::LIVE_FILTER }),
            )
        };
        let answer = self.web(http, ask.0, visitor.as_deref(), &ask.1).await?;
        let mut found: Vec<LiveVideo> = parse::live_videos(&answer)
            .into_iter()
            .filter(|video| video.live)
            .collect();
        found.truncate(limit);
        Ok(found)
    }

    /// A channel's Live tab (its id, `@handle` or address): what's on the air first, then what's
    /// coming up, then what was streamed before. A channel that never streamed has none.
    pub(crate) async fn channel_live(
        &self,
        http: &impl Http,
        channel: &str,
    ) -> Result<LiveChannel, String> {
        let wanted =
            channel_ref(channel).ok_or_else(|| format!("{channel:?} isn't a YouTube channel"))?;
        let visitor = self.visitor(http).await;
        let visitor = visitor.as_deref();
        let id = match wanted {
            ChannelRef::Id(id) => id,
            ChannelRef::Address(url) => {
                let answer = self
                    .web(
                        http,
                        "navigation/resolve_url",
                        visitor,
                        &json!({ "url": url }),
                    )
                    .await?;
                parse::resolved_channel(&answer)
                    .ok_or_else(|| "YouTube has no such channel".to_owned())?
            }
        };
        let home = self
            .web(http, "browse", visitor, &json!({ "browseId": id }))
            .await?;
        let (title, avatar_url) = parse::channel_header(&home);
        let Some(params) = parse::streams_tab(&home) else {
            return Ok(LiveChannel {
                id,
                title,
                avatar_url,
                videos: Vec::new(),
            });
        };
        let page = self
            .web(
                http,
                "browse",
                visitor,
                &json!({ "browseId": id, "params": params }),
            )
            .await?;
        let mut videos = parse::live_videos(&page);
        videos.sort_by_key(|video| match (video.live, video.upcoming) {
            (true, _) => 0,
            (false, true) => 1,
            _ => 2,
        });
        // A channel's own tab doesn't name it on each card.
        for video in &mut videos {
            if video.channel.is_none() {
                video.channel.clone_from(&title);
            }
            video.channel_id.get_or_insert_with(|| id.clone());
        }
        Ok(LiveChannel {
            id,
            title,
            avatar_url,
            videos,
        })
    }

    /// The order clients are asked for a live playlist: the one whose playlist played last, then
    /// the rest in theirs.
    fn live_client_order(&self) -> Vec<&'static PlayerClient> {
        let preferred = self.session().live_preferred;
        preferred
            .into_iter()
            .chain((0..PLAYER_CLIENTS.len()).filter(|index| Some(*index) != preferred))
            .filter_map(|index| PLAYER_CLIENTS.get(index))
            .collect()
    }

    /// A live stream's playlist (an id or an address): HLS naming every quality, from the first
    /// app client whose segments play ([`live_plays`]). A video that isn't a live stream, or a
    /// stream that has ended and became a plain video, has none.
    pub(crate) async fn live(&self, http: &impl Http, video: &str) -> Result<LiveStream, String> {
        let id = video_id(video).ok_or_else(|| format!("{video:?} isn't a YouTube video"))?;
        let started = Instant::now();
        let visitor = self.visitor(http).await;
        let mut refusal: Option<String> = None;
        let mut trouble: Option<String> = None;
        for client in self.live_client_order() {
            if started.elapsed() > LIVE_BUDGET {
                break;
            }
            let answer = match send(http, clients::player(client, &id, visitor.as_deref()))
                .await
                .and_then(|answer| json_of(answer, "YouTube"))
            {
                Ok(answer) => answer,
                Err(error) => {
                    trouble.get_or_insert(error);
                    continue;
                }
            };
            match parse::playability(&answer) {
                Playability::Ok => {}
                Playability::Gone(reason) => return Err(reason),
                Playability::Refused(reason) => {
                    refusal.get_or_insert(reason);
                    continue;
                }
            }
            if !parse::is_live_content(&answer) {
                return Err(NOT_LIVE.to_owned());
            }
            let Some(url) = parse::hls_manifest(&answer) else {
                if !parse::is_live(&answer) {
                    return Err("This live stream has ended".to_owned());
                }
                continue;
            };
            if !live_plays(http, &url, client.user_agent).await {
                tracing::debug!(
                    client = client.name,
                    video = %id,
                    "its live playlist's segments don't play"
                );
                trouble.get_or_insert_with(|| "YouTube's live playlist didn't play".to_owned());
                continue;
            }
            self.session().live_preferred = PLAYER_CLIENTS.iter().position(|known| known == client);
            let details = parse::details(&answer);
            return Ok(LiveStream {
                url,
                headers: BTreeMap::from([("User-Agent".to_owned(), client.user_agent.to_owned())]),
                expires_at: details
                    .expires_in_seconds
                    .and_then(|seconds| i64::try_from(seconds).ok())
                    .map(|seconds| now_ms().saturating_add(seconds.saturating_mul(1000))),
                title: details.title,
                channel: details.channel,
                live: parse::is_live(&answer),
                client: client.name.to_owned(),
            });
        }
        Err(refusal
            .or(trouble)
            .unwrap_or_else(|| "YouTube gave no live playlist Raygun can play".to_owned()))
    }
}

/// The addresses an HLS playlist names (its variants, or its segments), resolved against it.
fn playlist_uris(base: &str, body: &str) -> Vec<String> {
    let base = url::Url::parse(base).ok();
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| match &base {
            Some(base) => base.join(line).ok().map(String::from),
            None => line.starts_with("https://").then(|| line.to_owned()),
        })
        .collect()
}

/// Whether a live playlist's segments are handed out: its first variant's playlist, then the
/// segment three from its live edge, asked as the client it was given to.
async fn live_plays(http: &impl Http, master: &str, user_agent: &str) -> bool {
    let fetch = |url: String| async move {
        let request = clients::get(
            url,
            vec![("User-Agent", user_agent.to_owned())],
            Duration::from_secs(5),
        );
        send(http, request)
            .await
            .ok()
            .filter(|answer| answer.status == 200 && answer.body.starts_with("#EXTM3U"))
    };
    let Some(top) = fetch(master.to_owned()).await else {
        return false;
    };
    // A playlist naming variants, or already one of segments.
    let media = if top.body.contains("#EXT-X-STREAM-INF") {
        let Some(variant) = playlist_uris(master, &top.body).into_iter().next() else {
            return false;
        };
        match fetch(variant.clone()).await {
            Some(answer) => (variant, answer.body),
            None => return false,
        }
    } else {
        (master.to_owned(), top.body)
    };
    let segments = playlist_uris(&media.0, &media.1);
    let Some(segment) = segments.iter().rev().nth(2).or(segments.last()) else {
        return false;
    };
    let check = clients::get(
        segment.clone(),
        vec![
            ("Range", "bytes=0-1".to_owned()),
            ("User-Agent", user_agent.to_owned()),
        ],
        Duration::from_secs(5),
    );
    matches!(send(http, check).await, Ok(answer) if matches!(answer.status, 200 | 206))
}

/// Sends a request, waiting no longer than it allows.
async fn send(http: &impl Http, request: Request) -> Result<Response, String> {
    let host = url::Url::parse(&request.url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or_default();
    let limit = request.timeout;
    tokio::time::timeout(limit, http.send(request))
        .await
        .map_err(|_| format!("{host} didn't answer within {} s", limit.as_secs()))?
}

/// An answer's JSON, or why there is none.
fn json_of(answer: Response, site: &str) -> Result<Value, String> {
    match answer.status {
        200..=299 => serde_json::from_str(&answer.body)
            .map_err(|_| format!("{site} answered something Raygun can't read")),
        429 => {
            Err("YouTube is limiting requests from this network; try again in a while".to_owned())
        }
        status => Err(format!("{site} answered HTTP {status}")),
    }
}

/// How far `kbps` is from `target`: the percent by which the larger exceeds the smaller.
fn distance(kbps: u32, target: f64) -> f64 {
    let kbps = f64::from(kbps.max(1));
    let target = target.max(1.0);
    kbps.max(target) * 100.0 / kbps.min(target) - 100.0
}

/// The audio format to hand out: the nearest to the target bitrate (the highest without one), AAC
/// where it's wanted (`wants_aac`), else Opus on a tie. Plain addresses only, the regular sound
/// (not "stable volume", no audio description), and of a video with dubbed tracks its original
/// audio (NouTube's "Play original audio", nonbili/NouTube, AGPL-3.0), unless YouTube's default
/// track is wanted (`StreamPrefs::default_track`).
pub(crate) fn pick_format(formats: &[Format], prefs: StreamPrefs) -> Option<&Format> {
    let audio: Vec<&Format> = formats
        .iter()
        .filter(|format| format.is_audio() && format.url.is_some())
        .collect();
    let only = |keep: &dyn Fn(&Format) -> bool| -> Vec<&Format> {
        audio
            .iter()
            .copied()
            .filter(|format| !format.is_drc && !format.descriptive && keep(format))
            .collect()
    };
    let youtube_default = only(&|format| format.audio_is_default != Some(false));
    let tiers = if prefs.default_track {
        vec![youtube_default, only(&|_| true)]
    } else {
        vec![
            only(&|format| format.original),
            only(&|format| format.audio_is_default != Some(false) && !format.dubbed),
            youtube_default,
            only(&|_| true),
        ]
    };
    let usable = tiers
        .into_iter()
        .find(|tier| !tier.is_empty())
        .unwrap_or(audio);
    let aac = wants_aac(prefs);
    let in_aac: Vec<&Format> = usable
        .iter()
        .copied()
        .filter(|format| format.is_aac())
        .collect();
    let choices = if aac && !in_aac.is_empty() {
        in_aac
    } else {
        usable
    };
    let target = prefs
        .target_kbps
        .filter(|kbps| kbps.is_finite() && *kbps > 0.0);
    let score = |format: &Format| {
        let fit = match target {
            Some(target) => -distance(format.kbps(), target),
            None => f64::from(format.kbps()),
        };
        fit * 1000.0 + if format.is_aac() == aac { 1.0 } else { 0.0 }
    };
    choices
        .into_iter()
        .fold(None, |best: Option<&Format>, format| match best {
            Some(best) if score(best) >= score(format) => Some(best),
            _ => Some(format),
        })
}

/// Whether YouTube's server hands out a stream to its end: its last two bytes, asked as the
/// client it was given to (a stream that wants a proof-of-origin token stops after about a
/// megabyte). Without a length, its first two.
async fn plays_to_the_end(
    http: &impl Http,
    url: &str,
    content_length: Option<u64>,
    user_agent: &str,
) -> bool {
    let start = content_length
        .filter(|length| *length > 2)
        .map_or(0, |length| length - 2);
    send(http, clients::stream_check(url, start, user_agent))
        .await
        .is_ok_and(|answer| matches!(answer.status, 200 | 206))
}

fn stream_of(answer: &Value, format: &Format, url: &str, client: &PlayerClient) -> Stream {
    let details = parse::details(answer);
    let expires_at = details
        .expires_in_seconds
        .filter(|seconds| *seconds > 0)
        .map(|seconds| now_ms() + i64::try_from(seconds).unwrap_or(0) * 1000)
        .or_else(|| {
            parse::url_param(url, "expire")
                .and_then(|seconds| seconds.parse::<i64>().ok())
                .map(|seconds| seconds * 1000)
        });
    Stream {
        url: url.to_owned(),
        headers: BTreeMap::from([("User-Agent".to_owned(), client.user_agent.to_owned())]),
        expires_at,
        mime_type: Some(format.mime_type.clone()),
        container: format.container().map(str::to_owned),
        codec: format.codec(),
        bitrate_kbps: Some(format.kbps()).filter(|kbps| *kbps > 0),
        content_length: format.content_length,
        duration_ms: details.duration_ms.or(format.approx_duration_ms),
        artists: details.channel.clone().into_iter().collect(),
        title: details.title,
        channel: details.channel,
        album: None,
        upload_date: details.upload_date,
        client: client.name.to_owned(),
    }
}
