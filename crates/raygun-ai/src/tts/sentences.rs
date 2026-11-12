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

/// Abbreviations a full stop ends the sentence after unless a number follows (`No. 5`, `Vol.
/// III`, a letter's `Dec. 11th`).
const NUMBERED: &[&str] = &[
    "no", "nr", "vol", "ch", "chap", "bk", "pt", "sect", "sec", "art", "op", "ep", "sc", "fig",
    "p", "pp", "ca", "approx", "jan", "feb", "mar", "apr", "jun", "jul", "aug", "sep", "sept",
    "oct", "nov", "dec",
];

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
        || (NUMBERED.contains(&word.as_str())
            && (next_word.is_some_and(|c| c.is_ascii_digit())
                || numeral_follows(text, chars, after)))
}

/// Whether a Roman numeral comes next (`Vol. III`, `No. IV.`): two letters or more, or a single
/// one ending the clause (`No. I won't` is an answer).
fn numeral_follows(text: &str, chars: &[(usize, char)], after: usize) -> bool {
    let Some(start) = chars[after..]
        .iter()
        .find(|(_, c)| !c.is_whitespace())
        .map(|(at, _)| *at)
    else {
        return false;
    };
    let rest = &text[start..];
    let length = rest
        .find(|c: char| !c.is_alphabetic())
        .unwrap_or(rest.len());
    let word = &rest[..length];
    if super::numerals::numeral_value(word).is_none() {
        return false;
    }
    let clause_ends = rest[length..]
        .chars()
        .next()
        .is_none_or(|c| !c.is_whitespace() && !c.is_alphanumeric());
    word.chars().count() >= 2 || clause_ends
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

/// Shortest piece [`piece_end`] cuts: a voice takes about as long to start a shorter one as to
/// read it.
const MIN_PIECE: usize = 24;

/// Where the first piece of `text` a voice makes on its own ends (a byte offset; `text.len()`
/// when it all fits in `limit` characters): after the last comma, semicolon, colon or dash that
/// leaves a piece within the limit, else after the first one when that's not far past it, else
/// (for a text over twice the limit) at a space. Pieces are made one at a time, so a long
/// sentence starts sooner and takes less memory while it's made; the voice pauses at the marks
/// anyway.
pub fn piece_end(text: &str, limit: usize) -> usize {
    let length = text.chars().count();
    if length <= limit.max(MIN_PIECE) {
        return text.len();
    }
    // Places after a mark (and the quotes closing on it) with at least MIN_PIECE characters
    // before and some after: (byte offset, characters before).
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut marks = Vec::new();
    for (index, (_, c)) in chars.iter().enumerate() {
        if !is_pause_mark(*c) {
            continue;
        }
        let mut after = index + 1;
        while chars.get(after).is_some_and(|(_, c)| is_closer(*c)) {
            after += 1;
        }
        let wide = matches!(c, '，' | '、' | '；' | '：');
        let spaced = chars.get(after).is_some_and(|(_, c)| c.is_whitespace());
        if !(wide || spaced || matches!(c, '—' | '–')) {
            continue;
        }
        if after >= MIN_PIECE && length - after >= MIN_PIECE / 2 {
            marks.push((chars.get(after).map_or(text.len(), |(at, _)| *at), after));
        }
    }
    let within = marks.iter().rev().find(|(_, count)| *count <= limit);
    let near = marks.first().filter(|(_, count)| *count <= limit * 2);
    if let Some((at, _)) = within.or(near) {
        return *at;
    }
    // No mark to cut at: whole when not much longer, else at the last space before the limit.
    if length <= limit * 2 {
        return text.len();
    }
    chars
        .iter()
        .enumerate()
        .take(limit + 1)
        .skip(MIN_PIECE)
        .filter(|(_, (_, c))| c.is_whitespace())
        .map(|(_, (at, _))| *at)
        .next_back()
        .unwrap_or(text.len())
}

/// Marks a voice pauses at within a sentence.
fn is_pause_mark(c: char) -> bool {
    matches!(c, ',' | ';' | ':' | '—' | '–' | '，' | '、' | '；' | '：')
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
        // Roman numerals after numbering abbreviations, but not the pronoun after an answer.
        assert_eq!(
            split("See Vol. III for that. It is in Ch. IV. No. I won't."),
            vec![
                "See Vol. III for that.",
                "It is in Ch. IV.",
                "No.",
                "I won't."
            ]
        );
        assert_eq!(split("Read Vol. I."), vec!["Read Vol. I."]);
        // A letter's date line; a month ending a sentence still ends it.
        assert_eq!(
            split("St. Petersburgh, Dec. 11th, 17—. You will rejoice."),
            vec!["St. Petersburgh, Dec. 11th, 17—.", "You will rejoice."]
        );
        assert_eq!(
            split("It snowed in Dec. Then it rained."),
            vec!["It snowed in Dec.", "Then it rained."]
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

    /// `text` in the pieces [`piece_end`] cuts it into with `limit`.
    fn pieces(text: &str, limit: usize) -> Vec<&str> {
        let mut out = Vec::new();
        let mut rest = text;
        while !rest.is_empty() {
            let end = piece_end(rest, limit);
            out.push(&rest[..end]);
            rest = rest[end..].trim_start();
        }
        out
    }

    #[test]
    fn cuts_pieces_where_voices_pause() {
        let long = "There, Margaret, the sun is for ever visible, its broad disk just skirting the \
                    horizon and diffusing a perpetual splendour; there snow and frost are banished, \
                    and, sailing over a calm sea, we may be wafted to a land surpassing in wonders.";
        assert_eq!(
            pieces(long, 80),
            vec![
                "There, Margaret, the sun is for ever visible,",
                "its broad disk just skirting the horizon and diffusing a perpetual splendour;",
                "there snow and frost are banished, and, sailing over a calm sea,",
                "we may be wafted to a land surpassing in wonders."
            ]
        );
        // Short enough: whole.
        assert_eq!(
            pieces("It was late, and cold.", 80),
            vec!["It was late, and cold."]
        );
        // A long first clause is cut after it when it isn't far past the limit, else at a space.
        let clause = "I am already far north of London and as I walk in the streets of Petersburgh, \
                      I feel a cold northern breeze.";
        assert_eq!(
            pieces(clause, 60),
            vec![
                "I am already far north of London and as I walk in the streets of Petersburgh,",
                "I feel a cold northern breeze."
            ]
        );
        // Far longer than the limit with no mark: at spaces, every word kept.
        let unmarked = "word ".repeat(40);
        let cut = pieces(unmarked.trim_end(), 60);
        assert!(cut.len() > 2);
        assert!(
            cut.iter().all(|piece| piece.chars().count() <= 120),
            "{cut:?}"
        );
        assert_eq!(cut[0].chars().count(), 59);
        assert_eq!(cut.join(" ").split_whitespace().count(), 40);
        // A little over it with no mark to cut at, or only a tiny head or tail: whole.
        let yes = "Yes, I would like to go there with you tomorrow if the weather holds.";
        assert_eq!(pieces(yes, 40), vec![yes]);
        let said = "We may be wafted to a land surpassing in wonders and in beauty, he said.";
        assert_eq!(pieces(said, 60), vec![said]);
        // Chinese commas need no space after them.
        let chinese = format!("{}，{}。", "甲".repeat(30), "乙".repeat(30));
        assert_eq!(
            pieces(&chinese, 40),
            vec![
                format!("{}，", "甲".repeat(30)),
                format!("{}。", "乙".repeat(30))
            ]
        );
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
