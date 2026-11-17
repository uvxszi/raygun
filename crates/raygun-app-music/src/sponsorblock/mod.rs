//! SponsorBlock (sponsor.ajay.app): the parts of YouTube videos its users marked, such as sponsor
//! reads, reminders to subscribe and a music video's non-music parts, skipped or only marked as
//! Settings say (`SponsorBlockPrefs`). Skipping a part as playback enters it follows NouTube
//! (nonbili/NouTube, AGPL-3.0, `content/sponsorblock.ts` and `content/player.ts`). SponsorBlock's
//! data is CC BY-NC-SA 4.0; Settings names it where the switches are.
//!
//! - A lookup sends the first 4 characters of the video id's SHA-256
//!   (`/api/skipSegments/<prefix>`). The answer lists every video under that prefix, so
//!   SponsorBlock never learns which video plays; only this one's parts are kept.
//! - What's found is kept a day in memory and on disk (`cache/sponsorblock`, files named by the
//!   id's hash; none written while incognito) and swept after 30 days unused. Offline Mode asks
//!   nothing and uses what's kept however old; Data Saver asks only for what plays and keeps using
//!   what it has.
//! - Songs from YouTube (its embed, its audio file, a downloaded copy of either) are skipped by
//!   `PlaybackService` as its reports reach a part ([`LoadSkips`]). YouTube's own players in
//!   Raygun's windows (trailers, YouTube links) are given the parts and skip them themselves.

mod skips;
#[cfg(test)]
mod tests;

pub use skips::{LoadSkips, Skip};

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use raygun_model::{SegmentAction, SegmentCategory, SkipSegment, SponsorBlockPrefs};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// SponsorBlock's API.
pub const API: &str = "https://sponsor.ajay.app";
/// Characters of the video id's hash a lookup sends (SponsorBlock's recommendation).
const PREFIX_LEN: usize = 4;
/// A video's parts are asked for again after a day: new videos get theirs in their first hours.
const FRESH_MS: i64 = 24 * 60 * 60 * 1000;
/// Kept parts not asked for again in this long go.
const KEPT_MS: u64 = 30 * 24 * 60 * 60 * 1000;
/// Kept videos at most; the oldest go first.
const MAX_FILES: usize = 5_000;
/// Videos whose parts are held in memory at most.
const IN_MEMORY: usize = 256;
const TIMEOUT: Duration = Duration::from_secs(8);
/// A part marked on a video this much longer or shorter than the one playing was marked on
/// another cut of it: its times don't fit.
const LENGTH_SLACK_MS: f64 = 2_000.0;
/// What lookups say they are.
const AGENT: &str = concat!("Raygun/", env!("CARGO_PKG_VERSION"));

/// A part of a video, as SponsorBlock lists it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Segment {
    pub id: String,
    pub category: SegmentCategory,
    pub start_ms: f64,
    pub end_ms: f64,
    /// The video's length when the part was marked; 0 when SponsorBlock doesn't say.
    pub video_ms: f64,
}

impl Segment {
    /// Whether it was marked on a video as long as the one playing (`duration_ms`).
    pub fn fits(&self, duration_ms: Option<f64>) -> bool {
        match duration_ms {
            Some(duration) if duration > 0.0 && self.video_ms > 0.0 => {
                (self.video_ms - duration).abs() <= LENGTH_SLACK_MS
            }
            _ => true,
        }
    }
}

/// What a lookup is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Want {
    /// Something plays, or is about to: asked for under Data Saver too.
    Now,
    /// A song found ahead of its turn: not while Data Saver keeps from finding songs ahead.
    Ahead,
}

/// What the network and privacy allow now.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Conditions {
    pub offline: bool,
    /// Data Saver applies.
    pub saving: bool,
    /// Data Saver keeps from finding songs ahead.
    pub no_ahead: bool,
    /// Incognito: nothing goes to disk.
    pub incognito: bool,
}

type ConditionsFn = Box<dyn Fn() -> Conditions + Send + Sync>;

/// One video's parts and when they were asked for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Kept {
    video_id: String,
    fetched_ms: i64,
    segments: Vec<Segment>,
}

pub struct SponsorBlock {
    base: String,
    client: reqwest::Client,
    /// Where parts are kept on disk; none keeps them in memory only.
    dir: Option<PathBuf>,
    memory: Mutex<HashMap<String, Kept>>,
    prefs: Mutex<SponsorBlockPrefs>,
    conditions: ConditionsFn,
}

impl SponsorBlock {
    /// The app's: SponsorBlock's API, parts kept in `dir`, the app's switches.
    pub fn new(
        client: reqwest::Client,
        dir: PathBuf,
        incognito: impl Fn() -> bool + Send + Sync + 'static,
    ) -> Self {
        Self::with(
            API,
            client,
            Some(dir),
            Box::new(move || Conditions {
                offline: raygun_net::network_mode::offline(),
                saving: raygun_net::network_mode::data_saver(),
                no_ahead: crate::network_mode::saving(|saver| saver.no_prefetch),
                incognito: incognito(),
            }),
        )
    }

    fn with(
        base: &str,
        client: reqwest::Client,
        dir: Option<PathBuf>,
        conditions: ConditionsFn,
    ) -> Self {
        Self {
            base: base.trim_end_matches('/').to_owned(),
            client,
            dir,
            memory: Mutex::new(HashMap::new()),
            prefs: Mutex::new(SponsorBlockPrefs::default()),
            conditions,
        }
    }

    pub fn prefs(&self) -> SponsorBlockPrefs {
        *self
            .prefs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn set_prefs(&self, prefs: SponsorBlockPrefs) {
        *self
            .prefs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = prefs;
    }

    /// Whether any part is skipped or marked: else nothing is looked up.
    pub fn enabled(&self) -> bool {
        let prefs = self.prefs();
        prefs.enabled
            && SegmentCategory::ALL
                .iter()
                .any(|category| prefs.action(*category) != SegmentAction::Off)
    }

    /// The parts of `video_id` held in memory, however old: for a load to start past one.
    pub fn kept(&self, video_id: &str) -> Option<Vec<Segment>> {
        self.memory()
            .get(video_id)
            .map(|kept| kept.segments.clone())
    }

    /// The parts of `video_id`: kept ones while fresh, else asked for. What's kept stands in
    /// while asking isn't allowed (Offline Mode, Data Saver) or fails.
    pub async fn segments(&self, video_id: &str, want: Want) -> Result<Vec<Segment>, String> {
        if !is_video_id(video_id) || !self.enabled() {
            return Ok(Vec::new());
        }
        let now = crate::catalog::now_ms();
        let mut known = self.memory().get(video_id).cloned();
        if known.is_none()
            && let Some(read) = self.read(video_id).await
        {
            self.remember(read.clone());
            known = Some(read);
        }
        if let Some(kept) = &known
            && now - kept.fetched_ms < FRESH_MS
        {
            return Ok(kept.segments.clone());
        }
        let conditions = (self.conditions)();
        let stale = known.map(|kept| kept.segments);
        if conditions.offline
            || (conditions.saving && stale.is_some())
            || (want == Want::Ahead && conditions.no_ahead)
        {
            return Ok(stale.unwrap_or_default());
        }
        match self.ask(video_id).await {
            Ok(segments) => {
                let kept = Kept {
                    video_id: video_id.to_owned(),
                    fetched_ms: now,
                    segments: segments.clone(),
                };
                self.remember(kept.clone());
                if !conditions.incognito {
                    self.write(&kept).await;
                }
                Ok(segments)
            }
            Err(error) => match stale {
                Some(segments) => {
                    tracing::debug!(%error, "SponsorBlock didn't answer; its kept parts stand in");
                    Ok(segments)
                }
                None => Err(error),
            },
        }
    }

    /// `segments` as they're handled now: each with what's done with it, those left alone and
    /// those marked on another cut of the video left out, in order.
    pub fn shown(&self, segments: &[Segment], duration_ms: Option<f64>) -> Vec<SkipSegment> {
        shown(&self.prefs(), segments, duration_ms)
    }

    /// Forgets parts not asked for in 30 days (and the oldest past 5000 videos), in the background.
    pub fn sweep_later(&self) {
        if let Some(dir) = self.dir.clone() {
            tauri::async_runtime::spawn_blocking(move || sweep(&dir, MAX_FILES, KEPT_MS));
        }
    }

    async fn ask(&self, video_id: &str) -> Result<Vec<Segment>, String> {
        let prefix = &hash_hex(video_id)[..PREFIX_LEN];
        let categories: Vec<&str> = SegmentCategory::ALL
            .iter()
            .map(|category| category.api_name())
            .collect();
        let categories = serde_json::to_string(&categories).map_err(|error| error.to_string())?;
        let url = url::Url::parse_with_params(
            &format!("{}/api/skipSegments/{prefix}", self.base),
            [
                ("categories", categories.as_str()),
                ("actionTypes", r#"["skip"]"#),
            ],
        )
        .map_err(|error| error.to_string())?;
        let response = self
            .client
            .get(url)
            .header(reqwest::header::USER_AGENT, AGENT)
            .timeout(TIMEOUT)
            .send()
            .await
            .map_err(|error| {
                if raygun_net::network_mode::is_offline_error(&error) {
                    raygun_net::network_mode::OFFLINE_MESSAGE.to_owned()
                } else {
                    format!("SponsorBlock didn't answer ({error})")
                }
            })?;
        match response.status().as_u16() {
            // Nothing under the prefix: no video there has parts.
            404 => Ok(Vec::new()),
            200..=299 => {
                let body = response
                    .text()
                    .await
                    .map_err(|error| format!("SponsorBlock's answer broke off ({error})"))?;
                parse(&body, video_id)
            }
            429 => Err("SponsorBlock is limiting requests; its parts are asked for later".into()),
            status => Err(format!("SponsorBlock answered HTTP {status}")),
        }
    }

    fn memory(&self) -> std::sync::MutexGuard<'_, HashMap<String, Kept>> {
        self.memory
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn remember(&self, kept: Kept) {
        let mut memory = self.memory();
        if memory.len() >= IN_MEMORY
            && !memory.contains_key(&kept.video_id)
            && let Some(oldest) = memory
                .values()
                .min_by_key(|kept| kept.fetched_ms)
                .map(|kept| kept.video_id.clone())
        {
            memory.remove(&oldest);
        }
        memory.insert(kept.video_id.clone(), kept);
    }

    fn file(&self, video_id: &str) -> Option<PathBuf> {
        let hash = hash_hex(video_id);
        self.dir
            .as_ref()
            .map(|dir| dir.join(format!("{}.json", &hash[..32])))
    }

    async fn read(&self, video_id: &str) -> Option<Kept> {
        let bytes = tokio::fs::read(self.file(video_id)?).await.ok()?;
        let kept: Kept = serde_json::from_slice(&bytes).ok()?;
        (kept.video_id == video_id).then_some(kept)
    }

    async fn write(&self, kept: &Kept) {
        let (Some(dir), Some(file)) = (self.dir.as_ref(), self.file(&kept.video_id)) else {
            return;
        };
        let Ok(json) = serde_json::to_vec(kept) else {
            return;
        };
        let written = async {
            tokio::fs::create_dir_all(dir).await?;
            tokio::fs::write(&file, json).await
        }
        .await;
        if let Err(error) = written {
            tracing::debug!(%error, "couldn't keep SponsorBlock's parts");
        }
    }
}

/// [`SponsorBlock::shown`] with `prefs`.
pub fn shown(
    prefs: &SponsorBlockPrefs,
    segments: &[Segment],
    duration_ms: Option<f64>,
) -> Vec<SkipSegment> {
    let mut shown: Vec<SkipSegment> = segments
        .iter()
        .filter(|segment| segment.fits(duration_ms))
        .filter_map(|segment| {
            let action = prefs.action(segment.category);
            (action != SegmentAction::Off).then(|| SkipSegment {
                id: segment.id.clone(),
                category: segment.category,
                start_ms: segment.start_ms,
                end_ms: segment.end_ms,
                action,
            })
        })
        .collect();
    shown.sort_by(|a, b| a.start_ms.total_cmp(&b.start_ms));
    shown
}

/// The SHA-256 of `text`, in lowercase hex.
fn hash_hex(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// A YouTube video id: 11 characters of `[A-Za-z0-9_-]`.
pub fn is_video_id(id: &str) -> bool {
    id.len() == 11
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

#[derive(Deserialize)]
struct Listed {
    #[serde(rename = "videoID")]
    video_id: String,
    #[serde(default)]
    segments: Vec<ListedSegment>,
}

#[derive(Deserialize)]
struct ListedSegment {
    #[serde(default)]
    category: String,
    #[serde(default, rename = "actionType")]
    action_type: String,
    #[serde(default)]
    segment: Vec<f64>,
    #[serde(default, rename = "UUID")]
    uuid: String,
    #[serde(default, rename = "videoDuration")]
    video_duration: f64,
}

/// `video_id`'s skippable parts in an answer listing the videos under its hash's prefix.
fn parse(body: &str, video_id: &str) -> Result<Vec<Segment>, String> {
    let listed: Vec<Listed> = serde_json::from_str(body)
        .map_err(|_| "SponsorBlock answered something Raygun can't read".to_owned())?;
    let mut segments: Vec<Segment> = listed
        .into_iter()
        .filter(|video| video.video_id == video_id)
        .flat_map(|video| video.segments)
        .filter(|listed| listed.action_type.is_empty() || listed.action_type == "skip")
        .filter_map(|listed| {
            let category = SegmentCategory::from_api(&listed.category)?;
            let [start, end] = listed.segment.as_slice() else {
                return None;
            };
            let (start, end) = (start * 1000.0, end * 1000.0);
            if !start.is_finite() || !end.is_finite() || start < 0.0 || end <= start {
                return None;
            }
            let id = if listed.uuid.is_empty() {
                format!("{}-{start}-{end}", category.api_name())
            } else {
                listed.uuid
            };
            Some(Segment {
                id,
                category,
                start_ms: start,
                end_ms: end,
                video_ms: if listed.video_duration.is_finite() {
                    (listed.video_duration * 1000.0).max(0.0)
                } else {
                    0.0
                },
            })
        })
        .collect();
    segments.sort_by(|a, b| a.start_ms.total_cmp(&b.start_ms));
    Ok(segments)
}

/// Removes kept parts older than `max_age_ms`, then the oldest past `max` files.
fn sweep(dir: &Path, max: usize, max_age_ms: u64) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let now = SystemTime::now();
    let mut files: Vec<(SystemTime, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, entry.path()))
        })
        .collect();
    files.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    for (index, (modified, path)) in files.iter().enumerate() {
        let age = now
            .duration_since(*modified)
            .unwrap_or_default()
            .as_millis();
        if index >= max || age > u128::from(max_age_ms) {
            let _ = std::fs::remove_file(path);
        }
    }
}
