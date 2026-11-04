//! What played and what failed (M46.17, M46.49): sources are tried in the order they did last
//! time. A link counts as working once it has played for [`WORKED_AFTER_MS`], not when it
//! resolves. Per title (and per show, across its seasons) the link that worked is kept with the
//! links that failed; per server or addon name, how often it worked and when it last failed.
//! Both live in `local_value`, bounded, and fade over days.

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};

use raygun_model::VideoResolution;
use serde::{Deserialize, Serialize};
use specta::Type;

use super::{Group, Link};
use crate::video_quality;

/// How long a server or addon that failed stays at the end of the list (M46.17), and how long a
/// title remembers a dead link.
pub const FAILING_FOR_MS: i64 = 3 * 24 * 60 * 60 * 1000;
/// How long a link plays before it counts as working.
pub const WORKED_AFTER_MS: i64 = 10_000;
/// A source's success score halves every week without playing.
const HALF_LIFE_MS: f64 = 7.0 * 24.0 * 60.0 * 60.0 * 1000.0;
/// Sources not heard of for this long are forgotten.
const FORGET_AFTER_MS: i64 = 60 * 24 * 60 * 60 * 1000;
/// The most sources kept, the most recent first.
const MAX_SOURCES: usize = 300;
/// The most dead links a title keeps.
const MAX_DEAD: usize = 24;

/// How a server or addon did lately.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SourceStat {
    /// When it last failed, Unix ms (0: never).
    pub failed: i64,
    /// When it last played [`WORKED_AFTER_MS`], Unix ms (0: never).
    pub worked: i64,
    /// How often it worked, fading by [`HALF_LIFE_MS`] from `worked`.
    pub score: f64,
}

/// How servers and addons did lately, by name. One that failed since it last worked goes to the
/// end of the list for three days; the ones that worked most come first.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceHealth {
    #[serde(default, deserialize_with = "lenient_sources")]
    pub sources: HashMap<String, SourceStat>,
}

/// Reads the map, dropping entries of an older shape (M46.17 kept `[tried, failed]` pairs).
fn lenient_sources<'de, D>(deserializer: D) -> Result<HashMap<String, SourceStat>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = HashMap::<String, serde_json::Value>::deserialize(deserializer)?;
    Ok(raw
        .into_iter()
        .filter_map(|(name, value)| {
            let stat = match value {
                // The old pair: when it was tried and when it failed.
                serde_json::Value::Array(pair) => SourceStat {
                    failed: pair.get(1).and_then(serde_json::Value::as_i64).unwrap_or(0),
                    ..SourceStat::default()
                },
                other => serde_json::from_value(other).ok()?,
            };
            Some((name, stat))
        })
        .collect())
}

fn faded(score: f64, since: i64, now: i64) -> f64 {
    let age = (now - since).max(0) as f64;
    score * 0.5_f64.powf(age / HALF_LIFE_MS)
}

impl SourceHealth {
    /// The names that failed lately and haven't worked since.
    pub fn failing(&self, now: i64) -> HashSet<String> {
        self.sources
            .iter()
            .filter(|(_, stat)| stat.failed > stat.worked && now - stat.failed < FAILING_FOR_MS)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// How well a source worked lately: 0 for one that never did.
    pub fn score(&self, name: &str, now: i64) -> f64 {
        self.sources
            .get(name)
            .map_or(0.0, |stat| faded(stat.score, stat.worked, now))
    }

    pub fn worked(&mut self, name: &str, now: i64) {
        let stat = self.sources.entry(name.to_owned()).or_default();
        stat.score = faded(stat.score, stat.worked, now) + 1.0;
        stat.worked = now;
        self.prune(now);
    }

    pub fn failed(&mut self, name: &str, now: i64) {
        self.sources.entry(name.to_owned()).or_default().failed = now;
        self.prune(now);
    }

    /// Forgets sources not heard of for two months, and keeps the most recent few hundred.
    fn prune(&mut self, now: i64) {
        let last = |stat: &SourceStat| stat.failed.max(stat.worked);
        self.sources
            .retain(|_, stat| now - last(stat) < FORGET_AFTER_MS);
        if self.sources.len() > MAX_SOURCES {
            let mut by_age: Vec<(String, i64)> = self
                .sources
                .iter()
                .map(|(name, stat)| (name.clone(), last(stat)))
                .collect();
            by_age.sort_by_key(|(_, at)| Reverse(*at));
            for (name, _) in by_age.into_iter().skip(MAX_SOURCES) {
                self.sources.remove(&name);
            }
        }
    }
}

/// A link that failed for a title.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadLink {
    pub group: String,
    pub label: String,
    pub at: i64,
}

/// What played last for a title (or show), to try first next time, and its links that failed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Remembered {
    /// The server's (video sources) or addon's (Stremio) name; empty when nothing worked yet.
    pub group: String,
    /// Stremio's `bingeGroup` of the stream.
    pub tag: Option<String>,
    /// The link's label ("1080p", an addon's stream name).
    pub label: Option<String>,
    /// When it last played [`WORKED_AFTER_MS`], Unix ms.
    pub at: i64,
    /// Links that failed lately, newest first.
    pub dead: Vec<DeadLink>,
}

impl Remembered {
    /// The link played long enough: it's the one to start from, and no longer dead.
    pub fn worked(&mut self, group: &str, link: &Link, now: i64) {
        self.group = group.to_owned();
        self.tag = link.tag.clone();
        self.label = Some(link.label.clone());
        self.at = now;
        self.dead
            .retain(|dead| !(dead.group == group && dead.label == link.label));
    }

    /// The link failed: tried after every other one for a while.
    pub fn failed(&mut self, group: &str, link: &Link, now: i64) {
        self.dead.retain(|dead| {
            !(dead.group == group && dead.label == link.label) && now - dead.at < FAILING_FOR_MS
        });
        self.dead.insert(
            0,
            DeadLink {
                group: group.to_owned(),
                label: link.label.clone(),
                at: now,
            },
        );
        self.dead.truncate(MAX_DEAD);
    }

    fn is_dead(&self, group: &str, label: &str, now: i64) -> bool {
        self.dead.iter().any(|dead| {
            dead.group == group && dead.label == label && now - dead.at < FAILING_FOR_MS
        })
    }
}

/// Where a link or source stands for a title, best first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Standing {
    /// The very link (or, for a source, the server or addon) that played last time.
    LastPlayed,
    /// Its release (`bingeGroup`) in any addon.
    SameRelease,
    /// A source that worked lately.
    Worked,
    Untried,
    /// A source that failed since it last worked, or a link that failed for this title.
    Failing,
}

/// Orders links and sources by how they did: what played last time, the same release, what
/// worked lately, untried ones, then those that failed lately; within each of those by the
/// preferred resolution (`video_quality::rank`).
pub struct Ranking<'a> {
    memory: &'a Remembered,
    health: &'a SourceHealth,
    failing: HashSet<String>,
    resolution: VideoResolution,
    now: i64,
}

/// A sort key: lower plays first. (Names another episode, standing, not the remembered source,
/// less success, how far its size is from the preferred one.)
pub type RankKey = (bool, Standing, bool, Reverse<i64>, (u8, u32));

/// The size term of a source with no links yet: after every link of the same standing, so
/// nothing waits on a source for its sizes alone.
const UNKNOWN_SIZE: (u8, u32) = (u8::MAX, u32::MAX);

impl<'a> Ranking<'a> {
    pub fn new(
        memory: &'a Remembered,
        health: &'a SourceHealth,
        resolution: VideoResolution,
        now: i64,
    ) -> Self {
        Self {
            memory,
            health,
            failing: health.failing(now),
            resolution,
            now,
        }
    }

    pub fn is_failing(&self, group: &str) -> bool {
        self.failing.contains(group)
    }

    fn remembered(&self, group: &str) -> bool {
        !self.memory.group.is_empty() && group == self.memory.group
    }

    /// The source played long enough last time (kept before M46.49, only that it loaded).
    pub fn played_last(&self, group: &str) -> bool {
        self.remembered(group) && self.memory.at > 0
    }

    fn score(&self, group: &str) -> i64 {
        (self.health.score(group, self.now) * 1000.0) as i64
    }

    pub fn group_standing(&self, group: &str) -> Standing {
        if self.is_failing(group) {
            Standing::Failing
        } else if self.remembered(group) {
            Standing::LastPlayed
        } else if self.score(group) > 0 {
            Standing::Worked
        } else {
            Standing::Untried
        }
    }

    pub fn link_standing(&self, group: &str, label: &str, tag: Option<&str>) -> Standing {
        if self.is_failing(group) || self.memory.is_dead(group, label, self.now) {
            return Standing::Failing;
        }
        let memory = self.memory;
        let same_group = self.remembered(group);
        if same_group && memory.label.as_deref() == Some(label) {
            return Standing::LastPlayed;
        }
        if tag.is_some() && tag == memory.tag.as_deref() {
            Standing::SameRelease
        } else if same_group || self.score(group) > 0 {
            Standing::Worked
        } else {
            Standing::Untried
        }
    }

    /// The key of a source with no links (yet): what it could hold at best.
    pub fn group_key(&self, group: &str) -> RankKey {
        (
            false,
            self.group_standing(group),
            !self.remembered(group),
            Reverse(self.score(group)),
            UNKNOWN_SIZE,
        )
    }

    pub fn link_key(&self, group: &str, link: &Link) -> RankKey {
        (
            link.mismatch,
            self.link_standing(group, &link.label, link.tag.as_deref()),
            !self.remembered(group),
            Reverse(self.score(group)),
            video_quality::rank(link.height, self.resolution),
        )
    }

    /// A source's key: its best link's, or its own while it has none.
    pub fn best_key(&self, group: &Group) -> RankKey {
        group
            .links
            .iter()
            .map(|link| self.link_key(&group.name, link))
            .min()
            .unwrap_or_else(|| self.group_key(&group.name))
    }
}

/// Orders each source's links, then the sources by their best link: what played last time first,
/// those that failed lately last. Ties keep the order they came in.
pub fn order_groups(groups: &mut [Group], ranking: &Ranking) {
    for group in groups.iter_mut() {
        order_links(group, ranking);
    }
    groups.sort_by_cached_key(|group| ranking.best_key(group));
}

/// Orders one source's links by standing, then resolution; ties keep their order.
pub fn order_links(group: &mut Group, ranking: &Ranking) {
    let name = group.name.clone();
    group
        .links
        .sort_by_cached_key(|link| ranking.link_key(&name, link));
}

/// A source as a picker shows it, to order (`video_source_order`).
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceCandidate {
    /// The server's or addon's name.
    pub name: String,
    /// Its links, when known.
    pub links: Vec<LinkCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LinkCandidate {
    /// As sessions label it: a server's link label, or an addon stream's name and description
    /// joined by " · ".
    pub label: String,
    /// Stremio's `bingeGroup`.
    pub tag: Option<String>,
}

/// A source in play order, with how it did.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceRank {
    /// Its place in the list given.
    pub index: u32,
    /// It played last time for this title.
    pub worked: bool,
    /// It failed lately and hasn't worked since.
    pub failing: bool,
    /// Its link that played last time: its place in the list given.
    pub worked_link: Option<u32>,
    /// Its links' places in the list given, in play order.
    pub links: Vec<u32>,
}

/// Orders a picker's sources the way a session would try them.
pub fn rank_candidates(candidates: &[SourceCandidate], ranking: &Ranking) -> Vec<SourceRank> {
    let link = |candidate: &LinkCandidate| Link {
        label: candidate.label.clone(),
        tag: candidate.tag.clone(),
        failed: None,
        mismatch: false,
        height: video_quality::label_height(&candidate.label),
        request: raygun_model::VideoRequest {
            key: String::new(),
            title: String::new(),
            subtitle: None,
            poster: None,
            url: String::new(),
            headers: Default::default(),
            format: None,
            subtitles: Vec::new(),
            group: None,
            chapters: Vec::new(),
            anime: None,
        },
    };
    let mut ranked: Vec<(RankKey, SourceRank)> = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let mut links: Vec<(RankKey, u32)> = candidate
                .links
                .iter()
                .enumerate()
                .map(|(place, entry)| {
                    (
                        ranking.link_key(&candidate.name, &link(entry)),
                        place as u32,
                    )
                })
                .collect();
            links.sort();
            let key = links
                .first()
                .map(|(key, _)| *key)
                .unwrap_or_else(|| ranking.group_key(&candidate.name));
            let worked = ranking.played_last(&candidate.name);
            let rank = SourceRank {
                index: index as u32,
                worked,
                failing: ranking.is_failing(&candidate.name),
                worked_link: candidate
                    .links
                    .iter()
                    .position(|entry| {
                        worked && ranking.memory.label.as_deref() == Some(entry.label.as_str())
                    })
                    .map(|place| place as u32),
                links: links.into_iter().map(|(_, place)| place).collect(),
            };
            (key, rank)
        })
        .collect();
    ranked.sort_by_key(|(key, rank)| (*key, rank.index));
    ranked.into_iter().map(|(_, rank)| rank).collect()
}
