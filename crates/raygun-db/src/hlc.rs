//! Hybrid logical clock (ARCHITECTURE §10).
//!
//! A timestamp is a `u64`: the high 48 bits are physical milliseconds since the Unix epoch, the
//! low 16 bits a counter. Comparing two timestamps as plain integers orders writes across devices
//! even when their wall clocks disagree a little, and a device whose clock jumps backwards keeps
//! producing increasing values.

const COUNTER_BITS: u32 = 16;

#[derive(Debug, Clone, Copy)]
pub struct Clock {
    last: u64,
}

impl Clock {
    /// Resumes from the highest timestamp this device has issued or seen.
    pub fn new(last: u64) -> Self {
        Self { last }
    }

    pub fn last(&self) -> u64 {
        self.last
    }

    /// Timestamp for a local write.
    pub fn tick(&mut self, now_ms: u64) -> u64 {
        self.last = (now_ms << COUNTER_BITS).max(self.last + 1);
        self.last
    }

    /// Folds in a timestamp received from another device, so later local writes order after it.
    pub fn observe(&mut self, remote: u64, now_ms: u64) -> u64 {
        self.last = (now_ms << COUNTER_BITS).max(self.last + 1).max(remote + 1);
        self.last
    }
}

/// Physical milliseconds encoded in a timestamp.
pub fn physical_ms(timestamp: u64) -> u64 {
    timestamp >> COUNTER_BITS
}

pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_follow_the_wall_clock() {
        let mut clock = Clock::new(0);

        let timestamp = clock.tick(1_000);

        assert_eq!(physical_ms(timestamp), 1_000);
    }

    #[test]
    fn ticks_in_the_same_millisecond_still_increase() {
        let mut clock = Clock::new(0);

        let first = clock.tick(1_000);
        let second = clock.tick(1_000);

        assert!(second > first);
        assert_eq!(physical_ms(second), 1_000);
    }

    #[test]
    fn clock_going_backwards_never_produces_older_timestamps() {
        let mut clock = Clock::new(0);
        let before = clock.tick(5_000);

        let after = clock.tick(1_000);

        assert!(after > before);
    }

    #[test]
    fn resumes_after_the_persisted_value() {
        let mut first_run = Clock::new(0);
        let persisted = first_run.tick(9_000);

        let mut second_run = Clock::new(persisted);

        assert!(second_run.tick(1_000) > persisted);
    }

    #[test]
    fn observing_a_newer_remote_moves_the_clock_past_it() {
        let mut clock = Clock::new(0);
        let remote = Clock::new(0).tick(50_000);

        clock.observe(remote, 1_000);

        assert!(clock.tick(1_000) > remote);
    }
}
