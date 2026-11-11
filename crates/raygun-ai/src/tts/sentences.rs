//! Sentences, the pieces text is read aloud in: each is spoken and highlighted on its own,
//! synthesized ahead of the one playing, and where reading resumes after a pause.

use std::ops::Range;

/// Longest piece read at once, in characters: longer sentences are cut at a comma (or a space).
/// Voice models read long inputs less well and keep listeners waiting before they start.
pub const MAX_SENTENCE: usize = 300;

/// Words a full stop follows without ending the sentence.
const ABBREVIATIONS: &[&str] = &[
    "mr", "mrs", "ms", "dr", "prof", "sr", "jr", "st", "vs", "etc", "e.g", "i.e", "cf", "mt",
    "gen", "col", "lt", "capt", "sgt", "rev", "hon", "inc", "ltd", "dept", "hr", "frau", "herr",
    "sra", "srta", "z.b", "bzw", "usw",
];

/// Abbreviations a full stop ends the sentence after unless a number follows (`No. 5`).
const NUMBERED: &[&str] = &["no", "nr", "vol", "ch", "fig", "p", "pp", "ca", "approx"];

/// The sentences of `text` as byte ranges, trimmed of the spaces around them; pieces with no
/// letter or digit (`* * *`) are left out.
pub fn sentences(text: &str) -> Vec<Range<usize>> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut pieces = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < chars.len() {
        let mark = chars[index].1;
        if !is_terminator(mark) {
            index += 1;
            continue;
        }
        // Repeated marks and closing quotes or brackets stay with the sentence: `?!`, `...`, `."`.
        let mut after = index + 1;
        while after < chars.len() && (is_terminator(chars[after].1) || is_closer(chars[after].1)) {
            after += 1;
        }
        let end = chars.get(after).map_or(text.len(), |(at, _)| *at);
        let ends = match chars.get(after) {
            None => true,
            // Chinese and Japanese stops end a sentence without a space after them.
            Some(_) if is_wide_terminator(mark) => true,
            Some((_, next)) => next.is_whitespace() && !continues(text, &chars, index, after, mark),
        };
        if ends {
            pieces.push(start..end);
            start = end;
        }
        index = after;
    }
    pieces.push(start..text.len());
    pieces
        .into_iter()
        .filter_map(|range| trimmed(text, range))
        .flat_map(|range| split_long(text, range))
        .collect()
}

/// Whether the sentence goes on after the mark at `index`: an abbreviation or an initial before a
/// full stop, or a word in lower case after any mark (`Wait... what`, `the U.S. army`).
fn continues(text: &str, chars: &[(usize, char)], index: usize, after: usize, mark: char) -> bool {
    let next_word = chars[after..]
        .iter()
        .find(|(_, c)| !c.is_whitespace())
        .map(|(_, c)| *c);
    if next_word.is_some_and(|c| c.is_lowercase()) {
        return true;
    }
    if mark != '.' || chars.get(index + 1).is_some_and(|(_, c)| *c == '.') {
        return false;
    }
    let before = &text[..chars[index].0];
    let word = before
        .rsplit(|c: char| c.is_whitespace() || "(\"'“‘«".contains(c))
        .next()
        .unwrap_or("");
    let word = word.to_lowercase();
    // An initial (`J. R. R. Tolkien`), or a known abbreviation.
    (word.chars().count() == 1 && word.chars().all(char::is_alphabetic))
        || ABBREVIATIONS.contains(&word.as_str())
        || (NUMBERED.contains(&word.as_str()) && next_word.is_some_and(|c| c.is_ascii_digit()))
}

fn is_terminator(c: char) -> bool {
    matches!(c, '.' | '!' | '?' | '…' | '‽') || is_wide_terminator(c)
}

fn is_wide_terminator(c: char) -> bool {
    matches!(c, '。' | '！' | '？' | '｡')
}

fn is_closer(c: char) -> bool {
    matches!(
        c,
        '"' | '\'' | '”' | '’' | '»' | '›' | ')' | ']' | '」' | '』' | '）' | '】'
    )
}

/// `range` without the spaces around it, when something worth reading is left.
fn trimmed(text: &str, range: Range<usize>) -> Option<Range<usize>> {
    let piece = &text[range.clone()];
    let start = range.start + (piece.len() - piece.trim_start().len());
    let end = range.end - (piece.len() - piece.trim_end().len());
    (start < end && text[start..end].chars().any(char::is_alphanumeric)).then_some(start..end)
}

/// A sentence longer than [`MAX_SENTENCE`] in pieces: cut after the last comma, semicolon,
/// colon or dash that leaves a piece of at least a third of the limit, else at the last space,
/// else at the limit.
fn split_long(text: &str, range: Range<usize>) -> Vec<Range<usize>> {
    let mut pieces = Vec::new();
    let mut rest = range;
    loop {
        let piece = &text[rest.clone()];
        let Some((limit, _)) = piece.char_indices().nth(MAX_SENTENCE) else {
            pieces.push(rest);
            return pieces;
        };
        let head = &piece[..limit];
        let floor = head
            .char_indices()
            .nth(MAX_SENTENCE / 3)
            .map_or(0, |(at, _)| at);
        let at_mark = head
            .char_indices()
            .filter(|(at, c)| {
                *at >= floor && matches!(c, ',' | ';' | ':' | '—' | '–' | '，' | '、' | '；')
            })
            .map(|(at, c)| at + c.len_utf8())
            .next_back();
        let at_space = head
            .char_indices()
            .filter(|(at, c)| *at >= floor && c.is_whitespace())
            .map(|(at, _)| at)
            .next_back();
        let cut = at_mark.or(at_space).unwrap_or(limit);
        if let Some(first) = trimmed(text, rest.start..rest.start + cut) {
            pieces.push(first);
        }
        let next = rest.start + cut..rest.end;
        match trimmed(text, next) {
            Some(next) => rest = next,
            None => return pieces,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(text: &str) -> Vec<&str> {
        sentences(text)
            .into_iter()
            .map(|range| &text[range])
            .collect()
    }

    #[test]
    fn splits_at_sentence_ends() {
        assert_eq!(
            split("It was late. \"Are you there?\" she asked!  Nobody answered…"),
            vec![
                "It was late.",
                "\"Are you there?\" she asked!",
                "Nobody answered…"
            ]
        );
        assert_eq!(split("No end mark"), vec!["No end mark"]);
        assert_eq!(split("   "), Vec::<&str>::new());
        assert_eq!(split("Wait?! Who's there?"), vec!["Wait?!", "Who's there?"]);
    }

    #[test]
    fn keeps_abbreviations_initials_and_lower_case_on() {
        assert_eq!(
            split("Mr. Holmes met Dr. Watson. J. R. R. Tolkien wrote it. Then e.g. this one."),
            vec![
                "Mr. Holmes met Dr. Watson.",
                "J. R. R. Tolkien wrote it.",
                "Then e.g. this one."
            ]
        );
        assert_eq!(
            split("He paused... and went on. It cost 3.50 dollars."),
            vec!["He paused... and went on.", "It cost 3.50 dollars."]
        );
        assert_eq!(
            split("See No. 5 there. I said no. Then I left."),
            vec!["See No. 5 there.", "I said no.", "Then I left."]
        );
        assert_eq!(
            split("\"Help!\" he cried. Nobody came."),
            vec!["\"Help!\" he cried.", "Nobody came."]
        );
    }

    #[test]
    fn splits_chinese_and_japanese_without_spaces() {
        assert_eq!(
            split("你好。我很好！你呢？"),
            vec!["你好。", "我很好！", "你呢？"]
        );
        assert_eq!(
            split("「行こう。」彼は言った。"),
            vec!["「行こう。」", "彼は言った。"]
        );
    }

    #[test]
    fn leaves_out_pieces_without_words() {
        assert_eq!(split("* * *"), Vec::<&str>::new());
        assert_eq!(split("End. — . Start."), vec!["End.", "Start."]);
    }

    #[test]
    fn cuts_long_sentences_at_commas_then_spaces() {
        let clause = "and the road went on for miles, ";
        let long = clause.repeat(30);
        let pieces = split(&long);
        assert!(pieces.len() > 1);
        for piece in &pieces {
            assert!(piece.chars().count() <= MAX_SENTENCE, "{piece}");
            assert!(piece.ends_with(',') || piece == pieces.last().unwrap());
        }
        let words = "word ".repeat(200);
        assert!(
            split(&words)
                .iter()
                .all(|piece| piece.chars().count() <= MAX_SENTENCE && !piece.ends_with(' '))
        );
        let unbroken = "x".repeat(MAX_SENTENCE * 2 + 5);
        assert_eq!(split(&unbroken).len(), 3);
        // Nothing is lost: the pieces hold every word.
        let joined: String = split(&long).join(" ");
        assert_eq!(
            joined.split_whitespace().count(),
            long.split_whitespace().count()
        );
    }
}
