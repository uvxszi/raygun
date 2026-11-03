//! Playback sessions (M44): watching a title the way Aniyomi, CloudStream and Stremio do. Opening
//! an episode opens the player at once; its links resolve in the background (every server of a
//! video source, every addon for Stremio) and the best one plays as soon as it is known. A link
//! that fails hands over to the next. The player asks for the previous or next episode, another
//! episode or another source, and gets the episode and source lists to show; finishing an episode
//! moves Continue watching on to the next one.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;
use raygun_db::titles;
use raygun_db::video::{self as store, VideoEntry};
use raygun_db::video_downloads;
use raygun_model::{SettingChange, VideoRequest};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};

use crate::catalog::{now_ms, reading_order, to_message};
use crate::startup::AppDb;
use crate::stremio::{StremioPlayTarget, StremioService};
use crate::video_library;
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
}

impl Origin {
    /// The title's player memory (speed, aspect, tracks; M45), as `VideoRequest::group`.
    pub fn group_key(&self) -> String {
        match self {
            Self::Source { title_id } => format!("title:{title_id}"),
            Self::Stremio { kind, id } => format!("stremio:{kind}:{id}"),
        }
    }

    /// Stremio anime (Kitsu, MyAnimeList or AniList ids, or the `anime` type) can be looked up on
    /// AniSkip; a MyAnimeList id is used as it is.
    fn anime(&self) -> Option<Option<u32>> {
        let Self::Stremio { kind, id } = self else {
            return None;
        };
        if let Some(mal) = id.strip_prefix("mal:") {
            return Some(mal.parse().ok());
        }
        let anime = kind == "anime"
            || ["kitsu:", "anilist:", "kitsu-", "mal-"]
                .iter()
                .any(|prefix| id.starts_with(prefix));
        anime.then_some(None)
    }

    /// The key a title's remembered source is kept under.
    fn memory_key(&self) -> String {
        match self {
            Self::Source { title_id } => format!("video.lastSource.title:{title_id}"),
            Self::Stremio { kind, id } => format!("video.lastSource.stremio:{kind}:{id}"),
        }
    }
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
}

/// A playable link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub label: String,
    /// Stremio's `bingeGroup`: links of one release across episodes.
    pub tag: Option<String>,
    /// Why it can't play, once known.
    pub failed: Option<String>,
    pub request: VideoRequest,
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

/// How long a server or addon that failed stays at the end of the list (M46.17).
const FAILING_FOR_MS: i64 = 3 * 24 * 60 * 60 * 1000;
const HEALTH_KEY: &str = "video.sourceHealth";

/// How servers and addons did lately, by name (M46.17): when each was last tried and last failed.
/// One that failed since it was last tried goes to the end of the list for three days, so known
/// good sources are tried first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceHealth {
    pub sources: std::collections::HashMap<String, (i64, i64)>,
}

impl SourceHealth {
    /// The names that failed lately and haven't worked since.
    pub fn failing(&self, now: i64) -> HashSet<String> {
        self.sources
            .iter()
            .filter(|(_, (tried, failed))| *failed >= *tried && now - *failed < FAILING_FOR_MS)
            .map(|(name, _)| name.clone())
            .collect()
    }

    fn tried(&mut self, name: &str, now: i64) {
        self.sources.entry(name.to_owned()).or_default().0 = now;
    }

    fn failed(&mut self, name: &str, now: i64) {
        self.sources.entry(name.to_owned()).or_default().1 = now;
    }
}

/// The source played last for a title, to try first next time.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Remembered {
    /// The server's (video sources) or addon's (Stremio) name.
    pub group: String,
    /// Stremio's `bingeGroup` of the stream.
    pub tag: Option<String>,
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
    remembered: Option<Remembered>,
    /// Servers and addons that failed lately (M46.17): tried last, and marked.
    failing: HashSet<String>,
    /// Resolving finished; with nothing playing, the session has run out of links.
    settled: bool,
}

impl Session {
    fn episode(&self) -> &Episode {
        &self.episodes[self.index]
    }

    /// The next episode that is out, after `index`.
    fn next_after(&self, index: usize, now: i64) -> Option<usize> {
        (index + 1..self.episodes.len()).find(|&next| released(&self.episodes[next], now))
    }

    fn previous_before(&self, index: usize, now: i64) -> Option<usize> {
        (0..index)
            .rev()
            .find(|&previous| released(&self.episodes[previous], now))
    }

    /// The first untried link, in group order: what plays next.
    fn next_candidate(&self) -> Option<(usize, usize)> {
        self.groups
            .iter()
            .enumerate()
            .filter(|(_, group)| group.state == GroupState::Ready)
            .flat_map(|(group_index, group)| {
                group
                    .links
                    .iter()
                    .enumerate()
                    .map(move |(link_index, link)| (group_index, link_index, link))
            })
            .find(|(group, link, entry)| {
                entry.failed.is_none() && !self.tried.contains(&(*group, *link))
            })
            .map(|(group, link, _)| (group, link))
    }
}

fn released(episode: &Episode, now: i64) -> bool {
    episode.released.is_none_or(|at| at <= now)
}

/// Puts the remembered source first (its group, and within Stremio the links of the same
/// `bingeGroup` first) and the sources that failed lately last (M46.17).
pub fn order_groups(
    groups: &mut [Group],
    remembered: Option<&Remembered>,
    failing: &HashSet<String>,
) {
    groups.sort_by_key(|group| {
        (
            failing.contains(&group.name),
            remembered.is_none_or(|remembered| group.name != remembered.group),
        )
    });
    let Some(remembered) = remembered else {
        return;
    };
    if let Some(tag) = &remembered.tag {
        for group in groups.iter_mut() {
            group
                .links
                .sort_by_key(|link| link.tag.as_ref() != Some(tag));
        }
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

    fn label(&self, key: &str, fallback: &str) -> String {
        self.labels
            .lock()
            .expect("labels poisoned")
            .get(key)
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .unwrap_or(fallback)
            .to_owned()
    }

    /// Opens the player on an episode (or the first one not watched yet) and starts finding its
    /// links.
    pub async fn start(&self, origin: Origin, episode: Option<String>) -> Result<(), String> {
        let (title, poster, episodes) = self.episodes(&origin).await?;
        if episodes.is_empty() {
            return Err("This title has no episodes".into());
        }
        let now = now_ms();
        let index = episode
            .and_then(|id| episodes.iter().position(|item| item.remote_id == id))
            .or_else(|| {
                episodes
                    .iter()
                    .position(|item| !item.watched && released(item, now))
            })
            .unwrap_or(0);
        let remembered = self.remembered(&origin).await;
        let failing = self.health().await.failing(now_ms());
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
            start_at: None,
            remembered,
            failing,
            settled: false,
        });
        let shell = crate::video::session_shell(&self.app, &title, &label).await;
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
                self.fall_back(key, position, message).await;
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
            _ => {}
        }
    }

    async fn on_request(&self, report: &Value, position: i64) {
        let action = report
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let now = now_ms();
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
            session.settled = false;
            format!("{}\n{}", session.title, session.episode().label)
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
            session.start_at = (position > 0).then_some(position);
            session.generation
        };
        self.play(generation, group, link).await;
    }

    /// The playing link failed: the next one plays from the same spot.
    async fn fall_back(&self, key: &str, position: i64, error: String) {
        let mut failed_group = None;
        let generation = {
            let mut guard = self.session.lock().expect("session poisoned");
            let Some(session) = guard.as_mut() else {
                return;
            };
            if session.episode().key != key {
                return;
            }
            if let Some((group, link)) = session.current.take() {
                if let Some(entry) = session.groups.get(group) {
                    failed_group = Some(entry.name.clone());
                    session.failing.insert(entry.name.clone());
                }
                if let Some(entry) = session
                    .groups
                    .get_mut(group)
                    .and_then(|entry| entry.links.get_mut(link))
                {
                    entry.failed = Some(error);
                }
            }
            if position > 0 {
                session.start_at = Some(position);
            }
            session.generation
        };
        if let Some(name) = failed_group {
            self.note_health(|health| health.failed(&name, now_ms()))
                .await;
        }
        self.push_sources().await;
        self.try_play(generation).await;
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
            };
            let link = Link {
                label: self.label("onDevice", "On this device"),
                tag: None,
                failed: None,
                request,
            };
            let group = Group::finished(
                "download".into(),
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
            Origin::Stremio { kind, id } => {
                let target = StremioPlayTarget {
                    kind,
                    meta_id: Some(id),
                    video_id: episode.remote_id.clone(),
                    title,
                    subtitle: Some(episode.label.clone()),
                    poster,
                };
                let language = crate::video::subtitle_language(&db).await;
                let waiting = Group::resolving("addons".into(), self.label("addons", "Addons"));
                self.set_groups(generation, vec![waiting]).await;
                let stremio = self.app.state::<std::sync::Arc<StremioService>>();
                let groups = match stremio.playable(&target, &language).await {
                    Ok(groups) => groups,
                    Err(error) => vec![Group::finished(
                        "addons".into(),
                        self.label("addons", "Addons"),
                        Vec::new(),
                        Some(error),
                    )],
                };
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
                .map(|link| Link {
                    label: link.label.clone().unwrap_or_else(|| {
                        link.quality.map_or_else(
                            || self.label("video", "Video"),
                            |height| format!("{height}p"),
                        )
                    }),
                    tag: None,
                    failed: None,
                    request: video_sources::link_request(&row, unit, single, link),
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

    /// Replaces the groups (ordered by the remembered source), then plays if nothing does yet.
    async fn set_groups(&self, generation: u64, mut groups: Vec<Group>) {
        {
            let mut guard = self.session.lock().expect("session poisoned");
            let Some(session) = guard
                .as_mut()
                .filter(|session| session.generation == generation)
            else {
                return;
            };
            order_groups(&mut groups, session.remembered.as_ref(), &session.failing);
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
            if let Some(group) = session.groups.iter_mut().find(|group| group.id == id) {
                let (links, error) = match links {
                    Ok(links) => (links, None),
                    Err(error) => (Vec::new(), Some(error)),
                };
                *group = Group::finished(group.id.clone(), group.name.clone(), links, error);
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
            match session.next_candidate() {
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
                    if session.settled && !resolving {
                        Next::Exhausted
                    } else {
                        Next::Wait
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
            Next::Wait => {}
        }
    }

    /// Loads a link into the player and remembers its source for the title.
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
            let resume = EpisodeRef {
                origin: session.origin.clone(),
                episode: session.episode().remote_id.clone(),
            };
            let mut request = chosen.request.clone();
            request
                .group
                .get_or_insert_with(|| session.origin.group_key());
            if request.anime.is_none()
                && let Some(mal_id) = session.origin.anime()
                && let Some(episode) = session.episode().number
            {
                request.anime = Some(raygun_model::AnimeEpisode {
                    title: session.title.clone(),
                    episode,
                    mal_id,
                });
            }
            let remembered = Remembered {
                group: entry.name.clone(),
                tag: chosen.tag.clone(),
            };
            (
                request,
                resume,
                session.start_at,
                session.origin.clone(),
                remembered,
                entry.id == "download",
            )
        };
        let (request, resume, start_at, origin, remembered, downloaded) = prepared;
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
        let resume = serde_json::to_string(&resume).ok();
        match crate::video::prepare(&self.app, &request, resume, start_at).await {
            Ok(payload) => {
                if !self.is_current(generation) {
                    return;
                }
                let _ = crate::video::update_player(
                    &self.app,
                    &json!({ "type": "load", "request": payload }),
                );
                self.push_episodes().await;
                if !downloaded {
                    self.remember(&origin, &remembered).await;
                    let name = remembered.group.clone();
                    self.note_health(|health| health.tried(&name, now_ms()))
                        .await;
                    // It plays: no longer one that "didn't work lately".
                    let cleared = self
                        .session
                        .lock()
                        .expect("session poisoned")
                        .as_mut()
                        .is_some_and(|session| session.failing.remove(&name));
                    if cleared {
                        self.push_sources().await;
                    }
                }
            }
            Err(error) => {
                tracing::warn!(%error, "a link couldn't be prepared");
                let key = request.key.clone();
                Box::pin(self.fall_back(&key, start_at.unwrap_or(0), error)).await;
            }
        }
        self.push_sources().await;
    }

    /// An episode was watched: Continue watching moves on to the next one that is out, as Stremio
    /// does (its position is 1 ms, so it shows as up next).
    pub async fn on_watched(&self, key: &str) {
        let next = {
            let guard = self.session.lock().expect("session poisoned");
            let Some(session) = guard.as_ref() else {
                return;
            };
            let Some(index) = session.episodes.iter().position(|item| item.key == key) else {
                return;
            };
            session.next_after(index, now_ms()).map(|next| {
                let episode = &session.episodes[next];
                (
                    VideoEntry {
                        key: episode.key.clone(),
                        title: session.title.clone(),
                        subtitle: Some(episode.label.clone()),
                        poster: session.poster.clone(),
                        request: serde_json::to_string(&EpisodeRef {
                            origin: session.origin.clone(),
                            episode: episode.remote_id.clone(),
                        })
                        .unwrap_or_else(|_| "{}".into()),
                    },
                    episode.watched,
                )
            })
        };
        let Some((entry, already_watched)) = next else {
            return;
        };
        let db = self.db();
        let saved = store::progress(&db, &entry.key).await.ok().flatten();
        let untouched = saved
            .as_ref()
            .is_none_or(|row| row.position_ms <= 0 && row.watched_at.is_none());
        if already_watched || !untouched {
            return;
        }
        let now = now_ms();
        if let Err(error) = store::started(&db, &entry, now).await {
            tracing::warn!(%error, "could not queue the next episode");
            return;
        }
        let _ = store::set_progress(&db, &entry.key, 1, None, false, now).await;
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

    async fn push_sources(&self) {
        let update = {
            let guard = self.session.lock().expect("session poisoned");
            let Some(session) = guard.as_ref() else {
                return;
            };
            json!({
                "type": "sources",
                "groups": session.groups.iter().enumerate().map(|(group_index, group)| {
                    let (state, error) = match &group.state {
                        GroupState::Resolving => ("resolving", None),
                        GroupState::Ready => ("ready", None),
                        GroupState::Empty => ("empty", None),
                        GroupState::Failed(error) => ("failed", Some(error.clone())),
                    };
                    json!({
                        "name": group.name,
                        "state": state,
                        "error": error,
                        "unreliable": session.failing.contains(&group.name),
                        "links": group.links.iter().enumerate().map(|(link_index, link)| json!({
                            "label": link.label,
                            "current": session.current == Some((group_index, link_index)),
                            "failed": link.failed.is_some(),
                        })).collect::<Vec<_>>(),
                    })
                }).collect::<Vec<_>>(),
            })
        };
        let _ = crate::video::update_player(&self.app, &update);
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

    async fn remembered(&self, origin: &Origin) -> Option<Remembered> {
        raygun_db::local_value(&self.db(), &origin.memory_key())
            .await
            .ok()
            .flatten()
            .and_then(|text| serde_json::from_str(&text).ok())
    }

    async fn remember(&self, origin: &Origin, remembered: &Remembered) {
        let Ok(text) = serde_json::to_string(remembered) else {
            return;
        };
        if let Err(error) =
            raygun_db::set_local_value(&self.db(), &origin.memory_key(), &text).await
        {
            tracing::warn!(%error, "could not remember the source");
        }
    }

    /// A title's episodes in watching order, with its name and poster.
    async fn episodes(
        &self,
        origin: &Origin,
    ) -> Result<(String, Option<String>, Vec<Episode>), String> {
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
                let episodes = reading_order(&units)
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
                            key,
                        }
                    })
                    .collect();
                Ok((row.fields.title, row.fields.cover, episodes))
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
                    }]
                } else {
                    meta.videos
                        .iter()
                        .map(|video| Episode {
                            remote_id: video.id.clone(),
                            key: crate::stremio::content_key(kind, &video.id),
                            label: match (video.season, video.episode) {
                                (Some(season), Some(number)) => {
                                    format!("S{season} E{number} · {}", video.title)
                                }
                                _ => video.title.clone(),
                            },
                            unit_id: None,
                            released: video
                                .released
                                .as_deref()
                                .and_then(crate::stremio::release_ms),
                            watched: video.watched,
                            downloaded: video.downloaded,
                            number: video.episode.filter(|number| *number >= 1),
                        })
                        .collect()
                };
                Ok((meta.name, meta.poster, episodes))
            }
        }
    }

    fn db(&self) -> std::sync::Arc<raygun_db::Db> {
        self.app.state::<AppDb>().0.clone()
    }
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
    use super::*;

    fn link(label: &str, tag: Option<&str>) -> Link {
        Link {
            label: label.into(),
            tag: tag.map(str::to_owned),
            failed: None,
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
            remembered: None,
            failing: HashSet::new(),
            settled: false,
        }
    }

    #[test]
    fn the_remembered_source_and_binge_group_come_first() {
        let mut groups = vec![
            Group::finished("a".into(), "Alpha".into(), vec![link("a1", None)], None),
            Group::finished(
                "b".into(),
                "Beta".into(),
                vec![link("b1", Some("x")), link("b2", Some("release"))],
                None,
            ),
        ];
        order_groups(
            &mut groups,
            Some(&Remembered {
                group: "Beta".into(),
                tag: Some("release".into()),
            }),
            &HashSet::new(),
        );
        assert_eq!(groups[0].name, "Beta");
        assert_eq!(groups[0].links[0].label, "b2");
        order_groups(&mut groups, None, &HashSet::new());
        assert_eq!(groups[0].name, "Beta", "nothing remembered keeps the order");

        // One that failed lately goes last, even the remembered one (M46.17).
        order_groups(
            &mut groups,
            Some(&Remembered {
                group: "Beta".into(),
                tag: None,
            }),
            &HashSet::from(["Beta".to_owned()]),
        );
        assert_eq!(groups[0].name, "Alpha");
    }

    #[test]
    fn sources_that_failed_since_they_were_tried_count_as_failing_for_a_while() {
        let mut health = SourceHealth::default();
        health.tried("Good", 1_000);
        health.tried("Bad", 1_000);
        health.failed("Bad", 2_000);
        health.failed("Fixed", 1_000);
        health.tried("Fixed", 3_000);
        assert_eq!(health.failing(5_000), HashSet::from(["Bad".to_owned()]));
        assert!(
            health.failing(2_000 + FAILING_FOR_MS).is_empty(),
            "forgotten after three days"
        );
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
        assert_eq!(state.next_candidate(), Some((1, 1)));
        state.tried.insert((1, 1));
        assert_eq!(state.next_candidate(), Some((3, 0)));
        state.tried.insert((3, 0));
        assert_eq!(state.next_candidate(), None);
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
    fn stremio_anime_is_looked_up_on_aniskip() {
        let stremio = |kind: &str, id: &str| Origin::Stremio {
            kind: kind.into(),
            id: id.into(),
        };
        assert_eq!(stremio("series", "kitsu:7442").anime(), Some(None));
        assert_eq!(stremio("anime", "tt2560140").anime(), Some(None));
        assert_eq!(stremio("series", "mal:16498").anime(), Some(Some(16498)));
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
}
