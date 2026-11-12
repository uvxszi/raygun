//! Roman numerals read as numbers before speech: voices read "Chapter I" as the letter, and
//! "Henry VIII" letter by letter. Only places where a numeral is clear are touched: after words
//! that number things (Chapter, Book, Part, Act, Scene, Vol., No.…), at the head of a title or as
//! a title of its own, and, when asked, after names and before "century". The pronoun "I" is left
//! alone: after a numbering word it stays a word when a verb follows it ("the Book I Read",
//! "Book I read"), and nowhere else is a lone "I" read as one. English books get English words
//! ("Chapter one", "Henry the eighth"); others get digits, which voices read in their language.

/// Where Roman numerals are read as numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rules {
    /// In titles: after numbering words (Chapter, Book, Part…), at the head of a heading, and a
    /// heading or paragraph that is only a numeral.
    pub titles: bool,
    /// Also wherever one is clearly a numeral in English text: after names (Henry VIII, World
    /// War II) and before "century".
    pub everywhere: bool,
    /// Numbered things as ordinals in English: "Chapter the first" instead of "Chapter one".
    pub ordinals: bool,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            titles: true,
            everywhere: false,
            ordinals: false,
        }
    }
}

/// `text` with its Roman numerals read as numbers where `rules` say. `heading`: the text is a
/// title (a heading block). `language` is the text's (BCP 47); English (or none known) gets
/// English words, other languages digits.
pub fn read_numerals(text: &str, heading: bool, language: Option<&str>, rules: Rules) -> String {
    if !(rules.titles || rules.everywhere) || !text.chars().any(is_roman_letter) {
        return text.to_owned();
    }
    let language = language
        .map(|tag| {
            tag.split(['-', '_'])
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase()
        })
        .filter(|tag| !tag.is_empty());
    let english = language.as_deref().is_none_or(|tag| tag == "en");
    let words = words(text);
    let shouting = mostly_capitals(text);
    let mut edits: Vec<(std::ops::Range<usize>, String)> = Vec::new();
    for (index, word) in words.iter().enumerate() {
        let Some(value) = numeral_value(word.text) else {
            continue;
        };
        let context = Context {
            text,
            words: &words,
            index,
            heading,
            english,
            language: language.as_deref(),
            shouting,
        };
        let Some(reading) = context.reading(rules) else {
            continue;
        };
        let spoken = if english {
            reading.english(value)
        } else {
            value.to_string()
        };
        match reading {
            // "Vol. III" → "Volume three".
            Reading::Numbered {
                expansion: Some(full),
                ..
            } if english => {
                let keyword = &words[index - 1];
                edits.push((keyword.start..word.end, format!("{full} {spoken}")));
            }
            _ => edits.push((word.start..word.end, spoken)),
        }
    }
    if edits.is_empty() {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len() + 16);
    let mut at = 0;
    for (range, replacement) in edits {
        out.push_str(&text[at..range.start]);
        out.push_str(&replacement);
        at = range.end;
    }
    out.push_str(&text[at..]);
    out
}

/// How a numeral is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reading {
    /// After a numbering word: a number, or (asked for) "the first"; `expansion` is the
    /// abbreviation's full word.
    Numbered {
        ordinal: bool,
        expansion: Option<&'static str>,
    },
    /// A number: "two".
    Cardinal,
    /// A ruler's: "the eighth".
    Regnal,
    /// Before "century": "nineteenth".
    Ordinal,
}

impl Reading {
    fn english(self, value: u32) -> String {
        match self {
            Self::Numbered { ordinal: true, .. } | Self::Regnal => {
                format!("the {}", ordinal_words(value))
            }
            Self::Numbered { .. } | Self::Cardinal => cardinal_words(value),
            Self::Ordinal => ordinal_words(value),
        }
    }
}

/// A word of the text: a run of letters.
#[derive(Debug, Clone, Copy)]
struct Word<'a> {
    start: usize,
    end: usize,
    text: &'a str,
}

fn words(text: &str) -> Vec<Word<'_>> {
    let mut words = Vec::new();
    let mut start = None;
    for (at, c) in text.char_indices() {
        match (c.is_alphabetic(), start) {
            (true, None) => start = Some(at),
            (false, Some(begun)) => {
                words.push(Word {
                    start: begun,
                    end: at,
                    text: &text[begun..at],
                });
                start = None;
            }
            _ => {}
        }
    }
    if let Some(begun) = start {
        words.push(Word {
            start: begun,
            end: text.len(),
            text: &text[begun..],
        });
    }
    words
}

/// What follows a numeral.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Next<'a> {
    End,
    /// Another word, after spaces only.
    Word(&'a str),
    /// `'s` right after it.
    Possessive,
    /// `'m`, `'ll`, `'ve`…: the pronoun's contraction.
    Contraction,
    /// A mark; `spaced`: a space or the end comes right after it.
    Mark {
        mark: char,
        spaced: bool,
    },
}

struct Context<'a> {
    text: &'a str,
    words: &'a [Word<'a>],
    index: usize,
    heading: bool,
    english: bool,
    language: Option<&'a str>,
    shouting: bool,
}

impl<'a> Context<'a> {
    fn word(&self) -> Word<'a> {
        self.words[self.index]
    }

    fn previous(&self) -> Option<Word<'a>> {
        self.index.checked_sub(1).map(|index| self.words[index])
    }

    /// What lies between the word before and this one (from the text's start for the first).
    fn gap_before(&self) -> &'a str {
        let start = self.previous().map_or(0, |word| word.end);
        &self.text[start..self.word().start]
    }

    fn next(&self) -> Next<'a> {
        let word = self.word();
        let rest = &self.text[word.end..];
        let mut chars = rest.chars();
        let Some(first) = chars.next() else {
            return Next::End;
        };
        if is_apostrophe(first) {
            let after: String = chars.clone().take_while(|c| c.is_alphabetic()).collect();
            if !after.is_empty() {
                return if after.eq_ignore_ascii_case("s") {
                    Next::Possessive
                } else {
                    Next::Contraction
                };
            }
        }
        let trimmed = rest.trim_start();
        let Some(mark) = trimmed.chars().next() else {
            return Next::End;
        };
        if mark.is_alphabetic() {
            return Next::Word(self.next_word());
        }
        let spaced = trimmed[mark.len_utf8()..]
            .chars()
            .next()
            .is_none_or(char::is_whitespace);
        Next::Mark { mark, spaced }
    }

    /// Whether nothing but marks and spaces surround the numeral: a title of its own ("IV.",
    /// "— II —").
    fn alone(&self) -> bool {
        let word = self.word();
        self.words.len() == 1
            && no_letters_or_quotes(&self.text[..word.start])
            && no_letters_or_quotes(&self.text[word.end..])
    }

    /// Whether a numeral standing alone is a title's: "IV.", "— ii —"; a lone "I" only as a
    /// heading, and never a scene break ("XXX") or a small single letter ("x").
    fn titled_alone(&self, word: &str, single: bool) -> bool {
        if word.eq_ignore_ascii_case("xxx") {
            return false;
        }
        if word.chars().all(char::is_lowercase) {
            return !single && word.chars().all(|c| matches!(c, 'i' | 'v' | 'x'));
        }
        if EXCLUDED.contains(&word) {
            return false;
        }
        !single || self.heading || word != "I"
    }

    /// The word after the numeral with its contraction ("won't", "didn't").
    fn next_word(&self) -> &'a str {
        let Some(next) = self.words.get(self.index + 1) else {
            return "";
        };
        let rest = &self.text[next.end..];
        let end = match rest.chars().next() {
            Some(c) if is_apostrophe(c) => {
                let after: usize = rest[c.len_utf8()..]
                    .chars()
                    .take_while(|c| c.is_alphabetic())
                    .map(char::len_utf8)
                    .sum();
                if after > 0 {
                    next.end + c.len_utf8() + after
                } else {
                    next.end
                }
            }
            _ => next.end,
        };
        &self.text[next.start..end]
    }

    /// Whether the numeral heads the text, after spaces, dashes or brackets only.
    fn first(&self) -> bool {
        self.index == 0 && no_letters_or_quotes(self.gap_before())
    }

    fn reading(&self, rules: Rules) -> Option<Reading> {
        let word = self.word().text;
        let single = word.chars().count() == 1;
        // A lone L, C, D or M is a letter (Appendix C, Plan D), never a numeral here.
        if single && !matches!(word, "I" | "V" | "X" | "i" | "v" | "x") {
            return None;
        }
        if let Some(reading) = self.numbered(rules) {
            return Some(reading);
        }
        if self.alone() && self.titled_alone(word, single) {
            return Some(Reading::Cardinal);
        }
        if self.leads_title() {
            return Some(Reading::Cardinal);
        }
        if (self.heading || rules.everywhere) && self.english {
            return self.named();
        }
        None
    }

    /// After a numbering word: "Chapter IV", "Act IV, Scene ii", "Vol. III".
    fn numbered(&self, rules: Rules) -> Option<Reading> {
        let previous = self.previous()?;
        let keyword = keyword(previous.text, self.language)?;
        let gap = self.gap_before();
        let spaced = !gap.is_empty() && gap.chars().all(char::is_whitespace);
        let dotted = gap.starts_with('.') && gap[1..].chars().all(char::is_whitespace);
        if !(spaced && !keyword.period || dotted && keyword.abbreviation) {
            return None;
        }
        let word = self.word().text;
        let single = word.chars().count() == 1;
        let capitalized = previous.text.chars().next().is_some_and(char::is_uppercase);
        // "the Letter X", "the Book I Read": a letter, or the pronoun.
        if single && self.determiner_before(self.index - 1) {
            return None;
        }
        // "Letter I" is the first of a book's letters only as a title: a heading, or a line it
        // starts ("LETTER I.", "Letter V: To Mrs. Saville"); elsewhere "letter X" is the letter.
        if single && keyword.no_letters && !self.titles_with(capitalized) {
            return None;
        }
        let lower_numeral = word.chars().all(char::is_lowercase);
        if lower_numeral && !(capitalized || keyword.abbreviation) {
            return None;
        }
        if single {
            let ends = matches!(self.next(), Next::End | Next::Mark { .. });
            if lower_numeral && !ends {
                return None;
            }
            if word == "I" {
                match self.next() {
                    Next::Contraction => return None,
                    // "the chapter I wrote".
                    Next::Word(_) if !capitalized => return None,
                    // "Book I read", "Chapter I Remember".
                    Next::Word(next) if is_pronoun_verb(next) => return None,
                    _ => {}
                }
            }
        }
        Some(Reading::Numbered {
            ordinal: rules.ordinals,
            expansion: keyword.expansion,
        })
    }

    /// Whether the numbering word before the numeral and the numeral title the text: a heading's
    /// ("Frankenstein — Letter I"), or heading a line of their own ("LETTER I.", "Letter V: To
    /// Mrs. Saville"), capitalized, with only a mark or the end after the numeral.
    fn titles_with(&self, capitalized: bool) -> bool {
        let ends = matches!(self.next(), Next::End | Next::Mark { .. });
        let keyword_first =
            self.index == 1 && no_letters_or_quotes(&self.text[..self.words[0].start]);
        capitalized && ends && (self.heading || keyword_first)
    }

    /// A numeral heading a title: "IV. The Storm", "I: Origins", "II — Home"; in other text only
    /// in a short line that goes on with a capitalized word ("I. We hold…", not "V. S. Naipaul").
    fn leads_title(&self) -> bool {
        if !self.first() {
            return false;
        }
        let Next::Mark { mark, spaced } = self.next() else {
            return false;
        };
        let dash = matches!(mark, '—' | '–' | '-' | '|' | '·');
        let follows = &self.text[self.word().end..];
        let marked = if dash {
            // "II — Home", not "X-Men".
            follows.starts_with(char::is_whitespace)
        } else {
            matches!(mark, '.' | ':' | ')' | ']') && spaced
        };
        if !marked {
            return false;
        }
        if self.heading {
            return true;
        }
        let next = self.words.get(self.index + 1).map(|word| word.text);
        self.text.chars().count() <= 60
            && next.is_some_and(|next| {
                next.chars().count() >= 2
                    && next.chars().next().is_some_and(char::is_uppercase)
                    && !is_pronoun_verb(next)
            })
    }

    /// After a name, in English: "Henry VIII" (a ruler's: "the eighth"), "World War II"; before
    /// "century": "the XIX century".
    fn named(&self) -> Option<Reading> {
        let word = self.word().text;
        if !word.chars().all(|c| c.is_ascii_uppercase()) {
            return None;
        }
        if let Next::Word(next) = self.next()
            && matches!(
                next.to_lowercase().as_str(),
                "century" | "centuries" | "dynasty" | "dynasties" | "millennium" | "millennia"
            )
            && word.len() >= 2
            && !EXCLUDED.contains(&word)
        {
            return Some(Reading::Ordinal);
        }
        let previous = self.previous()?;
        let gap = self.gap_before();
        if gap.is_empty() || !gap.chars().all(char::is_whitespace) {
            return None;
        }
        let name = previous.text;
        let named = name.chars().next().is_some_and(char::is_uppercase);
        if !named && !self.heading {
            return None;
        }
        if self.determiner_before(self.index) {
            return None;
        }
        let regnal = is_regnal(name);
        if word.len() == 1 {
            // "Henry V", "Charles X", "Elizabeth I's"; never "Malcolm X" or "the Mary I knew".
            if !regnal || self.determiner_before(self.index - 1) {
                return None;
            }
            if word == "I" {
                match self.next() {
                    Next::End | Next::Possessive | Next::Mark { .. } => {}
                    Next::Word(next)
                        if next.chars().next().is_some_and(char::is_uppercase)
                            && !is_pronoun_verb(next) => {}
                    _ => return None,
                }
            }
            return Some(Reading::Regnal);
        }
        if EXCLUDED.contains(&word) {
            return None;
        }
        let plain = word.chars().all(|c| matches!(c, 'I' | 'V' | 'X'));
        // Capitals throughout (shouting, or a title in capitals): only I, V and X, where words
        // like MIX and CD are numerals in form only.
        if !plain && (word.len() < 3 || self.shouting) {
            return None;
        }
        Some(if regnal {
            Reading::Regnal
        } else {
            Reading::Cardinal
        })
    }

    /// Whether the word at `index` comes right after "the", "a", "his"…
    fn determiner_before(&self, index: usize) -> bool {
        if !self.english {
            return false;
        }
        let Some(before) = index.checked_sub(1).map(|at| self.words[at]) else {
            return false;
        };
        let gap = &self.text[before.end..self.words[index].start];
        !gap.is_empty()
            && gap.chars().all(char::is_whitespace)
            && DETERMINERS.contains(&before.text.to_lowercase().as_str())
    }
}

fn is_apostrophe(c: char) -> bool {
    matches!(c, '\'' | '’')
}

/// Whether `text` has no letters, digits or quotation marks: spaces, dashes, brackets, stops.
fn no_letters_or_quotes(text: &str) -> bool {
    text.chars().all(|c| {
        !(c.is_alphanumeric() || matches!(c, '"' | '“' | '”' | '„' | '«' | '»' | '‘' | '’' | '\''))
    })
}

/// Whether most of `text`'s letters (of enough) are capitals.
fn mostly_capitals(text: &str) -> bool {
    let (mut letters, mut capitals) = (0usize, 0usize);
    for c in text.chars().filter(|c| c.is_alphabetic()) {
        letters += 1;
        if c.is_uppercase() {
            capitals += 1;
        }
    }
    letters >= 8 && capitals * 10 >= letters * 8
}

fn is_roman_letter(c: char) -> bool {
    matches!(
        c,
        'I' | 'V' | 'X' | 'L' | 'C' | 'D' | 'M' | 'i' | 'v' | 'x' | 'l' | 'c' | 'd' | 'm'
    )
}

/// The value of a Roman numeral written the usual way (all capitals or all small letters), 1 to
/// 3999; `IIII` or `VX` aren't numerals.
pub fn numeral_value(word: &str) -> Option<u32> {
    let upper = word.chars().all(|c| c.is_ascii_uppercase());
    let lower = word.chars().all(|c| c.is_ascii_lowercase());
    if word.is_empty() || word.len() > 15 || !(upper || lower) {
        return None;
    }
    let upper = word.to_ascii_uppercase();
    let mut total = 0i32;
    let mut highest = 0i32;
    for c in upper.chars().rev() {
        let digit = match c {
            'I' => 1,
            'V' => 5,
            'X' => 10,
            'L' => 50,
            'C' => 100,
            'D' => 500,
            'M' => 1000,
            _ => return None,
        };
        if digit < highest {
            total -= digit;
        } else {
            total += digit;
            highest = digit;
        }
    }
    let value = u32::try_from(total)
        .ok()
        .filter(|value| (1..=3999).contains(value))?;
    (to_roman(value) == upper).then_some(value)
}

fn to_roman(mut value: u32) -> String {
    const PARTS: [(u32, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut out = String::new();
    for (part, letters) in PARTS {
        while value >= part {
            out.push_str(letters);
            value -= part;
        }
    }
    out
}

const ONES: [&str; 20] = [
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
];
const TENS: [&str; 10] = [
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];

/// `value` in English words: "one hundred nineteen".
pub fn cardinal_words(value: u32) -> String {
    match value {
        0..=19 => ONES[value as usize].to_owned(),
        20..=99 => {
            let tens = TENS[(value / 10) as usize];
            match value % 10 {
                0 => tens.to_owned(),
                ones => format!("{tens}-{}", ONES[ones as usize]),
            }
        }
        100..=999 => {
            let hundreds = format!("{} hundred", ONES[(value / 100) as usize]);
            match value % 100 {
                0 => hundreds,
                rest => format!("{hundreds} {}", cardinal_words(rest)),
            }
        }
        _ => {
            let thousands = format!("{} thousand", cardinal_words(value / 1000));
            match value % 1000 {
                0 => thousands,
                rest => format!("{thousands} {}", cardinal_words(rest)),
            }
        }
    }
}

/// `value` as an English ordinal: "eighth", "twenty-first", "one hundredth".
pub fn ordinal_words(value: u32) -> String {
    let words = cardinal_words(value);
    let split = words.rfind([' ', '-']).map_or(0, |at| at + 1);
    let (head, last) = words.split_at(split);
    let last = match last {
        "one" => "first".to_owned(),
        "two" => "second".to_owned(),
        "three" => "third".to_owned(),
        "five" => "fifth".to_owned(),
        "eight" => "eighth".to_owned(),
        "nine" => "ninth".to_owned(),
        "twelve" => "twelfth".to_owned(),
        tens if tens.ends_with('y') => format!("{}ieth", &tens[..tens.len() - 1]),
        other => format!("{other}th"),
    };
    format!("{head}{last}")
}

/// A numbering word: what may follow it, and how an abbreviation is said in full.
#[derive(Debug, Clone, Copy)]
struct Keyword {
    /// Only with its stop ("No. 5", "Art. II"): without it, it's a word ("No I won't").
    period: bool,
    /// May be followed by its stop ("Vol. III").
    abbreviation: bool,
    /// Single letters after it are letters ("the letter X").
    no_letters: bool,
    expansion: Option<&'static str>,
}

const fn word() -> Keyword {
    Keyword {
        period: false,
        abbreviation: false,
        no_letters: false,
        expansion: None,
    }
}

/// An abbreviation that needs its stop, said as `full` in English.
const fn dotted(full: &'static str) -> Keyword {
    Keyword {
        period: true,
        abbreviation: true,
        no_letters: false,
        expansion: Some(full),
    }
}

/// An abbreviation with or without its stop.
const fn short(full: &'static str) -> Keyword {
    Keyword {
        period: false,
        abbreviation: true,
        no_letters: false,
        expansion: Some(full),
    }
}

const ENGLISH_KEYWORDS: &[(&str, Keyword)] = &[
    ("chapter", word()),
    ("chap", dotted("Chapter")),
    ("ch", dotted("Chapter")),
    ("book", word()),
    ("bk", dotted("Book")),
    ("part", word()),
    ("pt", dotted("Part")),
    ("volume", word()),
    ("vol", short("Volume")),
    ("act", word()),
    ("scene", word()),
    ("sc", dotted("Scene")),
    ("canto", word()),
    (
        "letter",
        Keyword {
            no_letters: true,
            ..word()
        },
    ),
    ("section", word()),
    ("sect", dotted("Section")),
    ("sec", dotted("Section")),
    ("no", dotted("Number")),
    ("nr", dotted("Number")),
    ("number", word()),
    ("article", word()),
    ("art", dotted("Article")),
    ("appendix", word()),
    ("psalm", word()),
    ("stave", word()),
    ("lecture", word()),
    ("lesson", word()),
    ("episode", word()),
    ("ep", dotted("Episode")),
    ("tome", word()),
    ("round", word()),
    ("phase", word()),
    ("stage", word()),
    ("level", word()),
    ("class", word()),
    ("type", word()),
    ("grade", word()),
    ("series", word()),
    ("sonnet", word()),
    ("stanza", word()),
    ("hymn", word()),
    ("ode", word()),
    ("epistle", word()),
    ("movement", word()),
    ("opus", word()),
    ("op", dotted("Opus")),
    ("plate", word()),
    ("table", word()),
    ("figure", word()),
    ("fig", dotted("Figure")),
    ("dynasty", word()),
];

/// Numbering words of other languages, by primary language tag.
const OTHER_KEYWORDS: &[(&str, &[&str])] = &[
    (
        "de",
        &[
            "kapitel",
            "teil",
            "buch",
            "band",
            "akt",
            "aufzug",
            "szene",
            "auftritt",
            "abschnitt",
            "brief",
            "gesang",
            "nummer",
        ],
    ),
    (
        "fr",
        &[
            "chapitre", "livre", "partie", "tome", "acte", "scène", "scene", "lettre", "chant",
            "section", "volume", "numéro",
        ],
    ),
    (
        "es",
        &[
            "capítulo",
            "capitulo",
            "libro",
            "parte",
            "tomo",
            "acto",
            "escena",
            "carta",
            "canto",
            "sección",
            "seccion",
            "volumen",
            "número",
        ],
    ),
    (
        "it",
        &[
            "capitolo", "libro", "parte", "volume", "atto", "scena", "lettera", "canto", "sezione",
            "tomo",
        ],
    ),
    (
        "pt",
        &[
            "capítulo",
            "capitulo",
            "livro",
            "parte",
            "volume",
            "ato",
            "acto",
            "cena",
            "carta",
            "canto",
            "secção",
            "seção",
            "tomo",
        ],
    ),
    (
        "nl",
        &[
            "hoofdstuk",
            "boek",
            "deel",
            "bedrijf",
            "akte",
            "scène",
            "brief",
            "zang",
        ],
    ),
    (
        "pl",
        &[
            "rozdział",
            "księga",
            "część",
            "tom",
            "akt",
            "scena",
            "list",
            "pieśń",
        ],
    ),
    (
        "ru",
        &[
            "глава",
            "часть",
            "книга",
            "том",
            "акт",
            "сцена",
            "действие",
            "явление",
            "письмо",
            "песнь",
        ],
    ),
    (
        "uk",
        &[
            "розділ",
            "частина",
            "книга",
            "том",
            "дія",
            "сцена",
            "лист",
            "пісня",
        ],
    ),
    (
        "cs",
        &[
            "kapitola",
            "kniha",
            "část",
            "díl",
            "svazek",
            "dějství",
            "scéna",
        ],
    ),
    (
        "sk",
        &[
            "kapitola", "kniha", "časť", "diel", "zväzok", "dejstvo", "scéna",
        ],
    ),
    (
        "sv",
        &["kapitel", "bok", "del", "akt", "scen", "brev", "sång"],
    ),
    ("da", &["kapitel", "bog", "del", "akt", "scene", "brev"]),
    ("nb", &["kapittel", "bok", "del", "akt", "scene", "brev"]),
    (
        "ro",
        &[
            "capitolul",
            "capitol",
            "cartea",
            "partea",
            "volumul",
            "actul",
            "scena",
        ],
    ),
    (
        "ca",
        &[
            "capítol", "llibre", "part", "volum", "acte", "escena", "carta", "cant",
        ],
    ),
];

fn keyword(name: &str, language: Option<&str>) -> Option<Keyword> {
    let lower = name.to_lowercase();
    if let Some((_, keyword)) = ENGLISH_KEYWORDS.iter().find(|(name, _)| *name == lower) {
        return Some(*keyword);
    }
    let language = match language? {
        "no" | "nn" => "nb",
        other => other,
    };
    OTHER_KEYWORDS
        .iter()
        .find(|(tag, _)| *tag == language)
        .filter(|(_, words)| words.contains(&lower.as_str()))
        .map(|_| word())
}

/// Words that start a noun phrase: "the Book I Read" is the pronoun's.
const DETERMINERS: &[&str] = &[
    "the", "a", "an", "this", "that", "these", "those", "my", "your", "his", "her", "its", "our",
    "their", "which", "what", "whose", "every", "each", "any", "some",
];

/// Numerals in form only: discs, sizes, "Washington, DC".
const EXCLUDED: &[&str] = &[
    "CD", "DC", "MD", "MM", "CC", "CV", "XL", "XXL", "XXX", "LI", "MI", "DI", "CLI", "DIV", "DIX",
    "LIV", "CIV", "MIX", "MC", "CM",
];

/// Words "I" says ("I am", "I Remember"): after them "I" is the pronoun.
fn is_pronoun_verb(word: &str) -> bool {
    const VERBS: &[&str] = &[
        "am",
        "was",
        "were",
        "have",
        "had",
        "has",
        "do",
        "did",
        "will",
        "would",
        "shall",
        "should",
        "can",
        "could",
        "may",
        "might",
        "must",
        "need",
        "know",
        "knew",
        "think",
        "thought",
        "believe",
        "remember",
        "see",
        "saw",
        "said",
        "say",
        "feel",
        "felt",
        "want",
        "wanted",
        "like",
        "liked",
        "love",
        "loved",
        "hate",
        "hated",
        "wish",
        "hope",
        "hoped",
        "mean",
        "meant",
        "guess",
        "suppose",
        "found",
        "find",
        "went",
        "go",
        "come",
        "came",
        "got",
        "get",
        "made",
        "make",
        "took",
        "take",
        "gave",
        "give",
        "told",
        "tell",
        "asked",
        "ask",
        "read",
        "wrote",
        "write",
        "heard",
        "hear",
        "looked",
        "look",
        "left",
        "leave",
        "met",
        "meet",
        "kept",
        "keep",
        "tried",
        "try",
        "used",
        "use",
        "called",
        "call",
        "became",
        "become",
        "began",
        "begin",
        "ran",
        "run",
        "sat",
        "sit",
        "stood",
        "stand",
        "lost",
        "lose",
        "let",
        "put",
        "bought",
        "brought",
        "sent",
        "spent",
        "understand",
        "understood",
        "agree",
        "promise",
        "swear",
        "insist",
        "never",
        "always",
        "also",
        "just",
        "only",
        "still",
        "really",
        "too",
        "then",
        "now",
        "once",
        "often",
        "sometimes",
        "already",
        "almost",
        "ever",
        "finally",
        "again",
        "alone",
        "myself",
        "dont",
        "didnt",
        "cant",
        "wont",
        "wasnt",
        "havent",
        "couldnt",
        "wouldnt",
        "shouldnt",
        "hadnt",
        "aint",
        "adore",
        "lived",
        "live",
        "died",
        "die",
        "fell",
        "fall",
        "sang",
        "sing",
        "played",
        "play",
        "walked",
        "walk",
        "turned",
        "turn",
        "waited",
        "wait",
        "watched",
        "watch",
        "learned",
        "learnt",
        "learn",
        "decided",
        "decide",
        "noticed",
        "notice",
        "realized",
        "realised",
        "realize",
        "wonder",
        "wondered",
        "doubt",
        "fear",
        "feared",
        "dreamed",
        "dreamt",
        "dream",
        "returned",
        "return",
        "stayed",
        "stay",
        "slept",
        "sleep",
        "woke",
        "wake",
        "awoke",
    ];
    let lower: String = word
        .chars()
        .filter(|c| c.is_alphabetic())
        .collect::<String>()
        .to_lowercase();
    VERBS.contains(&lower.as_str())
}

/// Rulers' names, after which a numeral is an ordinal: "Henry VIII", "Pius XII".
fn is_regnal(name: &str) -> bool {
    const NAMES: &[&str] = &[
        "adrian",
        "afonso",
        "albert",
        "alexander",
        "alexios",
        "alfonso",
        "amenhotep",
        "anastasius",
        "andronikos",
        "anne",
        "antiochus",
        "artaxerxes",
        "baldwin",
        "basil",
        "benedict",
        "boleslaw",
        "boniface",
        "callixtus",
        "carlos",
        "casimir",
        "catherine",
        "celestine",
        "charles",
        "christian",
        "clement",
        "constantine",
        "cyrus",
        "darius",
        "david",
        "duncan",
        "edmund",
        "edward",
        "elizabeth",
        "eric",
        "erik",
        "eugene",
        "felipe",
        "felix",
        "ferdinand",
        "francis",
        "franz",
        "frederick",
        "frederik",
        "friedrich",
        "george",
        "gregory",
        "gustav",
        "gustavus",
        "haakon",
        "hadrian",
        "harald",
        "henry",
        "herod",
        "honorius",
        "innocent",
        "isabella",
        "ivan",
        "james",
        "john",
        "joseph",
        "juan",
        "julius",
        "justinian",
        "karl",
        "kenneth",
        "leo",
        "leopold",
        "louis",
        "ludwig",
        "magnus",
        "manuel",
        "marcellus",
        "martin",
        "mary",
        "maximilian",
        "mehmed",
        "michael",
        "murad",
        "napoleon",
        "nicholas",
        "olaf",
        "olav",
        "oscar",
        "otto",
        "paul",
        "pedro",
        "peter",
        "philip",
        "philippe",
        "pius",
        "ptolemy",
        "rameses",
        "ramesses",
        "richard",
        "robert",
        "rudolf",
        "seleucus",
        "selim",
        "senusret",
        "seti",
        "sixtus",
        "stephen",
        "suleiman",
        "thutmose",
        "umberto",
        "urban",
        "valdemar",
        "victor",
        "wilhelm",
        "willem",
        "william",
        "xerxes",
    ];
    NAMES.contains(&name.to_lowercase().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TITLES: Rules = Rules {
        titles: true,
        everywhere: false,
        ordinals: false,
    };
    const EVERYWHERE: Rules = Rules {
        titles: true,
        everywhere: true,
        ordinals: false,
    };

    fn read(text: &str, heading: bool, rules: Rules) -> String {
        read_numerals(text, heading, Some("en"), rules)
    }

    #[test]
    fn reads_numerals_values_and_words() {
        assert_eq!(numeral_value("I"), Some(1));
        assert_eq!(numeral_value("iv"), Some(4));
        assert_eq!(numeral_value("XIX"), Some(19));
        assert_eq!(numeral_value("MCMLXXXIV"), Some(1984));
        assert_eq!(numeral_value("IIII"), None);
        assert_eq!(numeral_value("VX"), None);
        assert_eq!(numeral_value("DID"), None);
        assert_eq!(numeral_value("Iv"), None);
        assert_eq!(numeral_value("CIVIL"), None);
        assert_eq!(cardinal_words(1), "one");
        assert_eq!(cardinal_words(21), "twenty-one");
        assert_eq!(cardinal_words(119), "one hundred nineteen");
        assert_eq!(
            cardinal_words(1984),
            "one thousand nine hundred eighty-four"
        );
        assert_eq!(ordinal_words(1), "first");
        assert_eq!(ordinal_words(8), "eighth");
        assert_eq!(ordinal_words(12), "twelfth");
        assert_eq!(ordinal_words(19), "nineteenth");
        assert_eq!(ordinal_words(20), "twentieth");
        assert_eq!(ordinal_words(23), "twenty-third");
        assert_eq!(ordinal_words(100), "one hundredth");
    }

    #[test]
    fn reads_numbered_titles_as_numbers() {
        let cases = [
            ("Chapter I", "Chapter one"),
            ("CHAPTER I.", "CHAPTER one."),
            ("Chapter IV", "Chapter four"),
            ("Chapter I: The Beginning", "Chapter one: The Beginning"),
            ("Chapter I The Beginning", "Chapter one The Beginning"),
            ("Book II", "Book two"),
            ("Book I, Chapter V", "Book one, Chapter five"),
            ("Part I: I Remember", "Part one: I Remember"),
            ("Act IV, Scene ii", "Act four, Scene two"),
            ("Vol. III", "Volume three"),
            ("See No. XII below.", "See Number twelve below."),
            ("Psalm CXIX", "Psalm one hundred nineteen"),
            ("Sonnet XVIII", "Sonnet eighteen"),
            (
                "Part I of the trilogy ends here.",
                "Part one of the trilogy ends here.",
            ),
            ("Act I begins at dawn.", "Act one begins at dawn."),
            (
                "As we saw in Chapter XIV, it rained.",
                "As we saw in Chapter fourteen, it rained.",
            ),
            ("Letter IV", "Letter four"),
        ];
        for (text, read_as) in cases {
            assert_eq!(read(text, false, TITLES), read_as, "{text}");
        }
    }

    /// Frankenstein's headings as Project Gutenberg's editions set them: the 1818 text (book 84)
    /// numbers in digits, the 1831 ones (books 41445, 42324) in capitals with Roman numerals, and
    /// Standard Ebooks' as "Letter I"; as headings, and as the line a letter starts with.
    #[test]
    fn reads_frankensteins_headings() {
        let headings = [
            ("Letter 1", "Letter 1"),
            ("Letter 4", "Letter 4"),
            ("Chapter 1", "Chapter 1"),
            ("Chapter 24", "Chapter 24"),
            ("Letter I", "Letter one"),
            ("Letter IV", "Letter four"),
            ("LETTER I.", "LETTER one."),
            ("LETTER II.", "LETTER two."),
            ("LETTER IV.", "LETTER four."),
            ("Letter V: To Mrs. Saville", "Letter five: To Mrs. Saville"),
            ("Frankenstein — Letter I", "Frankenstein — Letter one"),
            ("Chapter I", "Chapter one"),
            ("CHAPTER I.", "CHAPTER one."),
            ("CHAPTER XXIV.", "CHAPTER twenty-four."),
            ("Chapter XXIV", "Chapter twenty-four"),
        ];
        for (text, read_as) in headings {
            assert_eq!(read(text, true, TITLES), read_as, "heading {text}");
        }
        // A title set as a paragraph, alone or starting the letter's first line.
        for (text, read_as) in [
            ("LETTER I.", "LETTER one."),
            ("Letter I", "Letter one"),
            (
                "LETTER I. To Mrs. Saville, England.",
                "LETTER one. To Mrs. Saville, England.",
            ),
            ("CHAPTER XXIV.", "CHAPTER twenty-four."),
        ] {
            assert_eq!(read(text, false, TITLES), read_as, "paragraph {text}");
        }
        let ordinals = Rules {
            ordinals: true,
            ..TITLES
        };
        assert_eq!(read("LETTER I.", true, ordinals), "LETTER the first.");
        // Letters of the alphabet stay letters.
        for text in [
            "He wrote the letter I sent.",
            "the Letter X",
            "a capital Letter I.",
            "Letter X marks the spot.",
            "“Letter I.”",
        ] {
            assert_eq!(read(text, false, EVERYWHERE), text, "{text}");
        }
        assert_eq!(read("The Letter X", true, TITLES), "The Letter X");
    }

    #[test]
    fn never_touches_the_pronoun() {
        let unchanged = [
            "I am here.",
            "I",
            "the chapter I wrote",
            "The Book I Read",
            "Book I read last week was long.",
            "Chapter I Remember",
            "the letter I sent",
            "the Letter X",
            "No I won't.",
            "No, I won't.",
            "No. I won't.",
            "Part I'm sure of.",
            "Then I left, and I never came back.",
            "Malcolm X spoke.",
            "Appendix C",
            "Plan D",
            "I, Robot",
            "I Am Legend",
            "X-Men",
            "V. S. Naipaul wrote it.",
            "I. M. Pei designed it.",
        ];
        for text in unchanged {
            assert_eq!(read(text, false, EVERYWHERE), text, "{text}");
        }
        // As a title, too: only numerals heading it with a stop.
        for text in ["I Am Legend", "I, Robot", "I Remember", "X-Men"] {
            assert_eq!(read(text, true, EVERYWHERE), text, "{text}");
        }
    }

    #[test]
    fn reads_titles_that_are_numerals() {
        assert_eq!(read("IV", true, TITLES), "four");
        assert_eq!(read("IV.", false, TITLES), "four.");
        assert_eq!(read("— II —", false, TITLES), "— two —");
        assert_eq!(read("I", true, TITLES), "one");
        assert_eq!(read("I.", true, TITLES), "one.");
        assert_eq!(read("IV. The Storm", true, TITLES), "four. The Storm");
        assert_eq!(read("I: Origins", true, TITLES), "one: Origins");
        assert_eq!(read("II — Home", true, TITLES), "two — Home");
        assert_eq!(
            read("I. We hold these truths", false, TITLES),
            "one. We hold these truths"
        );
        // A quoted "I." is someone speaking.
        assert_eq!(read("“I.”", false, TITLES), "“I.”");
    }

    #[test]
    fn reads_names_numbers_only_when_asked() {
        // Titles only: text keeps them.
        assert_eq!(
            read("Henry VIII had six wives.", false, TITLES),
            "Henry VIII had six wives."
        );
        assert_eq!(read("World War II", false, TITLES), "World War II");
        // In a heading they're titles.
        assert_eq!(
            read("The Reign of Henry VIII", true, TITLES),
            "The Reign of Henry the eighth"
        );
        assert_eq!(read("World War II", true, TITLES), "World War two");
        let cases = [
            (
                "Henry VIII had six wives.",
                "Henry the eighth had six wives.",
            ),
            ("After World War II ended", "After World War two ended"),
            (
                "Louis XIV built Versailles.",
                "Louis the fourteenth built Versailles.",
            ),
            ("Pope Pius XII", "Pope Pius the twelfth"),
            ("Henry V won.", "Henry the fifth won."),
            ("Elizabeth I's reign", "Elizabeth the first's reign"),
            (
                "Charles I, King of England",
                "Charles the first, King of England",
            ),
            ("in the XIX century", "in the nineteenth century"),
            ("Super Bowl LVII", "Super Bowl fifty-seven"),
            ("Apollo XIII", "Apollo thirteen"),
        ];
        for (text, read_as) in cases {
            assert_eq!(read(text, false, EVERYWHERE), read_as, "{text}");
        }
        // Not clearly numerals: left as they are.
        for text in [
            "Charles I was beheaded.",
            "the Mary I knew",
            "hooked to an IV drip",
            "a MIX of things",
            "Washington DC",
            "THE MIX TAPE",
            "Xi Jinping",
        ] {
            assert_eq!(read(text, false, EVERYWHERE), text, "{text}");
        }
    }

    #[test]
    fn reads_ordinals_when_asked() {
        let ordinals = Rules {
            ordinals: true,
            ..TITLES
        };
        assert_eq!(read("Chapter I", true, ordinals), "Chapter the first");
        assert_eq!(read("Book XXI", false, ordinals), "Book the twenty-first");
    }

    #[test]
    fn other_languages_get_digits() {
        let german = |text: &str| read_numerals(text, true, Some("de-DE"), TITLES);
        assert_eq!(german("Kapitel IV"), "Kapitel 4");
        assert_eq!(german("Kapitel I"), "Kapitel 1");
        assert_eq!(german("Teil II: Die Reise"), "Teil 2: Die Reise");
        assert_eq!(
            read_numerals("Capítulo I", false, Some("es"), TITLES),
            "Capítulo 1"
        );
        assert_eq!(
            read_numerals("Rozdział XII", false, Some("pl"), TITLES),
            "Rozdział 12"
        );
        // Polish "i" (and) stays a word.
        assert_eq!(
            read_numerals("Część I i II", false, Some("pl"), TITLES),
            "Część 1 i II"
        );
        // English numbering words work in any book; names don't (no English words for them).
        assert_eq!(
            read_numerals("Chapter IV", false, Some("fr"), EVERYWHERE),
            "Chapter 4"
        );
        assert_eq!(
            read_numerals("Henri IV", false, Some("fr"), EVERYWHERE),
            "Henri IV"
        );
    }

    #[test]
    fn does_nothing_when_off() {
        let off = Rules {
            titles: false,
            everywhere: false,
            ordinals: false,
        };
        assert_eq!(read("Chapter IV", true, off), "Chapter IV");
    }
}
