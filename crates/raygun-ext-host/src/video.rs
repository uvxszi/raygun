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

/// The format of a torrent link (Miru's `torrent` videos): a `.torrent` file's address or a
/// magnet link, played by the app's torrent engine once torrents are turned on.
pub const TORRENT: &str = "torrent";

/// The format of a web page that plays the video in the site's own player (a site whose terms
/// allow only its embeddable player, such as Twitch's): opened in Raygun's browser.
pub const PAGE: &str = "page";

/// A torrent's link: a web address or a magnet link.
fn torrent_link(value: Option<&Value>) -> Option<String> {
    url(value).or_else(|| text(value).filter(|link| link.starts_with("magnet:?")))
}

fn videos_of(list: &Value) -> Vec<VideoLink> {
    list.as_array()
        .into_iter()
        .flatten()
        .filter_map(video_of)
        .take(MAX_VIDEOS)
        .collect()
}

/// One video of a `videos` answer (or one stream of a `streams` answer).
fn video_of(video: &Value) -> Option<VideoLink> {
    let quality = video
        .get("quality")
        .and_then(Value::as_u64)
        .filter(|height| (1..=10_000).contains(height))
        .map(|height| height as u32);
    let format = video
        .get("format")
        .and_then(Value::as_str)
        .filter(|format| matches!(*format, "hls" | "dash" | TORRENT | PAGE))
        .map(str::to_owned);
    let link = match format.as_deref() {
        Some(TORRENT) => torrent_link(video.get("url")),
        // A page opens in Raygun's browser: https only.
        Some(PAGE) => url(video.get("url")).filter(|page| page.starts_with("https://")),
        _ => url(video.get("url")).or_else(|| youtube_link(video.get("url"))),
    };
    Some(VideoLink {
        url: link?,
        label: label(video.get("label")).or_else(|| quality.map(|height| format!("{height}p"))),
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
}

// ---- `video.streams`: streams for a title known by its ids ----

/// Providers that find a movie's or an episode's streams by the ids the title is known by (its
/// TMDB and IMDb ids), as Stremio's stream addons do: Nuvio's providers (`crate::nuvio`), and any
/// extension written for it. Their streams play beside the addons' in a play session.
pub const VIDEO_STREAMS: &str = "video.streams";

/// How long a `video.streams` provider has for a title: as long as a Stremio addon's request.
pub const STREAMS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// Most streams one `video.streams` provider may give for a title.
const MAX_PROVIDED_STREAMS: usize = 100;
/// Longest name and description kept.
const MAX_STREAM_TEXT: usize = 200;

/// What a `video.streams` provider is asked: a movie or an episode, by the ids it's known by.
/// Sent as `streams({ type, tmdbId, imdbId, season, episode, title, year })`.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamQuery {
    /// `movie` or `series`.
    #[serde(rename = "type")]
    pub kind: String,
    /// TMDB's id (digits), when known.
    pub tmdb_id: Option<String>,
    /// IMDb's (`tt…`), when known.
    pub imdb_id: Option<String>,
    /// An episode's season and number, as TMDB and IMDb count them.
    pub season: Option<u32>,
    pub episode: Option<u32>,
    /// The title's name and year, for providers that search by them.
    pub title: Option<String>,
    pub year: Option<u32>,
}

/// One stream a `video.streams` provider gave.
#[derive(Debug, Clone, PartialEq)]
pub struct ProvidedStream {
    /// What the provider calls it (its source and quality, as a picker's first line).
    pub name: Option<String>,
    /// More about it: the file's name, its size, language.
    pub description: Option<String>,
    pub link: VideoLink,
    /// The file's size in bytes, when the provider says.
    pub size: Option<u64>,
}

/// A size as a number of bytes, or as written ("1.4 GB", "700 MB", "2,1 GiB").
fn size_of(value: Option<&Value>) -> Option<u64> {
    match value? {
        Value::Number(number) => number
            .as_f64()
            .filter(|bytes| *bytes >= 1.0)
            .map(|b| b as u64),
        Value::String(text) => {
            let text = text.trim().replace(',', ".");
            let split = text
                .find(|c: char| !(c.is_ascii_digit() || c == '.'))
                .unwrap_or(text.len());
            let number: f64 = text[..split].parse().ok()?;
            let unit = text[split..].trim().to_ascii_lowercase();
            let scale = match unit.trim_end_matches("ib").trim_end_matches('b') {
                "" => 1.0,
                "k" => 1024.0,
                "m" => 1024.0 * 1024.0,
                "g" => 1024.0 * 1024.0 * 1024.0,
                "t" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
                _ => return None,
            };
            let bytes = number * scale;
            (bytes >= 1.0 && bytes.is_finite()).then_some(bytes as u64)
        }
        _ => None,
    }
}

/// A picture height written as text ("1080p", "720", "4K"), or a number.
fn height_of(value: Option<&Value>) -> Option<u32> {
    let height = match value? {
        Value::Number(number) => number.as_u64()?,
        Value::String(text) => {
            let text = text.trim().to_ascii_lowercase();
            match text.as_str() {
                "4k" | "uhd" | "2160p" => 2160,
                "2k" | "1440p" | "qhd" => 1440,
                "fhd" | "full hd" => 1080,
                "hd" => 720,
                "sd" => 480,
                _ => text.trim_end_matches('p').parse().ok()?,
            }
        }
        _ => return None,
    };
    (1..=10_000).contains(&height).then_some(height as u32)
}

fn stream_text(value: Option<&Value>) -> Option<String> {
    text(value).map(|text| text.chars().take(MAX_STREAM_TEXT).collect())
}

/// A `streams` answer: `{ streams: [{ url, name, description, quality, size, format, headers,
/// subtitles }] }`, each as a `videos` answer's video with a name, a description and a size.
pub fn parse_provided_streams(value: &Value) -> Vec<ProvidedStream> {
    value
        .get("streams")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|stream| {
            let mut link = video_of(stream)?;
            if link.quality.is_none() {
                link.quality = height_of(stream.get("quality"));
            }
            Some(ProvidedStream {
                name: stream_text(stream.get("name")),
                description: stream_text(stream.get("description")),
                size: size_of(stream.get("size")),
                link,
            })
        })
        .take(MAX_PROVIDED_STREAMS)
        .collect()
}

/// One provider's answer to [`ExtensionHost::video_streams`].
#[derive(Debug)]
pub struct ProviderStreams {
    pub provider: raygun_model::ProviderInfo,
    pub answer: Result<Vec<ProvidedStream>, crate::CallError>,
    /// The provider failed lately (its calls did), so its answer comes last.
    pub failing: bool,
}

impl ExtensionHost {
    /// A movie's or an episode's streams from every `video.streams` provider that's on, in the
    /// order [`Self::video_streams_each`] asks them.
    pub async fn video_streams(
        &self,
        query: &StreamQuery,
        limit: std::time::Duration,
    ) -> Vec<ProviderStreams> {
        let providers = self.providers(VIDEO_STREAMS).await;
        let mut answers: Vec<(usize, ProviderStreams)> = Vec::new();
        let order: Vec<String> = self
            .health()
            .order(providers.clone())
            .into_iter()
            .map(|provider| provider.id)
            .collect();
        let mut each = std::pin::pin!(self.video_streams_each(providers, query, limit));
        while let Some(answer) = futures_util::StreamExt::next(&mut each).await {
            let at = order
                .iter()
                .position(|id| *id == answer.provider.id)
                .unwrap_or(usize::MAX);
            answers.push((at, answer));
        }
        answers.sort_by_key(|(at, _)| *at);
        answers.into_iter().map(|(_, answer)| answer).collect()
    }

    /// `providers`' (`video.streams`) streams for a movie or an episode, each answer as it
    /// comes: a few at a time, the healthy first, each within `limit`. Providers that kept
    /// failing sit out a while and answer at once with their last failure. Dropping the stream
    /// stops the calls still running.
    pub fn video_streams_each<'a>(
        &'a self,
        providers: Vec<raygun_model::ProviderInfo>,
        query: &StreamQuery,
        limit: std::time::Duration,
    ) -> impl futures_util::Stream<Item = ProviderStreams> + 'a {
        let args = serde_json::to_value(query).unwrap_or_default();
        let plan = crate::fanout::Plan {
            concurrency: 8,
            timeout: limit + std::time::Duration::from_secs(1),
            skip_resting: true,
        };
        let asks = crate::fanout::fan_out(self.health(), providers, plan, move |provider| {
            let args = args.clone();
            async move {
                self.call_within(&provider.id, "streams", args, limit)
                    .await
                    .map(|value| parse_provided_streams(&value))
            }
        });
        futures_util::StreamExt::map(asks, move |asked| ProviderStreams {
            failing: self.health().standing(&asked.provider.id) == crate::fanout::Standing::Failing,
            provider: asked.provider,
            answer: asked.answer,
        })
    }
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
        assert!(
            !videos[0].live && videos[2].live,
            "a source marks live streams"
        );
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
    fn pages_that_play_in_the_sites_own_player_are_https_addresses() {
        let videos = parse_videos(&json!({
            "videos": [
                { "url": "https://player.twitch.tv/?channel=example&parent=twitch.tv", "format": "page", "live": true },
                { "url": "http://player.example/watch", "format": "page" },
                { "url": "youtube:dQw4w9WgXcQ", "format": "page" },
            ]
        }));
        assert_eq!(videos.len(), 1, "plain http and other schemes are dropped");
        assert_eq!(videos[0].format.as_deref(), Some(PAGE));
        assert!(videos[0].live);
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
