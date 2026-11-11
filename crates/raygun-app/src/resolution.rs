//! Per-load bookkeeping for stream resolution (M14): what the current load is trying to play, which
//! candidates it already tried, and what to do when the engine reports success or failure.
//!
//! When what a load has stops working, it looks for another source before the song is skipped:
//! a remembered pick whose stream died is asked for a new stream (the pick itself didn't fail),
//! the other matches and the other sources are tried, and when a search comes up empty the load
//! looks harder a few times ([`RetryPolicy`]), waiting longer each time and longer still after a
//! source limited requests, within a time budget that depends on how the load came about
//! ([`Cause`]). Pure state; `PlaybackService` carries out the plans.

use std::time::Duration;

use raygun_model::{MediaSource, QueueItem, TrackQuery};
use raygun_resolver::{CandidateKey, Pick};

/// What to avoid on the next resolution attempt.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Exclusions {
    pub tried: Vec<CandidateKey>,
    /// Concrete sources that already failed (so a fallback doesn't pick the same stream again).
    pub failed_sources: Vec<MediaSource>,
    /// Skip the cache of verified picks: downloads check each pick's artist, which the cache
    /// doesn't keep (M46.10).
    pub fresh_only: bool,
    /// The quality to aim for; `None` takes the streaming setting (or the mobile data one while
    /// on mobile data). Downloads pass theirs (M46.18).
    pub quality: Option<raygun_model::AudioQuality>,
    /// The item's own YouTube video (M46.51): its audio file is tried first, then its embed,
    /// before other sources are searched.
    pub youtube: Option<String>,
    /// Raygun reads the stream itself (downloads, captions): sources with several formats give
    /// one it decodes (M46.51).
    pub decodable: bool,
    /// Look harder: no remembered pick and no earlier search's answers, matches that failed lately
    /// get a try too, and other sources are searched for the song.
    pub deep: bool,
    /// Candidates whose source couldn't answer just now (it timed out, limited requests, had
    /// server trouble, sits out): left alone until the next look.
    pub unanswered: Vec<CandidateKey>,
}

/// What a search met on its way, besides its answer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Notes {
    /// Candidates that failed for good (no stream, gone): kept out of the rest of the load.
    pub failed: Vec<CandidateKey>,
    /// Candidates whose source had trouble answering now: asked again by the next look.
    pub unanswered: Vec<CandidateKey>,
    /// A source limited requests: the next look waits at least [`LIMITED_WAIT`].
    pub limited: bool,
    /// How long a source asked to be left alone, when it said.
    pub retry_after: Option<Duration>,
    /// A source failed to answer at all: it isn't known that no source has the song.
    pub troubled: bool,
    /// A source that may have the song waits for its key or sign-in and wasn't asked: it isn't
    /// known that no source has the song (it isn't marked), but there's nothing to ask again
    /// until that source is set up.
    pub needs_setup: bool,
}

impl Notes {
    /// Whether some source couldn't answer the search properly.
    fn met_trouble(&self) -> bool {
        self.troubled || self.limited || !self.unanswered.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadPlan {
    /// The item's own source plays as it is.
    Direct,
    /// Find a stream first.
    Resolve,
}

/// How a load came about, which decides how long it may look for another source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    /// The listener asked for this song: played it, pressed Next or Previous, started a queue.
    Chosen,
    /// The queue moved on by itself: the song before ended.
    Advance,
    /// The song before couldn't play either. A run of songs that don't play must neither hold
    /// the queue up long nor keep the sources busy.
    AfterFailure,
}

/// How hard a load looks for another source once a search came up empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Harder looks after the first search.
    pub looks: u32,
    /// The wait before the first of them; each one after waits [`BACKOFF`] times longer.
    pub first_wait: Duration,
    /// Longest a song spends looking, from when what it had first failed.
    pub budget: Duration,
}

/// Each harder look waits this many times longer than the one before.
pub const BACKOFF: u32 = 3;
/// After a source limited requests, the next look waits at least this long.
pub const LIMITED_WAIT: Duration = Duration::from_secs(8);
/// The longest a source's own `Retry-After` is waited for.
pub const LONGEST_WAIT: Duration = Duration::from_secs(20);
/// Streams given to the engine for one load at most: a run of streams that fail one after the
/// other (each may take the start timeout) ends there.
pub const MOST_STREAMS: u32 = 8;

impl RetryPolicy {
    /// The policy for a load that came about by `cause`; `marked` says the song is marked as one
    /// no source had: only a song asked for looks harder for it.
    pub fn of(cause: Cause, marked: bool) -> Self {
        let seconds = Duration::from_secs;
        match (cause, marked) {
            (Cause::Chosen, _) => Self {
                looks: 3,
                first_wait: seconds(1),
                budget: seconds(45),
            },
            (Cause::Advance, false) => Self {
                looks: 2,
                first_wait: seconds(1),
                budget: seconds(25),
            },
            (Cause::AfterFailure, false) => Self {
                looks: 1,
                first_wait: seconds(1),
                budget: seconds(12),
            },
            (_, true) => Self {
                looks: 0,
                first_wait: Duration::ZERO,
                budget: seconds(12),
            },
        }
    }

    /// The wait before harder look `look` (1 is the first), `elapsed` after what the load had
    /// first failed. A source that limited requests makes it at least [`LIMITED_WAIT`], one that
    /// said how long to wait at least that (up to [`LONGEST_WAIT`]). `None` when no look is left
    /// or the look couldn't start within the budget.
    pub fn wait(&self, look: u32, elapsed: Duration, notes: &Notes) -> Option<Duration> {
        if look == 0 || look > self.looks {
            return None;
        }
        let mut wait = self.first_wait * BACKOFF.pow(look - 1);
        if notes.limited {
            wait = wait.max(LIMITED_WAIT);
        }
        if let Some(asked) = notes.retry_after {
            wait = wait.max(asked.min(LONGEST_WAIT));
        }
        (elapsed + wait < self.budget).then_some(wait)
    }

    /// Whether a load whose first failure was `elapsed` ago may still try another stream.
    pub fn within(&self, elapsed: Duration) -> bool {
        elapsed < self.budget
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorPlan {
    /// Let the playback core handle the error (it skips to the next item).
    Forward,
    /// Try another stream for the same load.
    Retry { failed: Failed },
    /// The load looked for another source as long as it may: the core skips the song. With
    /// `mark` it's marked as one no source plays (every source answered, none played it);
    /// `failed` is what failed last.
    GiveUp { mark: bool, failed: Option<Failed> },
}

/// What just failed for a load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failed {
    /// A stream the resolver found now: kept out of this load, and out for a while after.
    Pick(Box<Pick>),
    /// A remembered pick's stream: the stream is forgotten and the pick's source is asked for a
    /// new one, since it's the stream that stopped working, not the match.
    Stale(Box<Pick>),
    /// The song's own source (a stream picked for it, which it carries).
    Own(MediaSource),
    /// The song's downloaded copy: its stream plays instead, and the download gets checked.
    Download,
}

/// What to do after a search came up empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unresolved {
    /// Look again, harder, after `wait`.
    Retry { wait: Duration },
    /// Nothing more to try: the song is skipped, and with `mark` marked as one no source plays
    /// (every source answered, none had it).
    GiveUp { mark: bool },
}

/// A stream the engine started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Played {
    /// The resolver's pick, to remember as one that plays.
    pub pick: Pick,
    /// The load had looked for another source: the song plays again, and its marks go.
    pub rescued: bool,
    /// The song's own source that stopped working, which the found stream replaces.
    pub replaced: Option<MediaSource>,
}

#[derive(Debug, Clone)]
struct Attempt {
    load_id: u32,
    query: TrackQuery,
    start_ms: f64,
    exclusions: Exclusions,
    /// The resolver's pick now loaded in the engine.
    playing: Option<Pick>,
    /// The item's own source, when it has one that a fallback could replace.
    concrete: Option<MediaSource>,
    /// The engine was given the song's downloaded copy (this file).
    download: Option<MediaSource>,
    /// The engine was given the item's own source (or its download) and it hasn't failed.
    direct: bool,
    fell_back: bool,
    verified: bool,
    cause: Cause,
    /// The song is marked as one no source had.
    marked: bool,
    /// When (the service's clock, ms) what the load had first failed: it looks for another source
    /// from then until a stream plays.
    failing_since: Option<f64>,
    /// Harder looks so far.
    looks: u32,
    /// Streams given to the engine since the first failure.
    streams: u32,
    /// The song's own source failed.
    own_failed: Option<MediaSource>,
    /// The last search met sources that couldn't answer.
    troubled: bool,
}

impl Attempt {
    fn policy(&self) -> RetryPolicy {
        RetryPolicy::of(self.cause, self.marked)
    }

    fn take_notes(&mut self, notes: &Notes) {
        // A source waiting for its key keeps the song from being marked as one no source has.
        self.troubled = notes.met_trouble() || notes.needs_setup;
        let exclusions = &mut self.exclusions;
        for key in &notes.failed {
            if !exclusions.tried.contains(key) {
                exclusions.tried.push(key.clone());
            }
        }
        for key in &notes.unanswered {
            if !exclusions.unanswered.contains(key) {
                exclusions.unanswered.push(key.clone());
            }
        }
    }

    /// How long ago what it had first failed (from now, if this is the first failure).
    fn failing_for(&mut self, now_ms: f64) -> Duration {
        let since = *self.failing_since.get_or_insert(now_ms);
        Duration::from_secs_f64(((now_ms - since) / 1_000.0).max(0.0))
    }
}

#[derive(Debug, Default)]
pub struct Tracker {
    current: Option<Attempt>,
    /// A YouTube song's video is shown (M46.51): YouTube songs play in the embed, which has it.
    video: bool,
}

impl Tracker {
    /// Shows or hides YouTube songs' videos; whether that changed.
    pub fn set_video(&mut self, shown: bool) -> bool {
        std::mem::replace(&mut self.video, shown) != shown
    }

    /// The stream `load_id` plays now, when the resolver picked it.
    pub fn playing(&self, load_id: u32) -> Option<&MediaSource> {
        self.pick(load_id).map(|pick| &pick.source)
    }

    /// The resolver's pick `load_id` plays now, if it plays one.
    pub fn pick(&self, load_id: u32) -> Option<&Pick> {
        self.current
            .as_ref()
            .filter(|attempt| attempt.load_id == load_id)
            .and_then(|attempt| attempt.playing.as_ref())
    }

    /// How a load of `item` would play: as it is, or after finding a stream for this query with
    /// these exclusions (YouTube songs try their own video's audio file first, M46.51).
    fn plan_for(&self, item: &QueueItem) -> (LoadPlan, Option<MediaSource>, Exclusions) {
        let mut exclusions = Exclusions::default();
        let (plan, concrete) = match &item.source {
            MediaSource::Lookup { .. } => (LoadPlan::Resolve, None),
            // A file on this device has no alternative worth searching for.
            MediaSource::LocalFile { .. } => (LoadPlan::Direct, None),
            // YouTube's own audio file plays in the native player (M46.51); the resolver tries it
            // first, then the embed, then other sources. With its video shown, the embed plays.
            MediaSource::YouTube { video_id } if !self.video => {
                exclusions.youtube = Some(video_id.clone());
                (LoadPlan::Resolve, None)
            }
            source => (LoadPlan::Direct, Some(source.clone())),
        };
        (plan, concrete, exclusions)
    }

    /// What a load of `item` would search for, when it would (M46.53: the songs around the one
    /// playing have their streams found ahead); `None` for one that plays as it is.
    pub fn search_for(&self, item: &QueueItem) -> Option<(TrackQuery, Exclusions)> {
        match self.plan_for(item) {
            (LoadPlan::Resolve, _, exclusions) => Some((TrackQuery::for_item(item), exclusions)),
            (LoadPlan::Direct, _, _) => None,
        }
    }

    /// Whether the engine holds a stream for `load_id`: false while a stream is being found, when
    /// nothing could start or fail to start in time.
    pub fn loaded(&self, load_id: u32) -> bool {
        self.current
            .as_ref()
            .filter(|attempt| attempt.load_id == load_id)
            .is_some_and(|attempt| attempt.playing.is_some() || attempt.direct)
    }

    /// How load `load_id` came about.
    pub fn cause(&self, load_id: u32) -> Option<Cause> {
        self.current
            .as_ref()
            .filter(|attempt| attempt.load_id == load_id)
            .map(|attempt| attempt.cause)
    }

    /// Starts tracking a new load.
    pub fn begin(
        &mut self,
        load_id: u32,
        item: &QueueItem,
        start_ms: f64,
        cause: Cause,
    ) -> LoadPlan {
        let (plan, concrete, exclusions) = self.plan_for(item);
        self.current = Some(Attempt {
            load_id,
            query: TrackQuery::for_item(item),
            start_ms,
            exclusions,
            playing: None,
            concrete,
            download: None,
            direct: plan == LoadPlan::Direct,
            fell_back: false,
            verified: false,
            cause,
            marked: false,
            failing_since: None,
            looks: 0,
            streams: 0,
            own_failed: None,
            troubled: false,
        });
        plan
    }

    /// Starts tracking a load whose song plays from its downloaded copy, `file` (M46.10): when the
    /// copy doesn't play (gone, damaged), a stream is found for the song (`item`, as queued).
    pub fn begin_download(
        &mut self,
        load_id: u32,
        item: &QueueItem,
        file: MediaSource,
        start_ms: f64,
        cause: Cause,
    ) -> LoadPlan {
        self.begin(load_id, item, start_ms, cause);
        if let Some(attempt) = self.attempt(load_id) {
            // The song's own source plays only through the resolver now: a failed copy looks
            // for its stream as a lookup does.
            attempt.concrete = None;
            attempt.exclusions.youtube = None;
            attempt.download = Some(file);
            attempt.direct = true;
        }
        LoadPlan::Direct
    }

    fn attempt(&mut self, load_id: u32) -> Option<&mut Attempt> {
        self.current
            .as_mut()
            .filter(|attempt| attempt.load_id == load_id)
    }

    /// The webview holding `load_id`'s stream went (Raygun's window, with YouTube's player in it):
    /// what to search for another stream for it. The video's embed is out of the search, and the
    /// stream found plays from where the song was.
    pub fn without_webview(
        &mut self,
        load_id: u32,
        youtube_streams: &str,
    ) -> Option<(TrackQuery, Exclusions)> {
        let attempt = self.attempt(load_id)?;
        attempt.fell_back = true;
        let embed = match attempt.playing.as_ref().map(|pick| &pick.source) {
            Some(MediaSource::YouTube { video_id }) => Some(video_id.clone()),
            _ => match &attempt.concrete {
                Some(MediaSource::YouTube { video_id }) => Some(video_id.clone()),
                _ => None,
            },
        };
        let mut out = attempt.playing.as_ref().map(|pick| pick.key.clone());
        let embed = embed.map(|video_id| CandidateKey {
            provider_id: youtube_streams.to_owned(),
            candidate_id: video_id,
        });
        for key in [out.take(), embed].into_iter().flatten() {
            if !attempt.exclusions.tried.contains(&key) {
                attempt.exclusions.tried.push(key);
            }
        }
        Some((attempt.query.clone(), attempt.exclusions.clone()))
    }

    /// What to resolve for `load_id`, unless a newer load replaced it.
    pub fn request(&mut self, load_id: u32) -> Option<(TrackQuery, Exclusions)> {
        self.attempt(load_id)
            .map(|attempt| (attempt.query.clone(), attempt.exclusions.clone()))
    }

    /// A resolution finished. Returns where to start, or `None` when the load is stale and the
    /// pick should be dropped. Whether it plays is the player's call (`plays_when_found`): a
    /// Play or Pause may have come while the stream was found.
    pub fn resolved(
        &mut self,
        load_id: u32,
        pick: Pick,
        notes: &Notes,
        position_ms: f64,
    ) -> Option<f64> {
        let attempt = self.attempt(load_id)?;
        attempt.take_notes(notes);
        attempt.exclusions.tried.push(pick.key.clone());
        attempt.playing = Some(pick);
        attempt.verified = false;
        attempt.streams += 1;
        // A fallback after playback started resumes where the failed stream stopped.
        let start = if attempt.fell_back
            || attempt.failing_since.is_some()
            || attempt.exclusions.tried.len() > 1
        {
            position_ms.max(attempt.start_ms)
        } else {
            attempt.start_ms
        };
        Some(start)
    }

    /// The search for `load_id` came up empty at `now_ms` (the service's clock): when to look
    /// again, or that it's over. `marked` says the song is marked as one no source had. `None`
    /// when the load is stale.
    pub fn unresolved(
        &mut self,
        load_id: u32,
        notes: &Notes,
        now_ms: f64,
        marked: bool,
    ) -> Option<Unresolved> {
        let attempt = self.attempt(load_id)?;
        attempt.take_notes(notes);
        attempt.marked |= marked;
        let elapsed = attempt.failing_for(now_ms);
        let look = attempt.looks + 1;
        Some(match attempt.policy().wait(look, elapsed, notes) {
            Some(wait) => {
                attempt.looks = look;
                attempt.exclusions.deep = true;
                // Sources that couldn't answer are asked again by the new look.
                attempt.exclusions.unanswered.clear();
                Unresolved::Retry { wait }
            }
            None => Unresolved::GiveUp {
                mark: !attempt.troubled,
            },
        })
    }

    /// The engine failed for `load_id` at `now_ms` (the service's clock).
    pub fn engine_error(&mut self, load_id: u32, now_ms: f64) -> ErrorPlan {
        let Some(attempt) = self.attempt(load_id) else {
            return ErrorPlan::Forward;
        };
        if let Some(failed) = attempt.playing.take() {
            let elapsed = attempt.failing_for(now_ms);
            attempt
                .exclusions
                .failed_sources
                .push(failed.source.clone());
            let failed = if failed.from_cache {
                // The match played the song before: it's asked for a new stream.
                attempt.exclusions.tried.retain(|key| *key != failed.key);
                Failed::Stale(Box::new(failed))
            } else {
                Failed::Pick(Box::new(failed))
            };
            if !attempt.policy().within(elapsed) || attempt.streams >= MOST_STREAMS {
                return ErrorPlan::GiveUp {
                    mark: !attempt.troubled,
                    failed: Some(failed),
                };
            }
            return ErrorPlan::Retry { failed };
        }
        attempt.direct = false;
        if let Some(file) = attempt.download.take() {
            attempt.failing_for(now_ms);
            attempt.exclusions.failed_sources.push(file);
            return ErrorPlan::Retry {
                failed: Failed::Download,
            };
        }
        match attempt.concrete.take() {
            Some(source) if !attempt.fell_back && !attempt.query.title.trim().is_empty() => {
                attempt.failing_for(now_ms);
                attempt.fell_back = true;
                attempt.exclusions.failed_sources.push(source.clone());
                attempt.own_failed = Some(source.clone());
                ErrorPlan::Retry {
                    failed: Failed::Own(source),
                }
            }
            _ => ErrorPlan::Forward,
        }
    }

    /// The engine started playing `load_id`: the pick to record as verified, once, and whether
    /// it ended a search for another source.
    pub fn engine_playing(&mut self, load_id: u32) -> Option<Played> {
        let attempt = self.attempt(load_id)?;
        if attempt.verified {
            return None;
        }
        attempt.verified = true;
        let rescued = attempt.failing_since.take().is_some();
        // A later failure starts its search afresh.
        attempt.looks = 0;
        attempt.streams = 0;
        attempt.exclusions.deep = false;
        attempt.exclusions.unanswered.clear();
        let replaced = if rescued {
            attempt.own_failed.take()
        } else {
            None
        };
        attempt.playing.clone().map(|pick| Played {
            pick,
            rescued,
            replaced,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use raygun_model::MediaKind;

    use super::*;

    fn item(source: MediaSource) -> QueueItem {
        QueueItem {
            explicit: false,
            id: "item".into(),
            title: "Song".into(),
            artist: Some("Band".into()),
            album: None,
            duration_ms: Some(200_000),
            artwork_url: None,
            media_kind: MediaKind::Audio,
            source,
        }
    }

    fn url(name: &str) -> MediaSource {
        MediaSource::Url {
            url: format!("https://example.org/{name}.mp3"),
            headers: BTreeMap::new(),
        }
    }

    fn lookup() -> MediaSource {
        MediaSource::Lookup {
            title: "Song".into(),
            artist: Some("Band".into()),
            album: None,
            duration_ms: Some(200_000),
        }
    }

    fn pick(id: &str) -> Pick {
        Pick {
            key: key(id),
            track_key: "band|song".into(),
            title: "Song".into(),
            artist: None,
            provider_name: "Example".into(),
            source: url(id),
            expires_at: None,
            from_cache: false,
            bitrate_kbps: None,
        }
    }

    fn remembered(id: &str) -> Pick {
        Pick {
            from_cache: true,
            ..pick(id)
        }
    }

    fn key(id: &str) -> CandidateKey {
        CandidateKey {
            provider_id: "dev.example/stream".into(),
            candidate_id: id.into(),
        }
    }

    const NONE: Notes = Notes {
        failed: Vec::new(),
        unanswered: Vec::new(),
        limited: false,
        retry_after: None,
        troubled: false,
        needs_setup: false,
    };

    /// Seconds on the service's clock, in ms.
    fn at(seconds: f64) -> f64 {
        seconds * 1_000.0
    }

    #[test]
    fn lookups_wait_for_a_resolution() {
        let mut tracker = Tracker::default();
        assert_eq!(
            tracker.begin(1, &item(lookup()), 0.0, Cause::Chosen),
            LoadPlan::Resolve
        );
        let (query, exclusions) = tracker.request(1).unwrap();
        assert_eq!(query.title, "Song");
        assert_eq!(query.artist.as_deref(), Some("Band"));
        assert_eq!(exclusions, Exclusions::default());
        assert!(!tracker.loaded(1), "nothing loaded while it's found");
        assert_eq!(tracker.resolved(1, pick("a"), &NONE, 0.0), Some(0.0));
        assert!(tracker.loaded(1));
    }

    #[test]
    fn songs_are_found_ahead_as_their_load_would_find_them() {
        let mut tracker = Tracker::default();
        // A lookup is searched for by its names; a stream or a file of its own plays as it is.
        let (query, exclusions) = tracker.search_for(&item(lookup())).unwrap();
        assert_eq!(query.title, "Song");
        assert_eq!(exclusions, Exclusions::default());
        assert_eq!(tracker.search_for(&item(url("own"))), None);
        let file = MediaSource::LocalFile {
            path: "/sdcard/Music/a.mp3".into(),
        };
        assert_eq!(tracker.search_for(&item(file)), None);
        // A YouTube song tries its video's audio file first, unless its video shows.
        let video = MediaSource::YouTube {
            video_id: "dQw4w9WgXcQ".into(),
        };
        let (_, exclusions) = tracker.search_for(&item(video.clone())).unwrap();
        assert_eq!(exclusions.youtube.as_deref(), Some("dQw4w9WgXcQ"));
        tracker.set_video(true);
        assert_eq!(tracker.search_for(&item(video)), None);

        // The pick a load plays now (a download of the same song starts from it).
        tracker.begin(1, &item(lookup()), 0.0, Cause::Chosen);
        assert_eq!(tracker.pick(1), None, "nothing found yet");
        tracker.resolved(1, pick("a"), &NONE, 0.0);
        assert_eq!(tracker.pick(1), Some(&pick("a")));
        assert_eq!(tracker.pick(2), None, "another load's");
    }

    #[test]
    fn a_song_in_the_embed_finds_another_stream_without_the_webview() {
        let youtube = |id: &str| MediaSource::YouTube {
            video_id: id.into(),
        };
        let mut tracker = Tracker::default();
        tracker.begin(1, &item(lookup()), 0.0, Cause::Chosen);
        let embed = Pick {
            key: CandidateKey {
                provider_id: "dev.raygun.youtube/stream".into(),
                candidate_id: "dQw4w9WgXcQ".into(),
            },
            source: youtube("dQw4w9WgXcQ"),
            ..pick("embed")
        };
        tracker.resolved(1, embed.clone(), &NONE, 0.0);
        let (query, exclusions) = tracker
            .without_webview(1, "dev.raygun.youtube/stream")
            .unwrap();
        assert_eq!(query.title, "Song");
        assert_eq!(exclusions.tried, vec![embed.key.clone()]);
        // The stream found plays from where the song was.
        assert_eq!(
            tracker.resolved(1, pick("file"), &NONE, 42_000.0),
            Some(42_000.0)
        );

        // A YouTube song of its own, played as it is: its embed is out too.
        tracker.begin(2, &item(youtube("aqz-KE-bpKQ")), 0.0, Cause::Chosen);
        tracker.set_video(true);
        tracker.begin(3, &item(youtube("aqz-KE-bpKQ")), 0.0, Cause::Chosen);
        let (_, exclusions) = tracker
            .without_webview(3, "dev.raygun.youtube/stream")
            .unwrap();
        assert_eq!(exclusions.tried.len(), 1);
        assert_eq!(exclusions.tried[0].candidate_id, "aqz-KE-bpKQ");
        assert!(
            tracker
                .without_webview(2, "dev.raygun.youtube/stream")
                .is_none()
        );
    }

    #[test]
    fn stale_resolutions_are_dropped() {
        let mut tracker = Tracker::default();
        tracker.begin(1, &item(lookup()), 0.0, Cause::Chosen);
        tracker.begin(2, &item(lookup()), 0.0, Cause::Advance);
        assert_eq!(tracker.request(1), None);
        assert_eq!(tracker.resolved(1, pick("a"), &NONE, 0.0), None);
        assert_eq!(tracker.unresolved(1, &NONE, 0.0, false), None);
        assert_eq!(tracker.engine_error(1, 0.0), ErrorPlan::Forward);
        assert!(tracker.resolved(2, pick("a"), &NONE, 0.0).is_some());
        assert_eq!(tracker.cause(2), Some(Cause::Advance));
    }

    #[test]
    fn errors_move_through_candidates_then_reach_the_core() {
        let mut tracker = Tracker::default();
        tracker.begin(7, &item(lookup()), 0.0, Cause::Chosen);
        tracker.resolved(7, pick("a"), &NONE, 0.0);

        // The loaded pick failed: retry, excluding it.
        assert_eq!(
            tracker.engine_error(7, at(1.0)),
            ErrorPlan::Retry {
                failed: Failed::Pick(Box::new(pick("a")))
            }
        );
        let (_, exclusions) = tracker.request(7).unwrap();
        assert_eq!(exclusions.tried, vec![pick("a").key]);
        assert_eq!(exclusions.failed_sources, vec![url("a")]);

        // The next pick resumes from where playback got to.
        assert_eq!(
            tracker.resolved(7, pick("b"), &NONE, 42_000.0),
            Some(42_000.0)
        );
        let played = tracker.engine_playing(7).unwrap();
        assert_eq!(played.pick, pick("b"));
        assert!(played.rescued, "it played again after the failure");
        assert_eq!(tracker.engine_playing(7), None, "verified once");

        // An engine error with no pick loaded goes to the core.
        assert!(matches!(
            tracker.engine_error(7, at(2.0)),
            ErrorPlan::Retry { .. }
        ));
        assert_eq!(tracker.engine_error(7, at(2.0)), ErrorPlan::Forward);
    }

    #[test]
    fn a_remembered_stream_that_stopped_working_is_asked_for_again_at_its_source() {
        let mut tracker = Tracker::default();
        tracker.begin(2, &item(lookup()), 0.0, Cause::Advance);
        tracker.resolved(2, remembered("a"), &NONE, 0.0);

        // The stream the pick remembered died: it's forgotten, the match is asked for a new one.
        assert_eq!(
            tracker.engine_error(2, at(1.0)),
            ErrorPlan::Retry {
                failed: Failed::Stale(Box::new(remembered("a")))
            }
        );
        let (_, exclusions) = tracker.request(2).unwrap();
        assert!(exclusions.tried.is_empty(), "the match may play again");
        assert_eq!(
            exclusions.failed_sources,
            vec![url("a")],
            "its old stream may not"
        );

        // The same match's new stream fails too: now the match is out.
        tracker.resolved(2, pick("a"), &NONE, 0.0);
        assert!(matches!(
            tracker.engine_error(2, at(2.0)),
            ErrorPlan::Retry {
                failed: Failed::Pick(_)
            }
        ));
        assert_eq!(tracker.request(2).unwrap().1.tried, vec![key("a")]);
    }

    #[test]
    fn direct_sources_fall_back_once_and_are_replaced_by_what_plays() {
        let mut tracker = Tracker::default();
        assert_eq!(
            tracker.begin(3, &item(url("direct")), 0.0, Cause::Chosen),
            LoadPlan::Direct
        );
        assert!(tracker.loaded(3));
        assert_eq!(
            tracker.engine_playing(3),
            None,
            "nothing resolved to verify"
        );
        assert_eq!(
            tracker.engine_error(3, at(0.0)),
            ErrorPlan::Retry {
                failed: Failed::Own(url("direct"))
            }
        );
        assert!(!tracker.loaded(3), "nothing plays while another is found");
        let (_, exclusions) = tracker.request(3).unwrap();
        assert_eq!(exclusions.failed_sources, vec![url("direct")]);

        // Another source plays: it replaces the song's own.
        tracker.resolved(3, pick("alt"), &NONE, 10_000.0);
        let played = tracker.engine_playing(3).unwrap();
        assert!(played.rescued);
        assert_eq!(played.replaced, Some(url("direct")));

        assert!(matches!(
            tracker.engine_error(3, at(5.0)),
            ErrorPlan::Retry {
                failed: Failed::Pick(_)
            }
        ));
        // No second fallback for the item's own source.
        assert_eq!(tracker.engine_error(3, at(5.0)), ErrorPlan::Forward);
    }

    #[test]
    fn a_downloaded_copy_that_doesnt_play_gives_way_to_the_stream() {
        let mut tracker = Tracker::default();
        let file = MediaSource::LocalFile {
            path: "/data/downloads/music/1.mp3".into(),
        };
        let plan = tracker.begin_download(4, &item(url("own")), file.clone(), 0.0, Cause::Chosen);
        assert_eq!(plan, LoadPlan::Direct);
        assert_eq!(
            tracker.engine_error(4, 0.0),
            ErrorPlan::Retry {
                failed: Failed::Download
            }
        );
        let (query, exclusions) = tracker.request(4).unwrap();
        assert_eq!(query.title, "Song");
        assert_eq!(exclusions.failed_sources, vec![file]);
    }

    #[test]
    fn local_files_never_fall_back() {
        let mut tracker = Tracker::default();
        let file = MediaSource::LocalFile {
            path: "/sdcard/Music/a.mp3".into(),
        };
        assert_eq!(
            tracker.begin(4, &item(file), 0.0, Cause::Chosen),
            LoadPlan::Direct
        );
        assert!(tracker.loaded(4), "its start is watched");
        assert_eq!(tracker.engine_error(4, 0.0), ErrorPlan::Forward);
    }

    #[test]
    fn a_song_asked_for_looks_harder_three_times_waiting_longer_each_time() {
        let mut tracker = Tracker::default();
        tracker.begin(5, &item(lookup()), 0.0, Cause::Chosen);
        let mut now = at(1.0);
        let mut waits = Vec::new();
        while let Some(Unresolved::Retry { wait }) = tracker.unresolved(5, &NONE, now, false) {
            waits.push(wait);
            let (_, exclusions) = tracker.request(5).unwrap();
            assert!(exclusions.deep, "each look after the first looks harder");
            now += wait.as_secs_f64() * 1_000.0 + 500.0;
        }
        assert_eq!(
            waits,
            [1, 3, 9].map(Duration::from_secs),
            "three looks, each waiting {BACKOFF} times longer"
        );
        assert_eq!(
            tracker.unresolved(5, &NONE, now, false),
            Some(Unresolved::GiveUp { mark: true }),
            "every source answered: the song is marked as one no source has"
        );
    }

    #[test]
    fn a_source_waiting_for_its_key_keeps_a_song_unmarked_without_more_looks() {
        let mut tracker = Tracker::default();
        tracker.begin(13, &item(lookup()), 0.0, Cause::Chosen);
        let waiting = Notes {
            needs_setup: true,
            ..Notes::default()
        };
        let mut now = at(1.0);
        let mut looks = 0;
        while let Some(Unresolved::Retry { wait }) = tracker.unresolved(13, &waiting, now, false) {
            looks += 1;
            let (_, exclusions) = tracker.request(13).unwrap();
            assert!(exclusions.unanswered.is_empty(), "nothing to ask again");
            now += wait.as_secs_f64() * 1_000.0 + 500.0;
        }
        assert_eq!(looks, 3, "the same looks as any song");
        assert_eq!(
            tracker.unresolved(13, &waiting, now, false),
            Some(Unresolved::GiveUp { mark: false }),
            "a source wasn't asked: the song isn't marked as one no source has"
        );
    }

    #[test]
    fn looking_ends_with_its_budget() {
        let policy = RetryPolicy::of(Cause::Chosen, false);
        // A look that couldn't start within the budget isn't made.
        assert_eq!(
            policy.wait(2, Duration::from_secs(42), &NONE),
            None,
            "42 s + 3 s reaches the 45 s budget"
        );
        assert_eq!(
            policy.wait(2, Duration::from_secs(41), &NONE),
            Some(Duration::from_secs(3))
        );

        // Streams that keep failing stop once the budget is spent...
        let mut tracker = Tracker::default();
        tracker.begin(6, &item(lookup()), 0.0, Cause::Advance);
        tracker.resolved(6, pick("a"), &NONE, 0.0);
        assert!(matches!(
            tracker.engine_error(6, at(10.0)),
            ErrorPlan::Retry { .. }
        ));
        tracker.resolved(6, pick("b"), &NONE, 0.0);
        assert_eq!(
            tracker.engine_error(6, at(10.0 + 25.0)),
            ErrorPlan::GiveUp {
                mark: true,
                failed: Some(Failed::Pick(Box::new(pick("b")))),
            },
            "25 s after the first failure a song the queue moved on to stops looking"
        );

        // ... or after a few streams, however quick their failures.
        let mut tracker = Tracker::default();
        tracker.begin(7, &item(lookup()), 0.0, Cause::Chosen);
        let mut plans = Vec::new();
        for id in 0..=MOST_STREAMS {
            tracker.resolved(7, pick(&id.to_string()), &NONE, 0.0);
            plans.push(tracker.engine_error(7, at(1.0)));
        }
        assert!(matches!(
            plans.last(),
            Some(ErrorPlan::GiveUp { mark: true, .. })
        ));
        assert_eq!(
            plans
                .iter()
                .filter(|plan| matches!(plan, ErrorPlan::Retry { .. }))
                .count() as u32,
            MOST_STREAMS - 1
        );
    }

    #[test]
    fn sources_that_limit_requests_are_waited_for() {
        let policy = RetryPolicy::of(Cause::Chosen, false);
        let limited = Notes {
            limited: true,
            ..Notes::default()
        };
        assert_eq!(policy.wait(1, Duration::ZERO, &limited), Some(LIMITED_WAIT));
        // A source that said how long: at least that, up to the longest wait.
        let asked = |seconds| Notes {
            retry_after: Some(Duration::from_secs(seconds)),
            ..Notes::default()
        };
        assert_eq!(
            policy.wait(1, Duration::ZERO, &asked(12)),
            Some(Duration::from_secs(12))
        );
        assert_eq!(
            policy.wait(1, Duration::ZERO, &asked(600)),
            Some(LONGEST_WAIT)
        );
        // A wait that doesn't fit what's left: the song is skipped instead.
        assert_eq!(policy.wait(1, Duration::from_secs(30), &asked(600)), None);
        let quick = RetryPolicy::of(Cause::AfterFailure, false);
        assert_eq!(quick.wait(1, Duration::ZERO, &asked(600)), None);

        // Sources that couldn't answer are left alone until the next look, which asks them again.
        let mut tracker = Tracker::default();
        tracker.begin(8, &item(lookup()), 0.0, Cause::Chosen);
        let busy = Notes {
            unanswered: vec![key("busy")],
            failed: vec![key("gone")],
            ..limited
        };
        tracker.resolved(8, pick("a"), &busy, 0.0);
        let (_, exclusions) = tracker.request(8).unwrap();
        assert_eq!(exclusions.unanswered, vec![key("busy")]);
        assert_eq!(exclusions.tried, vec![key("gone"), key("a")]);
        tracker.engine_error(8, at(1.0));
        assert_eq!(
            tracker.unresolved(8, &busy, at(2.0), false),
            Some(Unresolved::Retry { wait: LIMITED_WAIT })
        );
        let (_, exclusions) = tracker.request(8).unwrap();
        assert!(exclusions.unanswered.is_empty(), "asked again");
        assert!(
            exclusions.tried.contains(&key("gone")),
            "what's gone stays out"
        );

        // A song given up on while sources couldn't answer isn't marked as one none has.
        let mut tracker = Tracker::default();
        tracker.begin(12, &item(lookup()), 0.0, Cause::AfterFailure);
        let offline = Notes {
            troubled: true,
            ..Notes::default()
        };
        assert!(matches!(
            tracker.unresolved(12, &offline, 0.0, false),
            Some(Unresolved::Retry { .. })
        ));
        assert_eq!(
            tracker.unresolved(12, &offline, at(2.0), false),
            Some(Unresolved::GiveUp { mark: false })
        );
    }

    #[test]
    fn the_queue_moving_on_looks_less_and_skips_marked_songs_at_once() {
        let looks = |cause, marked| {
            let mut tracker = Tracker::default();
            tracker.begin(9, &item(lookup()), 0.0, cause);
            let mut count = 0;
            let mut now = 0.0;
            while let Some(Unresolved::Retry { wait }) = tracker.unresolved(9, &NONE, now, marked) {
                count += 1;
                now += wait.as_secs_f64() * 1_000.0;
            }
            count
        };
        assert_eq!(looks(Cause::Chosen, false), 3);
        assert_eq!(
            looks(Cause::Chosen, true),
            3,
            "a song asked for is looked for"
        );
        assert_eq!(looks(Cause::Advance, false), 2);
        assert_eq!(looks(Cause::AfterFailure, false), 1, "a run of failures");
        assert_eq!(looks(Cause::Advance, true), 0, "marked: skipped quickly");
        assert_eq!(looks(Cause::AfterFailure, true), 0);
    }

    #[test]
    fn a_song_found_again_after_looking_is_rescued_once() {
        let mut tracker = Tracker::default();
        tracker.begin(10, &item(lookup()), 0.0, Cause::Chosen);
        assert!(matches!(
            tracker.unresolved(10, &NONE, 0.0, true),
            Some(Unresolved::Retry { .. })
        ));
        tracker.resolved(10, pick("found"), &NONE, 0.0);
        let played = tracker.engine_playing(10).unwrap();
        assert!(played.rescued, "its unavailable mark goes");
        assert_eq!(played.replaced, None);
        assert!(
            !tracker.request(10).unwrap().1.deep,
            "a later failure starts afresh"
        );

        // A song that played at once wasn't rescued.
        let mut tracker = Tracker::default();
        tracker.begin(11, &item(lookup()), 0.0, Cause::Chosen);
        tracker.resolved(11, pick("a"), &NONE, 0.0);
        assert!(!tracker.engine_playing(11).unwrap().rescued);
    }

    #[test]
    fn youtube_videos_are_resolved_to_their_own_audio_first() {
        let mut tracker = Tracker::default();
        let video = MediaSource::YouTube {
            video_id: "dQw4w9WgXcQ".into(),
        };
        assert_eq!(
            tracker.begin(5, &item(video), 0.0, Cause::Chosen),
            LoadPlan::Resolve
        );
        let (query, exclusions) = tracker.request(5).unwrap();
        assert_eq!(query.title, "Song");
        assert_eq!(exclusions.youtube.as_deref(), Some("dQw4w9WgXcQ"));
        assert!(exclusions.tried.is_empty());
        // Its audio file failed in the engine: tried again without it (its embed, then others).
        tracker.resolved(5, pick("audio:dQw4w9WgXcQ"), &NONE, 0.0);
        assert!(matches!(
            tracker.engine_error(5, 0.0),
            ErrorPlan::Retry {
                failed: Failed::Pick(_)
            }
        ));
        let (_, exclusions) = tracker.request(5).unwrap();
        assert_eq!(exclusions.tried, vec![pick("audio:dQw4w9WgXcQ").key]);
        assert_eq!(exclusions.youtube.as_deref(), Some("dQw4w9WgXcQ"));
        assert_eq!(
            tracker.playing(5),
            None,
            "nothing plays until the next pick"
        );
        tracker.resolved(5, pick("dQw4w9WgXcQ"), &NONE, 0.0);
        assert_eq!(tracker.playing(5), Some(&url("dQw4w9WgXcQ")));

        // With the video shown, YouTube songs play in the embed, which has it.
        assert!(tracker.set_video(true));
        assert!(!tracker.set_video(true), "no change");
        let video = MediaSource::YouTube {
            video_id: "dQw4w9WgXcQ".into(),
        };
        assert_eq!(
            tracker.begin(6, &item(video), 0.0, Cause::Chosen),
            LoadPlan::Direct
        );
        assert_eq!(tracker.playing(6), None);
    }
}
