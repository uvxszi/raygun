//! Stream providers beside the addons: extensions of kind `video.streams` (Nuvio's providers,
//! `raygun_ext_host::nuvio`) find a movie's or an episode's streams by the ids the title is known
//! by, as stream addons do. Here a title's details become what they're asked
//! ([`stream_query`]), and their answers become stream groups like an addon's.

use raygun_ext_host::video::{ProvidedStream, ProviderStreams, StreamQuery};
use raygun_model::{StremioStream, StremioStreamGroup, StremioStreamKind, VideoSubtitle};

use super::protocol::Meta;

/// Whether an id is an IMDb title's (`tt` and digits).
fn imdb(id: &str) -> Option<String> {
    (id.len() > 2 && id.starts_with("tt") && id[2..].bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| id.to_owned())
}

fn digits(text: &str) -> Option<String> {
    (!text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())).then(|| text.to_owned())
}

/// Ids by the ones addons are asked with (`tt…`, `tt…:<season>:<episode>`, `tmdb:<id>` and
/// `tmdb:<id>:<season>:<episode>`): IMDb's, TMDB's, the season and the episode.
type Parsed = (Option<String>, Option<String>, Option<(u32, u32)>);

fn parse_id(id: &str) -> Parsed {
    let parts: Vec<&str> = id.split(':').collect();
    let numbers = |season: &str, episode: &str| season.parse().ok().zip(episode.parse().ok());
    match parts.as_slice() {
        [title] => (imdb(title), None, None),
        [title, season, episode] if imdb(title).is_some() => {
            (imdb(title), None, numbers(season, episode))
        }
        ["tmdb", title] => (None, digits(title), None),
        ["tmdb", title, season, episode] => (None, digits(title), numbers(season, episode)),
        _ => (None, None, None),
    }
}

/// What stream providers are asked for a movie or an episode of a title of `kind`: its TMDB and
/// IMDb ids (the details' own, else the ids the video goes by), and an episode's season and
/// number as TMDB and IMDb count them (an anime entry's videos map to IMDb's episodes, Kitsu
/// keeping one entry per season). `None` when there's nothing a provider could find it by: no
/// such id, an episode without numbers, a live channel.
pub fn stream_query(kind: &str, meta: Option<&Meta>, video_id: &str) -> Option<StreamQuery> {
    if matches!(kind, "tv" | "channel") || meta.is_some_and(|meta| meta.live) {
        return None;
    }
    let anime_numbering = ["kitsu", "mal", "anilist", "anidb"].iter().any(|prefix| {
        video_id.starts_with(prefix) || meta.is_some_and(|meta| meta.preview.id.starts_with(prefix))
    });
    let (mut imdb_id, mut tmdb_id, mut numbers) = parse_id(video_id);
    if let Some(meta) = meta {
        match meta.video(video_id) {
            Some(video) => match &video.imdb {
                // An anime entry's episode, mapped to IMDb's numbering.
                Some((imdb, Some(season), Some(episode))) => {
                    imdb_id = Some(imdb.clone());
                    numbers = Some((*season, *episode));
                    tmdb_id = None;
                }
                // A film among an anime's videos.
                Some((imdb, None, None)) if video.season.is_none() => {
                    imdb_id = Some(imdb.clone());
                    numbers = None;
                    tmdb_id = None;
                }
                _ if !anime_numbering => {
                    numbers = numbers.or(video.season.zip(video.episode));
                    imdb_id = imdb_id.or_else(|| meta.imdb_id.clone());
                    tmdb_id = tmdb_id.or_else(|| meta.tmdb_id.clone());
                }
                _ => return None,
            },
            None if !anime_numbering => {
                imdb_id = imdb_id.or_else(|| meta.imdb_id.clone());
                tmdb_id = tmdb_id.or_else(|| meta.tmdb_id.clone());
            }
            None => {}
        }
    }
    if imdb_id.is_none() && tmdb_id.is_none() {
        return None;
    }
    let series = numbers.is_some() || meta.is_some_and(|meta| !meta.videos.is_empty());
    if series && numbers.is_none() {
        return None;
    }
    Some(StreamQuery {
        kind: if series { "series" } else { "movie" }.to_owned(),
        tmdb_id,
        imdb_id,
        season: numbers.map(|(season, _)| season),
        episode: numbers.map(|(_, episode)| episode),
        title: meta.map(|meta| meta.preview.name.clone()),
        year: meta
            .and_then(|meta| super::release_year(meta.preview.release_info.as_deref()?))
            .and_then(|year| u32::try_from(year).ok()),
    })
}

/// Whether stream providers could find a title of `kind` with this id at all: a movie or a
/// series known by an IMDb or a TMDB id.
pub fn findable(kind: &str, id: &str) -> bool {
    if matches!(kind, "tv" | "channel") {
        return false;
    }
    let (imdb_id, tmdb_id, _) = parse_id(id);
    imdb_id.is_some() || tmdb_id.is_some() || id.split(':').next().and_then(imdb).is_some()
}

/// A provider's stream as an addon's: direct, or a torrent (its magnet link).
pub fn stream_view(stream: ProvidedStream) -> StremioStream {
    let link = stream.link;
    let torrent = link.format.as_deref() == Some(raygun_ext_host::video::TORRENT);
    StremioStream {
        kind: if torrent {
            StremioStreamKind::Torrent
        } else {
            StremioStreamKind::Direct
        },
        name: stream.name.or(link.label),
        description: stream.description,
        target: link.url,
        headers: link.headers,
        subtitles: link
            .subtitles
            .into_iter()
            .map(|track| VideoSubtitle {
                url: track.url,
                label: track.label.or_else(|| track.lang.clone()),
                lang: track.lang,
                format: None,
                source: None,
            })
            .collect(),
        binge_group: None,
        filename: None,
        size: stream.size.map(|size| size as f64),
        video_hash: None,
        not_web_ready: false,
    }
}

/// A provider's answer as a stream group, named after the provider. One that answered nothing
/// (no streams for this title, or a type it doesn't serve) is left out, as an addon that doesn't
/// serve a title isn't asked.
pub fn group_of(answer: ProviderStreams) -> Option<StremioStreamGroup> {
    let (streams, error) = match answer.answer {
        Ok(streams) if streams.is_empty() => return None,
        Ok(streams) => (streams.into_iter().map(stream_view).collect(), None),
        Err(error) => (Vec::new(), Some(error.to_string())),
    };
    Some(StremioStreamGroup {
        addon_id: answer.provider.id,
        addon_name: answer.provider.name,
        streams,
        error,
        failing: answer.failing,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::stremio::protocol::parse_meta;

    fn meta(value: serde_json::Value, kind: &str) -> Meta {
        parse_meta(&json!({ "meta": value }), kind).unwrap()
    }

    #[test]
    fn movies_are_asked_by_their_tmdb_and_imdb_ids() {
        let film = meta(
            json!({ "id": "tt0063350", "type": "movie", "name": "Night of the Living Dead",
                "releaseInfo": "1968", "moviedb_id": 10331 }),
            "movie",
        );
        let query = stream_query("movie", Some(&film), "tt0063350").unwrap();
        assert_eq!(
            query,
            StreamQuery {
                kind: "movie".into(),
                tmdb_id: Some("10331".into()),
                imdb_id: Some("tt0063350".into()),
                season: None,
                episode: None,
                title: Some("Night of the Living Dead".into()),
                year: Some(1968),
            }
        );
        // Without details, the id says it all; an id no provider knows asks nothing.
        let bare = stream_query("movie", None, "tmdb:10331").unwrap();
        assert_eq!(bare.tmdb_id.as_deref(), Some("10331"));
        assert_eq!(bare.imdb_id, None);
        assert_eq!(stream_query("movie", None, "yt_id:UC1:abc"), None);
        assert_eq!(stream_query("tv", None, "tt0063350"), None);
    }

    #[test]
    fn episodes_carry_their_numbers() {
        let series = meta(
            json!({ "id": "tt0903747", "type": "series", "name": "Breaking Bad",
                "releaseInfo": "2008-2013", "moviedb_id": "1396",
                "videos": [{ "id": "tt0903747:2:3", "season": 2, "episode": 3, "title": "Bit by a Dead Bee" }] }),
            "series",
        );
        let query = stream_query("series", Some(&series), "tt0903747:2:3").unwrap();
        assert_eq!(query.kind, "series");
        assert_eq!(query.tmdb_id.as_deref(), Some("1396"));
        assert_eq!(query.imdb_id.as_deref(), Some("tt0903747"));
        assert_eq!((query.season, query.episode), (Some(2), Some(3)));
        assert_eq!(query.year, Some(2008));
        // An anime entry's episode goes by the IMDb episode it maps to.
        let anime = meta(
            json!({ "id": "kitsu:1", "type": "series", "name": "Cowboy Bebop",
                "videos": [{ "id": "kitsu:1:5", "season": 1, "episode": 5, "title": "Ballad",
                  "imdb_id": "tt0213338", "imdbSeason": 1, "imdbEpisode": 5 },
                  { "id": "kitsu:1:6", "season": 1, "episode": 6, "title": "Sympathy" }] }),
            "series",
        );
        let mapped = stream_query("series", Some(&anime), "kitsu:1:5").unwrap();
        assert_eq!(mapped.imdb_id.as_deref(), Some("tt0213338"));
        assert_eq!(mapped.tmdb_id, None);
        assert_eq!((mapped.season, mapped.episode), (Some(1), Some(5)));
        assert_eq!(
            stream_query("series", Some(&anime), "kitsu:1:6"),
            None,
            "anime numbering isn't IMDb's"
        );
    }

    #[test]
    fn answers_become_groups() {
        use raygun_ext_host::video::parse_provided_streams;
        let streams = parse_provided_streams(&json!({ "streams": [
            { "url": "https://archive.example/film.mp4", "name": "Archive 1080p",
              "description": "film.mp4", "size": "1.2 GB", "quality": "1080p",
              "headers": { "Referer": "https://archive.example/" },
              "subtitles": [{ "url": "https://archive.example/en.srt", "lang": "en" }] },
            { "url": "magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d566095a10", "format": "torrent" },
        ] }));
        let provider = raygun_model::ProviderInfo {
            id: "nuvio.archive/streams".into(),
            extension_id: "nuvio.archive".into(),
            kind: "video.streams".into(),
            name: "Internet Archive".into(),
            search_pill: None,
            adult: false,
        };
        let group = group_of(ProviderStreams {
            provider: provider.clone(),
            answer: Ok(streams),
            failing: false,
        })
        .unwrap();
        assert_eq!(group.addon_id, "nuvio.archive/streams");
        assert_eq!(group.addon_name, "Internet Archive");
        let direct = &group.streams[0];
        assert_eq!(direct.kind, StremioStreamKind::Direct);
        assert_eq!(direct.name.as_deref(), Some("Archive 1080p"));
        assert_eq!(direct.size, Some(1_288_490_188.0));
        assert_eq!(direct.headers["Referer"], "https://archive.example/");
        assert_eq!(direct.subtitles[0].label.as_deref(), Some("en"));
        assert_eq!(group.streams[1].kind, StremioStreamKind::Torrent);
        let nothing = ProviderStreams {
            provider,
            answer: Ok(Vec::new()),
            failing: false,
        };
        assert_eq!(group_of(nothing), None);
    }
}
