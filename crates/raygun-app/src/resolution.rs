//! Per-load bookkeeping for stream resolution (M14): what the current load is trying to play, which
//! candidates it already tried, and what to do when the engine reports success or failure.
//! Pure state; `PlaybackService` carries out the plans.

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadPlan {
    /// The item's own source plays as it is.
    Direct,
    /// Find a stream first.
    Resolve,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorPlan {
    /// Let the playback core handle the error (it skips to the next item).
    Forward,
    /// Try another stream for the same load; `failed` is the pick that just failed, if any.
    Retry { failed: Option<Box<Pick>> },
}

#[derive(Debug, Clone)]
struct Attempt {
    load_id: u32,
    query: TrackQuery,
    start_ms: f64,
    autoplay: bool,
    exclusions: Exclusions,
    /// The resolver's pick now loaded in the engine.
    playing: Option<Pick>,
    /// The item's own source, when it has one that a fallback could replace.
    concrete: Option<MediaSource>,
    fell_back: bool,
    verified: bool,
}

#[derive(Debug, Default)]
pub struct Tracker {
    current: Option<Attempt>,
}

impl Tracker {
    /// Starts tracking a new load.
    pub fn begin(
        &mut self,
        load_id: u32,
        item: &QueueItem,
        start_ms: f64,
        autoplay: bool,
    ) -> LoadPlan {
        let (plan, concrete) = match &item.source {
            MediaSource::Lookup { .. } => (LoadPlan::Resolve, None),
            // A file on this device has no alternative worth searching for.
            MediaSource::LocalFile { .. } => (LoadPlan::Direct, None),
            source => (LoadPlan::Direct, Some(source.clone())),
        };
        self.current = Some(Attempt {
            load_id,
            query: TrackQuery::for_item(item),
            start_ms,
            autoplay,
            exclusions: Exclusions::default(),
            playing: None,
            concrete,
            fell_back: false,
            verified: false,
        });
        plan
    }

    fn attempt(&mut self, load_id: u32) -> Option<&mut Attempt> {
        self.current
            .as_mut()
            .filter(|attempt| attempt.load_id == load_id)
    }

    /// What to resolve for `load_id`, unless a newer load replaced it.
    pub fn request(&mut self, load_id: u32) -> Option<(TrackQuery, Exclusions)> {
        self.attempt(load_id)
            .map(|attempt| (attempt.query.clone(), attempt.exclusions.clone()))
    }

    /// A resolution finished. Returns where to start and whether to autoplay, or `None` when the
    /// load is stale and the pick should be dropped.
    pub fn resolved(&mut self, load_id: u32, pick: Pick, position_ms: f64) -> Option<(f64, bool)> {
        let attempt = self.attempt(load_id)?;
        attempt.exclusions.tried.push(pick.key.clone());
        attempt.playing = Some(pick);
        attempt.verified = false;
        // A fallback after playback started resumes where the failed stream stopped.
        let start = if attempt.fell_back || attempt.exclusions.tried.len() > 1 {
            position_ms.max(attempt.start_ms)
        } else {
            attempt.start_ms
        };
        Some((start, attempt.autoplay))
    }

    /// The engine failed for `load_id`.
    pub fn engine_error(&mut self, load_id: u32) -> ErrorPlan {
        let Some(attempt) = self.attempt(load_id) else {
            return ErrorPlan::Forward;
        };
        if let Some(failed) = attempt.playing.take() {
            return ErrorPlan::Retry {
                failed: Some(Box::new(failed)),
            };
        }
        match attempt.concrete.take() {
            Some(source) if !attempt.fell_back && !attempt.query.title.trim().is_empty() => {
                attempt.fell_back = true;
                attempt.exclusions.failed_sources.push(source);
                ErrorPlan::Retry { failed: None }
            }
            _ => ErrorPlan::Forward,
        }
    }

    /// The engine started playing `load_id`; returns the pick to record as verified, once.
    pub fn engine_playing(&mut self, load_id: u32) -> Option<Pick> {
        let attempt = self.attempt(load_id)?;
        if attempt.verified {
            return None;
        }
        attempt.verified = true;
        attempt.playing.clone()
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
            key: CandidateKey {
                provider_id: "dev.example/stream".into(),
                candidate_id: id.into(),
            },
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

    #[test]
    fn lookups_wait_for_a_resolution() {
        let mut tracker = Tracker::default();
        assert_eq!(
            tracker.begin(1, &item(lookup()), 0.0, true),
            LoadPlan::Resolve
        );
        let (query, exclusions) = tracker.request(1).unwrap();
        assert_eq!(query.title, "Song");
        assert_eq!(query.artist.as_deref(), Some("Band"));
        assert_eq!(exclusions, Exclusions::default());
        assert_eq!(tracker.resolved(1, pick("a"), 0.0), Some((0.0, true)));
    }

    #[test]
    fn stale_resolutions_are_dropped() {
        let mut tracker = Tracker::default();
        tracker.begin(1, &item(lookup()), 0.0, true);
        tracker.begin(2, &item(lookup()), 0.0, true);
        assert_eq!(tracker.request(1), None);
        assert_eq!(tracker.resolved(1, pick("a"), 0.0), None);
        assert_eq!(tracker.engine_error(1), ErrorPlan::Forward);
        assert!(tracker.resolved(2, pick("a"), 0.0).is_some());
    }

    #[test]
    fn errors_move_through_candidates_then_reach_the_core() {
        let mut tracker = Tracker::default();
        tracker.begin(7, &item(lookup()), 0.0, true);
        tracker.resolved(7, pick("a"), 0.0);

        // The loaded pick failed: retry, excluding it.
        assert_eq!(
            tracker.engine_error(7),
            ErrorPlan::Retry {
                failed: Some(Box::new(pick("a")))
            }
        );
        let (_, exclusions) = tracker.request(7).unwrap();
        assert_eq!(exclusions.tried, vec![pick("a").key]);

        // The next pick resumes from where playback got to.
        assert_eq!(
            tracker.resolved(7, pick("b"), 42_000.0),
            Some((42_000.0, true))
        );
        assert_eq!(tracker.engine_playing(7), Some(pick("b")));
        assert_eq!(tracker.engine_playing(7), None, "verified once");

        // If the resolver then has nothing left it reports that itself; an engine error with no
        // pick loaded goes to the core.
        assert!(matches!(tracker.engine_error(7), ErrorPlan::Retry { .. }));
        assert_eq!(tracker.engine_error(7), ErrorPlan::Forward);
    }

    #[test]
    fn direct_sources_fall_back_once() {
        let mut tracker = Tracker::default();
        assert_eq!(
            tracker.begin(3, &item(url("direct")), 0.0, true),
            LoadPlan::Direct
        );
        assert_eq!(
            tracker.engine_playing(3),
            None,
            "nothing resolved to verify"
        );
        assert_eq!(tracker.engine_error(3), ErrorPlan::Retry { failed: None });
        let (_, exclusions) = tracker.request(3).unwrap();
        assert_eq!(exclusions.failed_sources, vec![url("direct")]);

        tracker.resolved(3, pick("alt"), 10_000.0);
        assert!(matches!(
            tracker.engine_error(3),
            ErrorPlan::Retry { failed: Some(_) }
        ));
        // No second fallback for the item's own source.
        assert_eq!(tracker.engine_error(3), ErrorPlan::Forward);
    }

    #[test]
    fn local_files_never_fall_back() {
        let mut tracker = Tracker::default();
        let file = MediaSource::LocalFile {
            path: "/sdcard/Music/a.mp3".into(),
        };
        assert_eq!(tracker.begin(4, &item(file), 0.0, true), LoadPlan::Direct);
        assert_eq!(tracker.engine_error(4), ErrorPlan::Forward);
    }
}
