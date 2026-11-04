//! Lining translations up with lyrics (M46.48). A translation is kept with the words of each line
//! it translates; laid onto lyrics, a line takes the translation of the kept line with the same
//! words (the nearest in time among repeats), else of a similar line sung at about the same time.
//! When few lines match but both have as many lines, they're paired in order.

use std::collections::HashMap;

use raygun_model::Lyrics;
use serde::{Deserialize, Serialize};

/// A line's words a translation was made for, and the translation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeptLine {
    /// When the line is sung, for synced lyrics.
    #[serde(rename = "t", default, skip_serializing_if = "Option::is_none")]
    pub time_ms: Option<u32>,
    #[serde(rename = "s")]
    pub text: String,
    /// Empty when the line has none.
    #[serde(rename = "r")]
    pub translation: String,
}

/// How far apart in time two lines may be sung and still be the same line, when their words
/// differ a little.
const NEAR_MS: u32 = 1_500;
/// How alike two lines' words must be to count as the same line.
const SIMILAR: f64 = 0.5;

/// The lines lyrics show, with their times when synced.
fn lines_of(lyrics: &Lyrics) -> Vec<(Option<u32>, &str)> {
    if lyrics.synced.is_empty() {
        lyrics
            .line_texts()
            .into_iter()
            .map(|text| (None, text))
            .collect()
    } else {
        lyrics
            .synced
            .iter()
            .map(|line| (Some(line.time_ms), line.text.as_str()))
            .collect()
    }
}

/// A line's letters and digits in lower case: what's compared.
fn simplify(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// How alike two simplified lines are: the share of character pairs they have in common.
fn similarity(a: &str, b: &str) -> f64 {
    let pairs = |text: &str| -> Vec<(char, char)> {
        let chars: Vec<char> = text.chars().collect();
        chars.windows(2).map(|pair| (pair[0], pair[1])).collect()
    };
    let (left, mut right) = (pairs(a), pairs(b));
    if left.is_empty() || right.is_empty() {
        return if a == b { 1.0 } else { 0.0 };
    }
    let total = left.len() + right.len();
    let mut shared = 0;
    for pair in left {
        if let Some(found) = right.iter().position(|other| *other == pair) {
            right.swap_remove(found);
            shared += 1;
        }
    }
    (2 * shared) as f64 / total as f64
}

/// `translation` (one entry per line of `lyrics`) as kept lines: every line with words, its
/// translation possibly empty.
pub fn kept_lines(lyrics: &Lyrics, translation: &[String]) -> Vec<KeptLine> {
    lines_of(lyrics)
        .into_iter()
        .zip(translation)
        .filter(|((_, text), _)| !simplify(text).is_empty())
        .map(|((time_ms, text), translated)| KeptLine {
            time_ms,
            text: text.to_owned(),
            translation: translated.trim().to_owned(),
        })
        .collect()
}

/// `kept` laid onto `lyrics`: one entry per line, empty where none matched.
pub fn align_onto(lyrics: &Lyrics, kept: &[KeptLine]) -> Vec<String> {
    let shown: Vec<(Option<u32>, String)> = lines_of(lyrics)
        .into_iter()
        .map(|(time, text)| (time, simplify(text)))
        .collect();
    let kept_simple: Vec<String> = kept.iter().map(|line| simplify(&line.text)).collect();
    let mut by_words: HashMap<&str, Vec<usize>> = HashMap::new();
    for (index, words) in kept_simple.iter().enumerate() {
        by_words.entry(words.as_str()).or_default().push(index);
    }
    let mut used = vec![false; kept.len()];
    let mut out = vec![String::new(); shown.len()];
    let mut matched = 0usize;
    for (index, (time, words)) in shown.iter().enumerate() {
        if words.is_empty() {
            continue;
        }
        let distance = |kept_index: usize| match (time, kept[kept_index].time_ms) {
            (Some(time), Some(kept_time)) => time.abs_diff(kept_time),
            _ => u32::MAX,
        };
        let same_words = by_words.get(words.as_str()).and_then(|candidates| {
            // An unused one first, then the nearest in time; in order without times.
            candidates
                .iter()
                .copied()
                .min_by_key(|candidate| (used[*candidate], distance(*candidate), *candidate))
        });
        let pick = same_words.or_else(|| {
            time.as_ref()?;
            kept_simple
                .iter()
                .enumerate()
                .filter(|(candidate, _)| distance(*candidate) <= NEAR_MS)
                .map(|(candidate, other)| (candidate, similarity(words, other)))
                .filter(|(_, alike)| *alike >= SIMILAR)
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(candidate, _)| candidate)
        });
        if let Some(found) = pick {
            used[found] = true;
            matched += 1;
            out[index].clone_from(&kept[found].translation);
        }
    }
    let with_words: Vec<usize> = shown
        .iter()
        .enumerate()
        .filter(|(_, (_, words))| !words.is_empty())
        .map(|(index, _)| index)
        .collect();
    if matched * 2 < with_words.len() && with_words.len() == kept.len() {
        // Another spelling of the same song (traditional and simplified characters, say).
        for (index, line) in with_words.into_iter().zip(kept) {
            out[index].clone_from(&line.translation);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use raygun_model::LyricLine;

    use super::*;

    fn synced(lines: &[(u32, &str)]) -> Lyrics {
        Lyrics {
            provider_name: "Test".into(),
            synced: lines
                .iter()
                .map(|(time_ms, text)| LyricLine {
                    time_ms: *time_ms,
                    text: (*text).into(),
                })
                .collect(),
            plain: None,
            offset_ms: 0,
            edited: false,
            translations: Vec::new(),
        }
    }

    fn strings(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|line| (*line).to_owned()).collect()
    }

    #[test]
    fn lines_find_their_translations_by_words_and_time() {
        let source = synced(&[
            (1_000, "Hold me close"),
            (5_000, "♪"),
            (9_000, "Hold me close"),
            (12_000, "Never let me go!"),
        ]);
        let kept = kept_lines(
            &source,
            &strings(&["Halt mich 1", "", "Halt mich 2", "Lass mich nie"]),
        );
        // The instrumental line has no words, so it isn't kept.
        assert_eq!(kept.len(), 3);
        // Another source: other times, punctuation and case, one line more, a line reworded.
        let other = synced(&[
            (900, "hold me close"),
            (9_200, "Hold me close,"),
            (11_800, "Never let me go"),
            (15_000, "A line only here"),
        ]);
        assert_eq!(
            align_onto(&other, &kept),
            ["Halt mich 1", "Halt mich 2", "Lass mich nie", ""]
        );
        // Close words at about the same time still match.
        let reworded = synced(&[(12_400, "Never let me goooo")]);
        assert_eq!(align_onto(&reworded, &kept), ["Lass mich nie"]);
        // Far away in time, a reworded line doesn't.
        let far = synced(&[(40_000, "Never let me goooo")]);
        assert_eq!(align_onto(&far, &kept), [""]);
    }

    #[test]
    fn plain_lyrics_and_other_spellings_pair_in_order() {
        let plain = Lyrics {
            plain: Some("One\n\nTwo".into()),
            ..synced(&[])
        };
        let kept = kept_lines(&plain, &strings(&["Eins", "", "Zwei"]));
        assert_eq!(align_onto(&plain, &kept), ["Eins", "", "Zwei"]);
        // Traditional characters against simplified: nothing matches, the counts agree.
        let simplified = synced(&[(0, "爱你"), (2_000, "说再见")]);
        let kept = kept_lines(&simplified, &strings(&["love you", "say goodbye"]));
        let traditional = synced(&[(0, "愛你"), (2_000, "說再見")]);
        assert_eq!(align_onto(&traditional, &kept), ["love you", "say goodbye"]);
        assert!(similarity("abc", "abd") > 0.4 && similarity("abc", "xyz") == 0.0);
    }
}
