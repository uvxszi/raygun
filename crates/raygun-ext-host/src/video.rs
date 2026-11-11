//! `video.source` providers (M42): the catalogue calls of `manga.source` (same parsers; details
//! add a type, year, cast and directors, episodes a season), then aniyomi's hoster model: an
//! episode's servers are listed first and each server's videos are resolved on their own, so the
//! app can show servers as they resolve.

use std::collections::BTreeMap;

use raygun_model::{ChapterKind, VideoChapter};
use serde_json::Value;

use crate::ExtensionHost;
use crate::manga::{MangaChapter, headers, id, parse_units, text, url};

pub const VIDEO_SOURCE: &str = "video.source";

/// Most servers one episode may list, videos one server may hold, and subtitles one video may carry.
const MAX_SERVERS: usize = 50;
const MAX_VIDEOS: usize = 50;
const MAX_SUBTITLES: usize = 50;
/// Most chapters one video may mark.
const MAX_CHAPTERS: usize = 100;
/// Longest server name, video label and subtitle label kept.
const MAX_LABEL: usize = 80;

/// A server (hoster) for an episode; `videos` when the source resolved it already.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoServer {
    pub id: String,
    pub name: String,
    pub videos: Option<Vec<VideoLink>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VideoLink {
    pub url: String,
    /// "1080p", "Dub", ...
    pub label: Option<String>,
    /// Height in pixels.
    pub quality: Option<u32>,
    pub headers: BTreeMap<String, String>,
    /// `hls` or `dash`; `None` for a progressive file (or unknown).
    pub format: Option<String>,
    pub subtitles: Vec<VideoTrack>,
    /// Openings, endings and other chapters, from the video's `timestamps` (M45).
    pub chapters: Vec<VideoChapter>,
    /// A live stream (`live: true`): it plays from its live edge, with no place to resume.
    pub live: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoTrack {
    pub url: String,
    pub lang: Option<String>,
    pub label: Option<String>,
}

/// Comments kept from one page.
const MAX_COMMENTS: usize = 100;

impl ExtensionHost {
    /// A title's episodes, in the source's order; a movie has one.
    pub async fn video_episodes(
        &self,
        provider: &str,
        id: &str,
    ) -> Result<Vec<MangaChapter>, String> {
        let value = self
            .call(provider, "episodes", serde_json::json!({ "id": id }))
            .await
            .map_err(|error| error.to_string())?;
        let mut episodes = parse_units(&value, "episodes", "Episode");
        fill_from_names(&mut episodes);
        Ok(episodes)
    }

    /// A title's comments, a page at a time (M46.45); a source without them has none.
    pub async fn video_comments(
        &self,
        provider: &str,
        id: &str,
        page: Option<&str>,
    ) -> Result<raygun_model::VideoComments, String> {
        let value = self
            .call_interactive(
                provider,
                "comments",
                serde_json::json!({ "id": id, "page": page }),
            )
            .await
            .map_err(|error| error.to_string())?;
        let mut answer: raygun_model::VideoComments =
            serde_json::from_value(value).map_err(|error| error.to_string())?;
        answer
            .comments
            .retain(|comment| !comment.text.trim().is_empty());
        answer.comments.truncate(MAX_COMMENTS);
        Ok(answer)
    }

    /// An episode's servers, some perhaps already resolved.
    pub async fn video_servers(
        &self,
        provider: &str,
        episode_id: &str,
    ) -> Result<Vec<VideoServer>, String> {
        let value = self
            .call(
                provider,
                "servers",
                serde_json::json!({ "episodeId": episode_id }),
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(parse_servers(&value))
    }

    /// One server's videos.
    pub async fn video_links(
        &self,
        provider: &str,
        server_id: &str,
    ) -> Result<Vec<VideoLink>, String> {
        let value = self
            .call(
                provider,
                "videos",
                serde_json::json!({ "serverId": server_id }),
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(parse_videos(&value))
    }
}

fn label(value: Option<&Value>) -> Option<String> {
    text(value).map(|label| label.chars().take(MAX_LABEL).collect())
}

/// Episodes as a source lists them, made whole: each id once (the first), a number read from the
/// name when the source gives none ("Episode 12", "第12話"), and seasons read from the names
/// ("S2 E5", "Season 2 Episode 5") when the source gives none and every name says one, so
/// seasons numbered from 1 each don't interleave.
pub fn fill_from_names(episodes: &mut Vec<MangaChapter>) {
    let mut seen = std::collections::HashSet::new();
    episodes.retain(|episode| seen.insert(episode.id.clone()));
    let read: Vec<raygun_resolver::video_match::EpisodeInfo> = episodes
        .iter()
        .map(|episode| raygun_resolver::video_match::parse_episode(&episode.name))
        .collect();
    for (episode, info) in episodes.iter_mut().zip(&read) {
        if episode.number.is_none() && !info.weak {
            episode.number = info.episode.filter(|number| *number >= 0.0);
        }
    }
    let seasons: Option<Vec<u32>> = read.iter().map(|info| info.season).collect();
    if let Some(seasons) = seasons
        && episodes.iter().all(|episode| episode.volume.is_none())
        && read.iter().all(|info| !info.weak && info.episode.is_some())
    {
        for (episode, season) in episodes.iter_mut().zip(seasons) {
            episode.volume = Some(season.to_string());
        }
    }
}

/// A `servers` answer: servers with ids and names, and any videos they came with.
pub fn parse_servers(value: &Value) -> Vec<VideoServer> {
    value
        .get("servers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|server| {
            let id = id(server.get("id")?)?;
            Some(VideoServer {
                name: label(server.get("name")).unwrap_or_else(|| id.clone()),
                videos: server.get("videos").map(videos_of),
                id,
            })
        })
        .take(MAX_SERVERS)
        .collect()
}

/// A `videos` answer.
/// `youtube:<video id>` (M46.35): a YouTube video, played in YouTube's embed.
fn youtube_link(value: Option<&Value>) -> Option<String> {
    let id = value?.as_str()?.strip_prefix("youtube:")?;
    let valid = id.len() == 11
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
    valid.then(|| format!("youtube:{id}"))
}

pub fn parse_videos(value: &Value) -> Vec<VideoLink> {
    value.get("videos").map(videos_of).unwrap_or_default()
}

/// The format a link marks as needing a torrent app (Miru's `torrent` videos): a `.torrent`
/// file's address or a magnet link. Raygun doesn't play these; the app says so.
pub const TORRENT: &str = "torrent";

/// A torrent's link: a web address or a magnet link.
fn torrent_link(value: Option<&Value>) -> Option<String> {
    url(value).or_else(|| text(value).filter(|link| link.starts_with("magnet:?")))
}

fn videos_of(list: &Value) -> Vec<VideoLink> {
    list.as_array()
        .into_iter()
        .flatten()
        .filter_map(|video| {
            let quality = video
                .get("quality")
                .and_then(Value::as_u64)
                .filter(|height| (1..=10_000).contains(height))
                .map(|height| height as u32);
            let format = video
                .get("format")
                .and_then(Value::as_str)
                .filter(|format| matches!(*format, "hls" | "dash" | TORRENT))
                .map(str::to_owned);
            let link = if format.as_deref() == Some(TORRENT) {
                torrent_link(video.get("url"))
            } else {
                url(video.get("url")).or_else(|| youtube_link(video.get("url")))
            };
            Some(VideoLink {
                url: link?,
                label: label(video.get("label"))
                    .or_else(|| quality.map(|height| format!("{height}p"))),
                quality,
                headers: headers(video.get("headers")),
                format,
                subtitles: video
                    .get("subtitles")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|track| {
                        Some(VideoTrack {
                            url: url(track.get("url"))?,
                            lang: label(track.get("lang")),
                            label: label(track.get("label")),
                        })
                    })
                    .take(MAX_SUBTITLES)
                    .collect(),
                chapters: video.get("timestamps").map(chapters_of).unwrap_or_default(),
                live: video.get("live").and_then(Value::as_bool) == Some(true),
            })
        })
        .take(MAX_VIDEOS)
        .collect()
}

/// A video's `timestamps` (seconds, Aniyomi's `TimeStamp`) as chapters in order. Ones that start
/// past a day or end before they start are dropped.
fn chapters_of(list: &Value) -> Vec<VideoChapter> {
    const DAY_MS: f64 = 86_400_000.0;
    let ms = |value: Option<&Value>| {
        value
            .and_then(Value::as_f64)
            .map(|seconds| seconds * 1000.0)
            .filter(|ms| ms.is_finite() && (0.0..DAY_MS).contains(ms))
            .map(|ms| ms as u32)
    };
    let mut chapters: Vec<VideoChapter> = list
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|stamp| {
            let start_ms = ms(stamp.get("start"))?;
            let end_ms = ms(stamp.get("end"));
            if end_ms.is_some_and(|end| end <= start_ms) {
                return None;
            }
            let kind = match stamp.get("type").and_then(Value::as_str) {
                Some("opening" | "mixedOp") => ChapterKind::Opening,
                Some("ending" | "mixedEd") => ChapterKind::Ending,
                Some("recap") => ChapterKind::Recap,
                _ => ChapterKind::Other,
            };
            Some(VideoChapter {
                start_ms,
                end_ms,
                name: label(stamp.get("name")),
                kind,
            })
        })
        .take(MAX_CHAPTERS)
        .collect();
    chapters.sort_by_key(|chapter| chapter.start_ms);
    chapters
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn youtube_videos_are_links_too() {
        let links = parse_videos(&json!({
            "videos": [
                { "url": "youtube:dQw4w9WgXcQ", "label": "YouTube" },
                { "url": "youtube:../../etc" },
                { "url": "javascript:alert(1)" }
            ]
        }));
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "youtube:dQw4w9WgXcQ");
    }

    #[test]
    fn torrents_stay_marked() {
        let links = parse_videos(&json!({
            "videos": [
                { "url": "https://nyaa.example/download/1.torrent", "format": "torrent" },
                { "url": "magnet:?xt=urn:btih:abc", "format": "torrent" },
                { "url": "magnet:?xt=urn:btih:abc" },
                { "url": "javascript:alert(1)", "format": "torrent" }
            ]
        }));
        assert_eq!(links.len(), 2, "a magnet link is kept only as a torrent");
        assert!(
            links
                .iter()
                .all(|link| link.format.as_deref() == Some(TORRENT))
        );
        assert_eq!(links[1].url, "magnet:?xt=urn:btih:abc");
    }

    #[test]
    fn timestamps_become_chapters() {
        let videos = parse_videos(&json!({
            "videos": [{
                "url": "https://cdn.example/1.mp4",
                "timestamps": [
                    { "start": 1300, "end": 1390.5, "name": "ED", "type": "ending" },
                    { "start": 0, "end": 85, "type": "mixedOp" },
                    { "start": 600, "name": "Part B" },
                    { "start": 900, "end": 800, "type": "recap" },
                    { "start": -5, "type": "recap" },
                    { "end": 10 }
                ]
            }]
        }));
        let chapters: Vec<(u32, Option<u32>, ChapterKind, Option<&str>)> = videos[0]
            .chapters
            .iter()
            .map(|chapter| {
                (
                    chapter.start_ms,
                    chapter.end_ms,
                    chapter.kind,
                    chapter.name.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            chapters,
            vec![
                (0, Some(85_000), ChapterKind::Opening, None),
                (600_000, None, ChapterKind::Other, Some("Part B")),
                (1_300_000, Some(1_390_500), ChapterKind::Ending, Some("ED")),
            ]
        );
    }

    #[test]
    fn servers_and_videos_are_checked() {
        let servers = parse_servers(&json!({
            "servers": [
                { "id": "a", "name": "Fast host" },
                { "id": 7, "videos": [{ "url": "https://cdn.example/7.m3u8", "format": "hls" }] },
                { "name": "no id" },
                "junk"
            ]
        }));
        assert_eq!(servers.len(), 2);
        assert_eq!(
            (servers[0].name.as_str(), servers[0].videos.is_none()),
            ("Fast host", true)
        );
        assert_eq!(
            servers[1].name, "7",
            "a nameless server is called by its id"
        );
        let resolved = servers[1].videos.as_ref().unwrap();
        assert_eq!(resolved[0].format.as_deref(), Some("hls"));

        let videos = parse_videos(&json!({
            "videos": [
                {
                    "url": "https://cdn.example/1080.mp4",
                    "quality": 1080,
                    "headers": { "Referer": "https://site.example/", "Host": "evil" },
                    "format": "progressive",
                    "subtitles": [
                        { "url": "https://cdn.example/en.vtt", "lang": "en", "label": "English" },
                        { "url": "file:///etc/passwd" }
                    ]
                },
                { "url": "javascript:alert(1)", "quality": 720 },
                { "url": "http://cdn.example/sd.mp4", "label": "SD", "quality": 99999 },
                { "url": "https://cdn.example/live.m3u8", "format": "hls", "live": true }
            ]
        }));
        assert_eq!(videos.len(), 3, "non-web URLs are dropped");
        assert!(!videos[0].live && videos[2].live, "a source marks live streams");
        assert_eq!(videos[0].label.as_deref(), Some("1080p"));
        assert_eq!(videos[0].format, None, "progressive files need no format");
        assert_eq!(
            videos[0].headers.keys().collect::<Vec<_>>(),
            ["Referer"],
            "hop-by-hop and host headers are dropped"
        );
        assert_eq!(videos[0].subtitles.len(), 1);
        assert_eq!(
            (videos[1].label.as_deref(), videos[1].quality),
            (Some("SD"), None),
            "an impossible height is ignored"
        );
    }

    #[test]
    fn episodes_keep_seasons() {
        let episodes = parse_units(
            &json!({ "episodes": [
                { "id": "s1e1", "season": 1, "number": 1 },
                { "id": "s1e2", "season": 1, "number": 2, "name": "The Return" },
                { "id": "film", "name": "Film" }
            ]}),
            "episodes",
            "Episode",
        );
        assert_eq!(episodes[0].name, "Episode 1");
        assert_eq!(episodes[0].volume.as_deref(), Some("1"));
        assert_eq!(episodes[1].name, "The Return");
        assert_eq!(
            (episodes[2].number, episodes[2].volume.as_deref()),
            (None, None)
        );
    }

    #[test]
    fn names_fill_in_numbers_and_seasons() {
        let parse = |episodes: Value| {
            let mut episodes = parse_units(&json!({ "episodes": episodes }), "episodes", "Episode");
            fill_from_names(&mut episodes);
            episodes
                .into_iter()
                .map(|episode| (episode.id, episode.volume, episode.number))
                .collect::<Vec<_>>()
        };
        let seasons = parse(json!([
            { "id": "a", "name": "S2 E5" },
            { "id": "b", "name": "Season 1 Episode 3" },
            { "id": "a", "name": "S2 E5 again" }
        ]));
        assert_eq!(
            seasons,
            [
                ("a".into(), Some("2".into()), Some(5.0)),
                ("b".into(), Some("1".into()), Some(3.0)),
            ],
            "each id once, seasons and numbers from the names"
        );
        let partly = parse(json!([
            { "id": "a", "name": "Episode 12" },
            { "id": "b", "name": "S2 E1" },
            { "id": "c", "name": "Film 2" }
        ]));
        assert_eq!(
            partly,
            [
                ("a".into(), None, Some(12.0)),
                ("b".into(), None, Some(1.0)),
                ("c".into(), None, None),
            ],
            "no seasons unless every name gives one; a trailing number is only a hint"
        );
        let given = parse(json!([{ "id": "a", "name": "Episode 9", "number": 4, "season": 3 }]));
        assert_eq!(
            given,
            [("a".into(), Some("3".into()), Some(4.0))],
            "the source's own numbers win"
        );
    }
}
