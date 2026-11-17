//! Links played on their own (Open link, another app; progress key `url:<address>`): their page
//! (`/watch/link`), what they hold (a torrent's files, kept once known), their name (renamed on
//! the page; an unnamed magnet link takes its torrent's), their poster (the movie they were
//! matched with, else a torrent's own "poster.jpg"), and how they play: through a session
//! (`VideoSessions::start_link`), so the player's Sources and Quality work for them.
//!
//! The match is optional: the page suggests the movies Cinemeta finds for the link's title, and
//! one picked gives the link its poster and, in the player, the addons' streams for that movie
//! as other sources.

use std::sync::Arc;
use std::time::Duration;

use raygun_db::video as store;
use raygun_model::{
    VideoLinkFile, VideoLinkFileKind, VideoLinkKind, VideoLinkMatch, VideoLinkPage, VideoRequest,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::events::WatchChangedEvent;
use crate::startup::{AppDb, AppPaths};
use crate::video_session::{Link, LinkPlay, VideoSessions};

/// What's known of a link is kept under this, then its key (device-local).
const MEMORY_PREFIX: &str = "video.link.";
/// Torrents' own posters, under the downloads folder (the proxy's `/local/` route serves them).
const POSTERS: &str = "links";
/// A link's title before anything better is known (`video::link_request`).
const UNNAMED: &str = "Video";
/// Cinemeta, whose catalogues find a movie or series by its title (the poster and the year a
/// match shows); only asked from a link's page.
const CINEMETA: &str = "https://v3-cinemeta.strem.io/manifest.json";
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(12);
/// Matches suggested at most, and how close their titles must be (`title_score`).
const SUGGESTIONS: usize = 6;
const CLOSE_ENOUGH: u32 = 60;

/// What's known of a link beyond its entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Memory {
    #[serde(default)]
    info_hash: Option<String>,
    /// The torrent's own name.
    #[serde(default)]
    name: Option<String>,
    /// The torrent's files, once known.
    #[serde(default)]
    files: Option<Vec<KeptFile>>,
    /// The torrent's own poster (`local:links/<hash>.jpg`).
    #[serde(default)]
    poster: Option<String>,
    #[serde(default)]
    matched: Option<VideoLinkMatch>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct KeptFile {
    index: usize,
    path: String,
    len: u64,
}

impl KeptFile {
    fn torrent_file(&self) -> raygun_torrent::TorrentFile {
        raygun_torrent::TorrentFile {
            index: self.index,
            path: self.path.clone(),
            len: self.len,
        }
    }
}

/// Whether a progress key is a link's.
pub fn is_link_key(key: &str) -> bool {
    key.starts_with("url:")
}

/// Whether a request is a link played on its own: its key is its address and it belongs to no
/// title (a source's episode picked by hand names its title).
pub fn is_link(request: &VideoRequest) -> bool {
    is_link_key(&request.key) && request.group.is_none()
}

fn db(app: &AppHandle) -> Arc<raygun_db::Db> {
    app.state::<AppDb>().0.clone()
}

async fn memory(db: &raygun_db::Db, key: &str) -> Memory {
    raygun_db::local_value(db, &format!("{MEMORY_PREFIX}{key}"))
        .await
        .ok()
        .flatten()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

async fn save_memory(db: &raygun_db::Db, key: &str, memory: &Memory) {
    let Ok(text) = serde_json::to_string(memory) else {
        return;
    };
    if let Err(error) =
        raygun_db::set_local_value(db, &format!("{MEMORY_PREFIX}{key}"), &text).await
    {
        tracing::warn!(%error, "could not keep what a link holds");
    }
}

/// A link's saved entry and the request it plays with, as the entry shows it (its title and
/// poster); an entry without a readable request is played from its key.
async fn saved(app: &AppHandle, key: &str) -> Result<(store::VideoProgress, VideoRequest), String> {
    let row = store::progress(&db(app), key)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "This link was removed".to_owned())?;
    let mut request = serde_json::from_str::<VideoRequest>(&row.entry.request)
        .ok()
        .filter(is_link)
        .unwrap_or_else(|| {
            crate::video::link_request(
                key.strip_prefix("url:").unwrap_or(key),
                Some(&row.entry.title),
                None,
            )
        });
    request.title = row.entry.title.clone();
    request.poster = row.entry.poster.clone();
    Ok((row, request))
}

/// What a link is, by its address and format.
pub fn kind_of(request: &VideoRequest) -> VideoLinkKind {
    let lower = request.url.to_ascii_lowercase();
    let path = lower.split(['?', '#']).next().unwrap_or_default();
    if raygun_torrent::is_torrent(&request.url, request.format.as_deref()) {
        VideoLinkKind::Torrent
    } else if request.format.as_deref() == Some("hls") || path.ends_with(".m3u8") {
        VideoLinkKind::Hls
    } else if request.format.as_deref() == Some("dash") || path.ends_with(".mpd") {
        VideoLinkKind::Dash
    } else if lower.starts_with("file:") || lower.starts_with("content:") {
        VideoLinkKind::Device
    } else {
        VideoLinkKind::File
    }
}

/// The web host a link plays from.
fn host_of(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()
        .filter(|parsed| matches!(parsed.scheme(), "http" | "https"))
        .and_then(|parsed| parsed.host_str().map(str::to_owned))
}

/// A file name's format, as shown ("MP4", "MKV"), for the video formats players know.
fn format_of(name: &str) -> Option<String> {
    let path = name.split(['?', '#']).next().unwrap_or_default();
    let file = path.rsplit('/').next().unwrap_or_default();
    let (_, extension) = file.rsplit_once('.')?;
    raygun_torrent::is_video(file).then(|| extension.to_ascii_uppercase())
}

/// The last part of a web address's path, decoded ("Sintel%20(2010).mp4" → "Sintel (2010).mp4").
fn file_name_of(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let last = parsed
        .path_segments()?
        .rev()
        .find(|part| !part.is_empty())?;
    let decoded = url::form_urlencoded::parse(format!("x={last}").as_bytes())
        .next()
        .map(|(_, value)| value.into_owned())
        .unwrap_or_else(|| last.to_owned());
    Some(decoded).filter(|name| !name.is_empty())
}

/// The name the link's group goes by in the player's Sources: a torrent's name (or the link's
/// title), a web address's host, a file's name.
pub fn group_name(request: &VideoRequest) -> String {
    match kind_of(request) {
        VideoLinkKind::Torrent => raygun_torrent::parse(&request.url)
            .ok()
            .and_then(|link| link.name().map(str::to_owned))
            .unwrap_or_else(|| request.title.clone()),
        VideoLinkKind::Device => request.title.clone(),
        _ => host_of(&request.url).unwrap_or_else(|| request.title.clone()),
    }
}

/// The torrent file the link opens: the one the page picked, else the one its link names, else
/// the main video ([`raygun_torrent::pick_file`]).
fn main_file(request: &VideoRequest, files: &[raygun_torrent::TorrentFile]) -> Option<usize> {
    let hint = raygun_torrent::parse(&request.url)
        .map(|link| link.hint().clone())
        .unwrap_or_default();
    raygun_torrent::pick_file(files, &hint)
}

/// A torrent's videos as the session's links, the one the link opens first, then the rest
/// largest first; each opens its own file.
fn torrent_links(request: &VideoRequest, files: &[raygun_torrent::TorrentFile]) -> Vec<Link> {
    let main = main_file(request, files);
    let mut videos: Vec<&raygun_torrent::TorrentFile> = files
        .iter()
        .filter(|file| raygun_torrent::is_video(&file.path))
        .collect();
    videos.sort_by_key(|file| (Some(file.index) != main, std::cmp::Reverse(file.len)));
    videos
        .into_iter()
        .map(|file| Link {
            label: file.name().to_owned(),
            tag: None,
            failed: None,
            mismatch: false,
            height: None,
            size: Some(file.len),
            request: VideoRequest {
                url: raygun_torrent::with_file(&request.url, request.format.as_deref(), file.index),
                ..request.clone()
            },
        })
        .collect()
}

/// The link's own links for its session, with its group's name: a torrent's videos (asked of
/// peers when they aren't known yet, and kept), else the link itself.
pub async fn own_links(
    app: &AppHandle,
    link: &LinkPlay,
    labels: &serde_json::Value,
) -> Result<(String, Vec<Link>), String> {
    let request = &link.request;
    if kind_of(request) != VideoLinkKind::Torrent {
        let label = file_name_of(&request.url)
            .or_else(|| {
                labels
                    .get("video")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| UNNAMED.to_owned());
        let own = Link {
            label,
            tag: None,
            failed: None,
            mismatch: false,
            height: None,
            size: None,
            request: request.clone(),
        };
        return Ok((group_name(request), vec![own]));
    }
    let db = db(app);
    let kept = memory(&db, &request.key).await;
    let (name, files) = match kept.files {
        Some(files) => (
            kept.name,
            files.iter().map(KeptFile::torrent_file).collect::<Vec<_>>(),
        ),
        None => {
            let torrents = app
                .try_state::<Arc<crate::torrents::TorrentService>>()
                .ok_or_else(|| crate::torrents::TORRENTS_OFF.to_owned())?;
            let listing = torrents.list(&request.url).await?;
            listed(app, &request.key, &listing).await;
            (listing.name, listing.files)
        }
    };
    // An unnamed magnet link plays (and is kept) under its torrent's name.
    let named = match &name {
        Some(name) if request.title == UNNAMED => VideoRequest {
            title: name.clone(),
            ..request.clone()
        },
        _ => request.clone(),
    };
    let links = torrent_links(&named, &files);
    if links.is_empty() {
        return Err(raygun_torrent::TorrentError::NoVideo.to_string());
    }
    Ok((name.unwrap_or_else(|| group_name(request)), links))
}

/// The movie a link was matched with, when it has one (series give no other sources: which
/// episode the link is isn't known).
pub async fn matched_movie(app: &AppHandle, key: &str) -> Option<VideoLinkMatch> {
    memory(&db(app), key)
        .await
        .matched
        .filter(|matched| matched.kind == "movie")
}

/// A torrent's files became known (it was listed or opened to play): they're kept for its page,
/// and an unnamed magnet link takes its torrent's name.
async fn listed(app: &AppHandle, key: &str, listing: &raygun_torrent::Listing) {
    let db = db(app);
    let mut kept = memory(&db, key).await;
    let files: Vec<KeptFile> = listing
        .files
        .iter()
        .map(|file| KeptFile {
            index: file.index,
            path: file.path.clone(),
            len: file.len,
        })
        .collect();
    let changed = kept.files.as_ref() != Some(&files)
        || kept.name != listing.name
        || kept.info_hash.as_deref() != Some(listing.info_hash.as_str());
    if !changed {
        return;
    }
    kept.files = Some(files);
    kept.name = listing.name.clone();
    kept.info_hash = Some(listing.info_hash.clone());
    save_memory(&db, key, &kept).await;
    if let (Some(name), Ok((row, mut request))) = (&listing.name, saved(app, key).await)
        && row.entry.title == UNNAMED
    {
        request.title = name.clone();
        let _ = store::set_entry(
            &db,
            key,
            name,
            row.entry.poster.as_deref(),
            &serde_json::to_string(&request).unwrap_or_default(),
        )
        .await;
    }
    let _ = WatchChangedEvent(key.to_owned()).emit(app);
}

/// A torrent playing opened (`torrents`): what it holds is kept for its page, when it's a link
/// played on its own.
pub async fn torrent_opened(app: AppHandle, key: String, listing: raygun_torrent::Listing) {
    if is_link_key(&key) {
        listed(&app, &key, &listing).await;
    }
}

/// Whether a link keeps its torrent's poster already.
pub async fn has_poster(app: &AppHandle, key: &str) -> bool {
    memory(&db(app), key).await.poster.is_some()
}

/// A torrent's own poster arrived: it's kept under the downloads folder and shown for the link
/// unless the movie it was matched with gives one.
pub async fn torrent_poster(
    app: &AppHandle,
    key: &str,
    info_hash: &str,
    name: &str,
    bytes: Vec<u8>,
) {
    let file = format!(
        "{POSTERS}/{}.{}",
        info_hash
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect::<String>(),
        raygun_torrent::image_extension(name)
    );
    let path = app.state::<AppPaths>().0.downloads().join(&file);
    let written = async {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&path, &bytes).await
    };
    if let Err(error) = written.await {
        tracing::warn!(%error, "could not keep a torrent's poster");
        return;
    }
    let db = db(app);
    let mut kept = memory(&db, key).await;
    let poster = format!("{}{file}", crate::catalog::LOCAL_PREFIX);
    kept.poster = Some(poster.clone());
    save_memory(&db, key, &kept).await;
    let shown = kept
        .matched
        .as_ref()
        .and_then(|matched| matched.poster.clone())
        .unwrap_or(poster);
    if let Ok((row, request)) = saved(app, key).await
        && row.entry.poster.as_deref() != Some(shown.as_str())
    {
        let request = VideoRequest {
            poster: Some(shown.clone()),
            ..request
        };
        let _ = store::set_entry(
            &db,
            key,
            &row.entry.title,
            Some(&shown),
            &serde_json::to_string(&request).unwrap_or_default(),
        )
        .await;
        let _ = WatchChangedEvent(key.to_owned()).emit(app);
    }
}

/// A poster as the webview loads it: a kept file through the proxy's `/local/` route, a web
/// picture through the lasting cover cache.
pub fn shown_poster(images: &raygun_net::ImageLinks, poster: &str) -> String {
    match poster.strip_prefix(crate::catalog::LOCAL_PREFIX) {
        Some(path) => images.local(path),
        None if images.is_remote(poster) => images.cover(poster, &Default::default()),
        None => poster.to_owned(),
    }
}

/// What a torrent's file is.
fn file_kind(name: &str) -> VideoLinkFileKind {
    if raygun_torrent::is_video(name) {
        VideoLinkFileKind::Video
    } else if raygun_torrent::subtitle_format(name).is_some() {
        VideoLinkFileKind::Subtitle
    } else if raygun_torrent::is_image(name) {
        VideoLinkFileKind::Image
    } else {
        VideoLinkFileKind::Other
    }
}

/// A torrent's files for its page: videos first (the one the link opens leading), then
/// subtitles, pictures and the rest, each by path.
fn page_files(request: &VideoRequest, files: &[raygun_torrent::TorrentFile]) -> Vec<VideoLinkFile> {
    let main = main_file(request, files);
    let main_stem = main
        .and_then(|index| files.iter().find(|file| file.index == index))
        .map(|file| {
            let name = file.name();
            name.rsplit_once('.')
                .map_or(name, |(stem, _)| stem)
                .to_owned()
        })
        .unwrap_or_default();
    let mut shown: Vec<VideoLinkFile> = files
        .iter()
        .map(|file| {
            let kind = file_kind(file.name());
            VideoLinkFile {
                index: u32::try_from(file.index).unwrap_or(u32::MAX),
                path: file.path.clone(),
                name: file.name().to_owned(),
                size_bytes: file.len as f64,
                kind,
                main: Some(file.index) == main,
                language: (kind == VideoLinkFileKind::Subtitle)
                    .then(|| raygun_torrent::subtitles::read_name(file.name(), &main_stem).0)
                    .flatten(),
            }
        })
        .collect();
    let rank = |file: &VideoLinkFile| match file.kind {
        VideoLinkFileKind::Video => 0,
        VideoLinkFileKind::Subtitle => 1,
        VideoLinkFileKind::Image => 2,
        VideoLinkFileKind::Other => 3,
    };
    shown.sort_by(|a, b| (rank(a), !a.main, &a.path).cmp(&(rank(b), !b.main, &b.path)));
    shown
}

/// A link's page.
pub async fn page(app: &AppHandle, key: &str) -> Result<VideoLinkPage, String> {
    let (row, request) = saved(app, key).await?;
    let kept = memory(&db(app), key).await;
    let kind = kind_of(&request);
    let files: Option<Vec<raygun_torrent::TorrentFile>> = kept
        .files
        .as_ref()
        .map(|files| files.iter().map(KeptFile::torrent_file).collect());
    let main = files.as_deref().and_then(|files| {
        let index = main_file(&request, files)?;
        files.iter().find(|file| file.index == index).cloned()
    });
    let images = app.state::<raygun_net::ImageLinks>();
    let poster = row
        .entry
        .poster
        .as_deref()
        .map(|poster| shown_poster(&images, poster));
    let device = kind == VideoLinkKind::Device;
    Ok(VideoLinkPage {
        key: key.to_owned(),
        url: request.url.clone(),
        title: row.entry.title.clone(),
        search_title: search_name(&row.entry.title).0,
        poster,
        host: host_of(&request.url),
        format: match &main {
            Some(file) => format_of(file.name()),
            None if kind == VideoLinkKind::Torrent => None,
            None => format_of(&request.url),
        },
        name: kept.name.clone(),
        size_bytes: main.as_ref().map(|file| file.len as f64),
        files: files.as_deref().map(|files| page_files(&request, files)),
        position_ms: row.position_ms as f64,
        duration_ms: row.duration_ms.map(|ms| ms as f64),
        watched: row.watched_at.is_some(),
        // Its poster through the lasting cover cache, as every poster shown.
        matched: kept.matched.map(|matched| VideoLinkMatch {
            poster: matched
                .poster
                .as_deref()
                .map(|poster| shown_poster(&images, poster)),
            ..matched
        }),
        can_download: !device && kind != VideoLinkKind::Dash,
        kind,
    })
}

/// Asks peers what a torrent link holds now (its page's files), and keeps it.
pub async fn list_files(app: &AppHandle, key: &str) -> Result<VideoLinkPage, String> {
    let (_, request) = saved(app, key).await?;
    if kind_of(&request) != VideoLinkKind::Torrent {
        return page(app, key).await;
    }
    let torrents = app
        .try_state::<Arc<crate::torrents::TorrentService>>()
        .ok_or_else(|| crate::torrents::TORRENTS_OFF.to_owned())?;
    let listing = torrents.list(&request.url).await?;
    listed(app, key, &listing).await;
    page(app, key).await
}

/// Renames a link: its page, Continue watching and the player show the new name.
pub async fn rename(app: &AppHandle, key: &str, title: &str) -> Result<(), String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("A name can't be empty".into());
    }
    let (row, request) = saved(app, key).await?;
    let request = VideoRequest {
        title: title.to_owned(),
        ..request
    };
    store::set_entry(
        &db(app),
        key,
        title,
        row.entry.poster.as_deref(),
        &serde_json::to_string(&request).map_err(|error| error.to_string())?,
    )
    .await
    .map_err(|error| error.to_string())?;
    let _ = WatchChangedEvent(key.to_owned()).emit(app);
    Ok(())
}

/// Matches a link with a movie or series (its poster shows for the link; a movie's other sources
/// join the player's Sources), or forgets the match (`None`: the torrent's own poster again).
pub async fn set_match(
    app: &AppHandle,
    key: &str,
    matched: Option<VideoLinkMatch>,
) -> Result<(), String> {
    let (row, request) = saved(app, key).await?;
    let db = db(app);
    let mut kept = memory(&db, key).await;
    let images = app.state::<raygun_net::ImageLinks>();
    kept.matched = matched.map(|matched| VideoLinkMatch {
        poster: matched.poster.map(|poster| images.original(&poster)),
        ..matched
    });
    save_memory(&db, key, &kept).await;
    let poster = kept
        .matched
        .as_ref()
        .and_then(|matched| matched.poster.clone())
        .or_else(|| kept.poster.clone());
    let request = VideoRequest {
        poster: poster.clone(),
        ..request
    };
    store::set_entry(
        &db,
        key,
        &row.entry.title,
        poster.as_deref(),
        &serde_json::to_string(&request).map_err(|error| error.to_string())?,
    )
    .await
    .map_err(|error| error.to_string())?;
    let _ = WatchChangedEvent(key.to_owned()).emit(app);
    Ok(())
}

/// Plays a link from its page or Continue watching: the file of its torrent picked (`file`), from
/// its start when `from_start`, else where it was left.
pub async fn play(
    app: &AppHandle,
    key: &str,
    file: Option<u32>,
    from_start: bool,
) -> Result<(), String> {
    let (_, mut request) = saved(app, key).await?;
    let file = file.map(|file| file as usize);
    if let Some(file) = file {
        request.url = raygun_torrent::with_file(&request.url, request.format.as_deref(), file);
    }
    app.state::<VideoSessions>()
        .start_link(LinkPlay { request, file }, from_start.then_some(0))
        .await
}

/// Plays a link request (Open link, another app, a link's entry resumed): what its entry shows
/// (a new name, a poster found) goes with it.
pub async fn play_request(
    app: &AppHandle,
    mut request: VideoRequest,
    start_at: Option<i64>,
) -> Result<(), String> {
    if let Ok(Some(row)) = store::progress(&db(app), &request.key).await {
        request.title = row.entry.title;
        request.poster = request.poster.or(row.entry.poster);
    }
    app.state::<VideoSessions>()
        .start_link(
            LinkPlay {
                request,
                file: None,
            },
            start_at,
        )
        .await
}

/// Downloads a link (the file of its torrent picked, or the one it opens), with its subtitle
/// file when it has one.
pub async fn download(app: &AppHandle, key: &str, file: Option<u32>) -> Result<(), String> {
    let (_, mut request) = saved(app, key).await?;
    if let Some(file) = file {
        request.url =
            raygun_torrent::with_file(&request.url, request.format.as_deref(), file as usize);
    }
    let downloads = app
        .try_state::<Arc<crate::video_downloads::VideoDownloadService>>()
        .ok_or("Downloads aren't available")?;
    let pick = crate::video_downloads::SubtitlePick {
        source: String::new(),
        shown: request.subtitles.clone(),
        chosen: Some(
            request
                .subtitles
                .iter()
                .map(|track| track.url.clone())
                .collect(),
        ),
    };
    downloads.enqueue(request, None, pick).await
}

/// Forgets what's kept of a link (its entry was removed).
pub async fn forget(app: &AppHandle, key: &str) {
    if is_link_key(key) {
        save_memory(&db(app), key, &Memory::default()).await;
    }
}

/// The title and year a link's name gives, for finding the movie it is: a release's dots and
/// underscores are spaces, and its year, size and encoding are left out ("Big.Buck.Bunny.2008.
/// 1080p.BluRay" → "Big Buck Bunny", 2008).
pub fn search_name(title: &str) -> (String, Option<u32>) {
    const RELEASE_WORDS: [&str; 26] = [
        "480p", "540p", "576p", "720p", "1080p", "1080i", "1440p", "2160p", "4k", "uhd", "bluray",
        "blu", "bdrip", "brrip", "webrip", "web", "webdl", "hdtv", "dvdrip", "dvd", "x264", "x265",
        "h264", "h265", "hevc", "remux",
    ];
    let mut name = title.trim();
    if raygun_torrent::is_video(name) {
        name = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    }
    // "[Group] Title": the group's part goes.
    while let Some(rest) = name.strip_prefix('[').and_then(|rest| rest.split_once(']')) {
        name = rest.1.trim_start();
    }
    let spaced = if name.contains(' ') {
        name.to_owned()
    } else {
        name.replace(['.', '_'], " ")
    };
    let mut words = Vec::new();
    let mut year = None;
    for (at, word) in spaced.split_whitespace().enumerate() {
        let bare = word.trim_matches(|character: char| "()[]{}".contains(character));
        let lower = bare.to_ascii_lowercase().replace('-', "");
        if let Ok(found) = bare.parse::<u32>()
            && at > 0
            && bare.len() == 4
            && (1900..=2099).contains(&found)
        {
            year = Some(found);
            break;
        }
        if at > 0 && RELEASE_WORDS.contains(&lower.as_str()) {
            break;
        }
        words.push(word);
    }
    let found = words
        .join(" ")
        .trim()
        .trim_end_matches(['-', ':'])
        .trim()
        .to_owned();
    if found.is_empty() {
        (title.trim().to_owned(), year)
    } else {
        (found, year)
    }
}

/// Movies and series Cinemeta finds for a link's title, closest first (the same year first when
/// the name gives one): the matches its page suggests. None offline or when it doesn't answer.
pub async fn suggestions(app: &AppHandle, key: &str) -> Result<Vec<VideoLinkMatch>, String> {
    let (row, _) = saved(app, key).await?;
    let (query, year) = search_name(&row.entry.title);
    if query.is_empty() {
        return Ok(Vec::new());
    }
    raygun_net::network_mode::check_online().map_err(|offline| offline.to_string())?;
    let client = raygun_net::http_client().map_err(|error| error.to_string())?;
    let extras = vec![("search".to_owned(), query.clone())];
    let asks = ["movie", "series"].map(|kind| {
        let url = crate::stremio::protocol::resource_url(CINEMETA, "catalog", kind, "top", &extras);
        let client = client.clone();
        async move {
            let answer = client.get(&url).timeout(LOOKUP_TIMEOUT).send().await.ok()?;
            let value: serde_json::Value = answer.error_for_status().ok()?.json().await.ok()?;
            Some(crate::stremio::protocol::parse_metas(&value, kind))
        }
    });
    let found: Vec<_> = futures_util::future::join_all(asks)
        .await
        .into_iter()
        .flatten()
        .flatten()
        .collect();
    let images = app.state::<raygun_net::ImageLinks>();
    Ok(rank_matches(found, &query, year)
        .into_iter()
        .map(|matched| VideoLinkMatch {
            poster: matched
                .poster
                .map(|poster| images.cover(&poster, &Default::default())),
            ..matched
        })
        .collect())
}

/// Found titles close enough to `query`, closest first; with a year known, that year's first.
fn rank_matches(
    found: Vec<crate::stremio::protocol::MetaPreview>,
    query: &str,
    year: Option<u32>,
) -> Vec<VideoLinkMatch> {
    let mut scored: Vec<(bool, u32, usize, VideoLinkMatch)> = found
        .into_iter()
        .enumerate()
        .filter(|(_, meta)| meta.kind == "movie" || meta.kind == "series")
        .map(|(at, meta)| {
            let score = raygun_resolver::title_score(&meta.name, query);
            let found_year = meta
                .release_info
                .as_deref()
                .and_then(|info| info.get(..4))
                .and_then(|start| start.parse::<u32>().ok());
            let same_year = year.is_some() && found_year == year;
            (
                same_year,
                score,
                at,
                VideoLinkMatch {
                    kind: meta.kind,
                    id: meta.id,
                    name: meta.name,
                    year: meta.release_info,
                    poster: meta.poster,
                },
            )
        })
        .filter(|(_, score, _, _)| *score >= CLOSE_ENOUGH)
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
    let mut seen = std::collections::HashSet::new();
    scored
        .into_iter()
        .map(|(_, _, _, matched)| matched)
        .filter(|matched| seen.insert(format!("{}:{}", matched.kind, matched.id)))
        .take(SUGGESTIONS)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(index: usize, path: &str, len: u64) -> raygun_torrent::TorrentFile {
        raygun_torrent::TorrentFile {
            index,
            path: path.into(),
            len,
        }
    }

    /// The Sintel torrent's files (WebTorrent's).
    fn sintel() -> Vec<raygun_torrent::TorrentFile> {
        vec![
            file(0, "Sintel/Sintel.de.srt", 1_652),
            file(1, "Sintel/Sintel.en.srt", 1_514),
            file(2, "Sintel/Sintel.mp4", 129_241_752),
            file(3, "Sintel/poster.jpg", 46_115),
            file(4, "Sintel/Sintel.sample.mp4", 3_000_000),
        ]
    }

    const MAGNET: &str = "magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d566095a10&dn=Sintel";

    #[test]
    fn links_say_what_they_are() {
        let kind = |url: &str| kind_of(&crate::video::link_request(url, None, None));
        assert_eq!(kind(MAGNET), VideoLinkKind::Torrent);
        assert_eq!(
            kind("https://cdn.example/show/master.m3u8?t=1"),
            VideoLinkKind::Hls
        );
        assert_eq!(kind("https://cdn.example/a.mpd"), VideoLinkKind::Dash);
        assert_eq!(kind("https://cdn.example/Sintel.mp4"), VideoLinkKind::File);
        assert_eq!(
            kind("file:///storage/emulated/0/a.mkv"),
            VideoLinkKind::Device
        );
        assert_eq!(
            host_of("https://cdn.example/a.mp4"),
            Some("cdn.example".into())
        );
        assert_eq!(host_of(MAGNET), None);
        assert_eq!(
            format_of("https://cdn.example/a/Sintel.mp4?x=1"),
            Some("MP4".into())
        );
        assert_eq!(format_of("https://cdn.example/master.m3u8"), None);
        assert_eq!(
            file_name_of("https://cdn.example/films/Sintel%20(2010).mp4"),
            Some("Sintel (2010).mp4".into())
        );
        let magnet = crate::video::link_request(MAGNET, None, None);
        assert_eq!(group_name(&magnet), "Sintel");
        let web = crate::video::link_request("https://cdn.example/a.mp4", Some("A"), None);
        assert_eq!(group_name(&web), "cdn.example");
        assert!(is_link(&web));
        let picked = VideoRequest {
            group: Some("title:t1".into()),
            ..web
        };
        assert!(!is_link(&picked), "a source's episode names its title");
    }

    #[test]
    fn a_torrents_videos_are_its_links_the_one_it_opens_first() {
        let request = crate::video::link_request(MAGNET, None, None);
        let links = torrent_links(&request, &sintel());
        let names: Vec<&str> = links.iter().map(|link| link.label.as_str()).collect();
        assert_eq!(names, ["Sintel.mp4", "Sintel.sample.mp4"]);
        assert!(
            links[0].request.url.ends_with("&so=2"),
            "{}",
            links[0].request.url
        );
        assert!(links[1].request.url.ends_with("&so=4"));
        assert_eq!(links[0].size, Some(129_241_752));
        // Every one plays under the link's own key and title.
        assert!(links.iter().all(|link| link.request.key == request.key));

        // A file picked on the page goes first.
        let picked = VideoRequest {
            url: raygun_torrent::with_file(MAGNET, None, 4),
            ..request
        };
        let links = torrent_links(&picked, &sintel());
        assert_eq!(links[0].label, "Sintel.sample.mp4");
    }

    #[test]
    fn a_torrents_page_lists_videos_then_subtitles_then_the_rest() {
        let request = crate::video::link_request(MAGNET, None, None);
        let files = page_files(&request, &sintel());
        let shown: Vec<(&str, VideoLinkFileKind, bool)> = files
            .iter()
            .map(|file| (file.name.as_str(), file.kind, file.main))
            .collect();
        assert_eq!(
            shown,
            [
                ("Sintel.mp4", VideoLinkFileKind::Video, true),
                ("Sintel.sample.mp4", VideoLinkFileKind::Video, false),
                ("Sintel.de.srt", VideoLinkFileKind::Subtitle, false),
                ("Sintel.en.srt", VideoLinkFileKind::Subtitle, false),
                ("poster.jpg", VideoLinkFileKind::Image, false),
            ]
        );
        assert_eq!(files[2].language.as_deref(), Some("de"));
        assert_eq!(files[0].index, 2);
    }

    #[test]
    fn release_names_give_the_title_and_year() {
        assert_eq!(search_name("Sintel"), ("Sintel".into(), None));
        assert_eq!(
            search_name("Big.Buck.Bunny.2008.1080p.BluRay.x264"),
            ("Big Buck Bunny".into(), Some(2008))
        );
        assert_eq!(
            search_name("[Group] Tears of Steel (2012) [1080p]"),
            ("Tears of Steel".into(), Some(2012))
        );
        assert_eq!(
            search_name("sintel_trailer.mp4"),
            ("sintel trailer".into(), None)
        );
        assert_eq!(
            search_name("Elephants.Dream.720p.mkv"),
            ("Elephants Dream".into(), None)
        );
        // A title that is a year stays itself.
        assert_eq!(search_name("1917"), ("1917".into(), None));
    }

    #[test]
    fn matches_are_close_titles_the_same_year_first() {
        let meta =
            |kind: &str, id: &str, name: &str, year: &str| crate::stremio::protocol::MetaPreview {
                kind: kind.into(),
                id: id.into(),
                name: name.into(),
                poster: Some(format!("https://images.example/{id}.jpg")),
                poster_shape: "poster".into(),
                release_info: Some(year.into()),
                rating: None,
                background: None,
                trailer: None,
            };
        let found = vec![
            meta("movie", "tt2", "Sintel", "2012"),
            meta("movie", "tt1", "Sintel", "2010"),
            meta("series", "tt3", "Something Else", "2001"),
            meta("channel", "c1", "Sintel", "2010"),
        ];
        let ranked = rank_matches(found.clone(), "Sintel", Some(2010));
        let ids: Vec<&str> = ranked.iter().map(|matched| matched.id.as_str()).collect();
        assert_eq!(ids, ["tt1", "tt2"]);
        let ranked = rank_matches(found, "Sintel", None);
        assert_eq!(ranked[0].id, "tt2", "as found when no year says");
        assert_eq!(ranked[0].year.as_deref(), Some("2012"));
    }
}
