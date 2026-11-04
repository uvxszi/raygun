//! AniSkip (M45): the openings, endings and recaps of anime episodes, from api.aniskip.com by
//! MyAnimeList id. Every season is an entry of its own there, with its own id and episodes
//! numbered from 1. An entry known by its Kitsu or AniList id gets its MyAnimeList id from
//! there; one known by name only is searched on AniList's public GraphQL API, and only a result
//! that is the same season counts ("Overlord II" never takes Overlord's times). Ids found are
//! kept.

use std::sync::OnceLock;

use raygun_db::Db;
use raygun_model::{AnimeEpisode, ChapterKind, VideoChapter};
use raygun_resolver::video_match::{self, TitleInfo};
use serde_json::{Value, json};

const ANISKIP: &str = "https://api.aniskip.com/v2/skip-times";
const ANILIST: &str = "https://graphql.anilist.co";
const KITSU: &str = "https://kitsu.app/api/edge/anime";
const SEARCH_QUERY: &str = "query ($search: String) { Page(perPage: 8) { media(search: $search, \
     type: ANIME) { idMal episodes title { romaji english native } synonyms } } }";
const ID_QUERY: &str = "query ($id: Int) { Media(id: $id, type: ANIME) { idMal episodes } }";
/// Where an id found by name is kept, after the lowercase name. (`video.malId.` held ids found
/// without checking the season; they're not read.)
const BY_NAME: &str = "video.malEntry.name:";
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
    let Some(entry) = entry(db, anime).await? else {
        return Ok(Vec::new());
    };
    // A number past the entry's count is counted across seasons: AniSkip has nothing for it,
    // or another season's times.
    if entry.episodes.is_some_and(|count| anime.episode > count) {
        return Ok(Vec::new());
    }
    let seconds = (duration_ms / 1000.0).round().max(0.0) as u64;
    let url = format!(
        "{ANISKIP}/{}/{}?{TYPES}&episodeLength={seconds}",
        entry.id, anime.episode
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

/// An anime entry on MyAnimeList: its id, and how many episodes it has when known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MalEntry {
    pub id: u32,
    pub episodes: Option<u32>,
}

fn positive(value: Option<&Value>) -> Option<u32> {
    value
        .and_then(Value::as_u64)
        .and_then(|number| u32::try_from(number).ok())
        .filter(|number| *number > 0)
}

/// AniList's answer for one entry: its MyAnimeList id.
pub fn parse_mal_id(body: &Value) -> Option<u32> {
    positive(body.pointer("/data/Media/idMal"))
}

/// Kitsu's mappings of an entry: its MyAnimeList id.
pub fn parse_kitsu_mal(body: &Value) -> Option<u32> {
    body.get("data")?
        .as_array()?
        .iter()
        .find(|mapping| {
            mapping
                .pointer("/attributes/externalSite")
                .and_then(Value::as_str)
                == Some("myanimelist/anime")
        })?
        .pointer("/attributes/externalId")?
        .as_str()?
        .trim()
        .parse()
        .ok()
        .filter(|id| *id > 0)
}

/// The AniList search result that is `title`'s season: one with a name that is the same show's
/// same season, else the best match when its names that name a season mostly agree with it (and
/// one does, when `title` names a later season). None rather than another season's entry.
pub fn pick_entry(body: &Value, title: &str) -> Option<MalEntry> {
    let wanted = video_match::parse_title(title);
    let candidates: Vec<(MalEntry, Vec<TitleInfo>)> = body
        .pointer("/data/Page/media")?
        .as_array()?
        .iter()
        .filter_map(|media| {
            let entry = MalEntry {
                id: positive(media.get("idMal"))?,
                episodes: positive(media.get("episodes")),
            };
            let titles = ["romaji", "english", "native"]
                .iter()
                .filter_map(|key| media.get("title")?.get(*key)?.as_str());
            let synonyms = media
                .get("synonyms")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str);
            let names = titles
                .chain(synonyms)
                .map(video_match::parse_title)
                .collect();
            Some((entry, names))
        })
        .collect();
    if let Some((entry, _)) = candidates
        .iter()
        .find(|(_, names)| names.iter().any(|name| name.same_title(&wanted)))
    {
        return Some(*entry);
    }
    let (entry, names) = candidates.first()?;
    let marked = names.iter().filter(|name| name.names_season());
    let agree = marked
        .clone()
        .filter(|name| name.same_season(&wanted))
        .count();
    let disagree = marked.count() - agree;
    let fits =
        if wanted.season.unwrap_or(1) > 1 || wanted.part.unwrap_or(1) > 1 || wanted.final_season {
            agree > disagree
        } else {
            agree >= disagree
        };
    fits.then_some(*entry)
}

/// A kept lookup: `Some(None)` when it found nothing ("0"), `None` when there's none kept.
async fn kept(db: &Db, key: &str) -> Option<Option<MalEntry>> {
    let saved = raygun_db::local_value(db, key).await.ok().flatten()?;
    let (id, episodes) = saved.split_once(':').unwrap_or((&saved, ""));
    let id: u32 = id.parse().ok()?;
    Some((id > 0).then(|| MalEntry {
        id,
        episodes: episodes.parse().ok(),
    }))
}

async fn keep(db: &Db, key: &str, found: Option<MalEntry>) {
    let text = match found {
        Some(MalEntry {
            id,
            episodes: Some(episodes),
        }) => format!("{id}:{episodes}"),
        Some(MalEntry { id, episodes: None }) => id.to_string(),
        None => "0".into(),
    };
    if let Err(error) = raygun_db::set_local_value(db, key, &text).await {
        tracing::warn!(%error, "could not keep the MyAnimeList id");
    }
}

/// The episode's MyAnimeList entry: its own id, else its Kitsu or AniList entry's, else the one
/// found by its name. Lookups are kept, failures not (a network error asks again next time).
async fn entry(db: &Db, anime: &AnimeEpisode) -> Result<Option<MalEntry>, String> {
    if let Some(id) = anime.mal_id {
        return Ok(Some(MalEntry { id, episodes: None }));
    }
    if let Some(kitsu) = anime.kitsu_id {
        let key = format!("video.malEntry.kitsu:{kitsu}");
        if let Some(found) = kept(db, &key).await {
            return Ok(found);
        }
        match kitsu_entry(kitsu).await {
            Ok(found) => {
                keep(db, &key, found).await;
                if found.is_some() {
                    return Ok(found);
                }
            }
            Err(error) => tracing::info!(%error, "Kitsu's mapping failed; searching by name"),
        }
    }
    if let Some(anilist) = anime.anilist_id {
        let key = format!("video.malEntry.anilist:{anilist}");
        if let Some(found) = kept(db, &key).await {
            return Ok(found);
        }
        let body = anilist_query(ID_QUERY, json!({ "id": anilist })).await?;
        let found = parse_mal_id(&body).map(|id| MalEntry {
            id,
            episodes: positive(body.pointer("/data/Media/episodes")),
        });
        keep(db, &key, found).await;
        if found.is_some() {
            return Ok(found);
        }
    }
    let key = format!("{BY_NAME}{}", anime.title.trim().to_lowercase());
    if let Some(found) = kept(db, &key).await {
        return Ok(found);
    }
    let body = anilist_query(SEARCH_QUERY, json!({ "search": anime.title })).await?;
    let found = pick_entry(&body, &anime.title);
    keep(db, &key, found).await;
    Ok(found)
}

async fn kitsu_entry(kitsu_id: u32) -> Result<Option<MalEntry>, String> {
    let url = format!("{KITSU}/{kitsu_id}/mappings?filter[externalSite]=myanimelist/anime");
    let body: Value = client()?
        .get(url)
        .header("Accept", "application/vnd.api+json")
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())?;
    Ok(parse_kitsu_mal(&body).map(|id| MalEntry { id, episodes: None }))
}

async fn anilist_query(query: &str, variables: Value) -> Result<Value, String> {
    client()?
        .post(ANILIST)
        .json(&json!({ "query": query, "variables": variables }))
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())
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

    #[test]
    fn kitsu_maps_an_entry_to_its_mal_id() {
        let body = json!({ "data": [
            { "id": "1", "attributes": { "externalSite": "anidb", "externalId": "4" } },
            { "id": "3985", "attributes": { "externalSite": "myanimelist/anime", "externalId": "25777" } }
        ] });
        assert_eq!(parse_kitsu_mal(&body), Some(25777));
        assert_eq!(parse_kitsu_mal(&json!({ "data": [] })), None);
    }

    /// AniList's answers as they come (2026-11-03), shortened.
    fn media(entries: &[(u32, Option<u32>, &str, &str, &[&str])]) -> Value {
        let media: Vec<Value> = entries
            .iter()
            .map(|(id, episodes, romaji, english, synonyms)| {
                json!({ "idMal": id, "episodes": episodes,
                    "title": { "romaji": romaji, "english": english, "native": null },
                    "synonyms": synonyms })
            })
            .collect();
        json!({ "data": { "Page": { "media": media } } })
    }

    #[test]
    fn a_search_only_takes_the_same_season() {
        let titans = media(&[
            (
                16498,
                Some(25),
                "Shingeki no Kyojin",
                "Attack on Titan",
                &["SnK", "AoT"],
            ),
            (
                40028,
                Some(16),
                "Shingeki no Kyojin: The Final Season",
                "Attack on Titan Final Season",
                &["SnK 4", "AoT 4"],
            ),
            (
                25777,
                Some(12),
                "Shingeki no Kyojin Season 2",
                "Attack on Titan Season 2",
                &["SnK 2", "AoT 2"],
            ),
            (
                35760,
                Some(12),
                "Shingeki no Kyojin Season 3",
                "Attack on Titan Season 3",
                &["SnK 3"],
            ),
            (
                38524,
                Some(10),
                "Shingeki no Kyojin Season 3 Part 2",
                "Attack on Titan Season 3 Part 2",
                &["SnK 3"],
            ),
        ]);
        let pick = |title: &str| pick_entry(&titans, title).map(|entry| entry.id);
        assert_eq!(pick("Attack on Titan"), Some(16498));
        assert_eq!(pick("Shingeki no Kyojin II"), Some(25777));
        assert_eq!(pick("Attack on Titan 2nd Season"), Some(25777));
        assert_eq!(pick("Attack on Titan Season 3 Part 2"), Some(38524));
        assert_eq!(pick("Attack on Titan: The Final Season"), Some(40028));
        assert_eq!(
            pick_entry(&titans, "Attack on Titan Season 2").map(|entry| entry.episodes),
            Some(Some(12))
        );
        assert_eq!(pick("Attack on Titan Season 5"), None, "no fifth season");

        // Only the best match is taken under another name, and only when its season agrees.
        let mushoku = media(&[(
            51179,
            Some(13),
            "Mushoku Tensei II: Isekai Ittara Honki Dasu",
            "Mushoku Tensei: Jobless Reincarnation Season 2",
            &["Mushoku Tensei: Isekai Ittara Honki Dasu 2nd Season"],
        )]);
        assert_eq!(
            pick_entry(&mushoku, "Jobless Reincarnation Season 2").map(|entry| entry.id),
            Some(51179)
        );
        assert_eq!(
            pick_entry(&mushoku, "Jobless Reincarnation").map(|entry| entry.id),
            None,
            "season 1 never takes season 2's times"
        );
        let first = media(&[(
            16498,
            Some(25),
            "Shingeki no Kyojin",
            "Attack on Titan",
            &["SnK"],
        )]);
        assert_eq!(
            pick_entry(&first, "Shingeki no Kyojin 2nd Season").map(|entry| entry.id),
            None,
            "season 2 never takes season 1's times"
        );
        assert_eq!(
            pick_entry(&first, "Kyojin").map(|entry| entry.id),
            Some(16498),
            "the best match of a name without a season"
        );
    }

    /// Kitsu's mapping, AniList's search and AniSkip's times, live. Run with
    /// `cargo test -p raygun-app -- --ignored aniskip_live --nocapture`.
    #[tokio::test]
    #[ignore = "needs network access to Kitsu, AniList and AniSkip"]
    async fn aniskip_live() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let episode = |title: &str, kitsu_id: Option<u32>| AnimeEpisode {
            title: title.into(),
            episode: 1,
            mal_id: None,
            kitsu_id,
            anilist_id: None,
        };
        // Attack on Titan Season 2 (Kitsu 8671) is MyAnimeList's 25777, not the first season.
        let by_kitsu = entry(&db, &episode("Attack on Titan Season 2", Some(8671)))
            .await
            .unwrap();
        assert_eq!(by_kitsu.map(|entry| entry.id), Some(25777));
        let by_name = entry(&db, &episode("Mushoku Tensei II", None))
            .await
            .unwrap();
        assert_eq!(by_name.map(|entry| entry.id), Some(51179));
        let times = chapters(
            &db,
            &episode("Attack on Titan Season 2", Some(8671)),
            1_450_000.0,
        )
        .await
        .unwrap();
        println!("{times:?}");
        assert!(
            times
                .iter()
                .any(|chapter| chapter.kind == ChapterKind::Opening),
            "season 2 episode 1 has an opening on AniSkip"
        );
    }
}
