//! What the download notifications say: every kind of download in one shape, each one's speed
//! and time left, the batch they belong to (from the first that starts until none is left), and
//! the notifications for it all. Pure, so it's tested without a phone.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

/// The kinds of download, each kept by its own service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Songs,
    Videos,
    Chapters,
    Podcasts,
    Exports,
    Models,
    Update,
    YtDlp,
    /// Files saved from websites.
    Files,
    /// The extension install queue, as one download: its batch.
    Extensions,
}

impl Kind {
    /// The prefix of its keys.
    pub fn tag(self) -> &'static str {
        match self {
            Kind::Songs => "songs",
            Kind::Videos => "videos",
            Kind::Chapters => "chapters",
            Kind::Podcasts => "podcasts",
            Kind::Exports => "exports",
            Kind::Models => "models",
            Kind::Update => "update",
            Kind::YtDlp => "ytdlp",
            Kind::Files => "files",
            Kind::Extensions => "extensions",
        }
    }

    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn from_tag(tag: &str) -> Option<Self> {
        Some(match tag {
            "songs" => Kind::Songs,
            "videos" => Kind::Videos,
            "chapters" => Kind::Chapters,
            "podcasts" => Kind::Podcasts,
            "exports" => Kind::Exports,
            "models" => Kind::Models,
            "update" => Kind::Update,
            "ytdlp" => Kind::YtDlp,
            "files" => Kind::Files,
            "extensions" => Kind::Extensions,
            _ => return None,
        })
    }

    /// Its queue waits in battery saver (unless downloads are allowed in it).
    pub fn waits_in_battery_saver(self) -> bool {
        matches!(self, Kind::Songs | Kind::Videos | Kind::Chapters)
    }

    /// Where its downloads are listed.
    pub fn route(self) -> &'static str {
        match self {
            Kind::Songs => "/downloads?tab=music",
            Kind::Videos => "/downloads?tab=videos",
            Kind::Chapters => "/downloads?tab=chapters",
            Kind::Exports | Kind::YtDlp => "/downloads?tab=exports",
            Kind::Files => "/downloads?tab=files",
            Kind::Podcasts => "/library?media=audio",
            Kind::Models => "/settings/ai#models",
            Kind::Update => "/about",
            Kind::Extensions => "/browse?tab=extensions&list=all",
        }
    }

    /// What one is called, for a download that has no title of its own.
    fn noun(self) -> &'static str {
        match self {
            Kind::Songs => "Song",
            Kind::Videos => "Video",
            Kind::Chapters => "Chapter",
            Kind::Podcasts => "Episode",
            Kind::Exports => "Export",
            Kind::Models => "AI model",
            Kind::Update => "Raygun update",
            Kind::YtDlp => "YouTube download",
            Kind::Files => "File",
            Kind::Extensions => "Extensions",
        }
    }

    /// What a running one is doing, when nothing else says.
    fn doing(self) -> &'static str {
        match self {
            Kind::Exports => "Exporting",
            Kind::Extensions => "Installing",
            _ => "Downloading",
        }
    }

    /// "3 songs", "1 video".
    fn count(self, count: usize) -> String {
        let (one, many) = match self {
            // A batch of installs counts as one.
            Kind::Extensions => return "extension installs".to_owned(),
            Kind::Songs => ("song", "songs"),
            Kind::Videos => ("video", "videos"),
            Kind::Chapters => ("chapter", "chapters"),
            Kind::Podcasts => ("episode", "episodes"),
            Kind::Exports => ("export", "exports"),
            Kind::Models => ("model", "models"),
            Kind::Update => ("update", "updates"),
            Kind::YtDlp => ("YouTube download", "YouTube downloads"),
            Kind::Files => ("file", "files"),
        };
        format!("{count} {}", if count == 1 { one } else { many })
    }
}

/// Where a download is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Queued,
    Running,
    /// Its queue is paused.
    Paused,
    /// It waits for battery saver to end.
    Held,
    Done,
    Failed,
}

impl Phase {
    pub fn active(self) -> bool {
        matches!(
            self,
            Phase::Queued | Phase::Running | Phase::Paused | Phase::Held
        )
    }

    fn moving(self) -> bool {
        self == Phase::Running
    }
}

/// What a download's progress counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Bytes,
    Pages,
    /// Pieces of a streamed video.
    Parts,
    /// Chapters fetched and written, for exports.
    Chapters,
    /// A share of the whole (yt-dlp's percent), as 0 to 1.
    Fraction,
    /// Steps of a batch the title already counts ("3 of 12").
    Steps,
}

/// What can be done to a download from its notification.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Abilities {
    /// Its queue pauses (and resumes).
    pub pause: bool,
    pub cancel: bool,
    /// A failed one can be tried again.
    pub retry: bool,
}

/// One download, from whichever service keeps it.
#[derive(Debug, Clone, PartialEq)]
pub struct Download {
    pub kind: Kind,
    pub id: String,
    pub title: String,
    /// The artist, the chapter, the podcast…
    pub subtitle: Option<String>,
    pub phase: Phase,
    pub done: f64,
    pub total: Option<f64>,
    pub unit: Unit,
    /// Bytes so far, for the speed, when progress counts something else (a video's parts).
    pub bytes: Option<f64>,
    /// Time left that's known outright (a recording ends with its song, or the service says),
    /// ms.
    pub left_ms: Option<f64>,
    /// Bytes a second as the service measured them.
    pub speed: Option<f64>,
    pub error: Option<String>,
    pub abilities: Abilities,
    pub route: String,
}

impl Download {
    pub fn key(&self) -> String {
        key(self.kind, &self.id)
    }

    /// This download with a title to show: its own, or what its kind is called.
    fn named(&self) -> Download {
        let mut named = self.clone();
        if named.title.trim().is_empty() {
            named.title = self.kind.noun().to_owned();
        }
        named
    }

    fn fraction(&self) -> Option<f64> {
        match (self.unit, self.total) {
            (Unit::Fraction, _) => Some(self.done.clamp(0.0, 1.0)),
            (_, Some(total)) if total > 0.0 => Some((self.done / total).clamp(0.0, 1.0)),
            _ => None,
        }
    }
}

pub fn key(kind: Kind, id: &str) -> String {
    format!("{}:{id}", kind.tag())
}

/// A button on a notification. `keys` are the downloads it acts on.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    /// `pause`, `resume`, `cancel`, `retry` or `stop`.
    pub id: &'static str,
    pub label: &'static str,
    pub keys: Vec<String>,
}

/// One notification's content.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shown {
    pub key: String,
    pub title: String,
    pub text: String,
    /// Speed and time left (the notification's header line).
    pub info: Option<String>,
    /// 0 to 1; none while it can't be told.
    pub progress: Option<f64>,
    /// Running or waiting: it can't be swiped away.
    pub ongoing: bool,
    pub actions: Vec<Action>,
    /// The screen tapping it opens.
    pub route: String,
}

/// The notification at a batch's end.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finished {
    /// The batch's number: a new batch's notice replaces the last one.
    pub batch: u64,
    pub title: String,
    pub text: String,
    pub failed: bool,
    pub actions: Vec<Action>,
    pub route: String,
    /// The download's key when the batch was one that finished, for an Open button where its
    /// kind has a file to open (a file from a website). Not sent as is.
    #[serde(skip)]
    pub lone: Option<String>,
}

/// Everything the notifications show now.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    /// Stands for every download (a lone one's own notification); the foreground one while
    /// anything downloads.
    pub summary: Option<Shown>,
    /// One per download when several show, grouped under the summary.
    pub items: Vec<Shown>,
    /// Something downloads or waits to: Raygun keeps running in the background.
    pub foreground: bool,
}

/// What [`Tracker::update`] found.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Update {
    pub plan: Plan,
    /// A batch just ended with this to say.
    pub finished: Option<Finished>,
    /// Downloads that started since the last update (not ones already running at the first).
    pub started: Vec<String>,
}

impl Shown {
    /// A line left blank says something general instead.
    fn fill_blanks(&mut self) {
        if self.title.trim().is_empty() {
            self.title = "Downloads".into();
        }
        if self.text.trim().is_empty() {
            self.text = if self.ongoing {
                "Downloading"
            } else {
                "In Downloads"
            }
            .into();
        }
    }
}

impl Update {
    /// No notification goes out with a blank title or text (Android shows it empty).
    fn fill_blanks(&mut self) {
        for shown in self
            .plan
            .summary
            .iter_mut()
            .chain(self.plan.items.iter_mut())
        {
            shown.fill_blanks();
        }
        if let Some(finished) = &mut self.finished {
            if finished.title.trim().is_empty() {
                finished.title = "Downloads finished".into();
            }
            if finished.text.trim().is_empty() {
                finished.text = "See them in Downloads".into();
            }
        }
    }
}

/// A plan that hasn't changed is sent again this often while anything shows: the notifications
/// time out on their own unless refreshed, so a Raygun that was killed leaves none behind, ms.
pub const RESEND_MS: f64 = 60_000.0;

/// Whether `update` goes to the notifications, given the last plan sent and when (`None` before
/// the first): the first turn always (it clears what a killed run left on screen), then every
/// change, a batch's end, and a refresh while anything shows.
pub fn should_send(last: Option<(f64, &Plan)>, now_ms: f64, update: &Update) -> bool {
    let Some((sent_at, sent)) = last else {
        return true;
    };
    update.plan != *sent
        || update.finished.is_some()
        || (update.plan.summary.is_some() && now_ms - sent_at >= RESEND_MS)
}

/// A download's speed, learned from its progress over time.
#[derive(Debug, Clone, Copy)]
struct Rate {
    at_ms: f64,
    done: f64,
    bytes: Option<f64>,
    /// Units a second, smoothed.
    units: Option<f64>,
    /// Bytes a second, smoothed.
    bytes_per_s: Option<f64>,
    /// When it last moved, ms.
    moved_at: f64,
}

/// Samples closer together than this don't update a speed, ms.
const MIN_SAMPLE_MS: f64 = 800.0;
/// How much a new sample counts in the smoothed speed.
const SMOOTHING: f64 = 0.3;
/// A download that hasn't moved for this long doesn't keep Raygun running, ms.
const STALLED_MS: f64 = 10.0 * 60.0 * 1000.0;
/// Downloads that only wait (none has run for this long) don't keep Raygun running or hold an
/// ongoing notification: their queue is stuck (no network, a source that never answers), ms.
const IDLE_MS: f64 = 5.0 * 60.0 * 1000.0;
/// Times left past this aren't worth saying, ms.
const LONGEST_LEFT_MS: f64 = 24.0 * 60.0 * 60.0 * 1000.0;

/// Follows the downloads from one update to the next.
#[derive(Debug, Default)]
pub struct Tracker {
    rates: HashMap<String, Rate>,
    /// The downloads of the batch running now, as last seen.
    batch: BTreeMap<String, Download>,
    batch_number: u64,
    /// Keys seen active at the last update; `None` before the first.
    active: Option<Vec<String>>,
    /// Since when downloads only wait, with none running, ms.
    idle_since: Option<f64>,
}

impl Tracker {
    /// Whether `key` belongs to the batch running now (its end is still news).
    pub fn watching(&self, key: &str) -> bool {
        self.batch.contains_key(key)
    }

    /// Takes every download as the services report them now.
    pub fn update(&mut self, now_ms: f64, downloads: &[Download]) -> Update {
        // Android shows a notification without a title or text as an empty one: every download
        // is named (by its kind when it has no title), and every line says something.
        let named: Vec<Download> = downloads.iter().map(Download::named).collect();
        let mut update = self.update_named(now_ms, &named);
        update.fill_blanks();
        update
    }

    fn update_named(&mut self, now_ms: f64, downloads: &[Download]) -> Update {
        let started = self.started(downloads);
        for download in downloads {
            self.learn(now_ms, download);
        }
        let mut finished = None;
        let active: Vec<&Download> = downloads.iter().filter(|d| d.phase.active()).collect();
        if active.is_empty() || active.iter().any(|d| d.phase.moving()) {
            self.idle_since = None;
        } else if self.idle_since.is_none() {
            self.idle_since = Some(now_ms);
        }
        if active.is_empty() {
            if !self.batch.is_empty() {
                // The batch's own downloads as they ended. One that left the lists while it was
                // still going was removed, and isn't news.
                let ended: Vec<Download> = std::mem::take(&mut self.batch)
                    .into_values()
                    .filter_map(|last| {
                        let now = downloads
                            .iter()
                            .find(|download| download.key() == last.key())
                            .cloned()
                            .unwrap_or(last);
                        (!now.phase.active()).then_some(now)
                    })
                    .collect();
                finished = finish(self.batch_number, &ended);
            }
            self.rates.clear();
        } else {
            if self.batch.is_empty() {
                self.batch_number += 1;
            }
            for download in &active {
                self.batch.insert(download.key(), (*download).clone());
            }
            // Ones of this batch that ended meanwhile keep their last state.
            for download in downloads.iter().filter(|d| !d.phase.active()) {
                if let Some(seen) = self.batch.get_mut(&download.key()) {
                    *seen = download.clone();
                }
            }
        }
        let plan = self.plan(now_ms, &active);
        Update {
            plan,
            finished,
            started,
        }
    }

    fn started(&mut self, downloads: &[Download]) -> Vec<String> {
        let now: Vec<String> = downloads
            .iter()
            .filter(|download| download.phase.active())
            .map(Download::key)
            .collect();
        let started = match &self.active {
            Some(before) => now
                .iter()
                .filter(|key| !before.contains(key))
                .cloned()
                .collect(),
            None => Vec::new(),
        };
        self.active = Some(now);
        started
    }

    fn learn(&mut self, now_ms: f64, download: &Download) {
        let key = download.key();
        if !download.phase.moving() {
            self.rates.remove(&key);
            return;
        }
        let Some(rate) = self.rates.get_mut(&key) else {
            self.rates.insert(
                key,
                Rate {
                    at_ms: now_ms,
                    done: download.done,
                    bytes: download.bytes,
                    units: None,
                    bytes_per_s: None,
                    moved_at: now_ms,
                },
            );
            return;
        };
        let elapsed = now_ms - rate.at_ms;
        if elapsed < MIN_SAMPLE_MS {
            return;
        }
        let seconds = elapsed / 1000.0;
        let units = download.done - rate.done;
        if units < 0.0 {
            // It started again from the beginning.
            *rate = Rate {
                at_ms: now_ms,
                done: download.done,
                bytes: download.bytes,
                units: None,
                bytes_per_s: None,
                moved_at: now_ms,
            };
            return;
        }
        let bytes = match (download.bytes, rate.bytes) {
            (Some(now), Some(before)) if now >= before => Some(now - before),
            _ => None,
        };
        if units > 0.0 || bytes.is_some_and(|bytes| bytes > 0.0) {
            rate.moved_at = now_ms;
        }
        rate.units = Some(smooth(rate.units, units / seconds));
        if let Some(bytes) = bytes {
            rate.bytes_per_s = Some(smooth(rate.bytes_per_s, bytes / seconds));
        }
        rate.at_ms = now_ms;
        rate.done = download.done;
        rate.bytes = download.bytes;
    }

    /// Bytes a second, when known.
    fn speed(&self, download: &Download) -> Option<f64> {
        if let Some(speed) = download.speed.filter(|speed| *speed > 0.0) {
            return Some(speed);
        }
        let rate = self.rates.get(&download.key())?;
        if download.bytes.is_some() {
            rate.bytes_per_s
        } else if download.unit == Unit::Bytes {
            rate.units
        } else {
            None
        }
        .filter(|speed| *speed > 0.0)
    }

    /// Time left, ms.
    fn left(&self, download: &Download) -> Option<f64> {
        if let Some(left) = download.left_ms {
            return Some(left.max(0.0));
        }
        let rate = self.rates.get(&download.key())?.units?;
        if rate <= 0.0 {
            return None;
        }
        let total = match download.unit {
            Unit::Fraction => 1.0,
            _ => download.total?,
        };
        let left = (total - download.done).max(0.0) / rate * 1000.0;
        (left <= LONGEST_LEFT_MS).then_some(left)
    }

    fn stalled(&self, now_ms: f64, download: &Download) -> bool {
        self.rates
            .get(&download.key())
            .is_some_and(|rate| now_ms - rate.moved_at > STALLED_MS)
    }

    fn plan(&self, now_ms: f64, active: &[&Download]) -> Plan {
        if active.is_empty() {
            return Plan::default();
        }
        let moving: Vec<&&Download> = active.iter().filter(|d| d.phase.moving()).collect();
        let waiting = active.iter().filter(|d| d.phase == Phase::Queued).count();
        let mut shown: Vec<Shown> = moving.iter().map(|d| self.shown(d)).collect();
        // A paused queue is one notification, whatever it holds.
        let mut paused_kinds: Vec<Kind> = active
            .iter()
            .filter(|d| d.phase == Phase::Paused)
            .map(|d| d.kind)
            .collect();
        paused_kinds.sort();
        paused_kinds.dedup();
        for kind in paused_kinds {
            let held: Vec<&&Download> = active
                .iter()
                .filter(|d| d.kind == kind && d.phase == Phase::Paused)
                .collect();
            shown.push(Shown {
                key: format!("{}:paused", kind.tag()),
                title: format!("{} paused", capitalized(&kind.count(held.len()))),
                text: held
                    .iter()
                    .map(|d| d.title.as_str())
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(", "),
                info: None,
                progress: None,
                ongoing: false,
                actions: vec![Action {
                    id: "resume",
                    label: "Resume",
                    keys: held.iter().map(|d| d.key()).collect(),
                }],
                route: kind.route().to_owned(),
            });
        }
        // Ones held by battery saver: one notification, without a bar that would seem to load.
        let held: Vec<&Download> = active
            .iter()
            .copied()
            .filter(|d| d.phase == Phase::Held)
            .collect();
        if !held.is_empty() {
            let kinds = kinds_of(held.iter().copied());
            shown.push(Shown {
                key: "held".into(),
                title: "Waiting for battery saver to end".into(),
                text: kinds_text(held.iter().copied()),
                info: None,
                progress: None,
                ongoing: false,
                actions: Vec::new(),
                route: route_for(&kinds),
            });
        }
        let stuck = self
            .idle_since
            .is_some_and(|since| now_ms - since > IDLE_MS);
        let foreground = active.iter().any(|d| {
            (d.phase == Phase::Queued && !stuck) || (d.phase.moving() && !self.stalled(now_ms, d))
        });
        if shown.is_empty() {
            // Between two downloads of a queue: only waiting ones.
            let kinds = kinds_of(active.iter().copied());
            return Plan {
                summary: Some(Shown {
                    key: "summary".into(),
                    title: format!("{waiting} waiting to download"),
                    text: kinds_text(active.iter().copied()),
                    info: None,
                    progress: None,
                    // A queue that hasn't moved for long can be swiped away.
                    ongoing: !stuck,
                    actions: Vec::new(),
                    route: route_for(&kinds),
                }),
                items: Vec::new(),
                foreground,
            };
        }
        if shown.len() == 1 {
            let mut only = shown.remove(0);
            if waiting > 0 {
                only.info = Some(join([only.info.take(), Some(format!("{waiting} waiting"))]));
            }
            only.key = "summary".into();
            return Plan {
                summary: Some(only),
                items: Vec::new(),
                foreground,
            };
        }
        let lefts: Vec<Option<f64>> = moving.iter().map(|d| self.left(d)).collect();
        let longest = if waiting == 0 && lefts.iter().all(Option::is_some) {
            lefts.iter().flatten().copied().reduce(f64::max)
        } else {
            None
        };
        let fractions: Vec<f64> = moving.iter().filter_map(|d| d.fraction()).collect();
        let progress = (fractions.len() == moving.len() && !fractions.is_empty())
            .then(|| fractions.iter().sum::<f64>() / fractions.len() as f64);
        let kinds = kinds_of(active.iter().copied());
        if moving.is_empty() {
            // Paused and held queues only: nothing loads, nothing keeps Raygun running.
            return Plan {
                summary: Some(Shown {
                    key: "summary".into(),
                    title: format!("{} downloads waiting", active.len()),
                    text: kinds_text(active.iter().copied()),
                    info: None,
                    progress: None,
                    ongoing: false,
                    actions: Vec::new(),
                    route: route_for(&kinds),
                }),
                items: shown,
                foreground,
            };
        }
        Plan {
            summary: Some(Shown {
                key: "summary".into(),
                title: format!("{} downloading", moving.len()),
                text: kinds_text(moving.iter().map(|d| **d)),
                info: Some(join([
                    longest.map(left_text),
                    (waiting > 0).then(|| format!("{waiting} waiting")),
                ]))
                .filter(|info| !info.is_empty()),
                progress,
                ongoing: true,
                actions: Vec::new(),
                route: route_for(&kinds),
            }),
            items: shown,
            foreground,
        }
    }

    fn shown(&self, download: &Download) -> Shown {
        let speed = self.speed(download);
        let left = self.left(download);
        let amount = amount_text(download);
        let mut text = join([download.subtitle.clone(), amount]);
        if text.is_empty() {
            text = download.kind.doing().to_owned();
        }
        let info = Some(join([speed.map(speed_text), left.map(left_text)]))
            .filter(|info| !info.is_empty());
        let key = download.key();
        let mut actions = Vec::new();
        if download.abilities.pause {
            actions.push(Action {
                id: "pause",
                label: "Pause",
                keys: vec![key.clone()],
            });
        }
        if download.abilities.cancel {
            actions.push(Action {
                id: "cancel",
                label: "Cancel",
                keys: vec![key.clone()],
            });
        }
        Shown {
            key,
            title: download.title.clone(),
            text,
            info,
            progress: download.fraction(),
            ongoing: true,
            actions,
            route: download.route.clone(),
        }
    }
}

fn smooth(before: Option<f64>, now: f64) -> f64 {
    match before {
        Some(before) => before * (1.0 - SMOOTHING) + now * SMOOTHING,
        None => now,
    }
}

/// The notice at a batch's end: what was saved and what wasn't. Nothing when every download of
/// the batch was removed.
fn finish(batch: u64, ended: &[Download]) -> Option<Finished> {
    // Extension installs end with their own summary ("11 installed · 1 failed"), as their title
    // and text; with other downloads, it follows theirs.
    let (installs, ended): (Vec<&Download>, Vec<&Download>) =
        ended.iter().partition(|d| d.kind == Kind::Extensions);
    let installs = installs.first().filter(|d| !d.title.is_empty());
    let mut finished = finish_downloads(batch, &ended);
    match (&mut finished, installs) {
        (Some(finished), Some(installs)) => {
            finished.text = join([
                Some(finished.text.clone()),
                installs
                    .subtitle
                    .clone()
                    .or_else(|| Some(installs.title.clone())),
            ]);
            finished.failed |= installs.phase == Phase::Failed;
            if installs.abilities.retry {
                match finished.actions.first_mut() {
                    Some(retry) => retry.keys.push(installs.key()),
                    None => finished.actions.push(Action {
                        id: "retry",
                        label: "Retry",
                        keys: vec![installs.key()],
                    }),
                }
            }
        }
        (None, Some(installs)) => {
            let failed = installs.phase == Phase::Failed;
            finished = Some(Finished {
                batch,
                title: installs.title.clone(),
                text: installs
                    .subtitle
                    .clone()
                    .filter(|text| !text.trim().is_empty())
                    .unwrap_or_else(|| {
                        if failed {
                            "Try it again from Extensions"
                        } else {
                            "Ready to use"
                        }
                        .to_owned()
                    }),
                failed,
                actions: if installs.abilities.retry {
                    vec![Action {
                        id: "retry",
                        label: "Retry",
                        keys: vec![installs.key()],
                    }]
                } else {
                    Vec::new()
                },
                route: installs.route.clone(),
                lone: None,
            });
        }
        _ => {}
    }
    finished
}

fn finish_downloads(batch: u64, ended: &[&Download]) -> Option<Finished> {
    // A downloaded update opens its installer, which says enough.
    let done: Vec<&Download> = ended
        .iter()
        .copied()
        .filter(|d| d.phase == Phase::Done && d.kind != Kind::Update)
        .collect();
    let failed: Vec<&Download> = ended
        .iter()
        .copied()
        .filter(|d| d.phase == Phase::Failed)
        .collect();
    if done.is_empty() && failed.is_empty() {
        return None;
    }
    let kinds = kinds_of(done.iter().chain(failed.iter()).copied());
    let route = match (done.len(), failed.len()) {
        // A lone download opens where it's listed (a podcast's page, say).
        (1, 0) => done[0].route.clone(),
        (0, 1) => failed[0].route.clone(),
        _ => route_for(&kinds),
    };
    let retry: Vec<String> = failed
        .iter()
        .filter(|d| d.abilities.retry)
        .map(|d| d.key())
        .collect();
    let actions = if retry.is_empty() {
        Vec::new()
    } else {
        vec![Action {
            id: "retry",
            label: "Retry",
            keys: retry,
        }]
    };
    let (title, text) = match (done.len(), failed.len()) {
        (1, 0) => (done[0].title.clone(), done_word(done[0].kind).to_owned()),
        (count, 0) => (
            format!("{count} downloads finished"),
            kinds_text(done.iter().copied()),
        ),
        (0, 1) => (
            format!("{} didn't download", failed[0].title),
            failed[0]
                .error
                .clone()
                .unwrap_or_else(|| "Try it again from Downloads".into()),
        ),
        (0, count) => (format!("{count} downloads didn't finish"), titles(&failed)),
        (saved, count) => (
            format!("{saved} downloaded, {count} didn't"),
            format!("Not downloaded: {}", titles(&failed)),
        ),
    };
    let lone = (done.len() == 1 && failed.is_empty()).then(|| done[0].key());
    Some(Finished {
        batch,
        title,
        text,
        failed: !failed.is_empty(),
        actions,
        route,
        lone,
    })
}

fn done_word(kind: Kind) -> &'static str {
    match kind {
        Kind::Exports => "Exported",
        Kind::Models => "Installed",
        _ => "Downloaded",
    }
}

fn titles(downloads: &[&Download]) -> String {
    let names: Vec<&str> = downloads.iter().map(|d| d.title.as_str()).take(3).collect();
    let more = downloads.len().saturating_sub(names.len());
    let mut text = names.join(", ");
    if more > 0 {
        text.push_str(&format!(" and {more} more"));
    }
    text
}

fn kinds_of<'a>(downloads: impl Iterator<Item = &'a Download>) -> Vec<Kind> {
    let mut kinds: Vec<Kind> = downloads.map(|d| d.kind).collect();
    kinds.sort();
    kinds.dedup();
    kinds
}

/// "2 songs, 1 video".
fn kinds_text<'a>(downloads: impl Iterator<Item = &'a Download>) -> String {
    let mut counts: BTreeMap<Kind, usize> = BTreeMap::new();
    for download in downloads {
        *counts.entry(download.kind).or_default() += 1;
    }
    counts
        .into_iter()
        .map(|(kind, count)| kind.count(count))
        .collect::<Vec<_>>()
        .join(", ")
}

fn route_for(kinds: &[Kind]) -> String {
    match kinds {
        [kind] => kind.route().to_owned(),
        _ => "/downloads".to_owned(),
    }
}

/// "14 of 30 pages", "2.1 of 5.0 MB", "Waiting".
fn amount_text(download: &Download) -> Option<String> {
    if download.phase == Phase::Queued {
        return Some("Waiting".into());
    }
    let count = |what: &str| match download.total {
        Some(total) if total > 0.0 => Some(format!(
            "{} of {} {what}",
            download.done.round() as u64,
            total.round() as u64
        )),
        _ if download.done > 0.0 => Some(format!("{} {what}", download.done.round() as u64)),
        _ => None,
    };
    match download.unit {
        Unit::Bytes => match download.total {
            Some(total) if total > 0.0 => Some(format!(
                "{} of {}",
                size_number(download.done, total),
                size_text(total)
            )),
            _ if download.done > 0.0 => Some(size_text(download.done)),
            _ => None,
        },
        Unit::Pages => count("pages"),
        Unit::Parts => match (download.total, download.bytes) {
            (_, Some(bytes)) if bytes > 0.0 => Some(match download.fraction() {
                Some(fraction) => format!("{} · {}%", size_text(bytes), percent(fraction)),
                None => size_text(bytes),
            }),
            _ => download
                .fraction()
                .map(|fraction| format!("{}%", percent(fraction))),
        },
        Unit::Chapters => count("chapters"),
        Unit::Fraction => Some(format!("{}%", percent(download.done))),
        Unit::Steps => None,
    }
}

fn percent(fraction: f64) -> u32 {
    (fraction.clamp(0.0, 1.0) * 100.0).floor() as u32
}

fn join<const N: usize>(parts: [Option<String>; N]) -> String {
    parts
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];

/// A size the way Android's file sizes read: 1000-based, one decimal under 10.
pub fn size_text(bytes: f64) -> String {
    let (value, unit) = scaled(bytes, bytes);
    format!("{value} {unit}")
}

/// `done`'s number in `total`'s unit ("2.1" of "5.0 MB").
fn size_number(done: f64, total: f64) -> String {
    scaled(done, total).0
}

fn scaled(bytes: f64, reference: f64) -> (String, &'static str) {
    if reference < 1000.0 {
        return (format!("{}", bytes.max(0.0).round() as u64), "B");
    }
    let mut value = bytes.max(0.0) / 1000.0;
    let mut scale = reference / 1000.0;
    let mut unit = 0;
    while scale >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        scale /= 1000.0;
        unit += 1;
    }
    let number = if scale < 10.0 {
        format!("{value:.1}")
    } else {
        format!("{}", value.round() as u64)
    };
    (number, UNITS[unit])
}

pub fn speed_text(bytes_per_s: f64) -> String {
    format!("{}/s", size_text(bytes_per_s))
}

/// "about 30 s left", "about 2 min left", "about 1 h 5 min left".
pub fn left_text(ms: f64) -> String {
    let seconds = (ms / 1000.0).round() as u64;
    if seconds < 60 {
        // Steps of five seconds keep it from flickering.
        let seconds = (seconds.div_ceil(5) * 5).max(5);
        format!("about {seconds} s left")
    } else if seconds < 3600 {
        format!("about {} min left", seconds.div_ceil(60))
    } else {
        let minutes = seconds.div_ceil(60);
        match minutes % 60 {
            0 => format!("about {} h left", minutes / 60),
            rest => format!("about {} h {rest} min left", minutes / 60),
        }
    }
}

/// The extension install queue's batch as one download: "Installing extensions · 3 of 12" with
/// the one installing now, then its summary ("11 installed · 1 failed"). Nothing before the first
/// batch, or for one cancelled before anything ended.
pub fn extension_installs(status: &raygun_model::ExtensionInstallStatus) -> Option<Download> {
    use raygun_model::{ExtensionInstallPhase as Step, ExtensionInstallState as State};

    if status.batch == 0 || status.total == 0 {
        return None;
    }
    let ended = status.done + status.failed;
    let batch: Vec<&raygun_model::ExtensionInstall> = status
        .items
        .iter()
        .filter(|item| item.batch == status.batch)
        .collect();
    let waiting = batch
        .iter()
        .filter(|item| item.state == State::Queued)
        .count();
    let route = Kind::Extensions.route().to_owned();
    let (phase, title, subtitle) = if status.running {
        let position = (ended + 1).min(status.total);
        let step = match status.phase {
            Some(Step::Downloading) => Some("Downloading"),
            Some(Step::Converting) => Some("Converting"),
            Some(Step::Installing) => Some("Installing"),
            Some(Step::Loading) => None,
            None => None,
        };
        let subtitle = match (&status.current, step, status.phase) {
            (_, _, Some(Step::Loading)) => Some("Loading extensions".to_owned()),
            (Some(name), Some(step), _) => Some(format!("{name} · {step}")),
            (Some(name), None, _) => Some(name.clone()),
            (None, _, _) => None,
        };
        (
            Phase::Running,
            format!("Installing extensions · {position} of {}", status.total),
            subtitle,
        )
    } else {
        if ended == 0 {
            return None;
        }
        let phase = if status.failed > 0 {
            Phase::Failed
        } else {
            Phase::Done
        };
        if status.total == 1 {
            let only = batch.first()?;
            let name = match only.name.trim() {
                "" => "Extension",
                name => name,
            };
            return Some(Download {
                title: match only.state {
                    State::Failed => format!("{name} didn't install"),
                    _ => format!("{name} installed"),
                },
                subtitle: only.error.clone(),
                ..installs_download(status, phase, String::new(), None, route)
            });
        }
        let counts = join([
            (status.done > 0).then(|| format!("{} installed", status.done)),
            (status.failed > 0).then(|| format!("{} failed", status.failed)),
            (status.cancelled > 0).then(|| format!("{} cancelled", status.cancelled)),
        ]);
        let title = if status.done > 0 {
            "Extensions installed"
        } else {
            "Extensions didn't install"
        };
        (phase, title.to_owned(), Some(counts))
    };
    let mut download = installs_download(status, phase, title, subtitle, route);
    download.abilities.cancel = status.running && waiting > 0;
    Some(download)
}

fn installs_download(
    status: &raygun_model::ExtensionInstallStatus,
    phase: Phase,
    title: String,
    subtitle: Option<String>,
    route: String,
) -> Download {
    Download {
        kind: Kind::Extensions,
        id: format!("batch-{}", status.batch),
        title,
        subtitle,
        phase,
        done: f64::from(status.done + status.failed),
        total: Some(f64::from(status.total)),
        unit: Unit::Steps,
        bytes: None,
        left_ms: None,
        speed: None,
        error: None,
        abilities: Abilities {
            pause: false,
            cancel: false,
            retry: phase == Phase::Failed,
        },
        route,
    }
}
