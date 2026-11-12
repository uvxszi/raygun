//! The words of a query, tidied for the answer engines.

/// The words as typed, tidied: spaces collapsed, look-alike symbols made plain (`×`, `÷`, `−`,
/// `²`, `√`, `π`), and the question mark or `=` at the end left off.
pub fn tidy(query: &str) -> String {
    let mut out = String::with_capacity(query.len());
    let mut chars = query.trim().chars().peekable();
    while let Some(char) = chars.next() {
        match char {
            '×' | '✕' | '⋅' | '∙' => out.push('*'),
            '÷' | '∕' => out.push('/'),
            '−' | '–' | '—' => out.push('-'),
            '²' => out.push_str("^2"),
            '³' => out.push_str("^3"),
            'π' => out.push_str(" pi "),
            '√' => {
                // √2 and √(…) are square roots.
                out.push_str("sqrt");
                if chars.peek().is_some_and(|next| next.is_ascii_digit()) {
                    out.push('(');
                    while let Some(digit) =
                        chars.next_if(|next| next.is_ascii_digit() || *next == '.')
                    {
                        out.push(digit);
                    }
                    out.push(')');
                }
            }
            '\u{a0}' | '\u{202f}' | '\u{2009}' => out.push(' '),
            _ => out.push(char),
        }
    }
    let mut tidied: Vec<char> = out
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .collect();
    // A `!` after a number is a factorial (`10!`), not an exclamation.
    while let Some(last) = tidied.last().copied() {
        let before = tidied.len().checked_sub(2).map(|at| tidied[at]);
        let factorial =
            last == '!' && before.is_some_and(|char| char.is_ascii_digit() || char == ')');
        if matches!(last, '?' | '=' | ' ' | '.') || (last == '!' && !factorial) {
            tidied.pop();
        } else {
            break;
        }
    }
    tidied.into_iter().collect::<String>().trim().to_owned()
}

/// `text` without the first of `prefixes` it starts with (as whole words), and whether one
/// was there.
pub fn strip_prefix_words<'a>(text: &'a str, prefixes: &[&str]) -> (&'a str, bool) {
    for prefix in prefixes {
        if let Some(rest) = text.strip_prefix(prefix)
            && (rest.is_empty() || rest.starts_with(' '))
        {
            return (rest.trim_start(), true);
        }
    }
    (text, false)
}

/// `text` without the first of `suffixes` it ends with (as whole words).
pub fn strip_suffix_words<'a>(text: &'a str, suffixes: &[&str]) -> (&'a str, bool) {
    for suffix in suffixes {
        if let Some(rest) = text.strip_suffix(suffix)
            && (rest.is_empty() || rest.ends_with(' '))
        {
            return (rest.trim_end(), true);
        }
    }
    (text, false)
}

/// The words before and after the last ` <word> ` of `words` in `text`: `5 km to mi` split at
/// `to`.
pub fn split_last<'a>(
    text: &'a str,
    words: &[&'static str],
) -> Option<(&'a str, &'static str, &'a str)> {
    let mut best: Option<(usize, &'static str)> = None;
    for word in words {
        let needle = format!(" {word} ");
        if let Some(at) = text.rfind(&needle)
            && best.is_none_or(|(found, _)| at > found)
        {
            best = Some((at, word));
        }
    }
    let (at, word) = best?;
    let before = text[..at].trim();
    let after = text[at + word.len() + 2..].trim();
    (!before.is_empty() && !after.is_empty()).then_some((before, word, after))
}

/// The words before and after the first ` <word> ` of `words` in `text`.
pub fn split_first<'a>(
    text: &'a str,
    words: &[&'static str],
) -> Option<(&'a str, &'static str, &'a str)> {
    let mut best: Option<(usize, &'static str)> = None;
    for word in words {
        let needle = format!(" {word} ");
        if let Some(at) = text.find(&needle)
            && best.is_none_or(|(found, _)| at < found)
        {
            best = Some((at, word));
        }
    }
    let (at, word) = best?;
    let before = text[..at].trim();
    let after = text[at + word.len() + 2..].trim();
    (!before.is_empty() && !after.is_empty()).then_some((before, word, after))
}

/// Words in quotes lose them: `"bonjour"` → `bonjour`.
pub fn unquoted(text: &str) -> (&str, bool) {
    let text = text.trim();
    for (open, close) in [
        ('"', '"'),
        ('“', '”'),
        ('\'', '\''),
        ('‘', '’'),
        ('«', '»'),
        ('„', '“'),
    ] {
        if let Some(inner) = text
            .strip_prefix(open)
            .and_then(|rest| rest.strip_suffix(close))
            && !inner.trim().is_empty()
        {
            return (inner.trim(), true);
        }
    }
    (text, false)
}

/// Leading articles left off: `the Eiffel Tower` → `Eiffel Tower`.
pub fn without_article(text: &str) -> &str {
    strip_prefix_words(text, &["the", "a", "an"]).0
}

/// The number a word or digits stand for (`twelve`, `12`), for counts in questions.
pub fn count_of(word: &str) -> Option<i64> {
    if let Ok(number) = word.parse::<i64>() {
        return Some(number);
    }
    const SMALL: [&str; 21] = [
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
        "twenty",
    ];
    if let Some(index) = SMALL.iter().position(|known| *known == word) {
        return Some(index as i64);
    }
    match word {
        "a" | "an" => Some(1),
        "thirty" => Some(30),
        "forty" => Some(40),
        "fifty" => Some(50),
        "sixty" => Some(60),
        "hundred" | "a hundred" => Some(100),
        _ => None,
    }
}

/// The first letter in upper case.
pub fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_are_tidied() {
        assert_eq!(tidy("  what is 2 × 3 ? "), "what is 2 * 3");
        assert_eq!(tidy("12 ÷ 4 ="), "12 / 4");
        assert_eq!(tidy("5² − 1"), "5^2 - 1");
        assert_eq!(tidy("√2 + √(9)"), "sqrt(2) + sqrt(9)");
        assert_eq!(tidy("2π"), "2 pi");
        assert_eq!(tidy("Blue"), "Blue");
        assert_eq!(tidy("Who is it?!"), "Who is it");
        assert_eq!(tidy("10!"), "10!");
        assert_eq!(tidy("what is 5!?"), "what is 5!");
        assert_eq!(tidy("U.S.A."), "U.S.A");
    }

    #[test]
    fn words_split_where_asked() {
        assert_eq!(
            split_last("5 km to mi", &["to", "in"]),
            Some(("5 km", "to", "mi"))
        );
        assert_eq!(
            split_last("1 inch in cm in mm", &["to", "in"]),
            Some(("1 inch in cm", "in", "mm"))
        );
        assert_eq!(split_last("to mi", &["to"]), None);
        assert_eq!(
            split_first("days between a and b", &["between"]),
            Some(("days", "between", "a and b"))
        );
        assert_eq!(
            strip_prefix_words("what is love", &["what is"]),
            ("love", true)
        );
        assert_eq!(
            strip_prefix_words("whatisit", &["what is"]),
            ("whatisit", false)
        );
        assert_eq!(unquoted("“bonjour”"), ("bonjour", true));
        assert_eq!(without_article("the Eiffel Tower"), "Eiffel Tower");
        assert_eq!(count_of("twelve"), Some(12));
    }
}
