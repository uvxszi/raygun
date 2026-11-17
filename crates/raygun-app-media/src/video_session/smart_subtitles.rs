//! Smart Subtitles (a Smart Chip): the subtitle tracks a video gets from its sources are compared
//! with the others of their language, so the ones most like the rest are listed first and marked
//! "Matching", and one unlike all the others says it might not match (another cut, another
//! episode, a bad sync). The comparison is quick: each track's words are grouped into time
//! windows, and two tracks are as alike as the words they share in the same windows (with a
//! window's slack either way) and as many of their lines start together. Sound from other sources
//! is compared by its stream's length.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex};

use raygun_net::subtitles::Cue;
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};

use super::VideoSessions;

/// Whether Smart Subtitles can be used here: its Smart Chip installed and on. Its switch shows in
/// the players only while this holds; with it switched on too (`smart_subtitles_on`), tracks are
/// compared.
pub fn smart_subtitles_available(app: &AppHandle) -> bool {
    raygun_app_base::chips::enabled(app, raygun_model::ChipId::SmartSubtitles)
}

/// Whether tracks are compared now: available, and switched on
/// (`VideoPlayerPrefs::smart_subtitles`).
pub async fn smart_subtitles_on(app: &AppHandle) -> bool {
    if !smart_subtitles_available(app) {
        return false;
    }
    let db = app.state::<crate::startup::AppDb>().0.clone();
    crate::settings::load(&db)
        .await
        .is_ok_and(|settings| settings.video_player.smart_subtitles)
}

/// Words are grouped into windows this long.
const WINDOW_MS: u64 = 8_000;

/// Lines start together within this.
const START_SLACK_MS: u64 = 1_000;

/// Tracks this alike (or more) match.
const MATCHING: f64 = 0.45;

/// A track under this likeness to every other doesn't match them.
const UNLIKE: f64 = 0.15;

/// Text likeness and timing likeness weigh this much each.
const TEXT_WEIGHT: f64 = 0.7;

/// How many subtitle files are read for one video.
const MOST_FILES: usize = 24;

/// A subtitle track and its lines.
#[derive(Debug, Clone)]
pub struct TrackCues {
    /// The address the player knows it by.
    pub key: String,
    pub language: Option<String>,
    pub cues: Arc<Vec<Cue>>,
    /// Read only to compare the others with (another stream's own track): not ranked.
    pub reference: bool,
}

/// What Smart Subtitles says of a track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    Matching,
    MightNotMatch,
}

impl Fit {
    pub fn key(self) -> &'static str {
        match self {
            Self::Matching => "matching",
            Self::MightNotMatch => "mightNotMatch",
        }
    }
}

/// A track's place among those of its language (0 first), and what it's marked.
#[derive(Debug, Clone, PartialEq)]
pub struct Ranked {
    pub key: String,
    pub rank: usize,
    pub score: f64,
    pub fit: Option<Fit>,
}

/// A language as one code: `en` for "en", "en-US", "eng" and "English".
pub fn language_key(language: Option<&str>) -> Option<String> {
    let text = language?.trim().to_lowercase();
    let base = text.split(['-', '_', ' ']).next().unwrap_or_default();
    if base.is_empty() {
        return None;
    }
    const NAMES: [(&str, &str); 40] = [
        ("eng", "en"),
        ("english", "en"),
        ("fre", "fr"),
        ("fra", "fr"),
        ("french", "fr"),
        ("ger", "de"),
        ("deu", "de"),
        ("german", "de"),
        ("spa", "es"),
        ("spanish", "es"),
        ("ita", "it"),
        ("italian", "it"),
        ("por", "pt"),
        ("portuguese", "pt"),
        ("pob", "pt"),
        ("rus", "ru"),
        ("russian", "ru"),
        ("jpn", "ja"),
        ("japanese", "ja"),
        ("chi", "zh"),
        ("zho", "zh"),
        ("chinese", "zh"),
        ("kor", "ko"),
        ("korean", "ko"),
        ("ara", "ar"),
        ("arabic", "ar"),
        ("dut", "nl"),
        ("nld", "nl"),
        ("dutch", "nl"),
        ("pol", "pl"),
        ("polish", "pl"),
        ("tur", "tr"),
        ("turkish", "tr"),
        ("swe", "sv"),
        ("gre", "el"),
        ("ell", "el"),
        ("heb", "he"),
        ("hun", "hu"),
        ("cze", "cs"),
        ("ces", "cs"),
    ];
    Some(
        NAMES
            .iter()
            .find(|(name, _)| *name == base)
            .map_or(base, |(_, code)| *code)
            .to_owned(),
    )
}

/// Words every line may say, which tell nothing of what it says.
const COMMON: [&str; 24] = [
    "the", "and", "you", "for", "that", "this", "with", "are", "was", "have", "les", "des", "une",
    "der", "die", "das", "und", "ein", "que", "los", "las", "del", "con", "non",
];

/// The words of a line, folded: letters and digits in runs of three or more (but the commonest
/// words), and each character of scripts written without spaces (Chinese, Japanese, Korean).
fn words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut Vec<String>| {
        if run.chars().count() >= 3 && !COMMON.contains(&run.as_str()) {
            out.push(std::mem::take(run));
        } else {
            run.clear();
        }
    };
    for character in text.chars() {
        let wide = matches!(character as u32, 0x3040..=0x30FF | 0x3400..=0x9FFF | 0xAC00..=0xD7AF);
        if wide {
            flush(&mut run, &mut out);
            out.push(character.to_string());
        } else if character.is_alphanumeric() {
            run.extend(character.to_lowercase());
        } else {
            flush(&mut run, &mut out);
        }
    }
    flush(&mut run, &mut out);
    out
}

/// Each window's words.
fn windows(cues: &[Cue]) -> Vec<HashSet<String>> {
    let mut out: Vec<HashSet<String>> = Vec::new();
    for cue in cues {
        let at = (cue.start_ms / WINDOW_MS) as usize;
        if at >= out.len() {
            out.resize_with(at + 1, HashSet::new);
        }
        // Markup isn't said.
        let text = cue
            .text
            .replace("<i>", " ")
            .replace("</i>", " ")
            .replace("<b>", " ")
            .replace("</b>", " ");
        out[at].extend(words(&text));
    }
    out
}

/// How many of `a`'s words `b` says in the same window or one either side, as a share of `a`'s.
fn text_overlap(a: &[HashSet<String>], b: &[HashSet<String>]) -> f64 {
    let mut said = 0usize;
    let mut shared = 0usize;
    for (at, words) in a.iter().enumerate() {
        said += words.len();
        shared += words
            .iter()
            .filter(|word| {
                [at.checked_sub(1), Some(at), Some(at + 1)]
                    .into_iter()
                    .flatten()
                    .filter_map(|near| b.get(near))
                    .any(|near| near.contains(*word))
            })
            .count();
    }
    if said == 0 {
        0.0
    } else {
        shared as f64 / said as f64
    }
}

/// The share of `a`'s lines that start within [`START_SLACK_MS`] of one of `b`'s.
fn timing_overlap(a: &[Cue], b: &[Cue]) -> f64 {
    if a.is_empty() {
        return 0.0;
    }
    let mut starts: Vec<u64> = b.iter().map(|cue| cue.start_ms).collect();
    starts.sort_unstable();
    let near = a
        .iter()
        .filter(|cue| {
            let at = starts
                .partition_point(|start| *start < cue.start_ms.saturating_sub(START_SLACK_MS));
            starts
                .get(at)
                .is_some_and(|start| start.abs_diff(cue.start_ms) <= START_SLACK_MS)
        })
        .count();
    near as f64 / a.len() as f64
}

/// How alike two tracks are, 0 to 1: their words in the same windows and their lines' starts,
/// each way.
pub fn similarity(a: &[Cue], b: &[Cue]) -> f64 {
    let (wa, wb) = (windows(a), windows(b));
    let text = (text_overlap(&wa, &wb) + text_overlap(&wb, &wa)) / 2.0;
    let timing = (timing_overlap(a, b) + timing_overlap(b, a)) / 2.0;
    TEXT_WEIGHT * text + (1.0 - TEXT_WEIGHT) * timing
}

/// Whether a track runs well past the end of a video of `video_ms` (another cut or episode).
fn runs_past(cues: &[Cue], video_ms: i64) -> bool {
    let end = cues.iter().map(|cue| cue.end_ms).max().unwrap_or(0) as i64;
    video_ms > 0 && end > video_ms + video_ms / 20 + 60_000
}

/// The tracks that aren't references, ranked among those of their language: the most like the
/// others first and marked [`Fit::Matching`] when like one at least; one unlike every other while
/// those agree among themselves (or running well past the video's end) marked
/// [`Fit::MightNotMatch`] and put last. A track alone in its language gets nothing.
pub fn rank(tracks: &[TrackCues], video_ms: Option<i64>) -> Vec<Ranked> {
    let mut by_language: HashMap<Option<String>, Vec<usize>> = HashMap::new();
    for (index, track) in tracks.iter().enumerate() {
        by_language
            .entry(language_key(track.language.as_deref()))
            .or_default()
            .push(index);
    }
    let mut out = Vec::new();
    for members in by_language.values() {
        let likeness: HashMap<(usize, usize), f64> = members
            .iter()
            .flat_map(|a| members.iter().map(move |b| (*a, *b)))
            .filter(|(a, b)| a < b)
            .map(|(a, b)| ((a, b), similarity(&tracks[a].cues, &tracks[b].cues)))
            .collect();
        let like = |a: usize, b: usize| likeness[&(a.min(b), a.max(b))];
        let mut ranked: Vec<Ranked> = Vec::new();
        for &index in members.iter().filter(|index| !tracks[**index].reference) {
            let others: Vec<usize> = members
                .iter()
                .copied()
                .filter(|other| *other != index)
                .collect();
            let scores: Vec<f64> = others.iter().map(|other| like(index, *other)).collect();
            let best = scores.iter().copied().fold(0.0, f64::max);
            let mean = if scores.is_empty() {
                0.0
            } else {
                scores.iter().sum::<f64>() / scores.len() as f64
            };
            // The others agree among themselves: one unlike them all is the odd one out.
            let others_agree = others
                .iter()
                .any(|a| others.iter().any(|b| a < b && like(*a, *b) >= MATCHING));
            let past = video_ms.is_some_and(|ms| runs_past(&tracks[index].cues, ms));
            let fit = if past || (others.len() >= 2 && others_agree && best < UNLIKE) {
                Some(Fit::MightNotMatch)
            } else if best >= MATCHING {
                Some(Fit::Matching)
            } else {
                None
            };
            ranked.push(Ranked {
                key: tracks[index].key.clone(),
                rank: 0,
                score: mean,
                fit,
            });
        }
        let order = |fit: Option<Fit>| match fit {
            Some(Fit::Matching) => 0,
            None => 1,
            Some(Fit::MightNotMatch) => 2,
        };
        ranked.sort_by(|a, b| {
            order(a.fit)
                .cmp(&order(b.fit))
                .then(b.score.total_cmp(&a.score))
        });
        for (rank, entry) in ranked.iter_mut().enumerate() {
            entry.rank = rank;
        }
        out.extend(ranked);
    }
    out
}

/// Sound from another source as Smart Subtitles judges it: its stream's length against the
/// video's (within half a percent matches; off by more than 2 % and 3 s might not).
pub fn audio_fit(video_ms: i64, stream_ms: i64) -> Option<&'static str> {
    if video_ms <= 0 || stream_ms <= 0 {
        return None;
    }
    let off = (video_ms - stream_ms).abs();
    if (off as f64) <= video_ms as f64 * 0.005 {
        Some(Fit::Matching.key())
    } else if (off as f64) > video_ms as f64 * 0.02 && off > 3_000 {
        Some(Fit::MightNotMatch.key())
    } else {
        None
    }
}

/// Subtitle files read this run, by address (a few hundred at most).
static READ: LazyLock<Mutex<HashMap<String, Arc<Vec<Cue>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Kept files past this are forgotten.
const MOST_KEPT: usize = 300;

/// A subtitle file's lines, read through the proxy (unpacked and as SubRip); none when it can't be
/// read.
async fn read_cues(client: &reqwest::Client, proxied: &str) -> Arc<Vec<Cue>> {
    if let Some(known) = READ.lock().expect("subtitles poisoned").get(proxied) {
        return known.clone();
    }
    let text = match client.get(proxied).send().await {
        Ok(response) => response.text().await.unwrap_or_default(),
        Err(error) => {
            tracing::debug!(%error, "a subtitle file couldn't be read to compare");
            String::new()
        }
    };
    let cues = Arc::new(raygun_net::subtitles::parse(&text));
    let mut read = READ.lock().expect("subtitles poisoned");
    if read.len() >= MOST_KEPT {
        read.clear();
    }
    read.insert(proxied.to_owned(), cues.clone());
    cues
}

impl VideoSessions {
    /// Smart Subtitles was switched (or its chip): the video playing is compared again, or its
    /// marks cleared.
    pub async fn smart_subtitles_changed(&self) {
        let on = smart_subtitles_on(&self.app).await;
        let generation = {
            let mut guard = self.session.lock().expect("session poisoned");
            let Some(session) = guard.as_mut() else {
                return;
            };
            if session.picking.smart == on {
                return;
            }
            session.picking.smart = on;
            session.generation
        };
        self.push_sources().await;
        self.compare_subtitles(generation);
    }

    /// Compares the subtitle tracks of the link playing (in the background) and tells the player
    /// their order and marks (`subtitleRanks`); with Smart Subtitles off, it clears them.
    pub(super) fn compare_subtitles(&self, generation: u64) {
        let found = {
            let guard = self.session.lock().expect("session poisoned");
            let Some(session) = guard
                .as_ref()
                .filter(|session| session.generation == generation)
            else {
                return;
            };
            let Some((group, link)) = session.current else {
                return;
            };
            let Some(playing) = session
                .groups
                .get(group)
                .and_then(|entry| entry.links.get(link))
            else {
                return;
            };
            let own: HashSet<&str> = playing
                .request
                .subtitles
                .iter()
                .map(|subtitle| subtitle.url.as_str())
                .collect();
            let languages: HashSet<Option<String>> = playing
                .request
                .subtitles
                .iter()
                .map(|subtitle| language_key(subtitle.lang.as_deref()))
                .collect();
            // Other streams' own tracks in those languages: more to compare with.
            let references: Vec<raygun_model::VideoSubtitle> = session
                .groups
                .iter()
                .flat_map(|entry| entry.links.iter())
                .flat_map(|other| other.request.subtitles.iter())
                .filter(|subtitle| !own.contains(subtitle.url.as_str()))
                .filter(|subtitle| languages.contains(&language_key(subtitle.lang.as_deref())))
                .cloned()
                .collect();
            (
                session.picking.smart,
                playing.request.key.clone(),
                playing.request.subtitles.clone(),
                references,
                session.duration_ms,
            )
        };
        let (on, key, subtitles, references, video_ms) = found;
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let items: Vec<Value> = if on && !subtitles.is_empty() {
                compared(&app, &subtitles, &references, video_ms).await
            } else {
                Vec::new()
            };
            let sessions = app.state::<VideoSessions>();
            if !sessions.is_current(generation) {
                return;
            }
            let _ = crate::video::update_player(
                &app,
                &json!({ "type": "subtitleRanks", "id": key, "on": on, "items": items }),
            );
        });
    }
}

/// The player's marks for `subtitles` (by their proxied address), compared with `references`.
async fn compared(
    app: &AppHandle,
    subtitles: &[raygun_model::VideoSubtitle],
    references: &[raygun_model::VideoSubtitle],
    video_ms: Option<i64>,
) -> Vec<Value> {
    let Ok(client) = raygun_net::http_client() else {
        return Vec::new();
    };
    let shown = crate::video::proxied_subtitles(app, subtitles);
    let mut seen = HashSet::new();
    let reference_count = MOST_FILES.saturating_sub(shown.len());
    let wanted: Vec<(String, Option<String>, bool, String)> = shown
        .iter()
        .map(|subtitle| (subtitle.url.clone(), subtitle.lang.clone(), false))
        .chain(
            crate::video::proxied_subtitles(app, references)
                .into_iter()
                .take(reference_count)
                .map(|subtitle| (subtitle.url, subtitle.lang, true)),
        )
        .filter(|(url, _, _)| seen.insert(url.clone()))
        .take(MOST_FILES)
        .map(|(url, language, reference)| {
            // Read as SubRip whatever it is.
            let readable = match url.split_once("?as=") {
                Some((base, _)) => format!("{base}?as=srt"),
                None => format!("{url}?as=srt"),
            };
            (url, language, reference, readable)
        })
        .collect();
    let reads = wanted
        .iter()
        .map(|(_, _, _, readable)| read_cues(&client, readable));
    let all = futures_util::future::join_all(reads).await;
    let tracks: Vec<TrackCues> = wanted
        .into_iter()
        .zip(all)
        .filter(|(_, cues)| !cues.is_empty())
        .map(|((key, language, reference, _), cues)| TrackCues {
            key,
            language,
            cues,
            reference,
        })
        .collect();
    rank(&tracks, video_ms)
        .into_iter()
        .map(|ranked| {
            json!({
                "url": ranked.key,
                "rank": ranked.rank,
                "fit": ranked.fit.map(Fit::key),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(start_s: u64, text: &str) -> Cue {
        Cue {
            start_ms: start_s * 1000,
            end_ms: start_s * 1000 + 2_000,
            text: text.to_owned(),
        }
    }

    fn film(shift_s: u64) -> Vec<Cue> {
        [
            "Where were you last night?",
            "I was at the harbour, waiting for the boat.",
            "The boat never came.",
            "Then we leave at dawn, all of us.",
            "Bring the map and the lantern.",
            "Nobody follows us past the bridge.",
        ]
        .iter()
        .enumerate()
        .map(|(index, line)| cue(10 + index as u64 * 12 + shift_s, line))
        .collect()
    }

    fn other_film() -> Vec<Cue> {
        [
            "Ladies and gentlemen, welcome aboard.",
            "Please fasten your seat belts.",
            "Our flight time today is two hours.",
            "Enjoy the flight.",
        ]
        .iter()
        .enumerate()
        .map(|(index, line)| cue(5 + index as u64 * 20, line))
        .collect()
    }

    fn track(key: &str, language: &str, cues: Vec<Cue>) -> TrackCues {
        TrackCues {
            key: key.into(),
            language: Some(language.into()),
            cues: Arc::new(cues),
            reference: false,
        }
    }

    #[test]
    fn the_same_lines_a_moment_apart_are_alike_and_another_film_isnt() {
        assert!(similarity(&film(0), &film(0)) > 0.99);
        assert!(similarity(&film(0), &film(1)) > 0.9);
        assert!(similarity(&film(0), &other_film()) < UNLIKE);
    }

    #[test]
    fn the_odd_track_out_might_not_match_and_goes_last() {
        let ranked = rank(
            &[
                track("odd", "en", other_film()),
                track("a", "eng", film(0)),
                track("b", "English", film(1)),
            ],
            None,
        );
        let of = |key: &str| {
            ranked
                .iter()
                .find(|entry| entry.key == key)
                .unwrap()
                .clone()
        };
        assert_eq!(of("a").fit, Some(Fit::Matching));
        assert_eq!(of("b").fit, Some(Fit::Matching));
        assert_eq!(of("odd").fit, Some(Fit::MightNotMatch));
        assert_eq!(of("odd").rank, 2);
    }

    #[test]
    fn two_that_disagree_say_nothing_and_a_lone_track_neither() {
        let ranked = rank(
            &[
                track("a", "en", film(0)),
                track("b", "en", other_film()),
                track("fr", "fr", film(0)),
            ],
            None,
        );
        assert!(ranked.iter().all(|entry| entry.fit.is_none()), "{ranked:?}");
    }

    #[test]
    fn references_compare_but_arent_ranked() {
        let mut reference = track("ref", "en", film(0));
        reference.reference = true;
        let ranked = rank(&[track("a", "en", film(2)), reference], None);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].fit, Some(Fit::Matching));
    }

    #[test]
    fn a_track_running_past_the_video_might_not_match() {
        let ranked = rank(&[track("a", "en", film(0))], Some(5_000));
        assert_eq!(ranked[0].fit, Some(Fit::MightNotMatch));
    }

    #[test]
    fn words_of_scripts_without_spaces_count_by_character() {
        assert_eq!(words("Hello, the wor-ld!"), vec!["hello", "wor"]);
        assert_eq!(words("今日は"), vec!["今", "日", "は"]);
    }

    #[test]
    fn sound_from_another_source_fits_by_its_length() {
        assert_eq!(audio_fit(3_600_000, 3_601_000), Some("matching"));
        assert_eq!(audio_fit(3_600_000, 3_400_000), Some("mightNotMatch"));
        assert_eq!(audio_fit(3_600_000, 3_640_000), None);
        assert_eq!(audio_fit(0, 3_600_000), None);
    }
}
