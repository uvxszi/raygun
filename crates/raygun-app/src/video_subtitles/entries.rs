//! The cues of a subtitle track being translated, each with its translation once made: which go
//! to the translator next (a batch from where the viewer is), how far past the viewer they're
//! done, and how a cue's text travels to a translator and back. Pure, so it's tested alone.

use std::collections::HashMap;

use raygun_model::Caption;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::lyrics_translation::has_words;

/// One cue of the source track, timed in milliseconds, and its translation once made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub start_ms: u32,
    pub end_ms: u32,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
}

impl Entry {
    /// Still to translate: it has words and no translation yet.
    pub fn pending(&self) -> bool {
        self.translation.is_none() && has_words(&self.text)
    }
}

/// How much one request takes and how far ahead of the viewer translating goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_cues: usize,
    pub max_chars: usize,
    /// A cue starting further than this past the viewer waits until they come nearer.
    pub ahead_ms: u32,
}

/// What a job does next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    /// Translate these entries (indices, in time order).
    Batch(Vec<usize>),
    /// Everything near the viewer is done: wait for them to come nearer.
    Wait,
    /// Nothing is left to translate.
    Done,
}

/// A cue's text as it's kept: each line trimmed, blank lines dropped.
pub fn clean(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The source's cues as entries in time order, each cue once (an empty one left out). A
/// translation already made for the same cue (the same start and words) in `kept` is kept, so a
/// track that grows (AI subtitles) or comes back later doesn't lose what was done.
pub fn entries(cues: &[Caption], kept: &[Entry]) -> Vec<Entry> {
    let made: HashMap<(u32, &str), &str> = kept
        .iter()
        .filter_map(|entry| {
            let translation = entry.translation.as_deref()?;
            Some(((entry.start_ms, entry.text.as_str()), translation))
        })
        .collect();
    let mut out: Vec<Entry> = cues
        .iter()
        .filter_map(|cue| {
            let text = clean(&cue.text);
            if text.is_empty() {
                return None;
            }
            let translation = made
                .get(&(cue.start_ms, text.as_str()))
                .map(|translation| (*translation).to_owned());
            Some(Entry {
                start_ms: cue.start_ms,
                end_ms: cue.end_ms.max(cue.start_ms),
                text,
                translation,
            })
        })
        .collect();
    out.sort_by_key(|entry| (entry.start_ms, entry.end_ms));
    out.dedup_by(|a, b| a.start_ms == b.start_ms && a.end_ms == b.end_ms && a.text == b.text);
    out
}

/// What to translate next for a viewer at `anchor_ms`: the first cues still to translate from
/// there on (one showing at the anchor included), up to the limits; once everything after the
/// viewer is done, what came before them. A cue too far ahead waits.
pub fn next_batch(entries: &[Entry], anchor_ms: u32, limits: &Limits) -> Next {
    let first = entries
        .iter()
        .position(|entry| entry.end_ms > anchor_ms)
        .unwrap_or(entries.len());
    let mut ahead = (first..entries.len())
        .filter(|index| entries[*index].pending())
        .peekable();
    if let Some(start) = ahead.peek() {
        if entries[*start].start_ms > anchor_ms.saturating_add(limits.ahead_ms) {
            return Next::Wait;
        }
        return Next::Batch(take(entries, ahead, limits));
    }
    let behind = take(
        entries,
        (0..first).filter(|index| entries[*index].pending()),
        limits,
    );
    if behind.is_empty() {
        Next::Done
    } else {
        Next::Batch(behind)
    }
}

/// The first of `indices` that fit in one request: at least one, then up to the limits.
fn take(entries: &[Entry], indices: impl Iterator<Item = usize>, limits: &Limits) -> Vec<usize> {
    let mut batch = Vec::new();
    let mut chars = 0;
    for index in indices {
        let length = entries[index].text.chars().count();
        if !batch.is_empty()
            && (batch.len() >= limits.max_cues || chars + length > limits.max_chars)
        {
            break;
        }
        chars += length;
        batch.push(index);
    }
    batch
}

/// How far past `anchor_ms` the translated track is ready: until the first cue still to
/// translate starts (0 when one shows now). `None` when everything from the anchor on is done.
pub fn ahead_ms(entries: &[Entry], anchor_ms: u32) -> Option<u32> {
    entries
        .iter()
        .filter(|entry| entry.end_ms > anchor_ms)
        .find(|entry| entry.pending())
        .map(|entry| entry.start_ms.saturating_sub(anchor_ms))
}

/// The translated track: every translated cue timed as the source's, and cues without words
/// ("♪") as they are.
pub fn translated(entries: &[Entry]) -> Vec<Caption> {
    entries
        .iter()
        .filter_map(|entry| {
            let text = match &entry.translation {
                Some(translation) => translation.trim().to_owned(),
                None if !has_words(&entry.text) => entry.text.clone(),
                None => return None,
            };
            (!text.is_empty()).then_some(Caption {
                start_ms: entry.start_ms,
                end_ms: entry.end_ms,
                text,
            })
        })
        .collect()
}

/// Cues with words, translated and in all.
pub fn counts(entries: &[Entry]) -> (u32, u32) {
    let worded = entries.iter().filter(|entry| has_words(&entry.text));
    let (mut done, mut total) = (0u32, 0u32);
    for entry in worded {
        total += 1;
        if entry.translation.is_some() {
            done += 1;
        }
    }
    (done, total)
}

/// Each different text once, in order, and where each text finds its translation in that list.
pub fn distinct(texts: &[String]) -> (Vec<String>, Vec<usize>) {
    let mut unique: Vec<String> = Vec::new();
    let mut index_of: HashMap<&str, usize> = HashMap::new();
    let places = texts
        .iter()
        .map(|text| {
            *index_of.entry(text.as_str()).or_insert_with(|| {
                unique.push(text.clone());
                unique.len() - 1
            })
        })
        .collect();
    (unique, places)
}

/// What this device's models translate of a cue: each line of a dialogue ("- Hi." then
/// "- Hello.") on its own, else its lines as one sentence, the way they were said. The models
/// take one sentence at a time and lose line breaks.
pub fn device_pieces(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let dialogue = lines.len() > 1 && lines.iter().any(|line| line.starts_with(['-', '–', '—']));
    if dialogue {
        lines.into_iter().map(str::to_owned).collect()
    } else {
        vec![lines.join(" ")]
    }
}

/// A source track's id from its cues' times and words: the same subtitles find their
/// translations again, whatever address they came from.
pub fn source_id(cues: &[Caption]) -> String {
    let mut hasher = Sha256::new();
    for cue in cues {
        hasher.update(cue.start_ms.to_le_bytes());
        hasher.update(cue.end_ms.to_le_bytes());
        hasher.update(clean(&cue.text).as_bytes());
        hasher.update([0]);
    }
    hasher
        .finalize()
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// How far past `anchor_ms` stretches already heard reach: from the anchor to the end of the
/// stretch it's in (0 when it isn't in one). Stretches are `[start, end)` in milliseconds.
pub fn covered_ahead(ranges: &[(u32, u32)], anchor_ms: u32) -> u32 {
    let mut at = anchor_ms;
    // Stretches may touch or come in any order: go on while one continues from here.
    loop {
        let next = ranges
            .iter()
            .filter(|(start, end)| *start <= at && at < *end)
            .map(|(_, end)| *end)
            .max();
        match next {
            Some(end) if end > at => at = end,
            _ => break,
        }
    }
    at - anchor_ms
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(start_ms: u32, end_ms: u32, text: &str) -> Caption {
        Caption {
            start_ms,
            end_ms,
            text: text.into(),
        }
    }

    const LIMITS: Limits = Limits {
        max_cues: 3,
        max_chars: 1_000,
        ahead_ms: 60_000,
    };

    fn translate(entries: &mut [Entry], batch: &[usize]) {
        for index in batch {
            entries[*index].translation = Some(format!("<{}>", entries[*index].text));
        }
    }

    #[test]
    fn batches_start_where_the_viewer_is_and_keep_ahead() {
        let cues: Vec<Caption> = (0..10)
            .map(|n| cue(n * 10_000, n * 10_000 + 4_000, &format!("line {n}")))
            .collect();
        let mut list = entries(&cues, &[]);
        // The cue showing at 21 s is the first one sent.
        let Next::Batch(first) = next_batch(&list, 21_000, &LIMITS) else {
            panic!("a batch")
        };
        assert_eq!(first, [2, 3, 4]);
        translate(&mut list, &first);
        assert_eq!(ahead_ms(&list, 21_000), Some(29_000));
        let Next::Batch(second) = next_batch(&list, 21_000, &LIMITS) else {
            panic!("a batch")
        };
        assert_eq!(second, [5, 6, 7]);
        translate(&mut list, &second);
        // 80 s is more than a minute past the viewer: it waits for them.
        assert_eq!(next_batch(&list, 19_000, &LIMITS), Next::Wait);
        // Further on, the rest; then what came before the viewer.
        let Next::Batch(third) = next_batch(&list, 75_000, &LIMITS) else {
            panic!("a batch")
        };
        assert_eq!(third, [8, 9]);
        translate(&mut list, &third);
        assert_eq!(ahead_ms(&list, 75_000), None);
        let Next::Batch(behind) = next_batch(&list, 75_000, &LIMITS) else {
            panic!("a batch")
        };
        assert_eq!(behind, [0, 1]);
        translate(&mut list, &behind);
        assert_eq!(next_batch(&list, 75_000, &LIMITS), Next::Done);
        assert_eq!(counts(&list), (10, 10));
    }

    #[test]
    fn batches_keep_to_a_size() {
        let cues: Vec<Caption> = (0..4)
            .map(|n| cue(n * 1_000, n * 1_000 + 900, "twelve chars"))
            .collect();
        let list = entries(&cues, &[]);
        let small = Limits {
            max_chars: 30,
            ..LIMITS
        };
        assert_eq!(next_batch(&list, 0, &small), Next::Batch(vec![0, 1]));
        // One cue longer than a whole batch still goes, on its own.
        let tiny = Limits {
            max_chars: 5,
            ..LIMITS
        };
        assert_eq!(next_batch(&list, 0, &tiny), Next::Batch(vec![0]));
    }

    #[test]
    fn timing_is_kept_and_wordless_cues_stay() {
        let cues = vec![
            cue(5_000, 7_500, "  Where are you going?  \n\n  Home. "),
            cue(1_000, 2_000, "♪"),
            cue(9_000, 9_000, "Wait"),
            cue(12_000, 13_000, "   "),
        ];
        let mut list = entries(&cues, &[]);
        // In time order, cleaned, an empty one dropped, a zero-length one kept.
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].text, "♪");
        assert_eq!(list[1].text, "Where are you going?\nHome.");
        assert!(!list[0].pending());
        assert_eq!(counts(&list), (0, 2));
        assert_eq!(next_batch(&list, 0, &LIMITS), Next::Batch(vec![1, 2]));
        list[1].translation = Some(" Wohin gehst du?\nNach Hause. ".into());
        assert_eq!(
            translated(&list),
            [
                cue(1_000, 2_000, "♪"),
                cue(5_000, 7_500, "Wohin gehst du?\nNach Hause.")
            ]
        );
        // The viewer at 6 s: the cue showing is done, the next one isn't.
        assert_eq!(ahead_ms(&list, 6_000), Some(3_000));
        assert_eq!(ahead_ms(&list, 9_500), None);
    }

    #[test]
    fn translations_follow_a_track_that_grows() {
        let mut list = entries(&[cue(0, 1_000, "One"), cue(2_000, 3_000, "Two")], &[]);
        list[0].translation = Some("Eins".into());
        // Heard later: a cue in between and one after; "Two" was heard again differently.
        let grown = entries(
            &[
                cue(0, 1_000, "One"),
                cue(1_000, 2_000, "And"),
                cue(2_000, 3_000, "Too"),
                cue(4_000, 5_000, "Three"),
            ],
            &list,
        );
        let made: Vec<Option<&str>> = grown
            .iter()
            .map(|entry| entry.translation.as_deref())
            .collect();
        assert_eq!(made, [Some("Eins"), None, None, None]);
        assert_eq!(next_batch(&grown, 0, &LIMITS), Next::Batch(vec![1, 2, 3]));
    }

    #[test]
    fn each_text_goes_once_and_dialogue_line_by_line() {
        let texts: Vec<String> = ["Yes.", "No.", "Yes."].map(String::from).to_vec();
        let (unique, places) = distinct(&texts);
        assert_eq!(unique, ["Yes.", "No."]);
        assert_eq!(places, [0, 1, 0]);
        assert_eq!(
            device_pieces("I don't know\nwhere she went."),
            ["I don't know where she went."]
        );
        assert_eq!(
            device_pieces("- Are you sure?\n- Yes."),
            ["- Are you sure?", "- Yes."]
        );
    }

    #[test]
    fn the_same_lines_have_the_same_id() {
        let track = [cue(0, 1_000, "Hello"), cue(2_000, 3_000, "World")];
        let spaced = [cue(0, 1_000, " Hello "), cue(2_000, 3_000, "World")];
        let moved = [cue(0, 1_000, "Hello"), cue(2_500, 3_000, "World")];
        assert_eq!(source_id(&track), source_id(&spaced));
        assert_ne!(source_id(&track), source_id(&moved));
        assert_eq!(source_id(&track).len(), 16);
    }

    #[test]
    fn heard_stretches_reach_ahead() {
        let ranges = [(0, 30_000), (30_000, 60_000), (90_000, 120_000)];
        assert_eq!(covered_ahead(&ranges, 10_000), 50_000);
        assert_eq!(covered_ahead(&ranges, 60_000), 0);
        assert_eq!(covered_ahead(&ranges, 95_000), 25_000);
        assert_eq!(covered_ahead(&[], 5_000), 0);
    }
}
