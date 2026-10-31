//! Decides when a load counts as a play for history (M18): half the track or four minutes of
//! actual listening, whichever comes first. Seeks don't add listening time.

/// Progress reports further apart than this are a seek (or a stall), not listening.
const MAX_STEP_MS: f64 = 3_000.0;
/// Listening this long always counts.
const ENOUGH_MS: f64 = 240_000.0;
/// Shorter tracks count only when they play to the end.
const SHORT_TRACK_MS: f64 = 30_000.0;

#[derive(Debug, Default)]
pub struct PlayCounter {
    load_id: Option<u32>,
    last_position: Option<f64>,
    listened_ms: f64,
    /// Shorter than [`SHORT_TRACK_MS`], so it counts only by ending.
    short: bool,
    counted: bool,
}

impl PlayCounter {
    /// A progress report for `load_id`. True exactly once per load, when it becomes a play.
    pub fn progress(&mut self, load_id: u32, position_ms: f64, duration_ms: Option<f64>) -> bool {
        self.follow(load_id);
        if let Some(last) = self.last_position.replace(position_ms) {
            let step = position_ms - last;
            if step > 0.0 && step <= MAX_STEP_MS {
                self.listened_ms += step;
            }
        }
        let needed = match duration_ms.filter(|duration| *duration > 0.0) {
            Some(duration) if duration < SHORT_TRACK_MS => {
                self.short = true;
                return false;
            }
            Some(duration) => (duration / 2.0).min(ENOUGH_MS),
            None => ENOUGH_MS,
        };
        self.count(self.listened_ms >= needed)
    }

    /// The load played to its end: a short track counts now. A longer one counts only by
    /// listening, so skipping to its end isn't a play.
    pub fn ended(&mut self, load_id: u32) -> bool {
        self.follow(load_id);
        self.count(self.short)
    }

    fn follow(&mut self, load_id: u32) {
        if self.load_id != Some(load_id) {
            *self = Self {
                load_id: Some(load_id),
                ..Self::default()
            };
        }
    }

    fn count(&mut self, reached: bool) -> bool {
        if reached && !self.counted {
            self.counted = true;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plays `load_id` from `from` to `to` in one-second reports; true if it counted on the way.
    fn play(counter: &mut PlayCounter, load_id: u32, from: f64, to: f64, duration: f64) -> bool {
        let mut counted = false;
        let mut position = from;
        while position <= to {
            counted |= counter.progress(load_id, position, Some(duration));
            position += 1_000.0;
        }
        counted
    }

    #[test]
    fn half_the_track_counts_once() {
        let mut counter = PlayCounter::default();
        assert!(!play(&mut counter, 1, 0.0, 89_000.0, 180_000.0));
        assert!(play(&mut counter, 1, 90_000.0, 91_000.0, 180_000.0));
        assert!(!play(&mut counter, 1, 92_000.0, 180_000.0, 180_000.0));
        assert!(!counter.ended(1), "a counted load never counts again");
    }

    #[test]
    fn long_tracks_count_after_four_minutes() {
        let mut counter = PlayCounter::default();
        assert!(play(&mut counter, 1, 0.0, 241_000.0, 3_600_000.0));
        let mut unknown = PlayCounter::default();
        assert!(!unknown.progress(2, 0.0, None));
        let mut position = 0.0;
        let mut counted = false;
        while position <= 241_000.0 {
            counted |= unknown.progress(2, position, None);
            position += 1_000.0;
        }
        assert!(counted, "unknown length counts after four minutes too");
    }

    #[test]
    fn seeking_ahead_adds_no_listening_time() {
        let mut counter = PlayCounter::default();
        assert!(!play(&mut counter, 1, 0.0, 10_000.0, 180_000.0));
        // Jump to near the end: only the seconds actually heard count.
        assert!(!play(&mut counter, 1, 170_000.0, 180_000.0, 180_000.0));
        // Seeking back and listening on does count.
        assert!(play(&mut counter, 1, 20_000.0, 95_000.0, 180_000.0));
    }

    #[test]
    fn short_tracks_count_when_they_end() {
        let mut counter = PlayCounter::default();
        assert!(!play(&mut counter, 1, 0.0, 20_000.0, 20_000.0));
        assert!(counter.ended(1));

        let mut skipped = PlayCounter::default();
        assert!(!play(&mut skipped, 1, 175_000.0, 180_000.0, 180_000.0));
        assert!(
            !skipped.ended(1),
            "skipping to the end of a long track isn't a play"
        );
    }

    #[test]
    fn a_new_load_starts_over() {
        let mut counter = PlayCounter::default();
        assert!(!play(&mut counter, 1, 0.0, 60_000.0, 180_000.0));
        assert!(!play(&mut counter, 2, 0.0, 60_000.0, 180_000.0));
        assert!(play(&mut counter, 2, 61_000.0, 91_000.0, 180_000.0));
    }
}
