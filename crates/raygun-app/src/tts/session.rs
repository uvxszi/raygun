//! What reading aloud reads and where it is: a chapter's sentences (as written, and as voices
//! read them), the ids engines get for them and send back, and the place kept for each chapter so
//! reading goes on from it.

use raygun_ai::tts::language::foreign_passage;
use raygun_ai::tts::numerals::{Rules, read_numerals};
use raygun_ai::tts::sentences::sentences;
use raygun_model::TtsParagraph;
use serde::{Deserialize, Serialize};

/// One sentence to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Utterance {
    pub paragraph: u32,
    /// As written.
    pub text: String,
    /// As voices read it: Roman numerals as numbers where settings say.
    pub spoken: String,
    /// Where it is in its paragraph's text, in UTF-16 units (what the UI counts).
    pub start: u32,
    pub end: u32,
    /// The language of a passage in another one than the book's (BCP 47).
    pub language: Option<String>,
}

/// How a chapter is read: in the book's language (when known), with passages in others when
/// they're to be found, and Roman numerals by `rules`.
#[derive(Debug, Clone, Copy)]
pub struct Reading<'a> {
    pub language: Option<&'a str>,
    pub detect_languages: bool,
    pub rules: Rules,
}

/// UTF-16 length of `text`.
fn utf16(text: &str) -> u32 {
    text.encode_utf16().count() as u32
}

/// The paragraphs as sentences, in order.
pub fn utterances(paragraphs: &[TtsParagraph], reading: &Reading<'_>) -> Vec<Utterance> {
    paragraphs
        .iter()
        .flat_map(|paragraph| {
            // A long paragraph clearly in another language is read in it; titles never are.
            let passage = reading
                .language
                .filter(|_| reading.detect_languages && !paragraph.heading)
                .and_then(|book| foreign_passage(&paragraph.text, book))
                .map(str::to_owned);
            let language = passage.clone().or(reading.language.map(str::to_owned));
            sentences(&paragraph.text).into_iter().map(move |range| {
                let text = &paragraph.text[range.clone()];
                Utterance {
                    paragraph: paragraph.id,
                    start: utf16(&paragraph.text[..range.start]),
                    end: utf16(&paragraph.text[..range.end]),
                    spoken: read_numerals(
                        text,
                        paragraph.heading,
                        language.as_deref(),
                        reading.rules,
                    ),
                    text: text.to_owned(),
                    language: passage.clone(),
                }
            })
        })
        .collect()
}

/// The chapter's text, to tell its language by when the book doesn't say: paragraphs first,
/// titles (often names or numbers) after.
pub fn language_sample(paragraphs: &[TtsParagraph]) -> String {
    let mut sample = String::new();
    for heading in [false, true] {
        for paragraph in paragraphs
            .iter()
            .filter(|paragraph| paragraph.heading == heading)
        {
            if sample.len() > 16_000 {
                return sample;
            }
            sample.push_str(&paragraph.text);
            sample.push('\n');
        }
    }
    sample
}

/// The first sentence of the first paragraph at or after `paragraph`.
pub fn first_of(items: &[Utterance], paragraph: u32) -> Option<usize> {
    items.iter().position(|item| item.paragraph >= paragraph)
}

/// The sentence of `paragraph` holding `offset` (UTF-16 in the paragraph's text): a place
/// between two sentences is the next one's, past the last the last one's.
pub fn sentence_at(items: &[Utterance], paragraph: u32, offset: u32) -> Option<usize> {
    let mut last = None;
    for (index, item) in items.iter().enumerate() {
        if item.paragraph != paragraph {
            if last.is_some() {
                break;
            }
            continue;
        }
        if offset < item.end {
            return Some(index);
        }
        last = Some(index);
    }
    last
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

/// Where reading starts: the sentence at `offset` in paragraph `from` when one was picked (a tap,
/// a selection), else the kept place when asked to `resume` and it's around paragraph `from` (on
/// the screen the reader opened at), else the start of `from`. The sentence, and the word in it.
pub fn start_point(
    items: &[Utterance],
    from: u32,
    offset: Option<u32>,
    resume: bool,
    mark: Option<&Mark>,
) -> Option<(usize, u32)> {
    if let Some(index) = offset.and_then(|offset| sentence_at(items, from, offset)) {
        return Some((index, 0));
    }
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
            heading: false,
        }
    }

    fn heading(id: u32, text: &str) -> TtsParagraph {
        TtsParagraph {
            heading: true,
            ..paragraph(id, text)
        }
    }

    const ENGLISH: Reading<'static> = Reading {
        language: Some("en"),
        detect_languages: false,
        rules: Rules {
            titles: true,
            everywhere: false,
            ordinals: false,
        },
    };

    fn chapter() -> Vec<Utterance> {
        utterances(
            &[
                paragraph(0, "Chapter One"),
                paragraph(2, "It was late. “Who’s there?” she asked."),
                paragraph(3, "  "),
                paragraph(5, "Nobody answered. The end."),
            ],
            &ENGLISH,
        )
    }

    #[test]
    fn reads_numerals_in_titles_as_numbers_but_shows_them_as_written() {
        let items = utterances(
            &[
                heading(0, "Chapter IV"),
                heading(1, "II."),
                paragraph(2, "I was there. Part I ends here."),
            ],
            &ENGLISH,
        );
        let read: Vec<(&str, &str)> = items
            .iter()
            .map(|item| (item.text.as_str(), item.spoken.as_str()))
            .collect();
        assert_eq!(
            read,
            vec![
                ("Chapter IV", "Chapter four"),
                ("II.", "two."),
                ("I was there.", "I was there."),
                ("Part I ends here.", "Part one ends here."),
            ]
        );
        // Highlighting follows the text as written.
        assert_eq!((items[3].start, items[3].end), (13, 30));
        let off = Reading {
            rules: Rules {
                titles: false,
                everywhere: false,
                ordinals: false,
            },
            ..ENGLISH
        };
        assert_eq!(
            utterances(&[heading(0, "Chapter IV")], &off)[0].spoken,
            "Chapter IV"
        );
    }

    #[test]
    fn reads_long_passages_in_their_language_only_when_asked() {
        let french = "Il était une fois un roi qui avait une fille, et elle était plus belle que \
                      le soleil. Je ne sais pas ce qu'il fait dans la maison, mais il ne veut pas \
                      que nous le sachions, et la reine est très triste depuis des années.";
        let paragraphs = [
            paragraph(0, "He said “je ne sais pas” and went on with the others."),
            paragraph(1, french),
        ];
        let detecting = Reading {
            detect_languages: true,
            ..ENGLISH
        };
        let items = utterances(&paragraphs, &detecting);
        assert!(items[0].language.is_none(), "a quotation stays English");
        assert!(
            items
                .iter()
                .filter(|item| item.paragraph == 1)
                .all(|item| item.language.as_deref() == Some("fr"))
        );
        assert!(
            utterances(&paragraphs, &ENGLISH)
                .iter()
                .all(|item| item.language.is_none())
        );
        // Without the book's language there's nothing to tell a passage from.
        let unknown = Reading {
            language: None,
            ..detecting
        };
        assert!(
            utterances(&paragraphs, &unknown)
                .iter()
                .all(|item| item.language.is_none())
        );
    }

    #[test]
    fn finds_the_sentence_a_place_is_in() {
        let items = chapter();
        // "It was late." is 0..12, "“Who’s there?” she asked." 13..38.
        assert_eq!(sentence_at(&items, 2, 0), Some(1));
        assert_eq!(sentence_at(&items, 2, 5), Some(1));
        assert_eq!(
            sentence_at(&items, 2, 12),
            Some(2),
            "between sentences: the next"
        );
        assert_eq!(sentence_at(&items, 2, 20), Some(2));
        assert_eq!(
            sentence_at(&items, 2, 99),
            Some(2),
            "past the end: the last"
        );
        assert_eq!(sentence_at(&items, 5, 17), Some(4));
        assert_eq!(sentence_at(&items, 3, 0), None, "no sentence there");
        let mark = Mark {
            paragraph: 5,
            start: 0,
            word: 3,
            at: 1,
        };
        // A picked place wins over where the chapter was left.
        assert_eq!(
            start_point(&items, 2, Some(20), true, Some(&mark)),
            Some((2, 0))
        );
        assert_eq!(start_point(&items, 3, Some(0), false, None), Some((3, 0)));
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
        assert_eq!(
            start_point(&items, 2, None, true, Some(&mark)),
            Some((2, 4))
        );
        assert_eq!(
            start_point(&items, 0, None, true, Some(&mark)),
            Some((2, 4))
        );
        // Not asked to resume, or the reader moved far from it: the paragraph on screen.
        assert_eq!(
            start_point(&items, 0, None, false, Some(&mark)),
            Some((0, 0))
        );
        assert_eq!(
            start_point(&items, 5, None, true, Some(&mark)),
            Some((3, 0))
        );
        assert_eq!(start_point(&items, 2, None, true, None), Some((1, 0)));
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
