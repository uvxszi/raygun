//! A `video.source`'s episodes put right where its answers come in, so every kind of source
//! (Raygun's own, Miru, Kodi, servers) gets the same reading:
//!
//! - **Lines** (`episodes` answered as `{ lines: [{ name, units: [{ name, id }] }] }`, as Miru's
//!   detail lists them): the units of each line are episodes, and lines listing the same episode
//!   (by its number, else its name) become its servers. Lines named as seasons are seasons. Lines
//!   named as distinct episodes whose units are qualities or mirrors ("第1集" → "1080P", "线路2")
//!   are the episodes themselves, their units the servers; units that are only qualities make
//!   one episode with a server each.
//! - **A lone unit whose servers or videos are episodes** ("Episode 1", "EP 02", "第3集",
//!   "S01E04", "05"): each becomes an episode of its own, its id the unit's with
//!   [`EPISODE_MARK`] and the episode after it, so later calls (after a restart too) know which
//!   of the unit's servers and videos are that episode's. Names that are resolutions stay
//!   qualities.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use regex::Regex;
use serde_json::Value;

use crate::manga::{MangaChapter, format_number, id, text};
use crate::video::{VideoLink, VideoServer};

/// Marks an episode made of part of a unit: `<unit id>#raygun-episode=<episode>`, and a server's
/// id the same way when its videos are several episodes'.
pub const EPISODE_MARK: &str = "#raygun-episode=";

/// How long telling a lone unit's episodes apart may take before the unit is kept as it is.
pub(crate) const PROBE_LIMIT: Duration = Duration::from_secs(8);
/// How long an answer read to tell episodes apart is kept for the call that follows it.
const ANSWER_KEPT: Duration = Duration::from_secs(120);
const MAX_ANSWERS: usize = 64;
/// Episodes whose servers came with their lines, kept by id.
const MAX_KEPT_EPISODES: usize = 5000;
/// Most lines, and units in a line, read from an answer.
const MAX_LINES: usize = 50;
const MAX_UNITS: usize = 5000;

static SEASON: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)season|staffel|saison|temporada|stagione|сезон|第.+季|シーズン|^s\d+$")
        .expect("valid pattern")
});

/// Heights a bare number may name a picture by.
const HEIGHTS: [&str; 11] = [
    "144", "240", "360", "480", "540", "576", "720", "1080", "1440", "2160", "4320",
];

/// Words that say how good a picture is, without numbers.
const QUALITY_WORDS: [&str; 22] = [
    "auto", "hd", "sd", "fhd", "uhd", "qhd", "full", "hq", "lq", "high", "low", "medium", "mid",
    "best", "quality", "original", "source", "fullhd", "max", "min", "standard", "default",
];
/// Chinese quality names, read as whole words.
const QUALITY_CJK: [&str; 9] = [
    "高清", "超清", "标清", "蓝光", "流畅", "原画", "极速", "清晰", "画质",
];

/// A name that only says how good the picture is: "1080p", "720P", "4K", "Full HD",
/// "1080p Full HD", "720", "auto", "高清".
pub fn is_quality(name: &str) -> bool {
    let mut lower = name.trim().to_lowercase();
    for word in QUALITY_CJK {
        lower = lower.replace(word, " q ");
    }
    let words: Vec<&str> = lower
        .split(|c: char| {
            c.is_whitespace() || matches!(c, '-' | '_' | '·' | '|' | '/' | '(' | ')' | '[' | ']')
        })
        .filter(|word| !word.is_empty())
        .collect();
    !words.is_empty()
        && words.iter().all(|word| {
            *word == "q"
                || QUALITY_WORDS.contains(word)
                || HEIGHTS.contains(word)
                || word.strip_suffix('p').is_some_and(|digits| {
                    (3..=4).contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_digit())
                })
                || word
                    .strip_suffix('k')
                    .is_some_and(|digit| matches!(digit, "2" | "4" | "8"))
        })
}

/// The episode a name says: a marker ("Episode 3", "EP 02", "第3集", "S01E04") or a name that is
/// only a number ("05"). Qualities ("1080p", "720") and numbered mirrors ("Server 2") are none.
pub fn episode_key(name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty() || is_quality(name) {
        return None;
    }
    let info = raygun_resolver::video_match::parse_episode(name);
    let number = info.episode.filter(|number| *number >= 0.0)?;
    let bare = name.len() <= 4 && name.bytes().all(|byte| byte.is_ascii_digit());
    if info.weak && !bare {
        return None;
    }
    Some(match info.season {
        Some(season) => format!("S{season}E{}", format_number(number)),
        None => format!("E{}", format_number(number)),
    })
}

/// The season and number an [`episode_key`] stands for.
fn key_parts(key: &str) -> (Option<u32>, Option<f64>) {
    let (season, number) = match key.strip_prefix('S').and_then(|rest| rest.split_once('E')) {
        Some((season, number)) => (season.parse().ok(), number),
        None => (None, key.strip_prefix('E').unwrap_or(key)),
    };
    (season, number.parse().ok())
}

/// Each name's episode when the names are mostly distinct episodes (at least three in five name
/// one, and at least two different ones), else `None`.
pub fn episodes_named(names: &[&str]) -> Option<Vec<Option<String>>> {
    if names.len() < 2 {
        return None;
    }
    let keys: Vec<Option<String>> = names.iter().map(|name| episode_key(name)).collect();
    let named = keys.iter().flatten().count();
    let distinct: HashSet<&String> = keys.iter().flatten().collect();
    (named * 5 >= names.len() * 3 && distinct.len() >= 2).then_some(keys)
}

/// One line of an `episodes` answer given as lines.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub name: String,
    /// Each unit's name (perhaps empty) and id.
    pub units: Vec<(String, String)>,
}

/// An episode, with the servers its answer gave it (a lines answer's).
#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    pub episode: MangaChapter,
    pub servers: Option<Vec<VideoServer>>,
}

/// An `episodes` answer's lines, when it gives them.
pub fn parse_lines(value: &Value) -> Option<Vec<Line>> {
    let lines = value.get("lines")?.as_array()?;
    Some(
        lines
            .iter()
            .take(MAX_LINES)
            .enumerate()
            .map(|(index, line)| Line {
                name: text(line.get("name")).unwrap_or_else(|| (index + 1).to_string()),
                units: line
                    .get("units")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|unit| {
                        Some((
                            text(unit.get("name")).unwrap_or_default(),
                            id(unit.get("id")?)?,
                        ))
                    })
                    .take(MAX_UNITS)
                    .collect(),
            })
            .collect(),
    )
}

fn episode(id: &str, name: &str, key: Option<&str>) -> MangaChapter {
    let (season, number) = key.map(key_parts).unwrap_or_default();
    MangaChapter {
        id: id.to_owned(),
        name: name.to_owned(),
        number,
        volume: season.map(|season| season.to_string()),
        scanlator: None,
        date_upload: None,
    }
}

fn server(id: &str, name: &str) -> VideoServer {
    VideoServer {
        id: id.to_owned(),
        name: name.to_owned(),
        videos: None,
    }
}

/// Episodes from lines (see the module's notes), numbered from their names when every name has
/// a number, else by their place.
pub fn from_lines(lines: Vec<Line>) -> Vec<Unit> {
    let lines: Vec<Line> = lines
        .into_iter()
        .filter(|line| !line.units.is_empty())
        .collect();
    let titles: Vec<&str> = lines.iter().map(|line| line.name.as_str()).collect();
    let no_unit_is_an_episode = lines
        .iter()
        .flat_map(|line| &line.units)
        .all(|(name, _)| episode_key(name).is_none());
    let mut units = if lines.len() > 1 && titles.iter().all(|title| SEASON.is_match(title)) {
        by_season(&lines)
    } else if let Some(keys) = episodes_named(&titles).filter(|_| no_unit_is_an_episode) {
        lines_as_episodes(&lines, &keys)
    } else if no_unit_is_an_episode
        && lines
            .iter()
            .flat_map(|line| &line.units)
            .all(|(name, _)| is_quality(name))
    {
        qualities(&lines)
    } else {
        merged(&lines)
    };
    let numbers: Option<Vec<f64>> = units
        .iter()
        .map(|unit| {
            unit.episode
                .number
                .or_else(|| episode_key(&unit.episode.name).and_then(|key| key_parts(&key).1))
        })
        .collect();
    for (index, unit) in units.iter_mut().enumerate() {
        unit.episode.number = Some(match &numbers {
            Some(numbers) => numbers[index],
            None => (index + 1) as f64,
        });
        if unit.episode.name.is_empty() {
            unit.episode.name = format!("Episode {}", index + 1);
        }
    }
    units
}

/// Lines named as seasons: each line's units are that season's episodes.
fn by_season(lines: &[Line]) -> Vec<Unit> {
    let mut ids = HashSet::new();
    let mut units = Vec::new();
    for (season, line) in lines.iter().enumerate() {
        for (name, id) in &line.units {
            if !ids.insert(id.clone()) {
                continue;
            }
            let mut found = episode(id, name, None);
            found.volume = Some((season + 1).to_string());
            units.push(Unit {
                episode: found,
                servers: Some(vec![server(id, &line.name)]),
            });
        }
    }
    units
}

/// Lines named as episodes, their units mirrors or qualities: each line (or each set of lines
/// naming the same episode) is an episode.
fn lines_as_episodes(lines: &[Line], keys: &[Option<String>]) -> Vec<Unit> {
    let mut units: Vec<Unit> = Vec::new();
    let mut at: HashMap<String, usize> = HashMap::new();
    for (line, key) in lines.iter().zip(keys) {
        let servers = line
            .units
            .iter()
            .map(|(name, id)| server(id, if name.is_empty() { &line.name } else { name }));
        let place = key.as_ref().and_then(|key| at.get(key).copied());
        match place {
            Some(place) => units[place].servers.get_or_insert_default().extend(servers),
            None => {
                if let Some(key) = key {
                    at.insert(key.clone(), units.len());
                }
                units.push(Unit {
                    episode: episode(&line.units[0].1, &line.name, key.as_deref()),
                    servers: Some(servers.collect()),
                });
            }
        }
    }
    units
}

/// Units that are only qualities: one episode, a server for each.
fn qualities(lines: &[Line]) -> Vec<Unit> {
    let several = lines.len() > 1;
    let servers: Vec<VideoServer> = lines
        .iter()
        .flat_map(|line| {
            line.units.iter().map(move |(name, id)| {
                let name = if several {
                    format!("{} · {name}", line.name)
                } else {
                    name.clone()
                };
                server(id, &name)
            })
        })
        .collect();
    let name = lines
        .first()
        .map(|line| line.name.clone())
        .filter(|name| !is_quality(name))
        .unwrap_or_default();
    vec![Unit {
        episode: episode(&servers[0].id, &name, None),
        servers: Some(servers),
    }]
}

/// Lines listing the same episodes: the first line's units are the episodes, and the same
/// episode in another line (by its number, else its name) is one more server for it.
fn merged(lines: &[Line]) -> Vec<Unit> {
    let mut units: Vec<Unit> = Vec::new();
    let mut lines_of: Vec<HashSet<usize>> = Vec::new();
    let mut at: HashMap<String, usize> = HashMap::new();
    let mut ids = HashSet::new();
    for (index, line) in lines.iter().enumerate() {
        for (name, id) in &line.units {
            let key = episode_key(name)
                .map(|key| format!("#{key}"))
                .or_else(|| (!name.is_empty()).then(|| name.clone()));
            if let Some(place) = key.as_ref().and_then(|key| at.get(key).copied())
                && !lines_of[place].contains(&index)
            {
                lines_of[place].insert(index);
                units[place]
                    .servers
                    .get_or_insert_default()
                    .push(server(id, &line.name));
                continue;
            }
            if !ids.insert(id.clone()) {
                continue;
            }
            if let Some(key) = key {
                at.entry(key).or_insert(units.len());
            }
            lines_of.push(HashSet::from([index]));
            units.push(Unit {
                episode: episode(id, name, None),
                servers: Some(vec![server(id, &line.name)]),
            });
        }
    }
    units
}

// ---- A lone unit whose servers or videos are episodes ----

/// An id with the episode it was split for: `("unit", Some("E3"))` for `unit#raygun-episode=E3`.
pub fn split_id(id: &str) -> (&str, Option<&str>) {
    match id.rsplit_once(EPISODE_MARK) {
        Some((base, key)) if !key.is_empty() => (base, Some(key)),
        _ => (id, None),
    }
}

fn marked(id: &str, key: &str) -> String {
    format!("{id}{EPISODE_MARK}{key}")
}

fn video_name(video: &VideoLink) -> &str {
    video.label.as_deref().unwrap_or_default()
}

/// The keys of videos that are distinct episodes.
fn video_keys(videos: &[VideoLink]) -> Option<Vec<Option<String>>> {
    let names: Vec<&str> = videos.iter().map(video_name).collect();
    episodes_named(&names)
}

/// The videos of one episode out of videos that are several episodes', named by what their
/// names say besides the episode (a quality), else their height.
pub fn videos_of_episode(videos: Vec<VideoLink>, key: &str) -> Vec<VideoLink> {
    let Some(keys) = video_keys(&videos) else {
        return videos;
    };
    videos
        .into_iter()
        .zip(keys)
        .filter(|(_, own)| own.as_deref() == Some(key))
        .map(|(mut video, _)| {
            let rest = raygun_resolver::video_match::parse_episode(video_name(&video)).rest;
            video.label = Some(rest)
                .filter(|rest| !rest.is_empty())
                .or_else(|| video.quality.map(|height| format!("{height}p")));
            video
        })
        .collect()
}

/// One episode's servers out of a unit's whose servers or videos are several episodes'.
pub fn servers_of_episode(servers: Vec<VideoServer>, key: &str) -> Vec<VideoServer> {
    let names: Vec<&str> = servers.iter().map(|server| server.name.as_str()).collect();
    if let Some(keys) = episodes_named(&names) {
        return servers
            .into_iter()
            .zip(keys)
            .filter(|(_, own)| own.as_deref() == Some(key))
            .map(|(server, _)| server)
            .collect();
    }
    servers
        .into_iter()
        .filter_map(|mut server| {
            server.id = marked(&server.id, key);
            if let Some(videos) = server.videos.take() {
                let several = video_keys(&videos).is_some();
                let own = videos_of_episode(videos, key);
                if several && own.is_empty() {
                    return None;
                }
                server.videos = Some(own);
            }
            Some(server)
        })
        .collect()
}

/// The episodes a lone unit holds, when its servers' names or its videos' are distinct
/// episodes. `videos` are the lone server's when it had to be resolved to tell.
pub fn split_unit(
    unit: &MangaChapter,
    servers: &[VideoServer],
    videos: Option<&[VideoLink]>,
) -> Option<Vec<MangaChapter>> {
    let names: Vec<String> = {
        let server_names: Vec<&str> = servers.iter().map(|server| server.name.as_str()).collect();
        if episodes_named(&server_names).is_some() {
            server_names.iter().map(|name| (*name).to_owned()).collect()
        } else {
            let listed = servers
                .iter()
                .filter_map(|server| server.videos.as_deref())
                .chain(videos)
                .find(|videos| video_keys(videos).is_some())?;
            listed
                .iter()
                .map(|video| video_name(video).to_owned())
                .collect()
        }
    };
    let mut seen = HashSet::new();
    let episodes: Vec<MangaChapter> = names
        .iter()
        .filter_map(|name| {
            let key = episode_key(name)?;
            seen.insert(key.clone())
                .then(|| episode(&marked(&unit.id, &key), name, Some(&key)))
        })
        .collect();
    (episodes.len() >= 2).then_some(episodes)
}

// ---- What's kept between calls ----

/// Episodes' servers from lines answers, and answers read to tell a lone unit's episodes apart
/// (each kept a short while for the one call that follows).
#[derive(Default)]
pub(crate) struct Memory {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    servers: HashMap<(String, String), Vec<VideoServer>>,
    order: VecDeque<(String, String)>,
    answers: HashMap<(String, &'static str, String), (Instant, Value)>,
}

impl Memory {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(crate) fn keep_servers(&self, provider: &str, units: &[Unit]) {
        let mut inner = self.lock();
        for unit in units {
            let Some(servers) = &unit.servers else {
                continue;
            };
            let key = (provider.to_owned(), unit.episode.id.clone());
            if inner.servers.insert(key.clone(), servers.clone()).is_none() {
                inner.order.push_back(key);
            }
        }
        while inner.order.len() > MAX_KEPT_EPISODES {
            if let Some(oldest) = inner.order.pop_front() {
                inner.servers.remove(&oldest);
            }
        }
    }

    pub(crate) fn servers(&self, provider: &str, episode: &str) -> Option<Vec<VideoServer>> {
        self.lock()
            .servers
            .get(&(provider.to_owned(), episode.to_owned()))
            .cloned()
    }

    pub(crate) fn keep_answer(
        &self,
        provider: &str,
        method: &'static str,
        id: &str,
        value: &Value,
    ) {
        let mut inner = self.lock();
        inner
            .answers
            .retain(|_, (at, _)| at.elapsed() < ANSWER_KEPT);
        if inner.answers.len() >= MAX_ANSWERS {
            inner.answers.clear();
        }
        inner.answers.insert(
            (provider.to_owned(), method, id.to_owned()),
            (Instant::now(), value.clone()),
        );
    }

    /// A kept answer, taken: the next call asks the source again.
    pub(crate) fn take_answer(
        &self,
        provider: &str,
        method: &'static str,
        id: &str,
    ) -> Option<Value> {
        self.lock()
            .answers
            .remove(&(provider.to_owned(), method, id.to_owned()))
            .filter(|(at, _)| at.elapsed() < ANSWER_KEPT)
            .map(|(_, value)| value)
    }
}

#[cfg(test)]
#[path = "video_units_tests.rs"]
mod tests;
