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
        prefs: StreamPrefs,
    ) -> Result<Stream, String> {
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
/// (not "stable volume") and the video's own language first.
pub(crate) fn pick_format(formats: &[Format], prefs: StreamPrefs) -> Option<&Format> {
    let audio: Vec<&Format> = formats
        .iter()
        .filter(|format| format.is_audio() && format.url.is_some())
        .collect();
    let regular: Vec<&Format> = audio
        .iter()
        .copied()
        .filter(|format| !format.is_drc && format.audio_is_default != Some(false))
        .collect();
    let usable = if regular.is_empty() { audio } else { regular };
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
