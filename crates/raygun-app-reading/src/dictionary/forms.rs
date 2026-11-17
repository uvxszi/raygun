//! What to look up for a selection: the selection tidied, the language its words are in, and the
//! forms tried in turn when a dictionary doesn't know the words as written (lower case, a French
//! or Italian article's elision dropped, an English word's ending taken off).

/// Longest selection looked up, in characters and in words: longer ones are sentences.
pub const MAX_CHARS: usize = 80;
pub const MAX_WORDS: usize = 6;
/// Forms tried for one selection, at most.
const MAX_FORMS: usize = 6;

/// The selection without the punctuation, quotes and spaces around it, its spaces made single.
pub fn tidy(text: &str) -> String {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    // An apostrophe or a dash at an edge is a quote mark or a dash, not part of the word.
    joined
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_owned()
}

/// Whether a tidied selection is short enough to look up.
pub fn is_lookup(text: &str) -> bool {
    !text.is_empty()
        && text.chars().count() <= MAX_CHARS
        && text.split_whitespace().count() <= MAX_WORDS
        && text.chars().any(char::is_alphabetic)
}

/// A language tag's primary subtag, in lower case (`pt-BR` → `pt`).
pub fn primary(tag: &str) -> String {
    tag.trim()
        .split(['-', '_'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// The writing systems told apart, as far as choosing a dictionary language goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Script {
    Latin,
    Cyrillic,
    Greek,
    Arabic,
    Hebrew,
    Devanagari,
    Han,
    Kana,
    Hangul,
    Thai,
    /// Digits and signs only, or a script not listed.
    Other,
}

fn script_of_char(c: char) -> Option<Script> {
    let script = match c as u32 {
        0x0041..=0x024F | 0x1E00..=0x1EFF => Script::Latin,
        0x0370..=0x03FF | 0x1F00..=0x1FFF => Script::Greek,
        0x0400..=0x052F => Script::Cyrillic,
        0x0590..=0x05FF => Script::Hebrew,
        0x0600..=0x06FF | 0x0750..=0x077F => Script::Arabic,
        0x0900..=0x097F => Script::Devanagari,
        0x0E00..=0x0E7F => Script::Thai,
        0x3040..=0x30FF => Script::Kana,
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF => Script::Han,
        0xAC00..=0xD7AF | 0x1100..=0x11FF => Script::Hangul,
        _ => return None,
    };
    (c.is_alphabetic()).then_some(script)
}

/// The script most of a text's letters are in; Japanese kana win over the Han characters they
/// come with.
pub fn script_of(text: &str) -> Script {
    let mut counts: Vec<(Script, usize)> = Vec::new();
    for script in text.chars().filter_map(script_of_char) {
        match counts.iter_mut().find(|(seen, _)| *seen == script) {
            Some((_, count)) => *count += 1,
            None => counts.push((script, 1)),
        }
    }
    if counts.iter().any(|(script, _)| *script == Script::Kana) {
        return Script::Kana;
    }
    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map_or(Script::Other, |(script, _)| script)
}

/// Whether words of language `lang` (a primary subtag) are written in `script`.
fn writes(lang: &str, script: Script) -> bool {
    let not_latin: &[(&str, Script)] = &[
        ("ru", Script::Cyrillic),
        ("uk", Script::Cyrillic),
        ("be", Script::Cyrillic),
        ("bg", Script::Cyrillic),
        ("mk", Script::Cyrillic),
        ("kk", Script::Cyrillic),
        ("ky", Script::Cyrillic),
        ("mn", Script::Cyrillic),
        ("tg", Script::Cyrillic),
        ("tt", Script::Cyrillic),
        ("el", Script::Greek),
        ("grc", Script::Greek),
        ("ar", Script::Arabic),
        ("fa", Script::Arabic),
        ("ur", Script::Arabic),
        ("ps", Script::Arabic),
        ("he", Script::Hebrew),
        ("yi", Script::Hebrew),
        ("hi", Script::Devanagari),
        ("mr", Script::Devanagari),
        ("ne", Script::Devanagari),
        ("sa", Script::Devanagari),
        ("th", Script::Thai),
        ("ko", Script::Hangul),
        ("zh", Script::Han),
    ];
    match script {
        Script::Other => true,
        // Serbian is written in both.
        Script::Cyrillic if lang == "sr" => true,
        Script::Han => matches!(lang, "zh" | "ja" | "ko"),
        Script::Kana => lang == "ja",
        Script::Latin => !(lang == "ja" || not_latin.iter().any(|(known, _)| *known == lang)),
        script => not_latin
            .iter()
            .any(|(known, written)| *known == lang && *written == script),
    }
}

/// The language a script's words are most likely in, when nothing else says.
fn usual_language(script: Script) -> Option<&'static str> {
    Some(match script {
        Script::Latin => "en",
        Script::Cyrillic => "ru",
        Script::Greek => "el",
        Script::Arabic => "ar",
        Script::Hebrew => "he",
        Script::Devanagari => "hi",
        Script::Han => "zh",
        Script::Kana => "ja",
        Script::Hangul => "ko",
        Script::Thai => "th",
        Script::Other => return None,
    })
}

/// The language a selection's words are looked up in: the one chosen in Settings, else the
/// book's, else the app's, as long as the words are written in its script (an English name in a
/// Japanese book is English); else the script's usual language.
pub fn choose_language(text: &str, chosen: &str, book: Option<&str>, app: &str) -> String {
    let chosen = primary(chosen);
    if !chosen.is_empty() {
        return chosen;
    }
    let script = script_of(text);
    let book = book
        .map(primary)
        .filter(|tag| !tag.is_empty() && tag != "und");
    let app = Some(primary(app)).filter(|tag| !tag.is_empty());
    for tag in [book.clone(), app.clone()].into_iter().flatten() {
        if writes(&tag, script) {
            return tag;
        }
    }
    usual_language(script)
        .map(str::to_owned)
        .or(book)
        .or(app)
        .unwrap_or_else(|| "en".to_owned())
}

/// Articles French, Italian and Catalan join to the next word with an apostrophe.
fn elisions(lang: &str) -> &'static [&'static str] {
    match lang {
        "fr" => &[
            "jusqu", "lorsqu", "puisqu", "quoiqu", "qu", "l", "d", "j", "m", "n", "s", "t", "c",
        ],
        "it" => &[
            "dell", "all", "dall", "nell", "sull", "coll", "quest", "quell", "un", "l", "d", "c",
        ],
        "ca" => &["l", "d", "s", "n", "m", "t"],
        _ => &[],
    }
}

fn without_elision(word: &str, lang: &str) -> Option<String> {
    let lower = word.to_lowercase();
    for article in elisions(lang) {
        for mark in ['\'', '’'] {
            let prefix = format!("{article}{mark}");
            if let Some(rest) = lower.strip_prefix(&prefix)
                && rest.chars().count() > 1
            {
                // The rest as written, not lowered: a name keeps its capital.
                let start = word.len().saturating_sub(rest.len());
                return Some(
                    word.get(start..)
                        .filter(|kept| kept.to_lowercase() == rest)
                        .unwrap_or(rest)
                        .to_owned(),
                );
            }
        }
    }
    None
}

fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u')
}

/// `stopp` → `stop`: a stem whose last consonant was doubled before an ending.
fn undoubled(stem: &str) -> Option<String> {
    let chars: Vec<char> = stem.chars().collect();
    let [.., before, last, again] = chars.as_slice() else {
        return None;
    };
    (last == again && !is_vowel(*last) && is_vowel(*before) && !matches!(last, 'l' | 's' | 'z'))
        .then(|| chars[..chars.len() - 1].iter().collect())
}

/// What an English word may be before its ending: plurals, past forms, `-ing`, comparatives and
/// `-ly` adverbs, the likeliest first.
fn english_stems(word: &str) -> Vec<String> {
    let mut stems: Vec<String> = Vec::new();
    let mut add = |stem: String| {
        if stem.chars().count() >= 2 && !stems.contains(&stem) {
            stems.push(stem);
        }
    };
    let with = |stem: &str, end: &str| format!("{stem}{end}");
    if let Some(stem) = word
        .strip_suffix("'s")
        .or_else(|| word.strip_suffix("’s"))
        .or_else(|| word.strip_suffix("s'"))
    {
        add(stem.to_owned());
    }
    for (ending, replacement) in [("ies", "y"), ("ied", "y"), ("ier", "y"), ("iest", "y")] {
        if let Some(stem) = word.strip_suffix(ending) {
            add(with(stem, replacement));
        }
    }
    if let Some(stem) = word.strip_suffix("ily") {
        add(with(stem, "y"));
    }
    if let Some(stem) = word.strip_suffix("ves") {
        add(with(stem, "f"));
        add(with(stem, "fe"));
    }
    if let Some(stem) = word.strip_suffix("ying") {
        add(with(stem, "ie"));
    }
    for ending in ["ing", "ed", "er", "est"] {
        if let Some(stem) = word.strip_suffix(ending) {
            if let Some(single) = undoubled(stem) {
                add(single);
            }
            add(stem.to_owned());
            add(with(stem, "e"));
        }
    }
    if let Some(stem) = word.strip_suffix("es")
        && ["s", "x", "z", "ch", "sh", "o"]
            .iter()
            .any(|end| stem.ends_with(end))
    {
        add(stem.to_owned());
    }
    if let Some(stem) = word.strip_suffix('s')
        && !(stem.ends_with('s') || stem.ends_with('u') || stem.ends_with('i'))
    {
        add(stem.to_owned());
    }
    if let Some(stem) = word.strip_suffix("ly") {
        add(stem.to_owned());
    }
    stems
}

/// The forms to look up for a tidied selection in `lang`, in turn: as selected, in lower case,
/// without an elided article, then (English, one word) without its ending.
pub fn candidates(query: &str, lang: &str) -> Vec<String> {
    let mut forms: Vec<String> = Vec::new();
    let mut add = |form: String| {
        if !form.is_empty() && !forms.contains(&form) {
            forms.push(form);
        }
    };
    add(query.to_owned());
    let lower = query.to_lowercase();
    add(lower.clone());
    if let Some(rest) = without_elision(query, lang) {
        add(rest.clone());
        add(rest.to_lowercase());
    }
    let one_word = !query.contains(char::is_whitespace);
    if lang == "en" && one_word && lower.chars().count() > 3 {
        for stem in english_stems(&lower) {
            add(stem);
        }
    }
    forms.truncate(MAX_FORMS);
    forms
}
