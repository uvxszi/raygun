//! Telling links that play the title from those that don't: trailers, teasers and samples by
//! name, and links far shorter than the title (placeholder clips, "this video isn't available"
//! videos) by the length the player measured, against what the other sources' links agree on or
//! the title's runtime. Nothing is judged without such a measure: a lone link of unknown length
//! always plays.

use std::collections::{HashMap, HashSet};

use raygun_model::SHORT_VIDEO_SHARE;

/// Against a runtime from a title's details (which may count a series' episodes loosely), a link
/// is short only under this share of it.
pub const RUNTIME_SHORT_SHARE: f64 = 0.25;

/// Lengths within this share of the median agree.
const AGREE_WITHIN: f64 = 0.15;

/// A link must last this long for its end to count as having played (a clip that ends sooner
/// isn't a title).
pub const SHORTEST_TITLE_MS: i64 = 60_000;

/// Sizes say a link is a sample only when it's under this share of the others' median size, and
/// under [`SAMPLE_BYTES`].
const SAMPLE_SIZE_SHARE: f64 = 1.0 / 30.0;
const SAMPLE_BYTES: u64 = 200 * 1024 * 1024;

/// Sizes are compared only with this many links of known size.
const SIZES_TO_COMPARE: usize = 3;

/// What the Sources toggles say (`VideoPlayerPrefs::leave_out_trailers`/`leave_out_short`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PickPrefs {
    pub leave_out_trailers: bool,
    pub leave_out_short: bool,
}

impl Default for PickPrefs {
    fn default() -> Self {
        Self {
            leave_out_trailers: true,
            leave_out_short: true,
        }
    }
}

impl PickPrefs {
    pub fn of(prefs: &raygun_model::VideoPlayerPrefs) -> Self {
        Self {
            leave_out_trailers: prefs.leave_out_trailers,
            leave_out_short: prefs.leave_out_short,
        }
    }
}

/// Why a link isn't played by itself (it can still be picked in Sources).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftOut {
    /// Its source is turned off for this title.
    Off,
    /// Named a trailer, teaser, sample or promo.
    Trailer,
    /// Far shorter than the title (or a sample by its size).
    Short,
}

impl LeftOut {
    pub fn key(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Trailer => "trailer",
            Self::Short => "short",
        }
    }
}

/// How long the title is expected to last.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Expected {
    /// Several sources' links agree (their median), or a link of it played before.
    Agreed(i64),
    /// The title's details say so.
    Runtime(i64),
}

impl Expected {
    pub fn ms(self) -> i64 {
        match self {
            Self::Agreed(ms) | Self::Runtime(ms) => ms,
        }
    }

    /// Whether a link of `length_ms` is far shorter than this.
    pub fn finds_short(self, length_ms: i64) -> bool {
        let share = match self {
            Self::Agreed(_) => SHORT_VIDEO_SHARE,
            Self::Runtime(_) => RUNTIME_SHORT_SHARE,
        };
        (length_ms as f64) < self.ms() as f64 * share
    }
}

/// Lengths further than this share from what sources agree on don't match the title.
const MATCH_WITHIN: f64 = 0.2;

/// Whether `length_ms` doesn't match what several sources agree on (a runtime is too loose to
/// say).
pub fn length_differs(expected: Expected, length_ms: i64) -> bool {
    match expected {
        Expected::Agreed(ms) => ((length_ms - ms).abs() as f64) > ms as f64 * MATCH_WITHIN,
        Expected::Runtime(_) => false,
    }
}

/// What a session knows of lengths: the title's runtime, what played before, and what the player
/// measured for each link (by address, with its source).
#[derive(Debug, Clone, Default)]
pub struct Picking {
    pub prefs: PickPrefs,
    pub runtime_ms: Option<i64>,
    /// The length of the link that played this title (or show) before.
    pub remembered_ms: Option<i64>,
    /// Measured lengths by link address: (source, ms).
    pub lengths: HashMap<String, (String, i64)>,
    /// Sources turned off for this title, by name.
    pub off_here: HashSet<String>,
    /// Links left for a clearly better one while they played well: tried again only when no
    /// other link is left.
    pub left: HashSet<(usize, usize)>,
    /// Smart Subtitles is on (`smart_subtitles`): sound from other sources says whether it fits.
    pub smart: bool,
    /// The title's name: its own words don't make a link a trailer.
    pub title: String,
}

impl Picking {
    /// What the title is expected to last, leaving out `except` (the link being judged): several
    /// sources agreeing first, then what played before, then the runtime.
    pub fn expected(&self, except: Option<&str>) -> Option<Expected> {
        let measured = self
            .lengths
            .iter()
            .filter(|(url, _)| except != Some(url.as_str()))
            .map(|(_, (source, ms))| (source.as_str(), *ms));
        agreed_length(measured)
            .map(Expected::Agreed)
            .or(self.remembered_ms.map(Expected::Agreed))
            .or(self.runtime_ms.map(Expected::Runtime))
    }

    /// Whether the link at `url` measured far shorter than the title.
    pub fn measured_short(&self, url: &str) -> bool {
        let Some((_, length)) = self.lengths.get(url) else {
            return false;
        };
        self.expected(Some(url))
            .is_some_and(|expected| expected.finds_short(*length))
    }

    /// Why a link of `source` isn't played by itself, if it isn't: its source is off here, its
    /// name says it's a trailer, it measured far shorter than the title, or its size says it's a
    /// sample (`sizes`: the session's links' sizes).
    pub fn left_out(
        &self,
        source: &str,
        label: &str,
        url: &str,
        size: Option<u64>,
        sizes: &[u64],
    ) -> Option<LeftOut> {
        if self.off_here.contains(source) {
            return Some(LeftOut::Off);
        }
        if self.prefs.leave_out_trailers && names_trailer(label, &self.title) {
            return Some(LeftOut::Trailer);
        }
        let short = self.measured_short(url) || size.is_some_and(|size| sized_sample(size, sizes));
        (self.prefs.leave_out_short && short).then_some(LeftOut::Short)
    }
}

/// The length several sources agree on: each source's longest link counts once; with two or more
/// sources, the median (the higher of the middle two), when another source is within
/// [`AGREE_WITHIN`] of it. `None` with fewer sources or no agreement.
pub fn agreed_length<'a>(measured: impl Iterator<Item = (&'a str, i64)>) -> Option<i64> {
    let mut by_source: HashMap<&str, i64> = HashMap::new();
    for (source, ms) in measured.filter(|(_, ms)| *ms > 0) {
        let longest = by_source.entry(source).or_insert(ms);
        *longest = (*longest).max(ms);
    }
    if by_source.len() < 2 {
        return None;
    }
    let mut lengths: Vec<i64> = by_source.into_values().collect();
    lengths.sort_unstable();
    let median = lengths[lengths.len() / 2];
    let near = |ms: i64| ((ms - median).abs() as f64) <= median as f64 * AGREE_WITHIN;
    (lengths.iter().filter(|ms| near(**ms)).count() >= 2).then_some(median)
}

/// Whether a link's name says it isn't the title itself: a trailer, teaser, sample, promo or
/// featurette, a word the title's own name doesn't have ("Trailer Park Boys").
pub fn names_trailer(label: &str, title: &str) -> bool {
    let in_title = |word: &str| {
        title
            .split(|character: char| !character.is_alphanumeric())
            .any(|own| own.eq_ignore_ascii_case(word))
    };
    const WORDS: [&str; 6] = [
        "trailer",
        "trailers",
        "teaser",
        "sample",
        "promo",
        "featurette",
    ];
    label
        .split(|character: char| !character.is_alphanumeric())
        .any(|word| WORDS.iter().any(|known| word.eq_ignore_ascii_case(known)) && !in_title(word))
}

/// Whether a file of `size` bytes is a sample beside the others' `sizes`: with a few of known
/// size, a tiny share of their median.
pub fn sized_sample(size: u64, sizes: &[u64]) -> bool {
    let mut known: Vec<u64> = sizes.iter().copied().filter(|size| *size > 0).collect();
    if known.len() < SIZES_TO_COMPARE || size == 0 {
        return false;
    }
    known.sort_unstable();
    let median = known[known.len() / 2];
    size < SAMPLE_BYTES && (size as f64) < median as f64 * SAMPLE_SIZE_SHARE
}

/// A runtime as details give it ("45 min", "2h 10min", "1h", "130", "PT2H10M"), in ms.
pub fn parse_runtime(text: &str) -> Option<i64> {
    let lower = text.trim().to_ascii_lowercase();
    let lower = lower.strip_prefix("pt").unwrap_or(&lower);
    let chars: Vec<char> = lower.chars().collect();
    let mut minutes = 0.0;
    let mut found = false;
    let mut at = 0;
    while at < chars.len() {
        if !chars[at].is_ascii_digit() {
            at += 1;
            continue;
        }
        let start = at;
        while at < chars.len() && (chars[at].is_ascii_digit() || chars[at] == '.') {
            at += 1;
        }
        let value: f64 = chars[start..at].iter().collect::<String>().parse().ok()?;
        while at < chars.len() && chars[at] == ' ' {
            at += 1;
        }
        // The unit's first letter: hours, minutes (also none), seconds.
        let unit = chars.get(at).copied().filter(char::is_ascii_alphabetic);
        while at < chars.len() && chars[at].is_ascii_alphabetic() {
            at += 1;
        }
        minutes += match unit {
            Some('h') => value * 60.0,
            Some('s') => value / 60.0,
            _ => value,
        };
        found = true;
    }
    (found && minutes > 0.0).then(|| (minutes * 60_000.0).round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: i64 = 60_000;

    #[test]
    fn several_sources_must_agree_on_a_length() {
        assert_eq!(agreed_length([("A", 90 * MIN)].into_iter()), None);
        // One source's links count once (its longest).
        assert_eq!(
            agreed_length([("A", 90 * MIN), ("A", 2 * MIN)].into_iter()),
            None
        );
        assert_eq!(
            agreed_length([("A", 90 * MIN), ("B", 92 * MIN)].into_iter()),
            Some(92 * MIN)
        );
        // Two that disagree say nothing.
        assert_eq!(
            agreed_length([("A", 90 * MIN), ("B", 2 * MIN)].into_iter()),
            None
        );
        // Most agree, one doesn't: the median stands.
        assert_eq!(
            agreed_length([("A", 90 * MIN), ("B", 91 * MIN), ("C", MIN / 6)].into_iter()),
            Some(90 * MIN)
        );
    }

    #[test]
    fn trailers_and_samples_are_known_by_name() {
        assert!(names_trailer("Official Trailer 1080p", "Dune"));
        assert!(names_trailer("movie.SAMPLE.mkv", "Movie"));
        assert!(names_trailer("Teaser · 4K", "Dune"));
        assert!(!names_trailer(
            "Torrentio 1080p · The.Trailer.Park.Boys.S01E01",
            "Trailer Park Boys"
        ));
        assert!(!names_trailer("Samples of Life 2160p", "Samples of Life"));
        assert!(!names_trailer("1080p WEB-DL", "Dune"));
    }

    #[test]
    fn short_links_need_others_that_agree_or_a_runtime() {
        let mut picking = Picking::default();
        picking.lengths.insert("a".into(), ("A".into(), 90 * MIN));
        picking.lengths.insert("t".into(), ("T".into(), 2 * MIN));
        // One other source's length isn't enough to call the trailer short.
        assert!(!picking.measured_short("t"));
        picking.lengths.insert("b".into(), ("B".into(), 91 * MIN));
        assert!(picking.measured_short("t"));
        assert!(!picking.measured_short("a"));
        // An unmeasured link is never short.
        assert!(!picking.measured_short("x"));
    }

    #[test]
    fn a_runtime_judges_alone_but_loosely() {
        let mut picking = Picking {
            runtime_ms: Some(45 * MIN),
            ..Picking::default()
        };
        picking.lengths.insert("p".into(), ("P".into(), MIN / 3));
        picking.lengths.insert("e".into(), ("E".into(), 22 * MIN));
        assert!(picking.measured_short("p"));
        // An anime episode under a runtime that counts loosely isn't a placeholder.
        assert!(!picking.measured_short("e"));
    }

    #[test]
    fn what_played_before_judges_alone() {
        let mut picking = Picking {
            remembered_ms: Some(100 * MIN),
            ..Picking::default()
        };
        picking.lengths.insert("p".into(), ("P".into(), 3 * MIN));
        picking.lengths.insert("c".into(), ("C".into(), 95 * MIN));
        assert!(picking.measured_short("p"));
        assert!(!picking.measured_short("c"));
    }

    #[test]
    fn left_out_follows_the_switches() {
        let mut picking = Picking::default();
        assert_eq!(
            picking.left_out("A", "Trailer", "u", None, &[]),
            Some(LeftOut::Trailer)
        );
        picking.prefs.leave_out_trailers = false;
        assert_eq!(picking.left_out("A", "Trailer", "u", None, &[]), None);
        let sizes = [4_000_000_000, 5_000_000_000, 6_000_000_000];
        assert_eq!(
            picking.left_out("A", "1080p", "u", Some(30_000_000), &sizes),
            Some(LeftOut::Short)
        );
        // A small encode isn't a sample.
        assert_eq!(
            picking.left_out("A", "480p", "u", Some(400_000_000), &sizes),
            None
        );
        picking.prefs.leave_out_short = false;
        assert_eq!(
            picking.left_out("A", "1080p", "u", Some(30_000_000), &sizes),
            None
        );
        picking.off_here.insert("A".into());
        assert_eq!(
            picking.left_out("A", "1080p", "u", None, &[]),
            Some(LeftOut::Off)
        );
    }

    #[test]
    fn runtimes_read_as_details_write_them() {
        assert_eq!(parse_runtime("45 min"), Some(45 * MIN));
        assert_eq!(parse_runtime("2h 10min"), Some(130 * MIN));
        assert_eq!(parse_runtime("1h"), Some(60 * MIN));
        assert_eq!(parse_runtime("130"), Some(130 * MIN));
        assert_eq!(parse_runtime("PT1H30M"), Some(90 * MIN));
        assert_eq!(parse_runtime("N/A"), None);
        assert_eq!(parse_runtime(""), None);
    }
}
