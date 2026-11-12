//! Reading YouTube's answers: YouTube Music's search rows, youtube.com's playlist pages and the
//! `player` answers of YouTube's app clients. Every reader walks the answer for the renderers it
//! knows wherever YouTube nests them, so a moved shelf doesn't lose results.
//!
//! The row and format readings follow Raygun's former YouTube extension, whose stream parts were
//! adapted from futo-org/grayjay-plugin-youtube (AGPL-3.0).

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::{LiveVideo, Thumbnail, Video, VideoKind};

/// How deep the walks go: YouTube nests its renderers a dozen levels down, never this far.
const MAX_DEPTH: usize = 48;

static CLOCK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:(\d+):)?(\d{1,2}):(\d{2})$").expect("valid clock pattern"));
static TOPIC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*-\s*Topic$").expect("valid topic pattern"));
static SIZED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"=w\d+-h\d+").expect("valid size pattern"));

/// "1:02:03" or "3:21" as milliseconds.
pub(crate) fn clock_ms(text: &str) -> Option<u64> {
    let found = CLOCK.captures(text.trim())?;
    let number = |index: usize| {
        found
            .get(index)
            .and_then(|part| part.as_str().parse::<u64>().ok())
            .unwrap_or(0)
    };
    Some(((number(1) * 60 + number(2)) * 60 + number(3)) * 1000)
}

/// An auto-generated artist channel ("Daft Punk - Topic") by its artist's name.
pub(crate) fn without_topic(channel: &str) -> String {
    TOPIC.replace(channel.trim(), "").into_owned()
}

/// Every object under `key`, anywhere in `value`, in document order.
pub(crate) fn all_under<'a>(value: &'a Value, key: &str) -> Vec<&'a Value> {
    fn walk<'a>(value: &'a Value, key: &str, depth: usize, found: &mut Vec<&'a Value>) {
        if depth > MAX_DEPTH {
            return;
        }
        match value {
            Value::Object(map) => {
                for (name, child) in map {
                    if name == key {
                        found.push(child);
                    } else {
                        walk(child, key, depth + 1, found);
                    }
                }
            }
            Value::Array(items) => {
                for item in items {
                    walk(item, key, depth + 1, found);
                }
            }
            _ => {}
        }
    }
    let mut found = Vec::new();
    walk(value, key, 0, &mut found);
    found
}

/// The first object under `key`, anywhere in `value`.
pub(crate) fn first_under<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    all_under(value, key).into_iter().next()
}

fn text(value: &Value) -> Option<&str> {
    value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

/// YouTube's text objects: `{ simpleText }`, `{ runs: [{ text }] }` or `{ content }`.
pub(crate) fn text_of(value: &Value) -> Option<String> {
    if let Some(simple) = value.get("simpleText").and_then(text) {
        return Some(simple.to_owned());
    }
    if let Some(content) = value.get("content").and_then(text) {
        return Some(content.to_owned());
    }
    let runs: String = value
        .get("runs")?
        .as_array()?
        .iter()
        .filter_map(|run| run.get("text").and_then(Value::as_str))
        .collect();
    let runs = runs.trim();
    (!runs.is_empty()).then(|| runs.to_owned())
}

fn thumbnails_in(list: Option<&Value>) -> Vec<Thumbnail> {
    list.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let url = item.get("url").and_then(text)?;
            let url = if url.starts_with("//") {
                format!("https:{url}")
            } else {
                url.to_owned()
            };
            url.starts_with("https://").then(|| Thumbnail {
                url,
                width: item
                    .get("width")
                    .and_then(Value::as_u64)
                    .and_then(|n| u32::try_from(n).ok()),
                height: item
                    .get("height")
                    .and_then(Value::as_u64)
                    .and_then(|n| u32::try_from(n).ok()),
            })
        })
        .collect()
}

/// The pictures of a YouTube Music row, with a 544 px square of the largest added: the size
/// music.youtube.com shows on its player.
fn music_thumbnails(row: &Value) -> Vec<Thumbnail> {
    let mut found =
        thumbnails_in(row.pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails"));
    if let Some(largest) = found.last()
        && SIZED.is_match(&largest.url)
    {
        let url = SIZED.replace(&largest.url, "=w544-h544").into_owned();
        found.push(Thumbnail {
            url,
            width: Some(544),
            height: Some(544),
        });
    }
    found
}

fn page_type(run: &Value) -> Option<&str> {
    run.pointer(
        "/navigationEndpoint/browseEndpoint/browseEndpointContextSupportedConfigs/browseEndpointContextMusicConfig/pageType",
    )
    .and_then(Value::as_str)
}

/// Words that label a row's kind or count its plays rather than name who made it.
fn is_label(text: &str) -> bool {
    let lower = text.trim().to_lowercase();
    matches!(
        lower.as_str(),
        "song" | "video" | "episode" | "single" | "ep" | "album" | "playlist" | "•" | ""
    ) || lower.ends_with(" views")
        || lower.ends_with(" plays")
        || lower.ends_with(" view")
        || lower.ends_with(" play")
        || clock_ms(&lower).is_some()
}

/// One YouTube Music row: its title column, then "Artist, Artist • Album • 3:45" (songs) or
/// "Channel • 12M views • 3:45" (videos).
pub(crate) fn music_row(row: &Value, kind: VideoKind) -> Option<Video> {
    let columns: Vec<&Vec<Value>> = row
        .get("flexColumns")?
        .as_array()?
        .iter()
        .filter_map(|column| {
            column
                .pointer("/musicResponsiveListItemFlexColumnRenderer/text/runs")
                .and_then(Value::as_array)
        })
        .collect();
    let title_run = columns.first()?.first()?;
    let title = title_run.get("text").and_then(text)?.to_owned();
    let id = row
        .pointer("/playlistItemData/videoId")
        .or_else(|| title_run.pointer("/navigationEndpoint/watchEndpoint/videoId"))
        .and_then(text)
        .filter(|id| super::is_video_id(id))?
        .to_owned();
    let details: &[Value] = columns.get(1).map_or(&[], |runs| runs.as_slice());
    let artists: Vec<&str> = details
        .iter()
        .filter(|run| page_type(run) == Some("MUSIC_PAGE_TYPE_ARTIST"))
        .filter_map(|run| run.get("text").and_then(text))
        .collect();
    let channel = if artists.is_empty() {
        // A video's uploader is a plain channel link (or no link at all): the first named run.
        details
            .iter()
            .filter_map(|run| run.get("text").and_then(text))
            .find(|part| !is_label(part))
            .map(without_topic)
    } else {
        Some(without_topic(&artists.join(", ")))
    };
    let album = details
        .iter()
        .find(|run| page_type(run) == Some("MUSIC_PAGE_TYPE_ALBUM"))
        .and_then(|run| run.get("text").and_then(text))
        .map(str::to_owned);
    let duration_ms = details
        .iter()
        .filter_map(|run| run.get("text").and_then(Value::as_str))
        .find_map(clock_ms);
    let thumbnails = music_thumbnails(row);
    Some(Video {
        id,
        title,
        channel: channel.filter(|name| !name.is_empty()),
        album,
        duration_ms,
        thumbnail_url: thumbnails.last().map(|thumbnail| thumbnail.url.clone()),
        thumbnails,
        kind,
    })
}

/// The rows of a YouTube Music search answer, in order.
pub(crate) fn music_search(answer: &Value, kind: VideoKind) -> Vec<Video> {
    all_under(answer, "musicResponsiveListItemRenderer")
        .into_iter()
        .filter_map(|row| music_row(row, kind))
        .collect()
}

/// A video's own large picture, which every video has.
fn video_picture(id: &str) -> Thumbnail {
    Thumbnail {
        url: format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg"),
        width: Some(480),
        height: Some(360),
    }
}

/// A playlist row on youtube.com: the newer lockup card, or the older video renderer.
fn playlist_row(row: &Value) -> Option<Video> {
    if let Some(lockup) = row.get("lockupViewModel") {
        if lockup.get("contentType").and_then(Value::as_str) != Some("LOCKUP_CONTENT_TYPE_VIDEO") {
            return None;
        }
        let id = lockup
            .get("contentId")
            .and_then(text)
            .filter(|id| super::is_video_id(id))?
            .to_owned();
        let metadata = lockup.pointer("/metadata/lockupMetadataViewModel")?;
        let title = metadata.get("title").and_then(text_of)?;
        // "Channel", then "12M views • 3 years ago".
        let channel = metadata
            .pointer("/metadata/contentMetadataViewModel/metadataRows")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .flat_map(|row| {
                row.get("metadataParts")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
            })
            .filter_map(|part| part.get("text").and_then(text_of))
            .find(|part| !is_label(part) && !part.ends_with(" ago"))
            .map(|name| without_topic(&name));
        let duration_ms = all_under(lockup, "thumbnailBadgeViewModel")
            .into_iter()
            .filter_map(|badge| badge.get("text").and_then(Value::as_str))
            .find_map(clock_ms);
        let mut thumbnails =
            thumbnails_in(lockup.pointer("/contentImage/thumbnailViewModel/image/sources"));
        thumbnails.push(video_picture(&id));
        return Some(Video {
            thumbnail_url: Some(video_picture(&id).url),
            id,
            title,
            channel: channel.filter(|name| !name.is_empty()),
            album: None,
            duration_ms,
            thumbnails,
            kind: VideoKind::Video,
        });
    }
    let renderer = row
        .get("playlistVideoRenderer")
        .or_else(|| row.get("playlistPanelVideoRenderer"))?;
    let id = renderer
        .get("videoId")
        .and_then(text)
        .filter(|id| super::is_video_id(id))?
        .to_owned();
    let title = renderer.get("title").and_then(text_of)?;
    let channel = renderer
        .get("shortBylineText")
        .or_else(|| renderer.get("longBylineText"))
        .and_then(text_of)
        .map(|name| without_topic(&name));
    let duration_ms = renderer
        .get("lengthSeconds")
        .and_then(|seconds| {
            seconds
                .as_str()
                .and_then(|seconds| seconds.parse::<u64>().ok())
                .or_else(|| seconds.as_u64())
        })
        .map(|seconds| seconds * 1000)
        .or_else(|| {
            renderer
                .get("lengthText")
                .and_then(text_of)
                .and_then(|length| clock_ms(&length))
        });
    let mut thumbnails = thumbnails_in(renderer.pointer("/thumbnail/thumbnails"));
    thumbnails.push(video_picture(&id));
    Some(Video {
        thumbnail_url: Some(video_picture(&id).url),
        id,
        title,
        channel: channel.filter(|name| !name.is_empty()),
        album: None,
        duration_ms,
        thumbnails,
        kind: VideoKind::Video,
    })
}

/// Every playlist row in `value`, in order.
fn videos_in(value: &Value, depth: usize, found: &mut Vec<Video>) {
    if depth > MAX_DEPTH {
        return;
    }
    match value {
        Value::Object(map) => {
            if map.contains_key("lockupViewModel")
                || map.contains_key("playlistVideoRenderer")
                || map.contains_key("playlistPanelVideoRenderer")
            {
                found.extend(playlist_row(value));
                return;
            }
            for child in map.values() {
                videos_in(child, depth + 1, found);
            }
        }
        Value::Array(items) => {
            for item in items {
                videos_in(item, depth + 1, found);
            }
        }
        _ => {}
    }
}

/// The videos of a playlist page or of a further page of it, in order.
pub(crate) fn playlist_videos(page: &Value) -> Vec<Video> {
    let mut found = Vec::new();
    // A further page's items come in its actions; a first page's under its contents.
    for part in [
        "contents",
        "onResponseReceivedActions",
        "onResponseReceivedEndpoints",
    ] {
        if let Some(value) = page.get(part) {
            videos_in(value, 0, &mut found);
        }
    }
    found
}

/// A watch page's playlist panel: what a mix (a radio playlist, `RD…`) has instead of a page.
fn mix_panel(answer: &Value) -> Option<&Value> {
    answer
        .pointer("/contents/twoColumnWatchNextResults/playlist/playlist")
        .or_else(|| first_under(answer, "playlistPanelRenderer"))
}

/// The videos of a mix, which only a watch page's panel lists.
pub(crate) fn mix_videos(answer: &Value) -> Vec<Video> {
    let mut found = Vec::new();
    if let Some(contents) = mix_panel(answer).and_then(|panel| panel.get("contents")) {
        videos_in(contents, 0, &mut found);
    }
    found
}

/// A mix's title and who made it.
pub(crate) fn mix_header(answer: &Value) -> (Option<String>, Option<String>) {
    let Some(panel) = mix_panel(answer) else {
        return (None, None);
    };
    let field = |name: &str| {
        panel
            .get(name)
            .and_then(|value| text(value).map(str::to_owned).or_else(|| text_of(value)))
    };
    (
        field("title").or_else(|| field("titleText")),
        field("ownerName").map(|name| without_topic(&name)),
    )
}

/// The first continuation token in `value`: a `continuationCommand`'s, which youtube.com nests
/// inside another command of the same name.
fn first_token(value: &Value, depth: usize) -> Option<&str> {
    if depth > MAX_DEPTH {
        return None;
    }
    match value {
        Value::Object(map) => {
            if let Some(token) = map
                .get("continuationCommand")
                .and_then(|command| command.get("token"))
                .and_then(text)
            {
                return Some(token);
            }
            map.values().find_map(|child| first_token(child, depth + 1))
        }
        Value::Array(items) => items.iter().find_map(|item| first_token(item, depth + 1)),
        _ => None,
    }
}

/// The token of the page after this one, if there is one: the first continuation among the
/// page's items (youtube.com puts one at the end of the list).
pub(crate) fn next_page(page: &Value) -> Option<String> {
    [
        "contents",
        "onResponseReceivedActions",
        "onResponseReceivedEndpoints",
    ]
    .iter()
    .filter_map(|part| page.get(*part))
    .find_map(|value| first_token(value, 0))
    .map(str::to_owned)
}

/// A playlist page's title and owner.
pub(crate) fn playlist_header(page: &Value) -> (Option<String>, Option<String>) {
    let title = page
        .pointer("/metadata/playlistMetadataRenderer/title")
        .and_then(text)
        .map(str::to_owned)
        .or_else(|| {
            page.pointer("/header/playlistHeaderRenderer/title")
                .and_then(text_of)
        })
        .or_else(|| {
            page.pointer("/header/pageHeaderRenderer/pageTitle")
                .and_then(text)
                .map(str::to_owned)
        });
    let owner = page
        .pointer("/header/playlistHeaderRenderer/ownerText")
        .and_then(text_of)
        .or_else(|| {
            // The newer header: an avatar stack saying "by Someone".
            page.pointer("/header/pageHeaderRenderer")
                .and_then(|header| first_under(header, "avatarStackViewModel"))
                .and_then(|stack| stack.get("text"))
                .and_then(text_of)
                .map(|by| by.strip_prefix("by ").unwrap_or(&by).trim().to_owned())
        })
        .map(|name| without_topic(&name))
        .filter(|name| !name.is_empty());
    (title, owner)
}

/// One adaptive format of a `player` answer.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Format {
    pub itag: u32,
    pub url: Option<String>,
    pub mime_type: String,
    pub bitrate: Option<u64>,
    pub average_bitrate: Option<u64>,
    pub content_length: Option<u64>,
    pub is_drc: bool,
    /// `Some(false)` for a dubbed or described track beside the video's own. Asked for in
    /// English, YouTube may make a track dubbed into it the default.
    pub audio_is_default: Option<bool>,
    /// The video's own audio, where it has dubbed tracks too: its track's name says "original",
    /// or its address's tags do (`xtags=acont=original:…`).
    pub original: bool,
    /// A dubbed track, by people or translated automatically (`acont=dubbed`, `dubbed-auto`).
    pub dubbed: bool,
    /// An audio description for people who can't see the picture.
    pub descriptive: bool,
    pub approx_duration_ms: Option<u64>,
}

impl Format {
    pub fn is_audio(&self) -> bool {
        self.mime_type.starts_with("audio/")
    }

    pub fn is_aac(&self) -> bool {
        self.mime_type.starts_with("audio/mp4")
    }

    pub fn kbps(&self) -> u32 {
        let bits = self.average_bitrate.or(self.bitrate).unwrap_or(0);
        u32::try_from((bits + 500) / 1000).unwrap_or(u32::MAX)
    }

    /// `webm` or `m4a`.
    pub fn container(&self) -> Option<&'static str> {
        if self.mime_type.starts_with("audio/webm") {
            Some("webm")
        } else if self.is_aac() {
            Some("m4a")
        } else {
            None
        }
    }

    /// `opus` or `mp4a.40.2`, from the MIME type's codecs.
    pub fn codec(&self) -> Option<String> {
        let (_, codecs) = self.mime_type.split_once("codecs=")?;
        let codec = codecs.trim_matches(|c: char| c == '"' || c.is_whitespace());
        (!codec.is_empty()).then(|| codec.to_owned())
    }
}

fn number(value: Option<&Value>) -> Option<u64> {
    let value = value?;
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
}

/// The audio content a stream address's tags name (`xtags=acont%3Ddubbed-auto%3Alang%3Des`):
/// `original`, `dubbed`, `dubbed-auto`, `descriptive`, `secondary`.
fn audio_content(url: &str) -> Option<String> {
    let tags = url::Url::parse(url)
        .ok()?
        .query_pairs()
        .find(|(name, _)| name == "xtags")
        .map(|(_, tags)| tags.into_owned())?;
    tags.split(':')
        .find_map(|tag| tag.strip_prefix("acont="))
        .map(str::to_ascii_lowercase)
}

/// The `clen` an address carries: the file's length.
pub(crate) fn url_param<'a>(url: &'a str, name: &str) -> Option<&'a str> {
    let (_, query) = url.split_once('?')?;
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
}

/// The adaptive formats of a `player` answer.
pub(crate) fn formats(answer: &Value) -> Vec<Format> {
    answer
        .pointer("/streamingData/adaptiveFormats")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|format| {
            let url = format
                .get("url")
                .and_then(text)
                .filter(|url| url.starts_with("https://"))
                .map(str::to_owned);
            let content_length = number(format.get("contentLength")).or_else(|| {
                url.as_deref()
                    .and_then(|url| url_param(url, "clen"))
                    .and_then(|length| length.parse().ok())
            });
            // Asked for in English: the track names say "original" and "descriptive" in it.
            let track_name = format
                .pointer("/audioTrack/displayName")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_lowercase();
            let content = url.as_deref().and_then(audio_content);
            let content = content.as_deref();
            Some(Format {
                itag: u32::try_from(number(format.get("itag"))?).ok()?,
                mime_type: format.get("mimeType").and_then(text)?.to_owned(),
                bitrate: number(format.get("bitrate")),
                average_bitrate: number(format.get("averageBitrate")),
                content_length,
                is_drc: format
                    .get("isDrc")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                audio_is_default: format
                    .pointer("/audioTrack/audioIsDefault")
                    .and_then(Value::as_bool),
                original: content == Some("original") || track_name.contains("original"),
                dubbed: content.is_some_and(|content| content.starts_with("dubbed"))
                    || format
                        .pointer("/audioTrack/isAutoDubbed")
                        .and_then(Value::as_bool)
                        == Some(true),
                descriptive: content == Some("descriptive") || track_name.contains("descriptive"),
                approx_duration_ms: number(format.get("approxDurationMs")),
                url,
            })
        })
        .collect()
}

/// What a `player` answer says about playing the video.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Playability {
    Ok,
    /// Gone for every client (removed, private): `reason` is YouTube's.
    Gone(String),
    /// This client may not play it, another may (or a sign-in is wanted).
    Refused(String),
}

pub(crate) fn playability(answer: &Value) -> Playability {
    let status = answer
        .pointer("/playabilityStatus/status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let reason = answer
        .pointer("/playabilityStatus/reason")
        .and_then(text)
        .map(str::to_owned);
    match status {
        "OK" => Playability::Ok,
        "ERROR" => Playability::Gone(match reason {
            Some(reason) => format!("YouTube can't play it: {reason}"),
            None => "YouTube can't play it".to_owned(),
        }),
        "LOGIN_REQUIRED"
            if reason
                .as_deref()
                .is_some_and(|reason| reason.contains("age")) =>
        {
            Playability::Refused("YouTube wants a sign-in to play it (age-restricted)".to_owned())
        }
        "UNPLAYABLE" => Playability::Refused(match reason {
            Some(reason) => format!("YouTube can't play it: {reason}"),
            None => "YouTube can't play it".to_owned(),
        }),
        _ => Playability::Refused(match reason {
            Some(reason) => format!("YouTube didn't play it: {reason}"),
            None => format!("YouTube didn't play it ({status})"),
        }),
    }
}

/// What a `player` answer tells about the video itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Details {
    pub title: Option<String>,
    pub channel: Option<String>,
    pub duration_ms: Option<u64>,
    pub expires_in_seconds: Option<u64>,
    pub upload_date: Option<String>,
}

pub(crate) fn details(answer: &Value) -> Details {
    let video = answer.get("videoDetails");
    let field = |name: &str| {
        video
            .and_then(|video| video.get(name))
            .and_then(text)
            .map(str::to_owned)
    };
    Details {
        title: field("title"),
        channel: field("author").map(|author| without_topic(&author)),
        duration_ms: video
            .and_then(|video| number(video.get("lengthSeconds")))
            .filter(|seconds| *seconds > 0)
            .map(|seconds| seconds * 1000),
        expires_in_seconds: number(answer.pointer("/streamingData/expiresInSeconds")),
        upload_date: answer
            .pointer("/microformat/playerMicroformatRenderer/uploadDate")
            .or_else(|| answer.pointer("/microformat/playerMicroformatRenderer/publishDate"))
            .and_then(Value::as_str)
            .and_then(|date| {
                let digits: String = date.chars().take(10).filter(char::is_ascii_digit).collect();
                (digits.len() == 8).then_some(digits)
            }),
    }
}

// ---- Live streams ----

/// A live stream's playlist (HLS, every quality) in a `player` answer.
pub(crate) fn hls_manifest(answer: &Value) -> Option<String> {
    answer
        .pointer("/streamingData/hlsManifestUrl")
        .and_then(text)
        .filter(|url| url.starts_with("https://"))
        .map(str::to_owned)
}

/// Whether a `player` answer is of a stream on the air now.
pub(crate) fn is_live(answer: &Value) -> bool {
    answer
        .pointer("/videoDetails/isLive")
        .and_then(Value::as_bool)
        == Some(true)
}

/// Whether a `player` answer is of a live stream at all: on the air, coming up or streamed before.
pub(crate) fn is_live_content(answer: &Value) -> bool {
    answer
        .pointer("/videoDetails/isLiveContent")
        .and_then(Value::as_bool)
        == Some(true)
        || answer
            .pointer("/playabilityStatus/liveStreamability")
            .is_some()
}

/// The people watching a live stream, as youtube.com counts them in English: "23,849 watching",
/// "3.6K watching", "1.2M watching".
pub(crate) fn watching(text: &str) -> Option<u64> {
    static WATCHING: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^\s*([\d.,]+)\s*([KMB])?\s+watching").expect("valid watching pattern")
    });
    let found = WATCHING.captures(text)?;
    let number: f64 = found.get(1)?.as_str().replace(',', "").parse().ok()?;
    let scale = match found.get(2).map(|unit| unit.as_str().to_ascii_uppercase()) {
        Some(unit) if unit == "K" => 1e3,
        Some(unit) if unit == "M" => 1e6,
        Some(unit) if unit == "B" => 1e9,
        _ => 1.0,
    };
    let count = (number * scale).round();
    (count.is_finite() && count >= 0.0).then_some(count as u64)
}

/// A youtube.com video renderer on a live listing (search, the Live page, older channel pages).
fn live_video_row(renderer: &Value) -> Option<LiveVideo> {
    let id = renderer
        .get("videoId")
        .and_then(text)
        .filter(|id| super::is_video_id(id))?
        .to_owned();
    let title = renderer.get("title").and_then(text_of)?;
    let byline = ["longBylineText", "ownerText", "shortBylineText"]
        .iter()
        .find_map(|key| renderer.get(*key));
    let channel = byline.and_then(text_of);
    let channel_id = byline
        .and_then(|byline| byline.pointer("/runs/0/navigationEndpoint/browseEndpoint/browseId"))
        .and_then(text)
        .map(str::to_owned);
    let overlay_styles: Vec<&str> = renderer
        .get("thumbnailOverlays")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|overlay| {
            overlay
                .pointer("/thumbnailOverlayTimeStatusRenderer/style")
                .and_then(Value::as_str)
        })
        .collect();
    let badged_live = renderer
        .get("badges")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|badge| {
            badge
                .pointer("/metadataBadgeRenderer/style")
                .and_then(Value::as_str)
                == Some("BADGE_STYLE_TYPE_LIVE_NOW")
        });
    let live = badged_live || overlay_styles.contains(&"LIVE");
    let upcoming = !live
        && (renderer.get("upcomingEventData").is_some() || overlay_styles.contains(&"UPCOMING"));
    let viewers = live
        .then(|| {
            ["viewCountText", "shortViewCountText"]
                .iter()
                .filter_map(|key| renderer.get(*key).and_then(text_of))
                .find_map(|count| watching(&count))
        })
        .flatten();
    let thumbnails = thumbnails_in(renderer.pointer("/thumbnail/thumbnails"));
    Some(LiveVideo {
        thumbnail_url: thumbnails.last().map(|picture| picture.url.clone()),
        id,
        title,
        channel,
        channel_id,
        live,
        upcoming,
        viewers,
        thumbnails,
    })
}

/// A newer lockup card on a live listing (a channel's Live tab): a "LIVE" badge, then "3.6K
/// watching", or "Streamed 4 years ago" for one streamed before.
fn live_lockup_row(lockup: &Value) -> Option<LiveVideo> {
    if lockup.get("contentType").and_then(Value::as_str) != Some("LOCKUP_CONTENT_TYPE_VIDEO") {
        return None;
    }
    let id = lockup
        .get("contentId")
        .and_then(text)
        .filter(|id| super::is_video_id(id))?
        .to_owned();
    let metadata = lockup.pointer("/metadata/lockupMetadataViewModel")?;
    let title = metadata.get("title").and_then(text_of)?;
    let parts: Vec<String> = metadata
        .pointer("/metadata/contentMetadataViewModel/metadataRows")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|row| {
            row.get("metadataParts")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter_map(|part| part.get("text").and_then(text_of))
        .collect();
    let badges = all_under(lockup, "thumbnailBadgeViewModel");
    let live = badges.iter().any(|badge| {
        badge.get("badgeStyle").and_then(Value::as_str)
            == Some("THUMBNAIL_OVERLAY_BADGE_STYLE_LIVE")
            || badge.get("text").and_then(Value::as_str) == Some("LIVE")
    });
    let upcoming = !live
        && (badges
            .iter()
            .any(|badge| badge.get("text").and_then(Value::as_str) == Some("UPCOMING"))
            || parts
                .iter()
                .any(|part| part.starts_with("Scheduled for") || part.ends_with(" waiting")));
    let viewers = live
        .then(|| parts.iter().find_map(|part| watching(part)))
        .flatten();
    let channel = parts
        .iter()
        .find(|part| {
            !is_label(part)
                && watching(part).is_none()
                && !part.ends_with(" ago")
                && !part.ends_with(" waiting")
                && !part.starts_with("Streamed ")
                && !part.starts_with("Scheduled for")
        })
        .map(|name| without_topic(name))
        .filter(|name| !name.is_empty());
    let thumbnails =
        thumbnails_in(lockup.pointer("/contentImage/thumbnailViewModel/image/sources"));
    Some(LiveVideo {
        thumbnail_url: thumbnails.last().map(|picture| picture.url.clone()),
        id,
        title,
        channel,
        channel_id: None,
        live,
        upcoming,
        viewers,
        thumbnails,
    })
}

/// Every video on a live listing, in order, each once: what's on the air, coming up, or was
/// streamed before, wherever the page nests them.
pub(crate) fn live_videos(answer: &Value) -> Vec<LiveVideo> {
    fn walk(value: &Value, depth: usize, found: &mut Vec<LiveVideo>) {
        if depth > MAX_DEPTH {
            return;
        }
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    let row = match key.as_str() {
                        "videoRenderer" | "gridVideoRenderer" => live_video_row(child),
                        "lockupViewModel" => live_lockup_row(child),
                        _ => {
                            walk(child, depth + 1, found);
                            continue;
                        }
                    };
                    if let Some(row) = row
                        && !found.iter().any(|seen| seen.id == row.id)
                    {
                        found.push(row);
                    }
                }
            }
            Value::Array(items) => {
                for item in items {
                    walk(item, depth + 1, found);
                }
            }
            _ => {}
        }
    }
    let mut found = Vec::new();
    walk(answer, 0, &mut found);
    found
}

/// A channel page's name and picture.
pub(crate) fn channel_header(page: &Value) -> (Option<String>, Option<String>) {
    let metadata = page.pointer("/metadata/channelMetadataRenderer");
    let title = metadata
        .and_then(|metadata| metadata.get("title"))
        .and_then(text)
        .map(str::to_owned);
    let avatar = metadata
        .map(|metadata| thumbnails_in(metadata.pointer("/avatar/thumbnails")))
        .and_then(|pictures| pictures.last().map(|picture| picture.url.clone()));
    (title, avatar)
}

/// The browse parameters of a channel page's Live tab (its address ends in `/streams`).
pub(crate) fn streams_tab(page: &Value) -> Option<String> {
    all_under(page, "tabRenderer").into_iter().find_map(|tab| {
        let endpoint = tab.get("endpoint")?;
        let url = endpoint
            .pointer("/commandMetadata/webCommandMetadata/url")
            .and_then(Value::as_str)?;
        url.ends_with("/streams")
            .then(|| endpoint.pointer("/browseEndpoint/params").and_then(text))
            .flatten()
            .map(str::to_owned)
    })
}

/// The channel `navigation/resolve_url` names for an address (`UC…`).
pub(crate) fn resolved_channel(answer: &Value) -> Option<String> {
    answer
        .pointer("/endpoint/browseEndpoint/browseId")
        .and_then(text)
        .filter(|id| super::is_channel_id(id))
        .map(str::to_owned)
}

/// The visitor token in YouTube's service worker data (`www.youtube.com/sw.js_data`): the
/// anonymous session its apps open, without which `player` asks to sign in.
pub(crate) fn visitor_in_sw_data(body: &str) -> Option<String> {
    let start = body.find('[')?;
    let data: Value = serde_json::from_str(&body[start..]).ok()?;
    let known = data
        .pointer("/0/2/0/0/13")
        .and_then(Value::as_str)
        .filter(|value| is_visitor(value));
    known.map(str::to_owned).or_else(|| find_visitor(&data, 0))
}

/// A visitor token as YouTube hands them out (base64 starting `Cg`).
pub(crate) fn is_visitor(value: &str) -> bool {
    value.len() >= 14
        && value.starts_with("Cg")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'%' | b'='))
}

fn find_visitor(value: &Value, depth: usize) -> Option<String> {
    if depth > 8 {
        return None;
    }
    match value {
        Value::String(text) if is_visitor(text) => Some(text.clone()),
        Value::Array(items) => items.iter().find_map(|item| find_visitor(item, depth + 1)),
        Value::Object(map) => map.values().find_map(|item| find_visitor(item, depth + 1)),
        _ => None,
    }
}

/// The web client's version a YouTube page states (`INNERTUBE_CLIENT_VERSION`).
pub(crate) fn client_version_in_page(page: &str) -> Option<String> {
    static VERSION: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#""INNERTUBE_CLIENT_VERSION":"([\d.]+)""#).expect("valid version pattern")
    });
    VERSION
        .captures(page)
        .and_then(|found| found.get(1))
        .map(|version| version.as_str().to_owned())
}
