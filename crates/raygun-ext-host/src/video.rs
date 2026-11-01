//! `video.source` providers (M42): the catalogue calls of `manga.source` (same parsers; details
//! add a type, year, cast and directors, episodes a season), then aniyomi's hoster model: an
//! episode's servers are listed first and each server's videos are resolved on their own, so the
//! app can show servers as they resolve.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::ExtensionHost;
use crate::manga::{MangaChapter, headers, id, parse_units, text, url};

pub const VIDEO_SOURCE: &str = "video.source";

/// Most servers one episode may list, videos one server may hold, and subtitles one video may carry.
const MAX_SERVERS: usize = 50;
const MAX_VIDEOS: usize = 50;
const MAX_SUBTITLES: usize = 50;
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoTrack {
    pub url: String,
    pub lang: Option<String>,
    pub label: Option<String>,
}

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
        Ok(parse_units(&value, "episodes", "Episode"))
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
pub fn parse_videos(value: &Value) -> Vec<VideoLink> {
    value.get("videos").map(videos_of).unwrap_or_default()
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
            Some(VideoLink {
                url: url(video.get("url"))?,
                label: label(video.get("label"))
                    .or_else(|| quality.map(|height| format!("{height}p"))),
                quality,
                headers: headers(video.get("headers")),
                format: video
                    .get("format")
                    .and_then(Value::as_str)
                    .filter(|format| matches!(*format, "hls" | "dash"))
                    .map(str::to_owned),
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
            })
        })
        .take(MAX_VIDEOS)
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

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
                { "url": "http://cdn.example/sd.mp4", "label": "SD", "quality": 99999 }
            ]
        }));
        assert_eq!(videos.len(), 2, "non-web URLs are dropped");
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
}
