//! Playback sessions (M44): watching a title the way Aniyomi, CloudStream and Stremio do. Opening
//! an episode opens the player at once; its links resolve in the background (every server of a
//! video source, every addon for Stremio) and the best one plays as soon as it is known. A link
//! that fails hands over to the next. The player asks for the previous or next episode, another
//! episode or another source, and gets the episode and source lists to show; finishing an episode
//! moves Continue watching on to the next one.
//!
//! A link played on its own (Open link, another app: [`Origin::Link`]) plays through a session
//! too, so the player's Sources and Quality work for it: its own group holds the link (a torrent's
//! video files, the one it names first), which always plays first; the movie it was matched with
//! (`video_links`) adds the addons' streams for it after, tried only once the link failed.

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;
use raygun_db::titles;
use raygun_db::video::{self as store, VideoEntry};
use raygun_db::video_downloads;
use raygun_model::{SettingChange, VideoRequest, VideoResolution};
use raygun_resolver::video_match;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::catalog::{continue_index, now_ms, to_message};
use crate::events::WatchChangedEvent;
use crate::startup::AppDb;
use crate::stremio::{StremioPlayTarget, StremioService};
use crate::video_library;
use crate::video_quality::{self, LinkFacts, Offer, QualityKey, Variant};
use crate::video_sources::{self, VideoSourceService};

/// What a session plays from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "origin", rename_all = "camelCase")]
pub enum Origin {
    /// A stored title of a video source.
    #[serde(rename_all = "camelCase")]
    Source { title_id: String },
    /// A Stremio movie or series, saved or not.
    #[serde(rename_all = "camelCase")]
    Stremio { kind: String, id: String },
    /// A link played on its own: its progress key (`url:<address>`). Never stored as an episode
    /// reference: a link's entry keeps its request.
    #[serde(rename_all = "camelCase")]
    Link { key: String },
}

impl Origin {
    /// The title's player memory (speed, aspect, tracks; M45), as `VideoRequest::group`; a link's
    /// is its own key, as when it played without a session.
    pub fn group_key(&self) -> String {
        match self {
            Self::Source { title_id } => format!("title:{title_id}"),
            Self::Stremio { kind, id } => format!("stremio:{kind}:{id}"),
            Self::Link { key } => key.clone(),
        }
    }

    /// Stremio anime (Kitsu, MyAnimeList, AniList or AniDB ids, or the `anime` type) can be
    /// looked up on AniSkip. Each season is an entry of its own there, so the entry's own id
    /// finds its MyAnimeList id; the name is searched only without one.
    fn anime(&self) -> Option<AnimeIds> {
        let Self::Stremio { kind, id } = self else {
            return None;
        };
        let number = |prefix: &str| {
            id.strip_prefix(prefix)
                .and_then(|rest| rest.split(':').next())
                .and_then(|number| number.parse().ok())
        };
        let ids = AnimeIds {
            mal: number("mal:"),
            kitsu: number("kitsu:"),
            anilist: number("anilist:"),
        };
        let anime = kind == "anime"
            || ids != AnimeIds::default()
            || ["kitsu-", "mal-", "anidb:"]
                .iter()
                .any(|prefix| id.starts_with(prefix));
        anime.then_some(ids)
    }

    /// The title a request's `group` names (the inverse of [`Origin::group_key`]): what a video
    /// played without a session belongs to.
    pub fn from_group(group: &str) -> Option<Self> {
        if let Some(title_id) = group.strip_prefix("title:") {
            return Some(Self::Source {
                title_id: title_id.to_owned(),
            });
        }
        let (kind, id) = group.strip_prefix("stremio:")?.split_once(':')?;
        (!kind.is_empty() && !id.is_empty()).then(|| Self::Stremio {
            kind: kind.to_owned(),
            id: id.to_owned(),
        })
    }

    /// The key a title's remembered source is kept under.
    fn memory_key(&self) -> String {
        match self {
            Self::Source { title_id } => format!("video.lastSource.title:{title_id}"),
            Self::Stremio { kind, id } => format!("video.lastSource.stremio:{kind}:{id}"),
            Self::Link { key } => format!("video.lastSource.{key}"),
        }
    }
}

/// A link played on its own ([`Origin::Link`]): what it plays (its title and poster as its entry
/// shows them), and the file of its torrent asked for (its page's files list).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkPlay {
    pub request: VideoRequest,
    pub file: Option<usize>,
}

/// A link session's own group: the link itself (its torrent's videos).
pub const LINK_GROUP: &str = "link";
/// Any session's download of the episode.
const DOWNLOAD_GROUP: &str = "download";
/// Where the sources found for the movie a link was matched with go, once they answer.
const MATCH_GROUP: &str = "match";

/// A session's own group, not a source: its download, and for a link played on its own
/// (`link_session`) the link itself. A source whose id happens to be `link` stays a source in
/// other sessions.
fn own_group(link_session: bool, group: &Group) -> bool {
    group.id == DOWNLOAD_GROUP || (link_session && group.id == LINK_GROUP)
}

/// An anime entry's ids that AniSkip's lookup starts from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct AnimeIds {
    mal: Option<u32>,
    kitsu: Option<u32>,
    anilist: Option<u32>,
}

/// An episode to play through a session. Saved as the resume request of episodes played this way,
/// so resuming finds fresh links (they expire) instead of replaying an old one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeRef {
    #[serde(flatten)]
    pub origin: Origin,
    /// The episode's id at its source (Stremio: the video id).
    pub episode: String,
}

/// One episode of the session, in watching order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Episode {
    pub remote_id: String,
    /// Its progress key.
    pub key: String,
    /// "S1 E2 · Name", or the title for a movie.
    pub label: String,
    /// The stored episode (video sources).
    pub unit_id: Option<String>,
    /// Release date, Unix ms; episodes not out yet are skipped.
    pub released: Option<i64>,
    pub watched: bool,
    pub downloaded: bool,
    /// Its episode number, when it's a whole one (for AniSkip).
    pub number: Option<u32>,
    /// A special (season 0, or no season among seasons): the next episode never crosses into
    /// the specials, as in Stremio.
    pub special: bool,
    /// The numberings a stream's name may give it by (Stremio: its season and episode, its
    /// IMDb numbering, its number across seasons), to tell streams of another episode.
    pub numbers: Vec<video_match::EpisodeNumber>,
}

/// A playable link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub label: String,
    /// Stremio's `bingeGroup`: links of one release across episodes.
    pub tag: Option<String>,
    /// Why it can't play, once known.
    pub failed: Option<String>,
    /// Its name says it's another episode or season ("S01E01" for S2E1): tried after the rest.
    pub mismatch: bool,
    /// Its picture height, when its source or name says (`video_quality`).
    pub height: Option<u32>,
    /// The file's size in bytes, when its source says (Stremio's `videoSize`).
    pub size: Option<u64>,
    pub request: VideoRequest,
}

impl Link {
    /// What its names say: range, codec, sound, size, release (the Sources sheet, the Quality menu).
    pub fn facts(&self) -> LinkFacts {
        video_quality::link_facts(&[Some(&self.label)], self.size, self.height)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupState {
    Resolving,
    Ready,
    Empty,
    Failed(String),
}

/// A server (video sources) or an addon (Stremio) and its links.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub state: GroupState,
    pub links: Vec<Link>,
}

impl Group {
    pub fn resolving(id: String, name: String) -> Self {
        Self {
            id,
            name,
            state: GroupState::Resolving,
            links: Vec::new(),
        }
    }

    /// A group whose links are known: ready, empty, or failed with `error`.
    pub fn finished(id: String, name: String, links: Vec<Link>, error: Option<String>) -> Self {
        let state = match (error, links.is_empty()) {
            (Some(error), true) => GroupState::Failed(error),
            (_, true) => GroupState::Empty,
            _ => GroupState::Ready,
        };
        Self {
            id,
            name,
            state,
            links,
        }
    }
}

mod memory;

use memory::WORKED_AFTER_MS;
pub use memory::{
    Ranking, Remembered, SourceCandidate, SourceHealth, SourceRank, order_groups, order_links,
    rank_candidates,
};

const HEALTH_KEY: &str = "video.sourceHealth";
/// How long a session waits for a source that worked before to answer, before it plays
/// another one that has.
const PATIENCE_MS: i64 = 8_000;

/// How often a source's measured speed is saved while it plays.
const SPEED_SAVE_MS: i64 = 60_000;

/// The link loaded last, until it has played long enough to count as working.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Watching {
    at: (usize, usize),
    /// Where it started playing.
    from: i64,
    /// A live stream's place in its first report: live positions follow the stream's own clock,
    /// not the start asked for.
    live_from: Option<i64>,
    /// It played [`WORKED_AFTER_MS`].
    confirmed: bool,
}

struct Session {
    generation: u64,
    origin: Origin,
    title: String,
    poster: Option<String>,
    episodes: Vec<Episode>,
    index: usize,
    groups: Vec<Group>,
    /// The link playing or loading, as (group, link).
    current: Option<(usize, usize)>,
    tried: HashSet<(usize, usize)>,
    /// Where the next link starts (a fallback or a source switch keeps the position).
    start_at: Option<i64>,
    /// What played last for the title or show, and its links that failed.
    memory: Remembered,
    /// Where the show's source is remembered across its entries ("Title" and "Title II" are one
    /// show's seasons on anime sites and Kitsu).
    show_memory: Option<String>,
    /// How servers and addons did lately (M46.17): those that worked first, failing ones last.
    health: SourceHealth,
    /// The picture size links are ordered by within each standing.
    resolution: VideoResolution,
    /// Addresses that failed in this session: never tried again, whichever source lists them.
    dead_urls: HashSet<String>,
    /// The link loaded, until it counts as working.
    watching: Option<Watching>,
    /// When the current episode's links started resolving, Unix ms.
    resolving_since: i64,
    /// Resolving finished; with nothing playing, the session has run out of links.
    settled: bool,
    /// The step asked for in the Quality menu: links with it are tried first, also when the one
    /// playing fails.
    wanted: Option<QualityKey>,
    /// Links already tried for `wanted`: tapping it again plays the next one.
    quality_tried: HashSet<(usize, usize)>,
    /// The playing stream's own variants and the one playing, as the player reports them.
    variants: Vec<Variant>,
    playing_variant: Option<QualityKey>,
    /// The episode's length, as the player reports it (for links' bit rates).
    duration_ms: Option<i64>,
    /// Connection speeds measured while sources played (kb/s), by source and by address.
    speeds: HashMap<String, u64>,
    link_speeds: HashMap<String, u64>,
    /// When a measured speed was last saved, Unix ms.
    speed_saved_at: i64,
    /// A link played on its own ([`Origin::Link`]).
    link: Option<LinkPlay>,
    /// Why the link couldn't be opened (a torrent's peers, the network), said when nothing else
    /// can play; a failure in the player is said by the player itself.
    link_error: Option<String>,
    /// The player, told another link would play when its own failed (`fallback`), waits for one:
    /// when none plays after all, it's told so.
    player_waits: bool,
}

impl Session {
    fn episode(&self) -> &Episode {
        &self.episodes[self.index]
    }

    /// The link's own link to play, when this is a link played on its own: the link itself (or
    /// its download) plays before any other source. Its torrent's other videos (a sample, other
    /// cuts) aren't tried in its place: they're picked in Sources.
    fn own_candidate(&self) -> Option<(usize, usize)> {
        self.groups
            .iter()
            .enumerate()
            .filter(|(_, group)| own_group(true, group) && group.state == GroupState::Ready)
            .filter_map(|(group_index, group)| group.links.first().map(|link| (group_index, link)))
            .find(|(group_index, link)| {
                link.failed.is_none()
                    && !self.dead_urls.contains(&link.request.url)
                    && !self.tried.contains(&(*group_index, 0))
            })
            .map(|(group_index, _)| (group_index, 0))
    }

    /// Whether another link would play if the one playing failed: one untried, or a source still
    /// finding links (the player then says it tries the next one).
    fn can_fall_back(&self, now: i64) -> bool {
        self.groups
            .iter()
            .any(|group| group.state == GroupState::Resolving)
            || self.next_candidate(now).is_some()
    }

    /// A link played on its own with nothing else to switch to: one link at most, and no source
    /// still finding any.
    fn alone(&self) -> bool {
        self.link.is_some()
            && self
                .groups
                .iter()
                .all(|group| group.state != GroupState::Resolving)
            && self
                .groups
                .iter()
                .map(|group| group.links.len())
                .sum::<usize>()
                <= 1
    }

    /// The next episode that is out, after `index`; from a regular episode, never a special.
    fn next_after(&self, index: usize, now: i64) -> Option<usize> {
        next_episode(&self.episodes, index, now)
    }

    fn previous_before(&self, index: usize, now: i64) -> Option<usize> {
        (0..index)
            .rev()
            .find(|&previous| released(&self.episodes[previous], now))
    }

    fn ranking(&self, now: i64) -> Ranking<'_> {
        Ranking::new(&self.memory, &self.health, self.resolution, now)
    }

    /// The best untried link: what played last time, the same release, sources that worked,
    /// untried ones, then those that failed lately; links whose names say they're another episode
    /// after every other one, and ties in list order. The tier asked for in the Quality menu
    /// comes before all of those, then the tiers nearest it (fewest tiers away; unknown sizes
    /// last). While a source that could hold a better one is still resolving, nothing (for a few
    /// seconds: [`PATIENCE_MS`]). A link played on its own plays itself first, whatever the
    /// others are (its torrent's other videos are picked by hand); the others once it failed.
    fn next_candidate(&self, now: i64) -> Option<(usize, usize)> {
        if self.link.is_some() {
            if let Some(own) = self.own_candidate() {
                return Some(own);
            }
            if self
                .groups
                .iter()
                .any(|group| own_group(true, group) && group.state == GroupState::Resolving)
            {
                return None;
            }
        }
        let ranking = self.ranking(now);
        let unwanted = |link: &Link| {
            self.wanted.map(|wanted| {
                let facts = link.facts();
                facts.height.map_or(usize::MAX, |height| {
                    wanted.distance(QualityKey::of(height, facts.range)).0
                })
            })
        };
        let best = self
            .groups
            .iter()
            .enumerate()
            .filter(|(_, group)| group.state == GroupState::Ready)
            // A link's own files past the first are picked by hand ([`Self::own_candidate`]).
            .filter(|(_, group)| self.link.is_none() || !own_group(true, group))
            .flat_map(|(group_index, group)| {
                group
                    .links
                    .iter()
                    .enumerate()
                    .map(move |(link_index, link)| (group_index, link_index, group, link))
            })
            .filter(|(group_index, link_index, _, link)| {
                link.failed.is_none()
                    && !self.dead_urls.contains(&link.request.url)
                    && !self.tried.contains(&(*group_index, *link_index))
            })
            .min_by_key(|(group_index, link_index, group, link)| {
                (
                    unwanted(link),
                    ranking.link_key(&group.name, link),
                    *group_index,
                    *link_index,
                )
            })
            .map(|(group_index, link_index, group, link)| {
                (group_index, link_index, ranking.link_key(&group.name, link))
            })?;
        let patient = now - self.resolving_since < PATIENCE_MS;
        let better_pending = self.groups.iter().any(|group| {
            group.state == GroupState::Resolving && ranking.group_key(&group.name) < best.2
        });
        (!(patient && better_pending)).then_some((best.0, best.1))
    }

    /// The links the Quality menu can play, with their steps and their place in the order the
    /// session tries them: not failed, not another episode's, with a known height.
    fn offers(&self, now: i64) -> Vec<Offer> {
        let ranking = self.ranking(now);
        let mut found: Vec<(memory::RankKey, usize, usize, QualityKey, LinkFacts)> = Vec::new();
        for (group_index, group) in self.groups.iter().enumerate() {
            if group.state != GroupState::Ready {
                continue;
            }
            for (link_index, link) in group.links.iter().enumerate() {
                let usable = link.failed.is_none()
                    && !link.mismatch
                    && !self.dead_urls.contains(&link.request.url);
                let facts = link.facts();
                let Some(height) = facts.height.filter(|_| usable) else {
                    continue;
                };
                found.push((
                    ranking.link_key(&group.name, link),
                    group_index,
                    link_index,
                    QualityKey::of(height, facts.range),
                    facts,
                ));
            }
        }
        found.sort_by_key(|(key, group, link, _, _)| (*key, *group, *link));
        found
            .into_iter()
            .enumerate()
            .map(|(rank, (_, group, link, key, facts))| Offer {
                group,
                link,
                key,
                codec: facts.codec,
                picture: facts.picture,
                rank,
            })
            .collect()
    }

    /// The Quality menu as the player shows it (`qualities`), with `notice` when a step tapped
    /// can't play.
    fn quality_update(&self, now: i64, notice: Option<String>) -> Value {
        let offers = self.offers(now);
        let names: Vec<String> = self.groups.iter().map(|group| group.name.clone()).collect();
        let rows = video_quality::rows(&video_quality::Menu {
            offers: &offers,
            names: &names,
            current: self.current,
            variants: &self.variants,
            playing: self.playing_variant,
        });
        json!({
            "type": "qualities",
            "items": rows,
            "wanted": self.wanted.map(|key| key.text()),
            "notice": notice,
        })
    }

    /// The measured connection speed of a source: this session's, else what was kept.
    fn speed_of(&self, name: &str) -> Option<u64> {
        self.speeds
            .get(name)
            .copied()
            .or_else(|| self.health.speed(name))
    }
}

/// What the player reports with progress (live streams, lengths, connection speed).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Heard {
    live: bool,
    duration_ms: Option<i64>,
    bandwidth_kbps: Option<u64>,
}

impl Heard {
    fn of(report: &Value) -> Self {
        Self {
            live: report.get("live").and_then(Value::as_bool) == Some(true),
            duration_ms: report
                .get("durationMs")
                .and_then(Value::as_f64)
                .filter(|ms| *ms > 0.0)
                .map(|ms| ms as i64),
            bandwidth_kbps: report
                .get("bandwidthKbps")
                .and_then(Value::as_f64)
                .filter(|kbps| *kbps > 0.0)
                .map(|kbps| kbps as u64),
        }
    }
}

/// The Quality menu of a video played without a session: its own variants alone.
fn variants_update(variants: &[Variant], playing: Option<QualityKey>) -> Value {
    let rows = video_quality::rows(&video_quality::Menu {
        offers: &[],
        names: &[],
        current: None,
        variants,
        playing,
    });
    json!({ "type": "qualities", "items": rows, "wanted": Value::Null, "notice": Value::Null })
}

/// "1080p", "2160p HDR": a step's name in a notice.
fn quality_name(key: QualityKey) -> String {
    match key.range {
        video_quality::Range::Sdr => format!("{}p", key.height),
        video_quality::Range::Hdr => format!("{}p HDR", key.height),
        video_quality::Range::Dv => format!("{}p Dolby Vision", key.height),
    }
}

/// The player's `sources` update: every source with its links as listed (Raw details), what
/// each link's names say and how fast its source was (the clean list), and the order the clean
/// list shows the sources in ([`source_order`]).
fn sources_update(session: &Session, now: i64) -> Value {
    let ranking = session.ranking(now);
    // "Playing from <source>" once the current link has played a while.
    let playing_from = session
        .watching
        .filter(|watching| watching.confirmed && session.current == Some(watching.at))
        .and_then(|watching| session.groups.get(watching.at.0))
        .map(|group| group.name.clone());
    let groups: Vec<Value> = session
        .groups
        .iter()
        .enumerate()
        .map(|(group_index, group)| {
            let (state, error) = match &group.state {
                GroupState::Resolving => ("resolving", None),
                GroupState::Ready => ("ready", None),
                GroupState::Empty => ("empty", None),
                // Not asked: the source waits for its key or sign-in. The sheet counts these in
                // one line instead of listing them as failed.
                GroupState::Failed(error) if raygun_ext_host::needs_setup_in(error).is_some() => {
                    ("setup", Some(error.clone()))
                }
                GroupState::Failed(error) => ("failed", Some(error.clone())),
            };
            let facts: Vec<LinkFacts> = group.links.iter().map(Link::facts).collect();
            let usable = |link: &Link| {
                link.failed.is_none() && !session.dead_urls.contains(&link.request.url)
            };
            let best = group
                .links
                .iter()
                .zip(&facts)
                .filter(|(link, _)| usable(link))
                .filter_map(|(_, facts)| facts.height)
                .max();
            let links: Vec<Value> = group
                .links
                .iter()
                .zip(&facts)
                .enumerate()
                .map(|(link_index, (link, facts))| {
                    json!({
                        "label": link.label,
                        "current": session.current == Some((group_index, link_index)),
                        "failed": !usable(link),
                        "worked": ranking.played_last(&group.name)
                            && session.memory.label.as_deref() == Some(link.label.as_str()),
                        "facts": facts,
                        "bitrateKbps": video_quality::bitrate_kbps(facts.size_bytes, session.duration_ms),
                        "speedKbps": session.link_speeds.get(&link.request.url),
                        // A torrent, with the seeds its source names when it does.
                        "torrent": raygun_torrent::is_torrent(&link.request.url, link.request.format.as_deref())
                            .then(|| json!({ "seeds": raygun_torrent::seeds_in(&link.label) })),
                    })
                })
                .collect();
            json!({
                "name": group.name,
                "state": state,
                "error": error,
                // A link played on its own: shown whatever its state (why it failed included).
                "own": session.link.is_some() && own_group(true, group),
                "unreliable": ranking.is_failing(&group.name),
                "worked": ranking.played_last(&group.name),
                "best": best,
                "options": group.links.iter().filter(|link| usable(link)).count(),
                "speedKbps": session.speed_of(&group.name),
                "links": links,
            })
        })
        .collect();
    json!({
        "type": "sources",
        "playingFrom": playing_from,
        "groups": groups,
        "order": source_order(session, &ranking),
        // A link played on its own: its own links come first, and with nothing else to switch
        // to the sheet says so ("Only this link") instead of the player hiding it. Without
        // another link to fall back on, a failure in the player is said by the player.
        "link": session.link.is_some(),
        "alone": session.alone(),
        "fallback": session.can_fall_back(now),
    })
}

/// The order the Sources sheet's clean list shows sources in: those with links that can play
/// first, then those still finding links, then empty and failed ones; within each, by how they
/// did (the session's [`Ranking`]: what played last time first, failing ones last), then by
/// the best picture they have, then by their measured speed.
fn source_order(session: &Session, ranking: &Ranking) -> Vec<usize> {
    let mut order: Vec<usize> = (0..session.groups.len()).collect();
    order.sort_by_cached_key(|&index| {
        let group = &session.groups[index];
        let playable: Vec<&Link> = group
            .links
            .iter()
            .filter(|link| link.failed.is_none() && !session.dead_urls.contains(&link.request.url))
            .collect();
        let state = match &group.state {
            GroupState::Ready if !playable.is_empty() => 0,
            GroupState::Resolving => 1,
            GroupState::Ready | GroupState::Empty => 2,
            GroupState::Failed(_) => 3,
        };
        let best = playable
            .iter()
            .filter_map(|link| link.facts().height)
            .max()
            .unwrap_or(0);
        let speed = session.speed_of(&group.name).unwrap_or(0);
        (
            // A link played on its own lists itself first.
            !(session.link.is_some() && own_group(true, group)),
            state,
            ranking.best_key(group).1,
            Reverse(best),
            Reverse(speed),
            index,
        )
    });
    order
}

/// The variants a `variants` request reports, and the one playing.
fn reported_variants(report: &Value) -> (Vec<Variant>, Option<QualityKey>) {
    let variants: Vec<Variant> = report
        .get("items")
        .cloned()
        .and_then(|items| serde_json::from_value(items).ok())
        .unwrap_or_default();
    let playing = report
        .get("playing")
        .cloned()
        .and_then(|playing| serde_json::from_value::<Variant>(playing).ok())
        .and_then(|variant| variant.key());
    (variants, playing)
}

/// The next episode that is out, after `index`; from a regular episode, never a special.
fn next_episode(episodes: &[Episode], index: usize, now: i64) -> Option<usize> {
    let special = episodes.get(index).is_some_and(|episode| episode.special);
    (index + 1..episodes.len()).find(|&next| {
        let episode = &episodes[next];
        released(episode, now) && (special || !episode.special)
    })
}

/// The player's loading line: the title, and the episode's label when it adds something.
fn title_and_label(title: &str, label: &str) -> String {
    if video_match::adds_to_title(title, label) {
        format!("{title}\n{label}")
    } else {
        title.to_owned()
    }
}

/// A link's label without the title it may repeat ("Show Name 1080p" → "1080p").
pub fn link_label(title: &str, label: String) -> String {
    let short = video_match::without_title(&label, title);
    if short.is_empty() { label } else { short }
}

fn released(episode: &Episode, now: i64) -> bool {
    episode.released.is_none_or(|at| at <= now)
}

/// Whether a Stremio title's episode labels show the season. Kitsu lists each season as an
/// entry of its own whose episodes are all "season 1" while its name says which season it is
/// ("Attack on Titan Season 2"); there the season is left out rather than shown wrong.
pub fn shows_seasons(meta: &raygun_model::StremioMeta) -> bool {
    let all_first = meta
        .videos
        .iter()
        .all(|video| video.season.is_none_or(|season| season == 1));
    !(all_first && video_match::parse_title(&meta.name).names_season())
}

/// Where a show's source is remembered across its entries: the scope (a source, or Stremio)
/// and the title without its season ("Overlord" for "Overlord II").
fn show_memory_key(scope: &str, title: &str) -> Option<String> {
    let key = video_match::parse_title(title).key();
    (!key.is_empty()).then(|| format!("video.lastSource.show:{scope}:{key}"))
}

/// Marks the links whose names say they're another episode than `numbers` give, and puts them
/// last in their group.
pub fn mark_mismatches(groups: &mut [Group], numbers: &[video_match::EpisodeNumber]) {
    if numbers.is_empty() {
        return;
    }
    for group in groups {
        for link in &mut group.links {
            link.mismatch = video_match::names_other_episode(&link.label, numbers);
        }
        group.links.sort_by_key(|link| link.mismatch);
    }
}

/// Playback sessions; one at a time.
pub struct VideoSessions {
    app: AppHandle,
    session: Mutex<Option<Session>>,
    generations: AtomicU64,
    /// The player's words in the app's language, set by the UI.
    labels: Mutex<serde_json::Map<String, Value>>,
}

impl VideoSessions {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            session: Mutex::new(None),
            generations: AtomicU64::new(0),
            labels: Mutex::new(serde_json::Map::new()),
        }
    }

    pub fn set_labels(&self, labels: serde_json::Map<String, Value>) {
        *self.labels.lock().expect("labels poisoned") = labels;
    }

    pub fn labels(&self) -> Value {
        Value::Object(self.labels.lock().expect("labels poisoned").clone())
    }

    /// A word of the player's in the app's language (`videoPlayer.*`), or `fallback`.
    pub(crate) fn label(&self, key: &str, fallback: &str) -> String {
        self.labels
            .lock()
            .expect("labels poisoned")
            .get(key)
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .unwrap_or(fallback)
            .to_owned()
    }

    /// Opens the player on an episode (or where watching continues: the episode watched last, or
    /// the next one once it was finished) and starts finding its links.
    pub async fn start(&self, origin: Origin, episode: Option<String>) -> Result<(), String> {
        self.start_at(origin, episode, None).await
    }

    /// Opens the player on the episode after `key` (Continue watching's Next episode): the next
    /// one that is out, never from a regular episode into the specials. False when there's none.
    pub async fn start_next(&self, origin: Origin, key: &str) -> Result<bool, String> {
        let (_, _, episodes, _) = self.episodes(&origin).await?;
        let Some(index) = episodes.iter().position(|episode| episode.key == key) else {
            return Err("This episode is no longer listed".into());
        };
        let Some(next) = next_episode(&episodes, index, now_ms()) else {
            return Ok(false);
        };
        let episode = episodes[next].remote_id.clone();
        self.start(origin, Some(episode)).await?;
        Ok(true)
    }

    /// [`Self::start`], the first link playing from `start_at` (ms) instead of the episode's saved
    /// place when given (Play from the start).
    pub async fn start_at(
        &self,
        origin: Origin,
        episode: Option<String>,
        start_at: Option<i64>,
    ) -> Result<(), String> {
        let (title, poster, episodes, scope) = self.episodes(&origin).await?;
        if episodes.is_empty() {
            return Err("This title has no episodes".into());
        }
        let now = now_ms();
        let chosen = episode.and_then(|id| episodes.iter().position(|item| item.remote_id == id));
        let index = match chosen {
            Some(index) => index,
            None => {
                let watched_at = self.watched_at(&origin, &episodes).await;
                continue_episode(&episodes, &watched_at, now)
            }
        };
        let show_memory = show_memory_key(&scope, &title);
        let memory = match self.recall(&origin.memory_key()).await {
            Some(memory) => Some(memory),
            None => match &show_memory {
                Some(key) => self.recall(key).await,
                None => None,
            },
        }
        .unwrap_or_default();
        let health = self.health().await;
        let resolution = self.resolution().await;
        let generation = self.generations.fetch_add(1, Ordering::SeqCst) + 1;
        let label = episodes[index].label.clone();
        *self.session.lock().expect("session poisoned") = Some(Session {
            generation,
            origin,
            title: title.clone(),
            poster,
            episodes,
            index,
            groups: Vec::new(),
            current: None,
            tried: HashSet::new(),
            start_at,
            memory,
            show_memory,
            health,
            resolution,
            dead_urls: HashSet::new(),
            watching: None,
            resolving_since: now,
            settled: false,
            wanted: None,
            quality_tried: HashSet::new(),
            variants: Vec::new(),
            playing_variant: None,
            duration_ms: None,
            speeds: HashMap::new(),
            link_speeds: HashMap::new(),
            speed_saved_at: 0,
            link: None,
            link_error: None,
            player_waits: false,
        });
        let label = if video_match::adds_to_title(&title, &label) {
            label
        } else {
            String::new()
        };
        let shell = crate::video::session_shell(&self.app, &title, &label).await;
        crate::video::open_player(&self.app, &shell)?;
        self.push_episodes().await;
        self.spawn_resolve(generation);
        Ok(())
    }

    /// Opens the player on a link played on its own ([`Origin::Link`]): the link (a torrent's
    /// file the page picked, else the one it names) plays from `start_at` or where it was left,
    /// and the Sources sheet lists its torrent's other videos and the sources found for the
    /// movie it was matched with. A torrent that can't play now (turned off, mobile data) says
    /// why without opening the player, unless it's downloaded.
    pub async fn start_link(&self, link: LinkPlay, start_at: Option<i64>) -> Result<(), String> {
        let request = &link.request;
        let torrent = raygun_torrent::is_torrent(&request.url, request.format.as_deref());
        if torrent
            && crate::video_downloads::offline(&self.app, &request.key)
                .await
                .is_none()
            && let Some(reason) = crate::torrents::unavailable_now(&self.app).await
        {
            return Err(reason);
        }
        let origin = Origin::Link {
            key: request.key.clone(),
        };
        let memory = self.recall(&origin.memory_key()).await.unwrap_or_default();
        let health = self.health().await;
        let resolution = self.resolution().await;
        let generation = self.generations.fetch_add(1, Ordering::SeqCst) + 1;
        let title = request.title.clone();
        let subtitle = request.subtitle.clone().unwrap_or_default();
        let episode = Episode {
            remote_id: request.key.clone(),
            key: request.key.clone(),
            label: title.clone(),
            unit_id: None,
            released: None,
            watched: false,
            downloaded: false,
            number: None,
            special: false,
            numbers: Vec::new(),
        };
        *self.session.lock().expect("session poisoned") = Some(Session {
            generation,
            origin,
            title: title.clone(),
            poster: request.poster.clone(),
            episodes: vec![episode],
            index: 0,
            groups: Vec::new(),
            current: None,
            tried: HashSet::new(),
            start_at,
            memory,
            show_memory: None,
            health,
            resolution,
            dead_urls: HashSet::new(),
            watching: None,
            resolving_since: now_ms(),
            settled: false,
            wanted: None,
            quality_tried: HashSet::new(),
            variants: Vec::new(),
            playing_variant: None,
            duration_ms: None,
            speeds: HashMap::new(),
            link_speeds: HashMap::new(),
            speed_saved_at: 0,
            link: Some(link.clone()),
            link_error: None,
            player_waits: false,
        });
        let mut shell = crate::video::session_shell(&self.app, &title, &subtitle).await;
        // A torrent's details come from peers first.
        if torrent {
            shell["loadingText"] = json!(self.label("torrentFindingPeers", "Finding peers…"));
        }
        crate::video::open_player(&self.app, &shell)?;
        self.push_episodes().await;
        self.spawn_resolve(generation);
        Ok(())
    }

    /// Ends the session (the player closed, or something else plays).
    pub fn end(&self) {
        self.generations.fetch_add(1, Ordering::SeqCst);
        *self.session.lock().expect("session poisoned") = None;
    }

    /// A report from the player that belongs to the session.
    pub async fn on_report(&self, report: &Value) {
        let state = report
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let key = report.get("id").and_then(Value::as_str).unwrap_or_default();
        let position = report
            .get("positionMs")
            .and_then(Value::as_f64)
            .unwrap_or(0.0) as i64;
        match state {
            "request" => self.on_request(report, position).await,
            "error" => {
                let message = report
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("The video couldn't be played")
                    .to_owned();
                self.fall_back(key, position, message, true).await;
            }
            "closed" => {
                let ends = self
                    .session
                    .lock()
                    .expect("session poisoned")
                    .as_ref()
                    .is_some_and(|session| key.is_empty() || session.episode().key == key);
                if ends {
                    self.end();
                }
            }
            "progress" | "ended" => {
                self.on_progress(key, position, state == "ended", Heard::of(report))
                    .await
            }
            _ => {}
        }
    }

    /// What a progress report says beside the position: the episode's length, and the connection
    /// speed while the current link plays (kept for its source, saved now and then).
    async fn note_heard(&self, key: &str, heard: Heard) {
        let save = {
            let mut guard = self.session.lock().expect("session poisoned");
            let Some(session) = guard.as_mut() else {
                return;
            };
            if session.episode().key != key {
                return;
            }
            if !heard.live && heard.duration_ms.is_some() {
                session.duration_ms = heard.duration_ms;
            }
            let (Some(kbps), Some((group, link))) = (heard.bandwidth_kbps, session.current) else {
                return;
            };
            // A download's speed says nothing of a source.
            let Some(entry) = session
                .groups
                .get(group)
                .filter(|entry| entry.id != DOWNLOAD_GROUP)
            else {
                return;
            };
            let name = entry.name.clone();
            // A link played on its own shows its own speed; it isn't a source whose speed is kept.
            let own = own_group(session.link.is_some(), entry);
            if let Some(url) = entry.links.get(link).map(|link| link.request.url.clone()) {
                session.link_speeds.insert(url, kbps);
            }
            if own {
                return;
            }
            session.speeds.insert(name.clone(), kbps);
            let now = now_ms();
            let due = now - session.speed_saved_at >= SPEED_SAVE_MS;
            if due {
                session.speed_saved_at = now;
                session.health.measured(&name, kbps, now);
            }
            due.then_some((name, kbps, now))
        };
        if let Some((name, kbps, now)) = save {
            self.note_health(|health| health.measured(&name, kbps, now))
                .await;
        }
    }

    /// The link loaded last has played [`WORKED_AFTER_MS`] (or to its end): it's the one this
    /// title starts from next time, and its source counts as working.
    async fn on_progress(&self, key: &str, position: i64, ended: bool, heard: Heard) {
        self.note_heard(key, heard).await;
        let worked = {
            let mut guard = self.session.lock().expect("session poisoned");
            let Some(session) = guard.as_mut() else {
                return;
            };
            if session.episode().key != key {
                return;
            }
            let Some(watching) = session.watching.as_mut() else {
                return;
            };
            // A live stream's positions are its own clock: it counts from its first report.
            let from = if heard.live {
                *watching.live_from.get_or_insert(position)
            } else {
                watching.from
            };
            let played = position - from >= WORKED_AFTER_MS;
            if watching.confirmed || session.current != Some(watching.at) || !(played || ended) {
                return;
            }
            watching.confirmed = true;
            let (group, link) = watching.at;
            let link_session = session.link.is_some();
            let Some(entry) = session
                .groups
                .get(group)
                .filter(|entry| !own_group(link_session, entry))
            else {
                return;
            };
            let Some(chosen) = entry.links.get(link) else {
                return;
            };
            let now = now_ms();
            let name = entry.name.clone();
            let chosen = chosen.clone();
            session.memory.worked(&name, &chosen, now);
            session.health.worked(&name, now);
            let memory: Vec<String> = [
                Some(session.origin.memory_key()),
                session.show_memory.clone(),
            ]
            .into_iter()
            .flatten()
            .collect();
            (session.memory.clone(), name, now, memory)
        };
        let (memory, name, now, keys) = worked;
        for key in &keys {
            self.remember(key, &memory).await;
        }
        self.note_health(|health| health.worked(&name, now)).await;
        self.push_sources().await;
    }

    async fn on_request(&self, report: &Value, position: i64) {
        let action = report
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let now = now_ms();
        match action {
            // The stream's own variants: the Quality menu, with or without a session.
            "variants" => {
                let (variants, playing) = reported_variants(report);
                let update = {
                    let mut guard = self.session.lock().expect("session poisoned");
                    match guard.as_mut() {
                        Some(session) => {
                            session.variants = variants;
                            session.playing_variant = playing;
                            session.quality_update(now, None)
                        }
                        None => variants_update(&variants, playing),
                    }
                };
                let _ = crate::video::update_player(&self.app, &update);
                return;
            }
            "quality" => {
                match report
                    .get("key")
                    .and_then(Value::as_str)
                    .and_then(QualityKey::parse)
                {
                    Some(key) => self.choose_quality(key, position).await,
                    // Automatic: the session picks by itself again.
                    None => self.forget_quality().await,
                }
                return;
            }
            _ => {}
        }
        let target = {
            let guard = self.session.lock().expect("session poisoned");
            let Some(session) = guard.as_ref() else {
                return;
            };
            match action {
                "next" => session.next_after(session.index, now).map(Target::Episode),
                "previous" => session
                    .previous_before(session.index, now)
                    .map(Target::Episode),
                "episode" => report
                    .get("index")
                    .and_then(Value::as_u64)
                    .map(|index| index as usize)
                    .filter(|&index| index < session.episodes.len())
                    .map(Target::Episode),
                "source" => {
                    let group = report.get("group").and_then(Value::as_u64);
                    let link = report.get("link").and_then(Value::as_u64);
                    group
                        .zip(link)
                        .map(|(group, link)| Target::Link(group as usize, link as usize))
                }
                "retry" => Some(Target::Episode(session.index)),
                "ready" => Some(Target::State),
                "autoplay" => Some(Target::Autoplay(
                    report.get("on").and_then(Value::as_bool).unwrap_or(true),
                )),
                _ => None,
            }
        };
        match target {
            Some(Target::Episode(index)) => self.change_episode(index).await,
            Some(Target::State) => {
                self.push_episodes().await;
                self.push_sources().await;
                // A link that couldn't be opened before the player was there says so again.
                let link_generation = self
                    .session
                    .lock()
                    .expect("session poisoned")
                    .as_ref()
                    .filter(|session| session.link.is_some())
                    .map(|session| session.generation);
                if let Some(generation) = link_generation {
                    self.try_play(generation).await;
                }
            }
            Some(Target::Link(group, link)) => self.choose_link(group, link, position).await,
            Some(Target::Autoplay(on)) => {
                let db = self.db();
                if let Err(error) =
                    crate::settings::update(&db, SettingChange::AutoplayNext(on)).await
                {
                    tracing::warn!(%error, "could not save the autoplay setting");
                }
            }
            None => {}
        }
    }

    /// Plays another episode in the open player.
    async fn change_episode(&self, index: usize) {
        let generation = self.generations.fetch_add(1, Ordering::SeqCst) + 1;
        let label = {
            let mut guard = self.session.lock().expect("session poisoned");
            let Some(session) = guard.as_mut() else {
                return;
            };
            session.generation = generation;
            session.index = index;
            session.groups.clear();
            session.current = None;
            session.tried.clear();
            session.start_at = None;
            session.watching = None;
            session.resolving_since = now_ms();
            session.settled = false;
            session.quality_tried.clear();
            session.variants.clear();
            session.playing_variant = None;
            session.duration_ms = None;
            session.link_speeds.clear();
            title_and_label(&session.title, &session.episode().label)
        };
        let _ = crate::video::update_player(
            &self.app,
            &json!({ "type": "loading", "text": format!("{label}\n{}", self.label("findingSource", "Finding a source…")) }),
        );
        self.push_episodes().await;
        self.spawn_resolve(generation);
    }

    /// Switches to a link chosen in the player, keeping the position.
    async fn choose_link(&self, group: usize, link: usize, position: i64) {
        let generation = {
            let mut guard = self.session.lock().expect("session poisoned");
            let Some(session) = guard.as_mut() else {
                return;
            };
            let exists = session
                .groups
                .get(group)
                .is_some_and(|entry| link < entry.links.len());
            if !exists {
                return;
            }
            session.tried.insert((group, link));
            session.current = Some((group, link));
            session.watching = None;
            session.start_at = (position > 0).then_some(position);
            // A link picked by hand: no step of the Quality menu is asked for anymore.
            session.wanted = None;
            session.quality_tried.clear();
            session.generation
        };
        self.play(generation, group, link).await;
    }

    /// A step tapped in the Quality menu: the source playing's link with it, else another
    /// source's (the first a session would try), from where the video is. Tapped again while one
    /// plays, the next source with it; links with the step stay first when one fails.
    async fn choose_quality(&self, key: QualityKey, position: i64) {
        let picked = {
            let mut guard = self.session.lock().expect("session poisoned");
            let Some(session) = guard.as_mut() else {
                return;
            };
            let now = now_ms();
            let offers = session.offers(now);
            let playing_key = session.current.and_then(|current| {
                offers
                    .iter()
                    .find(|offer| (offer.group, offer.link) == current)
                    .map(|offer| offer.key)
            });
            let again = session.wanted == Some(key) && playing_key == Some(key);
            if !again {
                session.quality_tried.clear();
            }
            if let Some(current) = session.current.filter(|_| again) {
                session.quality_tried.insert(current);
            }
            session.wanted = Some(key);
            match video_quality::pick(&offers, key, session.current, &session.quality_tried) {
                Some((group, link)) => {
                    session.quality_tried.insert((group, link));
                    session.tried.insert((group, link));
                    session.current = Some((group, link));
                    session.watching = None;
                    session.start_at = (position > 0).then_some(position);
                    Ok((session.generation, group, link))
                }
                None => Err(session.quality_update(
                    now,
                    Some(
                        self.label("qualityNoOther", "No other source has {n}")
                            .replace("{n}", &quality_name(key)),
                    ),
                )),
            }
        };
        match picked {
            Ok((generation, group, link)) => self.play(generation, group, link).await,
            Err(update) => {
                let _ = crate::video::update_player(&self.app, &update);
            }
        }
    }

    /// Automatic quality: no step is asked for anymore.
    async fn forget_quality(&self) {
        let update = {
            let mut guard = self.session.lock().expect("session poisoned");
            let Some(session) = guard.as_mut() else {
                return;
            };
            session.wanted = None;
            session.quality_tried.clear();
            session.quality_update(now_ms(), None)
        };
        let _ = crate::video::update_player(&self.app, &update);
    }

    /// The playing link failed: the next best one plays from the same spot at once. Its address
    /// isn't tried again in this session; unless it had played a while (a dropped connection),
    /// the title keeps it as dead and its source as failing. `in_player`: the player failed it
    /// (and says why itself), else it couldn't be opened.
    async fn fall_back(&self, key: &str, position: i64, error: String, in_player: bool) {
        let mut failed = None;
        let generation = {
            let mut guard = self.session.lock().expect("session poisoned");
            let Some(session) = guard.as_mut() else {
                return;
            };
            if session.episode().key != key {
                return;
            }
            if session.link.is_some() {
                session.link_error = (!in_player).then(|| error.clone());
                // As the player decided: with another link to come it waits for it, else it
                // says what went wrong itself.
                session.player_waits = in_player && session.can_fall_back(now_ms());
            }
            let watching = session.watching.take();
            if let Some((group, link)) = session.current.take() {
                let played = watching
                    .is_some_and(|watching| watching.at == (group, link) && watching.confirmed);
                let now = now_ms();
                if let Some(entry) = session
                    .groups
                    .get_mut(group)
                    .and_then(|entry| entry.links.get_mut(link))
                {
                    entry.failed = Some(error);
                    session.dead_urls.insert(entry.request.url.clone());
                }
                if let Some(entry) = session.groups.get(group)
                    && let Some(chosen) = entry.links.get(link)
                    && !played
                    && !own_group(session.link.is_some(), entry)
                {
                    let name = entry.name.clone();
                    let chosen = chosen.clone();
                    session.memory.failed(&name, &chosen, now);
                    session.health.failed(&name, now);
                    let keys: Vec<String> = [
                        Some(session.origin.memory_key()),
                        session.show_memory.clone(),
                    ]
                    .into_iter()
                    .flatten()
                    .collect();
                    failed = Some((name, now, session.memory.clone(), keys));
                }
            }
            if position > 0 {
                session.start_at = Some(position);
            }
            session.generation
        };
        if let Some((name, now, memory, keys)) = failed {
            self.note_health(|health| health.failed(&name, now)).await;
            for key in &keys {
                self.remember(key, &memory).await;
            }
        }
        self.push_sources().await;
        self.try_play(generation).await;
    }

    /// A picker's sources in the order a session tries them for the title `group` names
    /// (`Origin::group_key`), with what played last time marked. `title` is the show's name, for
    /// what its other seasons played; a stored title's own name is used when it has one.
    pub async fn source_order(
        &self,
        group: &str,
        title: Option<String>,
        sources: Vec<SourceCandidate>,
    ) -> Vec<SourceRank> {
        let health = self.health().await;
        let resolution = self.resolution().await;
        let Some(origin) = Origin::from_group(group) else {
            return rank_candidates(
                &sources,
                &Ranking::new(&Remembered::default(), &health, resolution, now_ms()),
            );
        };
        let show = match &origin {
            Origin::Source { title_id } => titles::title(&self.db(), title_id)
                .await
                .ok()
                .flatten()
                .map(|row| (format!("source:{}", row.fields.source_id), row.fields.title)),
            Origin::Stremio { .. } => title.map(|title| ("stremio".to_owned(), title)),
            Origin::Link { .. } => None,
        };
        let show_title = show.as_ref().map(|(_, title)| title.clone());
        let memory = match self.recall(&origin.memory_key()).await {
            Some(memory) => Some(memory),
            None => match show.and_then(|(scope, title)| show_memory_key(&scope, &title)) {
                Some(key) => self.recall(&key).await,
                None => None,
            },
        }
        .unwrap_or_default();
        // Sessions keep links by their labels without the title.
        let sources: Vec<SourceCandidate> = match show_title {
            Some(show_title) => sources
                .into_iter()
                .map(|mut source| {
                    for link in &mut source.links {
                        link.label = link_label(&show_title, std::mem::take(&mut link.label));
                    }
                    source
                })
                .collect(),
            None => sources,
        };
        rank_candidates(
            &sources,
            &Ranking::new(&memory, &health, resolution, now_ms()),
        )
    }

    /// A title's episodes in watching order, with its name and poster (downloads queued without a
    /// session use them).
    pub async fn title_episodes(
        &self,
        origin: &Origin,
    ) -> Result<(String, Option<String>, Vec<Episode>), String> {
        let (title, poster, episodes, _) = self.episodes(origin).await?;
        Ok((title, poster, episodes))
    }

    /// What a session would order a title's links by: its remembered source, how sources did
    /// lately, and the preferred picture size.
    pub async fn ranking_inputs(
        &self,
        origin: &Origin,
    ) -> (Remembered, SourceHealth, VideoResolution) {
        let memory = self.recall(&origin.memory_key()).await.unwrap_or_default();
        (memory, self.health().await, self.resolution().await)
    }

    /// The picture size links are ordered by (`VideoPlayerPrefs::preferred_resolution`).
    async fn resolution(&self) -> VideoResolution {
        crate::settings::load_in_effect(&self.db())
            .await
            .map(|settings| settings.video_player.preferred_resolution)
            .unwrap_or_default()
    }

    async fn health(&self) -> SourceHealth {
        raygun_db::local_value(&self.db(), HEALTH_KEY)
            .await
            .ok()
            .flatten()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    async fn note_health(&self, change: impl FnOnce(&mut SourceHealth)) {
        // Offline Mode says nothing about how sources do: none was asked.
        if raygun_net::network_mode::offline() {
            return;
        }
        let mut health = self.health().await;
        change(&mut health);
        if let Ok(text) = serde_json::to_string(&health)
            && let Err(error) = raygun_db::set_local_value(&self.db(), HEALTH_KEY, &text).await
        {
            tracing::warn!(%error, "could not keep how sources did");
        }
    }

    fn spawn_resolve(&self, generation: u64) {
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let sessions = app.state::<VideoSessions>();
            // A source that worked before gets a few seconds to answer before another plays;
            // then the session takes what it has.
            let patience = app.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(PATIENCE_MS as u64)).await;
                patience.state::<VideoSessions>().try_play(generation).await;
            });
            sessions.resolve(generation).await;
            let finished = {
                let mut guard = sessions.session.lock().expect("session poisoned");
                match guard.as_mut() {
                    Some(session) if session.generation == generation => {
                        session.settled = true;
                        true
                    }
                    _ => false,
                }
            };
            if finished {
                sessions.try_play(generation).await;
            }
        });
    }

    /// Finds the current episode's links: a download first, then the source's servers or the
    /// addons' streams.
    async fn resolve(&self, generation: u64) {
        let Some((origin, episode, title, poster)) = self.snapshot(generation) else {
            return;
        };
        let db = self.db();
        if let Ok(Some(row)) = video_downloads::done_for_key(&db, &episode.key).await {
            let download = row.download;
            let request = VideoRequest {
                key: download.key,
                title: download.title,
                subtitle: download.subtitle,
                poster: download.poster,
                url: download.url,
                headers: serde_json::from_str(&download.headers).unwrap_or_default(),
                format: download.format,
                subtitles: serde_json::from_str(&download.subtitles).unwrap_or_default(),
                group: None,
                chapters: Vec::new(),
                anime: None,
                live: false,
            };
            let link = Link {
                label: self.label("onDevice", "On this device"),
                tag: None,
                failed: None,
                mismatch: false,
                height: None,
                size: None,
                request,
            };
            let group = Group::finished(
                DOWNLOAD_GROUP.into(),
                self.label("downloaded", "Downloaded"),
                vec![link],
                None,
            );
            self.set_groups(generation, vec![group]).await;
            return;
        }
        match origin {
            Origin::Source { title_id } => {
                self.resolve_source(generation, &title_id, &episode).await
            }
            Origin::Link { .. } => self.resolve_link(generation).await,
            Origin::Stremio { kind, id } => {
                let target = StremioPlayTarget {
                    kind,
                    meta_id: Some(id),
                    video_id: episode.remote_id.clone(),
                    subtitle: video_match::adds_to_title(&title, &episode.label)
                        .then(|| episode.label.clone()),
                    title,
                    poster,
                };
                let language = crate::video::subtitle_language(&db).await;
                let waiting = Group::resolving("addons".into(), self.label("addons", "Addons"));
                self.set_groups(generation, vec![waiting]).await;
                let stremio = self.app.state::<std::sync::Arc<StremioService>>();
                let mut groups = match stremio.playable(&target, &language).await {
                    Ok(groups) => groups,
                    Err(error) => vec![Group::finished(
                        "addons".into(),
                        self.label("addons", "Addons"),
                        Vec::new(),
                        Some(error),
                    )],
                };
                // Addons that search by name can answer with another season's files.
                mark_mismatches(&mut groups, &episode.numbers);
                // A season's torrent without a file index opens the episode's file.
                for link in groups.iter_mut().flat_map(|group| group.links.iter_mut()) {
                    link.request.url = raygun_torrent::with_episodes(
                        &link.request.url,
                        link.request.format.as_deref(),
                        &episode.numbers,
                    );
                }
                self.set_groups(generation, groups).await;
            }
        }
    }

    async fn resolve_source(&self, generation: u64, title_id: &str, episode: &Episode) {
        let sources = self.app.state::<VideoSourceService>();
        let db = self.db();
        let stored = async {
            let row = titles::title(&db, title_id)
                .await
                .map_err(to_message)?
                .ok_or_else(|| "This title is no longer stored".to_owned())?;
            let units = titles::units(&db, title_id).await.map_err(to_message)?;
            Ok::<_, String>((row, units))
        };
        let (row, units) = match stored.await {
            Ok(found) => found,
            Err(error) => {
                let failed =
                    Group::finished("source".into(), String::new(), Vec::new(), Some(error));
                self.set_groups(generation, vec![failed]).await;
                return;
            }
        };
        let single = units.len() == 1;
        let Some(unit) = units
            .iter()
            .find(|unit| unit.fields.remote_id == episode.remote_id)
        else {
            return;
        };
        // Torrents play while they may (turned on, not on mobile data when only for Wi-Fi).
        let torrents_unavailable = crate::torrents::unavailable_now(&self.app).await;
        let source_id = row.fields.source_id.clone();
        let servers = match sources.servers(&source_id, &episode.remote_id).await {
            Ok(servers) => servers,
            Err(error) => {
                let failed = Group::finished("source".into(), source_id, Vec::new(), Some(error));
                self.set_groups(generation, vec![failed]).await;
                return;
            }
        };
        let to_links = |links: Vec<raygun_model::VideoLinkView>| -> Vec<Link> {
            links
                .into_iter()
                .map(|link| {
                    let torrent = link.format.as_deref() == Some(raygun_ext_host::video::TORRENT);
                    let mut request = video_sources::link_request(&row, unit, single, link.clone());
                    if torrent {
                        // A season's torrent opens the episode's file.
                        request.url = raygun_torrent::with_episodes(
                            &request.url,
                            request.format.as_deref(),
                            &episode.numbers,
                        );
                    }
                    Link {
                        label: link_label(
                            &row.fields.title,
                            link.label.clone().unwrap_or_else(|| {
                                link.quality.map_or_else(
                                    || self.label("video", "Video"),
                                    |height| format!("{height}p"),
                                )
                            }),
                        ),
                        tag: None,
                        mismatch: false,
                        height: link.quality,
                        size: None,
                        failed: torrents_unavailable.clone().filter(|_| torrent),
                        request,
                    }
                })
                .collect()
        };
        let mut pending = Vec::new();
        let groups: Vec<Group> = servers
            .into_iter()
            .map(|server| match server.links {
                Some(links) => Group::finished(server.id, server.name, to_links(links), None),
                None => {
                    pending.push(server.id.clone());
                    Group::resolving(server.id, server.name)
                }
            })
            .collect();
        self.set_groups(generation, groups).await;
        let mut lookups: FuturesUnordered<_> = pending
            .into_iter()
            .map(|server| {
                let sources = &sources;
                let source_id = &source_id;
                async move {
                    let result = sources.links(source_id, &server).await;
                    (server, result)
                }
            })
            .collect();
        while let Some((server, result)) = lookups.next().await {
            if !self.is_current(generation) {
                return;
            }
            let links = match result {
                Ok(links) => Ok(to_links(links)),
                Err(error) => Err(error),
            };
            self.update_group(generation, &server, links).await;
        }
    }

    /// A link played on its own: its own links (a torrent's videos, asked of peers when they
    /// aren't known yet), then, when it was matched with a movie, the sources found for it.
    async fn resolve_link(&self, generation: u64) {
        let link = {
            let guard = self.session.lock().expect("session poisoned");
            match guard
                .as_ref()
                .filter(|session| session.generation == generation)
                .and_then(|session| session.link.clone())
            {
                Some(link) => link,
                None => return,
            }
        };
        let name = crate::video_links::group_name(&link.request);
        // A torrent's videos may take a while (its details come from peers).
        self.set_groups(
            generation,
            vec![Group::resolving(LINK_GROUP.into(), name.clone())],
        )
        .await;
        let own = match crate::video_links::own_links(&self.app, &link, &self.labels()).await {
            Ok((name, links)) => Group::finished(LINK_GROUP.into(), name, links, None),
            Err(error) => {
                if let Some(session) = self
                    .session
                    .lock()
                    .expect("session poisoned")
                    .as_mut()
                    .filter(|session| session.generation == generation)
                {
                    session.link_error = Some(error.clone());
                }
                Group::finished(LINK_GROUP.into(), name, Vec::new(), Some(error))
            }
        };
        let matched = crate::video_links::matched_movie(&self.app, &link.request.key).await;
        let mut groups = vec![own];
        if matched.is_some() {
            groups.push(Group::resolving(
                MATCH_GROUP.into(),
                self.label("otherSources", "Other sources"),
            ));
        }
        self.set_groups(generation, groups).await;
        let Some(matched) = matched else {
            return;
        };
        let found = self.matched_groups(&matched, &link.request).await;
        self.replace_group(generation, MATCH_GROUP, found).await;
    }

    /// The addons' streams for the movie a link was matched with, as the link's own: they play
    /// under its key, title and poster, so its progress stays its own.
    async fn matched_groups(
        &self,
        matched: &raygun_model::VideoLinkMatch,
        base: &VideoRequest,
    ) -> Vec<Group> {
        let target = StremioPlayTarget {
            kind: matched.kind.clone(),
            meta_id: Some(matched.id.clone()),
            video_id: matched.id.clone(),
            subtitle: None,
            title: base.title.clone(),
            poster: base.poster.clone(),
        };
        let language = crate::video::subtitle_language(&self.db()).await;
        let stremio = self.app.state::<std::sync::Arc<StremioService>>();
        let mut groups = match stremio.playable(&target, &language).await {
            Ok(groups) => groups,
            Err(error) => {
                tracing::info!(%error, "no other sources for a link's movie");
                Vec::new()
            }
        };
        for link in groups.iter_mut().flat_map(|group| group.links.iter_mut()) {
            link.request.key = base.key.clone();
            link.request.group = None;
            link.request.title = base.title.clone();
            link.request.subtitle = base.subtitle.clone();
            link.request.poster = base.poster.clone();
        }
        groups
    }

    /// Puts `groups`, ordered as a session orders them, after the ones there (whose indices are
    /// in use) in place of the placeholder `id`; without the placeholder, nothing changes.
    async fn replace_group(&self, generation: u64, id: &str, mut groups: Vec<Group>) {
        {
            let mut guard = self.session.lock().expect("session poisoned");
            let Some(session) = guard
                .as_mut()
                .filter(|session| session.generation == generation)
            else {
                return;
            };
            let Some(at) = session.groups.iter().position(|group| group.id == id) else {
                return;
            };
            order_groups(&mut groups, &session.ranking(now_ms()));
            if at + 1 == session.groups.len() {
                session.groups.pop();
            } else {
                // Not the last (sessions put it last): it stays, empty, so no index moves.
                session.groups[at].state = GroupState::Empty;
            }
            session.groups.extend(groups);
        }
        self.push_sources().await;
        self.try_play(generation).await;
    }

    /// Replaces the groups (ordered by the remembered source), then plays if nothing does yet. A
    /// link played on its own keeps its own groups first, as they are.
    async fn set_groups(&self, generation: u64, mut groups: Vec<Group>) {
        {
            let mut guard = self.session.lock().expect("session poisoned");
            let Some(session) = guard
                .as_mut()
                .filter(|session| session.generation == generation)
            else {
                return;
            };
            let ranking = session.ranking(now_ms());
            if session.link.is_some() {
                groups.sort_by_key(|group| !own_group(true, group));
                let own = groups.iter().filter(|group| own_group(true, group)).count();
                order_groups(&mut groups[own..], &ranking);
            } else {
                order_groups(&mut groups, &ranking);
            }
            session.groups = groups;
            session.tried.clear();
            session.current = None;
        }
        self.push_sources().await;
        self.try_play(generation).await;
    }

    async fn update_group(&self, generation: u64, id: &str, links: Result<Vec<Link>, String>) {
        {
            let mut guard = self.session.lock().expect("session poisoned");
            let Some(session) = guard
                .as_mut()
                .filter(|session| session.generation == generation)
            else {
                return;
            };
            let link_session = session.link.is_some();
            if let Some(group) = session.groups.iter_mut().find(|group| group.id == id) {
                let (links, error) = match links {
                    Ok(links) => (links, None),
                    Err(error) => (Vec::new(), Some(error)),
                };
                *group = Group::finished(group.id.clone(), group.name.clone(), links, error);
                // Its place in the list stays (indices are in use); its links are ordered (a
                // link played on its own keeps its own order).
                let ranking = Ranking::new(
                    &session.memory,
                    &session.health,
                    session.resolution,
                    now_ms(),
                );
                if !own_group(link_session, group) {
                    order_links(group, &ranking);
                }
            }
        }
        self.push_sources().await;
        self.try_play(generation).await;
    }

    /// Plays the best untried link when nothing plays; says so when every link has failed.
    async fn try_play(&self, generation: u64) {
        enum Next {
            Play(usize, usize),
            Exhausted,
            /// A link played on its own couldn't be opened: why, and whether other sources were
            /// tried too.
            LinkFailed(String, bool),
            Wait,
        }
        let next = {
            let mut guard = self.session.lock().expect("session poisoned");
            let Some(session) = guard
                .as_mut()
                .filter(|session| session.generation == generation)
            else {
                return;
            };
            if session.current.is_some() {
                return;
            }
            match session.next_candidate(now_ms()) {
                Some(candidate) => {
                    session.tried.insert(candidate);
                    session.current = Some(candidate);
                    Next::Play(candidate.0, candidate.1)
                }
                None => {
                    let resolving = session
                        .groups
                        .iter()
                        .any(|group| group.state == GroupState::Resolving);
                    if !session.settled || resolving {
                        Next::Wait
                    } else if session.link.is_some() {
                        // The player says why the link failed in it, with its own buttons; what
                        // kept it from opening (peers, the network) is said here, and so is a
                        // player waiting for another source that none had.
                        match (session.link_error.clone(), session.player_waits) {
                            (Some(error), _) => Next::LinkFailed(error, !session.alone()),
                            (None, true) => Next::Exhausted,
                            (None, false) => Next::Wait,
                        }
                    } else {
                        Next::Exhausted
                    }
                }
            }
        };
        match next {
            Next::Play(group, link) => self.play(generation, group, link).await,
            Next::Exhausted => {
                let _ = crate::video::update_player(
                    &self.app,
                    &json!({
                        "type": "error",
                        "text": self.label("noSourceWorked", "No source could play this"),
                        "showSources": true,
                    }),
                );
            }
            Next::LinkFailed(text, others) => {
                let _ = crate::video::update_player(
                    &self.app,
                    &json!({ "type": "error", "text": text, "showSources": others }),
                );
            }
            Next::Wait => {}
        }
    }

    /// Loads a link into the player; it's remembered for the title once it plays a while.
    async fn play(&self, generation: u64, group: usize, link: usize) {
        let prepared = {
            let guard = self.session.lock().expect("session poisoned");
            let Some(session) = guard
                .as_ref()
                .filter(|session| session.generation == generation)
            else {
                return;
            };
            let Some(entry) = session.groups.get(group) else {
                return;
            };
            let Some(chosen) = entry.links.get(link) else {
                return;
            };
            // A link's entry keeps a request to play again: the file of its torrent chosen, or
            // the link itself after another source; anything else an episode reference.
            let resume = match &session.link {
                Some(_) if own_group(true, entry) => serde_json::to_string(&chosen.request),
                Some(own) => serde_json::to_string(&own.request),
                None => serde_json::to_string(&EpisodeRef {
                    origin: session.origin.clone(),
                    episode: session.episode().remote_id.clone(),
                }),
            };
            let mut request = chosen.request.clone();
            request
                .group
                .get_or_insert_with(|| session.origin.group_key());
            if request.anime.is_none()
                && let Some(ids) = session.origin.anime()
                && let Some(episode) = session.episode().number
            {
                request.anime = Some(raygun_model::AnimeEpisode {
                    title: session.title.clone(),
                    episode,
                    mal_id: ids.mal,
                    kitsu_id: ids.kitsu,
                    anilist_id: ids.anilist,
                });
            }
            (request, resume, session.start_at)
        };
        let (request, resume, start_at) = prepared;
        // A live YouTube stream plays its playlist here when a YouTube source can ask for it.
        let request = match crate::video::youtube_live_request(&self.app, &request).await {
            Some(live) => live,
            None => request,
        };
        if !self.is_current(generation) {
            return;
        }
        // A YouTube link plays in YouTube's embed: the player that opened for it closes.
        if let Some(video_id) = request.url.strip_prefix(crate::video::YOUTUBE_LINK) {
            let _ = crate::video::update_player(&self.app, &json!({ "type": "close" }));
            self.end();
            if let Err(error) = crate::video::play_youtube(
                &self.app,
                video_id,
                &request.title,
                request.subtitle.clone(),
            )
            .await
            {
                tracing::warn!(%error, "couldn't open the YouTube player");
            }
            return;
        }
        // A page plays in its site's own player, in Raygun's browser: the player closes.
        if crate::video::is_page(&request) {
            let _ = crate::video::update_player(&self.app, &json!({ "type": "close" }));
            self.end();
            if let Err(error) = crate::video::play_page(&self.app, &request).await {
                tracing::warn!(%error, "couldn't open the video's page");
            }
            return;
        }
        let resume = resume.ok();
        match crate::video::prepare(&self.app, &request, resume, start_at).await {
            Ok(payload) => {
                if !self.is_current(generation) {
                    return;
                }
                // The new stream reports its own variants once it loads.
                let link_session = {
                    let mut guard = self.session.lock().expect("session poisoned");
                    guard.as_mut().is_some_and(|session| {
                        session.variants.clear();
                        session.playing_variant = None;
                        session.link.is_some()
                    })
                };
                // A link played on its own: before it loads, the player knows whether another
                // link would stand in for it (`fallback`), should it fail at once.
                if link_session {
                    self.push_sources().await;
                }
                let _ = crate::video::update_player(
                    &self.app,
                    &json!({ "type": "load", "request": payload }),
                );
                // It counts as working once it has played a while (`on_progress`).
                let from = payload
                    .get("startMs")
                    .and_then(Value::as_i64)
                    .unwrap_or_else(|| start_at.unwrap_or(0));
                if let Some(session) = self
                    .session
                    .lock()
                    .expect("session poisoned")
                    .as_mut()
                    .filter(|session| session.current == Some((group, link)))
                {
                    session.watching = Some(Watching {
                        at: (group, link),
                        from,
                        live_from: None,
                        confirmed: false,
                    });
                }
                self.push_episodes().await;
            }
            Err(error) => {
                tracing::warn!(%error, "a link couldn't be prepared");
                let key = request.key.clone();
                Box::pin(self.fall_back(&key, start_at.unwrap_or(0), error, false)).await;
            }
        }
        self.push_sources().await;
    }

    /// An episode was watched: Continue watching moves on to the next one that is out, as Stremio
    /// does (its position is 1 ms, so it shows as up next). A video played without a session (a
    /// source picked by hand) moves on too, once, when `newly` finished: its title's episodes are
    /// looked up in the background.
    pub async fn on_watched(&self, key: &str, newly: bool) {
        let in_session = {
            let guard = self.session.lock().expect("session poisoned");
            guard.as_ref().and_then(|session| {
                let index = session.episodes.iter().position(|item| item.key == key)?;
                Some(session.next_after(index, now_ms()).map(|next| {
                    let images = self.app.try_state::<raygun_net::ImageLinks>();
                    next_entry(session, next, images.as_deref())
                }))
            })
        };
        match in_session {
            Some(Some((entry, already_watched))) => {
                self.queue_next(entry, already_watched).await;
            }
            Some(None) => {}
            None if newly => {
                let app = self.app.clone();
                let key = key.to_owned();
                tauri::async_runtime::spawn(async move {
                    app.state::<VideoSessions>().queue_next_outside(&key).await;
                });
            }
            None => {}
        }
    }

    /// Moves Continue watching on from `key`, played without a session, to the next episode of the
    /// title its request names.
    async fn queue_next_outside(&self, key: &str) {
        let db = self.db();
        let Some(saved) = store::progress(&db, key).await.ok().flatten() else {
            return;
        };
        let Some(origin) = serde_json::from_str::<VideoRequest>(&saved.entry.request)
            .ok()
            .and_then(|request| request.group)
            .and_then(|group| Origin::from_group(&group))
        else {
            return;
        };
        let (title, poster, episodes, _scope) = match self.episodes(&origin).await {
            Ok(found) => found,
            Err(error) => {
                tracing::info!(%error, "couldn't look up the next episode");
                return;
            }
        };
        let now = now_ms();
        let next = episodes
            .iter()
            .position(|item| item.key == key)
            .and_then(|index| {
                (index + 1..episodes.len()).find(|&next| released(&episodes[next], now))
            });
        let Some(next) = next else {
            return;
        };
        let episode = &episodes[next];
        let entry = VideoEntry {
            key: episode.key.clone(),
            subtitle: video_match::adds_to_title(&title, &episode.label)
                .then(|| episode.label.clone()),
            title,
            poster,
            request: serde_json::to_string(&EpisodeRef {
                origin,
                episode: episode.remote_id.clone(),
            })
            .unwrap_or_else(|_| "{}".into()),
        };
        if self.queue_next(entry, episode.watched).await {
            let _ = WatchChangedEvent(episode.key.clone()).emit(&self.app);
        }
    }

    /// Puts `entry` in Continue watching as up next, unless it was watched or started already;
    /// says whether it did.
    async fn queue_next(&self, entry: VideoEntry, already_watched: bool) -> bool {
        // Incognito (M47): a series doesn't move on to its next episode.
        if crate::privacy::incognito(&self.app) {
            return false;
        }
        let db = self.db();
        let saved = store::progress(&db, &entry.key).await.ok().flatten();
        let untouched = saved
            .as_ref()
            .is_none_or(|row| row.position_ms <= 0 && row.watched_at.is_none());
        if already_watched || !untouched {
            return false;
        }
        let now = now_ms();
        if let Err(error) = store::started(&db, &entry, now).await {
            tracing::warn!(%error, "could not queue the next episode");
            return false;
        }
        store::set_progress(&db, &entry.key, 1, None, false, now)
            .await
            .is_ok()
    }

    async fn push_episodes(&self) {
        let autoplay = crate::settings::load(&self.db())
            .await
            .map(|settings| settings.autoplay_next)
            .unwrap_or(true);
        let update = {
            let guard = self.session.lock().expect("session poisoned");
            let Some(session) = guard.as_ref() else {
                return;
            };
            let now = now_ms();
            json!({
                "type": "episodes",
                "items": session.episodes.iter().enumerate().map(|(index, episode)| json!({
                    "label": episode.label,
                    "watched": episode.watched,
                    "downloaded": episode.downloaded,
                    "current": index == session.index,
                })).collect::<Vec<_>>(),
                "hasNext": session.next_after(session.index, now).is_some(),
                "hasPrevious": session.previous_before(session.index, now).is_some(),
                "autoplay": autoplay,
            })
        };
        let _ = crate::video::update_player(&self.app, &update);
    }

    /// The sources and their links for the player's Sources sheet (raw labels for Raw details,
    /// what their names say for the clean list, in the order the list shows them), then the
    /// Quality menu that follows from them.
    async fn push_sources(&self) {
        let (update, qualities) = {
            let guard = self.session.lock().expect("session poisoned");
            let Some(session) = guard.as_ref() else {
                return;
            };
            let now = now_ms();
            (
                sources_update(session, now),
                session.quality_update(now, None),
            )
        };
        let _ = crate::video::update_player(&self.app, &update);
        let _ = crate::video::update_player(&self.app, &qualities);
    }

    fn snapshot(&self, generation: u64) -> Option<(Origin, Episode, String, Option<String>)> {
        let guard = self.session.lock().expect("session poisoned");
        let session = guard
            .as_ref()
            .filter(|session| session.generation == generation)?;
        Some((
            session.origin.clone(),
            session.episode().clone(),
            session.title.clone(),
            session.poster.clone(),
        ))
    }

    fn is_current(&self, generation: u64) -> bool {
        self.session
            .lock()
            .expect("session poisoned")
            .as_ref()
            .is_some_and(|session| session.generation == generation)
    }

    async fn recall(&self, key: &str) -> Option<Remembered> {
        raygun_db::local_value(&self.db(), key)
            .await
            .ok()
            .flatten()
            .and_then(|text| serde_json::from_str(&text).ok())
    }

    async fn remember(&self, key: &str, remembered: &Remembered) {
        // Incognito (M47): nothing watched is remembered, its source neither.
        if crate::privacy::incognito(&self.app) {
            return;
        }
        let Ok(text) = serde_json::to_string(remembered) else {
            return;
        };
        if let Err(error) = raygun_db::set_local_value(&self.db(), key, &text).await {
            tracing::warn!(%error, "could not remember the source");
        }
    }

    /// A title's episodes in watching order, with its name, poster, and where its show's
    /// remembered source is kept (the source, or Stremio).
    async fn episodes(
        &self,
        origin: &Origin,
    ) -> Result<(String, Option<String>, Vec<Episode>, String), String> {
        let db = self.db();
        match origin {
            Origin::Source { title_id } => {
                let row = titles::title(&db, title_id)
                    .await
                    .map_err(to_message)?
                    .ok_or_else(|| "This title is no longer stored".to_owned())?;
                let units = titles::units(&db, title_id).await.map_err(to_message)?;
                let single = units.len() == 1;
                let keys: Vec<String> = units
                    .iter()
                    .map(|unit| video_library::unit_key(&row, &unit.fields.remote_id))
                    .collect();
                let downloaded = video_library::downloaded_keys(&db, &keys).await?;
                let progress = store::progress_with_prefix(
                    &db,
                    &video_sources::content_key(&row.fields.source_id, ""),
                )
                .await
                .map_err(to_message)?;
                let watched: HashMap<&str, bool> = progress
                    .iter()
                    .map(|entry| (entry.entry.key.as_str(), entry.watched_at.is_some()))
                    .collect();
                // The order the title page lists them in: by season, then number.
                let episodes = video_sources::watching_order(&units)
                    .into_iter()
                    .map(|unit| {
                        let key = video_library::unit_key(&row, &unit.fields.remote_id);
                        Episode {
                            label: video_sources::episode_label(&row, unit, single),
                            watched: unit.completed_at.is_some()
                                || watched.get(key.as_str()).copied().unwrap_or(false),
                            downloaded: downloaded.contains(&key),
                            remote_id: unit.fields.remote_id.clone(),
                            unit_id: Some(unit.id.clone()),
                            released: unit.fields.date,
                            number: unit
                                .fields
                                .number
                                .filter(|number| *number >= 1.0 && number.fract() == 0.0)
                                .map(|number| number as u32),
                            special: video_sources::is_special(unit, &units),
                            numbers: Vec::new(),
                            key,
                        }
                    })
                    .collect();
                let scope = format!("source:{}", row.fields.source_id);
                Ok((row.fields.title, row.fields.cover, episodes, scope))
            }
            Origin::Stremio { kind, id } => {
                let stremio = self.app.state::<std::sync::Arc<StremioService>>();
                let meta = stremio.meta(kind, id).await?;
                let episodes = if meta.videos.is_empty() {
                    // A movie or channel plays by its own video id (TMDB's details may give IMDb's).
                    vec![Episode {
                        remote_id: meta.video_id.clone(),
                        key: crate::stremio::content_key(kind, &meta.video_id),
                        label: meta.name.clone(),
                        unit_id: None,
                        released: None,
                        watched: meta.watched,
                        downloaded: meta.downloaded,
                        number: None,
                        special: false,
                        numbers: Vec::new(),
                    }]
                } else {
                    let seasons = meta
                        .videos
                        .iter()
                        .any(|video| video.season.is_some_and(|season| season >= 1));
                    let show_seasons = shows_seasons(&meta);
                    meta.videos
                        .iter()
                        .map(|video| Episode {
                            remote_id: video.id.clone(),
                            key: crate::stremio::content_key(kind, &video.id),
                            label: Some(video_match::episode_label_in(
                                &meta.name,
                                video.season.filter(|_| show_seasons).map(f64::from),
                                video.episode.map(f64::from),
                                &video.title,
                            ))
                            .filter(|label| !label.is_empty())
                            .unwrap_or_else(|| video.title.clone()),
                            unit_id: None,
                            released: video
                                .released
                                .as_deref()
                                .and_then(crate::stremio::release_ms),
                            watched: video.watched,
                            downloaded: video.downloaded,
                            number: video.episode.filter(|number| *number >= 1),
                            special: video_match::is_special(video.season.map(f64::from), seasons),
                            numbers: stremio.episode_numbers(kind, &video.id),
                        })
                        .collect()
                };
                Ok((meta.name, meta.poster, episodes, "stremio".into()))
            }
            // A link has no title of episodes: it plays through `start_link`.
            Origin::Link { .. } => Err("A link has no episodes".into()),
        }
    }

    /// When each episode with progress was last watched (or marked in the library), by key.
    async fn watched_at(&self, origin: &Origin, episodes: &[Episode]) -> HashMap<String, i64> {
        let db = self.db();
        let keys: Vec<String> = episodes.iter().map(|episode| episode.key.clone()).collect();
        let mut watched_at: HashMap<String, i64> = store::progress_for_keys(&db, &keys)
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|row| row.position_ms > 0 || row.watched_at.is_some())
            .map(|row| (row.entry.key, row.updated_at))
            .collect();
        // Marks made in the library are on the stored episodes only.
        let stored = match origin {
            Origin::Source { title_id } => Some(title_id.clone()),
            Origin::Stremio { kind, id } => {
                titles::find_title(&db, video_library::STREMIO_SOURCE, &format!("{kind}:{id}"))
                    .await
                    .ok()
                    .flatten()
                    .map(|row| row.id)
            }
            Origin::Link { .. } => None,
        };
        if let Some(title_id) = stored {
            let units = titles::units(&db, &title_id).await.unwrap_or_default();
            let units: HashMap<&str, &titles::UnitRow> = units
                .iter()
                .map(|unit| (unit.fields.remote_id.as_str(), unit))
                .collect();
            for episode in episodes {
                let marked = units
                    .get(episode.remote_id.as_str())
                    .filter(|unit| unit.completed_at.is_some() || unit.position.unwrap_or(0) > 0)
                    .and_then(|unit| unit.progress_at);
                if let Some(at) = marked {
                    let entry = watched_at.entry(episode.key.clone()).or_insert(at);
                    *entry = (*entry).max(at);
                }
            }
        }
        watched_at
    }

    fn db(&self) -> std::sync::Arc<raygun_db::Db> {
        self.app.state::<AppDb>().0.clone()
    }
}

/// The episode `next` of the session as Continue watching's up next, and whether it was watched.
/// Its poster is the source's address: a proxy link wouldn't outlive the app.
fn next_entry(
    session: &Session,
    next: usize,
    images: Option<&raygun_net::ImageLinks>,
) -> (VideoEntry, bool) {
    let episode = &session.episodes[next];
    let poster = session
        .poster
        .as_deref()
        .map(|poster| images.map_or_else(|| poster.to_owned(), |images| images.original(poster)));
    (
        VideoEntry {
            key: episode.key.clone(),
            title: session.title.clone(),
            subtitle: video_match::adds_to_title(&session.title, &episode.label)
                .then(|| episode.label.clone()),
            poster,
            request: serde_json::to_string(&EpisodeRef {
                origin: session.origin.clone(),
                episode: episode.remote_id.clone(),
            })
            .unwrap_or_else(|_| "{}".into()),
        },
        episode.watched,
    )
}

/// Where watching a title continues when no episode is asked for (M46.48): the episode watched
/// last (`watched_at`, by key) while it's unfinished, else the next one that is out, else the
/// first one out after the last one watched, a gap, the first; specials only when the title has
/// nothing else. See [`continue_index`].
pub fn continue_episode(
    episodes: &[Episode],
    watched_at: &HashMap<String, i64>,
    now: i64,
) -> usize {
    continue_index(
        episodes,
        |episode| watched_at.get(&episode.key).copied(),
        |episode| episode.watched,
        |episode| released(episode, now),
        |episode| episode.special,
    )
    .unwrap_or(0)
}

/// What a player request asks for.
enum Target {
    /// The player opened: it wants the episodes and sources.
    State,
    Episode(usize),
    Link(usize, usize),
    Autoplay(bool),
}

#[cfg(test)]
mod tests {
    use super::memory::{LinkCandidate, Standing};
    use super::*;

    fn link(label: &str, tag: Option<&str>) -> Link {
        Link {
            label: label.into(),
            tag: tag.map(str::to_owned),
            failed: None,
            mismatch: false,
            height: crate::video_quality::label_height(label),
            size: None,
            request: VideoRequest {
                key: "k".into(),
                title: "T".into(),
                subtitle: None,
                poster: None,
                url: format!("https://cdn.example/{label}"),
                headers: Default::default(),
                format: None,
                subtitles: Vec::new(),
                group: None,
                chapters: Vec::new(),
                anime: None,
                live: false,
            },
        }
    }

    fn episode(id: &str, released: Option<i64>) -> Episode {
        Episode {
            remote_id: id.into(),
            key: format!("stremio:series:{id}"),
            label: id.into(),
            unit_id: None,
            released,
            watched: false,
            downloaded: false,
            number: None,
            special: false,
            numbers: Vec::new(),
        }
    }

    /// An episode watched or not, a special or not.
    fn marked(id: &str, watched: bool, special: bool) -> Episode {
        Episode {
            watched,
            special,
            ..episode(id, None)
        }
    }

    fn session(groups: Vec<Group>, episodes: Vec<Episode>) -> Session {
        Session {
            generation: 1,
            origin: Origin::Stremio {
                kind: "series".into(),
                id: "tt1".into(),
            },
            title: "Show".into(),
            poster: None,
            episodes,
            index: 0,
            groups,
            current: None,
            tried: HashSet::new(),
            start_at: None,
            memory: Remembered::default(),
            show_memory: None,
            health: SourceHealth::default(),
            resolution: VideoResolution::default(),
            dead_urls: HashSet::new(),
            watching: None,
            resolving_since: 0,
            settled: false,
            wanted: None,
            quality_tried: HashSet::new(),
            variants: Vec::new(),
            playing_variant: None,
            duration_ms: None,
            speeds: HashMap::new(),
            link_speeds: HashMap::new(),
            speed_saved_at: 0,
            link: None,
            link_error: None,
            player_waits: false,
        }
    }

    /// A link played on its own: its own group (a torrent's videos), and the sources found for
    /// the movie it was matched with.
    fn link_session(own: Vec<Link>, others: Vec<Group>) -> Session {
        let key = "url:magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d566095a10".to_owned();
        let request = crate::video::link_request(&key["url:".len()..], Some("Sintel"), None);
        let mut groups = vec![Group::finished(
            LINK_GROUP.into(),
            "Sintel".into(),
            own,
            None,
        )];
        groups.extend(others);
        Session {
            origin: Origin::Link { key: key.clone() },
            title: "Sintel".into(),
            settled: true,
            link: Some(LinkPlay {
                request,
                file: None,
            }),
            ..session(groups, vec![episode("x", None)])
        }
    }

    #[test]
    fn a_link_plays_its_own_files_first_and_other_sources_once_they_fail() {
        let mut own = link("Sintel.mp4", None);
        own.request.url =
            "magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d566095a10&so=5".into();
        let mut extra = link("Sintel.720p.mkv", None);
        extra.request.url =
            "magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d566095a10&so=7".into();
        // An addon's 1080p stream ranks above an unknown size, but the link is what was asked.
        let addon = Group::finished(
            "addon".into(),
            "Torrentio".into(),
            vec![link("1080p WEB-DL", None)],
            None,
        );
        let mut state = link_session(vec![own, extra], vec![addon]);
        assert_eq!(state.next_candidate(NOW), Some((0, 0)));
        state.tried.insert((0, 0));
        state.current = Some((0, 0));
        // Its torrent's other video is another file, picked by hand: the movie's other sources
        // stand in for it.
        assert_eq!(state.next_candidate(NOW), Some((1, 0)));
        assert!(!state.alone());

        let update = sources_update(&state, NOW);
        assert_eq!(update["link"], true);
        assert_eq!(update["alone"], false);
        assert_eq!(update["fallback"], true);
        assert_eq!(update["order"][0], 0, "the link lists itself first");
        assert_eq!(update["groups"][0]["own"], true);
        assert_eq!(update["groups"][1]["own"], false);

        // With no other source, nothing to fall back on: the player says why it failed.
        let mut own_only = link_session(
            vec![link("Sintel.mp4", None), link("Sintel.sample.mp4", None)],
            Vec::new(),
        );
        own_only.tried.insert((0, 0));
        own_only.current = Some((0, 0));
        assert_eq!(sources_update(&own_only, NOW)["fallback"], false);
        assert!(!own_only.alone(), "its other video can still be picked");
    }

    #[test]
    fn a_link_with_nothing_else_says_so() {
        let state = link_session(vec![link("Sintel.mp4", None)], Vec::new());
        assert!(state.alone());
        let update = sources_update(&state, NOW);
        assert_eq!(update["alone"], true);
        assert_eq!(update["groups"][0]["name"], "Sintel");
        // Still finding the matched movie's sources: not alone yet.
        let waiting = link_session(
            vec![link("Sintel.mp4", None)],
            vec![Group::resolving(MATCH_GROUP.into(), "Other sources".into())],
        );
        assert!(!waiting.alone());
        assert_eq!(
            waiting.next_candidate(NOW),
            Some((0, 0)),
            "no waiting for them"
        );
        // A regular session is never "alone".
        assert!(!session(vec![], vec![episode("e", None)]).alone());
    }

    #[test]
    fn a_link_group_still_finding_its_videos_holds_the_others() {
        let mut state = link_session(
            Vec::new(),
            vec![Group::finished(
                "addon".into(),
                "Torrentio".into(),
                vec![link("1080p", None)],
                None,
            )],
        );
        state.groups[0].state = GroupState::Resolving;
        assert_eq!(state.next_candidate(NOW), None);
    }

    #[test]
    fn link_origins_are_their_own_group() {
        let origin = Origin::Link {
            key: "url:https://cdn.example/a.mp4".into(),
        };
        assert_eq!(origin.group_key(), "url:https://cdn.example/a.mp4");
        assert_eq!(origin.anime(), None);
        // Never read back from a group: a link's entry keeps its request.
        assert_eq!(Origin::from_group(&origin.group_key()), None);

        // In other sessions a server whose id is `link` is a source like any other; a download
        // is every session's own.
        let regular = session(
            vec![Group::finished(
                LINK_GROUP.into(),
                "Server".into(),
                vec![link("720p", None)],
                None,
            )],
            vec![episode("e", None)],
        );
        assert!(!own_group(false, &regular.groups[0]));
        assert!(own_group(true, &regular.groups[0]));
        let download =
            Group::finished(DOWNLOAD_GROUP.into(), "Downloaded".into(), Vec::new(), None);
        assert!(own_group(false, &download));
        let update = sources_update(&regular, NOW);
        assert_eq!(update["groups"][0]["own"], false);
        assert_eq!(update["link"], false);
        assert_eq!(update["alone"], false);
    }

    #[test]
    fn the_next_episode_never_crosses_into_the_specials() {
        let state = session(
            Vec::new(),
            vec![
                marked("s1e1", false, false),
                marked("s1e2", false, false),
                marked("s0e1", false, true),
                marked("s0e2", false, true),
            ],
        );
        assert_eq!(state.next_after(0, 100), Some(1));
        assert_eq!(state.next_after(1, 100), None, "the last regular episode");
        assert_eq!(
            state.next_after(2, 100),
            Some(3),
            "from a special to the next"
        );
        assert_eq!(state.previous_before(2, 100), Some(1));
    }

    #[test]
    fn watching_continues_after_the_last_episode_watched() {
        let at = |episodes: Vec<Episode>| continue_episode(&episodes, &HashMap::new(), 100);
        assert_eq!(
            at(vec![marked("a", false, false), marked("b", false, false)]),
            0,
            "nothing watched: the first"
        );
        assert_eq!(
            at(vec![
                marked("a", true, false),
                marked("b", false, false),
                marked("c", true, false),
                marked("d", false, false),
            ]),
            3,
            "after the last watched, not the gap"
        );
        assert_eq!(
            at(vec![
                marked("a", true, false),
                marked("b", false, false),
                marked("c", true, false),
                marked("special", false, true),
            ]),
            1,
            "all watched to the end: the gap, never a special"
        );
        assert_eq!(
            at(vec![marked("a", true, false), marked("b", true, false)]),
            0,
            "all watched: from the start"
        );
        let mut upcoming = vec![marked("a", true, false), marked("b", false, false)];
        upcoming[1].released = Some(1_000);
        assert_eq!(at(upcoming), 0, "an episode not out yet isn't next");
        assert_eq!(
            at(vec![marked("ova", true, true), marked("ova2", false, true)]),
            1,
            "a title of specials only"
        );
    }

    #[test]
    fn links_naming_another_episode_play_last() {
        let numbers = [
            video_match::EpisodeNumber {
                season: Some(2),
                episode: 1,
            },
            video_match::EpisodeNumber {
                season: None,
                episode: 26,
            },
        ];
        let mut groups = vec![
            Group::finished(
                "a".into(),
                "Remembered addon".into(),
                vec![link("Show.S01E01.1080p", None)],
                None,
            ),
            Group::finished(
                "b".into(),
                "Other addon".into(),
                vec![
                    link("Show Season 1 Pack", None),
                    link("1080p · Show.S02E01.1080p", None),
                ],
                None,
            ),
        ];
        mark_mismatches(&mut groups, &numbers);
        assert!(groups[0].links[0].mismatch);
        assert_eq!(groups[1].links[0].label, "1080p · Show.S02E01.1080p");
        assert!(groups[1].links[1].mismatch);
        let mut state = session(groups, vec![episode("e1", None)]);
        assert_eq!(
            state.next_candidate(NOW),
            Some((1, 0)),
            "the right episode first"
        );
        state.tried.insert((1, 0));
        assert_eq!(
            state.next_candidate(NOW),
            Some((0, 0)),
            "then the rest, in group order"
        );
    }

    #[test]
    fn a_shows_source_is_remembered_across_its_seasons() {
        assert_eq!(
            show_memory_key("source:site", "Overlord II"),
            show_memory_key("source:site", "Overlord")
        );
        assert_eq!(
            show_memory_key("stremio", "Attack on Titan Season 2").as_deref(),
            Some("video.lastSource.show:stremio:attackontitan")
        );
        assert_ne!(
            show_memory_key("source:a", "Overlord"),
            show_memory_key("source:b", "Overlord")
        );
        assert_eq!(show_memory_key("stremio", "!!!"), None);
    }

    #[test]
    fn kitsu_entries_leave_their_one_season_out_of_labels() {
        let video = |season: Option<u32>| raygun_model::StremioVideo {
            id: "kitsu:8671:1".into(),
            title: "Beast Titan".into(),
            season,
            episode: Some(1),
            released: None,
            thumbnail: None,
            overview: None,
            position_ms: 0.0,
            duration_ms: None,
            watched: false,
            downloaded: false,
        };
        let meta =
            |name: &str, videos: Vec<raygun_model::StremioVideo>| raygun_model::StremioMeta {
                adult: false,
                continue_video_id: None,
                kind: "series".into(),
                id: "kitsu:8671".into(),
                name: name.into(),
                poster: None,
                background: None,
                logo: None,
                description: None,
                genres: Vec::new(),
                release_info: None,
                runtime: None,
                rating: None,
                ratings: Vec::new(),
                cast: Vec::new(),
                director: Vec::new(),
                trailer_yt_ids: Vec::new(),
                videos,
                position_ms: 0.0,
                duration_ms: None,
                watched: false,
                downloaded: false,
                in_library: false,
                title_id: None,
                addon_name: String::new(),
                addon_id: String::new(),
                video_id: "kitsu:8671".into(),
                live: false,
            };
        assert!(!shows_seasons(&meta(
            "Attack on Titan Season 2",
            vec![video(Some(1))]
        )));
        assert!(shows_seasons(&meta(
            "Attack on Titan",
            vec![video(Some(1))]
        )));
        assert!(shows_seasons(&meta(
            "Attack on Titan",
            vec![video(Some(1)), video(Some(2))]
        )));
    }

    /// Far enough from 0 that nothing waits for a resolving source.
    const NOW: i64 = 10 * 24 * 60 * 60 * 1000;

    fn worked_memory(group: &str, label: &str, tag: Option<&str>) -> Remembered {
        let mut memory = Remembered::default();
        memory.worked(group, &link(label, tag), NOW - 1_000);
        memory
    }

    fn names(groups: &[Group]) -> Vec<&str> {
        groups.iter().map(|group| group.name.as_str()).collect()
    }

    #[test]
    fn links_follow_the_resolution_within_each_standing() {
        let addon = || {
            vec![Group::finished(
                "a".into(),
                "Addon".into(),
                vec![
                    link("Addon 720p", Some("web-720")),
                    link("Addon 4k", Some("web-4k")),
                    link("Addon", None),
                    link("Addon 1080p", Some("web-1080")),
                    link("Addon 2160p S01E05", None),
                ],
                None,
            )]
        };
        let health = SourceHealth::default();
        let order = |memory: &Remembered, resolution| {
            let mut groups = addon();
            groups[0].links[4].mismatch = true;
            order_groups(&mut groups, &Ranking::new(memory, &health, resolution, NOW));
            groups[0]
                .links
                .iter()
                .map(|link| link.label.clone())
                .collect::<Vec<_>>()
        };
        let nothing = Remembered::default();
        assert_eq!(
            order(&nothing, VideoResolution::P1080),
            [
                "Addon 1080p",
                "Addon 720p",
                "Addon 4k",
                "Addon",
                "Addon 2160p S01E05"
            ]
        );
        assert_eq!(order(&nothing, VideoResolution::Highest)[0], "Addon 4k");
        // The link that played last time stays first; the rest follow the resolution.
        let memory = worked_memory("Addon", "Addon 720p", Some("web-720"));
        assert_eq!(
            order(&memory, VideoResolution::Highest)[..3],
            ["Addon 720p", "Addon 4k", "Addon 1080p"]
        );
    }

    #[test]
    fn the_link_that_played_comes_first_then_its_release_then_what_worked() {
        let mut groups = vec![
            Group::finished("a".into(), "Alpha".into(), vec![link("a1", None)], None),
            Group::finished("w".into(), "Worked".into(), vec![link("w1", None)], None),
            Group::finished(
                "c".into(),
                "Charlie".into(),
                vec![link("c1", Some("x")), link("c2", Some("release"))],
                None,
            ),
            Group::finished(
                "b".into(),
                "Beta".into(),
                vec![link("480p", None), link("Beta 1080p", None)],
                None,
            ),
        ];
        let memory = worked_memory("Beta", "Beta 1080p", Some("release"));
        let mut health = SourceHealth::default();
        health.worked("Worked", NOW - 1_000);
        order_groups(
            &mut groups,
            &Ranking::new(&memory, &health, VideoResolution::default(), NOW),
        );
        assert_eq!(names(&groups), ["Beta", "Charlie", "Worked", "Alpha"]);
        assert_eq!(groups[0].links[0].label, "Beta 1080p", "the very link");
        assert_eq!(groups[1].links[0].label, "c2", "the same bingeGroup");

        // Nothing remembered keeps the order.
        let mut fresh = groups.clone();
        let empty = Remembered::default();
        let none = SourceHealth::default();
        order_groups(
            &mut fresh,
            &Ranking::new(&empty, &none, VideoResolution::default(), NOW),
        );
        assert_eq!(names(&fresh), names(&groups));
    }

    #[test]
    fn the_server_that_played_comes_next_across_episodes_at_the_preferred_resolution() {
        // The next episode's links have other labels: the server that played comes first, its
        // links by the preferred resolution (1080p unless changed).
        let memory = worked_memory("Vidstream", "720p · Episode 3", None);
        let health = SourceHealth::default();
        let ranking = Ranking::new(&memory, &health, VideoResolution::default(), NOW);
        let mut groups = vec![
            Group::finished("o".into(), "Other".into(), vec![link("1080p", None)], None),
            Group::finished(
                "v".into(),
                "Vidstream".into(),
                vec![
                    link("720p · Episode 4", None),
                    link("1080p · Episode 4", None),
                ],
                None,
            ),
        ];
        order_groups(&mut groups, &ranking);
        assert_eq!(groups[0].name, "Vidstream");
        assert_eq!(groups[0].links[0].label, "1080p · Episode 4");
        assert_eq!(
            ranking.link_standing("Vidstream", "1080p · Episode 4", None),
            Standing::Worked
        );
        let capped = Ranking::new(&memory, &health, VideoResolution::P720, NOW);
        order_groups(&mut groups, &capped);
        assert_eq!(groups[0].links[0].label, "720p · Episode 4");
    }

    #[test]
    fn sources_that_failed_lately_go_last_even_the_remembered_one() {
        let memory = worked_memory("Beta", "b1", None);
        let mut health = SourceHealth::default();
        health.worked("Beta", NOW - 5_000);
        health.failed("Beta", NOW - 1_000);
        let mut groups = vec![
            Group::finished("b".into(), "Beta".into(), vec![link("b1", None)], None),
            Group::finished("a".into(), "Alpha".into(), vec![link("a1", None)], None),
        ];
        order_groups(
            &mut groups,
            &Ranking::new(&memory, &health, VideoResolution::default(), NOW),
        );
        assert_eq!(names(&groups), ["Alpha", "Beta"]);

        // Working again clears it.
        health.worked("Beta", NOW);
        order_groups(
            &mut groups,
            &Ranking::new(&memory, &health, VideoResolution::default(), NOW),
        );
        assert_eq!(names(&groups), ["Beta", "Alpha"]);
    }

    #[test]
    fn a_dead_link_goes_last_and_working_again_brings_it_back() {
        let mut memory = worked_memory("Alpha", "a1", None);
        memory.failed("Alpha", &link("a1", None), NOW);
        let health = SourceHealth::default();
        let ranking = Ranking::new(&memory, &health, VideoResolution::default(), NOW);
        assert_eq!(
            ranking.link_standing("Alpha", "a1", None),
            Standing::Failing
        );
        assert_eq!(ranking.link_standing("Alpha", "a2", None), Standing::Worked);
        // Forgotten after three days.
        let later = Ranking::new(
            &memory,
            &health,
            VideoResolution::default(),
            NOW + memory::FAILING_FOR_MS,
        );
        assert_eq!(
            later.link_standing("Alpha", "a1", None),
            Standing::LastPlayed
        );
        memory.worked("Alpha", &link("a1", None), NOW + 1);
        assert!(memory.dead.is_empty());
    }

    #[test]
    fn source_memory_fades_and_stays_bounded() {
        let mut health = SourceHealth::default();
        health.worked("Often", 0);
        health.worked("Often", 1);
        health.worked("Once", 1);
        assert!(health.score("Often", 2) > health.score("Once", 2));
        let week = 7 * 24 * 60 * 60 * 1000;
        assert!(
            (health.score("Once", 1 + week) - 0.5).abs() < 0.01,
            "halves in a week"
        );
        for index in 0..400 {
            health.failed(&format!("source {index}"), 10 + index);
        }
        assert_eq!(health.sources.len(), 300);
        assert!(health.sources.contains_key("source 399"));
        health.failed("Late", 61 * 24 * 60 * 60 * 1000);
        assert_eq!(health.sources.len(), 1, "two months later only the new one");

        let mut memory = Remembered::default();
        for index in 0..40 {
            memory.failed("Alpha", &link(&format!("l{index}"), None), index);
        }
        assert_eq!(memory.dead.len(), 24);
        assert_eq!(memory.dead[0].label, "l39", "newest first");
    }

    #[test]
    fn the_old_health_format_still_reads() {
        let health: SourceHealth =
            serde_json::from_str(r#"{"sources":{"Bad":[1000,2000],"New":{"failed":5}}}"#).unwrap();
        assert_eq!(health.sources["Bad"].failed, 2_000);
        assert_eq!(health.sources["New"].failed, 5);
        let memory: Remembered = serde_json::from_str(r#"{"group":"Beta","tag":null}"#).unwrap();
        assert_eq!(memory.group, "Beta");
        let ranking = Ranking::new(&memory, &health, VideoResolution::default(), NOW);
        assert!(!ranking.played_last("Beta"), "only loaded, never confirmed");
        assert_eq!(ranking.group_standing("Beta"), Standing::LastPlayed);
    }

    #[test]
    fn sources_that_failed_since_they_worked_count_as_failing_for_a_while() {
        let mut health = SourceHealth::default();
        health.worked("Good", 1_000);
        health.worked("Bad", 1_000);
        health.failed("Bad", 2_000);
        health.failed("Fixed", 1_000);
        health.worked("Fixed", 3_000);
        health.failed("Never worked", 1_000);
        assert_eq!(
            health.failing(5_000),
            HashSet::from(["Bad".to_owned(), "Never worked".to_owned()])
        );
        assert!(
            health.failing(2_000 + memory::FAILING_FOR_MS).is_empty(),
            "forgotten after three days"
        );
    }

    #[test]
    fn a_session_plays_what_worked_and_never_retries_a_dead_address() {
        let mut memory = Remembered::default();
        memory.worked("Beta", &link("b1", None), NOW - 1_000);
        let mut state = session(
            vec![
                Group::finished("a".into(), "Alpha".into(), vec![link("a1", None)], None),
                Group::finished(
                    "b".into(),
                    "Beta".into(),
                    vec![link("b0", None), link("b1", None)],
                    None,
                ),
                // Another addon listing the same address as Beta's b1.
                Group::finished("c".into(), "Copy".into(), vec![link("b1", None)], None),
            ],
            vec![episode("e1", None)],
        );
        state.memory = memory;
        assert_eq!(
            state.next_candidate(NOW),
            Some((1, 1)),
            "the link that played"
        );
        state.tried.insert((1, 1));
        state.dead_urls.insert("https://cdn.example/b1".into());
        assert_eq!(
            state.next_candidate(NOW),
            Some((1, 0)),
            "then its source's other link, never the same address elsewhere"
        );
        state.tried.insert((1, 0));
        assert_eq!(state.next_candidate(NOW), Some((0, 0)));
        state.tried.insert((0, 0));
        assert_eq!(state.next_candidate(NOW), None);
    }

    #[test]
    fn a_session_waits_a_little_for_the_source_that_worked() {
        let mut state = session(
            vec![
                Group::finished("a".into(), "Alpha".into(), vec![link("a1", None)], None),
                Group::resolving("b".into(), "Beta".into()),
            ],
            vec![episode("e1", None)],
        );
        state.memory = worked_memory("Beta", "b1", None);
        state.resolving_since = NOW;
        assert_eq!(
            state.next_candidate(NOW + 1_000),
            None,
            "Beta is still answering"
        );
        assert_eq!(
            state.next_candidate(NOW + PATIENCE_MS),
            Some((0, 0)),
            "not for long"
        );
        // An untried source resolving isn't worth waiting for.
        state.memory = Remembered::default();
        assert_eq!(state.next_candidate(NOW + 1_000), Some((0, 0)));
    }

    #[test]
    fn a_picker_lists_sources_in_play_order_with_the_one_that_worked() {
        let memory = worked_memory("Beta", "Beta 1080p", None);
        let mut health = SourceHealth::default();
        health.failed("Gamma", NOW - 1_000);
        let ranking = Ranking::new(&memory, &health, VideoResolution::default(), NOW);
        let candidate = |name: &str, labels: &[&str]| SourceCandidate {
            name: name.into(),
            links: labels
                .iter()
                .map(|label| LinkCandidate {
                    label: (*label).into(),
                    tag: None,
                })
                .collect(),
        };
        let ranks = rank_candidates(
            &[
                candidate("Gamma", &["g"]),
                candidate("Alpha", &[]),
                candidate("Beta", &["Beta 480p", "Beta 1080p"]),
            ],
            &ranking,
        );
        let order: Vec<u32> = ranks.iter().map(|rank| rank.index).collect();
        assert_eq!(order, [2, 1, 0]);
        assert!(ranks[0].worked);
        assert_eq!(ranks[0].worked_link, Some(1), "Beta 1080p");
        assert_eq!(ranks[0].links, [1, 0]);
        assert!(ranks[2].failing);
        assert!(!ranks[1].worked);
    }

    #[test]
    fn candidates_skip_failed_tried_and_unready_links() {
        let mut failed = link("a1", None);
        failed.failed = Some("403".into());
        let mut state = session(
            vec![
                Group::resolving("r".into(), "Slow".into()),
                Group::finished(
                    "a".into(),
                    "Alpha".into(),
                    vec![failed, link("a2", None)],
                    None,
                ),
                Group::finished("e".into(), "Empty".into(), Vec::new(), None),
                Group::finished("b".into(), "Beta".into(), vec![link("b1", None)], None),
            ],
            vec![episode("e1", None)],
        );
        assert_eq!(state.groups[2].state, GroupState::Empty);
        assert_eq!(state.next_candidate(NOW), Some((1, 1)));
        state.tried.insert((1, 1));
        assert_eq!(state.next_candidate(NOW), Some((3, 0)));
        state.tried.insert((3, 0));
        assert_eq!(state.next_candidate(NOW), None);
    }

    #[test]
    fn next_and_previous_skip_episodes_not_out_yet() {
        let mut state = session(
            Vec::new(),
            vec![
                episode("e1", Some(10)),
                episode("e2", Some(20)),
                episode("e3", Some(1_000)),
                episode("e4", None),
            ],
        );
        assert_eq!(state.next_after(0, 100), Some(1));
        assert_eq!(state.next_after(1, 100), Some(3), "e3 isn't out at 100");
        assert_eq!(state.next_after(3, 100), None);
        state.index = 3;
        assert_eq!(state.previous_before(3, 100), Some(1));
        assert_eq!(state.previous_before(0, 100), None);
    }

    #[test]
    fn a_session_without_an_episode_continues_where_watching_was() {
        let mut episodes: Vec<Episode> = (1..=8)
            .map(|number| episode(&format!("tt1:1:{number}"), None))
            .collect();
        episodes.insert(
            0,
            Episode {
                special: true,
                ..episode("tt1:0:1", None)
            },
        );
        let at = |pairs: &[(&str, i64)]| -> HashMap<String, i64> {
            pairs
                .iter()
                .map(|(id, at)| (format!("stremio:series:{id}"), *at))
                .collect()
        };
        assert_eq!(
            continue_episode(&episodes, &HashMap::new(), 0),
            1,
            "a fresh series starts at its first episode, not an extra"
        );
        // Episode 1 opened for a moment, then episode 7 watched.
        let watched = at(&[("tt1:1:1", 10), ("tt1:1:7", 20)]);
        assert_eq!(continue_episode(&episodes, &watched, 0), 7);
        // Episode 7 finished: the next one.
        episodes[7].watched = true;
        assert_eq!(continue_episode(&episodes, &watched, 0), 8);
        // Not out yet: back to the first one not watched.
        episodes[8].released = Some(1_000);
        assert_eq!(continue_episode(&episodes, &watched, 500), 1);
    }

    #[test]
    fn groups_name_their_titles() {
        let source = Origin::Source {
            title_id: "t1".into(),
        };
        assert_eq!(Origin::from_group(&source.group_key()), Some(source));
        let anime = Origin::Stremio {
            kind: "series".into(),
            id: "kitsu:7442".into(),
        };
        assert_eq!(Origin::from_group(&anime.group_key()), Some(anime));
        assert_eq!(Origin::from_group("url:https://cdn.example/a.mp4"), None);
        assert_eq!(Origin::from_group("stremio:series:"), None);
    }

    #[test]
    fn stremio_anime_is_looked_up_on_aniskip() {
        let stremio = |kind: &str, id: &str| Origin::Stremio {
            kind: kind.into(),
            id: id.into(),
        };
        let ids = |mal, kitsu, anilist| {
            Some(AnimeIds {
                mal,
                kitsu,
                anilist,
            })
        };
        assert_eq!(
            stremio("series", "kitsu:8671").anime(),
            ids(None, Some(8671), None),
            "each Kitsu entry is one season, with its own MyAnimeList id"
        );
        assert_eq!(stremio("anime", "tt2560140").anime(), ids(None, None, None));
        assert_eq!(
            stremio("series", "mal:16498").anime(),
            ids(Some(16498), None, None)
        );
        assert_eq!(
            stremio("series", "anilist:20958").anime(),
            ids(None, None, Some(20958))
        );
        assert_eq!(
            stremio("series", "anidb:9541").anime(),
            ids(None, None, None)
        );
        assert_eq!(stremio("series", "tt0903747").anime(), None);
        let source = Origin::Source {
            title_id: "t1".into(),
        };
        assert_eq!(source.anime(), None, "sources say so on the request");
        assert_eq!(source.group_key(), "title:t1");
        assert_eq!(stremio("series", "tt1").group_key(), "stremio:series:tt1");
    }

    #[test]
    fn episode_references_are_not_video_requests() {
        let reference = EpisodeRef {
            origin: Origin::Source {
                title_id: "t1".into(),
            },
            episode: "ep/1".into(),
        };
        let text = serde_json::to_string(&reference).unwrap();
        assert_eq!(
            text,
            r#"{"origin":"source","titleId":"t1","episode":"ep/1"}"#
        );
        assert!(serde_json::from_str::<VideoRequest>(&text).is_err());
        assert_eq!(
            serde_json::from_str::<EpisodeRef>(&text).unwrap(),
            reference
        );
    }

    /// Three sources: Alpha (1080p, 720p), Beta (2160p HDR, 720p), Gamma failed.
    fn quality_session() -> Session {
        let mut state = session(
            vec![
                Group::finished(
                    "a".into(),
                    "Alpha".into(),
                    vec![link("a 1080p", None), link("a 720p", None)],
                    None,
                ),
                Group::finished(
                    "b".into(),
                    "Beta".into(),
                    vec![link("b 2160p HDR HEVC", None), link("b 720p", None)],
                    None,
                ),
                Group::finished("c".into(), "Gamma".into(), Vec::new(), Some("down".into())),
            ],
            vec![episode("e1", None)],
        );
        state.current = Some((0, 0));
        state.tried.insert((0, 0));
        state
    }

    #[test]
    fn the_quality_asked_for_plays_first_and_after_a_failure() {
        let mut state = quality_session();
        state.wanted = Some(QualityKey::of(720, video_quality::Range::Sdr));
        state.current = None;
        assert_eq!(
            state.next_candidate(NOW),
            Some((0, 1)),
            "720p before the 1080p link the order would try"
        );
        state.tried.insert((0, 1));
        state.groups[0].links[1].failed = Some("gone".into());
        assert_eq!(
            state.next_candidate(NOW),
            Some((1, 1)),
            "another source's 720p when the first one fails"
        );
        state.tried.insert((1, 1));
        assert_eq!(state.next_candidate(NOW), Some((1, 0)), "then the rest");
    }

    #[test]
    fn the_quality_menu_follows_the_sessions_links() {
        let mut state = quality_session();
        let offers = state.offers(NOW);
        assert_eq!(offers.len(), 4);
        assert_eq!(offers[0].rank, 0);
        let update = state.quality_update(NOW, None);
        let items = update["items"].as_array().unwrap();
        let row = |key: &str| {
            items
                .iter()
                .find(|item| item["key"] == key)
                .cloned()
                .unwrap_or_else(|| panic!("no row {key}"))
        };
        assert_eq!(row("1080:sdr")["state"], "playing");
        assert_eq!(row("720:sdr")["state"], "thisSource");
        assert_eq!(row("720:sdr")["link"], 1);
        assert_eq!(row("2160:hdr")["state"], "otherSource");
        assert_eq!(row("2160:hdr")["source"], "Beta");
        assert_eq!(row("2160:hdr")["codec"], "HEVC");
        assert!(
            items.iter().all(|item| item["key"] != "480:sdr"),
            "no row for a tier nobody has"
        );
        // The stream's own variants come first.
        state.variants = vec![Variant {
            width: 1280,
            height: 720,
            range: video_quality::Range::Sdr,
            codec: None,
        }];
        state.playing_variant = Some(QualityKey::of(1080, video_quality::Range::Sdr));
        let update = state.quality_update(NOW, Some("note".into()));
        let items = update["items"].as_array().unwrap();
        let shown = items.iter().find(|item| item["key"] == "720:sdr").unwrap();
        assert_eq!(shown["state"], "variant");
        assert_eq!(update["notice"], "note");
    }

    #[test]
    fn variants_reported_without_a_session_make_a_menu_of_their_own() {
        let (variants, playing) = reported_variants(&json!({
            "items": [
                { "width": 3840, "height": 2160, "range": "hdr", "codec": "HEVC" },
                { "width": 1920, "height": 800 },
                { "height": 720 },
                { "height": 0 }
            ],
            "playing": { "width": 1920, "height": 800 }
        }));
        assert_eq!(variants.len(), 4);
        assert_eq!(
            playing,
            Some(QualityKey::of(1080, video_quality::Range::Sdr))
        );
        let update = variants_update(&variants, playing);
        let items = update["items"].as_array().unwrap();
        let row = |key: &str| items.iter().find(|item| item["key"] == key).cloned();
        assert_eq!(row("2160:hdr").unwrap()["state"], "variant");
        // A cropped film plays: its tier, with its own size.
        let playing = row("1080:sdr").unwrap();
        assert_eq!(playing["state"], "playing");
        assert_eq!(playing["exact"], "1920×800");
        assert_eq!(playing["sizes"], json!([{ "width": 1920, "height": 800 }]));
        assert_eq!(row("720:sdr").unwrap()["state"], "variant");
        assert_eq!(items.len(), 3, "only the tiers the stream has");
    }

    #[test]
    fn the_sources_sheet_lists_sources_by_what_works_quality_and_speed() {
        let mut state = quality_session();
        state
            .groups
            .push(Group::resolving("d".into(), "Delta".into()));
        state.groups.push(Group::finished(
            "e".into(),
            "Epsilon".into(),
            vec![link("e 1080p", None)],
            None,
        ));
        state.speeds.insert("Epsilon".into(), 40_000);
        state.speeds.insert("Alpha".into(), 9_000);
        state.duration_ms = Some(5_400_000);
        state.groups[1].links[0].size = Some(13_500_000_000);
        let update = sources_update(&state, NOW);
        // Beta has 2160p; Epsilon and Alpha both 1080p, Epsilon faster; Delta is still looking,
        // Gamma failed.
        assert_eq!(update["order"], json!([1, 4, 0, 3, 2]));
        let beta = &update["groups"][1];
        assert_eq!(beta["best"], 2160);
        assert_eq!(beta["options"], 2);
        let top = &beta["links"][0];
        assert_eq!(top["facts"]["range"], "hdr");
        assert_eq!(top["facts"]["codec"], "HEVC");
        assert_eq!(top["facts"]["sizeBytes"], 13_500_000_000_u64);
        assert_eq!(top["bitrateKbps"], 20_000);
        assert_eq!(update["groups"][0]["speedKbps"], 9_000);
        assert_eq!(update["groups"][0]["links"][0]["current"], true);
        assert_eq!(update["groups"][2]["state"], "failed");
        // Only torrents say so.
        assert!(top["torrent"].is_null());
    }

    #[test]
    fn torrents_in_the_sources_sheet_say_their_seeds() {
        let mut state = quality_session();
        let mut torrent = link(
            "Torrentio 1080p · Film.2010.1080p.mkv 👤 42 💾 1.4 GB",
            None,
        );
        torrent.request.url = "magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d566095a10".into();
        torrent.request.format = Some(raygun_torrent::FORMAT.into());
        let mut unknown = link("Torrent 720p", None);
        unknown.request.url = "https://example.com/film.torrent".into();
        state.groups.push(Group::finished(
            "t".into(),
            "Torrentio".into(),
            vec![torrent, unknown],
            None,
        ));
        let update = sources_update(&state, NOW);
        let links = &update["groups"][3]["links"];
        assert_eq!(links[0]["torrent"], json!({ "seeds": 42 }));
        assert_eq!(links[1]["torrent"], json!({ "seeds": null }));
    }

    #[test]
    fn measured_speeds_are_smoothed_kept_and_forgotten_with_their_source() {
        let mut health = SourceHealth::default();
        health.measured("Alpha", 9_000, NOW);
        assert_eq!(health.speed("Alpha"), Some(9_000));
        health.measured("Alpha", 3_000, NOW + 1);
        assert_eq!(health.speed("Alpha"), Some(7_000));
        assert_eq!(health.speed("Beta"), None);
        let text = serde_json::to_string(&health).unwrap();
        let back: SourceHealth = serde_json::from_str(&text).unwrap();
        assert_eq!(back.speed("Alpha"), Some(7_000));
        // A speed alone keeps a source for as long as a success would.
        health.worked("Beta", NOW + 61 * 24 * 60 * 60 * 1000);
        assert_eq!(health.speed("Alpha"), None);
    }

    #[test]
    fn progress_reports_say_live_length_and_speed() {
        let heard = Heard::of(&json!({
            "live": true, "durationMs": null, "bandwidthKbps": 4_200.0
        }));
        assert_eq!(
            heard,
            Heard {
                live: true,
                duration_ms: None,
                bandwidth_kbps: Some(4_200),
            }
        );
        assert_eq!(Heard::of(&json!({ "durationMs": 0 })), Heard::default());
    }
}
