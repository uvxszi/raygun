//! yt-dlp behind YouTube's own API, where the app has it (desktop, `set_ytdlp`): asked when
//! InnerTube gave nothing. Its JSON is read the way Nuclear's player reads it
//! (NuclearPlayer/nuclear, `ytdlp.rs`, AGPL-3.0): `--dump-json` lines, flat for lists.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use serde::Deserialize;

use super::{Playlist, Stream, StreamPrefs, Thumbnail, Video, VideoKind, now_ms};

/// Runs yt-dlp with these arguments and hands back what it printed, or why it failed.
pub type YtDlp = Arc<
    dyn Fn(Vec<String>) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send>>
        + Send
        + Sync,
>;

static YTDLP: RwLock<Option<YtDlp>> = RwLock::new(None);

/// Sets what runs yt-dlp (the app, on desktop): YouTube searches, streams and playlists InnerTube
/// didn't answer are asked from it.
pub fn set_ytdlp(run: YtDlp) {
    *YTDLP
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(run);
}

pub(super) fn installed() -> Option<YtDlp> {
    YTDLP
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// Searches are short; a stream's lookup runs the player's code; a long playlist takes a while.
const SEARCH_TIMEOUT: Duration = Duration::from_secs(15);
const STREAM_TIMEOUT: Duration = Duration::from_secs(15);
const PLAYLIST_TIMEOUT: Duration = Duration::from_secs(90);

async fn run(ytdlp: &YtDlp, args: Vec<String>, limit: Duration) -> Result<String, String> {
    tokio::time::timeout(limit, ytdlp(args))
        .await
        .map_err(|_| "yt-dlp took too long".to_owned())?
}

/// yt-dlp's options, then what it reads: after `--`, so it's never taken for an option.
fn args(options: &[&str], target: String) -> Vec<String> {
    options
        .iter()
        .map(|option| (*option).to_owned())
        .chain(["--".to_owned(), target])
        .collect()
}

pub(super) fn search_args(query: &str, limit: usize) -> Vec<String> {
    args(
        &["--dump-json", "--flat-playlist", "--no-warnings"],
        format!("ytsearch{limit}:{query}"),
    )
}

pub(super) fn stream_args(video_id: &str, prefs: StreamPrefs) -> Vec<String> {
    let format = if super::wants_aac(prefs) {
        "bestaudio[ext=m4a]/bestaudio[acodec^=mp4a]/bestaudio"
    } else {
        "bestaudio[ext=webm]/bestaudio[ext=m4a]/bestaudio"
    };
    args(
        &[
            "-f",
            format,
            "--dump-json",
            "--no-playlist",
            "--no-warnings",
        ],
        format!("https://www.youtube.com/watch?v={video_id}"),
    )
}

pub(super) fn playlist_args(playlist_id: &str) -> Vec<String> {
    args(
        &["--dump-json", "--flat-playlist", "--no-warnings"],
        format!("https://www.youtube.com/playlist?list={playlist_id}"),
    )
}

pub(super) async fn search(ytdlp: &YtDlp, query: &str, limit: usize) -> Result<Vec<Video>, String> {
    let printed = run(ytdlp, search_args(query, limit), SEARCH_TIMEOUT).await?;
    Ok(parse_search(&printed))
}

pub(super) async fn stream(
    ytdlp: &YtDlp,
    video_id: &str,
    prefs: StreamPrefs,
) -> Result<Stream, String> {
    let printed = run(ytdlp, stream_args(video_id, prefs), STREAM_TIMEOUT).await?;
    parse_stream(&printed)
}

pub(super) async fn playlist(ytdlp: &YtDlp, playlist_id: &str) -> Result<Playlist, String> {
    let printed = run(ytdlp, playlist_args(playlist_id), PLAYLIST_TIMEOUT).await?;
    parse_playlist(&printed, playlist_id)
}

#[derive(Debug, Default, Deserialize)]
struct RawThumbnail {
    url: Option<String>,
    width: Option<f64>,
    height: Option<f64>,
}

/// One line of yt-dlp's `--dump-json`.
#[derive(Debug, Default, Deserialize)]
struct Entry {
    id: Option<String>,
    title: Option<String>,
    duration: Option<f64>,
    url: Option<String>,
    thumbnail: Option<String>,
    #[serde(default)]
    thumbnails: Vec<RawThumbnail>,
    ext: Option<String>,
    acodec: Option<String>,
    abr: Option<f64>,
    tbr: Option<f64>,
    filesize: Option<f64>,
    channel: Option<String>,
    uploader: Option<String>,
    artist: Option<String>,
    artists: Option<Vec<String>>,
    creators: Option<Vec<String>>,
    album: Option<String>,
    upload_date: Option<String>,
    http_headers: Option<BTreeMap<String, String>>,
    playlist_title: Option<String>,
    playlist_uploader: Option<String>,
    playlist_channel: Option<String>,
}

fn entries(printed: &str) -> impl Iterator<Item = Entry> + '_ {
    printed
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with('{'))
        .filter_map(|line| serde_json::from_str::<Entry>(line).ok())
}

fn seconds_ms(seconds: Option<f64>) -> Option<u64> {
    seconds
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
        .map(|seconds| (seconds * 1000.0).round() as u64)
}

fn thumbnails(entry: &Entry) -> Vec<Thumbnail> {
    let size = |value: Option<f64>| {
        value
            .filter(|value| value.is_finite() && *value > 0.0)
            .map(|value| value as u32)
    };
    let mut found: Vec<Thumbnail> = entry
        .thumbnails
        .iter()
        .filter_map(|thumbnail| {
            let url = thumbnail.url.clone()?;
            url.starts_with("https://").then(|| Thumbnail {
                url,
                width: size(thumbnail.width),
                height: size(thumbnail.height),
            })
        })
        .collect();
    if let Some(url) = entry
        .thumbnail
        .clone()
        .filter(|url| url.starts_with("https://"))
        && !found.iter().any(|thumbnail| thumbnail.url == url)
    {
        found.push(Thumbnail {
            url,
            width: None,
            height: None,
        });
    }
    found
}

fn video(entry: Entry) -> Option<Video> {
    let id = entry.id.clone().filter(|id| super::is_video_id(id))?;
    let thumbnails = thumbnails(&entry);
    Some(Video {
        title: entry
            .title
            .clone()
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| id.clone()),
        channel: entry
            .channel
            .or(entry.uploader)
            .map(|name| super::parse::without_topic(&name))
            .filter(|name| !name.is_empty()),
        album: None,
        duration_ms: seconds_ms(entry.duration),
        thumbnail_url: entry
            .thumbnail
            .filter(|url| url.starts_with("https://"))
            .or_else(|| thumbnails.last().map(|thumbnail| thumbnail.url.clone())),
        thumbnails,
        kind: VideoKind::Video,
        id,
    })
}

pub(crate) fn parse_search(printed: &str) -> Vec<Video> {
    entries(printed).filter_map(video).collect()
}

pub(crate) fn parse_playlist(printed: &str, playlist_id: &str) -> Result<Playlist, String> {
    let mut title = None;
    let mut author = None;
    let mut videos = Vec::new();
    for entry in entries(printed) {
        title = title.or_else(|| entry.playlist_title.clone());
        author = author.or_else(|| {
            entry
                .playlist_uploader
                .clone()
                .or_else(|| entry.playlist_channel.clone())
        });
        videos.extend(video(entry));
    }
    let title = title.ok_or("yt-dlp didn't find the playlist")?;
    Ok(Playlist {
        id: playlist_id.to_owned(),
        title,
        author,
        videos,
    })
}

pub(crate) fn parse_stream(printed: &str) -> Result<Stream, String> {
    let entry = entries(printed)
        .next()
        .ok_or("yt-dlp didn't describe the video")?;
    let url = entry
        .url
        .clone()
        .filter(|url| url.starts_with("https://"))
        .ok_or("yt-dlp gave no stream address")?;
    let expires_at = super::parse::url_param(&url, "expire")
        .and_then(|seconds| seconds.parse::<i64>().ok())
        .map(|seconds| seconds * 1000)
        .filter(|at| *at > now_ms());
    let artists = entry
        .artists
        .clone()
        .or_else(|| entry.creators.clone())
        .or_else(|| entry.artist.clone().map(|artist| vec![artist]))
        .unwrap_or_default();
    let kbps = entry
        .abr
        .or(entry.tbr)
        .filter(|kbps| kbps.is_finite() && *kbps > 0.0)
        .map(|kbps| kbps.round() as u32);
    let container = entry.ext.clone().filter(|ext| !ext.is_empty());
    Ok(Stream {
        mime_type: container.as_deref().map(|ext| match ext {
            "webm" => "audio/webm".to_owned(),
            "m4a" | "mp4" => "audio/mp4".to_owned(),
            other => format!("audio/{other}"),
        }),
        container,
        codec: entry.acodec.clone().filter(|codec| codec != "none"),
        bitrate_kbps: kbps,
        content_length: entry
            .filesize
            .filter(|size| size.is_finite() && *size > 0.0)
            .map(|size| size as u64),
        duration_ms: seconds_ms(entry.duration),
        title: entry.title.clone(),
        channel: entry
            .channel
            .clone()
            .or(entry.uploader.clone())
            .map(|name| super::parse::without_topic(&name)),
        album: entry.album.clone(),
        artists,
        upload_date: entry.upload_date.clone(),
        headers: entry.http_headers.clone().unwrap_or_default(),
        expires_at,
        client: "yt-dlp".to_owned(),
        url,
    })
}
