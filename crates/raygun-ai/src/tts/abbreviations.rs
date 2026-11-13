//! Abbreviations said in full before speech: voices read "St. Paul" as "Street Paul", "Dr." in
//! an address as "doctor", and some spell "e.g." out letter by letter. Only English text (or text
//! whose language isn't known) is touched, and only where the words around make the reading
//! clear: a title before a name ("Mr. Holmes", "St. Paul", "Mt. Everest"), a street in an address
//! ("221 Baker St.", "Mulholland Dr."), a suffix after a name ("Smith Jr."), a number's label
//! ("No. 5", "Dec. 11th") and the Latin ones ("e.g.", "i.e.", "etc.", "vs."). What isn't clear is
//! left as written, and the text shown never changes.

use std::ops::Range;

/// Capitalized words that start a sentence rather than name someone or somewhere: before one, the
/// stop after "St." or "Dr." ends the sentence ("He lives on Baker St. The house…").
const STARTERS: &[&str] = &[
    "The", "A", "An", "He", "She", "It", "They", "We", "I", "You", "His", "Her", "Its", "Their",
    "Our", "My", "Your", "This", "That", "These", "Those", "There", "Then", "But", "And", "So",
    "When", "While", "After", "Before", "As", "If", "In", "On", "At", "For", "With", "Now",
];

/// How an abbreviation is said.
#[derive(Debug, Clone, Copy)]
enum Rule {
    /// A form of address, said in full wherever it's found ("Mr.", "Mrs.").
    Person(&'static str),
    /// A title said in full before a name ("Prof. Moriarty", "Mt. Everest"); elsewhere it may be
    /// something else ("Gen. 1:1" is Genesis, "Rev. 3:20" Revelation).
    Title(&'static str),
    /// "St." and "Dr.": a street in an address, else a title before a name; `otherwise` where
    /// neither is clear ("Thank you, Dr.").
    StreetOrTitle {
        street: &'static str,
        title: &'static str,
        otherwise: Option<&'static str>,
    },
    /// A kind of street, said in full only in an address ("Fifth Ave.").
    Street(&'static str),
    /// After a name ("Smith Jr.", "Acme, Inc.").
    Suffix(&'static str),
    /// Said in full wherever it's found; `ends`: it may end a sentence ("…, etc.").
    Always { said: &'static str, ends: bool },
    /// Before a number ("No. 5", "p. 12", "Dec. 11th").
    Numbered(&'static str),
}

/// Whether the abbreviation must be written with its stop to be one ("Gen" alone is a word).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stop {
    Required,
    Optional,
}

#[derive(Debug)]
struct Entry {
    written: &'static str,
    stop: Stop,
    rule: Rule,
    /// Also in lower case, or capitalized at a sentence's start ("no. 5", "Cf.").
    any_case: bool,
}

const fn entry(written: &'static str, stop: Stop, rule: Rule) -> Entry {
    Entry {
        written,
        stop,
        rule,
        any_case: false,
    }
}

const fn any_case(written: &'static str, stop: Stop, rule: Rule) -> Entry {
    Entry {
        written,
        stop,
        rule,
        any_case: true,
    }
}

const fn title(written: &'static str, said: &'static str) -> Entry {
    entry(written, Stop::Required, Rule::Title(said))
}

const fn street(written: &'static str, said: &'static str) -> Entry {
    entry(written, Stop::Optional, Rule::Street(said))
}

const fn always(written: &'static str, said: &'static str, ends: bool) -> Entry {
    any_case(written, Stop::Required, Rule::Always { said, ends })
}

const fn numbered(written: &'static str, said: &'static str) -> Entry {
    any_case(written, Stop::Required, Rule::Numbered(said))
}

const fn month(written: &'static str, said: &'static str) -> Entry {
    entry(written, Stop::Required, Rule::Numbered(said))
}

const ENTRIES: &[Entry] = &[
    entry("Mr", Stop::Optional, Rule::Person("Mister")),
    entry("Mrs", Stop::Optional, Rule::Person("Missus")),
    entry("Ms", Stop::Optional, Rule::Person("Miz")),
    entry("Messrs", Stop::Optional, Rule::Person("Messers")),
    entry(
        "Dr",
        Stop::Optional,
        Rule::StreetOrTitle {
            street: "Drive",
            title: "Doctor",
            otherwise: Some("Doctor"),
        },
    ),
    entry(
        "St",
        Stop::Optional,
        Rule::StreetOrTitle {
            street: "Street",
            title: "Saint",
            otherwise: None,
        },
    ),
    entry("Mt", Stop::Optional, Rule::Title("Mount")),
    entry("Mme", Stop::Optional, Rule::Title("Madame")),
    entry("Mlle", Stop::Optional, Rule::Title("Mademoiselle")),
    title("Ft", "Fort"),
    title("Prof", "Professor"),
    title("Gen", "General"),
    title("Col", "Colonel"),
    title("Maj", "Major"),
    title("Capt", "Captain"),
    title("Lt", "Lieutenant"),
    title("Sgt", "Sergeant"),
    title("Cpl", "Corporal"),
    title("Adm", "Admiral"),
    title("Brig", "Brigadier"),
    title("Rev", "Reverend"),
    title("Hon", "Honourable"),
    title("Gov", "Governor"),
    title("Sen", "Senator"),
    title("Rep", "Representative"),
    title("Pres", "President"),
    title("Supt", "Superintendent"),
    title("Insp", "Inspector"),
    title("Det", "Detective"),
    title("Fr", "Father"),
    entry("Jr", Stop::Optional, Rule::Suffix("Junior")),
    entry("Sr", Stop::Optional, Rule::Suffix("Senior")),
    entry("Inc", Stop::Required, Rule::Suffix("Incorporated")),
    entry("Ltd", Stop::Required, Rule::Suffix("Limited")),
    street("Ave", "Avenue"),
    street("Rd", "Road"),
    street("Blvd", "Boulevard"),
    street("Ln", "Lane"),
    street("Hwy", "Highway"),
    street("Pkwy", "Parkway"),
    street("Sq", "Square"),
    any_case(
        "vs",
        Stop::Optional,
        Rule::Always {
            said: "versus",
            ends: false,
        },
    ),
    always("etc", "et cetera", true),
    always("cf", "compare", false),
    always("viz", "namely", false),
    always("approx", "approximately", false),
    numbered("No", "Number"),
    numbered("Nos", "Numbers"),
    numbered("Vol", "Volume"),
    numbered("Vols", "Volumes"),
    numbered("Ch", "Chapter"),
    numbered("Chap", "Chapter"),
    numbered("Pt", "Part"),
    numbered("Bk", "Book"),
    numbered("Fig", "Figure"),
    numbered("Figs", "Figures"),
    numbered("Art", "Article"),
    numbered("Sec", "Section"),
    numbered("Sect", "Section"),
    numbered("Ep", "Episode"),
    numbered("Op", "Opus"),
    entry("p", Stop::Required, Rule::Numbered("page")),
    entry("pp", Stop::Required, Rule::Numbered("pages")),
    entry("ca", Stop::Required, Rule::Numbered("circa")),
    month("Jan", "January"),
    month("Feb", "February"),
    month("Mar", "March"),
    month("Apr", "April"),
    month("Jun", "June"),
    month("Jul", "July"),
    month("Aug", "August"),
    month("Sep", "September"),
    month("Sept", "September"),
    month("Oct", "October"),
    month("Nov", "November"),
    month("Dec", "December"),
];

/// Written with dots inside: "e.g.", "i.e." (also "e. g.") and "&c.".
const DOTTED: &[(&str, &str)] = &[
    ("e.g.", "for example"),
    ("e. g.", "for example"),
    ("i.e.", "that is"),
    ("i. e.", "that is"),
    ("&c.", "et cetera"),
];

/// `text` with its English abbreviations said in full where the words around make clear what
/// they stand for. `language` is the text's (BCP 47); other languages are left as written.
pub fn read_abbreviations(text: &str, language: Option<&str>) -> String {
    let english = language.is_none_or(|tag| {
        tag.split(['-', '_'])
            .next()
            .is_some_and(|primary| primary.eq_ignore_ascii_case("en"))
    });
    if !english {
        return text.to_owned();
    }
    let bytes = text.as_bytes();
    let mut edits: Vec<(Range<usize>, String)> = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let byte = bytes[at];
        // Only ASCII bytes start one, so `at` is a character's start wherever it's sliced.
        if !(byte.is_ascii_alphabetic() || byte == b'&') || !starts_word(text, at) {
            at += 1;
            continue;
        }
        let end = at
            + bytes[at..]
                .iter()
                .take_while(|byte| byte.is_ascii_alphabetic())
                .count();
        match said_at(text, at, end) {
            Some((range, said)) => {
                at = range.end;
                edits.push((range, said));
            }
            None => at = end.max(at + 1),
        }
    }
    if edits.is_empty() {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len() + 32);
    let mut done = 0;
    for (range, said) in edits {
        out.push_str(&text[done..range.start]);
        out.push_str(&said);
        done = range.end;
    }
    out.push_str(&text[done..]);
    out
}

/// Whether the stop after the abbreviation at `start` (the text goes on at `after`) also ends
/// the sentence: one starts after it ("…Smith Jr. He wrote", "on Baker St. The house…"), or, for
/// a `street` one, a capitalized word follows an address ("221 Baker St. Paul lived…").
pub(crate) fn stop_ends(text: &str, start: usize, after: usize, street: bool) -> bool {
    match next_after(text, after) {
        Next::Capital(word) if STARTERS.contains(&word) => true,
        Next::Capital(_) => street && numbered_street(text, start, true),
        _ => false,
    }
}

/// Whether a word starts at `at`: not inside another, an address or a number.
fn starts_word(text: &str, at: usize) -> bool {
    text[..at].chars().next_back().is_none_or(|c| {
        !(c.is_alphanumeric() || matches!(c, '\'' | '’' | '-' | '.' | '/' | '@' | '_' | '&'))
    })
}

/// The abbreviation starting at `start` (its letters end at `end`), as said: the text it replaces
/// and the words.
fn said_at(text: &str, start: usize, end: usize) -> Option<(Range<usize>, String)> {
    if let Some(found) = dotted(text, start) {
        return Some(found);
    }
    // A word of its own: no letter or digit runs on ("Stéphane", "St2").
    if end == start || text[end..].starts_with(char::is_alphanumeric) {
        return None;
    }
    let (entry, case) = lookup(&text[start..end])?;
    let stop = text[end..].starts_with('.');
    if entry.stop == Stop::Required && !stop {
        return None;
    }
    let after = end + usize::from(stop);
    let numbered = matches!(entry.rule, Rule::Numbered(_));
    // The stop is the abbreviation's only before a space, a mark or the end ("St.Paul" isn't one,
    // "No.5" is).
    if stop
        && text[after..]
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || (c.is_ascii_digit() && !numbered))
    {
        return None;
    }
    let (said, keeps_stop) = reading(entry.rule, text, start, next_after(text, after))?;
    let mut spoken = case.write(said);
    if stop && keeps_stop {
        spoken.push('.');
    }
    // "No.5" is said "Number 5".
    if text[after..].starts_with(char::is_alphanumeric) {
        spoken.push(' ');
    }
    Some((start..after, spoken))
}

/// What `rule` says at `start`, with `next` after it: the words, and whether the stop stays as
/// the sentence's (it may end there).
fn reading(rule: Rule, text: &str, start: usize, next: Next<'_>) -> Option<(&'static str, bool)> {
    let name = next.is_name();
    let ends = next.may_start_sentence();
    match rule {
        Rule::Person(said) => Some((said, !name && ends)),
        Rule::Title(said) => name.then_some((said, false)),
        Rule::StreetOrTitle {
            street,
            title,
            otherwise,
        } => {
            if is_street(text, start, next) {
                Some((street, ends))
            } else if name {
                Some((title, false))
            } else {
                otherwise.map(|said| (said, ends))
            }
        }
        Rule::Street(said) => is_street(text, start, next).then_some((said, ends)),
        Rule::Suffix(said) => after_name(text, start).then_some((said, ends)),
        Rule::Always {
            said,
            ends: may_end,
        } => Some((said, may_end && ends)),
        Rule::Numbered(said) => (next == Next::Digit).then_some((said, false)),
    }
}

/// "e.g.", "i.e." and "&c." at `start`, as said.
fn dotted(text: &str, start: usize) -> Option<(Range<usize>, String)> {
    let rest = &text[start..];
    for (form, said) in DOTTED {
        let (first, tail) = form.split_at(1);
        if !rest[1..].starts_with(tail) {
            continue;
        }
        let capital = match &rest[..1] {
            written if written == first => false,
            written if written.eq_ignore_ascii_case(first) => true,
            _ => continue,
        };
        let end = start + form.len();
        if text[end..].starts_with(char::is_alphanumeric) {
            return None;
        }
        let mut spoken = if capital {
            capitalized(said)
        } else {
            (*said).to_owned()
        };
        if first == "&" && next_after(text, end).may_start_sentence() {
            spoken.push('.');
        }
        return Some((start..end, spoken));
    }
    None
}

/// How a full form is written: as listed, or in the case of the abbreviation it stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Case {
    AsListed,
    Capital,
    Lower,
}

impl Case {
    fn write(self, said: &str) -> String {
        match self {
            Self::AsListed => said.to_owned(),
            Self::Capital => capitalized(said),
            Self::Lower => {
                let mut chars = said.chars();
                chars.next().map_or_else(String::new, |first| {
                    first.to_lowercase().chain(chars).collect()
                })
            }
        }
    }
}

/// The entry for `word`: written as listed, or (where the entry allows) in lower case or
/// capitalized at a sentence's start.
fn lookup(word: &str) -> Option<(&'static Entry, Case)> {
    if let Some(found) = ENTRIES.iter().find(|entry| entry.written == word) {
        return Some((found, Case::AsListed));
    }
    let mut letters = word.chars();
    let first = letters.next()?;
    if !letters.all(char::is_lowercase) {
        return None;
    }
    let found = ENTRIES
        .iter()
        .find(|entry| entry.any_case && entry.written.eq_ignore_ascii_case(word))?;
    let case = if first.is_uppercase() {
        Case::Capital
    } else {
        Case::Lower
    };
    Some((found, case))
}

fn capitalized(said: &str) -> String {
    let mut chars = said.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// What follows an abbreviation (after spaces).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Next<'a> {
    End,
    /// A capitalized word: its letters.
    Capital(&'a str),
    Lower,
    Digit,
    /// A comma, a bracket, a quote…
    Mark,
}

impl Next<'_> {
    /// A capitalized word that doesn't start a sentence: a name.
    fn is_name(self) -> bool {
        matches!(self, Self::Capital(word) if !STARTERS.contains(&word))
    }

    /// The end, or a capitalized word: a sentence may start after the stop.
    fn may_start_sentence(self) -> bool {
        matches!(self, Self::End | Self::Capital(_))
    }
}

fn next_after(text: &str, at: usize) -> Next<'_> {
    let rest = text[at..].trim_start();
    let Some(first) = rest.chars().next() else {
        return Next::End;
    };
    if first.is_uppercase() {
        let length = rest
            .find(|c: char| !c.is_alphabetic())
            .unwrap_or(rest.len());
        Next::Capital(&rest[..length])
    } else if first.is_lowercase() {
        Next::Lower
    } else if first.is_ascii_digit() {
        Next::Digit
    } else {
        Next::Mark
    }
}

/// Whether "St.", "Dr." or "Ave." at `start` is a street: after a house number and the street's
/// name ("221 Baker St.", "42nd St."), or after a capitalized word with no name following
/// ("Baker St., London", "on Baker St. The house…").
fn is_street(text: &str, start: usize, next: Next<'_>) -> bool {
    let name = next.is_name();
    numbered_street(text, start, name) || (!name && capital_word(token_before(text, start)))
}

/// Whether a house number and one to three capitalized words come right before `start` ("221
/// Baker St.", "1600 Pennsylvania Ave."), or a street's ordinal number ("42nd St."). Before a
/// name, a year isn't a house number ("In 1850 London St. Paul's…").
fn numbered_street(text: &str, start: usize, name_follows: bool) -> bool {
    let mut cursor = start;
    let mut names = 0;
    while let Some((at, token)) = token_before(text, cursor) {
        let Some(first) = token.chars().next() else {
            return false;
        };
        if first.is_ascii_digit() {
            let year = token.len() == 4 && token.bytes().all(|byte| byte.is_ascii_digit());
            return if names == 0 {
                is_ordinal(token)
            } else {
                !(name_follows && year)
            };
        }
        if !first.is_uppercase() || names == 3 {
            return false;
        }
        names += 1;
        cursor = at;
    }
    false
}

/// "1st", "42nd", "3rd", "5th".
fn is_ordinal(token: &str) -> bool {
    let digits = token.bytes().take_while(u8::is_ascii_digit).count();
    digits > 0 && matches!(&token[digits..], "st" | "nd" | "rd" | "th")
}

/// Whether `start` follows a name: a capitalized word, or one and a comma ("Davis, Jr.").
fn after_name(text: &str, start: usize) -> bool {
    if capital_word(token_before(text, start)) {
        return true;
    }
    let head = text[..start].trim_end();
    head.len() < start && head.ends_with(',') && capital_word(token_ending(text, head.len() - 1))
}

fn capital_word(token: Option<(usize, &str)>) -> bool {
    token.is_some_and(|(_, token)| token.starts_with(char::is_uppercase))
}

/// The word or number right before `at`, with only spaces between: where it starts, and it.
fn token_before(text: &str, at: usize) -> Option<(usize, &str)> {
    let head = &text[..at];
    let trimmed = head.trim_end();
    if trimmed.len() == head.len() {
        return None;
    }
    token_ending(text, trimmed.len())
}

/// The word or number ending at `end`: where it starts, and it.
fn token_ending(text: &str, end: usize) -> Option<(usize, &str)> {
    let head = &text[..end];
    let start = head
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_alphanumeric() || matches!(c, '\'' | '’' | '-'))
        .last()?
        .0;
    Some((start, &head[start..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> String {
        read_abbreviations(text, Some("en"))
    }

    #[test]
    fn says_saints_and_streets() {
        assert_eq!(read("St. Paul wrote to them."), "Saint Paul wrote to them.");
        assert_eq!(read("the St. Lawrence River"), "the Saint Lawrence River");
        assert_eq!(read("St Albans is near."), "Saint Albans is near.");
        assert_eq!(
            read("He lived at 221 Baker St."),
            "He lived at 221 Baker Street."
        );
        assert_eq!(
            read("221B Baker St. was his address"),
            "221B Baker Street was his address"
        );
        assert_eq!(
            read("a shop on Baker St., London"),
            "a shop on Baker Street, London"
        );
        assert_eq!(
            read("on Baker St. The house was old."),
            "on Baker Street. The house was old."
        );
        assert_eq!(
            read("on 42nd St. and Fifth Ave."),
            "on 42nd Street and Fifth Avenue."
        );
        assert_eq!(
            read("St. Petersburgh, Dec. 11th, 17—."),
            "Saint Petersburgh, December 11th, 17—."
        );
        // A surname, a café, and a year before a place: saints all.
        assert_eq!(read("John St. John came"), "John Saint John came");
        assert_eq!(read("Café St. Germain"), "Café Saint Germain");
        assert_eq!(
            read("In 1850 London St. Paul's stood"),
            "In 1850 London Saint Paul's stood"
        );
        // Neither a name after it nor a street before it: as written.
        assert_eq!(read("the St. of it"), "the St. of it");
    }

    #[test]
    fn says_doctors_and_drives() {
        assert_eq!(read("Dr. Watson came in."), "Doctor Watson came in.");
        assert_eq!(read("said Dr Watson"), "said Doctor Watson");
        assert_eq!(read("Dear Dr. Smith,"), "Dear Doctor Smith,");
        assert_eq!(read("the good Dr. Jekyll"), "the good Doctor Jekyll");
        assert_eq!(read("Dr. No and Dr. Who"), "Doctor No and Doctor Who");
        assert_eq!(read("Thank you, Dr."), "Thank you, Doctor.");
        assert_eq!(
            read("a house on Mulholland Dr."),
            "a house on Mulholland Drive."
        );
        assert_eq!(
            read("on Mulholland Dr. and then home"),
            "on Mulholland Drive and then home"
        );
        assert_eq!(
            read("at 12 Elm Dr., Springfield"),
            "at 12 Elm Drive, Springfield"
        );
        assert_eq!(read("the Dr.'s office"), "the Doctor's office");
    }

    #[test]
    fn says_forms_of_address_and_titles_before_names() {
        assert_eq!(read("Mr. and Mrs. Hudson"), "Mister and Missus Hudson");
        assert_eq!(read("Ms. Marple"), "Miz Marple");
        assert_eq!(read("Mr Darcy smiled."), "Mister Darcy smiled.");
        assert_eq!(read("said Mr."), "said Mister.");
        assert_eq!(read("Messrs. Smith and Jones"), "Messers Smith and Jones");
        assert_eq!(
            read("Mt. Everest and Mt Fuji"),
            "Mount Everest and Mount Fuji"
        );
        assert_eq!(read("Ft. Worth"), "Fort Worth");
        assert_eq!(read("Prof. Moriarty"), "Professor Moriarty");
        assert_eq!(
            read("Gen. Grant and Col. Brandon"),
            "General Grant and Colonel Brandon"
        );
        assert_eq!(
            read("Fr. Brown and Rev. Collins"),
            "Father Brown and Reverend Collins"
        );
        assert_eq!(read("Mme Bovary"), "Madame Bovary");
        // Not before a name: books of the Bible, a verse, a word of its own.
        assert_eq!(
            read("See Gen. 1:1 and Rev. 3:20."),
            "See Gen. 1:1 and Rev. 3:20."
        );
        assert_eq!(read("Mt. 5:3"), "Mt. 5:3");
        assert_eq!(read("Gen Z"), "Gen Z");
        assert_eq!(read("He did one more rep."), "He did one more rep.");
    }

    #[test]
    fn says_suffixes_after_names() {
        assert_eq!(
            read("Martin Luther King Jr. was born"),
            "Martin Luther King Junior was born"
        );
        assert_eq!(
            read("by John Smith Jr. He wrote"),
            "by John Smith Junior. He wrote"
        );
        assert_eq!(read("Sammy Davis, Jr., sang"), "Sammy Davis, Junior, sang");
        assert_eq!(
            read("Smith Sr. and Smith Jr."),
            "Smith Senior and Smith Junior."
        );
        assert_eq!(
            read("Acme, Inc. makes them"),
            "Acme, Incorporated makes them"
        );
        assert_eq!(read("the Sr. on duty"), "the Sr. on duty");
    }

    #[test]
    fn says_latin_ones() {
        assert_eq!(
            read("fruit, e.g. apples, i.e. food"),
            "fruit, for example apples, that is food"
        );
        assert_eq!(read("E.g., this one."), "For example, this one.");
        assert_eq!(
            read("e. g. this, i. e. that"),
            "for example this, that is that"
        );
        assert_eq!(
            read("tea, coffee, etc. Then we left."),
            "tea, coffee, et cetera. Then we left."
        );
        assert_eq!(read("tea, etc., and cake"), "tea, et cetera, and cake");
        assert_eq!(read("(tea, etc.)"), "(tea, et cetera)");
        assert_eq!(read("Kramer vs. Kramer"), "Kramer versus Kramer");
        assert_eq!(read("Kramer vs Kramer"), "Kramer versus Kramer");
        assert_eq!(read("Cf. the first"), "Compare the first");
        assert_eq!(read("bread, butter, &c."), "bread, butter, et cetera.");
        assert_eq!(read("approx. 5 miles"), "approximately 5 miles");
    }

    #[test]
    fn says_labels_of_numbers_and_dates() {
        assert_eq!(read("See No. 5 and no. 6"), "See Number 5 and number 6");
        assert_eq!(
            read("Vol. 2, p. 12 and pp. 30–34"),
            "Volume 2, page 12 and pages 30–34"
        );
        assert_eq!(read("Dec. 11th"), "December 11th");
        assert_eq!(read("Sept. 3, 1752"), "September 3, 1752");
        assert_eq!(read("No.5"), "Number 5");
        assert_eq!(read("Fig. 3 shows ca. 1850"), "Figure 3 shows circa 1850");
        // Without a number they may be words.
        assert_eq!(read("No. I won't."), "No. I won't.");
        assert_eq!(read("It snowed in Dec."), "It snowed in Dec.");
    }

    #[test]
    fn leaves_other_words_and_languages_alone() {
        assert_eq!(read("Stephen, Drew and Mrsa"), "Stephen, Drew and Mrsa");
        assert_eq!(read("www.st.com and a@dr.org"), "www.st.com and a@dr.org");
        assert_eq!(read("U.S. Army"), "U.S. Army");
        assert_eq!(read("ST. PAUL"), "ST. PAUL");
        assert_eq!(read_abbreviations("St. Pauli", Some("de")), "St. Pauli");
        assert_eq!(read_abbreviations("St. Paul", None), "Saint Paul");
        assert_eq!(read_abbreviations("St. Paul", Some("en-GB")), "Saint Paul");
        assert_eq!(read(""), "");
        assert_eq!(read("Ünïcödé — “quotes” …"), "Ünïcödé — “quotes” …");
    }

    #[test]
    fn says_where_a_stop_also_ends_the_sentence() {
        let ends = |text: &str, abbreviation: &str| {
            let start = text.find(abbreviation).unwrap();
            stop_ends(
                text,
                start,
                start + abbreviation.len(),
                abbreviation.starts_with("St"),
            )
        };
        assert!(ends("He lives on Baker St. The house is old.", "St."));
        assert!(ends("I live at 221 Baker St. Paul lives next door.", "St."));
        assert!(ends("by John Smith Jr. He wrote", "Jr."));
        assert!(!ends("He prayed to St. Paul.", "St."));
        assert!(!ends("Martin Luther King Jr. Day", "Jr."));
        assert!(!ends("In 1850 London St. Paul's stood", "St."));
        assert!(!ends("Baker St., London", "St."));
    }
}
