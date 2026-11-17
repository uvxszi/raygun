//! Skipping the parts of one load of a song as its player's reports reach them (pure:
//! `PlaybackService` keeps one per load and seeks where it says).

use std::collections::HashSet;

use raygun_model::{SegmentAction, SkipSegment};

use super::Segment;

/// Reports this close to a part's end don't skip it: the jump would be nothing.
const NEAR_END_MS: f64 = 500.0;
/// Parts this short aren't skipped: the jump would be heard more than the part.
const SHORTEST_MS: f64 = 1_000.0;
/// Parts that start within this much of where a jump lands are passed in the same jump.
const JOINED_MS: f64 = 1_000.0;
/// After a jump, reports from before it (still inside the part) are let through for this long.
const SETTLE_MS: f64 = 3_000.0;

/// A jump over a part (and any that follow on from it).
#[derive(Debug, Clone, PartialEq)]
pub struct Skip {
    /// The part reached.
    pub segment: SkipSegment,
    /// Where to go on from, ms.
    pub to_ms: f64,
}

/// The parts of one load and what was done with them.
#[derive(Debug, Clone, Default)]
pub struct LoadSkips {
    pub load_id: u32,
    pub item_id: String,
    pub video_id: String,
    /// Unknown until SponsorBlock answered.
    segments: Option<Vec<Segment>>,
    /// Parts taken back with Undo: played through for the rest of this load.
    undone: HashSet<String>,
    /// The last jump: until when (the clock's ms) and where it went.
    settling: Option<(f64, f64)>,
}

impl LoadSkips {
    pub fn new(load_id: u32, item_id: &str, video_id: &str) -> Self {
        Self {
            load_id,
            item_id: item_id.to_owned(),
            video_id: video_id.to_owned(),
            ..Self::default()
        }
    }

    pub fn segments(&self) -> Option<&[Segment]> {
        self.segments.as_deref()
    }

    pub fn set_segments(&mut self, segments: Vec<Segment>) {
        self.segments = Some(segments);
    }

    /// The part to jump over at `position_ms` (`shown`: the parts as handled now), if one is
    /// reached; `now_ms` is a clock for telling late reports from new ones.
    pub fn due(
        &mut self,
        shown: &[SkipSegment],
        position_ms: f64,
        duration_ms: Option<f64>,
        now_ms: f64,
    ) -> Option<Skip> {
        if let Some((until, landed)) = self.settling {
            // A report from before the jump: the player hasn't moved yet.
            if now_ms < until && position_ms < landed - NEAR_END_MS {
                return None;
            }
            self.settling = None;
        }
        let skipped = |segment: &&SkipSegment| {
            segment.action == SegmentAction::Skip
                && !self.undone.contains(&segment.id)
                && segment.end_ms - segment.start_ms >= SHORTEST_MS
        };
        let reached = shown.iter().filter(skipped).find(|segment| {
            segment.start_ms <= position_ms && position_ms < segment.end_ms - NEAR_END_MS
        })?;
        let mut to_ms = reached.end_ms;
        // Parts that overlap it or follow right on go in the same jump.
        while let Some(further) = shown
            .iter()
            .filter(skipped)
            .filter(|segment| segment.start_ms <= to_ms + JOINED_MS && segment.end_ms > to_ms)
            .map(|segment| segment.end_ms)
            .reduce(f64::max)
        {
            to_ms = further;
        }
        if let Some(duration) = duration_ms.filter(|duration| *duration > 0.0) {
            to_ms = to_ms.min(duration);
        }
        self.settling = Some((now_ms + SETTLE_MS, to_ms));
        Some(Skip {
            segment: reached.clone(),
            to_ms,
        })
    }

    /// Takes a jump over `segment_id` back: the part plays from its start, and isn't jumped over
    /// again in this load. Where it starts, ms; none for a part this load doesn't have.
    pub fn undo(&mut self, segment_id: &str) -> Option<f64> {
        let start = self
            .segments
            .as_ref()?
            .iter()
            .find(|segment| segment.id == segment_id)?
            .start_ms;
        self.undone.insert(segment_id.to_owned());
        self.settling = None;
        Some(start)
    }
}

#[cfg(test)]
mod tests {
    use raygun_model::SegmentCategory;

    use super::*;

    fn part(id: &str, start: f64, end: f64, action: SegmentAction) -> SkipSegment {
        SkipSegment {
            id: id.into(),
            category: SegmentCategory::Sponsor,
            start_ms: start,
            end_ms: end,
            action,
        }
    }

    #[test]
    fn a_part_is_jumped_over_once_its_reached_and_late_reports_dont_jump_again() {
        let mut skips = LoadSkips::new(1, "item", "dQw4w9WgXcQ");
        let shown = [part("a", 10_000.0, 40_000.0, SegmentAction::Skip)];
        assert_eq!(skips.due(&shown, 9_000.0, Some(200_000.0), 0.0), None);
        let skip = skips.due(&shown, 10_400.0, Some(200_000.0), 100.0).unwrap();
        assert_eq!(skip.to_ms, 40_000.0);
        assert_eq!(skip.segment.id, "a");
        // The player hadn't moved yet when it reported this.
        assert_eq!(skips.due(&shown, 10_900.0, Some(200_000.0), 600.0), None);
        // It moved.
        assert_eq!(skips.due(&shown, 40_100.0, Some(200_000.0), 900.0), None);
        // Played back into later on: jumped over again.
        assert!(
            skips
                .due(&shown, 20_000.0, Some(200_000.0), 9_000.0)
                .is_some()
        );
    }

    #[test]
    fn parts_only_shown_short_ones_and_their_last_moment_are_played() {
        let mut skips = LoadSkips::new(1, "item", "dQw4w9WgXcQ");
        let shown = [
            part("shown", 0.0, 5_000.0, SegmentAction::Show),
            part("short", 6_000.0, 6_500.0, SegmentAction::Skip),
            part("long", 8_000.0, 20_000.0, SegmentAction::Skip),
        ];
        assert_eq!(skips.due(&shown, 1_000.0, None, 0.0), None);
        assert_eq!(skips.due(&shown, 6_100.0, None, 0.0), None);
        assert_eq!(skips.due(&shown, 19_700.0, None, 0.0), None);
    }

    #[test]
    fn parts_that_follow_on_go_in_one_jump_and_none_goes_past_the_end() {
        let mut skips = LoadSkips::new(1, "item", "dQw4w9WgXcQ");
        let shown = [
            part("a", 0.0, 12_000.0, SegmentAction::Skip),
            part("b", 12_500.0, 30_000.0, SegmentAction::Skip),
            part("c", 29_000.0, 45_000.0, SegmentAction::Skip),
            part("end", 170_000.0, 190_000.0, SegmentAction::Skip),
        ];
        assert_eq!(
            skips
                .due(&shown, 200.0, Some(180_000.0), 0.0)
                .unwrap()
                .to_ms,
            45_000.0
        );
        assert_eq!(
            skips
                .due(&shown, 171_000.0, Some(180_000.0), 10_000.0)
                .unwrap()
                .to_ms,
            180_000.0
        );
    }

    #[test]
    fn undo_goes_back_to_the_part_and_lets_it_play_for_the_rest_of_the_load() {
        let mut skips = LoadSkips::new(1, "item", "dQw4w9WgXcQ");
        skips.set_segments(vec![Segment {
            id: "a".into(),
            category: SegmentCategory::MusicOfftopic,
            start_ms: 10_000.0,
            end_ms: 40_000.0,
            video_ms: 0.0,
        }]);
        let shown = [part("a", 10_000.0, 40_000.0, SegmentAction::Skip)];
        assert!(skips.due(&shown, 10_500.0, None, 0.0).is_some());
        assert_eq!(skips.undo("a"), Some(10_000.0));
        assert_eq!(skips.due(&shown, 10_100.0, None, 100.0), None);
        assert_eq!(skips.undo("unknown"), None);
    }
}
