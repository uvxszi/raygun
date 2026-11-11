//! What reading aloud reads and where it is: a chapter's sentences, the ids engines get for them
//! and send back, and the place kept for each chapter so reading goes on from it.

use raygun_ai::tts::sentences::sentences;
use raygun_model::TtsParagraph;
use serde::{Deserialize, Serialize};

/// One sentence to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Utterance {
    pub paragraph: u32,
    pub text: String,
    /// Where it is in its paragraph's text, in UTF-16 units (what the UI counts).
    pub start: u32,
    pub end: u32,
}

/// UTF-16 length of `text`.
fn utf16(text: &str) -> u32 {
    text.encode_utf16().count() as u32
}

/// The paragraphs as sentences, in order.
pub fn utterances(paragraphs: &[TtsParagraph]) -> Vec<Utterance> {
    paragraphs
        .iter()
        .flat_map(|paragraph| {
            sentences(&paragraph.text)
                .into_iter()
                .map(|range| Utterance {
                    paragraph: paragraph.id,
                    start: utf16(&paragraph.text[..range.start]),
                    end: utf16(&paragraph.text[..range.end]),
                    text: paragraph.text[range].to_owned(),
                })
        })
        .collect()
}

/// The first sentence of the first paragraph at or after `paragraph`.
pub fn first_of(items: &[Utterance], paragraph: u32) -> Option<usize> {
    items.iter().position(|item| item.paragraph >= paragraph)
}

/// The paragraph `step` places from the one holding sentence `at` (0: its own start).
pub fn step_from(items: &[Utterance], at: usize, step: i32) -> Option<usize> {
    let mut paragraphs: Vec<u32> = items.iter().map(|item| item.paragraph).collect();
    paragraphs.dedup();
    let current = items.get(at)?.paragraph;
    let place = paragraphs
        .iter()
        .position(|paragraph| *paragraph == current)? as i64;
    let target = (place + i64::from(step)).clamp(0, paragraphs.len() as i64 - 1) as usize;
    first_of(items, paragraphs[target])
}

/// An engine's id for sentence `index` of run `run`, `:end` on the chapter's last.
pub fn item_id(run: u64, index: usize, last: bool) -> String {
    if last {
        format!("{run}:{index}:end")
    } else {
        format!("{run}:{index}")
    }
}

/// The run and sentence of an engine's id.
pub fn parse_id(id: &str) -> Option<(u64, usize)> {
    let mut parts = id.split(':');
    let run = parts.next()?.parse().ok()?;
    let index = parts.next()?.parse().ok()?;
    Some((run, index))
}

/// The sentence's text from the word holding `offset` (UTF-16, within the sentence) on, and
/// where that word starts.
pub fn from_word(text: &str, offset: u32) -> (String, u32) {
    if offset == 0 {
        return (text.to_owned(), 0);
    }
    let mut units = 0u32;
    let mut word_start = (0usize, 0u32);
    for (at, c) in text.char_indices() {
        if units >= offset {
            break;
        }
        if c.is_whitespace() {
            word_start = (at + c.len_utf8(), units + c.len_utf16() as u32);
        }
        units += c.len_utf16() as u32;
    }
    let (byte, unit) = word_start;
    (text[byte..].to_owned(), unit)
}

/// A control from the notification, a headset or the system's media controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Pause,
    Resume,
    Toggle,
    Next,
    Previous,
    Stop,
}

/// An engine's report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Report {
    /// The sentence started.
    Speaking {
        run: u64,
        index: usize,
    },
    /// The word at `start` (UTF-16, within the text sent for the sentence) started.
    Range {
        run: u64,
        index: usize,
        start: u32,
    },
    /// The chapter's last sentence finished.
    Ended {
        run: u64,
    },
    Error {
        run: u64,
    },
    /// The player paused itself (another app took the sound).
    Paused {
        run: u64,
    },
    Control(Control),
}

pub fn parse_report(json: &str) -> Option<Report> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    if let Some(control) = value.get("control").and_then(serde_json::Value::as_str) {
        return Some(Report::Control(match control {
            "pause" => Control::Pause,
            "resume" | "play" => Control::Resume,
            "toggle" => Control::Toggle,
            "next" => Control::Next,
            "previous" => Control::Previous,
            "stop" => Control::Stop,
            _ => return None,
        }));
    }
    let (run, index) = parse_id(value.get("id")?.as_str()?)?;
    Some(match value.get("state")?.as_str()? {
        "speaking" => Report::Speaking { run, index },
        "range" => Report::Range {
            run,
            index,
            start: value.get("start")?.as_u64()? as u32,
        },
        "ended" => Report::Ended { run },
        "paused" => Report::Paused { run },
        _ => Report::Error { run },
    })
}

/// Where a chapter was left aloud.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mark {
    pub paragraph: u32,
    /// The sentence's start in the paragraph (UTF-16).
    pub start: u32,
    /// The word reached within the sentence (UTF-16), for voices that report words.
    pub word: u32,
    /// When it was kept (ms since the epoch).
    pub at: i64,
}

/// How many paragraphs past the one on screen a kept place may be and still be gone on from.
const RESUME_AHEAD: u32 = 12;

/// Where reading starts: the kept place when asked to `resume` and it's around paragraph `from`
/// (on the screen the reader opened at), else the start of `from`. The sentence, and the word in
/// it.
pub fn start_point(
    items: &[Utterance],
    from: u32,
    resume: bool,
    mark: Option<&Mark>,
) -> Option<(usize, u32)> {
    let kept = mark
        .filter(|_| resume)
        .filter(|mark| mark.paragraph + 1 >= from && mark.paragraph <= from + RESUME_AHEAD)
        .and_then(|mark| {
            items
                .iter()
                .position(|item| {
                    item.paragraph == mark.paragraph
                        && item.start <= mark.start
                        && mark.start < item.end.max(item.start + 1)
                })
                .map(|index| (index, mark.word))
        });
    kept.or_else(|| first_of(items, from).map(|index| (index, 0)))
}

/// The kept places, newest first, at most `limit`.
pub fn trimmed_marks(
    mut marks: Vec<(String, Mark)>,
    limit: usize,
) -> std::collections::BTreeMap<String, Mark> {
    marks.sort_by_key(|(_, mark)| std::cmp::Reverse(mark.at));
    marks.into_iter().take(limit).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paragraph(id: u32, text: &str) -> TtsParagraph {
        TtsParagraph {
            id,
            text: text.into(),
        }
    }

    fn chapter() -> Vec<Utterance> {
        utterances(&[
            paragraph(0, "Chapter One"),
            paragraph(2, "It was late. “Who’s there?” she asked."),
            paragraph(3, "  "),
            paragraph(5, "Nobody answered. The end."),
        ])
    }

    #[test]
    fn reads_paragraphs_as_sentences_with_their_places() {
        let items = chapter();
        let texts: Vec<&str> = items.iter().map(|item| item.text.as_str()).collect();
        assert_eq!(
            texts,
            vec![
                "Chapter One",
                "It was late.",
                "“Who’s there?” she asked.",
                "Nobody answered.",
                "The end."
            ]
        );
        assert_eq!(
            (items[2].paragraph, items[2].start, items[2].end),
            (2, 13, 38)
        );
        assert_eq!(first_of(&items, 3), Some(3));
        assert_eq!(first_of(&items, 9), None);
        // A step moves by paragraphs: back to the start of the one before, or on to the next.
        assert_eq!(step_from(&items, 2, -1), Some(0));
        assert_eq!(step_from(&items, 2, 0), Some(1));
        assert_eq!(step_from(&items, 2, 1), Some(3));
        assert_eq!(step_from(&items, 4, 1), Some(3), "the last paragraph stays");
    }

    #[test]
    fn ids_name_the_run_and_the_sentence() {
        assert_eq!(item_id(7, 3, false), "7:3");
        assert_eq!(item_id(7, 4, true), "7:4:end");
        assert_eq!(parse_id("7:4:end"), Some((7, 4)));
        assert_eq!(parse_id("nope"), None);
        assert_eq!(
            parse_report(r#"{"state":"speaking","id":"7:3"}"#),
            Some(Report::Speaking { run: 7, index: 3 })
        );
        assert_eq!(
            parse_report(r#"{"state":"range","id":"7:3","start":5,"end":9}"#),
            Some(Report::Range {
                run: 7,
                index: 3,
                start: 5
            })
        );
        assert_eq!(
            parse_report(r#"{"state":"ended","id":"7:4:end"}"#),
            Some(Report::Ended { run: 7 })
        );
        assert_eq!(
            parse_report(r#"{"state":"error","id":"7:4"}"#),
            Some(Report::Error { run: 7 })
        );
        assert_eq!(
            parse_report(r#"{"control":"toggle"}"#),
            Some(Report::Control(Control::Toggle))
        );
        assert_eq!(parse_report(r#"{"control":"dance"}"#), None);
        assert_eq!(parse_report("nope"), None);
    }

    #[test]
    fn resumes_from_the_word_reached() {
        assert_eq!(
            from_word("It was a dark night.", 0),
            ("It was a dark night.".into(), 0)
        );
        // In the middle of "dark": from its start.
        assert_eq!(
            from_word("It was a dark night.", 11),
            ("dark night.".into(), 9)
        );
        assert_eq!(
            from_word("It was a dark night.", 9),
            ("dark night.".into(), 9)
        );
        // Words after UTF-16 pairs are counted as the UI counts them.
        assert_eq!(from_word("😀 smile now", 4), ("smile now".into(), 3));
    }

    #[test]
    fn goes_on_from_a_kept_place_around_the_screen() {
        let items = chapter();
        let mark = Mark {
            paragraph: 2,
            start: 13,
            word: 4,
            at: 1,
        };
        assert_eq!(start_point(&items, 2, true, Some(&mark)), Some((2, 4)));
        assert_eq!(start_point(&items, 0, true, Some(&mark)), Some((2, 4)));
        // Not asked to resume, or the reader moved far from it: the paragraph on screen.
        assert_eq!(start_point(&items, 0, false, Some(&mark)), Some((0, 0)));
        assert_eq!(start_point(&items, 5, true, Some(&mark)), Some((3, 0)));
        assert_eq!(start_point(&items, 2, true, None), Some((1, 0)));
        let kept = trimmed_marks(
            vec![
                ("a".into(), Mark { at: 1, ..mark }),
                ("b".into(), Mark { at: 3, ..mark }),
                ("c".into(), Mark { at: 2, ..mark }),
            ],
            2,
        );
        assert_eq!(kept.keys().collect::<Vec<_>>(), vec!["b", "c"]);
    }
}
