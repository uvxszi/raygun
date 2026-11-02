//! AniSkip (M45): the openings, endings and recaps of anime episodes, from api.aniskip.com by
//! MyAnimeList id. A title without that id is looked up by name on AniList's public GraphQL API,
//! and the id found is kept per title.

use std::sync::OnceLock;

use raygun_db::Db;
use raygun_model::{AnimeEpisode, ChapterKind, VideoChapter};
use serde_json::{Value, json};

const ANISKIP: &str = "https://api.aniskip.com/v2/skip-times";
const ANILIST: &str = "https://graphql.anilist.co";
const MAL_QUERY: &str = "query ($search: String) { Media(search: $search, type: ANIME) { idMal } }";
/// The skip types asked for: openings, endings, their mixed versions, and recaps.
const TYPES: &str = "types[]=op&types[]=ed&types[]=mixed-op&types[]=mixed-ed&types[]=recap";

fn client() -> Result<&'static reqwest::Client, String> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client);
    }
    let client = raygun_net::http_client().map_err(|error| error.to_string())?;
    Ok(CLIENT.get_or_init(|| client))
}

/// The episode's skip times as chapters; none when AniSkip doesn't know it. `duration_ms` lets
/// AniSkip leave out times submitted for a differently cut episode.
pub async fn chapters(
    db: &Db,
    anime: &AnimeEpisode,
    duration_ms: f64,
) -> Result<Vec<VideoChapter>, String> {
    let mal_id = match anime.mal_id {
        Some(id) => id,
        None => match mal_id(db, &anime.title).await? {
            Some(id) => id,
            None => return Ok(Vec::new()),
        },
    };
    let seconds = (duration_ms / 1000.0).round().max(0.0) as u64;
    let url = format!(
        "{ANISKIP}/{mal_id}/{}?{TYPES}&episodeLength={seconds}",
        anime.episode
    );
    let response = client()?
        .get(url)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    // An episode nobody has submitted times for is a 404.
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(Vec::new());
    }
    let body: Value = response
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())?;
    Ok(parse_skip_times(&body))
}

/// AniSkip's `results` as chapters, in order. AniSkip answers with one result per type, which can
/// overlap: plain openings and endings win over the "mixed" ones (kept only when there is no plain
/// one), and a time overlapping one already kept is dropped.
pub fn parse_skip_times(body: &Value) -> Vec<VideoChapter> {
    let ms = |value: Option<&Value>| {
        value
            .and_then(Value::as_f64)
            .filter(|seconds| seconds.is_finite() && (0.0..86_400.0).contains(seconds))
            .map(|seconds| (seconds * 1000.0) as u32)
    };
    let results: Vec<(&str, u32, u32)> = body
        .get("results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|result| {
            let interval = result.get("interval")?;
            let start_ms = ms(interval.get("startTime"))?;
            let end_ms = ms(interval.get("endTime")).filter(|end| *end > start_ms)?;
            Some((
                result.get("skipType").and_then(Value::as_str)?,
                start_ms,
                end_ms,
            ))
        })
        .collect();
    let has = |kind: &str| results.iter().any(|(found, ..)| *found == kind);
    let mut candidates: Vec<VideoChapter> = results
        .iter()
        .filter_map(|&(kind, start_ms, end_ms)| {
            let kind = match kind {
                "op" => ChapterKind::Opening,
                "mixed-op" if !has("op") => ChapterKind::Opening,
                "ed" => ChapterKind::Ending,
                "mixed-ed" if !has("ed") => ChapterKind::Ending,
                "recap" => ChapterKind::Recap,
                _ => return None,
            };
            Some(VideoChapter {
                start_ms,
                end_ms: Some(end_ms),
                name: None,
                kind,
            })
        })
        .collect();
    candidates.sort_by_key(|chapter| chapter.start_ms);
    let mut chapters: Vec<VideoChapter> = Vec::new();
    for chapter in candidates {
        let overlaps = chapters
            .last()
            .and_then(|last| last.end_ms)
            .is_some_and(|end| chapter.start_ms < end);
        if !overlaps {
            chapters.push(chapter);
        }
    }
    chapters
}

/// AniList's answer: the best match's MyAnimeList id.
pub fn parse_mal_id(body: &Value) -> Option<u32> {
    body.pointer("/data/Media/idMal")
        .and_then(Value::as_u64)
        .and_then(|id| u32::try_from(id).ok())
        .filter(|id| *id > 0)
}

/// The MyAnimeList id for a series name, found once and kept (0 marks "not found").
async fn mal_id(db: &Db, title: &str) -> Result<Option<u32>, String> {
    let key = format!("video.malId.{}", title.trim().to_lowercase());
    if let Ok(Some(saved)) = raygun_db::local_value(db, &key).await
        && let Ok(id) = saved.parse::<u32>()
    {
        return Ok((id > 0).then_some(id));
    }
    let body: Value = client()?
        .post(ANILIST)
        .json(&json!({ "query": MAL_QUERY, "variables": { "search": title } }))
        .send()
        .await
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())?;
    let found = parse_mal_id(&body);
    if let Err(error) = raygun_db::set_local_value(db, &key, &found.unwrap_or(0).to_string()).await
    {
        tracing::warn!(%error, "could not keep the MyAnimeList id");
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_times_become_chapters_in_order() {
        let body = json!({
            "found": true,
            "results": [
                { "interval": { "startTime": 1342.795, "endTime": 1430.616 }, "skipType": "ed",
                  "skipId": "b", "episodeLength": 1446.9973 },
                { "interval": { "startTime": 128.406, "endTime": 218.406 }, "skipType": "op",
                  "skipId": "a", "episodeLength": 1441.9413 },
                { "interval": { "startTime": 3.533, "endTime": 181.625 }, "skipType": "mixed-op" },
                { "interval": { "startTime": 24.41, "endTime": 114.41 }, "skipType": "mixed-ed" },
                { "interval": { "startTime": 0.0, "endTime": 20.0 }, "skipType": "recap" },
                { "interval": { "startTime": 50.0, "endTime": 40.0 }, "skipType": "recap" },
                { "interval": { "startTime": 5.0, "endTime": 9.0 }, "skipType": "preview" }
            ],
            "message": "Successfully found skip times",
            "statusCode": 200
        });
        let summary: Vec<(u32, Option<u32>, ChapterKind)> = parse_skip_times(&body)
            .iter()
            .map(|chapter| (chapter.start_ms, chapter.end_ms, chapter.kind))
            .collect();
        assert_eq!(
            summary,
            vec![
                (0, Some(20_000), ChapterKind::Recap),
                (128_406, Some(218_406), ChapterKind::Opening),
                (1_342_795, Some(1_430_616), ChapterKind::Ending),
            ],
            "plain openings and endings win over mixed ones; backwards and unknown types go"
        );
        // Without a plain opening, the mixed one is used, and overlaps are dropped.
        let mixed = json!({ "results": [
            { "interval": { "startTime": 3.5, "endTime": 181.6 }, "skipType": "mixed-op" },
            { "interval": { "startTime": 100.0, "endTime": 130.0 }, "skipType": "recap" }
        ] });
        let summary: Vec<(u32, ChapterKind)> = parse_skip_times(&mixed)
            .iter()
            .map(|chapter| (chapter.start_ms, chapter.kind))
            .collect();
        assert_eq!(summary, vec![(3_500, ChapterKind::Opening)]);
        assert!(parse_skip_times(&json!({ "found": false, "results": [] })).is_empty());
    }

    #[test]
    fn the_mal_id_comes_from_anilist_media() {
        assert_eq!(
            parse_mal_id(&json!({ "data": { "Media": { "idMal": 52991 } } })),
            Some(52991)
        );
        assert_eq!(
            parse_mal_id(&json!({ "data": { "Media": { "idMal": null } } })),
            None
        );
        assert_eq!(parse_mal_id(&json!({ "data": { "Media": null } })), None);
    }
}
