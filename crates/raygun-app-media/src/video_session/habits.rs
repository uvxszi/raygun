//! The sources a person keeps choosing, per kind of video (movies, series): a source (a server,
//! an addon, a stream provider's group) counts when it's picked by hand in Sources and when a
//! video from it is watched through (most of it, or to its end). Counts fade by half a month at a
//! time, so a new habit takes over from an old one. Across titles, the source most chosen for the
//! kind (clearly ahead of the next) is tried first among sources that stand alike, and the Sources
//! sheet says so ("You usually watch from …"). What played for the title itself still comes
//! first ([`super::memory::Standing`]).

use std::cmp::Reverse;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Where the habits are kept (`local_value`).
pub const HABITS_KEY: &str = "video.sourceHabits";

/// A count halves every 15 days without the source being chosen again.
const HALF_LIFE_MS: f64 = 15.0 * 24.0 * 60.0 * 60.0 * 1000.0;
/// Sources not chosen for this long are forgotten.
const FORGET_AFTER_MS: i64 = 120 * 24 * 60 * 60 * 1000;
/// The most sources kept per kind.
const MOST_KEPT: usize = 60;

/// A source picked by hand in Sources.
pub const PICKED: f64 = 1.0;
/// A video from the source watched through.
pub const WATCHED: f64 = 1.0;

/// A source is the usual one from this count on (two choices, say)...
const USUAL_FROM: f64 = 1.9;
/// ...and this far ahead of the next one.
const USUAL_LEAD: f64 = 1.5;

/// What makes the ranking: a count is worth this much of a source's health score (which counts
/// plays of any kind).
const WEIGHT: f64 = 1.0;
/// The usual source's lead over every other of its standing.
const USUAL_BONUS: f64 = 1_000.0;

/// Watched through: most of it.
pub const WATCHED_SHARE: f64 = 0.7;

/// The kind of video a habit is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WatchKind {
    Movie,
    Series,
}

impl WatchKind {
    /// Stremio's types and episode lists: a movie unless it has episodes or says it's a series.
    pub fn of(stremio_kind: Option<&str>, episodes: usize) -> Self {
        match stremio_kind {
            Some("movie") => Self::Movie,
            Some(_) => Self::Series,
            None if episodes > 1 => Self::Series,
            None => Self::Movie,
        }
    }
}

/// How often a source was chosen, fading from `at`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Habit {
    pub score: f64,
    /// When it was last chosen, Unix ms.
    pub at: i64,
}

fn faded(habit: &Habit, now: i64) -> f64 {
    let age = (now - habit.at).max(0) as f64;
    habit.score * 0.5_f64.powf(age / HALF_LIFE_MS)
}

/// Sources chosen, by kind and name.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SourceHabits {
    pub movie: HashMap<String, Habit>,
    pub series: HashMap<String, Habit>,
}

impl SourceHabits {
    fn of(&self, kind: WatchKind) -> &HashMap<String, Habit> {
        match kind {
            WatchKind::Movie => &self.movie,
            WatchKind::Series => &self.series,
        }
    }

    fn of_mut(&mut self, kind: WatchKind) -> &mut HashMap<String, Habit> {
        match kind {
            WatchKind::Movie => &mut self.movie,
            WatchKind::Series => &mut self.series,
        }
    }

    /// `name` was chosen (`weight`: [`PICKED`], [`WATCHED`]).
    pub fn note(&mut self, kind: WatchKind, name: &str, weight: f64, now: i64) {
        if name.is_empty() {
            return;
        }
        let habits = self.of_mut(kind);
        let habit = habits.entry(name.to_owned()).or_default();
        habit.score = faded(habit, now) + weight;
        habit.at = now;
        habits.retain(|_, habit| now - habit.at < FORGET_AFTER_MS);
        if habits.len() > MOST_KEPT {
            let mut by_count: Vec<(String, f64)> = habits
                .iter()
                .map(|(name, habit)| (name.clone(), faded(habit, now)))
                .collect();
            by_count.sort_by(|a, b| b.1.total_cmp(&a.1));
            for (name, _) in by_count.into_iter().skip(MOST_KEPT) {
                habits.remove(&name);
            }
        }
    }

    /// How often `name` was chosen lately for `kind`.
    pub fn count(&self, kind: WatchKind, name: &str, now: i64) -> f64 {
        self.of(kind)
            .get(name)
            .map_or(0.0, |habit| faded(habit, now))
    }

    /// The source usually chosen for `kind`: chosen about twice or more lately, and clearly more
    /// than any other.
    pub fn usual(&self, kind: WatchKind, now: i64) -> Option<&str> {
        let mut counts: Vec<(&str, f64)> = self
            .of(kind)
            .iter()
            .map(|(name, habit)| (name.as_str(), faded(habit, now)))
            .collect();
        counts.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        let (name, best) = *counts.first()?;
        let next = counts.get(1).map_or(0.0, |(_, count)| *count);
        (best >= USUAL_FROM && best >= next * USUAL_LEAD).then_some(name)
    }

    /// What `name` adds to its rank for `kind` (in the health score's units): its count, and a
    /// lead over every other source of its standing when it's the usual one.
    pub fn weight(&self, kind: WatchKind, name: &str, now: i64) -> f64 {
        let usual = self.usual(kind, now) == Some(name);
        self.count(kind, name, now) * WEIGHT + if usual { USUAL_BONUS } else { 0.0 }
    }

    /// The names chosen for `kind`, most first (for tests and the sheet).
    pub fn ranked(&self, kind: WatchKind, now: i64) -> Vec<String> {
        let mut names: Vec<(String, Reverse<i64>)> = self
            .of(kind)
            .iter()
            .map(|(name, habit)| (name.clone(), Reverse((faded(habit, now) * 1000.0) as i64)))
            .collect();
        names.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        names.into_iter().map(|(name, _)| name).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 24 * 60 * 60 * 1000;

    #[test]
    fn the_kind_comes_from_the_type_or_the_episodes() {
        assert_eq!(WatchKind::of(Some("movie"), 1), WatchKind::Movie);
        assert_eq!(WatchKind::of(Some("series"), 1), WatchKind::Series);
        assert_eq!(WatchKind::of(Some("anime"), 12), WatchKind::Series);
        assert_eq!(WatchKind::of(None, 8), WatchKind::Series);
        assert_eq!(WatchKind::of(None, 1), WatchKind::Movie);
    }

    #[test]
    fn a_source_chosen_again_and_again_becomes_the_usual_one_for_its_kind() {
        let mut habits = SourceHabits::default();
        // Picked once: not a habit yet.
        habits.note(WatchKind::Movie, "Alpha", PICKED, 0);
        assert_eq!(habits.usual(WatchKind::Movie, 0), None);
        // Watched through after that: Alpha is the usual source for movies, not for series.
        habits.note(WatchKind::Movie, "Alpha", WATCHED, DAY);
        assert_eq!(habits.usual(WatchKind::Movie, DAY), Some("Alpha"));
        assert_eq!(habits.usual(WatchKind::Series, DAY), None);
        assert!(habits.weight(WatchKind::Movie, "Alpha", DAY) > USUAL_BONUS);
        assert_eq!(habits.weight(WatchKind::Series, "Alpha", DAY), 0.0);
    }

    #[test]
    fn two_sources_chosen_alike_make_no_usual_one() {
        let mut habits = SourceHabits::default();
        for day in 0..3 {
            habits.note(WatchKind::Series, "Alpha", WATCHED, day * DAY);
            habits.note(WatchKind::Series, "Beta", WATCHED, day * DAY + 1);
        }
        assert_eq!(habits.usual(WatchKind::Series, 3 * DAY), None);
        // Each still counts for its own rank.
        assert!(habits.weight(WatchKind::Series, "Alpha", 3 * DAY) > 1.0);
    }

    #[test]
    fn habits_fade_and_a_new_one_takes_over() {
        let mut habits = SourceHabits::default();
        for day in 0..4 {
            habits.note(WatchKind::Series, "Old", WATCHED, day * DAY);
        }
        assert_eq!(habits.usual(WatchKind::Series, 4 * DAY), Some("Old"));
        // Two months later, a few choices of another source take over.
        let later = 64 * DAY;
        for step in 0..3 {
            habits.note(WatchKind::Series, "New", PICKED, later + step);
        }
        assert_eq!(habits.usual(WatchKind::Series, later + 3), Some("New"));
        assert_eq!(habits.ranked(WatchKind::Series, later + 3), ["New", "Old"]);
        // Four months without choosing it: forgotten once anything is noted.
        habits.note(WatchKind::Series, "New", PICKED, later + 121 * DAY);
        assert_eq!(
            habits.count(WatchKind::Series, "Old", later + 121 * DAY),
            0.0
        );
    }

    #[test]
    fn kept_as_json() {
        let mut habits = SourceHabits::default();
        habits.note(WatchKind::Movie, "Alpha", PICKED, 5);
        let text = serde_json::to_string(&habits).unwrap();
        let back: SourceHabits = serde_json::from_str(&text).unwrap();
        assert_eq!(back, habits);
        // Missing or unknown parts read as nothing.
        let empty: SourceHabits = serde_json::from_str("{}").unwrap();
        assert_eq!(empty, SourceHabits::default());
    }
}
