//! Audiobook names as listeners read them. Sources name books the way their catalogues file
//! them ("Count of Monte Cristo, The by Alexandre Dumas (1802 - 1870)", "Alice's Adventures in
//! Wonderland, by Lewis Carroll", "Pride and Prejudice (version 3)") and chapters the way their
//! files are numbered ("01 - Chapter 01", "Chapter 001-002"). These give the book's own title, its
//! author and which recording it is, and chapter titles without the file numbering. Feeds keep
//! what they said; names are tidied when they're shown, so every list reads the same.

use crate::podcast_feed::decode_entities;

/// A book's name, tidied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookName {
    pub title: String,
    pub author: Option<String>,
    /// Which recording this is when a source has several: "Version 2", "Dramatic Reading".
    pub edition: Option<String>,
}

/// Who sources put where a book's author goes when it's the publisher, not the writer.
const PUBLISHERS: [&str; 7] = [
    "librivox",
    "librivox volunteers",
    "loyal books",
    "loyalbooks",
    "lit2go",
    "project gutenberg",
    "books should be free",
];

/// Articles catalogues move to the end ("Count of Monte Cristo, The").
const ARTICLES: [&str; 27] = [
    "The", "A", "An", "Le", "La", "Les", "L'", "L’", "El", "Los", "Las", "Lo", "Der", "Die", "Das",
    "Ein", "Eine", "Il", "Gli", "Un", "Une", "Una", "De", "Het", "Een", "Os", "As",
];

/// Words that say who else worked on a book: "transl. Helen Zimmern", "ed. Frank Pine".
const ROLES: [&str; 12] = [
    "tr.",
    "trans.",
    "transl",
    "translated",
    "translator",
    "ed.",
    "edited",
    "editor",
    "illustrated",
    "read by",
    "narrated",
    "compiled",
];

/// Words that number a part of a book ("Chapter 01", "Bk 01, ch. 01").
const PART_WORDS: [&str; 24] = [
    "chapter",
    "chapters",
    "ch",
    "chap",
    "bk",
    "book",
    "part",
    "parts",
    "section",
    "sec",
    "vol",
    "volume",
    "canto",
    "act",
    "scene",
    "letter",
    "track",
    "capítulo",
    "capitulo",
    "kapitel",
    "chapitre",
    "hoofdstuk",
    "capitolo",
    "parte",
];

/// A book's title, author and edition from what a feed or directory calls it. `author` is the
/// source's author field, which may name the publisher ("LibriVox") rather than the writer.
pub fn book(title: &str, author: Option<&str>) -> BookName {
    let title = squeeze(&decode_entities(title));
    let given = author
        .map(|author| squeeze(&decode_entities(author)))
        .filter(|author| !author.is_empty());
    let publisher = given.as_deref().is_none_or(is_publisher);
    let mut book = title.clone();
    let mut writer = None;
    if let Some((front, back)) = split_by(&title) {
        // A publisher's feed names the writer in its title ("<book> by <writer> (dates)");
        // otherwise only a title that names the author again loses it.
        if publisher
            || given
                .as_deref()
                .is_some_and(|given| same_person(&back, given))
        {
            book = front;
            writer = Some(back);
        }
    }
    let (book, edition) = take_editions(&book);
    let book = article_to_front(&book);
    let book = if book.is_empty() { title } else { book };
    let author = match (writer, given) {
        (Some(writer), Some(given)) if !publisher => person(&given).or_else(|| person(&writer)),
        (Some(writer), given) => person(&writer).or_else(|| given.as_deref().and_then(person)),
        (None, given) => given.as_deref().and_then(person),
    };
    BookName {
        title: book,
        author,
        edition,
    }
}

/// A person's name as a byline: dates, translators and editors left out, "Last, First" turned
/// round ("Townsend, George Fyler; tr. Jorge R. Rodríguez" is "George Fyler Townsend").
pub fn person(name: &str) -> Option<String> {
    let name = squeeze(&decode_entities(name));
    let people: Vec<String> = name
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty() && !starts_with_role(part))
        .map(|part| {
            let part = without_parentheses(part, |inner| {
                inner.chars().any(|ch| ch.is_ascii_digit()) || starts_with_role(inner)
            });
            let part = before_role(&part);
            turn_round(part.trim().trim_end_matches([',', '.']).trim())
        })
        .filter(|part| !part.is_empty())
        .collect();
    let joined = squeeze(&people.join(", "));
    (!joined.is_empty()).then_some(joined)
}

/// A chapter's title without the file's numbering: "01 - Chapter 01" is "Chapter 1", "00 -
/// Dramatis Personae" is "Dramatis Personae", "Chapter 001-002" is "Chapters 1-2". `book` is the
/// book's tidied title, left out when a chapter repeats it ("Moby Dick - Chapter 3").
pub fn chapter(title: &str, book: &str) -> String {
    let original = squeeze(&decode_entities(title));
    let mut text = original.clone();
    if let Some(rest) = after_book(&text, book) {
        text = rest;
    }
    if let Some(rest) = after_track_number(&text) {
        text = rest;
    }
    text = without_reader(&text);
    text = unpadded(&text);
    if let Some(rest) = after_part_zero(&text) {
        text = rest;
    }
    if text.is_empty() { original } else { text }
}

/// Whether two names are the same person, however the sources write them ("Sir Arthur Conan
/// Doyle" is "Arthur Conan Doyle", "Carroll, Lewis" is "Lewis Carroll").
pub fn same_person(a: &str, b: &str) -> bool {
    let (a, b) = (person(a).unwrap_or_default(), person(b).unwrap_or_default());
    let (a, b) = (simple(&a), simple(&b));
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    !short.is_empty() && (short == long || (short.len() >= 6 && long.ends_with(&short)))
}

/// Letters and digits only, lower case: how titles are compared.
pub fn simple(text: &str) -> String {
    text.chars()
        .filter(|letter| letter.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn is_publisher(name: &str) -> bool {
    let name = name.trim().to_lowercase();
    PUBLISHERS.contains(&name.as_str())
}

/// Spaces run together and trimmed, and no space before a comma.
fn squeeze(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for word in text.split(|ch: char| ch.is_whitespace()) {
        if word.is_empty() {
            continue;
        }
        if !out.is_empty() && !word.starts_with([',', ';']) {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}

/// "<book> by <writer>" or "<book>, by <writer>" at the last " by " that isn't "translated by"
/// or "read by".
fn split_by(title: &str) -> Option<(String, String)> {
    let lower = title.to_ascii_lowercase();
    let at = lower.rmatch_indices(" by ").map(|(at, _)| at).find(|at| {
        let word = lower[..*at].rsplit(' ').next().unwrap_or_default();
        !matches!(
            word,
            "translated" | "edited" | "read" | "illustrated" | "narrated" | "compiled"
        )
    })?;
    let front = title[..at].trim().trim_end_matches(',').trim();
    let back = title[at + 4..].trim();
    (!front.is_empty() && !back.is_empty() && back.chars().any(char::is_alphabetic))
        .then(|| (front.to_owned(), back.to_owned()))
}

/// Parenthesised notes that say which recording this is, taken out of the title.
fn take_editions(title: &str) -> (String, Option<String>) {
    let mut editions = Vec::new();
    let rest = without_parentheses(title, |inner| {
        let lower = inner.trim().to_lowercase();
        let edition = lower.contains("version")
            || lower.starts_with("read by")
            || matches!(
                lower.as_str(),
                "solo" | "dramatic reading" | "dramatized" | "dramatised" | "abridged"
            );
        if edition {
            editions.push(edition_label(inner.trim()));
        }
        edition
    });
    let rest = squeeze(&rest).trim_end_matches(',').trim().to_owned();
    let edition = (!editions.is_empty()).then(|| editions.join(", "));
    (rest, edition)
}

/// "version 02" reads "Version 2".
fn edition_label(text: &str) -> String {
    let text = unpad_all(text);
    let mut letters = text.chars();
    match letters.next() {
        Some(first) => first.to_uppercase().chain(letters).collect(),
        None => text,
    }
}

/// `text` without the parenthesised parts `drop` picks; the others stay.
fn without_parentheses(text: &str, mut drop: impl FnMut(&str) -> bool) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('(') {
        let Some(close) = rest[open..].find(')').map(|close| open + close) else {
            break;
        };
        out.push_str(&rest[..open]);
        let inner = &rest[open + 1..close];
        if !drop(inner) {
            out.push_str(&rest[open..=close]);
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    squeeze(&out)
}

/// "Count of Monte Cristo, The" reads "The Count of Monte Cristo"; "Homme qui rit, L'" reads
/// "L'Homme qui rit".
fn article_to_front(title: &str) -> String {
    let trimmed = title.trim();
    if let Some((front, last)) = trimmed.rsplit_once(',') {
        // "Iliad, The (Butler's translation)": a note may follow the article.
        let (word, note) = last
            .trim()
            .split_once(" (")
            .map_or((last.trim(), ""), |(word, _)| {
                (word, &last.trim()[word.len()..])
            });
        if let Some(article) = ARTICLES.iter().find(|article| **article == word)
            && !front.trim().is_empty()
        {
            let joiner = if article.ends_with(['\'', '’']) {
                ""
            } else {
                " "
            };
            return format!("{article}{joiner}{}{note}", front.trim());
        }
    }
    trimmed.to_owned()
}

fn starts_with_role(text: &str) -> bool {
    let lower = text.trim().to_lowercase();
    ROLES.iter().any(|role| lower.starts_with(role))
}

/// A name without ", ed. …" or ", translated by …" after it.
fn before_role(name: &str) -> String {
    let lower = name.to_lowercase();
    let cut = lower.match_indices(", ").find_map(|(at, _)| {
        let after = &lower[at + 2..];
        ROLES
            .iter()
            .any(|role| after.starts_with(role))
            .then_some(at)
    });
    match cut {
        Some(at) => name[..at].to_owned(),
        None => name.to_owned(),
    }
}

/// "Austen, Jane" reads "Jane Austen" ("de Cervantes, Miguel" too); "Smith, Jr." and lists of
/// names ("Jane Austen, Charlotte Brontë") stay as they are.
fn turn_round(name: &str) -> String {
    let parts: Vec<&str> = name.split(',').map(str::trim).collect();
    if let [last, first] = parts[..] {
        let suffix = matches!(
            first.to_lowercase().trim_end_matches('.'),
            "jr" | "sr" | "ii" | "iii" | "iv" | "phd" | "md" | "esq"
        );
        let plain = |part: &str| {
            !part.is_empty()
                && part.chars().all(|ch| !ch.is_ascii_digit())
                && !part.contains('&')
                && !part
                    .to_lowercase()
                    .split_whitespace()
                    .any(|word| word == "and")
        };
        let words: Vec<&str> = last.split_whitespace().collect();
        let surname = match words[..] {
            [_] => true,
            [particle, _] => matches!(
                particle,
                "de" | "da" | "di" | "du" | "van" | "von" | "le" | "la" | "del" | "della" | "ten"
            ),
            _ => false,
        };
        if !suffix
            && surname
            && plain(last)
            && plain(first)
            && first.split_whitespace().count() <= 3
        {
            return format!("{first} {last}");
        }
    }
    name.to_owned()
}

/// The rest of a chapter title that starts by naming the book.
fn after_book(text: &str, book: &str) -> Option<String> {
    let book = book.trim();
    if book.is_empty() || text.len() <= book.len() {
        return None;
    }
    let head = text.get(..book.len())?;
    if !head.eq_ignore_ascii_case(book) && head.to_lowercase() != book.to_lowercase() {
        return None;
    }
    let tail = &text[book.len()..];
    // A separator follows ("Moby Dick - Chapter 3"), or a part word ("Moby Dick Chapter 3").
    let spaced = tail.trim_start();
    let separated = spaced.starts_with([':', '-', '–', '—', ',', '.', '|']);
    let rest = spaced
        .trim_start_matches([' ', ':', '-', '–', '—', ',', '.', '|'])
        .trim();
    let names_part = rest.split(' ').next().is_some_and(is_part_word);
    let follows = tail.starts_with([' ', ':', ',', '.', '|', '-', '–', '—']);
    (follows && (separated || names_part) && rest.chars().any(char::is_alphanumeric))
        .then(|| rest.to_owned())
}

/// The rest of a title that starts with a file's number: "01 - ", "12. ", "3) ", "07 – ".
fn after_track_number(text: &str) -> Option<String> {
    let digits = text.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 || digits > 3 {
        return None;
    }
    let rest = text[digits..].trim_start();
    let mut chars = rest.chars();
    let separator = chars.next()?;
    if !matches!(separator, '-' | '–' | '—' | '.' | ')' | ':' | '_' | '|') {
        return None;
    }
    let after = chars.as_str();
    if !after.starts_with(char::is_whitespace) {
        return None;
    }
    let after = after.trim();
    after
        .chars()
        .any(char::is_alphabetic)
        .then(|| after.to_owned())
}

/// A title without the reader's name at its end: "Chapter 1 - read by Ann", "(read by Ann)".
fn without_reader(text: &str) -> String {
    let lower = text.to_lowercase();
    let Some(at) = lower.rfind("read by") else {
        return text.to_owned();
    };
    let before = text[..at].trim_end();
    let opened = before.ends_with(['(', '[']);
    let separated = opened || before.ends_with(['-', '–', '—', ',', ':', '|']);
    if !separated {
        return text.to_owned();
    }
    let kept = before
        .trim_end_matches(['(', '[', '-', '–', '—', ',', ':', '|'])
        .trim_end();
    if kept.chars().any(char::is_alphanumeric) {
        kept.to_owned()
    } else {
        text.to_owned()
    }
}

/// Numbers after a part word lose their padding ("Chapter 001-002" reads "Chapters 1-2").
fn unpadded(text: &str) -> String {
    let words: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    for (index, word) in words.iter().enumerate() {
        let after_part = index > 0 && is_part_word(words[index - 1]);
        if after_part && word.starts_with(|ch: char| ch.is_ascii_digit()) {
            let unpadded = unpad_all(word);
            // "Chapter 1-2" names two chapters.
            if words[index - 1] == "Chapter"
                && is_range(&unpadded)
                && let Some(previous) = out.last_mut()
            {
                *previous = "Chapters".to_owned();
            }
            out.push(unpadded);
        } else {
            out.push((*word).to_owned());
        }
    }
    out.join(" ")
}

fn is_part_word(word: &str) -> bool {
    let word = word.trim_end_matches(['.', ',']).to_lowercase();
    PART_WORDS.contains(&word.as_str())
}

fn is_range(word: &str) -> bool {
    let word = word.trim_end_matches([':', ',', '.']);
    word.split_once(['-', '–'])
        .is_some_and(|(from, to)| is_number(from) && is_number(to))
}

fn is_number(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|ch| ch.is_ascii_digit())
}

/// Every run of digits without its leading zeros: "001-002:" reads "1-2:".
fn unpad_all(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_number = false;
    while let Some(ch) = chars.next() {
        if ch.is_ascii_digit() {
            let padding = ch == '0' && !in_number && chars.peek().is_some_and(char::is_ascii_digit);
            if !padding {
                out.push(ch);
                in_number = true;
            }
        } else {
            in_number = false;
            out.push(ch);
        }
    }
    out
}

/// "Chapter 0: Etymology and Extracts" reads "Etymology and Extracts".
fn after_part_zero(text: &str) -> Option<String> {
    let (word, rest) = text.split_once(' ')?;
    if !is_part_word(word) {
        return None;
    }
    let rest = rest.strip_prefix('0')?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix([':', '-', '–', '—', '.'])?.trim();
    (rest.starts_with(char::is_alphabetic)).then(|| rest.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(title: &str, author: Option<&str>) -> (String, Option<String>, Option<String>) {
        let name = book(title, author);
        (name.title, name.author, name.edition)
    }

    fn owned(
        title: &str,
        author: Option<&str>,
        edition: Option<&str>,
    ) -> (String, Option<String>, Option<String>) {
        (
            title.to_owned(),
            author.map(str::to_owned),
            edition.map(str::to_owned),
        )
    }

    /// Titles of LibriVox's own feeds (`librivox.org/rss/<n>`), as they read in November 2026.
    #[test]
    fn librivox_feed_titles_give_the_book_its_author_and_the_recording() {
        for (title, expected) in [
            (
                "Pride and Prejudice by Jane Austen (1775 - 1817)",
                owned("Pride and Prejudice", Some("Jane Austen"), None),
            ),
            (
                "Count of Monte Cristo, The by Alexandre Dumas (1802 - 1870)",
                owned("The Count of Monte Cristo", Some("Alexandre Dumas"), None),
            ),
            (
                "Adventures of Sherlock Holmes (version 4), The by Sir Arthur Conan Doyle (1859 - 1930)",
                owned(
                    "The Adventures of Sherlock Holmes",
                    Some("Sir Arthur Conan Doyle"),
                    Some("Version 4"),
                ),
            ),
            (
                "Misérables, Volume 1, Les by Victor Hugo (1802 - 1885)",
                owned("Les Misérables, Volume 1", Some("Victor Hugo"), None),
            ),
            (
                "Fábulas de Esopo, Vol. 1, Las by Aesop (c. 620 BCE - 564)",
                owned("Las Fábulas de Esopo, Vol. 1", Some("Aesop"), None),
            ),
            (
                "Jane Eyre (version 2) by Charlotte Brontë (1816 - 1855)",
                owned("Jane Eyre", Some("Charlotte Brontë"), Some("Version 2")),
            ),
            (
                "Wuthering Heights (Version 2) by Emily Brontë (1818 - 1848)",
                owned("Wuthering Heights", Some("Emily Brontë"), Some("Version 2")),
            ),
            (
                "Alice's Adventures in Wonderland (Dramatic Reading) by Lewis Carroll (1832 - 1898)",
                owned(
                    "Alice's Adventures in Wonderland",
                    Some("Lewis Carroll"),
                    Some("Dramatic Reading"),
                ),
            ),
            (
                "Ghost Story Collection 001 by Various",
                owned("Ghost Story Collection 001", Some("Various"), None),
            ),
            (
                "Grimms' Fairy Tales by Jacob Grimm (1785 - 1863) and Wilhelm Grimm (1786 - 1859)",
                owned(
                    "Grimms' Fairy Tales",
                    Some("Jacob Grimm and Wilhelm Grimm"),
                    None,
                ),
            ),
            (
                "Stories by English Authors: Africa by Various",
                owned("Stories by English Authors: Africa", Some("Various"), None),
            ),
            (
                "Don Quijote 1 by Miguel de Cervantes Saavedra (1547 - 1616)",
                owned("Don Quijote 1", Some("Miguel de Cervantes Saavedra"), None),
            ),
        ] {
            assert_eq!(named(title, Some("LibriVox")), expected, "{title}");
        }
        assert_eq!(
            named("Short Poetry Collection 281", Some("LibriVox")),
            owned("Short Poetry Collection 281", Some("LibriVox"), None),
            "no writer named: the publisher stays"
        );
    }

    /// Loyal Books' feeds (`/book/<slug>/feed`): "<book> by <author>", the author "Loyal Books".
    #[test]
    fn loyal_books_feed_titles_lose_the_publisher() {
        assert_eq!(
            named(
                "Alice's Adventures in Wonderland by Lewis Carroll",
                Some("Loyal Books")
            ),
            owned(
                "Alice's Adventures in Wonderland",
                Some("Lewis Carroll"),
                None
            )
        );
        assert_eq!(
            named("The Art of War by Sun Tzu", Some("Loyal Books")),
            owned("The Art of War", Some("Sun Tzu"), None)
        );
    }

    /// The Internet Archive's `librivoxaudio` titles and creators.
    #[test]
    fn archive_titles_and_creators() {
        assert_eq!(
            named(
                "Alice's Adventures in Wonderland, by Lewis Carroll",
                Some("Lewis Carroll")
            ),
            owned(
                "Alice's Adventures in Wonderland",
                Some("Lewis Carroll"),
                None
            )
        );
        assert_eq!(
            named("Pride and Prejudice (version 3)", Some("Jane Austen")),
            owned(
                "Pride and Prejudice",
                Some("Jane Austen"),
                Some("Version 3")
            )
        );
        assert_eq!(
            named(
                "Adventures of Huckleberry Finn (version 02)",
                Some("Mark Twain")
            ),
            owned(
                "Adventures of Huckleberry Finn",
                Some("Mark Twain"),
                Some("Version 2")
            )
        );
        assert_eq!(
            named("Wuthering Heights (Solo Version)", Some("Emily Bronte")),
            owned(
                "Wuthering Heights",
                Some("Emily Bronte"),
                Some("Solo Version")
            )
        );
        assert_eq!(
            named("Aesop's Fables, Volume 1 (Fables 1-25)", Some("Aesop")),
            owned(
                "Aesop's Fables, Volume 1 (Fables 1-25)",
                Some("Aesop"),
                None
            ),
            "other notes stay"
        );
        assert_eq!(
            named(
                "Beyond Good and Evil",
                Some("Friedrich Nietzsche (transl. Helen Zimmern)")
            ),
            owned("Beyond Good and Evil", Some("Friedrich Nietzsche"), None)
        );
        assert_eq!(
            person("Nicolo Machiavelli; translated by W. K. Marriott").as_deref(),
            Some("Nicolo Machiavelli")
        );
        assert_eq!(
            person("Townsend, George Fyler; tr. Jorge R. Rodríguez").as_deref(),
            Some("George Fyler Townsend")
        );
        assert_eq!(
            person("Benjamin Franklin, ed. Frank Woodworth Pine").as_deref(),
            Some("Benjamin Franklin")
        );
        assert_eq!(
            person("Jacob & Wilhelm Grimm").as_deref(),
            Some("Jacob & Wilhelm Grimm")
        );
        assert_eq!(
            person("Dumas, Alexandre").as_deref(),
            Some("Alexandre Dumas")
        );
        assert_eq!(
            person("Jane Austen, Charlotte Brontë").as_deref(),
            Some("Jane Austen, Charlotte Brontë")
        );
        assert_eq!(
            named(
                "Iliad, The (Butler's translation) by Homer",
                Some("LibriVox")
            ),
            owned("The Iliad (Butler's translation)", Some("Homer"), None)
        );
        assert_eq!(
            named(
                "Odyssey, The by Homer, translated by Samuel Butler",
                Some("LibriVox")
            ),
            owned("The Odyssey", Some("Homer"), None)
        );
        assert_eq!(person("Smith, Jr.").as_deref(), Some("Smith, Jr"));
        assert_eq!(person("  "), None);
    }

    #[test]
    fn real_titles_and_authors_are_kept() {
        assert_eq!(
            named("Stand by Me", Some("Ben E. King")),
            owned("Stand by Me", Some("Ben E. King"), None)
        );
        assert_eq!(
            named("The Prince", Some("Niccolò Machiavelli")),
            owned("The Prince", Some("Niccolò Machiavelli"), None)
        );
        assert_eq!(
            named("A Tale of Two Cities", None),
            owned("A Tale of Two Cities", None, None)
        );
        assert_eq!(
            named("Plan, A by Someone", Some("LibriVox")),
            owned("A Plan", Some("Someone"), None)
        );
    }

    #[test]
    fn entities_and_spaces_are_cleaned() {
        assert_eq!(
            named("Tom &amp; Jerry&#39;s  Tales", Some("Ann &amp; Bob")),
            owned("Tom & Jerry's Tales", Some("Ann & Bob"), None)
        );
        assert_eq!(
            chapter(
                "Dedicación: Al Duque de Béjar &amp; Prólogo",
                "Don Quijote 1"
            ),
            "Dedicación: Al Duque de Béjar & Prólogo"
        );
    }

    /// Chapter titles from LibriVox and Loyal Books feeds.
    #[test]
    fn chapters_lose_the_file_numbering() {
        for (title, book_title, expected) in [
            ("01 - Chapter 01", "Pride and Prejudice", "Chapter 1"),
            ("61 - Chapter 61", "Pride and Prejudice", "Chapter 61"),
            ("00 - Dramatis Personae", "Alice", "Dramatis Personae"),
            ("12 - Alice's Evidence", "Alice", "Alice's Evidence"),
            ("01 – Laying Plans", "The Art of War", "Laying Plans"),
            ("Chapter 001-002", "Moby Dick", "Chapters 1-2"),
            ("Chapter 003", "Moby Dick", "Chapter 3"),
            (
                "Chapter 000: Etymology and Extracts",
                "Moby Dick",
                "Etymology and Extracts",
            ),
            (
                "Chapter 135 and Epilogue",
                "Moby Dick",
                "Chapter 135 and Epilogue",
            ),
            ("Chapters 1-3", "Pride and Prejudice", "Chapters 1-3"),
            ("Chapter I", "Wuthering Heights", "Chapter I"),
            ("Capítulo I-III", "Don Quijote 1", "Capítulo I-III"),
            (
                "Bk 01, ch. 01: M. Myriel",
                "Les Misérables, Volume 1",
                "Bk 1, ch. 1: M. Myriel",
            ),
            ("Moby Dick - Chapter 3", "Moby Dick", "Chapter 3"),
            ("Chapter 4 - read by Ruth Golding", "Emma", "Chapter 4"),
            ("Chapter 5 (read by Ruth Golding)", "Emma", "Chapter 5"),
            (
                " Effect of Thought on Health and the Body",
                "As a Man Thinketh",
                "Effect of Thought on Health and the Body",
            ),
            ("2 B R 0 2 B", "2 B R 0 2 B", "2 B R 0 2 B"),
            ("1984", "Stories", "1984"),
            ("Moby Dick", "Moby Dick", "Moby Dick"),
        ] {
            assert_eq!(chapter(title, book_title), expected, "{title}");
            assert_eq!(
                chapter(&chapter(title, book_title), book_title),
                expected,
                "tidying twice changes nothing: {title}"
            );
        }
    }

    #[test]
    fn tidied_names_stay_tidied() {
        for (title, author) in [
            (
                "Count of Monte Cristo, The by Alexandre Dumas (1802 - 1870)",
                Some("LibriVox"),
            ),
            (
                "Alice's Adventures in Wonderland, by Lewis Carroll",
                Some("Lewis Carroll"),
            ),
            ("Moby Dick by Herman Melville", Some("Loyal Books")),
        ] {
            let once = book(title, author);
            let twice = book(&once.title, once.author.as_deref());
            assert_eq!((once.title, once.author), (twice.title, twice.author));
        }
    }

    #[test]
    fn people_compare_however_they_are_written() {
        assert!(same_person("Lewis Carroll", "Carroll, Lewis"));
        assert!(same_person("Jane Austen (1775 - 1817)", "Jane Austen"));
        assert!(same_person("Sir Arthur Conan Doyle", "Arthur Conan Doyle"));
        assert!(!same_person("Ben E. King", "Me"));
        assert!(!same_person("", "Anyone"));
    }
}
