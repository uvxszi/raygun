//! Stream resolution (ARCHITECTURE §6.1, M14): matching `music.stream` candidates against the track
//! being played, ranking them, and keying the cache. Pure logic; the app does the I/O.

use std::collections::{HashMap, HashSet};

use raygun_model::{MediaSource, TrackQuery};
use unicode_normalization::UnicodeNormalization;

#[cfg(test)]
mod tests;

/// Candidates scoring below this are dropped: skipping a track beats playing the wrong song.
pub const MIN_SCORE: i32 = 35;

/// What a candidate that played this track before earns.
const VERIFIED_BONUS: i32 = 50;

/// Matches this close to the best one count as the same recording when choosing by quality
/// (M46.18); the verified bonus doesn't count here, so a better bitrate can replace a pick.
const QUALITY_BAND: i32 = 6;

/// What a stream of unknown bitrate is taken to be.
const UNKNOWN_KBPS: u32 = 160;

/// What lossless streams count as (CD audio).
pub const LOSSLESS_KBPS: u32 = 1411;

/// The bitrate a resolution aims for (M46.18): the Audio quality setting for streaming, mobile
/// data or downloads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum QualityTarget {
    /// Whatever matches best.
    #[default]
    Any,
    /// Nearest this many kbps.
    Kbps(u32),
    /// The highest there is, lossless included.
    Best,
}

impl QualityTarget {
    /// How far `bitrate` is from what's wanted; 0 is a perfect fit. For a kbps target it's the
    /// percentage by which the larger exceeds the smaller, so 64 vs 128 and 256 vs 128 weigh alike.
    pub fn distance(self, bitrate: Option<u32>) -> u32 {
        let bitrate = bitrate.unwrap_or(UNKNOWN_KBPS).max(1);
        match self {
            Self::Any => 0,
            Self::Best => LOSSLESS_KBPS.saturating_sub(bitrate),
            Self::Kbps(target) => {
                let target = target.max(1);
                bitrate.max(target) * 100 / bitrate.min(target) - 100
            }
        }
    }

    /// Whether a stream remembered from an earlier play is close enough to keep using: within
    /// 60 % of a kbps target; for Best, lossless or 256 kbps and up. Unknown bitrates fit.
    pub fn fits(self, bitrate: Option<u32>) -> bool {
        let Some(bitrate) = bitrate else {
            return true;
        };
        match self {
            Self::Any => true,
            Self::Best => bitrate >= 256,
            Self::Kbps(_) => self.distance(Some(bitrate)) <= 60,
        }
    }

    /// The target as `music.stream` providers get it: kbps, `None` for the best there is.
    pub fn kbps(self) -> Option<u32> {
        match self {
            Self::Kbps(kbps) => Some(kbps),
            Self::Any | Self::Best => None,
        }
    }
}

/// The bitrate a provider's quality label names: "320 kbps", "128k MP3", "64Kbps MP3"; FLAC,
/// ALAC, WAV and "lossless" count as [`LOSSLESS_KBPS`].
pub fn label_kbps(label: &str) -> Option<u32> {
    let lower = label.to_lowercase();
    if ["flac", "lossless", "alac", "wav", "hi-res", "hires"]
        .iter()
        .any(|word| lower.contains(word))
    {
        return Some(LOSSLESS_KBPS);
    }
    let bytes = lower.as_bytes();
    let mut start = None;
    for (index, byte) in bytes.iter().enumerate() {
        if byte.is_ascii_digit() {
            start.get_or_insert(index);
            continue;
        }
        if let Some(from) = start.take() {
            let rest = lower[index..].trim_start();
            if rest.starts_with('k')
                && let Ok(kbps) = lower[from..index].parse::<u32>()
                && (8..=9_999).contains(&kbps)
            {
                return Some(kbps);
            }
        }
    }
    None
}

/// Variants a listener rarely wants unless they asked for them.
const UNWANTED_VARIANTS: &[&str] = &[
    "live",
    "cover",
    "karaoke",
    "instrumental",
    "remix",
    "sped up",
    "slowed",
    "8d",
    "nightcore",
    "reaction",
    "acapella",
    "a cappella",
];

/// A stream a provider offered for a track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub provider_id: String,
    pub provider_name: String,
    /// The provider's own id, passed back to its `resolve` method.
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    pub duration_ms: Option<u32>,
    pub quality: Option<String>,
    /// The stream's bitrate, as the provider gives it (M46.18).
    pub bitrate_kbps: Option<u32>,
    /// Inline stream, when the provider already knows it.
    pub source: Option<MediaSource>,
}

impl Candidate {
    pub fn key(&self) -> CandidateKey {
        CandidateKey {
            provider_id: self.provider_id.clone(),
            candidate_id: self.id.clone(),
        }
    }

    /// The bitrate given, else the one its quality label names.
    pub fn bitrate(&self) -> Option<u32> {
        self.bitrate_kbps
            .or_else(|| self.quality.as_deref().and_then(label_kbps))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CandidateKey {
    pub provider_id: String,
    pub candidate_id: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProviderRecord {
    pub successes: u32,
    pub failures: u32,
}

/// What earlier plays taught us about this track and its providers.
#[derive(Debug, Clone, Default)]
pub struct History {
    /// Candidates that played this track before.
    pub verified: HashSet<CandidateKey>,
    /// Candidates that failed recently (skipped entirely).
    pub failed: HashSet<CandidateKey>,
    pub providers: HashMap<String, ProviderRecord>,
}

/// The stream chosen for a track, ready to play.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pick {
    pub key: CandidateKey,
    pub track_key: String,
    pub title: String,
    /// The candidate's artist, when its source named one (fresh picks only: the cache keeps
    /// titles).
    pub artist: Option<String>,
    pub provider_name: String,
    pub source: MediaSource,
    /// Unix milliseconds after which `source` stops working, if known.
    pub expires_at: Option<i64>,
    /// Came from the cache of verified picks rather than a fresh search.
    pub from_cache: bool,
    /// The stream's bitrate, when known (M46.18).
    pub bitrate_kbps: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scored {
    pub candidate: Candidate,
    pub score: i32,
}

/// Lower case, accents folded, `&` spelled out, everything but letters and digits turned into
/// single spaces. Bracketed parts and `feat.` credits are kept; see [`core_title`].
pub fn normalize(text: &str) -> String {
    let folded: String = text
        .nfd()
        .filter(|c| !('\u{300}'..='\u{36f}').contains(c))
        .collect::<String>()
        .to_lowercase()
        .replace('&', " and ");
    folded
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// A title without its decorations: bracketed parts (`(Remastered 2011)`, `[Official Video]`) and
/// trailing `feat.` credits are dropped, then the rest is normalized.
pub fn core_title(title: &str) -> String {
    let mut plain = String::with_capacity(title.len());
    let mut depth = 0usize;
    for c in title.chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => plain.push(c),
            _ => {}
        }
    }
    let normalized = normalize(&plain);
    let words: Vec<&str> = normalized.split(' ').collect();
    let cut = words
        .iter()
        .position(|word| matches!(*word, "feat" | "ft" | "featuring"))
        .unwrap_or(words.len());
    words[..cut].join(" ")
}

/// The cache key of a track: normalized artist and core title.
pub fn track_key(query: &TrackQuery) -> String {
    format!(
        "{}|{}",
        normalize(query.artist.as_deref().unwrap_or_default()),
        core_title(&query.title)
    )
}

fn tokens(text: &str) -> HashSet<&str> {
    text.split(' ').filter(|word| !word.is_empty()).collect()
}

/// Whether `phrase` appears as whole words in normalized `text`.
fn contains_phrase(text: &str, phrase: &str) -> bool {
    format!(" {text} ").contains(&format!(" {phrase} "))
}

/// How well `candidate` matches `query`; higher is better. See the M14 spec for the weights.
pub fn score(query: &TrackQuery, candidate: &Candidate, history: &History) -> i32 {
    let query_title = core_title(&query.title);
    let query_artist = normalize(query.artist.as_deref().unwrap_or_default());
    let candidate_artist = normalize(candidate.artist.as_deref().unwrap_or_default());
    let full_title = normalize(&candidate.title);
    let artist_words = tokens(&query_artist);

    // Title: how much of the query title the candidate covers, and how little else it adds.
    // Words of the artist's name (as in "Artist - Title") don't count as extra.
    let wanted = tokens(&query_title);
    let candidate_core = core_title(&candidate.title);
    let offered: HashSet<&str> = tokens(&candidate_core)
        .into_iter()
        .filter(|word| !artist_words.contains(word))
        .collect();
    let mut total = 0f64;
    if !wanted.is_empty() {
        let shared = wanted.intersection(&offered).count() as f64;
        let recall = shared / wanted.len() as f64;
        let precision = if offered.is_empty() {
            0.0
        } else {
            shared / offered.len() as f64
        };
        total += 40.0 * (0.7 * recall + 0.3 * precision);
    }

    // Artist.
    if !artist_words.is_empty() {
        let named_in = |text: &str| {
            let words = tokens(text);
            let shared = artist_words.intersection(&words).count();
            shared * 2 >= artist_words.len().max(1)
        };
        if !candidate_artist.is_empty() {
            total += if named_in(&candidate_artist) || contains_phrase(&full_title, &query_artist) {
                20.0
            } else {
                -10.0
            };
        } else if named_in(&full_title) {
            total += 15.0;
        } else {
            // Nothing says it's this artist's: a one-word title ("Swim") would match anyone's.
            total -= 10.0;
        }
    }

    // Duration.
    if let (Some(wanted), Some(offered)) = (query.duration_ms, candidate.duration_ms) {
        let off = (i64::from(wanted) - i64::from(offered)).unsigned_abs();
        total += match off {
            0..=3_000 => 25.0,
            3_001..=8_000 => 15.0,
            8_001..=20_000 => 5.0,
            20_001..=30_000 => 0.0,
            _ => -30.0,
        };
    }

    // Variants the query didn't ask for.
    let asked = normalize(&query.title);
    for variant in UNWANTED_VARIANTS {
        if contains_phrase(&full_title, variant) && !contains_phrase(&asked, variant) {
            total -= 25.0;
        }
    }

    // Kind: the YouTube embed is the fallback (it pauses in the background on Android).
    total += match &candidate.source {
        Some(MediaSource::YouTube { .. }) => 0.0,
        Some(_) => 10.0,
        None => 5.0,
    };

    // Provider record: up to ±10 once it has a few plays behind it.
    if let Some(record) = history.providers.get(&candidate.provider_id) {
        let plays = record.successes + record.failures;
        if plays > 0 {
            let rate = f64::from(record.successes) / f64::from(plays);
            let confidence = (f64::from(plays) / 5.0).min(1.0);
            total += (rate - 0.5) * 20.0 * confidence;
        }
    }

    if history.verified.contains(&candidate.key()) {
        total += f64::from(VERIFIED_BONUS);
    }
    total.round() as i32
}

/// Scores every candidate, drops failed ones and poor matches, best first. Ties keep the
/// providers' own order.
pub fn rank(query: &TrackQuery, candidates: Vec<Candidate>, history: &History) -> Vec<Scored> {
    rank_for(query, candidates, history, QualityTarget::Any)
}

/// [`rank`], then the matches as good as the best one (within [`QUALITY_BAND`], not counting
/// the bonus for having played before) ordered by how near their bitrate is to `target`
/// (M46.18): the same song at 64 kbps on mobile data, lossless when Best is asked for.
pub fn rank_for(
    query: &TrackQuery,
    candidates: Vec<Candidate>,
    history: &History,
    target: QualityTarget,
) -> Vec<Scored> {
    let mut scored = rank_by_match(query, candidates, history);
    if target == QualityTarget::Any {
        return scored;
    }
    let base = |scored: &Scored| {
        scored.score
            - if history.verified.contains(&scored.candidate.key()) {
                VERIFIED_BONUS
            } else {
                0
            }
    };
    let Some(top) = scored.iter().map(base).max() else {
        return scored;
    };
    let (mut close, rest): (Vec<Scored>, Vec<Scored>) = scored
        .drain(..)
        .partition(|scored| base(scored) >= top - QUALITY_BAND);
    close.sort_by_key(|scored| {
        (
            target.distance(scored.candidate.bitrate()),
            std::cmp::Reverse(scored.score),
        )
    });
    close.extend(rest);
    close
}

fn rank_by_match(query: &TrackQuery, candidates: Vec<Candidate>, history: &History) -> Vec<Scored> {
    let mut scored: Vec<Scored> = candidates
        .into_iter()
        .filter(|candidate| !history.failed.contains(&candidate.key()))
        .map(|candidate| Scored {
            score: score(query, &candidate, history),
            candidate,
        })
        .filter(|scored| scored.score >= MIN_SCORE)
        .collect();
    scored.sort_by_key(|scored| std::cmp::Reverse(scored.score));
    scored
}
